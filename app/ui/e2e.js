// End-to-end privacy overlay: the settings page, the per-chat lock button and
// safety-number verification. Backend commands live in app/src-tauri/src/e2e.rs.
"use strict";

(() => {
  const { Bridge, UI, h } = window.MelloGram;
  const $ = (id) => document.getElementById(id);
  const invoke = (cmd, args) => Bridge.invoke(cmd, args);

  async function call(cmd, args, ok) {
    try {
      const result = await invoke(cmd, args);
      if (ok) UI.toast(ok, "success");
      return result;
    } catch (e) {
      UI.toast(String(e), "error");
      throw e;
    }
  }

  function safetyBlock(state) {
    if (!state.safety_number) {
      return h("p", { class: "mg-hint", text: "Add their overlay key to see a safety number." });
    }
    return h("div", {},
      h("p", { class: "mg-hint", text: "Compare these 60 digits with them over a call or in person. They must match on both sides." }),
      h("pre", { class: "mg-raw mg-e2e-safety", text: state.safety_number }));
  }

  // ---- the lock button in the composer ---------------------------------------

  // One shape for both overlays: a ChatView for people, a GroupView adapted to
  // the same fields for groups and channels.
  async function stateFor(chatId) {
    if (chatId < 0) {
      const group = await invoke("e2e_group_state", { chatId });
      return {
        chat_id: chatId,
        enabled: group.enabled,
        ready: group.has_sender_key,
        rotation_needed: group.rotation_needed,
        verified: false,
        safety_number: null,
        peer_key_known: false,
        group,
      };
    }
    return invoke("e2e_chat_state", { chatId });
  }

  async function paint(chatId) {
    const button = $("e2e-toggle");
    if (!button || chatId == null) return;
    try {
      const state = await stateFor(chatId);
      button.dataset.state = !state.enabled
        ? "off"
        : state.ready && !state.rotation_needed
          ? "on"
          : "waiting";
      button.title = !state.enabled
        ? "Send end-to-end encrypted (MelloGram overlay)"
        : state.rotation_needed
          ? "Group membership changed — rotate the key before sending"
          : state.ready
            ? "End-to-end encrypted — click for safety number and settings"
            : state.enabled && chatId < 0
              ? "Group encryption is on; use the lock to share your key"
              : "Encryption is on; waiting for the other person's key";
    } catch (e) {
      button.dataset.state = "off";
      button.title = "End-to-end overlay";
    }
  }

  async function sync(chat) {
    const button = $("e2e-toggle");
    if (!button) return;
    button.hidden = !chat;
    if (chat) await paint(chat.id);
  }

  async function toggle() {
    const chat = window.MelloGram.App?.currentChat();
    if (!chat) return;
    let state;
    try {
      state = await stateFor(chat.id);
    } catch (e) {
      UI.toast(String(e), "error");
      return;
    }
    if (chat.id < 0) return groupDialog(chat, state);
    if (state.enabled) return details(chat, state);
    if (!state.peer_key_known) return askKey(chat);
    try {
      const next = await invoke("e2e_set_chat", { chatId: chat.id, enabled: true });
      await paint(chat.id);
      UI.toast("Encryption is on. Your next message starts the encrypted session.", "success");
      details(chat, next);
    } catch (e) {
      UI.toast(String(e), "error");
    }
  }

  // ---- groups (sender keys) --------------------------------------------------

  function groupDialog(chat, state) {
    const group = state.group || { senders: [], pairwise_peers: 0 };
    const senders = group.senders || [];
    if (chat.kind === "channel" && !state.enabled) {
      // A broadcast channel has subscribers, not members: there is no one to
      // deliver sender keys to, so the overlay must not claim to cover it.
      const close = UI.modal(`Encryption · ${chat.title}`, [
        h("p", { text: "Channels are not covered by the overlay." }),
        h("p", { class: "mg-hint", text: "Channel subscribers cannot receive sender keys through one-to-one encrypted chats, so encrypting here would be a claim without substance. The overlay works in groups and supergroups." }),
        h("div", { class: "mg-toolbar" }, h("button", { class: "mg-btn", text: "Close", onclick: () => close() })),
      ]);
      return;
    }
    const close = UI.modal(`Group encryption · ${chat.title}`, [
      h("p", {
        text: !state.enabled
          ? "Sender keys: each member encrypts a message once for the whole group, and every member's key is delivered inside your one-to-one encrypted chat with them."
          : state.rotation_needed
            ? "Membership changed. Rotate the key and share it, or the person who left could still read what you send."
            : "On for this group.",
      }),
      !state.enabled
        ? h("p", { class: "mg-hint", text: "Members who have not exchanged one-to-one overlay keys with you will see ciphertext until they do." })
        : h("p", { class: "mg-hint", text: `${senders.length} member key${senders.length === 1 ? "" : "s"} on this device · ${group.members || 0} member${group.members === 1 ? "" : "s"} seen · ${group.pairwise_peers} contact${group.pairwise_peers === 1 ? "" : "s"} can receive keys.` }),
      senders.length
        ? h("div", { class: "mg-raw" }, senders.map((sender) =>
            h("div", { class: "mg-hint", text: `User ${sender.sender_id} · key ${sender.key}${sender.verified ? " · verified" : ""}` })))
        : null,
      h("div", { class: "mg-toolbar" },
        !state.enabled
          ? h("button", {
              class: "mg-btn mg-primary", text: "Turn on for this group",
              onclick: async () => {
                try {
                  await invoke("e2e_group_enable", { chatId: chat.id });
                  close();
                  await paint(chat.id);
                  await shareKeys(chat);
                } catch (e) { UI.toast(String(e), "error"); }
              },
            })
          : h("button", { class: "mg-btn mg-primary", text: "Share my key", onclick: () => shareKeys(chat, close) }),
        state.enabled ? h("button", { class: "mg-btn", text: "Ask for missing keys", onclick: () => requestKeys(chat) }) : null,
        state.enabled ? h("button", {
          class: "mg-btn", text: "Rotate key",
          onclick: async () => {
            const sure = await UI.confirm(
              "Rotate this group's key?",
              "Your next messages use a new key. Share it with the group or nobody can read them.",
              "Rotate",
            );
            if (!sure) return;
            try {
              await invoke("e2e_group_rotate", { chatId: chat.id });
              close();
              await paint(chat.id);
              await shareKeys(chat);
            } catch (e) { UI.toast(String(e), "error"); }
          },
        }) : null,
        state.enabled ? h("button", {
          class: "mg-btn mg-danger", text: "Turn encryption off",
          onclick: async () => {
            try {
              await invoke("e2e_group_disable", { chatId: chat.id });
              close();
              await paint(chat.id);
              UI.toast("Group encryption is off for this chat.", "info");
            } catch (e) { UI.toast(String(e), "error"); }
          },
        }) : null,
        h("button", { class: "mg-btn", text: "Close", onclick: () => close() })),
      h("p", { class: "mg-hint", text: "Text only. Telegram still sees the member list and who sent what." }),
    ]);
  }

  /// Delivers our sender key to every member we have a pairwise session with.
  async function shareKeys(chat, close) {
    try {
      const report = await invoke("e2e_group_share", { chatId: chat.id });
      if (close) close();
      await paint(chat.id);
      const skipped = report.skipped.length;
      const rest = report.truncated ? " This group is larger than one pass covers; the rest get the key on the next one." : "";
      UI.toast(
        report.sent.length
          ? `Key sent to ${report.sent.length} member${report.sent.length === 1 ? "" : "s"}${skipped ? `; ${skipped} could not be reached` : ""}.${rest}`
          : "No member could be reached: exchange one-to-one overlay keys with them first.",
        skipped ? "warning" : "success",
      );
    } catch (e) { UI.toast(String(e), "error"); }
  }

  /// Asks members we have no key from (but can reach) to send theirs.
  async function requestKeys(chat) {
    try {
      const report = await invoke("e2e_group_request_keys", { chatId: chat.id });
      await paint(chat.id);
      const rest = report.truncated ? " This group is larger than one pass covers; ask again for the rest." : "";
      UI.toast(
        report.sent.length
          ? `Asked ${report.sent.length} member${report.sent.length === 1 ? "" : "s"} for their key.${rest}`
          : "Nobody to ask: every reachable member's key is here, or none of them have one-to-one encryption with you.",
        report.sent.length ? "success" : "info",
      );
    } catch (e) { UI.toast(String(e), "error"); }
  }

  function askKey(chat) {
    const area = h("textarea", { rows: 3, placeholder: "mg-e2e-key:v1:…" });
    const close = UI.modal(`Encrypt this chat with ${chat.title}`, [
      h("p", { class: "mg-muted", text: "Both people need MelloGram. Send them your key from Settings → End-to-End Overlay, and paste theirs here once." }),
      area,
      h("div", { class: "mg-toolbar" },
        h("button", {
          class: "mg-btn", text: "Paste from clipboard",
          onclick: async () => {
            try { area.value = await navigator.clipboard.readText(); }
            catch (e) { UI.toast("Clipboard access was blocked — paste with Ctrl+V.", "warning"); }
          },
        }),
        h("button", {
          class: "mg-btn", text: "Copy my key",
          onclick: async () => {
            const key = await call("e2e_identity");
            await navigator.clipboard.writeText(key)
              .then(() => UI.toast("Your key is on the clipboard — send it to them.", "success"))
              .catch(() => UI.toast("Copy failed; your key is in Settings → End-to-End Overlay.", "warning"));
          },
        })),
      h("div", { class: "mg-toolbar" },
        h("button", {
          class: "mg-btn mg-primary", text: "Add key & turn on",
          onclick: async () => {
            if (!area.value.trim()) {
              UI.toast("Paste their key first.", "warning");
              return;
            }
            try {
              await invoke("e2e_import_peer", { peerId: chat.id, key: area.value.trim() });
              const next = await invoke("e2e_set_chat", { chatId: chat.id, enabled: true });
              close();
              await paint(chat.id);
              UI.toast("Encryption is on for this chat.", "success");
              details(chat, next);
            } catch (e) {
              UI.toast(String(e), "error");
            }
          },
        }),
        h("button", { class: "mg-btn", text: "Cancel", onclick: () => close() })),
    ]);
  }

  function details(chat, state) {
    const digits = h("input", { type: "text", placeholder: "Their 60 digits", autocomplete: "off" });
    const close = UI.modal(`End-to-end overlay · ${chat.title}`, [
      h("p", { text: state.enabled ? "On for this chat." : "Off for this chat." }),
      h("p", { class: "mg-hint", text: state.verified ? "This contact is verified." : "Not verified yet: compare the safety number below." }),
      safetyBlock(state),
      h("h3", { text: "Mark as verified" }),
      h("p", { class: "mg-hint", text: "Type the digits you read out, without spaces, to pin this contact." }),
      digits,
      h("div", { class: "mg-toolbar" },
        h("button", {
          class: "mg-btn", text: "Check digits",
          onclick: async () => {
            try {
              const ok = await invoke("e2e_verify_peer", { peerId: chat.id, digits: digits.value });
              if (ok) {
                UI.toast("Safety number matches — contact verified.", "success");
                close();
                await paint(chat.id);
              } else {
                UI.toast("Those digits do not match. Do not send anything sensitive until they do.", "error");
              }
            } catch (e) { UI.toast(String(e), "error"); }
          },
        }),
        h("button", {
          class: "mg-btn", text: "Copy safety number",
          onclick: () => navigator.clipboard.writeText(state.safety_number || "")
            .then(() => UI.toast("Safety number copied.", "success"))
            .catch(() => UI.toast("Copy failed.", "warning")),
        })),
      h("p", { class: "mg-hint", text: "The overlay covers text messages. Attachments and edits stay off while it is enabled." }),
      h("div", { class: "mg-toolbar" },
        h("button", {
          class: "mg-btn mg-danger", text: "Turn encryption off",
          onclick: async () => {
            try {
              await invoke("e2e_set_chat", { chatId: chat.id, enabled: false });
              close();
              await paint(chat.id);
              UI.toast("Encryption is off for this chat.", "info");
            } catch (e) { UI.toast(String(e), "error"); }
          },
        }),
        h("button", { class: "mg-btn", text: "Close", onclick: () => close() })),
    ]);
  }

  // ---- settings page ----------------------------------------------------------

  function peerCard(peer) {
    return h("div", { class: "mg-field-row" },
      h("div", {},
        h("label", { text: `User ${peer.peer_id}${peer.verified ? " · verified" : ""}` }),
        h("div", { class: "mg-hint", text: peer.fingerprint ? `Key ${peer.fingerprint}${peer.has_session ? " · session open" : ""}` : "No key yet" }),
        peer.safety_number ? h("pre", { class: "mg-raw mg-e2e-safety", text: peer.safety_number }) : null),
      h("button", {
        class: "mg-btn", text: "Forget",
        onclick: async () => {
          if (!(await UI.confirm("Forget this contact's overlay key?", "Their session and verification are removed from this device.", "Forget"))) return;
          await call("e2e_forget_peer", { peerId: peer.peer_id }, "Contact forgotten.");
          window.MelloGram.Settings.show("e2e");
        },
      }));
  }

  async function page() {
    const status = await invoke("e2e_status");
    const keyArea = h("textarea", { rows: 3, readonly: true });
    keyArea.value = status.key || "";
    const rows = [
      h("h2", { text: "End-to-End Overlay" }),
      h("p", { class: "mg-muted", text: "An extra layer of encryption on top of Telegram. Telegram's servers, and anyone reading them, only see ciphertext. Both people need MelloGram. It covers text in one-to-one chats and, with sender keys, in groups." }),
      h("h3", { text: "Your key" }),
      keyArea,
      h("div", { class: "mg-toolbar" },
        h("button", {
          class: "mg-btn", text: status.has_identity ? "Copy my key" : "Create my key",
          onclick: async () => {
            const key = await call("e2e_identity");
            keyArea.value = key;
            await navigator.clipboard.writeText(key)
              .then(() => UI.toast("Overlay key copied — send it to the person you want to encrypt with.", "success"))
              .catch(() => UI.toast("Copy failed; select the key above and copy it.", "warning"));
          },
        }),
        status.has_identity ? h("button", {
          class: "mg-btn mg-danger", text: "Reset identity",
          onclick: async () => {
            if (!(await UI.confirm("Reset your overlay identity?", "Every encrypted session, contact key and safety number is thrown away. Contacts must exchange keys again.", "Reset"))) return;
            await call("e2e_reset_identity", {}, "Overlay identity reset.");
            window.MelloGram.Settings.show("e2e");
          },
        }) : null,
        h("button", {
          class: "mg-btn", text: "Check group membership",
          onclick: async () => {
            try {
              const flagged = await invoke("e2e_group_sweep");
              UI.toast(
                flagged
                  ? `${flagged} group${flagged === 1 ? "" : "s"} lost a member: rotate the key before sending.`
                  : "No group membership changes found.",
                flagged ? "warning" : "success",
              );
              window.MelloGram.Settings.show("e2e");
            } catch (e) { UI.toast(String(e), "error"); }
          },
        }),
        h("button", {
          class: "mg-btn", text: `Forget decrypted messages (${status.decrypted_messages})`,
          onclick: async () => {
            const dropped = await call("e2e_clear_history");
            UI.toast(`Removed ${dropped} decrypted messages from this device.`, "success");
            window.MelloGram.Settings.show("e2e");
          },
        })),
      h("p", { class: "mg-hint", text: `Fingerprint: ${status.fingerprint || "—"}. Your key and every session are stored encrypted with a key unique to this install.` }),
      h("h3", { text: "Contacts" }),
    ];
    if (!status.peers.length) {
      rows.push(h("p", { class: "mg-muted", text: "No contact keys yet. Copy your key to a friend, then paste theirs from a one-to-one chat's lock button." }));
    }
    for (const peer of status.peers) rows.push(peerCard(peer));
    rows.push(h("h3", { text: "What it does not protect" }),
      h("p", { class: "mg-hint", text: "Metadata (who talks to whom, when and how often), your account, media, and anything on a device that is already unlocked. See docs/E2E.md." }));
    return rows;
  }

  // ---- wiring -----------------------------------------------------------------

  document.getElementById("e2e-toggle")?.addEventListener("click", toggle);

  window.MelloGram.Pages = window.MelloGram.Pages || {};
  window.MelloGram.Pages.e2e = page;
  window.MelloGram.E2E = { sync, toggle, page };
})();
