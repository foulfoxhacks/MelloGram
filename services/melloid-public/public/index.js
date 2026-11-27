"use strict";
document.addEventListener("DOMContentLoaded", () => {
  const params = new URLSearchParams(location.search);
  if (params.has("error")) {
    const p = document.createElement("p");
    p.className = "result error";
    p.textContent = "Telegram sign-in could not be completed. Please try again.";
    document.querySelector(".hero")?.append(p);
  }
});
