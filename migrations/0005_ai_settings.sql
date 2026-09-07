CREATE TABLE ai_settings (
  workspace_id INTEGER NOT NULL,
  user_id INTEGER NOT NULL,
  base_url TEXT NOT NULL,
  model TEXT NOT NULL,
  api_key TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (workspace_id, user_id),
  FOREIGN KEY (workspace_id, user_id)
    REFERENCES workspace_members(workspace_id, user_id) ON DELETE CASCADE
);
