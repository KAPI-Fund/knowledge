CREATE TABLE organizations (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  created_by TEXT NOT NULL,
  created_at TEXT NOT NULL,
  FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE organization_members (
  id TEXT PRIMARY KEY NOT NULL,
  org_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('org_admin', 'org_member')),
  created_at TEXT NOT NULL,
  UNIQUE(org_id, user_id),
  FOREIGN KEY(org_id) REFERENCES organizations(id) ON DELETE CASCADE,
  FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE spaces (
  id TEXT PRIMARY KEY NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('personal', 'org')),
  owner_user_id TEXT,
  org_id TEXT,
  created_at TEXT NOT NULL,
  CHECK (
    (kind = 'personal' AND owner_user_id IS NOT NULL AND org_id IS NULL) OR
    (kind = 'org' AND org_id IS NOT NULL AND owner_user_id IS NULL)
  ),
  FOREIGN KEY(owner_user_id) REFERENCES users(id) ON DELETE CASCADE,
  FOREIGN KEY(org_id) REFERENCES organizations(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX spaces_personal_owner ON spaces(owner_user_id) WHERE kind = 'personal';
CREATE UNIQUE INDEX spaces_org ON spaces(org_id) WHERE kind = 'org';

ALTER TABLE projects ADD COLUMN space_id TEXT REFERENCES spaces(id);

-- Backfill: one personal space per existing user.
INSERT INTO spaces (id, kind, owner_user_id, org_id, created_at)
SELECT gen_random_uuid()::text, 'personal', u.id, NULL, u.created_at
FROM users u;

-- Assign each existing project to its owner's personal space.
UPDATE projects p
SET space_id = s.id
FROM project_members pm
JOIN spaces s ON s.owner_user_id = pm.user_id AND s.kind = 'personal'
WHERE pm.project_id = p.id AND pm.role = 'project_owner';

-- Any project still unassigned (no project_owner row) falls back to the
-- seeded admin's personal space.
UPDATE projects p
SET space_id = s.id
FROM users u
JOIN spaces s ON s.owner_user_id = u.id AND s.kind = 'personal'
WHERE p.space_id IS NULL AND u.username = 'admin';

-- Rename per-KB grant role and instance role.
UPDATE project_members SET role = 'owner' WHERE role = 'project_owner';
UPDATE users SET role = 'operator' WHERE role = 'admin';
UPDATE users SET role = 'user' WHERE role <> 'operator';

-- Every project now belongs to a space.
ALTER TABLE projects ALTER COLUMN space_id SET NOT NULL;
