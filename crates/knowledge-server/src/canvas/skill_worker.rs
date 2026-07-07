use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::canvas::skill_jobs;
use crate::skills::SkillRuntime;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const LEASE_SECONDS: i64 = 30;
const MAX_CONCURRENT: usize = 2;
const WORKER_OWNER: &str = "canvas-skill-worker";

pub fn spawn_skill_worker(state: crate::AppState) {
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT));
    tokio::spawn(async move {
        loop {
            // Only lease when a permit is free, so we never over-subscribe the LLM.
            let permit = match Arc::clone(&permits).acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };
            match skill_jobs::acquire_next_job(&state.pool, WORKER_OWNER, LEASE_SECONDS).await {
                Ok(Some(job)) => {
                    let state = state.clone();
                    tokio::spawn(async move {
                        run_one(state, job).await;
                        drop(permit); // release only after the job finishes
                    });
                }
                Ok(None) => {
                    drop(permit);
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
                Err(err) => {
                    tracing::error!(%err, "acquire_next_job failed");
                    drop(permit);
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
            }
        }
    });
}

async fn run_one(state: crate::AppState, job: skill_jobs::SkillJob) {
    match execute(&state, &job).await {
        Ok(value) => {
            if let Err(err) = skill_jobs::complete_job(&state.pool, &job.id, value).await {
                tracing::error!(%err, job_id = %job.id, "complete_job failed");
            }
        }
        Err(message) => {
            let payload = serde_json::json!({ "message": message });
            if let Err(err) = skill_jobs::fail_job(&state.pool, &job.id, payload).await {
                tracing::error!(%err, job_id = %job.id, "fail_job failed");
            }
        }
    }
}

async fn execute(
    state: &crate::AppState,
    job: &skill_jobs::SkillJob,
) -> Result<serde_json::Value, String> {
    let descriptor = state
        .skill_registry
        .all()
        .iter()
        .find(|s| s.id == job.skill_id && matches!(s.runtime, SkillRuntime::LlmSkill))
        .cloned()
        .ok_or_else(|| format!("unknown llm skill: {}", job.skill_id))?;

    let selection = job.input.get("selection").and_then(|v| v.as_str()).unwrap_or_default();
    let argument = job.input.get("argument").and_then(|v| v.as_str()).unwrap_or_default();

    // Reuse the same active-connection resolution as ingest.
    let connections =
        crate::providers::list_connections(&state.pool).await.map_err(|e| e.to_string())?;
    let active =
        crate::providers::resolve_active(&connections).ok_or("no active LLM connection")?;
    let provider = crate::providers::ActiveConnection::from(active).provider();

    let outcome =
        crate::canvas::deck_renderer::render_deck(&descriptor, &provider, selection, argument)
            .await
            .map_err(|e| e.to_string())?;

    // Store deck.html as a text/html asset owned by the job creator.
    let asset = crate::assets::store::NewAsset::new(
        &job.created_by,
        "text/html",
        outcome.deck_html.into_bytes(),
    );
    let asset_id =
        crate::assets::store::insert_asset(&state.pool, &asset).await.map_err(|e| e.to_string())?;
    let url = crate::assets::store::asset_url(&asset_id);

    Ok(serde_json::json!({
        "assetId": asset_id,
        "url": url,
        "title": "PPT",
    }))
}
