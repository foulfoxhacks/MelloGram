// Telegram sign-in: resume a session kept in this browser, or start a new one
// by QR code or phone code. The client and its keys live in IndexedDB through
// mtcute's IdbStorage; nothing is sent to the Worker beyond opaque MTProto.
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

const READY_NOTE = "Sign in with Telegram. This browser keeps the session until you sign out.";
const DISABLED_NOTE = "Browser sign-in is disabled on this deployment.";

let available = false;
let current; // the shared client: { sdk, client }
let active; // the attempt in progress, if any
let signedIn = false;
let busy = false;

function setButtons() {
  const off = !available || busy || signedIn;
  qr.disabled = phone.disabled = off;
  cancel.disabled = !active;
}

function clearCode(attempt) {
  if (attempt) clearTimeout(attempt.qrTimer);
  canvas.width = 0;
  qrPanel.hidden = true;
}

// Builds (or rebuilds) the client and restores a session left by an earlier
// visit. A stored session that cannot be checked right now is not fatal: the
// sign-in buttons still work, and the next attempt reconnects.
async function boot() {
  if (!available || busy || active || signedIn || current) return;
  busy = true;
  setButtons();
  try {
    const [sdk, response] = await Promise.all([
      import("./generated/telegram.js"),
      fetch("/api/telegram-config", {
        credentials: "omit",
        cache: "no-store",
        signal: AbortSignal.timeout(8000),
      }),
    ]);
    if (!response.ok) throw new Error("Sign-in unavailable");
    const client = sdk.createTelegram(await response.json());
    if (!available) {
      void client.destroy().catch(() => {});
      return;
    }
    current = { sdk, client };
    note.textContent = "Connecting to Telegram…";
    // A hung connection must not leave the page on "Connecting…": after the
    // timeout the sign-in buttons are offered instead.
    const user = await sdk.resumeSession(client, AbortSignal.timeout(15000)).catch(() => null);
    if (!available) return;
    if (user) return signedInAs(user);
    note.textContent = READY_NOTE;
  } catch {
    current?.client.destroy().catch(() => {});
    current = undefined;
    note.textContent = "Could not reach Telegram. Check your connection and try again.";
  } finally {
    busy = false;
    setButtons();
  }
}

function signedInAs(user) {
  if (!current) return;
  signedIn = true;
  clearCode(active);
  active = undefined;
  form.reset();
  form.hidden = true;
  cancel.hidden = true;
  options.hidden = false;
  submit.disabled = false;
  qrPanel.hidden = true;
  signin.hidden = true;
  note.textContent = "";
  setButtons();
  openChats(current.client, current.sdk, user, ended);
}

// Called by the chats view when the session ends, either because the user
// signed out, cleared this browser, or Telegram revoked the key.
function ended(message) {
  signedIn = false;
  active = undefined;
  clearCode(active);
  form.reset();
  form.hidden = true;
  cancel.hidden = true;
  options.hidden = false;
  signin.hidden = false;
  submit.disabled = false;
  note.textContent = message || READY_NOTE;
  setButtons();
  current?.client.destroy().catch(() => {});
  current = undefined;
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
  submit.disabled = false;
  form.hidden = false;
  input.focus();
}

// A half-finished attempt leaves the socket in an unknown state, so the client
// is thrown away and rebuilt from storage on the next click.
function reset(attempt, message) {
  if (active !== attempt) return;
  active = undefined;
  attempt.abort.abort();
  clearCode(attempt);
  form.reset();
  form.hidden = true;
  cancel.hidden = true;
  options.hidden = false;
  submit.disabled = false;
  note.textContent = message;
  setButtons();
  if (current?.client === attempt.client) {
    current.client.destroy().catch(() => {});
    current = undefined;
  }
  (available ? qr : signin.querySelector("a"))?.focus();
}

async function start(kind) {
  if (!available || active || signedIn) return;
  if (!current) {
    await boot();
    if (!current || !available) return;
  }
  const attempt = { abort: new AbortController(), sdk: current.sdk, client: current.client };
  active = attempt;
  options.hidden = true;
  cancel.hidden = false;
  setButtons();
  note.textContent = kind === "phone"
    ? "Enter your existing Telegram account’s phone number."
    : "Connecting to Telegram for a QR code…";
  try {
    const sdk = attempt.sdk;
    const client = attempt.client;
    if (kind === "phone") {
      field(attempt, "phone", "Phone number with country code", "tel", "tel", "tel");
      return;
    }
    const signal = AbortSignal.any([attempt.abort.signal, AbortSignal.timeout(180000)]);
    const user = await client.signInQr({
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
        note.textContent = attempt.passwordRejected
          ? "Telegram rejected that password. Try again."
          : "Telegram requires your two-step verification password.";
      }),
      invalidPasswordCallback() { attempt.passwordRejected = true; },
    });
    signedInAs(user);
  } catch (error) {
    reset(attempt, attempt.sdk?.telegramError(error, "Could not complete Telegram sign-in. Try again.") ?? "Sign-in could not be loaded. Try again.");
  }
}

export function setLoginAvailability(enabled, unavailable = false) {
  available = enabled;
  if (!enabled) {
    if (current && !signedIn) {
      current.client.destroy().catch(() => {});
      current = undefined;
    }
    qr.disabled = phone.disabled = true;
    if (!active && !signedIn && !signin.hidden) {
      note.textContent = unavailable ? "Could not check sign-in availability. Reload to try again." : DISABLED_NOTE;
    }
    return;
  }
  setButtons();
  if (!active && !signedIn && !current) void boot();
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
      if (!sent.phoneCodeHash) return signedInAs(sent);
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
      signedInAs(user);
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
