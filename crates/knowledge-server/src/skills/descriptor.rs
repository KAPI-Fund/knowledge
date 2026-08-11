use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillRuntime {
    Builtin,
    /// 异步技能：在 CubeSandbox 微虚拟机里由 codex CLI 做 agentic 生成，
    /// 产出单文件 HTML。经 canvas_skill_jobs 队列 + skill_worker + SkillExecutor 执行。
    LlmSkill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputSource {
    Selection,
    Argument,
    None,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputSpec {
    pub source: InputSource,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub argument_hint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputSpec {
    pub node_type: String,
    #[serde(default)]
    pub r#async: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillDescriptor {
    pub id: String,
    pub command: String,
    pub name: String,
    pub description: String,
    pub runtime: SkillRuntime,
    #[serde(default)]
    pub entry: Option<String>,
    pub input: InputSpec,
    pub output: OutputSpec,
    /// Absolute path to the skill directory on disk. Filled in by the loader,
    /// not present in skill.toml. Skipped during deserialization.
    #[serde(skip)]
    pub dir: std::path::PathBuf,
}

impl SkillDescriptor {
    pub fn parse_toml(source: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(source)
    }

    pub fn requires_selection(&self) -> bool {
        matches!(self.input.source, InputSource::Selection) && self.input.required
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUIZANG_TOML: &str = r#"
id          = "guizang-ppt"
command     = "ppt"
name        = "PPT 生成"
description = "把选中内容做成单文件 HTML 幻灯片"
runtime     = "llm-skill"
entry       = "SKILL.md"

[input]
source   = "selection"
required = true
argument_hint = "可选:风格 / 要求"

[output]
node_type = "html"
async     = true
"#;

    #[test]
    fn parses_guizang_descriptor() {
        let d = SkillDescriptor::parse_toml(GUIZANG_TOML).expect("parse");
        assert_eq!(d.id, "guizang-ppt");
        assert_eq!(d.command, "ppt");
        assert_eq!(d.runtime, SkillRuntime::LlmSkill);
        assert_eq!(d.entry.as_deref(), Some("SKILL.md"));
        assert_eq!(d.input.source, InputSource::Selection);
        assert!(d.input.required);
        assert_eq!(d.output.node_type, "html");
        assert!(d.output.r#async);
        assert!(d.requires_selection());
    }

    #[test]
    fn builtin_without_selection_does_not_require_selection() {
        let toml = r#"
id = "web-search"
command = "search"
name = "Search"
description = "Web search"
runtime = "builtin"

[input]
source = "argument"
required = false

[output]
node_type = "search"
async = false
"#;
        let d = SkillDescriptor::parse_toml(toml).expect("parse");
        assert_eq!(d.runtime, SkillRuntime::Builtin);
        assert!(!d.requires_selection());
    }
}
