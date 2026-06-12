-- Page identity moves from basename stem to project-relative path.
-- Safe under UNIQUE(project_id, page_id, chunk_index): the old upsert kept
-- at most one page's rows per stem, and each row maps to its own relative_path.
UPDATE project_embedding_chunks
SET page_id = relative_path
WHERE page_id <> relative_path;
