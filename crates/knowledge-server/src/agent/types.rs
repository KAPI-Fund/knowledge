use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    Fast,
    Standard,
    Deep,
}

impl Default for AgentMode {
    fn default() -> Self {
        Self::Standard
    }
}

impl AgentMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentSkillMode {
    // Enabled skills are available as a candidate set. The model may choose
    // which one, if any, fits the request.
    Auto,
    // The user explicitly selected this skill for the turn. The runtime
    // narrows skill context to it and tells the model to apply it.
    Explicit,
}

impl Default for AgentSkillMode {
    fn default() -> Self {
        Self::Explicit
    }
}

/// Per-message agent options carried on the chat send request. `None` on the
/// request means the plain RAG chat path; `Some` routes through the agent loop.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageOptions {
    #[serde(default)]
    pub mode: AgentMode,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub skill_mode: AgentSkillMode,
    #[serde(default)]
    pub web: bool,
    // Set when the frontend resumes a run paused by user.ask: the id of the
    // AgentUserInputRequest being answered plus the collected field values.
    // Resume is stateless — the loop restarts with the form result rendered
    // into the user context.
    #[serde(default)]
    pub resume_request_id: Option<String>,
    #[serde(default)]
    pub form_result: Option<serde_json::Value>,
    // Session whitelist for shell.exec. The frontend keeps approved commands
    // per conversation and sends the full list on every agent request; the
    // loop only runs a shell command when it matches this list exactly.
    #[serde(default)]
    pub approved_shell_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentReference {
    pub title: String,
    pub path: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentUserInputOption {
    pub label: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentUserInputField {
    pub id: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<AgentUserInputOption>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentUserInputRequest {
    pub request_id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub fields: Vec<AgentUserInputField>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_options_accept_camelcase_api_shape_with_defaults() {
        let options: AgentMessageOptions = serde_json::from_value(serde_json::json!({
            "mode": "deep",
            "skill": "reviewer",
            "skillMode": "auto",
            "approvedShellCommands": ["python make.py"]
        }))
        .unwrap();

        assert_eq!(options.mode, AgentMode::Deep);
        assert_eq!(options.skill.as_deref(), Some("reviewer"));
        assert_eq!(options.skill_mode, AgentSkillMode::Auto);
        assert!(!options.web);
        assert!(options.resume_request_id.is_none());
        assert_eq!(options.approved_shell_commands, vec!["python make.py"]);
    }

    #[test]
    fn agent_options_default_to_standard_explicit() {
        let options: AgentMessageOptions = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(options.mode, AgentMode::Standard);
        assert_eq!(options.skill_mode, AgentSkillMode::Explicit);
        assert!(options.approved_shell_commands.is_empty());
    }
}
