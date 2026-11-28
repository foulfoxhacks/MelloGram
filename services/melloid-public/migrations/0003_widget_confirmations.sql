CREATE TABLE IF NOT EXISTS widget_confirmations (
  confirmation_hash TEXT PRIMARY KEY NOT NULL,
  state_hash TEXT NOT NULL UNIQUE,
  session_hash TEXT NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_widget_confirmations_expiry ON widget_confirmations (expires_at);
