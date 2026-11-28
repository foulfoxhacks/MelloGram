// The Telegram application the browser client signs in with.
//
// These are client application identifiers, not an account credential: the
// same public pair ships inside the MelloGram desktop app (see
// crates/mellogram-core/src/settings.rs). A user's login code, password and
// session key never pass through this API.
const BUILTIN_API_ID = 30886269;
const BUILTIN_API_HASH = "957f1de9d7ccb6e6639fec6ef79ad217";

// Values that turn browser sign-in off for a deployment.
const OFF = new Set(["false", "0", "off", "no"]);

/**
 * Resolves the application credentials the page should use.
 *
 * Browser sign-in is on by default so `wc.mellogram.foulfoxhacks.dev` works out
 * of the box. A deployment can disable it with `TELEGRAM_WEB_BETA=false`, or
 * point it at another registered application with `TELEGRAM_API_ID` and
 * `TELEGRAM_API_HASH`. An incomplete or malformed override disables sign-in
 * rather than mixing half of one application with half of another.
 */
export function telegramConfig(env = {}) {
  if (OFF.has(String(env.TELEGRAM_WEB_BETA ?? "").trim().toLowerCase())) return null;

  const id = String(env.TELEGRAM_API_ID ?? "").trim();
  const hash = String(env.TELEGRAM_API_HASH ?? "").trim();
  if (!id && !hash) return { apiId: BUILTIN_API_ID, apiHash: BUILTIN_API_HASH };

  const apiId = Number(id);
  if (!/^[1-9]\d*$/.test(id) || apiId > 2147483647 || !/^[a-f\d]{32}$/i.test(hash)) return null;
  return { apiId, apiHash: hash };
}
