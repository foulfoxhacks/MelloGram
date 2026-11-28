// The Telegram application the browser client signs in with.
//
// The pair is read from `config/telegram-app.json`, the same file the desktop
// app compiles in (crates/mellogram-core/src/settings.rs), so the two clients
// cannot drift apart. These are client application identifiers, not an account
// credential: a user's login code, password and session key never pass through
// this API.
import app from "../../../config/telegram-app.json" with { type: "json" };

const SHARED_API_ID = Number(app.api_id);
const SHARED_API_HASH = String(app.api_hash ?? "").trim();
const SHARED = /^[a-f\d]{32}$/i.test(SHARED_API_HASH)
  && Number.isInteger(SHARED_API_ID) && SHARED_API_ID > 0 && SHARED_API_ID <= 2147483647
  ? { apiId: SHARED_API_ID, apiHash: SHARED_API_HASH }
  : null;

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
  if (!id && !hash) return SHARED;

  const apiId = Number(id);
  if (!/^[1-9]\d*$/.test(id) || apiId > 2147483647 || !/^[a-f\d]{32}$/i.test(hash)) return null;
  return { apiId, apiHash: hash };
}
