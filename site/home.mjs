// Homepage content using MelloGram branding and shipped app previews.

const ICON = {
  desktop: '<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M12 17v4M8 21h8"/>',
  android: '<rect x="6.5" y="2.5" width="11" height="19" rx="2.4"/><path d="M10.5 5.5h3M11.5 18h1"/>',
  arrow: '<path d="M4 12h15m-6-6 6 6-6 6"/>',
};

const icon = (name) =>
  `<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round" stroke-linecap="round">${ICON[name]}</svg>`;

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
<div class="tgo-intro">
  <svg class="tgo-signal" viewBox="0 0 300 240" fill="none" aria-hidden="true" focusable="false">
    <ellipse cx="145" cy="120" rx="130" ry="48" transform="rotate(-30 145 120)" />
    <ellipse cx="145" cy="120" rx="130" ry="68" transform="rotate(26 145 120)" />
    <circle cx="78" cy="168" r="4" />
    <path d="M232 43v18m-9-9h18M65 57v12m-6-6h12M240 171v10m-5-5h10" />
  </svg>
  <section class="tgo-hero" aria-labelledby="home-title">
    ${img("logo.png", "MelloGram fox-paw and paper-plane mascot logo", "MelloGram logo", { eager: true, cls: "tgo-logo" })}
    <h1 id="home-title">MelloGram</h1>
    <p class="tgo-tagline">Your chats. Your colors.</p>
    <p class="tgo-sub">An independent Telegram client with themes, plugins, and room to make it yours.</p>
  </section>
  <aside class="tgo-highlights" aria-labelledby="highlights-title">
    <h2 id="highlights-title">Explore MelloGram</h2>
    <ul>
      <li><a href="${href("/themes/")}">Themes &amp; wallpapers</a><span>Make it yours</span></li>
      <li><a href="${href("/plugins/")}">Built-in plugins</a><span>Pick what works for you</span></li>
      <li><a href="${c.esc(c.WEB)}">MelloGram Web <span aria-hidden="true">↗</span></a><span>Browser preview · optional beta</span></li>
    </ul>
  </aside>
</div>

<section class="tgo-apps" aria-labelledby="apps-title">
  <h2 id="apps-title">Get MelloGram</h2>
  <a class="tgo-app tgo-android" href="${href("/download/")}#android">
    <div class="tgo-app-visual">
      ${img("screenshot-android.webp", "MelloGram chat list on Android", "MelloGram for Android", { cls: "tgo-app-shot tgo-app-phone" })}
    </div>
    <span class="tgo-app-title">
      <span>${icon("android")} <strong>Android</strong></span>
      ${icon("arrow")}
    </span>
    <span class="tgo-app-platforms">Download the Android app</span>
  </a>
  <a class="tgo-app tgo-desktop" href="${href("/download/")}">
    <div class="tgo-app-visual">
      ${img("screenshot-chat.webp", "MelloGram desktop chat with custom colors and a paw-print wallpaper", "MelloGram for Desktop", { cls: "tgo-app-shot" })}
    </div>
    <span class="tgo-app-title">
      <span>${icon("desktop")} <strong>Desktop</strong></span>
      ${icon("arrow")}
    </span>
    <span class="tgo-app-platforms">Windows, macOS &amp; Linux</span>
  </a>
</section>

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
