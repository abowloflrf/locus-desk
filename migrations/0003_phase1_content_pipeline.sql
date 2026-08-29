ALTER TABLE library_items
  ADD COLUMN author TEXT CHECK (author IS NULL OR length(author) <= 500);

ALTER TABLE library_items
  ADD COLUMN published_at INTEGER;

ALTER TABLE library_items
  ADD COLUMN excerpt TEXT NOT NULL DEFAULT ''
    CHECK (length(CAST(excerpt AS BLOB)) <= 65536);

ALTER TABLE library_items
  ADD COLUMN fetched_at INTEGER;

ALTER TABLE library_items
  ADD COLUMN content_hash TEXT CHECK (
    content_hash IS NULL
    OR (
      length(content_hash) = 64
      AND content_hash NOT GLOB '*[^0-9a-f]*'
    )
  );

ALTER TABLE library_items
  ADD COLUMN content_version INTEGER NOT NULL DEFAULT 0 CHECK (content_version >= 0);

CREATE TABLE blobs (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  uid          TEXT NOT NULL UNIQUE CHECK (length(uid) BETWEEN 1 AND 128),
  workspace_id INTEGER NOT NULL,
  sha256       TEXT NOT NULL CHECK (
    length(sha256) = 64
    AND sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  mime_type    TEXT NOT NULL CHECK (length(mime_type) BETWEEN 1 AND 255),
  byte_len     INTEGER NOT NULL CHECK (byte_len BETWEEN 0 AND 8388608),
  body         BLOB NOT NULL CHECK (
    typeof(body) = 'blob'
    AND length(body) = byte_len
    AND length(body) <= 8388608
  ),
  created_at   INTEGER NOT NULL,
  UNIQUE (id, workspace_id),
  UNIQUE (workspace_id, sha256),
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX idx_blobs_workspace_created
  ON blobs(workspace_id, created_at DESC, id DESC);

CREATE TABLE object_blobs (
  object_id   INTEGER NOT NULL,
  workspace_id INTEGER NOT NULL,
  blob_id     INTEGER NOT NULL,
  purpose     TEXT NOT NULL CHECK (
    purpose IN ('SOURCE_HTML', 'READER_HTML', 'READER_TEXT')
  ),
  PRIMARY KEY (object_id, purpose),
  FOREIGN KEY (object_id, workspace_id)
    REFERENCES objects(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (blob_id, workspace_id)
    REFERENCES blobs(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX idx_object_blobs_blob ON object_blobs(blob_id);

CREATE TABLE jobs (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  uid              TEXT NOT NULL UNIQUE CHECK (length(uid) BETWEEN 1 AND 128),
  workspace_id     INTEGER NOT NULL,
  object_id        INTEGER NOT NULL,
  job_type         TEXT NOT NULL CHECK (job_type = 'FETCH_LIBRARY_ITEM'),
  status           TEXT NOT NULL CHECK (
    status IN ('PENDING', 'RUNNING', 'RETRY', 'COMPLETED', 'DEAD')
  ) DEFAULT 'PENDING',
  attempt_count    INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
  max_attempts     INTEGER NOT NULL DEFAULT 5 CHECK (max_attempts > 0),
  run_after        INTEGER NOT NULL,
  lease_owner      TEXT CHECK (
    lease_owner IS NULL OR length(lease_owner) BETWEEN 1 AND 255
  ),
  lease_expires_at INTEGER,
  last_error       TEXT CHECK (last_error IS NULL OR length(last_error) <= 4096),
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL,
  UNIQUE (id, workspace_id),
  FOREIGN KEY (object_id, workspace_id)
    REFERENCES objects(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
  CHECK (
    (status = 'RUNNING' AND lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
    OR (status <> 'RUNNING' AND lease_owner IS NULL AND lease_expires_at IS NULL)
  )
);

CREATE UNIQUE INDEX idx_jobs_active_object_type
  ON jobs(workspace_id, object_id, job_type)
  WHERE status IN ('PENDING', 'RUNNING', 'RETRY');

CREATE INDEX idx_jobs_due
  ON jobs(status, run_after, id)
  WHERE status IN ('PENDING', 'RETRY');

CREATE INDEX idx_jobs_expired_lease
  ON jobs(lease_expires_at, id)
  WHERE status = 'RUNNING';

INSERT INTO jobs (
  uid, workspace_id, object_id, job_type, status, attempt_count, max_attempts,
  run_after, lease_owner, lease_expires_at, last_error, created_at, updated_at
)
SELECT
  'job_' || lower(hex(randomblob(16))),
  library_items.workspace_id,
  objects.id,
  'FETCH_LIBRARY_ITEM',
  'PENDING',
  0,
  5,
  objects.updated_at,
  NULL,
  NULL,
  NULL,
  objects.updated_at,
  objects.updated_at
FROM library_items
JOIN objects
  ON objects.id = library_items.object_id
 AND objects.workspace_id = library_items.workspace_id
WHERE objects.object_type = 'LIBRARY_ITEM'
  AND library_items.processing_status = 'NOT_FETCHED';

UPDATE library_items
SET processing_status = 'PENDING', last_error = NULL
WHERE processing_status = 'NOT_FETCHED';
