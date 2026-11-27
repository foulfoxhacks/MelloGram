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
