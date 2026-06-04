use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ProjectRoot(PathBuf);

#[derive(Debug, thiserror::Error)]
pub enum ProjectRootError {
  #[error("project root must be an absolute path")]
  NotAbsolute,
  #[error("project root must be a directory")]
  NotDirectory,
  #[error("resolved path escapes project root")]
  EscapesRoot,
  #[error("invalid source payload: {0}")]
  InvalidSourcePayload(String),
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
}

impl ProjectRoot {
  pub fn new(input: impl AsRef<Path>) -> Result<Self, ProjectRootError> {
    let path = input.as_ref();
    if !path.is_absolute() {
      return Err(ProjectRootError::NotAbsolute);
    }
    let canonical = path.canonicalize()?;
    if !canonical.is_dir() {
      return Err(ProjectRootError::NotDirectory);
    }
    Ok(Self(canonical))
  }

  pub fn as_str(&self) -> &str {
    self.0.to_str().unwrap_or_default()
  }

  pub fn as_path(&self) -> &Path {
    &self.0
  }

  pub fn safe_join(&self, relative: &str) -> Result<PathBuf, ProjectRootError> {
    let joined = self.0.join(relative);
    let normalized = joined.components().fold(PathBuf::new(), |mut acc, part| {
      match part {
        Component::ParentDir => {
          acc.pop();
        }
        Component::CurDir => {}
        Component::Prefix(prefix) => acc.push(prefix.as_os_str()),
        Component::RootDir => acc.push(std::path::MAIN_SEPARATOR.to_string()),
        Component::Normal(segment) => acc.push(segment),
      }
      acc
    });

    if !normalized.starts_with(&self.0) {
      return Err(ProjectRootError::EscapesRoot);
    }

    Ok(normalized)
  }
}

#[cfg(test)]
mod tests {
  use super::ProjectRoot;

  #[test]
  fn rejects_relative_project_root() {
    assert!(ProjectRoot::new("relative/path").is_err());
  }

  #[test]
  fn rejects_missing_directory() {
    assert!(ProjectRoot::new("Z:/definitely-missing").is_err());
  }

  #[test]
  fn safe_join_rejects_escape_attempts() {
    let temp = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    assert!(root.safe_join("../secret.md").is_err());
  }
}
