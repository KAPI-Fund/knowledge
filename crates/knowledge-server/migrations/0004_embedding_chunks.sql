CREATE TABLE project_embedding_chunks (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL,
  page_id TEXT NOT NULL,
  relative_path TEXT NOT NULL,
  title TEXT NOT NULL,
  embedding_model TEXT NOT NULL,
  chunk_index INTEGER NOT NULL,
  heading_path TEXT NOT NULL,
  chunk_text TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  embedding JSONB NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY(project_id) REFERENCES projects(id) ON DELETE CASCADE,
  UNIQUE(project_id, page_id, chunk_index)
);

CREATE INDEX idx_project_embedding_chunks_project_page
  ON project_embedding_chunks(project_id, page_id);
