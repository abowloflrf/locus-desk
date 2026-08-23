CREATE TABLE users (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  uid           TEXT NOT NULL UNIQUE,
  username      TEXT NOT NULL UNIQUE,
  password_hash TEXT NOT NULL,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);

CREATE TABLE workspaces (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  uid        TEXT NOT NULL UNIQUE,
  name       TEXT NOT NULL,
  timezone   TEXT NOT NULL,
  created_by INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE workspace_members (
  workspace_id INTEGER NOT NULL,
  user_id      INTEGER NOT NULL,
  role         TEXT NOT NULL CHECK (role IN ('OWNER', 'ADMIN', 'MEMBER')),
  created_at   INTEGER NOT NULL,
  PRIMARY KEY (workspace_id, user_id),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_workspace_members_user ON workspace_members(user_id);

CREATE TABLE sessions (
  token_hash          TEXT PRIMARY KEY,
  user_id             INTEGER NOT NULL,
  active_workspace_id INTEGER NOT NULL,
  created_at          INTEGER NOT NULL,
  expires_at          INTEGER NOT NULL,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
  FOREIGN KEY (active_workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY (active_workspace_id, user_id)
    REFERENCES workspace_members(workspace_id, user_id) ON DELETE CASCADE
);

CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

CREATE TABLE notes (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  uid          TEXT NOT NULL UNIQUE,
  workspace_id INTEGER NOT NULL,
  creator_id   INTEGER NOT NULL,
  content      TEXT NOT NULL,
  status       TEXT NOT NULL CHECK (status IN ('ACTIVE', 'ARCHIVED')) DEFAULT 'ACTIVE',
  pinned       INTEGER NOT NULL CHECK (pinned IN (0, 1)) DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY (creator_id) REFERENCES users(id)
);

CREATE INDEX idx_notes_status_order
  ON notes(workspace_id, status, pinned DESC, created_at DESC, id DESC);

CREATE TABLE note_tags (
  note_id INTEGER NOT NULL,
  tag     TEXT NOT NULL CHECK (length(tag) BETWEEN 1 AND 64),
  PRIMARY KEY (note_id, tag),
  FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
);

CREATE INDEX idx_note_tags_tag ON note_tags(tag);

CREATE TABLE tasks (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  uid          TEXT NOT NULL UNIQUE,
  workspace_id INTEGER NOT NULL,
  creator_id   INTEGER NOT NULL,
  title        TEXT NOT NULL,
  description  TEXT NOT NULL DEFAULT '',
  status       TEXT NOT NULL CHECK (status IN ('TODO', 'DONE')) DEFAULT 'TODO',
  priority     INTEGER NOT NULL CHECK (priority BETWEEN 0 AND 1) DEFAULT 0,
  due_date     TEXT,
  due_time     TEXT,
  sort_key     INTEGER NOT NULL DEFAULT 0,
  completed_at INTEGER,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY (creator_id) REFERENCES users(id),
  CHECK (due_time IS NULL OR due_date IS NOT NULL),
  CHECK (
    (status = 'TODO' AND completed_at IS NULL)
    OR (status = 'DONE' AND completed_at IS NOT NULL)
  )
);

CREATE INDEX idx_tasks_today
  ON tasks(workspace_id, status, due_date, priority DESC, sort_key ASC, created_at ASC);
