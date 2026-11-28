// These identify the Telegram application and are publicly readable when
// enabled. Account credentials and session keys never go through this API.
export function telegramConfig(env) {
  const id = String(env.TELEGRAM_API_ID ?? "");
  const hash = String(env.TELEGRAM_API_HASH ?? "");
  const apiId = Number(id);
  if (env.TELEGRAM_WEB_BETA !== "true" || !/^[1-9]\d*$/.test(id)
      || apiId > 2147483647 || !/^[a-f\d]{32}$/i.test(hash)) return null;
  return { apiId, apiHash: hash };
}
