import { openChats } from "./chats.js";

const byId = (id) => document.getElementById(id);
const signin = byId("signin");
const options = byId("signin-options");
const qr = byId("signin-qr");
const phone = byId("signin-phone");
const note = byId("signin-note");
const qrPanel = byId("qr-panel");
const canvas = byId("qr-code");
const form = byId("signin-form");
const input = byId("signin-value");
const submit = byId("signin-submit");
const cancel = byId("signin-cancel");

let available = false;
let active;

export function setLoginAvailability(enabled, unavailable = false) {
  available = enabled;
  qr.disabled = phone.disabled = !enabled;
  if (!active && !signin.hidden) {
    note.textContent = enabled
      ? "Telegram beta. Sign-in is kept in this tab only."
      : unavailable ? "Could not check sign-in availability. Reload to try again."
        : "Browser sign-in is disabled on this deployment.";
  }
}

function clearCode(attempt) {
  clearTimeout(attempt.qrTimer);
  canvas.width = 0;
  qrPanel.hidden = true;
}

function reset(attempt, message) {
  if (active !== attempt) return;
  active = undefined;
  attempt.abort.abort();
  clearCode(attempt);
  void attempt.client?.destroy().catch(() => {});
  form.reset();
  signin.hidden = false;
  form.hidden = cancel.hidden = true;
  options.hidden = false;
  submit.disabled = false;
  note.textContent = message;
  (available ? qr : signin.querySelector("a")).focus();
}

function signedIn(attempt, user) {
  if (active !== attempt) return;
  clearCode(attempt);
  form.reset();
  delete attempt.phone;
  delete attempt.codeHash;
  delete attempt.password;
  form.hidden = true;
  signin.hidden = true;
  openChats(attempt.client, attempt.sdk, user, (message) => reset(attempt, message));
}

function field(attempt, step, label, type, autocomplete, mode = "text") {
  if (active !== attempt) return;
  attempt.step = step;
  input.value = "";
  input.type = type;
  input.autocomplete = autocomplete;
  input.inputMode = mode;
  byId("signin-label").textContent = label;
  submit.textContent = step === "phone" ? "Request code" : "Continue";
  form.hidden = false;
  input.focus();
}

async function start(kind) {
  if (!available || active) return;
  const attempt = { abort: new AbortController() };
  active = attempt;
  options.hidden = true;
  cancel.hidden = false;
  note.textContent = "Preparing Telegram sign-in…";
  try {
    const [sdk, response] = await Promise.all([
      import("./generated/telegram.js"),
      fetch("/api/telegram-config", {
        credentials: "omit", cache: "no-store",
        signal: AbortSignal.any([attempt.abort.signal, AbortSignal.timeout(8000)]),
      }),
    ]);
    if (!response.ok) throw new Error("Sign-in unavailable");
    const config = await response.json();
    if (active !== attempt) return;
    attempt.sdk = sdk;
    attempt.client = sdk.createTelegram(config);
    if (kind === "phone") {
      note.textContent = "Enter your existing Telegram account’s phone number.";
      field(attempt, "phone", "Phone number with country code", "tel", "tel", "tel");
      return;
    }
    note.textContent = "Connecting to Telegram for a QR code…";
    const signal = AbortSignal.any([attempt.abort.signal, AbortSignal.timeout(180000)]);
    const user = await attempt.client.signInQr({
      abortSignal: signal,
      onUrlUpdated(url, expires) {
        if (active !== attempt) return;
        clearCode(attempt);
        if (expires.getTime() <= Date.now()) return;
        sdk.drawLoginCode(canvas, url);
        qrPanel.hidden = false;
        note.textContent = "Scan the QR code in Telegram. The code refreshes automatically.";
        attempt.qrTimer = setTimeout(() => clearCode(attempt), expires.getTime() - Date.now());
      },
      onQrScanned() {
        if (active !== attempt) return;
        clearCode(attempt);
        note.textContent = "QR code scanned. Finishing sign-in…";
      },
      password: () => new Promise((resolve, reject) => {
        signal.throwIfAborted();
        const abort = () => reject(signal.reason);
        signal.addEventListener("abort", abort, { once: true });
        attempt.password = (value) => {
          signal.removeEventListener("abort", abort);
          resolve(value);
        };
        clearCode(attempt);
        field(attempt, "qr-password", "Telegram two-step verification password", "password", "current-password");
        submit.disabled = false;
        note.textContent = attempt.passwordRejected ? "Telegram rejected that password. Try again." : "Telegram requires your two-step verification password.";
      }),
      invalidPasswordCallback() { attempt.passwordRejected = true; },
    });
    signedIn(attempt, user);
  } catch (error) {
    reset(attempt, attempt.sdk?.telegramError(error, "Could not complete Telegram sign-in. Try again.") ?? "Sign-in could not be loaded. Try again.");
  }
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const attempt = active;
  if (!attempt?.step || submit.disabled) return;
  const { client, sdk, step } = attempt;
  const value = input.value;
  input.value = "";
  if (step === "phone" && !/^\+[1-9]\d{5,14}$/.test(value.trim())) {
    note.textContent = "Enter your phone number with a + and country code.";
    input.focus();
    return;
  }
  submit.disabled = true;
  note.textContent = "Waiting for Telegram…";
  try {
    if (step === "qr-password") {
      attempt.password(value);
      return;
    }
    const abortSignal = AbortSignal.any([attempt.abort.signal, AbortSignal.timeout(30000)]);
    if (step === "phone") {
      attempt.phone = value.trim();
      const sent = await client.sendCode({ phone: attempt.phone, abortSignal });
      if (active !== attempt) return;
      if (!sent.phoneCodeHash) return signedIn(attempt, sent);
      if (["email_required", "firebase", "fragment", "missed_call", "flash_call"].includes(sent.type)) {
        return reset(attempt, "This Telegram verification method is unavailable on the web. Try QR sign-in.");
      }
      attempt.codeHash = sent.phoneCodeHash;
      field(attempt, "code", "Telegram verification code", "text", "one-time-code");
      note.textContent = "Enter the code Telegram sent you.";
    } else {
      const user = step === "code"
        ? await client.signIn({ phone: attempt.phone, phoneCodeHash: attempt.codeHash, phoneCode: value.trim(), abortSignal })
        : await client.checkPassword({ password: value, abortSignal });
      signedIn(attempt, user);
    }
  } catch (error) {
    if (active !== attempt) return;
    if (step === "code" && sdk.needsPassword(error)) {
      field(attempt, "password", "Telegram two-step verification password", "password", "current-password");
      note.textContent = "Telegram requires your two-step verification password.";
    } else {
      note.textContent = sdk.telegramError(error, "Telegram could not complete this step. Try again or use QR sign-in.");
      input.focus();
    }
  } finally {
    if (active === attempt && step !== "qr-password") submit.disabled = false;
  }
});

qr.addEventListener("click", () => void start("qr"));
phone.addEventListener("click", () => void start("phone"));
cancel.addEventListener("click", () => {
  if (active) reset(active, "Sign-in canceled.");
});
