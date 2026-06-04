use std::fs;
use std::path::Path;

use crate::project::root::{ProjectRoot, ProjectRootError};

const PROJECT_DIRS: &[&str] = &[
  "raw/sources",
  "raw/assets",
  "wiki/entities",
  "wiki/concepts",
  "wiki/sources",
  "wiki/queries",
  "wiki/comparisons",
  "wiki/synthesis",
  ".knowledge/ingest",
  ".knowledge/index",
  ".knowledge/reviews",
  ".knowledge/locks",
];

pub fn initialize_project(root_path: &Path) -> Result<ProjectRoot, ProjectRootError> {
  fs::create_dir_all(root_path)?;

  for relative in PROJECT_DIRS {
    fs::create_dir_all(root_path.join(relative))?;
  }

  fs::write(root_path.join("purpose.md"), purpose_template())?;
  fs::write(root_path.join("schema.md"), schema_template())?;
  fs::write(root_path.join("wiki/index.md"), index_template())?;
  fs::write(root_path.join("wiki/log.md"), "# Research Log\n\n")?;
  fs::write(root_path.join("wiki/overview.md"), overview_template())?;
  fs::write(root_path.join(".knowledge/ingest/queue.json"), "{\"tasks\":[]}")?;

  ProjectRoot::new(root_path)
}

fn purpose_template() -> &'static str {
  "# Project Purpose\n\n## Goal\n\n## Key Questions\n\n1.\n2.\n3.\n"
}

fn schema_template() -> &'static str {
  "# Wiki Schema\n\n## Page Types\n\n- entity\n- concept\n- source\n- query\n- comparison\n- synthesis\n"
}

fn index_template() -> &'static str {
  "# Wiki Index\n\n## Entities\n\n## Concepts\n\n## Sources\n\n## Queries\n\n## Comparisons\n\n## Synthesis\n"
}

fn overview_template() -> &'static str {
  "---\ntype: overview\ntitle: Project Overview\nsources: []\n---\n\n# Overview\n"
}
