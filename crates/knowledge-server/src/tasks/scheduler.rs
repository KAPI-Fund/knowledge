use std::time::Duration;

use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::tasks::{executors, store};

const LEASE_SECONDS: i64 = 30;
const SCHEDULER_POLL_INTERVAL_MS: u64 = 250;

pub fn spawn_scheduler(state: AppState) {
  tokio::spawn(async move {
    loop {
      let _ = run_scheduler_tick(&state).await;
      tokio::time::sleep(Duration::from_millis(SCHEDULER_POLL_INTERVAL_MS)).await;
    }
  });
}

pub async fn run_scheduler_tick(state: &AppState) -> Result<bool, ApiError> {
  let Some(task) = store::acquire_next_task(state, "embedded-worker", LEASE_SECONDS).await? else {
    return Ok(false);
  };

  executors::run_task_executor(state, &task).await?;
  Ok(true)
}
