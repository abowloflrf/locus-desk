CREATE TABLE objects (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  uid           TEXT NOT NULL UNIQUE,
  workspace_id  INTEGER NOT NULL,
  creator_id    INTEGER NOT NULL,
  object_type   TEXT NOT NULL CHECK (object_type IN ('NOTE', 'TASK', 'LIBRARY_ITEM')),
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  UNIQUE (id, workspace_id),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY (creator_id) REFERENCES users(id)
);

CREATE INDEX idx_objects_workspace_type_updated
  ON objects(workspace_id, object_type, updated_at DESC, id DESC);

INSERT INTO objects (uid, workspace_id, creator_id, object_type, created_at, updated_at)
SELECT uid, workspace_id, creator_id, 'NOTE', created_at, updated_at
FROM notes;

INSERT INTO objects (uid, workspace_id, creator_id, object_type, created_at, updated_at)
SELECT uid, workspace_id, creator_id, 'TASK', created_at, updated_at
FROM tasks;

CREATE TABLE migration_note_tags (
  note_id INTEGER NOT NULL,
  tag     TEXT NOT NULL
);

INSERT INTO migration_note_tags (note_id, tag)
SELECT note_id, tag
FROM note_tags;

DROP TABLE note_tags;

CREATE TABLE notes_v2 (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  object_id    INTEGER NOT NULL UNIQUE,
  uid          TEXT NOT NULL UNIQUE,
  workspace_id INTEGER NOT NULL,
  creator_id   INTEGER NOT NULL,
  content      TEXT NOT NULL,
  status       TEXT NOT NULL CHECK (status IN ('ACTIVE', 'ARCHIVED')) DEFAULT 'ACTIVE',
  pinned       INTEGER NOT NULL CHECK (pinned IN (0, 1)) DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL,
  FOREIGN KEY (object_id, workspace_id)
    REFERENCES objects(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY (creator_id) REFERENCES users(id)
);

INSERT INTO notes_v2 (
  id, object_id, uid, workspace_id, creator_id, content, status, pinned, created_at, updated_at
)
SELECT
  notes.id,
  objects.id,
  notes.uid,
  notes.workspace_id,
  notes.creator_id,
  notes.content,
  notes.status,
  notes.pinned,
  notes.created_at,
  notes.updated_at
FROM notes
JOIN objects
  ON objects.uid = notes.uid
 AND objects.workspace_id = notes.workspace_id
 AND objects.object_type = 'NOTE';

DROP TABLE notes;
ALTER TABLE notes_v2 RENAME TO notes;

CREATE INDEX idx_notes_status_order
  ON notes(workspace_id, status, pinned DESC, created_at DESC, id DESC);

CREATE TABLE note_tags (
  note_id INTEGER NOT NULL,
  tag     TEXT NOT NULL CHECK (length(tag) BETWEEN 1 AND 64),
  PRIMARY KEY (note_id, tag),
  FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
);

INSERT INTO note_tags (note_id, tag)
SELECT note_id, tag
FROM migration_note_tags;

DROP TABLE migration_note_tags;

CREATE INDEX idx_note_tags_tag ON note_tags(tag);

CREATE TABLE tasks_v2 (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  object_id    INTEGER NOT NULL UNIQUE,
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
  FOREIGN KEY (object_id, workspace_id)
    REFERENCES objects(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  FOREIGN KEY (creator_id) REFERENCES users(id),
  CHECK (due_time IS NULL OR due_date IS NOT NULL),
  CHECK (
    (status = 'TODO' AND completed_at IS NULL)
    OR (status = 'DONE' AND completed_at IS NOT NULL)
  )
);

INSERT INTO tasks_v2 (
  id, object_id, uid, workspace_id, creator_id, title, description, status, priority,
  due_date, due_time, sort_key, completed_at, created_at, updated_at
)
SELECT
  tasks.id,
  objects.id,
  tasks.uid,
  tasks.workspace_id,
  tasks.creator_id,
  tasks.title,
  tasks.description,
  tasks.status,
  tasks.priority,
  tasks.due_date,
  tasks.due_time,
  tasks.sort_key,
  tasks.completed_at,
  tasks.created_at,
  tasks.updated_at
FROM tasks
JOIN objects
  ON objects.uid = tasks.uid
 AND objects.workspace_id = tasks.workspace_id
 AND objects.object_type = 'TASK';

DROP TABLE tasks;
ALTER TABLE tasks_v2 RENAME TO tasks;

CREATE INDEX idx_tasks_today
  ON tasks(workspace_id, status, due_date, priority DESC, sort_key ASC, created_at ASC);

CREATE TABLE object_tags (
  object_id INTEGER NOT NULL,
  tag       TEXT NOT NULL CHECK (length(tag) BETWEEN 1 AND 64),
  PRIMARY KEY (object_id, tag),
  FOREIGN KEY (object_id) REFERENCES objects(id) ON DELETE CASCADE
);

INSERT INTO object_tags (object_id, tag)
SELECT notes.object_id, note_tags.tag
FROM note_tags
JOIN notes ON notes.id = note_tags.note_id;

CREATE INDEX idx_object_tags_tag ON object_tags(tag);

CREATE TABLE library_items (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  object_id         INTEGER NOT NULL UNIQUE,
  workspace_id      INTEGER NOT NULL,
  original_url      TEXT NOT NULL CHECK (length(original_url) BETWEEN 1 AND 8192),
  normalized_url    TEXT NOT NULL CHECK (length(normalized_url) BETWEEN 1 AND 8192),
  canonical_url     TEXT CHECK (
    canonical_url IS NULL OR length(canonical_url) BETWEEN 1 AND 8192
  ),
  title             TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 1000),
  site_name         TEXT CHECK (site_name IS NULL OR length(site_name) <= 255),
  item_kind         TEXT NOT NULL CHECK (item_kind IN ('BOOKMARK', 'ARTICLE')) DEFAULT 'BOOKMARK',
  status            TEXT NOT NULL CHECK (status IN ('ACTIVE', 'ARCHIVED')) DEFAULT 'ACTIVE',
  read_at           INTEGER,
  starred           INTEGER NOT NULL CHECK (starred IN (0, 1)) DEFAULT 0,
  processing_status TEXT NOT NULL CHECK (
    processing_status IN ('NOT_FETCHED', 'PENDING', 'READY', 'FAILED')
  ) DEFAULT 'NOT_FETCHED',
  last_error        TEXT CHECK (last_error IS NULL OR length(last_error) <= 4096),
  UNIQUE (id, workspace_id),
  UNIQUE (workspace_id, normalized_url),
  FOREIGN KEY (object_id, workspace_id)
    REFERENCES objects(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX idx_library_items_workspace_status
  ON library_items(workspace_id, status, starred DESC, id DESC);

CREATE UNIQUE INDEX idx_library_items_workspace_canonical_url
  ON library_items(workspace_id, canonical_url)
  WHERE canonical_url IS NOT NULL;

CREATE TABLE library_captures (
  id                INTEGER PRIMARY KEY AUTOINCREMENT,
  uid               TEXT NOT NULL UNIQUE,
  library_item_id   INTEGER NOT NULL,
  workspace_id      INTEGER NOT NULL,
  idempotency_key   TEXT CHECK (
    idempotency_key IS NULL OR length(idempotency_key) BETWEEN 1 AND 255
  ),
  selected_text     TEXT NOT NULL DEFAULT '' CHECK (length(CAST(selected_text AS BLOB)) <= 262144),
  note              TEXT NOT NULL DEFAULT '' CHECK (length(CAST(note AS BLOB)) <= 262144),
  captured_title    TEXT CHECK (captured_title IS NULL OR length(captured_title) <= 1000),
  created_at        INTEGER NOT NULL,
  FOREIGN KEY (library_item_id, workspace_id)
    REFERENCES library_items(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX idx_library_captures_item_created
  ON library_captures(library_item_id, created_at DESC, id DESC);

CREATE UNIQUE INDEX idx_library_captures_workspace_idempotency
  ON library_captures(workspace_id, idempotency_key)
  WHERE idempotency_key IS NOT NULL;
