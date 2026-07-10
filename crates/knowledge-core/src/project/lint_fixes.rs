//! Deterministic lint fix operations ported from
//! upstream_llm_wiki/src/lib/lint-fixes.ts.

use std::sync::OnceLock;

use regex::Regex;
use time::OffsetDateTime;

use crate::project::query_save::make_query_slug;
use crate::project::root::ProjectRoot;
use crate::project::wiki_pages::{WikiPageError, save_wiki_page};

// upstream_llm_wiki/src/lib/lint-fixes.ts L5-10
pub fn lint_link_target(target: &str) -> String {
  let mut value = target.replace('\\', "/");
  if value
    .get(.."wiki/".len())
    .is_some_and(|prefix| prefix.eq_ignore_ascii_case("wiki/"))
  {
    value.drain(.."wiki/".len());
  }
  if value.len() >= ".md".len()
    && value
      .get(value.len() - ".md".len()..)
      .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".md"))
  {
    value.truncate(value.len() - ".md".len());
  }
  value.trim().to_string()
}

// upstream_llm_wiki/src/lib/lint-fixes.ts L12-14
fn normalized_lint_link_target(target: &str) -> String {
  lint_link_target(target).to_lowercase()
}

fn wikilink_regex() -> &'static Regex {
  static WIKILINK: OnceLock<Regex> = OnceLock::new();
  WIKILINK.get_or_init(|| Regex::new(r"\[\[([^\]|]+?)(\|[^\]]+?)?\]\]").expect("valid regex"))
}

// upstream_llm_wiki/src/lib/lint-fixes.ts L16-20
fn has_wikilink_to_target(content: &str, target: &str) -> bool {
  let normalized = normalized_lint_link_target(target);
  wikilink_regex()
    .captures_iter(content)
    .any(|captures| {
      normalized_lint_link_target(captures.get(1).map_or("", |m| m.as_str())) == normalized
    })
}

// upstream_llm_wiki/src/lib/lint-fixes.ts L22-32
pub fn append_wikilink(content: &str, target: &str) -> String {
  static RELATED_HEADING: OnceLock<Regex> = OnceLock::new();

  let link_target = lint_link_target(target);
  if has_wikilink_to_target(content, &link_target) {
    return content.to_string();
  }
  let link_line = format!("- [[{link_target}]]");
  let related_heading = RELATED_HEADING
    .get_or_init(|| Regex::new(r"(?im)^##\s+Related\s*$").expect("valid regex"));
  if let Some(found) = related_heading.find(content) {
    let insert_at = found.end();
    return format!(
      "{}\n{link_line}{}",
      &content[..insert_at],
      &content[insert_at..]
    );
  }
  format!("{}\n\n## Related\n{link_line}\n", content.trim_end())
}

// upstream_llm_wiki/src/lib/lint-fixes.ts L34-48
pub fn rewrite_wikilink_target(
  content: &str,
  broken_target: &str,
  suggested_target: &str,
) -> String {
  let broken = normalized_lint_link_target(broken_target);
  let replacement = lint_link_target(suggested_target);
  wikilink_regex()
    .replace_all(content, |captures: &regex::Captures<'_>| {
      let raw_target = captures.get(1).map_or("", |m| m.as_str());
      if normalized_lint_link_target(raw_target) != broken {
        return captures.get(0).map_or("", |m| m.as_str()).to_string();
      }
      let raw_alias = captures.get(2).map_or("", |m| m.as_str());
      format!("[[{replacement}{raw_alias}]]")
    })
    .into_owned()
}

// upstream_llm_wiki/src/lib/lint-fixes.ts L50-60
pub fn stub_relative_path_from_broken_target(broken_target: &str) -> String {
  let normalized = lint_link_target(broken_target);
  let parts = normalized
    .split('/')
    .map(make_query_slug)
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>();
  let rel = if parts.len() > 1 {
    parts.join("/")
  } else {
    format!(
      "queries/{}",
      parts.first().map(String::as_str).unwrap_or("missing-page")
    )
  };
  format!("{rel}.md")
}

// upstream_llm_wiki/src/lib/lint-fixes.ts L62-66
fn stub_title_from_broken_target(broken_target: &str) -> String {
  static SEPARATORS: OnceLock<Regex> = OnceLock::new();

  let normalized = lint_link_target(broken_target);
  let name = normalized.rsplit('/').next().unwrap_or(&normalized);
  let title = SEPARATORS
    .get_or_init(|| Regex::new(r"[-_]+").expect("valid regex"))
    .replace_all(name, " ")
    .trim()
    .to_string();
  if title.is_empty() {
    "Missing Page".to_string()
  } else {
    title
  }
}

#[derive(Debug, Clone)]
pub struct BrokenLinkStub {
  /// Path relative to wiki/, e.g. "queries/missing-page.md".
  pub relative_path: String,
  pub created: bool,
}

// upstream_llm_wiki/src/lib/lint-fixes.ts L68-100
pub fn ensure_broken_link_stub(
  root: &ProjectRoot,
  broken_target: &str,
) -> Result<BrokenLinkStub, WikiPageError> {
  let relative_path = stub_relative_path_from_broken_target(broken_target);
  let wiki_rel = format!("wiki/{relative_path}");
  let path = root.safe_join(&wiki_rel)?;
  if path.exists() {
    return Ok(BrokenLinkStub {
      relative_path,
      created: false,
    });
  }

  let title = stub_title_from_broken_target(broken_target);
  let now = OffsetDateTime::now_utc();
  let date = format!("{:04}-{:02}-{:02}", now.year(), u8::from(now.month()), now.day());
  let lines = [
    "---".to_string(),
    "type: query".to_string(),
    format!("title: \"{}\"", title.replace('"', "\\\"")),
    format!("created: {date}"),
    format!("updated: {date}"),
    "tags: [stub, lint]".to_string(),
    "related: []".to_string(),
    "sources: []".to_string(),
    "---".to_string(),
    String::new(),
    format!("# {title}"),
    String::new(),
    "Created by Wiki Lint as a placeholder for a missing wikilink target.".to_string(),
    String::new(),
  ];
  save_wiki_page(root, &wiki_rel, &lines.join("\n"))?;
  Ok(BrokenLinkStub {
    relative_path,
    created: true,
  })
}

#[cfg(test)]
mod tests {
  use tempfile::tempdir;

  use crate::project::scaffold::initialize_project;

  use super::{
    append_wikilink, ensure_broken_link_stub, lint_link_target, rewrite_wikilink_target,
    stub_relative_path_from_broken_target,
  };

  #[test]
  fn lint_link_target_strips_prefix_and_extension() {
    assert_eq!(lint_link_target("wiki/entities/foo.md"), "entities/foo");
    assert_eq!(lint_link_target("Wiki/entities/Foo.MD"), "entities/Foo");
    assert_eq!(lint_link_target(" concepts/bar "), "concepts/bar");
  }

  #[test]
  fn append_wikilink_skips_existing_and_inserts_into_related_section() {
    let existing = "# Page\n\nSee [[entities/foo|Foo]].\n";
    assert_eq!(append_wikilink(existing, "wiki/entities/foo.md"), existing);

    let with_related = "# Page\n\n## Related\n- [[other]]\n";
    let updated = append_wikilink(with_related, "entities/foo");
    assert!(updated.contains("## Related\n- [[entities/foo]]\n- [[other]]"));

    let without_related = "# Page\n\nBody.\n";
    let appended = append_wikilink(without_related, "entities/foo");
    assert!(appended.ends_with("# Page\n\nBody.\n\n## Related\n- [[entities/foo]]\n"));
  }

  #[test]
  fn rewrite_wikilink_target_preserves_alias() {
    let content = "See [[Broken Page]] and [[broken page|Alias]] and [[other]].";
    let rewritten = rewrite_wikilink_target(content, "broken page", "entities/fixed");
    assert_eq!(
      rewritten,
      "See [[entities/fixed]] and [[entities/fixed|Alias]] and [[other]]."
    );
  }

  #[test]
  fn stub_path_slugs_each_segment() {
    assert_eq!(
      stub_relative_path_from_broken_target("Missing Page"),
      "queries/missing-page.md"
    );
    assert_eq!(
      stub_relative_path_from_broken_target("Entities/Some Topic"),
      "entities/some-topic.md"
    );
    assert_eq!(stub_relative_path_from_broken_target("注意力 机制"), "queries/注意力-机制.md");
  }

  #[test]
  fn ensure_broken_link_stub_writes_frontmatter_once() {
    let temp = tempdir().unwrap();
    let root = initialize_project(temp.path()).unwrap();

    let first = ensure_broken_link_stub(&root, "Missing Concept").unwrap();
    assert!(first.created);
    assert_eq!(first.relative_path, "queries/missing-concept.md");

    let content = std::fs::read_to_string(temp.path().join("wiki/queries/missing-concept.md")).unwrap();
    assert!(content.starts_with("---\ntype: query\ntitle: \"Missing Concept\"\n"));
    assert!(content.contains("tags: [stub, lint]"));
    assert!(content.contains("# Missing Concept"));

    let second = ensure_broken_link_stub(&root, "Missing Concept").unwrap();
    assert!(!second.created);
    assert_eq!(second.relative_path, first.relative_path);
  }
}
