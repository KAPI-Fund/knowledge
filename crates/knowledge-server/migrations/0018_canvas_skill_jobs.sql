-- Async jobs for canvas /skill invocations (llm-skill runtime). Independent of
-- project_tasks (which is project-bound); mirrors only the scheduler *pattern*
-- (lease + status machine), not that table.
CREATE TABLE canvas_skill_jobs (
    id                TEXT PRIMARY KEY NOT NULL DEFAULT (gen_random_uuid()::text),
    canvas_id         TEXT NOT NULL,
    node_id           TEXT NOT NULL,
    skill_id          TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'queued'
                          CHECK (status IN ('queued','running','done','error')),
    input             JSONB NOT NULL DEFAULT '{}'::jsonb,
    result            JSONB,
    error             JSONB,
    created_by        TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    started_at        TEXT,
    finished_at       TEXT,
    lease_owner       TEXT,
    lease_expires_at  TEXT
);

CREATE INDEX canvas_skill_jobs_queued_idx
    ON canvas_skill_jobs (status, created_at)
    WHERE status IN ('queued', 'running');
