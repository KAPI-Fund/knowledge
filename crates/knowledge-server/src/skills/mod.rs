pub mod descriptor;

use std::path::Path;
use std::sync::Arc;

pub use descriptor::{InputSource, SkillDescriptor, SkillRuntime};
use descriptor::{InputSpec, OutputSpec};

/// In-memory registry of skills scanned from the skills directory at startup.
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: Arc<Vec<SkillDescriptor>>,
}

impl SkillRegistry {
    pub fn new(skills: Vec<SkillDescriptor>) -> Self {
        Self { skills: Arc::new(skills) }
    }

    /// Scan `dir` for `*/skill.toml` files and parse each into a descriptor.
    /// Missing directory → empty registry (not an error): builtins are appended
    /// by the caller regardless.
    pub fn load_from_dir(dir: &Path) -> std::io::Result<Vec<SkillDescriptor>> {
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(err) => return Err(err),
        };
        for entry in entries {
            let entry = entry?;
            let sub = entry.path();
            if !sub.is_dir() {
                continue;
            }
            let toml_path = sub.join("skill.toml");
            if !toml_path.exists() {
                continue;
            }
            let source = std::fs::read_to_string(&toml_path)?;
            match SkillDescriptor::parse_toml(&source) {
                Ok(mut d) => {
                    d.dir = sub;
                    out.push(d);
                }
                Err(err) => {
                    tracing::error!(?toml_path, %err, "failed to parse skill.toml; skipping");
                }
            }
        }
        Ok(out)
    }

    pub fn all(&self) -> &[SkillDescriptor] {
        &self.skills
    }

    pub fn by_command(&self, command: &str) -> Option<&SkillDescriptor> {
        self.skills.iter().find(|s| s.command == command)
    }
}

/// The three formerly-hardcoded chat commands, folded into the registry as
/// builtin records so dispatch is uniform (no per-skill if-branches).
pub fn builtin_descriptors() -> Vec<SkillDescriptor> {
    fn builtin(
        id: &str,
        command: &str,
        name: &str,
        description: &str,
        node_type: &str,
        source: InputSource,
    ) -> SkillDescriptor {
        SkillDescriptor {
            id: id.to_string(),
            command: command.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            runtime: SkillRuntime::Builtin,
            entry: None,
            input: InputSpec { source, required: false, argument_hint: None },
            output: OutputSpec { node_type: node_type.to_string(), r#async: false },
            dir: std::path::PathBuf::new(),
        }
    }
    vec![
        builtin("web-search", "search", "Web search", "Search the web and add a note", "search", InputSource::Argument),
        builtin("ai-image", "image", "Image", "Generate an image from a prompt", "ai_image", InputSource::Argument),
        builtin("ai-analyze", "analyze", "Analyze", "Analyze selected nodes", "ai_analyze", InputSource::Selection),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_dir_yields_empty() {
        let dir = std::path::Path::new("/nonexistent/skills/dir/xyz");
        let skills = SkillRegistry::load_from_dir(dir).expect("no error");
        assert!(skills.is_empty());
    }

    #[test]
    fn builtins_cover_the_three_legacy_commands() {
        let reg = SkillRegistry::new(builtin_descriptors());
        for cmd in ["search", "image", "analyze"] {
            let d = reg.by_command(cmd).unwrap_or_else(|| panic!("missing {cmd}"));
            assert_eq!(d.runtime, SkillRuntime::Builtin);
        }
    }
}
