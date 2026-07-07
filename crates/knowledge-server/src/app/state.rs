use std::sync::Arc;

use sqlx::PgPool;

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
}
