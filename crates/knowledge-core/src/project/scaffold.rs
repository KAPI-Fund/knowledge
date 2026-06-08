use std::fs;
use std::path::Path;

use time::OffsetDateTime;

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
  ".obsidian",
];

pub fn initialize_project(root_path: &Path) -> Result<ProjectRoot, ProjectRootError> {
  fs::create_dir_all(root_path)?;

  for relative in PROJECT_DIRS {
    fs::create_dir_all(root_path.join(relative))?;
  }

  let today = current_date_string();

  fs::write(root_path.join("purpose.md"), purpose_template())?;
  fs::write(root_path.join("schema.md"), schema_template())?;
  fs::write(root_path.join("wiki/index.md"), index_template())?;
  fs::write(root_path.join("wiki/log.md"), log_template(&today))?;
  fs::write(root_path.join("wiki/overview.md"), overview_template())?;
  fs::write(root_path.join(".knowledge/ingest/queue.json"), "{\"tasks\":[]}")?;
  fs::write(root_path.join(".knowledge/reviews/items.json"), "{\"reviews\":[]}")?;
  fs::write(root_path.join(".obsidian/app.json"), obsidian_app_template())?;
  fs::write(
    root_path.join(".obsidian/appearance.json"),
    obsidian_appearance_template(),
  )?;
  fs::write(
    root_path.join(".obsidian/core-plugins.json"),
    obsidian_core_plugins_template(),
  )?;

  ProjectRoot::new(root_path)
}

fn purpose_template() -> &'static str {
  r#"# Project Purpose

## Goal

<!-- What are you trying to understand or build? -->

## Key Questions

<!-- List the primary questions driving this research -->

1.
2.
3.

## Scope

<!-- What is in scope? What is explicitly out of scope? -->

**In scope:**
-

**Out of scope:**
-

## Thesis

<!-- Your current working hypothesis or conclusion (update as research progresses) -->

> TBD
"#
}

fn schema_template() -> &'static str {
  r#"# Wiki Schema

## Page Types

| Type | Directory | Purpose |
|------|-----------|---------|
| entity | wiki/entities/ | Named things such as models, companies, people, and datasets |
| concept | wiki/concepts/ | Ideas, techniques, and phenomena |
| source | wiki/sources/ | Papers, articles, talks, and blog posts |
| query | wiki/queries/ | Open questions under investigation |
| comparison | wiki/comparisons/ | Side-by-side analysis of related entities |
| synthesis | wiki/synthesis/ | Cross-cutting summaries and conclusions |

## Naming Conventions

- Files: `kebab-case.md`
- Entities: match the official name when possible
- Concepts: use descriptive noun phrases
- Sources: prefer `author-year-slug.md`
- Queries: use question-oriented slugs

## Frontmatter

All pages must include YAML frontmatter:

```yaml
---
type: entity | concept | source | query | comparison | synthesis | overview
title: Human-readable title
tags: []
related: []
created: YYYY-MM-DD
updated: YYYY-MM-DD
---
```

Source pages may also include:

```yaml
authors: []
year: YYYY
url: ""
venue: ""
sources: []
```

## Index Format

`wiki/index.md` lists all pages grouped by type. Each entry:

```
- [[page-slug]] - one-line description
```

## Log Format

`wiki/log.md` records research activity in reverse chronological order:

```
## YYYY-MM-DD

- Action taken / finding noted
```

## Cross-referencing Rules

- Use `[[page-slug]]` syntax to link between wiki pages
- Every entity and concept should appear in `wiki/index.md`
- Queries should link to the sources and concepts they draw on
- Synthesis pages should cite contributing sources via frontmatter

## Contradiction Handling

When sources contradict each other:
1. Note the contradiction in the relevant concept or entity page
2. Create or update a query page to track the open question
3. Link both sources from the query page
4. Resolve in a synthesis page once sufficient evidence exists
"#
}

fn index_template() -> &'static str {
  "# Wiki Index\n\n## Entities\n\n## Concepts\n\n## Sources\n\n## Queries\n\n## Comparisons\n\n## Synthesis\n"
}

fn overview_template() -> &'static str {
  r#"---
type: overview
title: Project Overview
tags: []
related: []
sources: []
---

# Overview

<!-- Provide a high-level summary of what this wiki covers and its current state. Update regularly as understanding deepens. -->
"#
}

fn log_template(today: &str) -> String {
  format!("# Research Log\n\n## {today}\n\n- Project created\n")
}

fn obsidian_app_template() -> &'static str {
  r#"{
  "attachmentFolderPath": "raw/assets",
  "userIgnoreFilters": [
    ".cache",
    ".knowledge",
    ".superpowers"
  ],
  "useMarkdownLinks": false,
  "newLinkFormat": "shortest",
  "showUnsupportedFiles": false
}"#
}

fn obsidian_appearance_template() -> &'static str {
  r#"{
  "baseFontSize": 16,
  "theme": "obsidian"
}"#
}

fn obsidian_core_plugins_template() -> &'static str {
  r#"{
  "file-explorer": true,
  "global-search": true,
  "graph": true,
  "backlink": true,
  "tag-pane": true,
  "page-preview": true,
  "outgoing-link": true,
  "starred": true
}"#
}

fn current_date_string() -> String {
  let now = OffsetDateTime::now_utc().date();
  format!(
    "{:04}-{:02}-{:02}",
    now.year(),
    u8::from(now.month()),
    now.day()
  )
}
