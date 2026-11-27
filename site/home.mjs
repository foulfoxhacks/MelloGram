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

export function homeBody(c) {
  const { href, img, plugins } = c;
  const compare = COMPARE.map(
    ([label, telegram, mellogram]) => `
      <tr>
        <th scope="row">${label}</th>
        <td>${telegram.replace("{PLUGINS}", plugins.length)}</td>
        <td>${mellogram.replace("{PLUGINS}", plugins.length)}</td>
      </tr>`,
  ).join("");

  return `
<section class="tg-hero">
  <div class="home-hero-mark">
    ${img("logo.png", "MelloGram fox-paw and paper-plane mascot logo", "MelloGram logo", { eager: true, cls: "cute-logo" })}
  </div>
  <p class="eyebrow">Free · Open source · GPL-3.0 · No telemetry</p>
  <h1>MelloGram</h1>
  <h2 class="hero-tagline">Messaging, on your terms.</h2>
  <p class="lead">MelloGram is a Rust-based Telegram client for desktop and Android with plugins, CSS themes and an end-to-end overlay. It signs in to your own Telegram account, so your chats stay in sync with every other device you use.</p>
  <div class="cta home-cta buttons">
    <a class="btn main-btn" href="${href("/download/")}" data-download>Download MelloGram</a>
    <a class="btn btn-ghost" href="${href("/plugins/")}">See what's inside</a>
    <a class="btn btn-ghost" href="${c.REPO}" rel="noopener">Source on GitHub ↗</a>
  </div>
  <ul class="tg-downloads" aria-label="Download MelloGram for your platform">
    ${tile(c, "Windows", ".exe · .msi", "windows", "/download/")}
    ${tile(c, "macOS", ".dmg, universal", "macos", "/download/")}
    ${tile(c, "Linux", ".deb · .rpm · AppImage", "linux", "/download/")}
    ${tile(c, "Android", ".apk · .aab", "android", "/download/")}
    ${tile(c, "Web", "early shell", "web", c.WEB)}
  </ul>
</section>

<figure class="home-hero-shot">
  ${img("screenshot-chat.webp", "MelloGram desktop chat window with custom colours and a paw-print wallpaper", "MelloGram desktop preview", { eager: true })}
  <figcaption>One desktop app, ${plugins.length} plugins, and every chat looking the way you set it up.</figcaption>
</figure>

<section class="section tg-why" aria-labelledby="why-title">
  <div class="home-section-heading">
    <p class="eyebrow">Why MelloGram?</p>
    <h2 id="why-title">Telegram underneath. Yours on top.</h2>
    <p class="lead">A native client that keeps what makes Telegram good and rebuilds everything around it to be configurable.</p>
  </div>
  <ul class="tg-why-grid">
    ${why("Simple", "It opens on your chats. The plugins, themes and controls live in settings until you go looking for them.")}
    ${why("Fast", "Written in Rust with a native webview — no Electron, no bundled browser, no multi-second launch.")}
    ${why("Secure", "The overlay adds X3DH and a Double Ratchet on top of Telegram, with 60-digit safety numbers and group sender keys.")}
    ${why("Synced", "Messages come from Telegram itself, so the phone, the desktop and every other client see the same history.")}
    ${why("Customizable", "CSS themes, QuickCSS, an Appearance Studio, wallpapers and workspaces — all applied live, no rebuild.")}
    ${why("Open", "GPL-3.0-or-later source, reproducible CI packages for every platform, and no analytics or telemetry.")}
  </ul>
</section>

<section class="section tg-rows" aria-labelledby="themes-title">
  <article class="tg-row">
    <div class="tg-row-copy">
      <p class="eyebrow">Appearance</p>
      <h2 id="themes-title">The interface is CSS, so you can change it</h2>
      <p>Install a theme someone shared, edit its variables with colour pickers, or write CSS that applies while you type. Nothing is compiled, nothing needs a restart.</p>
      <ul class="tg-list">
        <li>CSS themes with ordering and variables</li>
        <li>Live QuickCSS with instant preview</li>
        <li>Appearance Studio: bubbles, radius, spacing, fonts, density, motion</li>
      </ul>
      <a class="tg-link" href="${href("/themes/")}">Explore themes and wallpapers <span aria-hidden="true">↗</span></a>
    </div>
    <figure class="tg-row-shot">
      ${img("screenshot-appearance-studio.webp", "MelloGram Appearance Studio with live controls for bubbles, fonts and spacing", "Appearance Studio")}
      <figcaption>Appearance Studio — every control previews immediately.</figcaption>
    </figure>
  </article>

  <article class="tg-row tg-row-flip">
    <div class="tg-row-copy">
      <p class="eyebrow">Plugins</p>
      <h2>Features without a patched installer</h2>
      <p>${plugins.length} plugins ship in the app: link cleaning, message history, templates, bookmarks, statistics, translation, quiet hours and more. Each one states what it reads and changes before you enable it.</p>
      <ul class="tg-list">
        <li>Search, filter and favourite plugins</li>
        <li>Permission labels on every plugin</li>
        <li>Backend logic in Rust, UI halves as plain scripts</li>
      </ul>
      <a class="tg-link" href="${href("/plugins/")}">Browse all ${plugins.length} plugins <span aria-hidden="true">↗</span></a>
    </div>
    <figure class="tg-row-shot">
      ${img("screenshot-plugins.webp", "MelloGram plugin manager listing searchable extensions with permission labels", "Plugin manager")}
      <figcaption>The plugin manager — every extension declares its permissions.</figcaption>
    </figure>
  </article>

  <article class="tg-row">
    <div class="tg-row-copy">
      <p class="eyebrow">Privacy</p>
      <h2>End-to-end when a chat calls for it</h2>
      <p>Turn on the overlay for a one-to-one chat or a group and MelloGram seals messages with its own protocol before Telegram ever sees them. Verification is a 60-digit safety number you read aloud, and group keys rotate when someone leaves.</p>
      <ul class="tg-list">
        <li>One-to-one: X3DH setup, Double Ratchet, per-message keys</li>
        <li>Groups: sender keys delivered inside pairwise sessions</li>
        <li>Device-sealed state, bounded decrypted cache, no cloud copies</li>
      </ul>
      <a class="tg-link" href="${href("/privacy/")}">Read what the overlay protects <span aria-hidden="true">↗</span></a>
    </div>
    <figure class="tg-row-shot">
      ${img("screenshot-chat.webp", "A MelloGram chat showing the encrypted overlay badge on messages", "Encrypted chat preview")}
      <figcaption>The overlay marks every sealed message and shows when a key is missing.</figcaption>
    </figure>
  </article>
</section>

<section class="section tg-compare-section" aria-labelledby="compare-title">
  <div class="home-section-heading">
    <p class="eyebrow">Comparison</p>
    <h2 id="compare-title">MelloGram and Telegram Desktop, side by side</h2>
    <p class="lead">MelloGram is an independent client, not a fork. It uses the same account and the same servers, and adds a customization layer Telegram Desktop does not have.</p>
  </div>
  <div class="tg-compare">
    <table>
      <thead>
        <tr><td></td><th scope="col">Telegram Desktop</th><th scope="col" class="tg-compare-us">MelloGram</th></tr>
      </thead>
      <tbody>${compare}</tbody>
    </table>
  </div>
  <p class="muted small">Telegram Desktop details follow Telegram's public documentation and the current desktop app. MelloGram never asks for your password outside Telegram's own login flow, and it is not affiliated with Telegram.</p>
</section>

<section class="section tg-do" aria-labelledby="do-title">
  <div class="home-section-heading">
    <p class="eyebrow">Get going</p>
    <h2 id="do-title">What you can do next</h2>
  </div>
  <ul class="tg-do-grid">
    <li><a href="${href("/download/")}"><strong>Install it</strong><span>Pick your platform and sign in with your phone number.</span></a></li>
    <li><a href="${href("/plugins/")}"><strong>Enable plugins</strong><span>Start with link cleaning and message history.</span></a></li>
    <li><a href="${href("/themes/")}"><strong>Make it yours</strong><span>Theme, wallpaper and workspace in a few minutes.</span></a></li>
    <li><a href="${href("/privacy/")}"><strong>Turn on the overlay</strong><span>Exchange keys with one contact and compare digits.</span></a></li>
    <li><a href="${href("/faq/")}"><strong>Read the FAQ</strong><span>Backups, notifications, safe mode and account safety.</span></a></li>
    <li><a href="${href("/developers/")}"><strong>Build on it</strong><span>Plugin API, architecture notes and diagnostics.</span></a></li>
  </ul>
</section>

<section class="home-bottom-cta" aria-labelledby="home-bottom-title">
  <div>
    <p class="eyebrow">Ready when you are</p>
    <h2 id="home-bottom-title">Start with MelloGram.</h2>
    <p>Free, open source under GPL-3.0, and built for the way you use Telegram.</p>
  </div>
  <div class="cta">
    <a class="btn main-btn" href="${href("/download/")}" data-download>Get MelloGram</a>
    <a class="btn btn-ghost" href="${href("/faq/")}">Read the FAQ</a>
  </div>
</section>`;
}
