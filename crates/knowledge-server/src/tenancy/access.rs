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
    let space = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT s.kind, s.owner_user_id, s.org_id \
         FROM spaces s JOIN projects p ON p.space_id = s.id \
         WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let Some((kind, owner_user_id, org_id)) = space else {
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
                        Some("viewer") => Ok(Some(AccessRole::Viewer)),
                        _ => Ok(None),
                    }
                }
            }
        }
        _ => Ok(None),
    }
}
