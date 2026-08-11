use std::path::PathBuf;
use std::sync::Arc;

use sqlx::PgPool;

use crate::agent::cancel::AgentCancellationRegistry;
use crate::cache::CacheStore;
use crate::canvas::executor::SkillExecutor;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub cache: CacheStore,
    pub project_root: String,
    pub session_ttl_hours: u64,
    pub skill_registry: crate::skills::SkillRegistry,
    pub executor: Arc<dyn SkillExecutor>,
    pub agent_cancellations: AgentCancellationRegistry,
    pub global_skills_dir: Option<PathBuf>,
    /// 单个用户同时在途(queued+running)的 canvas skill job 上限。
    pub skill_jobs_per_user: usize,
}
