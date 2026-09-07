CREATE TABLE user_preferences (
  user_id INTEGER PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  language TEXT NOT NULL DEFAULT 'en' CHECK (language IN ('en', 'zh-CN'))
);

CREATE TABLE article_summaries (
  object_id INTEGER NOT NULL,
  workspace_id INTEGER NOT NULL,
  user_id INTEGER NOT NULL,
  content_version INTEGER NOT NULL,
  language TEXT NOT NULL CHECK (language IN ('en', 'zh-CN')),
  content TEXT NOT NULL,
  PRIMARY KEY (object_id, user_id),
  FOREIGN KEY (object_id, workspace_id) REFERENCES objects(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (workspace_id, user_id) REFERENCES workspace_members(workspace_id, user_id) ON DELETE CASCADE
);
