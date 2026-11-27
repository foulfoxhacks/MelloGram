// Privacy screen on window blur, and Ctrl+Shift+H panic blur.
const onBlur = () => { if (MG.settings.privacy_screen) document.body.classList.add("mg-privacy-screen"); };
const onFocus = () => document.body.classList.remove("mg-privacy-screen");
const onKey = (e) => {
  if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "h") {
    e.preventDefault();
    document.body.classList.toggle("mg-panic");
  }
};
window.addEventListener("blur", onBlur);
window.addEventListener("focus", onFocus);
document.addEventListener("keydown", onKey);
return () => {
  window.removeEventListener("blur", onBlur);
  window.removeEventListener("focus", onFocus);
  document.removeEventListener("keydown", onKey);
  document.body.classList.remove("mg-privacy-screen", "mg-panic");
};
