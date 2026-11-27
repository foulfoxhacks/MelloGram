// Mobile (Android/iOS) and narrow-screen behaviour. Additive: it only reacts
// to the DOM the rest of the UI builds, so desktop is unaffected.
//
//  * body.mg-mobile-app  – running in the Android/iOS app (startup_status().mobile)
//  * body.mg-mobile      – narrow viewport (set by app.js); one pane at a time
//
// Features: hardware back button / back gesture closes the topmost layer
// (menu → dialog → palette → settings → chat) instead of leaving the app,
// keyboard-aware viewport, Enter inserts a newline on touch keyboards,
// tap-to-reveal for hover-only timestamps, and a one-time notification
// permission request on Android 13+.
"use strict";

(() => {
  const body = document.body;
  const $ = (id) => document.getElementById(id);
  const ua = navigator.userAgent;
  const mobileUA = /Android|iPhone|iPad|iPod/i.test(ua);
  const touch = matchMedia("(hover: none) and (pointer: coarse)");

  if (mobileUA) body.classList.add("mg-mobile-app");
  if (/Android/i.test(ua)) body.classList.add("mg-android");
  const applyTouch = () => body.classList.toggle("mg-touch", touch.matches);
  touch.addEventListener?.("change", applyTouch);
  applyTouch();

  let isMobileApp = mobileUA;
  try {
    Bridge.invoke("startup_status").then((s) => {
      if (s?.mobile) {
        isMobileApp = true;
        body.classList.add("mg-mobile-app");
      }
    }).catch(() => {});
  } catch { /* no backend (static preview) */ }

  const narrow = () => body.classList.contains("mg-mobile");
  const visible = (el) => !!el && !el.hidden;

  // ---- back button / gesture ---------------------------------------------------------
  //
  // Android's WebView navigates back in history before leaving the app, so each
  // open layer gets a history entry. Back pops one and we close the top layer;
  // with nothing open there is no entry left and Android leaves the app as usual.

  function dynamicOverlays() {
    return [...document.querySelectorAll("body > .mg-overlay")]
      .filter((o) => !["login", "settings", "palette"].includes(o.id) && !o.hidden);
  }

  /** Open layers, bottom first. Each entry closes itself. */
  function layers() {
    const list = [];
    if (narrow() && $("app")?.classList.contains("mg-show-chat")) {
      list.push(() => $("chat-back")?.click());
    }
    if (visible($("settings"))) {
      list.push(() => $("close-settings")?.click());
      // Phone shell: a settings page goes back to the settings list first.
      const card = document.querySelector(".mg-settings");
      if (body.classList.contains("mg-phone") && card && !card.classList.contains("mg-ph-list")) {
        list.push(() => card.classList.add("mg-ph-list"));
      }
    }
    if (visible(document.querySelector(".mg-ph-drawer"))) list.push(() => window.MelloGram.Phone?.closeDrawer());
    if (visible(document.querySelector(".mg-info-panel"))) list.push(() => { document.querySelector(".mg-info-panel").hidden = true; });
    if (visible($("palette"))) list.push(() => { $("palette").hidden = true; });
    for (const overlay of dynamicOverlays()) {
      // Modals and the MelloID editor close on a click on their backdrop.
      list.push(() => overlay.dispatchEvent(new MouseEvent("click", { bubbles: true })));
    }
    if (visible($("context-menu"))) list.push(() => { $("context-menu").hidden = true; });
    return list;
  }

  let depth = 0;          // history entries we own
  let ignorePops = 0;     // pops caused by our own history.go()
  let syncQueued = false;

  function sync() {
    syncQueued = false;
    if (visible($("lock"))) return; // the lock screen sits above everything
    const open = layers().length;
    while (depth < open) {
      history.pushState({ mg: ++depth }, "");
    }
    if (depth > open) {
      // Closed from the UI (✕, Esc, backdrop): drop our extra entries.
      ignorePops++;
      history.go(open - depth);
      depth = open;
    }
  }
  function queueSync() {
    if (syncQueued) return;
    syncQueued = true;
    requestAnimationFrame(sync);
  }

  addEventListener("popstate", () => {
    if (ignorePops > 0) { ignorePops--; return; }
    depth = Math.max(0, depth - 1);
    if (visible($("lock"))) {
      // Back must not reveal anything behind the lock screen.
      history.pushState({ mg: ++depth }, "");
      return;
    }
    const open = layers();
    if (open.length > depth) open[open.length - 1]();
    queueSync();
  });

  new MutationObserver(queueSync).observe(body, {
    subtree: true, childList: true, attributes: true, attributeFilter: ["hidden", "class"],
  });
  matchMedia("(max-width: 700px)").addEventListener?.("change", queueSync);

  // ---- keyboard-aware viewport ---------------------------------------------------------
  //
  // On Android the activity resizes the WebView for the keyboard (see
  // MainActivity.kt); visualViewport covers WebViews/browsers that overlay it.

  const vv = window.visualViewport;
  function fitViewport() {
    if (!vv) return;
    document.documentElement.style.setProperty("--mg-vvh", `${Math.round(vv.height)}px`);
  }
  vv?.addEventListener("resize", fitViewport);
  fitViewport();

  // Keep the newest message in view when the keyboard (or anything else)
  // shrinks the message list while the user was at the bottom.
  const messages = $("messages");
  if (messages && "ResizeObserver" in window) {
    let atBottom = true;
    messages.addEventListener("scroll", () => {
      atBottom = messages.scrollHeight - messages.scrollTop - messages.clientHeight < 48;
    }, { passive: true });
    new ResizeObserver(() => { if (atBottom) messages.scrollTop = messages.scrollHeight; }).observe(messages);
  }

  // ---- composer on touch keyboards -----------------------------------------------------
  //
  // Phone keyboards have no Shift+Enter, so Enter inserts a newline and the
  // send button sends (like Telegram's mobile apps). Command picking still
  // works because the command popup handles Enter first.

  const composer = $("composer");
  if (composer) {
    composer.setAttribute("enterkeyhint", "enter");
    document.addEventListener("keydown", (e) => {
      if (e.target !== composer || e.key !== "Enter" || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey) return;
      if (!body.classList.contains("mg-touch") && !isMobileApp) return;
      if (!$("command-popup")?.hidden) return;
      e.stopPropagation(); // keep app.js from sending; the default inserts "\n"
    }, true);
    // Tapping send must not dismiss the keyboard between messages.
    $("send")?.addEventListener("pointerdown", (e) => {
      if (document.activeElement === composer) e.preventDefault();
    });
  }

  // ---- hover-only controls -------------------------------------------------------------
  //
  // "Timestamps: on hover" (and themes using :hover) have no hover on touch:
  // a tap toggles .mg-touch-reveal, which mobile.css treats like :hover.

  document.addEventListener("click", (e) => {
    if (!body.classList.contains("mg-touch")) return;
    const msg = e.target.closest?.(".mg-message");
    if (!msg || e.target.closest("a, button, .mg-reply-preview")) return;
    for (const other of document.querySelectorAll(".mg-message.mg-touch-reveal")) {
      if (other !== msg) other.classList.remove("mg-touch-reveal");
    }
    msg.classList.toggle("mg-touch-reveal");
  });

  // ---- notification permission (Android 13+) -------------------------------------------
  //
  // Asked once, after sign-in when the chat list first appears, and only if
  // message notifications are enabled in Settings → Notifications.

  const ASKED_KEY = "mg-notification-permission-asked";
  async function askNotificationPermission() {
    const core = window.__TAURI__?.core;
    if (!core || !isMobileApp) return;
    try {
      if (localStorage.getItem(ASKED_KEY)) return;
    } catch { /* storage unavailable: ask anyway */ }
    try {
      const settings = await Bridge.invoke("get_settings");
      if (!settings?.notifications?.desktop) return;
      const granted = await core.invoke("plugin:notification|is_permission_granted");
      if (granted !== null && granted !== undefined) return; // already decided
      try { localStorage.setItem(ASKED_KEY, "1"); } catch { /* ignore */ }
      await core.invoke("plugin:notification|request_permission");
    } catch (e) {
      console.warn("notification permission request failed", e);
    }
  }
  const chatList = $("chat-list");
  if (chatList) {
    const waitForChats = new MutationObserver(() => {
      if (!chatList.children.length || visible($("login"))) return;
      waitForChats.disconnect();
      setTimeout(askNotificationPermission, 800);
    });
    waitForChats.observe(chatList, { childList: true });
  }
})();
