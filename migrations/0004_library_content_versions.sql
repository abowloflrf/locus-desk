ALTER TABLE library_items
  ADD COLUMN refresh_status TEXT NOT NULL DEFAULT 'IDLE'
    CHECK (refresh_status IN ('IDLE', 'PENDING', 'FAILED', 'REVIEW'));

ALTER TABLE library_items
  ADD COLUMN refresh_error TEXT
    CHECK (refresh_error IS NULL OR length(refresh_error) <= 4096);

CREATE TABLE library_content_versions (
  id                  INTEGER PRIMARY KEY AUTOINCREMENT,
  uid                 TEXT NOT NULL UNIQUE CHECK (length(uid) BETWEEN 1 AND 128),
  library_item_id     INTEGER NOT NULL,
  workspace_id        INTEGER NOT NULL,
  version_number      INTEGER NOT NULL CHECK (version_number > 0),
  status              TEXT NOT NULL CHECK (status IN ('CURRENT', 'HISTORICAL', 'CANDIDATE')),
  source_blob_id      INTEGER NOT NULL,
  reader_html_blob_id INTEGER NOT NULL,
  reader_text_blob_id INTEGER NOT NULL,
  content_hash        TEXT NOT NULL CHECK (
    length(content_hash) = 64
    AND content_hash NOT GLOB '*[^0-9a-f]*'
  ),
  text_byte_len       INTEGER NOT NULL CHECK (text_byte_len BETWEEN 1 AND 8388608),
  canonical_url       TEXT CHECK (
    canonical_url IS NULL OR length(canonical_url) BETWEEN 1 AND 8192
  ),
  title               TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 1000),
  site_name           TEXT CHECK (site_name IS NULL OR length(site_name) <= 255),
  author              TEXT CHECK (author IS NULL OR length(author) <= 500),
  published_at        INTEGER,
  excerpt             TEXT NOT NULL DEFAULT ''
    CHECK (length(CAST(excerpt AS BLOB)) <= 65536),
  fetched_at          INTEGER NOT NULL,
  created_at          INTEGER NOT NULL,
  UNIQUE (id, workspace_id),
  UNIQUE (library_item_id, version_number),
  FOREIGN KEY (library_item_id, workspace_id)
    REFERENCES library_items(id, workspace_id) ON DELETE CASCADE,
  FOREIGN KEY (source_blob_id, workspace_id)
    REFERENCES blobs(id, workspace_id) ON DELETE RESTRICT,
  FOREIGN KEY (reader_html_blob_id, workspace_id)
    REFERENCES blobs(id, workspace_id) ON DELETE RESTRICT,
  FOREIGN KEY (reader_text_blob_id, workspace_id)
    REFERENCES blobs(id, workspace_id) ON DELETE RESTRICT,
  FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX idx_library_content_versions_current
  ON library_content_versions(library_item_id)
  WHERE status = 'CURRENT';

CREATE UNIQUE INDEX idx_library_content_versions_candidate
  ON library_content_versions(library_item_id)
  WHERE status = 'CANDIDATE';

CREATE INDEX idx_library_content_versions_item_created
  ON library_content_versions(library_item_id, created_at DESC, id DESC);

INSERT INTO library_content_versions (
  uid, library_item_id, workspace_id, version_number, status,
  source_blob_id, reader_html_blob_id, reader_text_blob_id,
  content_hash, text_byte_len, canonical_url, title, site_name, author,
  published_at, excerpt, fetched_at, created_at
)
SELECT
  'content_' || lower(hex(randomblob(16))),
  li.id,
  li.workspace_id,
  CASE WHEN li.content_version > 0 THEN li.content_version ELSE 1 END,
  'CURRENT',
  source_link.blob_id,
  html_link.blob_id,
  text_link.blob_id,
  COALESCE(li.content_hash, html_blob.sha256),
  text_blob.byte_len,
  li.canonical_url,
  li.title,
  li.site_name,
  li.author,
  li.published_at,
  li.excerpt,
  COALESCE(li.fetched_at, o.updated_at),
  COALESCE(li.fetched_at, o.updated_at)
FROM library_items li
JOIN objects o
  ON o.id = li.object_id AND o.workspace_id = li.workspace_id
JOIN object_blobs source_link
  ON source_link.object_id = li.object_id
 AND source_link.workspace_id = li.workspace_id
 AND source_link.purpose = 'SOURCE_HTML'
JOIN object_blobs html_link
  ON html_link.object_id = li.object_id
 AND html_link.workspace_id = li.workspace_id
 AND html_link.purpose = 'READER_HTML'
JOIN blobs html_blob
  ON html_blob.id = html_link.blob_id
 AND html_blob.workspace_id = html_link.workspace_id
JOIN object_blobs text_link
  ON text_link.object_id = li.object_id
 AND text_link.workspace_id = li.workspace_id
 AND text_link.purpose = 'READER_TEXT'
JOIN blobs text_blob
  ON text_blob.id = text_link.blob_id
 AND text_blob.workspace_id = text_link.workspace_id;
