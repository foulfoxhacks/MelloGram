CREATE TABLE IF NOT EXISTS oauth_transactions (
  state_hash TEXT PRIMARY KEY NOT NULL,
  provider TEXT NOT NULL CHECK (provider IN ('github', 'discord', 'steam')),
  owner_telegram_id TEXT NOT NULL,
  session_hash TEXT NOT NULL,
  code_verifier TEXT NOT NULL,
  expires_at INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_oauth_transactions_expiry ON oauth_transactions (expires_at);

CREATE TABLE IF NOT EXISTS connections (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  owner_telegram_id TEXT NOT NULL,
  provider TEXT NOT NULL CHECK (provider IN ('github', 'discord', 'steam')),
  provider_user_id TEXT NOT NULL,
  username TEXT NOT NULL,
  display_name TEXT NOT NULL,
  profile_url TEXT NOT NULL,
  avatar_url TEXT,
  verified_at TEXT NOT NULL,
  visible INTEGER NOT NULL DEFAULT 1 CHECK (visible IN (0, 1)),
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  UNIQUE (owner_telegram_id, provider),
  UNIQUE (provider, provider_user_id)
);

CREATE INDEX IF NOT EXISTS idx_connections_owner ON connections (owner_telegram_id);

CREATE TABLE IF NOT EXISTS steam_openid_nonces (
  nonce_hash TEXT PRIMARY KEY NOT NULL,
  expires_at INTEGER NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_steam_openid_nonces_expiry ON steam_openid_nonces (expires_at);
