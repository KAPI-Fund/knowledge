use axum::extract::State;
use axum::http::HeaderMap;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::app::state::AppState;
use crate::auth::principal::resolve_principal;
use crate::http::error::ApiError;
use crate::skills::descriptor::SkillDescriptor;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub command: String,
    pub name: String,
    pub description: String,
    pub requires_selection: bool,
    pub argument_hint: Option<String>,
    pub output_node_type: String,
}

impl From<&SkillDescriptor> for SkillMetadata {
    fn from(d: &SkillDescriptor) -> Self {
        SkillMetadata {
            command: d.command.clone(),
            name: d.name.clone(),
            description: d.description.clone(),
            requires_selection: d.requires_selection(),
            argument_hint: d.input.argument_hint.clone(),
            output_node_type: d.output.node_type.clone(),
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/skills", get(list_skills))
}

async fn list_skills(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SkillMetadata>>, ApiError> {
    let _principal = resolve_principal(&state, &headers).await?;
    let items = state
        .skill_registry
        .all()
        .iter()
        .map(SkillMetadata::from)
        .collect();
    Ok(Json(items))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::descriptor::{InputSource, InputSpec, OutputSpec, SkillRuntime};

    fn llm_skill() -> SkillDescriptor {
        SkillDescriptor {
            id: "guizang-ppt".into(),
            command: "ppt".into(),
            name: "PPT".into(),
            description: "slides".into(),
            runtime: SkillRuntime::LlmSkill,
            entry: Some("SKILL.md".into()),
            input: InputSpec { source: InputSource::Selection, required: true, argument_hint: Some("hint".into()) },
            output: OutputSpec { node_type: "html".into(), r#async: true },
            dir: std::path::PathBuf::new(),
        }
    }

    #[test]
    fn metadata_hides_runtime_and_entry() {
        let meta = SkillMetadata::from(&llm_skill());
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("runtime"));
        assert!(!json.contains("entry"));
        assert!(!json.contains("SKILL.md"));
        assert!(json.contains("\"requiresSelection\":true"));
        assert!(json.contains("\"outputNodeType\":\"html\""));
    }
}
