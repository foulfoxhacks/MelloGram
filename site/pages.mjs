// MelloGram website content. Each page: path, title (≤60), description
// (120–160), h1, body(ctx), optional jsonld(ctx). Copy is written answer-first:
// the first sentence of each section answers the question its heading asks.

const CATEGORY = {
  appearance: "Appearance", chat: "Chat", media: "Media", privacy: "Privacy", profiles: "Chats & profiles",
  notifications: "Notifications", developer: "Developer", utility: "Utility", productivity: "Productivity", accessibility: "Accessibility",
};
const PERMISSION = {
  "read-messages": "reads messages", "modify-outgoing": "changes what you send", ui: "adds interface",
  storage: "stores data locally", network: "uses the network", clipboard: "uses the clipboard", styles: "adds styles",
};

const faqs = (c) => [
  ["What is MelloGram?",
    `MelloGram is a free, open-source Telegram client for Windows, macOS, Linux and Android that you can customize like a Vencord-modded Discord: it ships ${c.plugins.length} built-in plugins, a theme engine with QuickCSS, a wallpaper engine, workspaces and an Appearance Studio, all in a fast Rust application.`],
  ["Is MelloGram free?",
    "Yes. MelloGram is completely free and open source under the GPL-3.0-or-later licence. There are no ads, no subscriptions and no paid features."],
  ["Is it safe to use MelloGram with my Telegram account?",
    "Yes. MelloGram talks to Telegram through Telegram's official MTProto API as a registered application, the same way other third-party Telegram clients do. Your login code and two-step verification password go only to Telegram, and your session is stored only on your device."],
  ["Does MelloGram collect my messages or data?",
    "No. MelloGram has no telemetry and no analytics. Settings, plugin data, message history and diagnostics stay on your device. Optional services such as MelloID public profiles only receive what you choose to publish."],
  ["How is MelloGram different from Telegram Desktop?",
    `Telegram Desktop is the official client; MelloGram is an independent client focused on customization. MelloGram adds a plugin manager with ${c.plugins.length} plugins, CSS themes and QuickCSS, animated and per-chat wallpapers, workspaces, a command palette, an app lock and local diagnostics.`],
  ["Is MelloGram a Vencord port?",
    "No. MelloGram is inspired by the Vencord user experience — a plugin manager, themes and QuickCSS — but it is a standalone Telegram client written from scratch in Rust. It does not inject into another app and contains no Vencord code."],
  ["Which platforms does MelloGram support?",
    "MelloGram runs on Windows 10 and 11 (setup .exe or .msi), macOS 11 or newer on Apple Silicon and Intel (.dmg), Linux (.deb, .rpm or AppImage) and Android 7.0 or newer (.apk)."],
  ["How do I install plugins in MelloGram?",
    "Open Settings → Plugins (Ctrl+,) and switch a plugin on. Every plugin is built in, shows the permissions it uses, and has its own settings panel. Plugins that crash are isolated and paused automatically."],
  ["Can I use custom themes and CSS?",
    "Yes. Install CSS themes from a file or URL, reorder them, edit their colour variables with pickers, or write your own styles in QuickCSS, which applies live as you type. The Appearance Studio changes bubbles, fonts and spacing without any CSS."],
  ["Can I set my own chat wallpaper?",
    `Yes. The wallpaper engine supports your own images, gradients, patterns and animated backgrounds, with blur, dim and parallax, a separate dark-mode wallpaper and per-chat wallpapers. There are ${c.wallpapers.length} built-in presets.`],
  ["Does MelloGram run in the background and send notifications?",
    "Yes, on desktop. MelloGram can keep running in the system tray when you close the window, start minimised, launch at login and show native notifications, with Do Not Disturb, quiet hours, per-chat muting and an option to hide message previews."],
  ["Do I need a separate account for MelloGram?",
    "No. You sign in with your normal Telegram account. MelloID, MelloGram's optional public profile and badge system, is separate and never required for messaging."],
  ["How do I back up or share my MelloGram setup?",
    "Use Settings → Backup & Restore to export everything to one JSON file, or share a workspace as a profile file. Imports show every plugin and permission before anything changes, and Telegram credentials are never included."],
  ["What if a plugin or theme breaks MelloGram?",
    "Start MelloGram in safe mode with the --safe-mode flag, or from the banner MelloGram shows after an unclean exit. Safe mode turns off plugins, themes and custom CSS so you can fix the problem."],
];

function faqJsonLd(list) {
  return {
    "@type": "FAQPage",
    mainEntity: list.map(([q, a]) => ({ "@type": "Question", name: q, acceptedAnswer: { "@type": "Answer", text: a } })),
  };
}

function softwareApp(c) {
  return {
    "@type": "SoftwareApplication",
    "@id": c.url("/#software"),
    name: "MelloGram",
    description: "A free, open-source, customizable Telegram client with plugins, themes, wallpapers and workspaces.",
    applicationCategory: "CommunicationApplication",
    applicationSubCategory: "Messaging client",
    operatingSystem: "Windows 10, Windows 11, macOS 11+, Linux, Android 7+",
    softwareVersion: c.version,
    license: "https://www.gnu.org/licenses/gpl-3.0.html",
    isAccessibleForFree: true,
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
    downloadUrl: [c.downloads.windows[0][1], c.downloads.macos[0][1], c.downloads.linux[0][1], c.downloads.android[0][1]],
    installUrl: c.url("/download/"),
    softwareHelp: { "@type": "CreativeWork", url: c.url("/faq/") },
    codeRepository: c.REPO,
    url: c.url("/"),
    image: c.url("/assets/og-image.png"),
    screenshot: ["screenshot-chat.webp", "screenshot-plugins.webp", "screenshot-wallpapers.webp", "screenshot-appearance-studio.webp"].map((f) => c.url("/assets/" + f)),
    featureList: [
      `${c.plugins.length} built-in plugins with declared permissions`,
      "CSS themes, theme variables and live QuickCSS",
      "Wallpaper engine: images, gradients, patterns, animated, per-chat",
      "Appearance Studio for bubbles, fonts, density and layout",
      "Workspaces and shareable profiles",
      "System tray, background mode and native notifications",
      "App lock, privacy mode and secret guard",
      "Local diagnostics and safe mode",
    ],
    author: { "@id": c.url("/#organization") },
    publisher: { "@id": c.url("/#organization") },
    maintainer: { "@id": c.url("/#organization") },
    brand: { "@type": "Brand", name: "MelloGram", logo: c.url("/assets/logo.png") },
    sameAs: [c.REPO, "https://github.com/foulfoxhacks"],
    keywords: "MelloGram, Telegram client, messaging, plugins, themes, QuickCSS, open source",
    inLanguage: "en",
    audience: { "@type": "Audience", audienceType: "Telegram users, open-source users and developers" },
  };
}

const featureCards = (c) => [
  ["Plugin manager", `${c.plugins.length} built-in plugins — link cleaning, message history, templates, bookmarks, statistics and more. Search, filter, favourite and configure each one.`, "/plugins/"],
  ["Themes & QuickCSS", "Drop in CSS themes, order them, tweak their variables with colour pickers, or write live CSS that applies as you type.", "/themes/"],
  ["Wallpaper engine", `Your own images, gradients, patterns and animated scenes behind every chat, with blur, dim, parallax and ${c.wallpapers.length} presets.`, "/themes/#wallpapers"],
  ["Appearance Studio", "Bubble styles, radius, spacing, fonts, density, avatars and animations — with a live preview and no CSS required.", "/themes/#appearance-studio"],
  ["Workspaces", "Save complete setups for work, gaming or privacy and switch between them instantly from the title bar or command palette.", "/faq/#how-do-i-back-up-or-share-my-mellogram-setup"],
  ["Background & notifications", "Runs in the tray, starts with your computer and notifies you natively, with quiet hours and Do Not Disturb.", "/faq/#does-mellogram-run-in-the-background-and-send-notifications"],
  ["Privacy tools", "App lock with a PIN, privacy blur, a guard that stops you pasting API keys or card numbers, and zero telemetry.", "/privacy/"],
  ["Diagnostics", "A local testing centre checks storage, themes, plugins and Telegram connectivity, and safe mode recovers from broken setups.", "/developers/#diagnostics"],
];

const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

export function pages(c) {
  const { href, img, esc } = c;
  const faqList = faqs(c);
  const home = faqList.slice(0, 6);
  const byCategory = {};
  for (const p of c.plugins) (byCategory[p.category] ||= []).push(p);

  return [
    {
      path: "/",
      title: "MelloGram — The Customizable Telegram Client",
      description: "MelloGram is a free, open-source Telegram client for Windows, macOS, Linux and Android with plugins, themes, QuickCSS, wallpapers and workspaces.",
      h1: "MelloGram, the customizable Telegram client",
      priority: 1.0,
      changefreq: "weekly",
      images: ["screenshot-chat.webp", "screenshot-plugins.webp", "screenshot-wallpapers.webp", "screenshot-appearance-studio.webp", "og-image.png"],
      jsonld: (c) => [softwareApp(c), faqJsonLd(home)],
      body: () => `
<section class="home-hero">
  <div class="home-hero-copy">
    <p class="eyebrow">A customizable Telegram client by FoulFoxHacks</p>
    <h1>Make Telegram <span class="gradient-text">your own.</span></h1>
    <p class="lead">The client-mod experience, built into a standalone Telegram app. Explore ${c.plugins.length} built-in plugins, themes, live QuickCSS, custom wallpapers and workspaces — without patching another app.</p>
    <div class="cta home-cta">
      <a class="btn" href="${href("/download/")}" data-download>Download MelloGram</a>
      <a class="btn btn-ghost" href="${href("/plugins/")}">Explore plugins</a>
    </div>
    <p class="home-meta">Free and open source · Windows · macOS · Linux · Android</p>
  </div>
  <figure class="home-hero-shot">
    ${img("screenshot-chat.webp", "MelloGram chat interface with custom colours and a paw-print wallpaper", "See MelloGram in action", { eager: true })}
  </figure>
</section>

<section class="home-features section" aria-labelledby="home-features-title">
  <div class="home-section-heading">
    <p class="eyebrow">Built around your Telegram experience</p>
    <h2 id="home-features-title">More than a different look.</h2>
    <p class="lead">Useful tools, thoughtful customization and control over how your client behaves.</p>
  </div>
  <ul class="home-feature-grid">
    <li class="home-feature-card">
      ${img("screenshot-plugins.webp", "MelloGram plugin manager with searchable plugins and permission labels", "Built-in plugin manager", { cls: "home-feature-image" })}
      <div><h3>Plugins, ready to use</h3><p>${c.plugins.length} built-in plugins for privacy, productivity, chat tools and quality-of-life improvements. Enable what you want and inspect permissions before switching it on.</p><a href="${href("/plugins/")}">Browse plugins</a></div>
    </li>
    <li class="home-feature-card">
      ${img("screenshot-appearance-studio.webp", "MelloGram Appearance Studio with live interface customization controls", "Appearance Studio", { cls: "home-feature-image" })}
      <div><h3>Make it look like yours</h3><p>Customize message bubbles, spacing, fonts, density, colours and animations with a live preview — no CSS knowledge required.</p><a href="${href("/themes/#appearance-studio")}">Explore Appearance Studio</a></div>
    </li>
    <li class="home-feature-card">
      ${img("screenshot-wallpapers.webp", "MelloGram wallpaper gallery with gradients, animated scenes and paw-print patterns", "Themes and wallpapers", { cls: "home-feature-image" })}
      <div><h3>Themes, wallpapers and QuickCSS</h3><p>Install CSS themes, edit theme variables, create your own styles and set image, gradient, patterned or animated chat backgrounds.</p><a href="${href("/themes/")}">Explore customization</a></div>
    </li>
    <li class="home-feature-card">
      ${img("screenshot-workspaces.webp", "MelloGram workspace selector showing saved client configurations", "MelloGram workspaces", { cls: "home-feature-image" })}
      <div><h3>Workspaces that switch with you</h3><p>Save complete setups for work, gaming or privacy, then switch between plugins, themes, CSS and appearance settings in one step.</p><a href="${href("/faq/#how-do-i-back-up-or-share-my-mellogram-setup")}">Learn about workspaces</a></div>
    </li>
    <li class="home-feature-card">
      ${img("screenshot-diagnostics.webp", "MelloGram local diagnostics and testing centre", "MelloGram diagnostics", { cls: "home-feature-image" })}
      <div><h3>Privacy and diagnostics</h3><p>Keep settings and diagnostics local, use app lock and privacy blur, and troubleshoot with the built-in testing centre and safe mode.</p><a href="${href("/privacy/")}">Read about privacy</a></div>
    </li>
    <li class="home-feature-card">
      ${img("screenshot-android.webp", "MelloGram running on an Android phone", "MelloGram on Android", { cls: "home-feature-image" })}
      <div><h3>Your client across platforms</h3><p>Take the MelloGram experience to Windows, macOS, Linux and Android, with platform-specific installers from the project releases.</p><a href="${href("/download/")}">Choose your platform</a></div>
    </li>
  </ul>
</section>

<section class="home-showcase section" aria-labelledby="home-showcase-title">
  <div class="home-showcase-copy">
    <p class="eyebrow">Your Telegram, your rules</p>
    <h2 id="home-showcase-title">A familiar messenger. A much more personal client.</h2>
    <p>Sign in with your existing Telegram account. MelloGram is a standalone client built around the official Telegram API, with its own customization platform — not a patch, injector or modified Telegram Desktop installation.</p>
    <ul class="ticks">
      <li>Plugins declare their permissions before you enable them.</li>
      <li>Themes are CSS and QuickCSS applies changes live.</li>
      <li>Backups never include your Telegram login credentials.</li>
    </ul>
    <p><a href="${href("/faq/")}">Read the FAQ and getting-started guide</a></p>
  </div>
  <figure class="home-showcase-shot">
    ${img("screenshot-plugins.webp", "MelloGram plugin manager showing built-in extensions and permission information", "MelloGram plugin experience")}
  </figure>
</section>

<section class="home-faq section" aria-labelledby="faq">
  <div class="home-section-heading"><p class="eyebrow">Good to know</p><h2 id="faq">Frequently asked questions</h2></div>
  ${home.map(([q, a]) => `<details class="faq"><summary><h3>${esc(q)}</h3></summary><p>${esc(a)}</p></details>`).join("")}
  <p><a href="${href("/faq/")}">Read all questions and answers</a></p>
</section>

<section class="home-download section" aria-labelledby="get">
  <p class="eyebrow">Ready when you are</p>
  <h2 id="get">Give your Telegram a little more you.</h2>
  <p>Free, open source and available across your devices. Download MelloGram and start customizing.</p>
  <div class="cta home-cta">
    <a class="btn" href="${href("/download/")}" data-download>Download MelloGram</a>
    <a class="btn btn-ghost" href="${c.REPO}" rel="noopener">View source on GitHub</a>
  </div>
</section>`,
    },

    {
      path: "/download/",
      crumb: "Download",
      title: "Download MelloGram: Windows, macOS, Linux, Android",
      description: "Download MelloGram, the free customizable Telegram client, for Windows (.exe, .msi), macOS (.dmg), Linux (.deb, .rpm, AppImage) and Android.",
      h1: "Download MelloGram",
      priority: 0.9,
      changefreq: "weekly",
      jsonld: (c) => [softwareApp(c)],
      body: () => `
<header class="page-head"><h1>Download MelloGram</h1>
<p class="lead">MelloGram ${esc(c.version)} is free for Windows, macOS, Linux and Android. Pick your platform below; every file is built automatically from the public source code and published on GitHub.</p></header>

<section class="section" aria-labelledby="platforms">
  <h2 id="platforms">Choose your platform</h2>
  <ul class="cards cards-2">
    ${[["windows", "Windows", "Windows 10 and 11, 64-bit."], ["macos", "macOS", "macOS 11 Big Sur or newer, Apple Silicon and Intel."], ["linux", "Linux", "64-bit distributions with WebKitGTK 4.1."], ["android", "Android", "Android 7.0 or newer."]].map(([id, name, req]) => `
    <li class="card" id="${id}"><h3>MelloGram for ${name}</h3><p>${req}</p>
      <ul class="dl-list">${c.downloads[id].map(([label, link, note]) => `<li><a class="btn${label === c.downloads[id][0][0] ? "" : " btn-ghost"}" href="${link}" rel="noopener" data-os="${id}">Download ${esc(label)}</a><span class="muted small">${esc(note)}</span></li>`).join("")}</ul>
    </li>`).join("")}
  </ul>
  <p class="muted">All files come from the <a href="${c.REPO}/releases/latest" rel="noopener">latest GitHub release</a>, built automatically from the public source code. Older versions and checksums are on the <a href="${c.REPO}/releases" rel="noopener">releases page</a>.</p>
</section>

<section class="section" aria-labelledby="install">
  <h2 id="install">How to install MelloGram</h2>
  <ol class="steps">
    <li><strong>Download</strong> the file for your platform from the latest release.</li>
    <li><strong>Install it.</strong> On Windows, run <em>MelloGram-Setup-x64.exe</em> and follow the setup wizard; if SmartScreen warns about a new app, choose <em>More info → Run anyway</em>. On macOS, open the .dmg and drag MelloGram to Applications; the first time, right-click the app and choose <em>Open</em>. On Linux, open the .deb or .rpm with your software centre, or make the AppImage executable and run it. On Android, open the APK and allow installs from your browser.</li>
    <li><strong>Sign in with Telegram:</strong> enter your phone number, the code Telegram sends to your other devices, and your two-step password if you have one.</li>
    <li><strong>Make it yours:</strong> press Ctrl+, to open Settings, then explore Plugins, Themes, Wallpapers and the Appearance Studio.</li>
  </ol>
</section>

<section class="section" aria-labelledby="requirements">
  <h2 id="requirements">System requirements</h2>
  <div class="table-wrap"><table>
    <caption>Minimum requirements for MelloGram ${esc(c.version)}</caption>
    <thead><tr><th scope="col">Platform</th><th scope="col">Requirement</th></tr></thead>
    <tbody>
      <tr><th scope="row">Windows</th><td>Windows 10 (1809) or 11, 64-bit; WebView2 is installed automatically if missing</td></tr>
      <tr><th scope="row">macOS</th><td>macOS 11 Big Sur or newer, Apple Silicon or Intel</td></tr>
      <tr><th scope="row">Linux</th><td>64-bit distribution with WebKitGTK 4.1 and GTK 3</td></tr>
      <tr><th scope="row">Android</th><td>Android 7.0 (API 24) or newer</td></tr>
      <tr><th scope="row">All</th><td>An existing Telegram account and under 100 MB of free disk space</td></tr>
    </tbody>
  </table></div>
</section>

<section class="section" aria-labelledby="verify">
  <h2 id="verify">Is the download safe?</h2>
  <p>Yes. MelloGram releases are built by GitHub Actions directly from the <a href="${c.REPO}" rel="noopener">public MelloGram repository</a>, so anyone can review exactly what goes into each installer. Only download MelloGram from the official releases page linked here.</p>
</section>`,
    },

    {
      path: "/plugins/",
      crumb: "Plugins",
      title: `MelloGram Plugins: All ${c.plugins.length} Built-in Plugins`,
      description: `Browse all ${c.plugins.length} built-in MelloGram plugins for Telegram: link cleaning, message history, templates, privacy tools and more, with permissions.`,
      h1: "MelloGram plugins",
      priority: 0.8,
      images: ["screenshot-plugins.webp"],
      jsonld: (c) => [{
        "@type": "ItemList",
        name: "MelloGram built-in plugins",
        numberOfItems: c.plugins.length,
        itemListElement: c.plugins.map((p, i) => ({ "@type": "ListItem", position: i + 1, name: p.name, description: p.description, url: c.url(`/plugins/#${p.id}`) })),
      }],
      body: () => `
<header class="page-head"><h1>MelloGram plugins</h1>
<p class="lead">MelloGram includes ${c.plugins.length} built-in plugins that you can switch on in Settings → Plugins. Each one lists the permissions it uses, has its own settings panel, and is isolated so a crash cannot take down the app.</p></header>
${img("screenshot-plugins.webp", "MelloGram plugin manager with search, category filter, toggles and permission tags", "MelloGram plugin manager", { cls: "wide" })}
<nav class="toc" aria-label="Plugin categories"><ul>${Object.keys(byCategory).sort().map((k) => `<li><a href="#${k}">${CATEGORY[k] || k}</a></li>`).join("")}</ul></nav>
${Object.keys(byCategory).sort().map((k) => `
<section class="section" aria-labelledby="${k}">
  <h2 id="${k}">${CATEGORY[k] || k} plugins</h2>
  <ul class="plugin-list">${byCategory[k].sort((a, b) => a.name.localeCompare(b.name)).map((p) => `
    <li class="plugin" id="${p.id}">
      <h3>${esc(p.name)}</h3>
      <p>${esc(p.description)}</p>
      <p class="tags">${p.default_on ? '<span class="tag on">On by default</span>' : ""}${p.has_ui ? '<span class="tag">Adds interface</span>' : ""}${p.settings ? `<span class="tag">${p.settings} setting${p.settings === 1 ? "" : "s"}</span>` : ""}${(p.permissions || []).filter((x) => x !== "ui").map((x) => `<span class="tag perm">${PERMISSION[x] || x}</span>`).join("")}</p>
      ${p.inspired_by ? `<p class="muted small">Adapts: ${esc(p.inspired_by)}</p>` : ""}
    </li>`).join("")}
  </ul>
</section>`).join("")}
<section class="section" aria-labelledby="custom">
  <h2 id="custom">Can I write my own MelloGram plugin?</h2>
  <p>Yes. Plugins are Rust types with optional JavaScript UI halves, and the plugin API is documented for developers. Sandboxed third-party plugin loading with permission prompts is on the roadmap. <a href="${href("/developers/")}">Read the developer overview</a>.</p>
</section>`,
    },

    {
      path: "/themes/",
      crumb: "Themes & Wallpapers",
      title: "MelloGram Themes, Wallpapers & Appearance Studio",
      description: "Customize Telegram with MelloGram: CSS themes, live QuickCSS, animated and per-chat wallpapers, and an Appearance Studio for bubbles and fonts.",
      h1: "Themes, wallpapers and the Appearance Studio",
      priority: 0.8,
      images: ["screenshot-wallpapers.webp", "screenshot-appearance-studio.webp"],
      body: () => `
<header class="page-head"><h1>Themes, wallpapers and the Appearance Studio</h1>
<p class="lead">MelloGram lets you restyle Telegram at three levels: CSS themes with editable colour variables, a no-code Appearance Studio, and live QuickCSS — plus a wallpaper engine for what sits behind your messages.</p></header>

<section class="section split" aria-labelledby="wallpapers">
  <div>
    <h2 id="wallpapers">Wallpaper engine</h2>
    <p><strong>MelloGram wallpapers can be your own images, gradients, patterns or animated scenes.</strong> Show them behind your messages or behind the whole window with translucent panels, and set blur, dim, saturation and parallax so text stays readable.</p>
    <ul class="ticks">
      <li>${c.wallpapers.length} presets, including animated Aurora, Nebula and Ocean Waves.</li>
      <li>Upload PNG, JPEG, WebP or GIF images; they are stored only on your device.</li>
      <li>A separate wallpaper for dark mode, and a different wallpaper for any chat.</li>
      <li>Animated wallpapers pause automatically when your system asks for reduced motion.</li>
    </ul>
  </div>
  <figure>${img("screenshot-wallpapers.webp", "MelloGram wallpaper settings with a preset gallery and live preview", "MelloGram wallpaper gallery")}</figure>
</section>

<section class="section" aria-labelledby="presets">
  <h2 id="presets">Built-in wallpaper presets</h2>
  <ul class="swatches">${c.wallpapers.map((w) => `<li><span class="swatch" style="${esc((w.css || "").replace(/inset:[^;]+;/, ""))}" role="img" aria-label="${esc(w.name)} wallpaper preview" title="${esc(w.name)} wallpaper"></span><span>${esc(w.name)}</span></li>`).join("")}</ul>
</section>

<section class="section split reverse" aria-labelledby="appearance-studio">
  <div>
    <h2 id="appearance-studio">Appearance Studio</h2>
    <p><strong>The Appearance Studio changes how MelloGram looks without writing CSS.</strong> Pick a bubble style (rounded, squared, compact, glass, outline or flat), set the radius, spacing and maximum width, choose fonts and density, add gradients to your own messages, and watch a live preview update as you go.</p>
    <p>It also covers avatar shape and size, sidebar width, interface scale, glass panels, reduced motion and per-chat accent colours.</p>
  </div>
  <figure>${img("screenshot-appearance-studio.webp", "MelloGram Appearance Studio with bubble style, radius and spacing controls beside a live preview", "MelloGram Appearance Studio")}</figure>
</section>

<section class="section" aria-labelledby="css-themes">
  <h2 id="css-themes">CSS themes and QuickCSS</h2>
  <p><strong>A MelloGram theme is a CSS file.</strong> Install one from a file or an HTTPS URL, enable several and put them in order, and edit any <code>--mg-*</code> colour variable the theme exposes with a colour picker. QuickCSS is a live editor for your own rules and always applies last.</p>
  <pre><code>/**
 * @name Sunset
 * @author you
 */
:root {
  --mg-accent: #ff8fab;
  --mg-bubble-out: linear-gradient(135deg, #ff9a8b, #7f53ac);
}</code></pre>
  <p>Themes cannot run code: MelloGram only applies them as styles, and shared profiles are inspected before import.</p>
</section>`,
    },

    {
      path: "/faq/",
      crumb: "FAQ",
      title: "MelloGram FAQ: Safety, Features & Platforms",
      description: "Answers about MelloGram: is it free and safe, does it collect data, how it differs from Telegram Desktop and Vencord, plugins, themes and platforms.",
      h1: "MelloGram frequently asked questions",
      priority: 0.7,
      jsonld: () => [faqJsonLd(faqList)],
      body: () => `
<header class="page-head"><h1>MelloGram frequently asked questions</h1>
<p class="lead">Short, direct answers about what MelloGram is, how safe it is, and what it can do.</p></header>
${faqList.map(([q, a]) => `<section class="section faq-item" id="${slug(q)}" aria-labelledby="${slug(q)}-q"><h2 id="${slug(q)}-q">${esc(q)}</h2><p>${esc(a)}</p></section>`).join("")}`,
    },

    {
      path: "/privacy/",
      crumb: "Privacy",
      title: "MelloGram Privacy: What Stays on Your Device",
      description: "How MelloGram handles your data: no telemetry, Telegram login only with Telegram, local settings and plugin data, and optional services you control.",
      h1: "Privacy in MelloGram",
      priority: 0.5,
      body: () => `
<header class="page-head"><h1>Privacy in MelloGram</h1>
<p class="lead">MelloGram is built so that your messages, settings and credentials stay between you and Telegram. There is no telemetry, no analytics and no advertising.</p></header>
<section class="section" aria-labelledby="stays-local"><h2 id="stays-local">What stays on your device</h2>
<ul class="ticks"><li>Your Telegram session, which is stored only in MelloGram's settings folder.</li><li>Settings, workspaces, themes, wallpapers and plugin data.</li><li>Message History, Bookmarks and Quick Notes, which are kept locally only.</li><li>Diagnostics and statistics, which are computed locally and never uploaded.</li></ul></section>
<section class="section" aria-labelledby="telegram"><h2 id="telegram">What goes to Telegram</h2>
<p>Your phone number, login code, two-step password and messages go directly to Telegram over Telegram's encrypted MTProto protocol, exactly as with any Telegram client. MelloGram never sends them anywhere else.</p></section>
<section class="section" aria-labelledby="optional"><h2 id="optional">Optional services</h2>
<p>MelloID public profiles and verified connections are opt-in. If you publish a profile, only the fields you mark public are shown, and verification of GitHub, Discord or Steam accounts happens on the server, never from imported data. MelloRPC rich presence is off until you enable it.</p></section>
<section class="section" aria-labelledby="exports"><h2 id="exports">Backups and shared profiles</h2>
<p>Backups and shared workspace profiles never include your Telegram credentials, session or app-lock PIN. Support bundles redact them too.</p></section>
<section class="section" aria-labelledby="limits"><h2 id="limits">What privacy tools cannot do</h2>
<p>Privacy Mode, the app lock and other local controls change what your own screen shows. They cannot stop someone you message from screenshotting, forwarding or copying what you send.</p></section>`,
    },

    {
      path: "/developers/",
      crumb: "Developers",
      title: "MelloGram for Developers: Plugins & Architecture",
      description: "How MelloGram is built: Rust core, Tauri desktop and Android app, the plugin API with permissions, theming variables, diagnostics and the roadmap.",
      h1: "MelloGram for developers",
      priority: 0.6,
      images: ["screenshot-diagnostics.webp"],
      body: () => `
<header class="page-head"><h1>MelloGram for developers</h1>
<p class="lead">MelloGram is a Rust workspace: a backend-neutral core, a pure-Rust Telegram backend, built-in plugins, and a Tauri app for desktop and Android. It is GPL-3.0 and open to contributions.</p></header>
<section class="section" aria-labelledby="architecture"><h2 id="architecture">Architecture</h2>
<div class="table-wrap"><table><caption>MelloGram crates and services</caption><thead><tr><th scope="col">Component</th><th scope="col">Responsibility</th></tr></thead><tbody>
<tr><th scope="row">mellogram-core</th><td>Plugin manager, settings, themes, Appearance Studio, wallpapers, workspaces</td></tr>
<tr><th scope="row">mellogram-telegram</th><td>Telegram MTProto backend built on grammers</td></tr>
<tr><th scope="row">mellogram-plugins</th><td>The ${c.plugins.length} built-in plugins and their UI scripts</td></tr>
<tr><th scope="row">melloid-core</th><td>MelloID profiles, badges and widgets</td></tr>
<tr><th scope="row">mellorpc-core</th><td>MelloRPC rich-presence protocol</td></tr>
<tr><th scope="row">app/src-tauri</th><td>Desktop and Android application shell</td></tr>
<tr><th scope="row">installer/windows</th><td>Branded Inno Setup installer</td></tr>
<tr><th scope="row">services/melloid-public</th><td>Public profile and verified-connection service</td></tr>
</tbody></table></div></section>
<section class="section" aria-labelledby="plugin-api"><h2 id="plugin-api">Plugin API</h2>
<p>A plugin implements the <code>Plugin</code> trait: hooks for outgoing messages, incoming messages, edits, deletions, typing, slash commands and CSS, plus an optional JavaScript half that adds buttons and menus through the <code>MG</code> API. Plugins declare permissions such as <code>ModifyOutgoing</code> or <code>Storage</code>, which users see before enabling them.</p>
<pre><code>impl Plugin for Shout {
    fn manifest(&amp;self) -&gt; PluginManifest { /* id, name, permissions… */ }
    fn run_command(&amp;self, ctx: &amp;PluginContext, name: &amp;str, msg: &amp;mut OutgoingMessage) -&gt; CommandOutcome {
        if name != "shout" { return CommandOutcome::NotHandled; }
        msg.text = msg.text.to_uppercase();
        CommandOutcome::Send
    }
}</code></pre>
<p><a href="${c.REPO}/blob/main/docs/PLUGIN_API.md" rel="noopener">Read the full plugin API documentation</a>.</p></section>
<section class="section split" aria-labelledby="diagnostics"><div><h2 id="diagnostics">Diagnostics and safe mode</h2>
<p>MelloGram's local testing centre checks storage, settings integrity, themes, plugin health and performance, and Telegram connectivity, and exports support bundles with secrets redacted. Safe mode starts the client with plugins, themes and custom CSS turned off.</p></div>
<figure>${img("screenshot-diagnostics.webp", "MelloGram diagnostics dashboard with uptime, ping, message counters and test results", "MelloGram diagnostics")}</figure></section>
<section class="section" aria-labelledby="contribute"><h2 id="contribute">How to contribute</h2>
<ol class="steps"><li>Clone <a href="${c.REPO}" rel="noopener">foulfoxhacks/MelloGram</a> and install Rust and the Tauri prerequisites.</li><li>Run <code>cargo test --workspace --exclude mellogram</code> to check the core, plugins and services.</li><li>Run <code>cargo tauri dev</code> from <code>app/src-tauri</code> to start the app.</li><li>Open a pull request; CI builds the Windows installer, macOS universal .dmg, Linux packages and Android APK.</li></ol></section>`,
    },

    {
      path: "/404",
      noindex: true,
      crumb: "Not found",
      title: "Page not found — MelloGram",
      description: "This MelloGram page does not exist. Go back to the homepage to download the customizable Telegram client or browse plugins and themes.",
      h1: "Page not found",
      body: () => `<header class="page-head"><h1>Page not found</h1><p class="lead">That page does not exist. Try the <a href="${href("/")}">MelloGram homepage</a>, <a href="${href("/download/")}">downloads</a> or <a href="${href("/plugins/")}">plugins</a>.</p></header>`,
    },
  ];
}

export function llms(c, { full = false } = {}) {
  return `# MelloGram

> MelloGram is a free, open-source (GPL-3.0-or-later) customizable Telegram client for Windows, macOS, Linux and Android, built in Rust by FoulFoxHacks. It offers a Vencord-style plugin manager with ${c.plugins.length} built-in plugins, CSS themes and live QuickCSS, a wallpaper engine, an Appearance Studio, workspaces, background/tray operation with native notifications, an app lock and local diagnostics. It is an unofficial client, not affiliated with Telegram, and uses Telegram's official MTProto API.

Key facts:
- Current version: ${c.version}
- Price: free, no ads, no telemetry
- Platforms: Windows 10/11 (setup .exe, .msi), macOS 11+ universal (.dmg), Linux (.deb, .rpm, AppImage), Android 7+ (.apk)
- Direct downloads: ${c.downloads.windows[0][1]} · ${c.downloads.macos[0][1]} · ${c.downloads.linux[0][1]} · ${c.downloads.android[0][1]}
- Website: ${c.url("/")}
- Source code: ${c.REPO}
- Not a Vencord port: inspired by its user experience, written from scratch, contains no Vencord code, does not inject into other apps
- Sign-in: normal Telegram account; login codes and 2FA passwords go only to Telegram

## Identity and ownership

- Product: MelloGram
- Product type: desktop and Android communication software / Telegram client
- Author, publisher and maintainer: FoulFoxHacks
- Official website: ${c.url("/")}
- Official source repository: ${c.REPO}
- Brand mark: ${c.url("/assets/logo.png")}
- License: GNU General Public License v3.0 or later (GPL-3.0-or-later)
- Relationship to Telegram: independent third-party client; not affiliated with, endorsed by, or operated by Telegram
- Primary technology: Rust, Tauri and Telegram MTProto
- Primary purpose: customizable Telegram messaging client with plugins, themes, wallpapers and productivity tools

## Pages

- [Home](${c.url("/")}): what MelloGram is, features, comparison with Telegram Desktop
- [Download](${c.url("/download/")}): installers, install steps, system requirements
- [Plugins](${c.url("/plugins/")}): all ${c.plugins.length} built-in plugins with permissions
- [Themes & Wallpapers](${c.url("/themes/")}): CSS themes, QuickCSS, wallpaper engine, Appearance Studio
- [FAQ](${c.url("/faq/")}): safety, privacy, platforms, troubleshooting
- [Privacy](${c.url("/privacy/")}): what stays on your device and what goes to Telegram
- [Developers](${c.url("/developers/")}): architecture, plugin API, contributing

## Plugins

${c.plugins.map((p) => `- ${p.name}: ${p.description}`).join("\n")}
`;
}
