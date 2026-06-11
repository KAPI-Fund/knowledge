# Wiki Page Editing (Phase 3a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add server-side wiki page save (create/update) and batch cascade delete with full reference cleanup — ported from `upstream_llm_wiki/src/lib/wiki-cleanup.ts` and `wiki-page-delete.ts` — and expose both in the admin Files page.

**Architecture:** Pure string-cleanup helpers and filesystem operations live in `knowledge-core` (`project/wiki_cleanup.rs`, `project/wiki_pages.rs`) so they are unit-testable without a server. `knowledge-server` adds two endpoints (`PUT /api/projects/{id}/files/content`, `POST /api/projects/{id}/wiki-pages:delete`) that wrap the core functions with session+CSRF auth, embedding-chunk deletion in Postgres, and audit logs. The admin Files page gains an editor panel (Edit/Save/Cancel/Delete) on the preview pane for `wiki/**/*.md` files.

**Tech Stack:** Rust (axum, sqlx, thiserror, tempfile for tests), React 19 + TanStack Query 5 + Zod, Vitest + Testing Library.

**Upstream semantics being ported (do not deviate):**
- *Bug A guard (false negatives):* a deleted page must be matched by BOTH its slug form (`kv-cache`) and its frontmatter title form (`KV Cache`). Keys are normalized: lowercase, strip path prefix and trailing `.md`, remove whitespace/`-`/`_`.
- *Bug B guard (false positives):* matching is structural (parse `[[target]]`), never substring. Deleting `ai` must NOT touch `[[OpenAI]]`.
- Delete order: snapshot slug+title → delete file → delete embeddings → delete `wiki/media/<slug>/` for `wiki/sources/` pages (slug must be non-empty and not start with `.`) → best-effort sweep of surviving wiki `.md` files (index listing lines, body wikilinks, `related:` frontmatter).
- `[[deleted]]` → `deleted`; `[[deleted|display]]` → `display`; `[[kept]]` unchanged.

---

## File Structure Map

### Create

- `crates/knowledge-core/src/project/wiki_cleanup.rs`
  - pure helpers: `DeletedPageInfo`, `normalize_wiki_ref_key`, `build_deleted_keys`, `extract_frontmatter_title`, `clean_index_listing`, `strip_deleted_wikilinks` + unit tests
- `crates/knowledge-core/src/project/wiki_pages.rs`
  - `is_wiki_page_rel`, `save_wiki_page` (`SavedWikiPage`), `delete_wiki_pages_with_refs` (`WikiPagesDeleteResult`), `WikiPageError` + unit tests
- `apps/admin/src/features/files/wiki-page-editor.tsx`
  - `WikiPageEditor` component + `isEditableWikiPath`
- `apps/admin/src/features/files/wiki-page-editor.test.tsx`
  - component test (mocked mutations)

### Modify

- `crates/knowledge-core/src/project/mod.rs` — register the two new modules
- `crates/knowledge-core/src/ingest.rs:661,708` — make `parse_frontmatter_array` / `write_frontmatter_array` `pub(crate)` (reused for `related:` rewriting)
- `crates/knowledge-server/src/retrieval/store.rs` — add `delete_pages`
- `crates/knowledge-server/src/projects/routes.rs` — `put` route on files/content, new `wiki-pages:delete` route, two handlers, `map_wiki_page_error`
- `tests/rust-integration/tests/file_api.rs` — end-to-end save + cascade-delete test
- `apps/admin/src/features/shared/api.ts` — `saveProjectFileContent`, `deleteProjectWikiPages` + schemas
- `apps/admin/src/features/files/queries.ts` — `useSaveFileContentMutation`, `useDeleteWikiPagesMutation`
- `apps/admin/src/features/files/page.tsx` — mount `WikiPageEditor` in the preview pane

Note on indentation: `crates/knowledge-core` uses 2-space indent; `crates/knowledge-server` and `tests/rust-integration` use 4-space. Match the file you are in.

---

### Task 1: Core cleanup helpers (`wiki_cleanup.rs`)

**Files:**
- Create: `crates/knowledge-core/src/project/wiki_cleanup.rs`
- Modify: `crates/knowledge-core/src/project/mod.rs`

- [ ] **Step 1: Create the module with tests only (failing)**

Write `crates/knowledge-core/src/project/wiki_cleanup.rs` containing ONLY the test module below (no implementations yet), and register the module.

```rust
#[cfg(test)]
mod tests {
  use super::*;

  fn keys(infos: &[(&str, &str)]) -> std::collections::BTreeSet<String> {
    build_deleted_keys(
      &infos
        .iter()
        .map(|(slug, title)| DeletedPageInfo {
          slug: (*slug).to_string(),
          title: (*title).to_string(),
        })
        .collect::<Vec<_>>(),
    )
  }

  #[test]
  fn normalize_collapses_title_and_slug_forms() {
    assert_eq!(normalize_wiki_ref_key("KV Cache"), "kvcache");
    assert_eq!(normalize_wiki_ref_key("kv-cache"), "kvcache");
    assert_eq!(normalize_wiki_ref_key("kv_cache"), "kvcache");
    assert_eq!(normalize_wiki_ref_key("wiki/concepts/kv-cache.md"), "kvcache");
    assert_eq!(normalize_wiki_ref_key("wiki\\concepts\\kv-cache.md"), "kvcache");
  }

  #[test]
  fn frontmatter_title_tolerates_quotes() {
    assert_eq!(
      extract_frontmatter_title("---\ntype: concept\ntitle: \"KV Cache\"\n---\nbody"),
      "KV Cache"
    );
    assert_eq!(extract_frontmatter_title("---\ntitle: 'KV Cache'\n---\n"), "KV Cache");
    assert_eq!(extract_frontmatter_title("---\ntitle:   KV Cache  \n---\n"), "KV Cache");
    assert_eq!(extract_frontmatter_title("no frontmatter"), "");
  }

  #[test]
  fn index_cleanup_drops_title_form_entries_and_keeps_others() {
    let deleted = keys(&[("kv-cache", "KV Cache")]);
    let text = "# Index\n\n- [[KV Cache]] cached attention states\n- [[Attention]] focus mechanism\nplain prose mentioning KV Cache\n";
    let cleaned = clean_index_listing(text, &deleted);
    assert!(!cleaned.contains("- [[KV Cache]]"));
    assert!(cleaned.contains("- [[Attention]] focus mechanism"));
    assert!(cleaned.contains("plain prose mentioning KV Cache"));
  }

  #[test]
  fn wikilink_strip_replaces_deleted_and_ignores_superstrings() {
    let deleted = keys(&[("ai", "AI")]);
    let text = "See [[AI]] and [[OpenAI]] and [[ai|the alias]].";
    let stripped = strip_deleted_wikilinks(text, &deleted);
    assert_eq!(stripped, "See AI and [[OpenAI]] and the alias.");
  }

  #[test]
  fn empty_key_set_is_a_no_op() {
    let deleted = std::collections::BTreeSet::new();
    let text = "- [[Anything]] stays\nbody [[Anything]]";
    assert_eq!(clean_index_listing(text, &deleted), text);
    assert_eq!(strip_deleted_wikilinks(text, &deleted), text);
  }
}
```

Modify `crates/knowledge-core/src/project/mod.rs` (alphabetical-ish placement, current content is `files, enrich, ingest_sanitize, lint, multimodal, page_merge, queries, root, reviews, scaffold, source_identity, source_text, sources`):

```rust
pub mod files;
pub mod enrich;
pub mod ingest_sanitize;
pub mod lint;
pub mod multimodal;
pub mod page_merge;
pub mod queries;
pub mod root;
pub mod reviews;
pub mod scaffold;
pub mod source_identity;
pub mod source_text;
pub mod sources;
pub mod wiki_cleanup;
pub mod wiki_pages;
```

Note: `wiki_pages` does not exist yet — for THIS task only add `pub mod wiki_cleanup;` (add `wiki_pages` in Task 3).

- [ ] **Step 2: Verify compilation fails**

Run: `cargo test -p knowledge-core wiki_cleanup`
Expected: FAIL — `build_deleted_keys`, `DeletedPageInfo`, etc. not found.

- [ ] **Step 3: Implement the helpers**

Prepend to `crates/knowledge-core/src/project/wiki_cleanup.rs` (above the test module):

```rust
//! Pure string helpers for cleaning up wiki references after page
//! deletion. Ported from upstream_llm_wiki/src/lib/wiki-cleanup.ts.
//!
//! Matching is structural (parsed wikilinks + normalized keys), never
//! substring-based: title-form `[[KV Cache]]` matches slug `kv-cache`,
//! while deleting `ai` leaves `[[OpenAI]]` untouched.

use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct DeletedPageInfo {
  pub slug: String,
  pub title: String,
}

pub fn normalize_wiki_ref_key(value: &str) -> String {
  let normalized = value.trim().replace('\\', "/");
  let leaf = normalized.rsplit('/').next().unwrap_or("");
  let lower = leaf.to_lowercase();
  let without_md = lower.strip_suffix(".md").unwrap_or(&lower);
  without_md
    .chars()
    .filter(|ch| !ch.is_whitespace() && *ch != '-' && *ch != '_')
    .collect()
}

pub fn build_deleted_keys(infos: &[DeletedPageInfo]) -> BTreeSet<String> {
  let mut keys = BTreeSet::new();
  for info in infos {
    if !info.slug.is_empty() {
      keys.insert(normalize_wiki_ref_key(&info.slug));
    }
    if !info.title.is_empty() {
      keys.insert(normalize_wiki_ref_key(&info.title));
    }
  }
  keys
}

pub fn extract_frontmatter_title(content: &str) -> String {
  for line in content.lines() {
    let Some(rest) = line.strip_prefix("title:") else {
      continue;
    };
    let trimmed = rest.trim();
    let unquoted = trimmed
      .strip_prefix('"')
      .and_then(|value| value.strip_suffix('"'))
      .or_else(|| {
        trimmed
          .strip_prefix('\'')
          .and_then(|value| value.strip_suffix('\''))
      })
      .unwrap_or(trimmed);
    return unquoted.trim().to_string();
  }
  String::new()
}

pub fn clean_index_listing(text: &str, deleted_keys: &BTreeSet<String>) -> String {
  if deleted_keys.is_empty() {
    return text.to_string();
  }
  text
    .split('\n')
    .filter(|line| {
      let Some(target) = index_entry_target(line) else {
        return true;
      };
      !deleted_keys.contains(&normalize_wiki_ref_key(&target))
    })
    .collect::<Vec<_>>()
    .join("\n")
}

fn index_entry_target(line: &str) -> Option<String> {
  let trimmed = line.trim_start();
  let after_bullet = trimmed
    .strip_prefix('-')
    .or_else(|| trimmed.strip_prefix('*'))?;
  let after_open = after_bullet.trim_start().strip_prefix("[[")?;
  let end = after_open.find("]]")?;
  let inner = &after_open[..end];
  let target = inner.split('|').next().unwrap_or("").trim();
  if target.is_empty() || target.contains(']') {
    return None;
  }
  Some(target.to_string())
}

pub fn strip_deleted_wikilinks(text: &str, deleted_keys: &BTreeSet<String>) -> String {
  if deleted_keys.is_empty() {
    return text.to_string();
  }

  let mut output = String::with_capacity(text.len());
  let mut remaining = text;

  while let Some(start) = remaining.find("[[") {
    let Some(end_offset) = remaining[start + 2..].find("]]") else {
      break;
    };
    let inner = &remaining[start + 2..start + 2 + end_offset];
    let link_end = start + 2 + end_offset + 2;
    output.push_str(&remaining[..start]);

    let mut parts = inner.splitn(2, '|');
    let target = parts.next().unwrap_or("").trim();
    let display = parts.next();

    if target.is_empty()
      || target.contains(']')
      || !deleted_keys.contains(&normalize_wiki_ref_key(target))
    {
      output.push_str(&remaining[start..link_end]);
    } else {
      output.push_str(display.unwrap_or(target));
    }
    remaining = &remaining[link_end..];
  }

  output.push_str(remaining);
  output
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p knowledge-core wiki_cleanup`
Expected: PASS (5 tests).

- [ ] **Step 5: Clippy**

Run: `cargo clippy -p knowledge-core --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/knowledge-core/src/project/wiki_cleanup.rs crates/knowledge-core/src/project/mod.rs
git commit -m "feat: add wiki reference cleanup helpers to knowledge-core"
```

---

### Task 2: Expose frontmatter array helpers from ingest.rs

**Files:**
- Modify: `crates/knowledge-core/src/ingest.rs:661,708`

- [ ] **Step 1: Change visibility**

In `crates/knowledge-core/src/ingest.rs`, change exactly two function signatures (line numbers approximate — search for the names):

Line 661: `fn parse_frontmatter_array(content: &str, field_name: &str) -> Vec<String> {`
→ `pub(crate) fn parse_frontmatter_array(content: &str, field_name: &str) -> Vec<String> {`

Line 708: `fn write_frontmatter_array(content: &str, field_name: &str, values: &[String]) -> String {`
→ `pub(crate) fn write_frontmatter_array(content: &str, field_name: &str, values: &[String]) -> String {`

Do NOT change their bodies. They already handle inline `related: ["a", "b"]` and block-style YAML lists; `write_frontmatter_array` rewrites the field as an inline quoted array and preserves every other frontmatter line.

- [ ] **Step 2: Verify the crate still compiles clean**

Run: `cargo clippy -p knowledge-core --all-targets -- -D warnings`
Expected: clean (no unused-visibility warnings; the functions are already used inside the crate).

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-core/src/ingest.rs
git commit -m "refactor: expose frontmatter array helpers within knowledge-core"
```

---

### Task 3: Core `save_wiki_page` (`wiki_pages.rs`)

**Files:**
- Create: `crates/knowledge-core/src/project/wiki_pages.rs`
- Modify: `crates/knowledge-core/src/project/mod.rs` (add `pub mod wiki_pages;`)

- [ ] **Step 1: Create the module with save tests only (failing)**

Create `crates/knowledge-core/src/project/wiki_pages.rs` with ONLY this test module, and add `pub mod wiki_pages;` to `crates/knowledge-core/src/project/mod.rs`:

```rust
#[cfg(test)]
mod tests {
  use super::*;
  use crate::project::root::ProjectRoot;
  use std::fs;

  fn project_root() -> (tempfile::TempDir, ProjectRoot) {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join("wiki/concepts")).unwrap();
    let root = ProjectRoot::new(temp.path()).unwrap();
    (temp, root)
  }

  #[test]
  fn save_creates_then_updates_wiki_page() {
    let (_temp, root) = project_root();

    let created = save_wiki_page(&root, "wiki/concepts/kv-cache.md", "# KV Cache\n").unwrap();
    assert_eq!(created.path, "wiki/concepts/kv-cache.md");
    assert!(created.created);

    let updated = save_wiki_page(&root, "wiki/concepts/kv-cache.md", "# KV Cache v2\n").unwrap();
    assert!(!updated.created);
    let content = fs::read_to_string(root.safe_join("wiki/concepts/kv-cache.md").unwrap()).unwrap();
    assert_eq!(content, "# KV Cache v2\n");
  }

  #[test]
  fn save_creates_missing_parent_directories() {
    let (_temp, root) = project_root();
    save_wiki_page(&root, "wiki/entities/new-dir/page.md", "body").unwrap();
    assert!(root.safe_join("wiki/entities/new-dir/page.md").unwrap().exists());
  }

  #[test]
  fn save_rejects_non_wiki_paths() {
    let (_temp, root) = project_root();
    assert!(matches!(
      save_wiki_page(&root, "raw/sources/notes.md", "x"),
      Err(WikiPageError::NotWikiPage)
    ));
    assert!(matches!(
      save_wiki_page(&root, "wiki/page.txt", "x"),
      Err(WikiPageError::NotWikiPage)
    ));
    assert!(matches!(
      save_wiki_page(&root, "wiki/../escape.md", "x"),
      Err(WikiPageError::NotWikiPage)
    ));
    assert!(matches!(
      save_wiki_page(&root, "wiki/.hidden.md", "x"),
      Err(WikiPageError::NotWikiPage)
    ));
  }
}
```

- [ ] **Step 2: Verify compilation fails**

Run: `cargo test -p knowledge-core wiki_pages`
Expected: FAIL — `save_wiki_page` / `WikiPageError` not found.

- [ ] **Step 3: Implement validation, error type, and save**

Prepend to `crates/knowledge-core/src/project/wiki_pages.rs`:

```rust
//! Wiki page write operations: save (create/update) and cascade
//! delete with reference cleanup. Delete semantics ported from
//! upstream_llm_wiki/src/lib/wiki-page-delete.ts.

use std::fs;
use std::path::{Path, PathBuf};

use crate::ingest::{parse_frontmatter_array, write_frontmatter_array};
use crate::project::files::{MAX_FILE_CONTENT_BYTES, is_public_project_rel};
use crate::project::root::{ProjectRoot, ProjectRootError};
use crate::project::wiki_cleanup::{
  DeletedPageInfo, build_deleted_keys, clean_index_listing, extract_frontmatter_title,
  normalize_wiki_ref_key, strip_deleted_wikilinks,
};

#[derive(Debug, thiserror::Error)]
pub enum WikiPageError {
  #[error("path must be a markdown file under wiki/")]
  NotWikiPage,
  #[error("file is too large to save via API")]
  FileTooLarge,
  #[error("io error: {0}")]
  Io(#[from] std::io::Error),
  #[error("{0}")]
  Root(#[from] ProjectRootError),
}

#[derive(Debug, Clone)]
pub struct SavedWikiPage {
  pub path: String,
  pub created: bool,
}

pub fn is_wiki_page_rel(relative_path: &str) -> bool {
  let normalized = normalize_rel(relative_path);
  let lower = normalized.to_lowercase();
  lower.starts_with("wiki/") && lower.ends_with(".md") && is_public_project_rel(&normalized)
}

fn normalize_rel(value: &str) -> String {
  value
    .replace('\\', "/")
    .trim_start_matches('/')
    .trim_end_matches('/')
    .to_string()
}

pub fn save_wiki_page(
  root: &ProjectRoot,
  relative_path: &str,
  content: &str,
) -> Result<SavedWikiPage, WikiPageError> {
  let normalized = normalize_rel(relative_path);
  if !is_wiki_page_rel(&normalized) {
    return Err(WikiPageError::NotWikiPage);
  }
  if content.len() as u64 > MAX_FILE_CONTENT_BYTES {
    return Err(WikiPageError::FileTooLarge);
  }

  let path = root.safe_join(&normalized)?;
  let created = !path.exists();
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }
  fs::write(&path, content)?;

  Ok(SavedWikiPage {
    path: normalized,
    created,
  })
}
```

Note: `is_public_project_rel` already rejects any path segment that is empty or starts with `.`, which covers both traversal (`..`) and hidden files.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p knowledge-core wiki_pages`
Expected: PASS (3 tests).

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p knowledge-core --all-targets -- -D warnings` — expected clean, then:

```bash
git add crates/knowledge-core/src/project/wiki_pages.rs crates/knowledge-core/src/project/mod.rs
git commit -m "feat: add wiki page save to knowledge-core"
```

---

### Task 4: Core `delete_wiki_pages_with_refs`

**Files:**
- Modify: `crates/knowledge-core/src/project/wiki_pages.rs`

- [ ] **Step 1: Add the failing cascade-delete test**

Append inside the existing `mod tests` in `wiki_pages.rs`:

```rust
  #[test]
  fn cascade_delete_cleans_index_body_links_related_and_media() {
    let (_temp, root) = project_root();
    fs::create_dir_all(root.safe_join("wiki/sources").unwrap()).unwrap();
    fs::create_dir_all(root.safe_join("wiki/media/paper").unwrap()).unwrap();
    fs::write(root.safe_join("wiki/media/paper/fig1.png").unwrap(), [0u8]).unwrap();

    fs::write(
      root.safe_join("wiki/sources/paper.md").unwrap(),
      "---\ntype: source\ntitle: The Paper\n---\n\n# The Paper\n",
    )
    .unwrap();
    fs::write(
      root.safe_join("wiki/concepts/kv-cache.md").unwrap(),
      "---\ntype: concept\ntitle: KV Cache\n---\n\n# KV Cache\n",
    )
    .unwrap();
    fs::write(
      root.safe_join("wiki/concepts/attention.md").unwrap(),
      "---\ntype: concept\ntitle: Attention\nrelated: [\"kv-cache\", \"transformer\"]\n---\n\nUses [[KV Cache]] and [[Transformer]].\n",
    )
    .unwrap();
    fs::write(
      root.safe_join("wiki/index.md").unwrap(),
      "# Index\n\n- [[KV Cache]] cached states\n- [[The Paper]] source\n- [[Attention]] focus\n",
    )
    .unwrap();

    let result = delete_wiki_pages_with_refs(
      &root,
      &[
        "wiki/concepts/kv-cache.md".to_string(),
        "wiki/sources/paper.md".to_string(),
      ],
    )
    .unwrap();

    assert_eq!(result.deleted_paths.len(), 2);
    assert!(result.deleted_page_ids.contains(&"kv-cache".to_string()));
    assert!(result.deleted_page_ids.contains(&"paper".to_string()));
    assert_eq!(result.rewritten_files, 2);

    assert!(!root.safe_join("wiki/concepts/kv-cache.md").unwrap().exists());
    assert!(!root.safe_join("wiki/media/paper").unwrap().exists());

    let index = fs::read_to_string(root.safe_join("wiki/index.md").unwrap()).unwrap();
    assert!(!index.contains("KV Cache"));
    assert!(!index.contains("The Paper"));
    assert!(index.contains("- [[Attention]] focus"));

    let attention = fs::read_to_string(root.safe_join("wiki/concepts/attention.md").unwrap()).unwrap();
    assert!(attention.contains("Uses KV Cache and [[Transformer]]."));
    assert!(attention.contains("related: [\"transformer\"]"));
  }

  #[test]
  fn cascade_delete_skips_missing_files_without_error() {
    let (_temp, root) = project_root();
    let result =
      delete_wiki_pages_with_refs(&root, &["wiki/concepts/ghost.md".to_string()]).unwrap();
    assert!(result.deleted_paths.is_empty());
    assert_eq!(result.rewritten_files, 0);
  }
```

- [ ] **Step 2: Verify compilation fails**

Run: `cargo test -p knowledge-core wiki_pages`
Expected: FAIL — `delete_wiki_pages_with_refs` not found.

- [ ] **Step 3: Implement cascade delete**

Add to `wiki_pages.rs` (below `save_wiki_page`, above the tests):

```rust
#[derive(Debug, Clone)]
pub struct WikiPagesDeleteResult {
  pub deleted_paths: Vec<String>,
  pub deleted_page_ids: Vec<String>,
  pub rewritten_files: usize,
}

pub fn delete_wiki_pages_with_refs(
  root: &ProjectRoot,
  relative_paths: &[String],
) -> Result<WikiPagesDeleteResult, WikiPageError> {
  let mut result = WikiPagesDeleteResult {
    deleted_paths: Vec::new(),
    deleted_page_ids: Vec::new(),
    rewritten_files: 0,
  };

  // 1. Validate the whole batch and snapshot slug + title before
  //    deleting anything (titles are unreadable after the delete).
  let mut targets = Vec::new();
  let mut infos = Vec::new();
  for relative_path in relative_paths {
    let normalized = normalize_rel(relative_path);
    if !is_wiki_page_rel(&normalized) {
      return Err(WikiPageError::NotWikiPage);
    }
    let absolute = root.safe_join(&normalized)?;
    let slug = file_stem(&normalized);
    let title = fs::read_to_string(&absolute)
      .map(|content| extract_frontmatter_title(&content))
      .unwrap_or_default();
    if !slug.is_empty() {
      infos.push(DeletedPageInfo {
        slug: slug.clone(),
        title,
      });
    }
    targets.push((normalized, absolute, slug));
  }

  // 2. Delete each page; cascade the media directory for source pages.
  for (normalized, absolute, slug) in &targets {
    match fs::remove_file(absolute) {
      Ok(()) => {}
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
      Err(error) => return Err(WikiPageError::Io(error)),
    }
    result.deleted_paths.push(normalized.clone());
    if !slug.is_empty() {
      result.deleted_page_ids.push(slug.clone());
    }

    // Source pages own wiki/media/<slug>/. Guard the slug so a
    // degenerate name can never resolve to the media root itself.
    if normalized.to_lowercase().starts_with("wiki/sources/")
      && !slug.is_empty()
      && !slug.starts_with('.')
    {
      if let Ok(media_dir) = root.safe_join(&format!("wiki/media/{slug}")) {
        let _ = fs::remove_dir_all(media_dir);
      }
    }
  }

  if result.deleted_paths.is_empty() {
    return Ok(result);
  }

  // 3. Best-effort sweep of surviving wiki markdown files.
  let deleted_keys = build_deleted_keys(&infos);
  let wiki_root = root.safe_join("wiki")?;
  let mut markdown_files = Vec::new();
  collect_markdown_files(&wiki_root, &mut markdown_files);

  for path in markdown_files {
    let Ok(content) = fs::read_to_string(&path) else {
      continue;
    };

    let mut updated = content.clone();
    if path.file_name().and_then(|value| value.to_str()) == Some("index.md") {
      updated = clean_index_listing(&updated, &deleted_keys);
    }
    updated = strip_deleted_wikilinks(&updated, &deleted_keys);

    let related = parse_frontmatter_array(&updated, "related");
    if !related.is_empty() {
      let filtered = related
        .iter()
        .filter(|value| !deleted_keys.contains(&normalize_wiki_ref_key(value)))
        .cloned()
        .collect::<Vec<_>>();
      if filtered.len() != related.len() {
        updated = write_frontmatter_array(&updated, "related", &filtered);
      }
    }

    if updated != content && fs::write(&path, updated).is_ok() {
      result.rewritten_files += 1;
    }
  }

  Ok(result)
}

fn file_stem(relative_path: &str) -> String {
  Path::new(relative_path)
    .file_stem()
    .and_then(|value| value.to_str())
    .unwrap_or_default()
    .to_string()
}

fn collect_markdown_files(dir: &Path, output: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(dir) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let Ok(file_type) = entry.file_type() else {
      continue;
    };
    if file_type.is_dir() {
      collect_markdown_files(&path, output);
      continue;
    }
    if path.extension().and_then(|value| value.to_str()) == Some("md") {
      output.push(path);
    }
  }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p knowledge-core wiki_pages`
Expected: PASS (5 tests). If the `rewritten_files == 2` assertion fails, check that the deleted targets were removed BEFORE the sweep collected files (deleted files must not be swept).

- [ ] **Step 5: Clippy and commit**

Run: `cargo clippy -p knowledge-core --all-targets -- -D warnings` — expected clean, then:

```bash
git add crates/knowledge-core/src/project/wiki_pages.rs
git commit -m "feat: add wiki page cascade delete with reference cleanup"
```

---

### Task 5: Failing integration test for the two endpoints

**Files:**
- Modify: `tests/rust-integration/tests/file_api.rs`

This test drives Tasks 6–7. It is written first and stays red until both endpoints exist.

- [ ] **Step 1: Add the test and a local save helper**

Append to `tests/rust-integration/tests/file_api.rs` (4-space indent; reuses the file's existing `login_and_csrf`, `create_project`, `read_json` helpers):

```rust
#[tokio::test]
async fn wiki_page_save_and_cascade_delete_clean_references() {
    let temp = tempdir().unwrap();
    let _env = TestEnvironment::start("wiki-page-edit").await.unwrap();
    let config = AppConfig::for_tests(_env.database_url.clone(), _env.redis_url.clone());
    let state = bootstrap_state(&config).await.unwrap();
    let (cookie, csrf) = login_and_csrf(state.clone()).await;
    let project_root = temp.path().join("wiki-edit-project");
    let project_id = create_project(state.clone(), &cookie, &csrf, project_root.clone()).await;

    // Missing CSRF header is rejected.
    let missing_csrf = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/projects/{project_id}/files/content"))
                .header(header::COOKIE, &cookie)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "path": "wiki/concepts/kv-cache.md", "content": "x" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    // Non-wiki paths are rejected.
    assert_eq!(
        save_page(state.clone(), &cookie, &csrf, &project_id, "raw/sources/notes.md", "x").await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        save_page(state.clone(), &cookie, &csrf, &project_id, "../escape.md", "x").await,
        StatusCode::BAD_REQUEST
    );

    // Create two concept pages and an index referencing both forms.
    assert_eq!(
        save_page(
            state.clone(),
            &cookie,
            &csrf,
            &project_id,
            "wiki/concepts/kv-cache.md",
            "---\ntype: concept\ntitle: KV Cache\n---\n\n# KV Cache\n",
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        save_page(
            state.clone(),
            &cookie,
            &csrf,
            &project_id,
            "wiki/concepts/attention.md",
            "---\ntype: concept\ntitle: Attention\nrelated: [\"kv-cache\", \"transformer\"]\n---\n\nUses [[KV Cache]] and [[Transformer]].\n",
        )
        .await,
        StatusCode::OK
    );
    assert_eq!(
        save_page(
            state.clone(),
            &cookie,
            &csrf,
            &project_id,
            "wiki/index.md",
            "# Index\n\n- [[KV Cache]] cached states\n- [[Attention]] focus\n",
        )
        .await,
        StatusCode::OK
    );

    // Cascade delete kv-cache.
    let delete_response = build_app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/projects/{project_id}/wiki-pages:delete"))
                .header(header::COOKIE, &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({ "paths": ["wiki/concepts/kv-cache.md"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_payload = read_json(delete_response.into_body()).await;
    assert_eq!(
        delete_payload
            .get("deletedPaths")
            .and_then(Value::as_array)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        delete_payload.get("rewrittenFiles").and_then(Value::as_u64),
        Some(2)
    );

    // Page is gone; index entry dropped (title-form match, Bug A);
    // sibling entry survives; body wikilink became plain text while
    // [[Transformer]] is untouched (Bug B); related: filtered.
    assert!(!project_root.join("wiki/concepts/kv-cache.md").exists());
    let index = fs::read_to_string(project_root.join("wiki/index.md")).unwrap();
    assert!(!index.contains("KV Cache"));
    assert!(index.contains("- [[Attention]] focus"));
    let attention = fs::read_to_string(project_root.join("wiki/concepts/attention.md")).unwrap();
    assert!(attention.contains("Uses KV Cache and [[Transformer]]."));
    assert!(attention.contains("related: [\"transformer\"]"));
}

async fn save_page(
    state: knowledge_server::app::state::AppState,
    cookie: &str,
    csrf: &str,
    project_id: &str,
    path: &str,
    content: &str,
) -> StatusCode {
    build_app(state)
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/projects/{project_id}/files/content"))
                .header(header::COOKIE, cookie)
                .header("x-csrf-token", csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json!({ "path": path, "content": content }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
}
```

- [ ] **Step 2: Run to verify it fails for the right reason**

Run: `cargo test -p rust-integration --test file_api wiki_page_save_and_cascade_delete_clean_references`
Expected: FAIL — the PUT returns `405 Method Not Allowed` (route not registered yet), so the first `save_page` assertion fails. Requires local Postgres/Redis from the docker stack (`npm run docker:up` ports 55432/56379).

- [ ] **Step 3: Commit the red test**

```bash
git add tests/rust-integration/tests/file_api.rs
git commit -m "test: add failing integration test for wiki page save and cascade delete"
```

---

### Task 6: Server endpoint — save (`PUT /files/content`)

**Files:**
- Modify: `crates/knowledge-server/src/projects/routes.rs`

- [ ] **Step 1: Imports and route registration**

In `crates/knowledge-server/src/projects/routes.rs` (4-space indent):

Line 4, add `put`:

```rust
use axum::routing::{delete, get, patch, post, put};
```

After the `knowledge_core::project::files` import block (line 26-29), add (only these two names — Task 7 extends this import):

```rust
use knowledge_core::project::wiki_pages::{WikiPageError, save_wiki_page};
```

In `router()`, change the files/content route (lines 70-73):

```rust
        .route(
            "/api/projects/{project_id}/files/content",
            get(file_content_handler).put(save_file_content_handler),
        )
```

(The `wiki-pages:delete` route and its handler are added in Task 7, keeping this task compiling on its own.)

- [ ] **Step 2: Add request struct, error mapper, and save handler**

Near the other request structs (search for `struct FileContentRequest`), add:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveFileContentRequest {
    path: String,
    content: String,
}
```

Below `map_project_files_error` (line ~1160), add:

```rust
fn map_wiki_page_error(error: WikiPageError) -> ApiError {
    match error {
        WikiPageError::NotWikiPage => ApiError::bad_request(error.to_string()),
        WikiPageError::FileTooLarge => ApiError::payload_too_large(error.to_string()),
        WikiPageError::Io(_) | WikiPageError::Root(_) => ApiError::bad_request(error.to_string()),
    }
}
```

Below `file_content_handler` (line ~578), add:

```rust
async fn save_file_content_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(payload): Json<SaveFileContentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_session(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session.csrf_token)?;
    let root = project_root_for_id(&state, &project_id).await?;
    let saved =
        save_wiki_page(&root, &payload.path, &payload.content).map_err(map_wiki_page_error)?;
    append_audit_log(
        &state,
        CreateAuditLog {
            project_id: Some(project_id),
            actor_id: session.user_id,
            action: "project.wiki_page_saved".to_string(),
            target_type: "wiki_page".to_string(),
            target_id: saved.path.clone(),
            task_id: None,
            summary: format!(
                "{} wiki page {}",
                if saved.created { "Created" } else { "Updated" },
                saved.path
            ),
            metadata: json!({ "path": saved.path, "created": saved.created }),
        },
    )
    .await?;
    Ok(Json(json!({ "path": saved.path, "created": saved.created })))
}
```

- [ ] **Step 3: Verify save assertions now pass (delete still red)**

Run: `cargo test -p rust-integration --test file_api wiki_page_save_and_cascade_delete_clean_references`
Expected: FAIL later in the test — all `save_page` assertions pass; the `wiki-pages:delete` POST returns 404/405.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/projects/routes.rs
git commit -m "feat: add wiki page save endpoint"
```

---

### Task 7: Server endpoint — cascade delete (`POST /wiki-pages:delete`)

**Files:**
- Modify: `crates/knowledge-server/src/retrieval/store.rs`
- Modify: `crates/knowledge-server/src/projects/routes.rs`

- [ ] **Step 1: Add `delete_pages` to the retrieval store**

In `crates/knowledge-server/src/retrieval/store.rs`, below `delete_missing_pages` (line ~163):

```rust
pub async fn delete_pages(
  pool: &sqlx::PgPool,
  project_id: &str,
  page_ids: &[String],
) -> Result<(), ApiError> {
  for page_id in page_ids {
    sqlx::query(
      "DELETE FROM project_embedding_chunks
       WHERE project_id = $1 AND page_id = $2",
    )
    .bind(project_id)
    .bind(page_id)
    .execute(pool)
    .await
    .map_err(ApiError::from)?;
  }

  Ok(())
}
```

(Note: store.rs uses 2-space indent — match the file.)

- [ ] **Step 2: Register route, request struct, and handler**

In `routes.rs`, extend the wiki_pages import (from Task 6) to:

```rust
use knowledge_core::project::wiki_pages::{
    WikiPageError, delete_wiki_pages_with_refs, save_wiki_page,
};
```

Add after `use crate::retrieval::service::search_project_hybrid;` (line 24):

```rust
use crate::retrieval::store::delete_pages;
```

Register in `router()` right after the files/content route:

```rust
        .route(
            "/api/projects/{project_id}/wiki-pages:delete",
            post(delete_wiki_pages_handler),
        )
```

Add the request struct near `SaveFileContentRequest`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteWikiPagesRequest {
    paths: Vec<String>,
}
```

Add the handler below `save_file_content_handler`:

```rust
const MAX_WIKI_DELETE_BATCH: usize = 100;

async fn delete_wiki_pages_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
    Json(payload): Json<DeleteWikiPagesRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = authorized_session(&state, &headers, Some(&project_id)).await?;
    validate_csrf(&headers, &session.csrf_token)?;
    if payload.paths.is_empty() {
        return Err(ApiError::bad_request("paths must not be empty"));
    }
    if payload.paths.len() > MAX_WIKI_DELETE_BATCH {
        return Err(ApiError::bad_request("too many paths in one delete request"));
    }
    let root = project_root_for_id(&state, &project_id).await?;
    let result = delete_wiki_pages_with_refs(&root, &payload.paths).map_err(map_wiki_page_error)?;
    delete_pages(&state.pool, &project_id, &result.deleted_page_ids).await?;
    append_audit_log(
        &state,
        CreateAuditLog {
            project_id: Some(project_id),
            actor_id: session.user_id,
            action: "project.wiki_pages_deleted".to_string(),
            target_type: "wiki_page".to_string(),
            target_id: "batch".to_string(),
            task_id: None,
            summary: format!(
                "Deleted {} wiki pages, rewrote {} files",
                result.deleted_paths.len(),
                result.rewritten_files
            ),
            metadata: json!({
              "deletedPaths": result.deleted_paths,
              "rewrittenFiles": result.rewritten_files
            }),
        },
    )
    .await?;
    Ok(Json(json!({
      "deletedPaths": result.deleted_paths,
      "rewrittenFiles": result.rewritten_files
    })))
}
```

- [ ] **Step 3: Run the integration test to verify it passes end-to-end**

Run: `cargo test -p rust-integration --test file_api wiki_page_save_and_cascade_delete_clean_references`
Expected: PASS.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/retrieval/store.rs crates/knowledge-server/src/projects/routes.rs
git commit -m "feat: add wiki page cascade delete endpoint"
```

---

### Task 8: Admin API functions

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts`

- [ ] **Step 1: Add schemas and functions**

In `apps/admin/src/features/shared/api.ts`, add the schemas near the other file schemas (search for `projectFileContentSchema`) and the functions right after `getProjectFileContent` (line ~388):

```ts
const saveFileContentSchema = z.object({
  path: z.string(),
  created: z.boolean(),
});

const deleteWikiPagesSchema = z.object({
  deletedPaths: z.array(z.string()),
  rewrittenFiles: z.number(),
});

export async function saveProjectFileContent(input: {
  projectId: string;
  path: string;
  content: string;
}) {
  return apiFetch(
    `/api/projects/${input.projectId}/files/content`,
    {
      method: "PUT",
      headers: csrfHeader(),
      body: JSON.stringify({ path: input.path, content: input.content }),
    },
    saveFileContentSchema,
  );
}

export async function deleteProjectWikiPages(input: { projectId: string; paths: string[] }) {
  return apiFetch(
    `/api/projects/${input.projectId}/wiki-pages:delete`,
    {
      method: "POST",
      headers: csrfHeader(),
      body: JSON.stringify({ paths: input.paths }),
    },
    deleteWikiPagesSchema,
  );
}
```

- [ ] **Step 2: Verify the workspace still type-checks and lints**

Run: `npm run lint`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add apps/admin/src/features/shared/api.ts
git commit -m "feat: add wiki page save and delete api functions"
```

---

### Task 9: Admin mutations (`files/queries.ts`)

**Files:**
- Modify: `apps/admin/src/features/files/queries.ts`

- [ ] **Step 1: Replace the file content with**

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  deleteProjectWikiPages,
  getProjectFileContent,
  listProjectFiles,
  saveProjectFileContent,
} from "../shared/api";

export function useProjectFilesQuery(
  projectId: string,
  input: { root: string; recursive: boolean; maxFiles: number },
) {
  return useQuery({
    queryKey: ["project-files", projectId, input.root, input.recursive, input.maxFiles],
    queryFn: () => listProjectFiles({ projectId, ...input }),
  });
}

export function useProjectFileContentQuery(projectId: string, path: string) {
  return useQuery({
    queryKey: ["project-file-content", projectId, path],
    queryFn: () => getProjectFileContent({ projectId, path }),
    enabled: Boolean(projectId) && Boolean(path),
  });
}

export function useSaveFileContentMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { path: string; content: string }) =>
      saveProjectFileContent({ projectId, ...input }),
    onSuccess: (_data, input) => {
      void queryClient.invalidateQueries({ queryKey: ["project-files", projectId] });
      void queryClient.invalidateQueries({
        queryKey: ["project-file-content", projectId, input.path],
      });
    },
  });
}

export function useDeleteWikiPagesMutation(projectId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: { paths: string[] }) => deleteProjectWikiPages({ projectId, ...input }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["project-files", projectId] });
      void queryClient.invalidateQueries({ queryKey: ["project-file-content", projectId] });
    },
  });
}
```

- [ ] **Step 2: Lint and commit**

Run: `npm run lint` — expected clean, then:

```bash
git add apps/admin/src/features/files/queries.ts
git commit -m "feat: add wiki page save and delete mutations"
```

---

### Task 10: `WikiPageEditor` component (TDD)

**Files:**
- Create: `apps/admin/src/features/files/wiki-page-editor.test.tsx`
- Create: `apps/admin/src/features/files/wiki-page-editor.tsx`

- [ ] **Step 1: Write the failing component test**

Create `apps/admin/src/features/files/wiki-page-editor.test.tsx`:

```tsx
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  saveMutateAsync: vi.fn(),
  deleteMutateAsync: vi.fn(),
}));

vi.mock("./queries", () => ({
  useSaveFileContentMutation: () => ({ mutateAsync: mocks.saveMutateAsync, isPending: false }),
  useDeleteWikiPagesMutation: () => ({ mutateAsync: mocks.deleteMutateAsync, isPending: false }),
}));

import { WikiPageEditor, isEditableWikiPath } from "./wiki-page-editor";

describe("WikiPageEditor", () => {
  beforeEach(() => {
    mocks.saveMutateAsync.mockReset().mockResolvedValue({ path: "wiki/a.md", created: false });
    mocks.deleteMutateAsync
      .mockReset()
      .mockResolvedValue({ deletedPaths: ["wiki/a.md"], rewrittenFiles: 2 });
  });

  it("only treats wiki markdown paths as editable", () => {
    expect(isEditableWikiPath("wiki/concepts/a.md")).toBe(true);
    expect(isEditableWikiPath("raw/sources/a.md")).toBe(false);
    expect(isEditableWikiPath("wiki/media/a.png")).toBe(false);
  });

  it("renders nothing for non-wiki paths", () => {
    const { container } = render(
      <WikiPageEditor content="x" onDeleted={() => {}} path="raw/sources/a.md" projectId="p1" />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("saves the edited draft", async () => {
    render(
      <WikiPageEditor content="original" onDeleted={() => {}} path="wiki/a.md" projectId="p1" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit" }));
    fireEvent.change(screen.getByLabelText("Page content"), { target: { value: "updated" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(mocks.saveMutateAsync).toHaveBeenCalledWith({ path: "wiki/a.md", content: "updated" }),
    );
  });

  it("deletes after confirmation and reports the cascade result", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const onDeleted = vi.fn();
    render(
      <WikiPageEditor content="x" onDeleted={onDeleted} path="wiki/a.md" projectId="p1" />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() =>
      expect(mocks.deleteMutateAsync).toHaveBeenCalledWith({ paths: ["wiki/a.md"] }),
    );
    expect(onDeleted).toHaveBeenCalled();
    expect(screen.getByText(/rewrote 2 file/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm run test --workspace @knowledge/admin -- src/features/files/wiki-page-editor.test.tsx`
Expected: FAIL — module `./wiki-page-editor` not found.

- [ ] **Step 3: Implement the component**

Create `apps/admin/src/features/files/wiki-page-editor.tsx`:

```tsx
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { normalizeAppError } from "@/lib/app-error";

import { useDeleteWikiPagesMutation, useSaveFileContentMutation } from "./queries";

export function isEditableWikiPath(path: string) {
  return path.startsWith("wiki/") && path.endsWith(".md");
}

export function WikiPageEditor({
  projectId,
  path,
  content,
  onDeleted,
}: {
  projectId: string;
  path: string;
  content: string;
  onDeleted: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(content);
  const [feedback, setFeedback] = useState("");
  const save = useSaveFileContentMutation(projectId);
  const remove = useDeleteWikiPagesMutation(projectId);

  useEffect(() => {
    setEditing(false);
    setDraft(content);
    setFeedback("");
  }, [path, content]);

  if (!isEditableWikiPath(path)) {
    return null;
  }

  async function handleSave() {
    try {
      await save.mutateAsync({ path, content: draft });
      setEditing(false);
      setFeedback("Saved.");
    } catch (error) {
      setFeedback(normalizeAppError(error).message);
    }
  }

  async function handleDelete() {
    if (!window.confirm(`Delete ${path} and clean up references to it?`)) {
      return;
    }
    try {
      const result = await remove.mutateAsync({ paths: [path] });
      setFeedback(
        `Deleted ${result.deletedPaths.length} page(s), rewrote ${result.rewrittenFiles} file(s).`,
      );
      onDeleted();
    } catch (error) {
      setFeedback(normalizeAppError(error).message);
    }
  }

  return (
    <div className="grid gap-3">
      <div className="flex flex-wrap gap-2">
        {editing ? (
          <>
            <Button disabled={save.isPending} onClick={handleSave}>
              Save
            </Button>
            <Button
              onClick={() => {
                setEditing(false);
                setDraft(content);
              }}
              variant="ghost"
            >
              Cancel
            </Button>
          </>
        ) : (
          <Button onClick={() => setEditing(true)} variant="secondary">
            Edit
          </Button>
        )}
        <Button disabled={remove.isPending} onClick={handleDelete} variant="destructive">
          Delete
        </Button>
      </div>
      {editing ? (
        <Textarea
          aria-label="Page content"
          className="min-h-[320px] font-mono text-sm"
          onChange={(event) => setDraft(event.target.value)}
          value={draft}
        />
      ) : null}
      {feedback ? <p className="text-sm text-muted-foreground">{feedback}</p> : null}
    </div>
  );
}
```

(`Button` already supports `variant="destructive"` — see `apps/admin/src/components/ui/button.tsx:15`.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm run test --workspace @knowledge/admin -- src/features/files/wiki-page-editor.test.tsx`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/files/wiki-page-editor.tsx apps/admin/src/features/files/wiki-page-editor.test.tsx
git commit -m "feat: add wiki page editor component"
```

---

### Task 11: Mount the editor in the Files page + verify in browser

**Files:**
- Modify: `apps/admin/src/features/files/page.tsx`

- [ ] **Step 1: Integrate the editor**

In `apps/admin/src/features/files/page.tsx`:

Add the import below the `queries` import (line 14):

```tsx
import { WikiPageEditor } from "./wiki-page-editor";
```

Add a deletion callback inside `FilesPage` next to `handleSelectPath` (line ~75):

```tsx
  function handleDeleted() {
    setSelectedPath("");
    const nextParams = new URLSearchParams(searchParams);
    nextParams.delete("path");
    setSearchParams(nextParams);
  }
```

Replace the preview branch (lines 172-176) — the final `:` arm of the nested ternary —

```tsx
            ) : (
              <ScrollArea className="max-h-[520px] rounded-xl border border-border/70 bg-muted/30 p-4">
                <pre className="whitespace-pre-wrap break-words font-mono text-sm">{content.data?.content}</pre>
              </ScrollArea>
            )}
```

with:

```tsx
            ) : (
              <div className="grid gap-4">
                <WikiPageEditor
                  content={content.data?.content ?? ""}
                  onDeleted={handleDeleted}
                  path={selectedPath}
                  projectId={projectId}
                />
                <ScrollArea className="max-h-[520px] rounded-xl border border-border/70 bg-muted/30 p-4">
                  <pre className="whitespace-pre-wrap break-words font-mono text-sm">{content.data?.content}</pre>
                </ScrollArea>
              </div>
            )}
```

- [ ] **Step 2: Run admin unit tests and lint**

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS.

Run: `npm run lint`
Expected: clean.

- [ ] **Step 3: Verify in the browser**

With Postgres/Redis up (`npm run docker:up` if not already), start the backend and admin dev server:

Run: `cargo run -p knowledge-server` (background) and `npm run dev` (background), open `http://localhost:5173` (Vite default; use the port Vite prints), log in as `admin` / `secret-password`.

Manual checks on a project's Files page:
1. Select a `wiki/**/*.md` file → Edit/Delete buttons appear; select a `raw/sources/` file → they do not.
2. Edit → change text → Save → "Saved." appears; re-select the file → new content shown.
3. Delete → confirm dialog → feedback line reports deleted/rewritten counts; tree refreshes without the file; references in `wiki/index.md` are gone.
4. Cancel restores the original draft.

- [ ] **Step 4: Commit**

```bash
git add apps/admin/src/features/files/page.tsx
git commit -m "feat: wire wiki page editor into admin files page"
```

---

### Task 12: Full-suite gate

**Files:** none (verification only)

- [ ] **Step 1: Run everything**

Run: `npm run lint`
Expected: clean (eslint + clippy strict).

Run: `cargo test --workspace`
Expected: PASS (requires docker stack Postgres/Redis for integration tests).

Run: `npm run test --workspace @knowledge/admin`
Expected: PASS.

- [ ] **Step 2: Fix anything red, then final commit if fixes were needed**

If a fix was required, commit it with an appropriate `fix:` message. No empty commits.
