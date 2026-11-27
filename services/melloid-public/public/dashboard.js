"use strict";

const byId = (id) => document.getElementById(id);
let importedProfile = null;

function showMessage(message, isError = false) {
  const result = byId("result");
  result.textContent = message;
  result.classList.toggle("error", isError);
}
async function request(url, options = {}) {
  const response = await fetch(url, { credentials: "same-origin", ...options });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(body.error || `Request failed: ${response.status}`);
  return body;
}
function renderSummary(profile) {
  const box = byId("profile-summary");
  box.hidden = false;
  box.replaceChildren();
  const title = document.createElement("strong");
  title.textContent = profile.display_name || "Unnamed profile";
  const details = document.createElement("p");
  details.textContent = `@${profile.slug} · ${profile.visibility} · ${(profile.widgets || []).length} widgets · ${(profile.connections || []).length} connections`;
  box.append(title, details);
}
async function loadConnections() {
  const list = byId("connections-list");
  try {
    const result = await request("/api/v1/me/connections");
    list.replaceChildren();
    const connections = result.connections || [];
    for (const provider of ["github", "discord", "steam"]) {
      const record = connections.find((item) => item.provider === provider);
      const connectButton = byId(`connect-${provider}`);
      if (connectButton) connectButton.hidden = Boolean(record);
      const card = document.createElement("article");
      card.className = "connection-card";
      const meta = document.createElement("div");
      const title = document.createElement("strong");
      title.textContent = provider === "github" ? "GitHub" : provider === "steam" ? "Steam" : "Discord";
      const detail = document.createElement("p");
      detail.className = "muted";
      detail.textContent = record
        ? `${record.display_name} · ${record.username} · verified ${new Date(record.verified_at).toLocaleDateString()}`
        : "Not connected";
      meta.append(title, detail);
      card.append(meta);
      if (record) {
        const actions = document.createElement("div");
        actions.className = "actions";
        const visibility = document.createElement("button");
        visibility.className = "button ghost";
        visibility.textContent = record.visible ? "Hide from profile" : "Show on profile";
        visibility.addEventListener("click", async () => {
          visibility.disabled = true;
          try {
            await request(`/api/v1/me/connections/${provider}`, {
              method: "PUT",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ visible: !record.visible }),
            });
            await loadConnections();
            showMessage(`${title.textContent} visibility updated.`);
          } catch (error) {
            showMessage(error.message, true);
            visibility.disabled = false;
          }
        });
        const unlink = document.createElement("button");
        unlink.className = "button danger";
        unlink.textContent = "Disconnect";
        unlink.addEventListener("click", async () => {
          if (!confirm(`Disconnect your ${title.textContent} account from MelloID?`)) return;
          unlink.disabled = true;
          try {
            await request(`/api/v1/me/connections/${provider}`, { method: "DELETE" });
            await loadConnections();
            showMessage(`${title.textContent} disconnected.`);
          } catch (error) {
            showMessage(error.message, true);
            unlink.disabled = false;
          }
        });
        actions.append(visibility, unlink);
        card.append(actions);
      } else {
        const connect = document.createElement("a");
        connect.className = "button";
        connect.href = `/auth/connections/${provider}/start`;
        connect.textContent = `Connect ${title.textContent}`;
        card.append(connect);
      }
      list.append(card);
    }
  } catch (error) {
    list.replaceChildren();
    const message = document.createElement("p");
    message.className = "muted";
    message.textContent = error.message;
    list.append(message);
  }
}
async function load() {
  const params = new URLSearchParams(location.search);
  if (params.get("status") === "connected") {
    showMessage(`${params.get("connection") || "Account"} verified and connected.`);
    history.replaceState({}, "", "/dashboard");
  } else if (params.has("connection_error")) {
    showMessage("The account link could not be completed. Start again from the verified connections section.", true);
    history.replaceState({}, "", "/dashboard");
  }
  loadConnections().catch(() => {});
  try {
    const result = await request("/api/v1/me/profile");
    if (result.profile) {
      importedProfile = result.profile;
      renderSummary(importedProfile);
      byId("publish").disabled = false;
      byId("delete").disabled = false;
      showMessage(`Currently hosted at ${result.public_url}`);
    } else {
      showMessage("No profile is hosted yet. Import a MelloGram JSON export to begin.");
    }
  } catch (error) {
    showMessage(error.message + " Return to the home page to sign in.", true);
  }
}
byId("profile-file").addEventListener("change", async (event) => {
  const file = event.target.files?.[0];
  if (!file) return;
  if (file.size > 256 * 1024) {
    showMessage("Profile JSON must be 256 KiB or smaller.", true);
    return;
  }
  try {
    const parsed = JSON.parse(await file.text());
    if (!parsed || parsed.schema_version !== 1) throw new Error("Unsupported MelloID profile schema.");
    importedProfile = parsed;
    renderSummary(parsed);
    byId("file-status").textContent = `Selected ${file.name} (${file.size.toLocaleString()} bytes)`;
    byId("publish").disabled = false;
    showMessage("Ready to publish. Server-side validation will remove untrusted verification claims.");
  } catch (error) {
    importedProfile = null;
    byId("publish").disabled = true;
    showMessage("Could not import this file: " + error.message, true);
  }
});
byId("publish").addEventListener("click", async () => {
  if (!importedProfile) return;
  byId("publish").disabled = true;
  try {
    const result = await request("/api/v1/me/profile", {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(importedProfile),
    });
    showMessage("Profile published: " + result.public_url);
    byId("delete").disabled = false;
  } catch (error) {
    showMessage(error.message, true);
  } finally {
    byId("publish").disabled = false;
  }
});
byId("delete").addEventListener("click", async () => {
  if (!confirm("Remove your hosted MelloID profile? This does not delete your local MelloGram profile.")) return;
  byId("delete").disabled = true;
  try {
    await request("/api/v1/me/profile", { method: "DELETE" });
    importedProfile = null;
    byId("profile-summary").hidden = true;
    byId("publish").disabled = true;
    showMessage("Hosted profile and its verified connections were removed. Your local profile is unchanged.");
    await loadConnections();
  } catch (error) {
    showMessage(error.message, true);
    byId("delete").disabled = false;
  }
});
byId("logout").addEventListener("click", async () => {
  try { await request("/auth/logout", { method: "POST" }); } catch {}
  location.assign("/");
});
load();
