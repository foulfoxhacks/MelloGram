// Progressive enhancement only: the site works fully without JavaScript.
(() => {
  const ua = navigator.userAgent;
  const os = /Android/i.test(ua) ? "android" : /iPhone|iPad/i.test(ua) ? null : /Windows/i.test(ua) ? "windows" : /Mac OS X|Macintosh/i.test(ua) ? "macos" : /Linux|X11/i.test(ua) ? "linux" : null;
  if (!os) return;
  const label = { windows: "Windows", macos: "macOS", linux: "Linux", android: "Android" }[os];
  for (const a of document.querySelectorAll("a[data-download]")) {
    a.textContent = `Download for ${label}`;
    a.href = a.href.replace(/\/?$/, "") + `/#${os}`;
  }
  const card = document.getElementById(os);
  if (card) card.classList.add("recommended");
})();
