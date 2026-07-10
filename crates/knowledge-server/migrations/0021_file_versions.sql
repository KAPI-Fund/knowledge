CREATE TABLE file_versions (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  path TEXT NOT NULL,
  author TEXT NOT NULL,
  tool TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX file_versions_project_path_idx
  ON file_versions (project_id, path, created_at DESC);
