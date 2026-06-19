use sqlx::PgPool;

/// Effective access a user has to a project, resolved from space ownership,
/// org membership, and per-KB grants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessRole {
    /// Personal-space owner or org admin: full control incl. managing access.
    Owner,
    /// Read + import + edit.
    Editor,
    /// Read-only (query/search/read wiki).
    Viewer,
}

/// Resolve the requesting user's effective role on a project, or `None` if the
/// user has no access (or the project does not exist).
///
/// Rules:
/// - personal space  -> the owner is `Owner`; anyone else has no access.
/// - org space       -> `org_admin` is `Owner`; an `org_member` has access only
///   via a `project_members` grant (`owner`/`editor` -> Editor, `viewer` ->
///   Viewer); a non-member has no access.
pub async fn project_access_role(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
) -> Result<Option<AccessRole>, sqlx::Error> {
    let space = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
        "SELECT s.kind, s.owner_user_id, s.org_id, s.team_id \
         FROM spaces s JOIN projects p ON p.space_id = s.id \
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let Some((kind, owner_user_id, org_id, team_id)) = space else {
        return Ok(None);
    };

    match kind.as_str() {
        "personal" => {
            if owner_user_id.as_deref() == Some(user_id) {
                Ok(Some(AccessRole::Owner))
            } else {
                Ok(None)
            }
        }
        "org" => {
            let Some(org_id) = org_id else {
                return Ok(None);
            };
            let member_role = sqlx::query_scalar::<_, String>(
                "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
            )
            .bind(&org_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

            match member_role.as_deref() {
                None => Ok(None),
                Some("org_admin") => Ok(Some(AccessRole::Owner)),
                Some(_) => {
                    let grant = sqlx::query_scalar::<_, String>(
                        "SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2",
                    )
                    .bind(project_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    match grant.as_deref() {
                        Some("owner") | Some("editor") => Ok(Some(AccessRole::Editor)),
                        // Public org KB: any org member reads by default.
                        _ => Ok(Some(AccessRole::Viewer)),
                    }
                }
            }
        }
        "team" => {
            let Some(team_id) = team_id else {
                return Ok(None);
            };
            let owning_org = sqlx::query_scalar::<_, String>(
                "SELECT org_id FROM teams WHERE id = $1",
            )
            .bind(&team_id)
            .fetch_optional(pool)
            .await?;
            let Some(owning_org) = owning_org else {
                return Ok(None);
            };
            if is_org_admin(pool, &owning_org, user_id).await? {
                return Ok(Some(AccessRole::Owner));
            }
            let team_role = sqlx::query_scalar::<_, String>(
                "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
            )
            .bind(&team_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            if team_role.as_deref() == Some("leader") {
                return Ok(Some(AccessRole::Editor));
            }
            let grant = sqlx::query_scalar::<_, String>(
                "SELECT role FROM project_members WHERE project_id = $1 AND user_id = $2",
            )
            .bind(project_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            match grant.as_deref() {
                Some("owner") | Some("editor") => Ok(Some(AccessRole::Editor)),
                Some("viewer") => Ok(Some(AccessRole::Viewer)),
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

/// Whether `user_id` may manage per-KB access grants on `project_id`.
///
/// True for the owning org's `org_admin` (public or team KB), and for the
/// `leader` of the team that owns a team-space KB. Personal-space KBs have no
/// grant management (the owner controls them implicitly).
pub async fn can_manage_kb_access(
    pool: &PgPool,
    project_id: &str,
    user_id: &str,
) -> Result<bool, sqlx::Error> {
    let space = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT s.kind, s.org_id, s.team_id \
         FROM spaces s JOIN projects p ON p.space_id = s.id \
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let Some((kind, org_id, team_id)) = space else {
        return Ok(false);
    };

    match kind.as_str() {
        "org" => {
            let Some(org_id) = org_id else {
                return Ok(false);
            };
            is_org_admin(pool, &org_id, user_id).await
        }
        "team" => {
            let Some(team_id) = team_id else {
                return Ok(false);
            };
            let owning_org = sqlx::query_scalar::<_, String>(
                "SELECT org_id FROM teams WHERE id = $1",
            )
            .bind(&team_id)
            .fetch_optional(pool)
            .await?;
            if let Some(owning_org) = owning_org
                && is_org_admin(pool, &owning_org, user_id).await?
            {
                return Ok(true);
            }
            let team_role = sqlx::query_scalar::<_, String>(
                "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2",
            )
            .bind(&team_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(team_role.as_deref() == Some("leader"))
        }
        _ => Ok(false),
    }
}

async fn is_org_admin(pool: &PgPool, org_id: &str, user_id: &str) -> Result<bool, sqlx::Error> {
    let role = sqlx::query_scalar::<_, String>(
        "SELECT role FROM organization_members WHERE org_id = $1 AND user_id = $2",
    )
    .bind(org_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(role.as_deref() == Some("org_admin"))
}
