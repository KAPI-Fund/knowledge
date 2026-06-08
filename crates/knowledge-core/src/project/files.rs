use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::project::root::{ProjectRoot, ProjectRootError};

pub const DEFAULT_MAX_FILES: usize = 2_000;
pub const HARD_MAX_FILES: usize = 10_000;
pub const MAX_FILE_CONTENT_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectFileRoot {
  Wiki,
  Sources,
  All,
}

impl ProjectFileRoot {
  pub fn as_relative_path(self) -> Option<&'static str> {
    match self {
      Self::Wiki => Some("wiki"),
      Self::Sources => Some("raw/sources"),
      Self::All => None,
    }
  }

  pub fn as_response_root(self) -> &'static str {
    match self {
      Self::Wiki => "wiki",
      Self::Sources => "raw/sources",
      Self::All => "all",
    }
  }
}

#[derive(Debug, Clone)]
pub struct ProjectFileListOptions {
  pub root: ProjectFileRoot,
  pub recursive: bool,
  pub max_files: usize,
}

impl Default for ProjectFileListOptions {
  fn default() -> Self {
    Self {
      root: ProjectFileRoot::Wiki,
      recursive: true,
      max_files: DEFAULT_MAX_FILES,
    }
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileNode {
  pub name: String,
  pub path: String,
  pub is_dir: bool,
  pub size: Option<u64>,
  pub children: Option<Vec<ProjectFileNode>>,
}

#[derive(Debug, Clone)]
pub struct ProjectFileListResult {
  pub root: String,
  pub files: Vec<ProjectFileNode>,
  pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectFileContent {
  pub path: String,
  pub content: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectFilesError {
  #[error("root must be wiki, sources, or all")]
  InvalidRoot,
  #[error("path is not exposed by the project API")]
  NonPublicPath,
  #[error("only text-like project files can be read via this endpoint")]
  NonTextPath,
  #[error("file is too large to return via API")]
  FileTooLarge,
  #[error("file is not valid UTF-8 text")]
  InvalidUtf8,
  #[error("file listing exceeds maxFiles limit ({0})")]
  ListingExceedsMaxFiles(usize),
  #[error("file not found")]
  NotFound,
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
  #[error("{0}")]
  Root(#[from] ProjectRootError),
}

pub fn parse_project_file_root(value: Option<&str>) -> Result<ProjectFileRoot, ProjectFilesError> {
  match value.unwrap_or("wiki") {
    "wiki" => Ok(ProjectFileRoot::Wiki),
    "sources" | "raw" | "raw/sources" => Ok(ProjectFileRoot::Sources),
    "all" | "" => Ok(ProjectFileRoot::All),
    _ => Err(ProjectFilesError::InvalidRoot),
  }
}

pub fn clamp_max_files(value: Option<usize>) -> usize {
  value.unwrap_or(DEFAULT_MAX_FILES).clamp(1, HARD_MAX_FILES)
}

pub fn list_project_files(
  root: &ProjectRoot,
  options: &ProjectFileListOptions,
) -> Result<ProjectFileListResult, ProjectFilesError> {
  let files = match options.root.as_relative_path() {
    Some(relative_path) => {
      let directory = root.safe_join(relative_path)?;
      let mut count = 0usize;
      list_tree(root, &directory, options.recursive, options.max_files, &mut count)?
    }
    None => list_public_roots(root, options.recursive, options.max_files)?,
  };

  Ok(ProjectFileListResult {
    root: options.root.as_response_root().to_string(),
    files,
    truncated: false,
  })
}

pub fn read_project_file_content(
  root: &ProjectRoot,
  relative_path: &str,
) -> Result<ProjectFileContent, ProjectFilesError> {
  if !is_public_project_rel(relative_path) {
    return Err(ProjectFilesError::NonPublicPath);
  }
  if !is_text_content_rel(relative_path) {
    return Err(ProjectFilesError::NonTextPath);
  }

  let path = root.safe_join(relative_path)?;
  let metadata = match fs::metadata(&path) {
    Ok(metadata) => metadata,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
      return Err(ProjectFilesError::NotFound);
    }
    Err(error) => return Err(ProjectFilesError::Io(error)),
  };

  if metadata.len() > MAX_FILE_CONTENT_BYTES {
    return Err(ProjectFilesError::FileTooLarge);
  }

  let content = fs::read_to_string(&path).map_err(|error| match error.kind() {
    std::io::ErrorKind::InvalidData => ProjectFilesError::InvalidUtf8,
    std::io::ErrorKind::NotFound => ProjectFilesError::NotFound,
    _ => ProjectFilesError::Io(error),
  })?;

  Ok(ProjectFileContent {
    path: normalize_path(relative_path),
    content,
  })
}

pub fn is_public_project_rel(relative_path: &str) -> bool {
  let normalized = normalize_path(relative_path);
  if normalized
    .split('/')
    .any(|part| part.is_empty() || part.starts_with('.'))
  {
    return false;
  }

  let lower = normalized.to_lowercase();
  lower == "purpose.md"
    || lower == "schema.md"
    || lower.starts_with("wiki/")
    || lower.starts_with("raw/sources/")
}

pub fn is_text_content_rel(relative_path: &str) -> bool {
  let normalized = normalize_path(relative_path).to_lowercase();
  let extension = Path::new(&normalized)
    .extension()
    .and_then(|value| value.to_str())
    .unwrap_or_default();

  matches!(
    extension,
    "md"
      | "mdx"
      | "txt"
      | "csv"
      | "json"
      | "yaml"
      | "yml"
      | "xml"
      | "html"
      | "htm"
      | "rtf"
      | "log"
  )
}

fn list_public_roots(
  root: &ProjectRoot,
  recursive: bool,
  max_files: usize,
) -> Result<Vec<ProjectFileNode>, ProjectFilesError> {
  let mut count = 0usize;
  let mut roots = Vec::new();

  for relative_path in ["purpose.md", "schema.md", "wiki", "raw/sources"] {
    let path = root.safe_join(relative_path)?;
    if !path.exists() {
      continue;
    }
    push_file_node(root, &path, recursive, max_files, &mut count, &mut roots)?;
  }

  Ok(roots)
}

fn list_tree(
  root: &ProjectRoot,
  path: &Path,
  recursive: bool,
  max_files: usize,
  count: &mut usize,
) -> Result<Vec<ProjectFileNode>, ProjectFilesError> {
  let mut files = Vec::new();

  for entry in fs::read_dir(path)? {
    let entry = entry?;
    push_file_node(root, &entry.path(), recursive, max_files, count, &mut files)?;
  }

  files.sort_by(|left, right| {
    right
      .is_dir
      .cmp(&left.is_dir)
      .then_with(|| left.name.cmp(&right.name))
  });
  Ok(files)
}

fn push_file_node(
  root: &ProjectRoot,
  path: &Path,
  recursive: bool,
  max_files: usize,
  count: &mut usize,
  output: &mut Vec<ProjectFileNode>,
) -> Result<(), ProjectFilesError> {
  let name = path
    .file_name()
    .and_then(|value| value.to_str())
    .unwrap_or_default()
    .to_string();
  if name.starts_with('.') {
    return Ok(());
  }

  let metadata = fs::symlink_metadata(path)?;
  let file_type = metadata.file_type();
  if file_type.is_symlink() {
    return Ok(());
  }

  *count += 1;
  if *count > max_files {
    return Err(ProjectFilesError::ListingExceedsMaxFiles(max_files));
  }

  let is_dir = file_type.is_dir();
  let children = if recursive && is_dir {
    Some(list_tree(root, path, true, max_files, count)?)
  } else {
    None
  };

  output.push(ProjectFileNode {
    name,
    path: relative_to_project(root, path),
    is_dir,
    size: if is_dir { None } else { Some(metadata.len()) },
    children,
  });

  Ok(())
}

fn relative_to_project(root: &ProjectRoot, path: &Path) -> String {
  path
    .strip_prefix(root.as_path())
    .map(path_to_string)
    .unwrap_or_else(|_| path_to_string(path))
}

fn normalize_path(value: &str) -> String {
  value
    .replace('\\', "/")
    .trim_start_matches('/')
    .trim_end_matches('/')
    .to_string()
}

fn path_to_string(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
  use super::{is_public_project_rel, is_text_content_rel, normalize_path};

  #[test]
  fn public_project_paths_exclude_internal_state() {
    assert!(is_public_project_rel("wiki/index.md"));
    assert!(is_public_project_rel("Wiki/index.md"));
    assert!(is_public_project_rel("raw/sources/source.md"));
    assert!(is_public_project_rel("Raw/Sources/source.md"));
    assert!(!is_public_project_rel(".knowledge/reviews/items.json"));
    assert!(!is_public_project_rel("wiki/.draft.md"));
  }

  #[test]
  fn text_content_filter_rejects_binary_extensions() {
    assert!(is_text_content_rel("wiki/index.md"));
    assert!(!is_text_content_rel("wiki/media/image.png"));
    assert!(!is_text_content_rel("raw/sources/book.pdf"));
    assert_eq!(normalize_path("/wiki/index.md"), "wiki/index.md");
  }
}
