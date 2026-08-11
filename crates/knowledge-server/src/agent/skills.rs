use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_SKILL_FILE_BYTES: usize = 64_000;
const MAX_SKILL_SCAN_DEPTH: usize = 8;

pub const PROJECT_SKILLS_SUBDIR: &str = ".knowledge/skills";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSkill {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub base_dir: String,
    pub location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableAgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
}

pub fn load_skills(
    project_path: &Path,
    global_skills_dir: Option<&Path>,
    requested: &[String],
) -> Vec<AgentSkill> {
    if requested.is_empty() {
        return Vec::new();
    }
    let roots = skill_roots(project_path, global_skills_dir);
    requested
        .iter()
        .filter_map(|name| normalize_skill_name(name))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|name| load_one_skill_from_roots(&roots, &name))
        .collect()
}

pub fn list_available_skills(
    project_path: &Path,
    global_skills_dir: Option<&Path>,
) -> Vec<AvailableAgentSkill> {
    let mut skills = BTreeMap::<String, AvailableAgentSkill>::new();
    for root in skill_roots(project_path, global_skills_dir) {
        for candidate in discover_skill_candidates(&root.path) {
            let Ok(skill) = load_skill_path(&candidate.path, &candidate.id) else {
                continue;
            };
            // `id` is the path slug used for loading. `name` is display-only
            // metadata from frontmatter and may contain spaces or punctuation.
            // Roots are ordered from most specific to least specific. Keep the
            // first occurrence so project-local skills can override global
            // skills with the same id.
            skills
                .entry(candidate.id.clone())
                .or_insert(AvailableAgentSkill {
                    id: candidate.id,
                    name: skill.name,
                    description: skill.description,
                    source: root.source.clone(),
                });
        }
    }
    skills.into_values().collect()
}

#[derive(Debug, Clone)]
struct SkillRoot {
    path: PathBuf,
    source: String,
}

fn skill_roots(project_path: &Path, global_skills_dir: Option<&Path>) -> Vec<SkillRoot> {
    let mut roots = vec![SkillRoot {
        path: project_path.join(PROJECT_SKILLS_SUBDIR),
        source: "project".to_string(),
    }];
    if let Some(global) = global_skills_dir {
        roots.push(SkillRoot {
            path: global.to_path_buf(),
            source: "global".to_string(),
        });
    }
    roots
}

fn load_one_skill_from_roots(roots: &[SkillRoot], name: &str) -> Option<AgentSkill> {
    let name = normalize_skill_name(name)?;
    roots
        .iter()
        .find_map(|root| load_one_skill(&root.path, &name))
}

fn load_one_skill(root: &Path, name: &str) -> Option<AgentSkill> {
    let single_file = root.join(format!("{name}.md"));
    if let Ok(skill) = load_skill_file(&single_file, name) {
        return Some(skill);
    }
    if let Ok(skill) = load_skill_directory(&root.join(name), name) {
        return Some(skill);
    }
    // Skills may be grouped in nested folders. The public id remains the
    // portable directory/file name, while the location in the prompt points to
    // the exact SKILL.md path so the Agent can lazily inspect references.
    discover_skill_candidates(root)
        .into_iter()
        .find(|candidate| candidate.id == name)
        .and_then(|candidate| load_skill_path(&candidate.path, name).ok())
}

#[derive(Debug, Clone)]
struct SkillCandidate {
    id: String,
    path: PathBuf,
}

fn discover_skill_candidates(root: &Path) -> Vec<SkillCandidate> {
    let mut out = Vec::new();
    discover_skill_candidates_inner(root, 0, &mut out);
    out
}

fn discover_skill_candidates_inner(dir: &Path, depth: usize, out: &mut Vec<SkillCandidate>) {
    if depth > MAX_SKILL_SCAN_DEPTH {
        return;
    }
    let Ok(meta) = fs::symlink_metadata(dir) else {
        return;
    };
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_file() {
            if path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
            {
                if let Some(id) = path
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|s| s.to_str())
                    .and_then(normalize_skill_name)
                {
                    out.push(SkillCandidate { id, path });
                }
                continue;
            }
            if path
                .extension()
                .and_then(|s| s.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            {
                if let Some(id) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(normalize_skill_name)
                {
                    out.push(SkillCandidate { id, path });
                }
            }
            continue;
        }
        if meta.is_dir() {
            if is_hidden_or_unsafe_skill_dir(&path) {
                continue;
            }
            discover_skill_candidates_inner(&path, depth + 1, out);
        }
    }
}

fn is_hidden_or_unsafe_skill_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    name.starts_with('.') || name == "node_modules" || normalize_skill_name(name).is_none()
}

fn load_skill_path(path: &Path, fallback_name: &str) -> Result<AgentSkill, String> {
    if path
        .file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
    {
        let dir = path
            .parent()
            .ok_or_else(|| "Skill file has no parent directory".to_string())?;
        return load_skill_directory(dir, fallback_name);
    }
    load_skill_file(path, fallback_name)
}

fn load_skill_file(path: &Path, fallback_name: &str) -> Result<AgentSkill, String> {
    let meta = fs::symlink_metadata(path).map_err(|err| format!("Skill not found: {err}"))?;
    if meta.file_type().is_symlink()
        || !meta.is_file()
        || meta.len() as usize > MAX_SKILL_FILE_BYTES
    {
        return Err("Skill file is not readable or is too large".to_string());
    }
    let raw = fs::read_to_string(path).map_err(|err| format!("Failed to read skill: {err}"))?;
    let (frontmatter, instructions) = split_frontmatter(&raw);
    let name = frontmatter
        .as_deref()
        .and_then(|fm| yaml_string_field(fm, "name"))
        .unwrap_or_else(|| fallback_name.to_string());
    let description = frontmatter
        .as_deref()
        .and_then(|fm| yaml_string_field(fm, "description"))
        .unwrap_or_default();
    if description.trim().is_empty() {
        return Err("Skill description is required".to_string());
    }
    Some(AgentSkill {
        name,
        description,
        instructions: instructions.trim().to_string(),
        base_dir: path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_string_lossy()
            .replace('\\', "/"),
        location: path.to_string_lossy().replace('\\', "/"),
    })
    .filter(|skill| !skill.instructions.is_empty())
    .ok_or_else(|| "Skill instructions are empty".to_string())
}

fn load_skill_directory(dir: &Path, fallback_name: &str) -> Result<AgentSkill, String> {
    let meta = fs::symlink_metadata(dir).map_err(|err| format!("Skill folder not found: {err}"))?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err("Skill folder is not readable".to_string());
    }
    let main_path = find_skill_main_file(dir).unwrap_or_else(|| dir.join("SKILL.md"));
    // Only SKILL.md is injected into the Agent prompt. Supporting Markdown
    // files stay on disk and should be read lazily after the Agent has chosen
    // to use this skill; this keeps automatic skill availability cheap and
    // avoids flooding ordinary chat turns with unused reference material.
    load_skill_file(&main_path, fallback_name)
}

fn find_skill_main_file(dir: &Path) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.eq_ignore_ascii_case("SKILL.md"))
        })
        .map(|entry| entry.path())
}

pub fn normalize_skill_name(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || !is_portable_skill_name(trimmed)
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn split_frontmatter(raw: &str) -> (Option<String>, String) {
    let normalized = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let normalized = normalized.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.starts_with("---\n") {
        return (None, normalized);
    }
    let rest = &normalized[4..];
    if let Some(end) = rest.find("\n---") {
        let fm = rest[..end].to_string();
        let after = rest[end + "\n---".len()..]
            .strip_prefix('\n')
            .unwrap_or(&rest[end + "\n---".len()..])
            .to_string();
        (Some(fm), after)
    } else {
        (None, normalized)
    }
}

fn is_portable_skill_name(value: &str) -> bool {
    if value.ends_with([' ', '.']) {
        return false;
    }
    if value
        .chars()
        .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*') || ch <= '\u{1f}')
    {
        return false;
    }
    let stem = value
        .split('.')
        .next()
        .unwrap_or(value)
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

fn yaml_string_field(frontmatter: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}:");
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with(&prefix) {
            continue;
        }
        let value = trimmed[prefix.len()..].trim();
        let value = value
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
            .unwrap_or(value);
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::fs;
    use uuid::Uuid;

    use super::*;

    fn temp_project() -> PathBuf {
        std::env::temp_dir().join(format!("knowledge-agent-skills-{}", Uuid::new_v4()))
    }

    #[test]
    fn load_skills_reads_frontmatter_skill() {
        let root = temp_project();
        let skills_dir = root.join(PROJECT_SKILLS_SUBDIR);
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Review source quality\n---\nCheck claims carefully.",
        )
        .unwrap();

        let skills = load_skills(&root, None, &["reviewer".to_string()]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "reviewer");
        assert_eq!(skills[0].description, "Review source quality");
        assert_eq!(skills[0].instructions, "Check claims carefully.");
        assert!(skills[0].location.ends_with("/reviewer.md"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_skills_reads_crlf_frontmatter() {
        let root = temp_project();
        let skills_dir = root.join(PROJECT_SKILLS_SUBDIR);
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("reviewer.md"),
            "---\r\nname: reviewer\r\ndescription: Review source quality\r\n---\r\nCheck claims carefully.",
        )
        .unwrap();

        let skills = load_skills(&root, None, &["reviewer".to_string()]);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].instructions, "Check claims carefully.");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_skills_rejects_path_traversal_and_reserved_names() {
        let missing = Path::new("/tmp/missing-knowledge-project");
        assert!(load_skills(missing, None, &["../secret".to_string()]).is_empty());
        assert!(
            load_skills(
                missing,
                None,
                &[
                    "con".to_string(),
                    "a:b".to_string(),
                    "topic.".to_string(),
                    "topic ".to_string(),
                ],
            )
            .is_empty()
        );
    }

    #[test]
    fn oversized_skill_files_are_ignored() {
        let root = temp_project();
        let skills_dir = root.join(PROJECT_SKILLS_SUBDIR);
        fs::create_dir_all(&skills_dir).unwrap();
        let body = "x".repeat(MAX_SKILL_FILE_BYTES + 1);
        fs::write(
            skills_dir.join("huge.md"),
            format!("---\nname: huge\ndescription: Huge skill\n---\n{body}"),
        )
        .unwrap();

        let listed = list_available_skills(&root, None);
        assert!(listed.iter().all(|skill| skill.id != "huge"));
        assert!(load_skills(&root, None, &["huge".to_string()]).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_available_skills_reads_markdown_and_skill_folders() {
        let root = temp_project();
        let skills_dir = root.join(PROJECT_SKILLS_SUBDIR);
        fs::create_dir_all(skills_dir.join("illustrator")).unwrap();
        fs::write(
            skills_dir.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Review source quality\n---\nCheck claims.",
        )
        .unwrap();
        fs::write(
            skills_dir.join("illustrator").join("SKILL.md"),
            "---\nname: illustrator\ndescription: Draw article images\n---\nCreate image prompts.",
        )
        .unwrap();

        let skills = list_available_skills(&root, None);
        let ids = skills
            .iter()
            .map(|skill| skill.id.as_str())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"reviewer"));
        assert!(ids.contains(&"illustrator"));
        assert!(skills.iter().all(|skill| skill.source == "project"));

        let loaded = load_skills(&root, None, &["illustrator".to_string()]);
        assert_eq!(loaded[0].name, "illustrator");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_skill_overrides_global_skill_with_same_id() {
        let root = temp_project();
        let project_dir = root.join("project").join(PROJECT_SKILLS_SUBDIR);
        let global_dir = root.join("global-skills");
        fs::create_dir_all(&project_dir).unwrap();
        fs::create_dir_all(&global_dir).unwrap();
        fs::write(
            project_dir.join("reviewer.md"),
            "---\nname: project reviewer\ndescription: Project variant\n---\nProject instructions.",
        )
        .unwrap();
        fs::write(
            global_dir.join("reviewer.md"),
            "---\nname: global reviewer\ndescription: Global variant\n---\nGlobal instructions.",
        )
        .unwrap();
        fs::write(
            global_dir.join("summarizer.md"),
            "---\nname: summarizer\ndescription: Summarize pages\n---\nSummarize.",
        )
        .unwrap();

        let project_path = root.join("project");
        let listed = list_available_skills(&project_path, Some(&global_dir));
        let reviewer = listed
            .iter()
            .find(|skill| skill.id == "reviewer")
            .expect("reviewer listed");
        assert_eq!(reviewer.source, "project");
        assert_eq!(reviewer.name, "project reviewer");
        let summarizer = listed
            .iter()
            .find(|skill| skill.id == "summarizer")
            .expect("global summarizer listed");
        assert_eq!(summarizer.source, "global");

        let loaded = load_skills(&project_path, Some(&global_dir), &["reviewer".to_string()]);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].instructions, "Project instructions.");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn nested_skill_folder_is_listed_and_loadable() {
        let root = temp_project();
        let skill_dir = root
            .join(PROJECT_SKILLS_SUBDIR)
            .join("writing")
            .join("article-illustrator");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Article Illustrator\ndescription: Draw article images\n---\nUse draw.sh after reading references.",
        )
        .unwrap();

        let skills = list_available_skills(&root, None);
        let article = skills
            .iter()
            .find(|skill| skill.id == "article-illustrator")
            .expect("nested skill should be listed");
        assert_eq!(article.name, "Article Illustrator");

        let loaded = load_skills(&root, None, &[article.id.clone()]);
        assert_eq!(loaded.len(), 1);
        assert!(
            loaded[0]
                .location
                .ends_with("/writing/article-illustrator/SKILL.md")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn skills_without_description_are_ignored() {
        let root = temp_project();
        let skills_dir = root.join(PROJECT_SKILLS_SUBDIR);
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("anonymous.md"),
            "---\nname: anonymous\n---\nDo something.",
        )
        .unwrap();

        let listed = list_available_skills(&root, None);
        assert!(listed.iter().all(|skill| skill.id != "anonymous"));
        assert!(load_skills(&root, None, &["anonymous".to_string()]).is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn load_skills_rejects_symlink_skill_files() {
        use std::os::unix::fs::symlink;

        let root = temp_project();
        let skills_dir = root.join(PROJECT_SKILLS_SUBDIR);
        fs::create_dir_all(&skills_dir).unwrap();
        let target = skills_dir.join("target.md");
        fs::write(
            &target,
            "---\nname: target\ndescription: Target skill\n---\nDo not load through a symlink.",
        )
        .unwrap();
        symlink(&target, skills_dir.join("evil.md")).unwrap();

        assert!(load_skills(&root, None, &["evil".to_string()]).is_empty());
        let listed = list_available_skills(&root, None);
        assert!(listed.iter().all(|skill| skill.id != "evil"));
        let _ = fs::remove_dir_all(root);
    }
}
