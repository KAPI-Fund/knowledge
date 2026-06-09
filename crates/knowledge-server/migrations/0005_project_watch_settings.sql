CREATE TABLE IF NOT EXISTS project_watch_settings (
  project_id TEXT PRIMARY KEY NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT FALSE,
  auto_ingest BOOLEAN NOT NULL DEFAULT TRUE,
  path TEXT NOT NULL DEFAULT '',
  include_extensions JSONB NOT NULL DEFAULT '[]'::jsonb,
  exclude_extensions JSONB NOT NULL DEFAULT '[]'::jsonb,
  exclude_dirs JSONB NOT NULL DEFAULT '[]'::jsonb,
  exclude_globs JSONB NOT NULL DEFAULT '[]'::jsonb,
  max_file_size_mb BIGINT NOT NULL DEFAULT 100,
  interval_minutes BIGINT NOT NULL DEFAULT 5,
  last_scan_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE
);
