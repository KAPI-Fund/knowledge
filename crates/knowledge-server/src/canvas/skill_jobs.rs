use serde_json::Value;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SkillJob {
    pub id: String,
    pub canvas_id: String,
    pub node_id: String,
    pub skill_id: String,
    pub status: String,
    pub input: Value,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub progress: Option<Value>,
    pub created_by: String,
}

#[derive(Debug, Clone)]
pub struct NewSkillJob {
    pub canvas_id: String,
    pub node_id: String,
    pub skill_id: String,
    pub input: Value,
    pub created_by: String,
}

// Column tuple shared by acquire_next_job + get_job reads (matches the SELECT/RETURNING order).
type JobRow = (
    String,
    String,
    String,
    String,
    String,
    Value,
    Option<Value>,
    Option<Value>,
    Option<Value>,
    String,
);

fn row_to_job(row: JobRow) -> SkillJob {
    let (id, canvas_id, node_id, skill_id, status, input, result, error, progress, created_by) =
        row;
    SkillJob { id, canvas_id, node_id, skill_id, status, input, result, error, progress, created_by }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("format current time as RFC3339")
}

pub async fn create_job(pool: &PgPool, job: NewSkillJob) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let input = serde_json::to_string(&job.input).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        "INSERT INTO canvas_skill_jobs
           (id, canvas_id, node_id, skill_id, input, created_by, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $7)",
    )
    .bind(&id)
    .bind(&job.canvas_id)
    .bind(&job.node_id)
    .bind(&job.skill_id)
    .bind(&input)
    .bind(&job.created_by)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Requeue jobs left mid-flight by a worker that died before completing them.
///
/// The lease held by a live worker never expires into a second worker (the lease is
/// shorter than a configurable provider timeout, so live reclaim would double-execute
/// the render). Instead we reclaim only at startup: any job still `running` when the
/// process boots has no owner, so it is safe to requeue and clear its lease. Mirrors
/// [`crate::tasks::recovery::recover_tasks`].
pub async fn recover_skill_jobs(pool: &PgPool) -> Result<(), sqlx::Error> {
    let now = now_rfc3339();
    sqlx::query(
        "UPDATE canvas_skill_jobs
         SET status = 'queued',
             updated_at = $1,
             lease_owner = NULL,
             lease_expires_at = NULL
         WHERE status IN ('queued', 'running')",
    )
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Lease the oldest queued job for `owner`, marking it running for `lease_seconds`.
pub async fn acquire_next_job(
    pool: &PgPool,
    owner: &str,
    lease_seconds: i64,
) -> Result<Option<SkillJob>, sqlx::Error> {
    let now = now_rfc3339();
    let lease_expires_at = OffsetDateTime::now_utc()
        .checked_add(time::Duration::seconds(lease_seconds))
        .expect("compute lease expiry")
        .format(&Rfc3339)
        .expect("format lease expiry as RFC3339");

    let row = sqlx::query_as::<_, JobRow>(
        "WITH next_job AS (
             SELECT id FROM canvas_skill_jobs
             WHERE status = 'queued'
             ORDER BY created_at
             FOR UPDATE SKIP LOCKED
             LIMIT 1
         )
         UPDATE canvas_skill_jobs j
         SET status = 'running',
             started_at = COALESCE(j.started_at, $2),
             updated_at = $2,
             lease_owner = $1,
             lease_expires_at = $3
         FROM next_job
         WHERE j.id = next_job.id
         RETURNING j.id, j.canvas_id, j.node_id, j.skill_id, j.status, j.input, j.result, j.error, j.progress, j.created_by",
    )
    .bind(owner)
    .bind(&now)
    .bind(&lease_expires_at)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(row_to_job))
}

pub async fn complete_job(pool: &PgPool, id: &str, result: Value) -> Result<(), sqlx::Error> {
    let now = now_rfc3339();
    let result = serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string());
    sqlx::query(
        "UPDATE canvas_skill_jobs
         SET status = 'done', result = $2::jsonb, error = NULL, progress = NULL,
             finished_at = $3, updated_at = $3, lease_owner = NULL, lease_expires_at = NULL
         WHERE id = $1",
    )
    .bind(id)
    .bind(&result)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail_job(pool: &PgPool, id: &str, error: Value) -> Result<(), sqlx::Error> {
    let now = now_rfc3339();
    let error = serde_json::to_string(&error).unwrap_or_else(|_| "null".to_string());
    sqlx::query(
        "UPDATE canvas_skill_jobs
         SET status = 'error', error = $2::jsonb, progress = NULL,
             finished_at = $3, updated_at = $3, lease_owner = NULL, lease_expires_at = NULL
         WHERE id = $1",
    )
    .bind(id)
    .bind(&error)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// 覆盖运行中 job 的进度快照(latest-wins;终态由 complete/fail 清空)。
pub async fn update_progress(pool: &PgPool, id: &str, progress: Value) -> Result<(), sqlx::Error> {
    let now = now_rfc3339();
    let progress = serde_json::to_string(&progress).unwrap_or_else(|_| "null".to_string());
    sqlx::query(
        "UPDATE canvas_skill_jobs
         SET progress = $2::jsonb, updated_at = $3
         WHERE id = $1 AND status = 'running'",
    )
    .bind(id)
    .bind(&progress)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// 该用户当前在途(queued+running)的 job 数,入队配额检查用。
pub async fn count_active_jobs_for_user(pool: &PgPool, user_id: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM canvas_skill_jobs
         WHERE created_by = $1 AND status IN ('queued', 'running')",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// queued job 的 1-based 排队位置(与 acquire_next_job 的 created_at 顺序一致,
/// id 作并列破序)。job 不存在或已不在 queued 时返回 None。
pub async fn queue_position(pool: &PgPool, id: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM canvas_skill_jobs j, canvas_skill_jobs target
         WHERE target.id = $1 AND target.status = 'queued'
           AND j.status = 'queued'
           AND (j.created_at, j.id) <= (target.created_at, target.id)",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map(|count| if count == 0 { None } else { Some(count) })
}

pub async fn get_job(pool: &PgPool, id: &str) -> Result<Option<SkillJob>, sqlx::Error> {
    let row = sqlx::query_as::<_, JobRow>(
        "SELECT id, canvas_id, node_id, skill_id, status, input, result, error, progress, created_by
         FROM canvas_skill_jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_job))
}
