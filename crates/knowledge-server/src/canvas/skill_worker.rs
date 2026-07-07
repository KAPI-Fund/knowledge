use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::canvas::executor::{RenderProvider, RenderRequest};
use crate::canvas::skill_jobs;
use crate::skills::SkillRuntime;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
// 一次 CubeSandbox+codex 渲染可达数分钟；租约须覆盖真实执行墙钟(> sidecar HTTP 660s)。
const LEASE_SECONDS: i64 = 900;
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

/// 从 active 连接与 job 输入组装 RenderRequest(纯函数，可单测)。
fn build_render_request(
    skill_id: &str,
    selection: &str,
    argument: &str,
    active: &crate::providers::ActiveConnection,
) -> RenderRequest {
    RenderRequest {
        skill_id: skill_id.to_string(),
        selection: selection.to_string(),
        argument: argument.to_string(),
        provider: RenderProvider {
            base_url: active.base_url.clone(),
            api_key: active.api_key.clone(),
            model: active.model.clone(),
        },
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

    let connections =
        crate::providers::list_connections(&state.pool).await.map_err(|e| e.to_string())?;
    let active =
        crate::providers::resolve_active(&connections).ok_or("no active LLM connection")?;
    let active = crate::providers::ActiveConnection::from(active);

    let req = build_render_request(&descriptor.id, selection, argument, &active);
    let rendered = state.executor.render(req).await?;

    let asset = crate::assets::store::NewAsset::new(
        &job.created_by,
        "text/html",
        rendered.deck_html.into_bytes(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_render_request_maps_active_connection() {
        let active = crate::providers::ActiveConnection {
            base_url: "https://api.x/v1".into(),
            api_key: "sk-9".into(),
            model: "gpt-5.4".into(),
            timeout_seconds: 30,
        };
        let req = build_render_request("guizang-ppt", "sel", "arg", &active);
        assert_eq!(req.skill_id, "guizang-ppt");
        assert_eq!(req.selection, "sel");
        assert_eq!(req.argument, "arg");
        assert_eq!(req.provider.base_url, "https://api.x/v1");
        assert_eq!(req.provider.api_key, "sk-9");
        assert_eq!(req.provider.model, "gpt-5.4");
    }
}
