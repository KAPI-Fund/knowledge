-- Knowledge Canvas: per-user infinite canvas boards + unified asset store.

CREATE TABLE canvases (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    title TEXT NOT NULL,
    document TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(owner_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_canvases_owner_id ON canvases (owner_id);

-- Reserved for future sharing; v1 only ever reads owner_id = current user.
CREATE TABLE canvas_access (
    canvas_id TEXT NOT NULL,
    principal_id TEXT NOT NULL,
    role TEXT NOT NULL,
    PRIMARY KEY (canvas_id, principal_id),
    FOREIGN KEY(canvas_id) REFERENCES canvases(id) ON DELETE CASCADE
);

-- Unified asset store (e.g. generated images). Reusable beyond canvas.
CREATE TABLE assets (
    id TEXT PRIMARY KEY NOT NULL,
    owner_id TEXT NOT NULL,
    mime TEXT NOT NULL,
    bytes BYTEA NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(owner_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX idx_assets_owner_id ON assets (owner_id);
