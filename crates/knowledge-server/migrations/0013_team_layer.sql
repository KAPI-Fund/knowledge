CREATE TABLE teams (
  id TEXT PRIMARY KEY NOT NULL,
  org_id TEXT NOT NULL,
  name TEXT NOT NULL,
  slug TEXT NOT NULL,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(org_id, slug),
  FOREIGN KEY(org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE team_members (
  id TEXT PRIMARY KEY NOT NULL,
  team_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('leader', 'member')),
  created_at TEXT NOT NULL,
  UNIQUE(team_id, user_id),
  FOREIGN KEY(team_id) REFERENCES teams(id) ON DELETE CASCADE,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

ALTER TABLE spaces ADD COLUMN team_id TEXT REFERENCES teams(id) ON DELETE CASCADE;

ALTER TABLE spaces DROP CONSTRAINT spaces_kind_check;
ALTER TABLE spaces DROP CONSTRAINT spaces_check;

ALTER TABLE spaces ADD CONSTRAINT spaces_kind_check
  CHECK (kind IN ('personal', 'org', 'team'));

ALTER TABLE spaces ADD CONSTRAINT spaces_owner_check CHECK (
  (kind = 'personal' AND owner_user_id IS NOT NULL AND org_id IS NULL AND team_id IS NULL) OR
  (kind = 'org' AND org_id IS NOT NULL AND owner_user_id IS NULL AND team_id IS NULL) OR
  (kind = 'team' AND team_id IS NOT NULL AND owner_user_id IS NULL AND org_id IS NULL)
);

CREATE UNIQUE INDEX spaces_team ON spaces(team_id) WHERE kind = 'team';
