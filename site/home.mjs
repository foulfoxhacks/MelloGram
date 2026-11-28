// Homepage body. Structure follows telegram.org — tagline hero, per-platform
// download tiles, a "Why <product>?" grid, spotlight sections and a closing
// call to action — rendered in MelloGram's dark purple palette with MelloGram's
// own copy. All claims describe shipped behaviour; the comparison table is
// checked against Telegram's public documentation.

const ICON = {
  windows:
    '<path d="M3 5.5 10.5 4.4v7.1H3Zm8.5-1.3L21 3v8.5h-9.5ZM3 12.5h7.5v7.1L3 18.5Zm8.5 0H21V21l-9.5-1.3Z"/>',
  macos:
    '<rect x="3" y="4" width="18" height="12.5" rx="1.8"/><path d="M2 19.5h20"/><path d="M9.5 16.5h5"/>',
  linux:
    '<rect x="3" y="4" width="18" height="15" rx="2"/><path d="m7 9 3 3-3 3"/><path d="M12.5 15H17"/>',
  android:
    '<rect x="6.5" y="2.5" width="11" height="19" rx="2.4"/><path d="M10.5 5.5h3"/><circle cx="12" cy="18" r="1"/>',
  web:
    '<circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3c2.6 2.4 4 5.5 4 9s-1.4 6.6-4 9c-2.6-2.4-4-5.5-4-9s1.4-6.6 4-9Z"/>',
};

const icon = (name) =>
  `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round">${ICON[name]}</svg>`;

const tile = (c, name, meta, glyph, path) => `
    <li>
      <a class="tg-tile" href="${c.href(path)}">
        ${icon(glyph)}
        <span class="tg-tile-name">${name}</span>
        <span class="tg-tile-meta">${meta}</span>
      </a>
    </li>`;

const why = (title, text) => `
    <li class="tg-why-card">
      <h3>${title}</h3>
      <p>${text}</p>
    </li>`;

const COMPARE = [
  ["Where it runs", "Windows, macOS, Linux, Android, iOS and web", "Windows, macOS, Linux and Android; a browser shell is in progress"],
  ["License", "GPL-3.0", "GPL-3.0-or-later, built from the public repository"],
  ["Plugin system", "None", "{PLUGINS} built-in plugins, each declaring its permissions"],
  ["Themes", "Cloud themes and a palette editor", "CSS themes, theme variables and live QuickCSS"],
  ["Chat backgrounds", "Cloud wallpapers and gradients", "Images, gradients, patterns and animated scenes with blur, dim and parallax"],
  ["Saved setups", "Not available", "Workspaces that switch plugins, theme, CSS and appearance together"],
  ["End-to-end messaging", "Secret Chats: one-to-one, mobile only", "MelloGram overlay: one-to-one and groups, opt-in per chat"],
  ["Group end-to-end", "Not available", "Sender keys, rotated when a member leaves or is removed"],
  ["Built with", "C++ (tdesktop)", "Rust and Tauri"],
];

// Feature icons for the "Why MelloGram?" grid, drawn as simple line icons
// inside round badges, the way telegram.org presents its features.
const WHY_ICON = {
  simple: '<path d="M5 12h14M12 5v14"/>',
  private: '<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>',
  synced: '<path d="M4 12a8 8 0 0 1 14-5.3L20 9"/><path d="M20 4v5h-5"/><path d="M20 12a8 8 0 0 1-14 5.3L4 15"/><path d="M4 20v-5h5"/>',
  fast: '<path d="M13 3 5 14h6l-1 7 8-11h-6Z"/>',
  custom: '<circle cx="12" cy="12" r="8.5"/><circle cx="9" cy="10" r="1.3"/><circle cx="14.5" cy="9" r="1.3"/><circle cx="15.5" cy="14" r="1.3"/>',
  plugins: '<path d="M9 4h6v4a2 2 0 1 0 0 4v4H9v-4a2 2 0 1 1 0-4Z"/>',
  secure: '<path d="M12 3 5 6v6c0 4.5 3 7.5 7 9 4-1.5 7-4.5 7-9V6Z"/><path d="m9 12 2 2 4-4"/>',
  open: '<path d="m8 8-4 4 4 4"/><path d="m16 8 4 4-4 4"/><path d="m13.5 5-3 14"/>',
  free: '<path d="M20 12v8H4v-8"/><path d="M2 8h20v4H2z"/><path d="M12 8v12"/><path d="M12 8S10 3 7.5 4.5 9 8 12 8Zm0 0s2-5 4.5-3.5S15 8 12 8Z"/>',
};
const whyItem = (key, title, text) => `
    <li class="tgo-why-item">
      <span class="tgo-why-icon"><svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round" stroke-linecap="round">${WHY_ICON[key]}</svg></span>
      <h3>${title}</h3>
      <p>${text}</p>
    </li>`;

export function homeBody(c) {
  const { href, img, plugins } = c;
  return `
<section class="tgo-hero">
  <div class="tgo-hero-logo">
    ${img("logo.png", "MelloGram fox-paw and paper-plane mascot logo", "MelloGram logo", { eager: true, cls: "cute-logo" })}
  </div>
  <h1>MelloGram</h1>
  <p class="tgo-tagline">a customizable era of messaging</p>
  <p class="tgo-sub">Your Telegram account, your chats, your look. Free, open source and written in Rust.</p>
</section>

<section class="tgo-apps" aria-label="Get MelloGram">
  <a class="tgo-app" href="${href("/download/")}#android" data-os="android">
    <span class="tgo-app-title">MelloGram for <strong>Android</strong></span>
    ${img("screenshot-android.webp", "MelloGram on an Android phone, showing the chat list with filter chips and bottom navigation", "MelloGram for Android", { eager: true, cls: "tgo-app-shot tgo-app-phone" })}
  </a>
  <a class="tgo-app" href="${href("/download/")}">
    <span class="tgo-app-title">MelloGram for <strong>Desktop</strong></span>
    ${img("screenshot-chat.webp", "MelloGram desktop chat window with custom colours and a paw-print wallpaper", "MelloGram for Desktop", { eager: true, cls: "tgo-app-shot" })}
  </a>
</section>
<nav class="tgo-more-apps" aria-label="Other platforms">
  <a href="${href("/download/")}#windows">MelloGram for <strong>Windows</strong></a>
  <a href="${href("/download/")}#macos">MelloGram for <strong>macOS</strong></a>
  <a href="${href("/download/")}#linux">MelloGram for <strong>Linux</strong></a>
  <a href="${c.WEB}" rel="noopener">MelloGram <strong>Web</strong></a>
</nav>

<section class="tgo-why" aria-labelledby="why-title">
  <h2 id="why-title">Why MelloGram?</h2>
  <ul class="tgo-why-grid">
    ${whyItem("simple", "Simple", "It opens on your chats. Plugins, themes and controls wait in settings until you want them.")}
    ${whyItem("private", "Private", "No analytics, no telemetry, and a traffic log that shows every connection the app makes.")}
    ${whyItem("synced", "Synced", "Messages come from Telegram itself, so your phone, desktop and other apps share one history.")}
    ${whyItem("fast", "Fast", "Native Rust with the system webview — no Electron and no bundled browser.")}
    ${whyItem("custom", "Customizable", "CSS themes, live QuickCSS, wallpapers, layouts and an Appearance Studio, applied instantly.")}
    ${whyItem("plugins", "Powerful", `${plugins.length} built-in plugins, from scheduled messages to message history, each showing its permissions.`)}
    ${whyItem("secure", "Secure", "An optional end-to-end overlay for one-to-one chats and groups, with 60-digit safety numbers.")}
    ${whyItem("open", "Open", "GPL-3.0-or-later source, with every installer built in public CI from that code.")}
    ${whyItem("free", "Free", "Free to use on Windows, macOS, Linux and Android, with no ads and no subscriptions.")}
  </ul>
</section>

<section class="tgo-about" aria-labelledby="about-title">
  <h2 id="about-title">What is MelloGram?</h2>
  <p>MelloGram is an independent, open-source Telegram client for Windows, macOS, Linux and Android. You sign in with your own Telegram account, so every chat, group, channel and contact is the same one you see on your phone and in any other Telegram app. Nothing moves to a new network and nobody has to switch with you.</p>
  <p>On top of Telegram, MelloGram adds what the official apps leave out: a plugin manager with permission labels, CSS themes and live QuickCSS, chat wallpapers, saved workspaces, layout styles, scheduled messages, a redacted traffic log, proxy and tunnel presets for Tor, Tailscale and WireGuard, and an optional end-to-end overlay for one-to-one chats and groups. It is not affiliated with Telegram, and it never asks for your login code or password outside Telegram's own sign-in.</p>
</section>

<section class="tgo-strip" aria-label="Learn more">
  <a href="${href("/plugins/")}"><strong>Plugins</strong><span>Browse all ${plugins.length}</span></a>
  <a href="${href("/themes/")}"><strong>Themes &amp; wallpapers</strong><span>Make it yours</span></a>
  <a href="${href("/privacy/")}"><strong>Privacy</strong><span>What the overlay protects</span></a>
  <a href="${href("/faq/")}"><strong>FAQ</strong><span>Answers in one place</span></a>
</section>`;
}
