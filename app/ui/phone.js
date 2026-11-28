// Phone shell: a separate, touch-first layout for the Android/iOS app (and
// touch phones using the web client). It is not the desktop layout squeezed
// down: the chat list gets a Material-style app bar, bottom navigation and a
// navigation drawer; chats open as full-screen pages with their own app bar;
// settings become a two-level list → page flow; the info panel and menus are
// sheets. Desktop (and narrow desktop windows) never get body.mg-phone.
//
// The shell only moves and wraps nodes the rest of the UI builds, so plugin
// buttons, hooks and settings keep working unchanged.
"use strict";

(() => {
  const { h } = window.MelloGram;
  const $ = (id) => document.getElementById(id);
  const visible = (el) => !!el && !el.hidden;
  const body = document.body;
  const app = $("app");

  const phoneQuery = matchMedia("(max-width: 700px)");
  const isPhone = () => phoneQuery.matches && (body.classList.contains("mg-mobile-app") || body.classList.contains("mg-touch"));

  // ---- chat list app bar ------------------------------------------------------------

  const title = h("div", { class: "mg-ph-title", text: "MelloGram" });
  const searchBtn = h("button", { class: "mg-icon-btn mg-ph-action", title: "Search", "aria-label": "Search chats", text: "⌕" });
  const menuBtn = h("button", { class: "mg-icon-btn mg-ph-action", title: "Menu", "aria-label": "Open menu", text: "☰" });
  const appBar = h("header", { class: "mg-ph-appbar", id: "ph-appbar" }, menuBtn, title, searchBtn);

  const search = $("chat-search");
  searchBtn.addEventListener("click", () => {
    const on = !app.classList.contains("mg-ph-searching");
    app.classList.toggle("mg-ph-searching", on);
    if (on) search.focus();
    else { search.value = ""; search.dispatchEvent(new Event("input")); }
  });

  // ---- chat filter tabs (All / Unread / Groups / Channels / Personal) ----------------

  const filters = [
    ["all", "All", () => true],
    ["unread", "Unread", (el) => !!el.querySelector(".mg-unread")],
    ["personal", "Personal", (el) => kindOf(el) === "user"],
    ["groups", "Groups", (el) => ["group", "supergroup"].includes(kindOf(el))],
    ["channels", "Channels", (el) => kindOf(el) === "channel"],
  ];
  let filter = "all";
  const kinds = new Map(); // chat id -> kind, filled by the chatRender hook
  const kindOf = (el) => kinds.get(el.dataset.chatId);
  window.MelloGram.Hooks.chatRender.add((chat, el) => { kinds.set(String(chat.id), chat.kind); applyFilterTo(el); });

  const tabs = h("nav", { class: "mg-ph-tabs", "aria-label": "Chat filters" },
    ...filters.map(([key, label]) => h("button", {
      class: key === filter ? "mg-active" : "", "data-filter": key, text: label,
      onclick: () => { filter = key; for (const b of tabs.children) b.classList.toggle("mg-active", b.dataset.filter === key); applyFilter(); },
    })));
  function applyFilterTo(el) {
    const test = filters.find(([k]) => k === filter)[2];
    el.classList.toggle("mg-ph-filtered", !test(el));
  }
  function applyFilter() { for (const el of $("chat-list").children) applyFilterTo(el); }

  // ---- bottom navigation ----------------------------------------------------------------

  const navItem = (icon, label, onclick) => h("button", { class: "mg-ph-nav-item", onclick, "aria-label": label },
    h("span", { class: "mg-ph-nav-icon", text: icon }), h("span", { class: "mg-ph-nav-label", text: label }));
  const navChats = navItem("💬", "Chats", () => { closeDrawer(); window.MelloGram.Settings.close(); $("palette").hidden = true; $("chat-list").scrollTo({ top: 0, behavior: "smooth" }); });
  const navActions = navItem("⚡", "Actions", () => $("open-palette").click());
  const navStudio = navItem("🎨", "Studio", () => window.MelloGram.Settings.open("appearance").catch?.(() => {}));
  const navSettings = navItem("⚙", "Settings", () => openSettingsList());
  navChats.classList.add("mg-active");
  const bottomNav = h("nav", { class: "mg-ph-bottomnav", "aria-label": "Main" }, navChats, navActions, navStudio, navSettings);

  // ---- navigation drawer -----------------------------------------------------------------

  const drawerPlugins = h("div", { class: "mg-ph-drawer-plugins" });
  const drawerItem = (icon, label, onclick) => h("button", { class: "mg-ph-drawer-item", onclick: () => { closeDrawer(); onclick(); } },
    h("span", { class: "mg-ph-drawer-icon", text: icon }), h("span", { text: label }));
  const drawer = h("aside", { class: "mg-ph-drawer", "aria-label": "Menu", hidden: true },
    h("div", { class: "mg-ph-drawer-head" },
      h("img", { src: "logo.png", alt: "" }),
      h("div", {}, h("b", { text: "MelloGram" }), h("div", { class: "mg-muted", id: "ph-drawer-sub", text: "Your Telegram, your way" }))),
    drawerItem("⚙", "Settings", () => openSettingsList()),
    drawerItem("🎨", "Appearance", () => window.MelloGram.Settings.open("appearance")),
    drawerItem("🧩", "Plugins", () => window.MelloGram.Settings.open("plugins")),
    drawerItem("🔔", "Notifications", () => window.MelloGram.Settings.open("notifications")),
    drawerItem("🛡", "Privacy & security", () => window.MelloGram.Settings.open("privacy")),
    drawerItem("⚡", "Command palette", () => $("open-palette").click()),
    h("div", { class: "mg-ph-drawer-sep", text: "Plugin shortcuts" }),
    drawerPlugins);
  const scrim = h("div", { class: "mg-ph-scrim", hidden: true, onclick: () => closeDrawer() });

  // The drawer sits above every overlay (z-index 61). Opening it over the
  // login form, the lock screen or a dialog would cover the control the user
  // is trying to reach, so it belongs to the signed-in chat list only.
  function drawerAllowed() {
    if (!isPhone() || app.classList.contains("mg-show-chat")) return false;
    if (visible($("login")) || visible($("lock"))) return false;
    if (document.querySelector(".mg-overlay:not([hidden])")) return false;
    if (visible(document.querySelector(".mg-info-panel"))) return false;
    if (visible($("context-menu"))) return false;
    return true;
  }
  function openDrawer() {
    if (!drawerAllowed()) return;
    drawer.hidden = false;
    scrim.hidden = false;
    // Force a reflow so the slide transition starts from the closed position,
    // then add the class synchronously. requestAnimationFrame can be paused in
    // a background webview, which would leave the drawer hidden off-screen.
    void drawer.offsetWidth;
    drawer.classList.add("mg-open");
  }
  function closeDrawer() { drawer.classList.remove("mg-open"); scrim.hidden = true; drawer.hidden = true; }
  menuBtn.addEventListener("click", openDrawer);
  // If the session ends while the drawer is open, the login form must not be
  // hidden behind it.
  new MutationObserver(() => { if (!$("login").hidden) closeDrawer(); })
    .observe($("login"), { attributes: true, attributeFilter: ["hidden"] });
  // Swipe from the left edge opens the drawer on the chat list.
  let edge = null;
  document.addEventListener("touchstart", (e) => {
    const t = e.touches[0];
    edge = t && drawerAllowed() && t.clientX < 24 ? t.clientX : null;
  }, { passive: true });
  document.addEventListener("touchmove", (e) => {
    if (edge != null && e.touches[0].clientX - edge > 60) { edge = null; openDrawer(); }
  }, { passive: true });

  // ---- chat page app bar -------------------------------------------------------------------
  //
  // The chat header gets the chat's avatar (cloned from the list item, so photos
  // and colours match) and an overflow button that opens the info sheet.

  const headerAvatar = h("div", { class: "mg-ph-chat-avatar", "aria-hidden": "true" });
  const overflow = h("button", { class: "mg-icon-btn mg-ph-overflow", title: "Chat info", "aria-label": "Chat info", text: "⋮",
    onclick: () => $("chat-title")?.parentElement?.click() });
  window.MelloGram.Hooks.chatOpen.add((chat) => {
    const src = document.querySelector(`.mg-chat-item[data-chat-id="${CSS.escape(String(chat.id))}"] .mg-avatar`);
    headerAvatar.replaceChildren();
    if (src) {
      const clone = src.cloneNode(true);
      clone.classList.remove("mg-avatar-online");
      headerAvatar.append(clone);
    }
  });

  // ---- composer: plugin buttons fold behind "+" ----------------------------------------------

  const composerRow = document.querySelector(".mg-composer-row");
  const plus = h("button", { class: "mg-icon-btn mg-ph-plus", title: "More", "aria-label": "Attachments and tools", text: "+",
    onclick: () => composerRow.classList.toggle("mg-ph-tools-open") });
  document.addEventListener("click", (e) => {
    if (!e.target.closest?.(".mg-composer-row")) composerRow?.classList.remove("mg-ph-tools-open");
  });

  // ---- settings: list → page ----------------------------------------------------------------

  const settingsCard = document.querySelector(".mg-settings");
  const pageBar = h("div", { class: "mg-ph-pagebar" },
    h("button", { class: "mg-icon-btn", "aria-label": "Back to settings", text: "←", onclick: () => settingsCard.classList.add("mg-ph-list") }),
    h("b", { class: "mg-ph-pagebar-title" }));
  const listBar = h("div", { class: "mg-ph-pagebar mg-ph-listbar" },
    h("button", { class: "mg-icon-btn", "aria-label": "Close settings", text: "←", onclick: () => window.MelloGram.Settings.close() }),
    h("b", { text: "Settings" }));
  async function openSettingsList() {
    await window.MelloGram.Settings.open();
    settingsCard.classList.add("mg-ph-list");
  }
  $("settings-nav").addEventListener("click", (e) => {
    const btn = e.target.closest("button");
    if (!btn || !isPhone()) return;
    pageTitle.textContent = btn.textContent;
    settingsCard.classList.remove("mg-ph-list");
  });
  // Opening a specific page (drawer, Studio tab) goes straight to it; the nav is
  // re-rendered on every page change, so keep the page bar title in step.
  const pageTitle = pageBar.querySelector(".mg-ph-pagebar-title");
  new MutationObserver(() => {
    const t = $("settings-nav").querySelector("button.mg-active")?.textContent || "";
    if (pageTitle.textContent !== t) pageTitle.textContent = t;
  }).observe($("settings-nav"), { childList: true });
  new MutationObserver(() => {
    if ($("settings").hidden) settingsCard.classList.remove("mg-ph-list");
  }).observe($("settings"), { attributes: true, attributeFilter: ["hidden"] });

  // ---- mount / unmount -------------------------------------------------------------------------

  const sidebar = document.querySelector(".mg-sidebar");
  const chatHeader = document.querySelector(".mg-chat-header");
  const headerButtons = $("header-buttons");
  const titlebar = document.querySelector(".mg-titlebar");
  const composerButtons = $("composer-buttons");
  let mounted = false;

  function mount() {
    if (mounted) return;
    mounted = true;
    body.classList.add("mg-phone");
    sidebar.prepend(appBar, tabs);
    sidebar.append(bottomNav);
    body.append(scrim, drawer);
    drawerPlugins.append(headerButtons); // plugin header buttons live in the drawer
    chatHeader.insertBefore(headerAvatar, $("chat-back").nextSibling);
    chatHeader.append(overflow);
    composerRow.insertBefore(plus, composerButtons);
    $("composer").dataset.desktopPlaceholder ??= $("composer").placeholder;
    $("composer").placeholder = "Message";
    settingsCard.prepend(listBar, pageBar);
    applyFilter();
  }
  function unmount() {
    if (!mounted) return;
    mounted = false;
    body.classList.remove("mg-phone");
    closeDrawer();
    for (const n of [appBar, tabs, bottomNav, scrim, drawer, headerAvatar, overflow, plus, listBar, pageBar]) n.remove();
    titlebar.insertBefore(headerButtons, $("open-palette"));
    if ($("composer").dataset.desktopPlaceholder) $("composer").placeholder = $("composer").dataset.desktopPlaceholder;
    settingsCard.classList.remove("mg-ph-list");
    app.classList.remove("mg-ph-searching");
    filter = "all";
    for (const el of $("chat-list").children) el.classList.remove("mg-ph-filtered");
  }
  const apply = () => (isPhone() ? mount() : unmount());
  phoneQuery.addEventListener?.("change", apply);
  new MutationObserver(apply).observe(body, { attributes: true, attributeFilter: ["class"] });
  apply();

  // Back (hardware button / gesture) handling lives in mobile.js; it closes the
  // drawer through its backdrop click like any other overlay.
  window.MelloGram.Phone = { isPhone, openDrawer, closeDrawer };
})();
