const isRecord = (value) => value !== null && typeof value === "object" && !Array.isArray(value);

function requiredText(value, label, maximum = 80) {
  if (typeof value !== "string" || !value.trim() || [...value].length > maximum) {
    throw new Error(`Provider returned an invalid ${label}.`);
  }
  return value.trim();
}

function safeHttps(value, hostname) {
  if (typeof value !== "string") return null;
  try {
    const url = new URL(value);
    if (url.protocol !== "https:" || url.hostname !== hostname || url.username || url.password) return null;
    return url.toString();
  } catch {
    return null;
  }
}

/** Convert a provider's authenticated /me response to the stored public identity fields. */
export function normalizeProviderIdentity(provider, user) {
  if (!isRecord(user)) throw new Error("Provider returned an invalid account object.");

  if (provider === "github") {
    if (!Number.isSafeInteger(user.id) || user.id <= 0) {
      throw new Error("GitHub returned an invalid stable user ID.");
    }
    const username = requiredText(user.login, "GitHub username", 39);
    const profileUrl = safeHttps(user.html_url, "github.com");
    if (!profileUrl) throw new Error("GitHub returned an unexpected profile URL.");
    const avatarUrl = safeHttps(user.avatar_url, "avatars.githubusercontent.com");
    const name = typeof user.name === "string" && user.name.trim() ? user.name.trim() : username;
    return {
      provider_user_id: String(user.id),
      username,
      display_name: [...name].slice(0, 80).join(""),
      profile_url: profileUrl,
      avatar_url: avatarUrl,
    };
  }

  if (provider === "discord") {
    if (typeof user.id !== "string" || !/^\d{5,24}$/.test(user.id)) {
      throw new Error("Discord returned an invalid stable user ID.");
    }
    const username = requiredText(user.username, "Discord username");
    const globalName = typeof user.global_name === "string" && user.global_name.trim()
      ? user.global_name.trim() : username;
    const avatarHash = typeof user.avatar === "string" && /^[a-zA-Z0-9_]{1,128}$/.test(user.avatar)
      ? user.avatar : null;
    return {
      provider_user_id: user.id,
      username,
      display_name: [...globalName].slice(0, 80).join(""),
      profile_url: `https://discord.com/users/${user.id}`,
      avatar_url: avatarHash ? `https://cdn.discordapp.com/avatars/${user.id}/${avatarHash}.png?size=128` : null,
    };
  }

  throw new Error("Unsupported external account provider.");
}

/** Steam authenticates browser account links with OpenID 2.0, not OAuth. */
export function normalizeSteamIdentity(steamId, summary = null) {
  if (typeof steamId !== "string" || !/^\d{17}$/.test(steamId)) {
    throw new Error("Steam returned an invalid SteamID64.");
  }
  const persona = isRecord(summary) && typeof summary.personaname === "string"
    ? summary.personaname.trim() : "";
  let profileUrl = `https://steamcommunity.com/profiles/${steamId}`;
  let avatarUrl = null;
  if (isRecord(summary)) {
    if (typeof summary.profileurl === "string") {
      try {
        const candidate = new URL(summary.profileurl);
        if (candidate.protocol === "https:" && candidate.hostname === "steamcommunity.com") {
          profileUrl = candidate.toString();
        }
      } catch {}
    }
    if (typeof summary.avatarfull === "string") {
      try {
        const candidate = new URL(summary.avatarfull);
        if (candidate.protocol === "https:"
            && (candidate.hostname.endsWith(".steamstatic.com") || candidate.hostname === "steamcdn-a.akamaihd.net")) {
          avatarUrl = candidate.toString();
        }
      } catch {}
    }
  }
  return {
    provider_user_id: steamId,
    username: steamId,
    display_name: [...(persona || `Steam user ${steamId}`)].slice(0, 80).join(""),
    profile_url: profileUrl,
    avatar_url: avatarUrl,
  };
}

function steamNonceIsRecent(value, nowSeconds) {
  if (typeof value !== "string" || value.length < 20) return false;
  const timestamp = Date.parse(value.slice(0, 20));
  return Number.isFinite(timestamp)
    && timestamp <= (nowSeconds + 60) * 1000
    && timestamp >= (nowSeconds - 600) * 1000;
}

/** Validate Steam's OpenID response envelope before checking its signature server-to-server. */
export function validateSteamOpenIdResponse(params, expectedReturnTo, nowSeconds = Math.floor(Date.now() / 1000)) {
  if (!params || typeof params.get !== "function") return null;
  const claimedId = params.get("openid.claimed_id") || "";
  const identity = params.get("openid.identity") || "";
  const match = claimedId.match(/^https?:\/\/steamcommunity\.com\/openid\/id\/(\d{17})$/);
  const endpoint = params.get("openid.op_endpoint") || "";
  let endpointUrl;
  try { endpointUrl = new URL(endpoint); } catch { return null; }
  const signedFields = new Set((params.get("openid.signed") || "").split(","));
  const requiredSignedFields = ["op_endpoint", "claimed_id", "identity", "return_to", "response_nonce"];
  const signed = requiredSignedFields.every((field) => signedFields.has(field));
  const responseNonce = params.get("openid.response_nonce") || "";
  if (params.get("openid.ns") !== "http://specs.openid.net/auth/2.0"
      || params.get("openid.mode") !== "id_res"
      || endpointUrl.protocol !== "https:"
      || endpointUrl.hostname !== "steamcommunity.com"
      || !["/openid/login", "/openid/"].includes(endpointUrl.pathname)
      || params.get("openid.return_to") !== expectedReturnTo
      || !match
      || identity !== claimedId
      || !signed
      || !(params.get("openid.sig") || "")
      || !steamNonceIsRecent(responseNonce, nowSeconds)) {
    return null;
  }
  return { steamId: match[1], responseNonce, opEndpoint: endpointUrl.toString() };
}
