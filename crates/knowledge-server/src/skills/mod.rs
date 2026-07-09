pub mod descriptor;
pub mod routes;

use std::path::Path;
use std::sync::Arc;

pub use descriptor::{InputSource, SkillDescriptor, SkillRuntime};

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
    /// Missing directory → empty registry (not an error).
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
    fn loads_vendored_guizang_ppt_skill() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
        let skills = SkillRegistry::load_from_dir(&dir).expect("load skills dir");
        let reg = SkillRegistry::new(skills);
        let ppt = reg.by_command("ppt").expect("ppt skill present");
        assert_eq!(ppt.id, "guizang-ppt");
        assert_eq!(ppt.runtime, SkillRuntime::LlmSkill);
        assert!(ppt.requires_selection());
    }
}
