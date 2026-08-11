//! "Save to Wiki" for chat assistant messages. Ported from
//! upstream_llm_wiki/src/lib/chat-save-to-wiki.ts (content cleaning + title),
//! upstream_llm_wiki/src/lib/wiki-filename.ts (Unicode-aware slug + filename),
//! and upstream_llm_wiki/src/components/chat/chat-message.tsx L559-603
//! (frontmatter, index.md insert, log.md append). Upstream runs three frontend
//! writes; here it is one transactional-ish server call guarded by a lock.

use std::fs;
use std::sync::{Mutex, OnceLock};

use regex::Regex;
use time::OffsetDateTime;
use unicode_normalization::UnicodeNormalization;

use crate::project::root::ProjectRoot;
use crate::project::wiki_pages::{WikiPageError, save_wiki_page};

#[derive(Debug, thiserror::Error)]
pub enum QuerySaveError {
  #[error("message has no visible content after cleaning")]
  EmptyContent,
  #[error(transparent)]
  Wiki(#[from] WikiPageError),
}

/// chat-save-to-wiki.ts L1-8: strip `<!--save-worthy|sources:...-->` markers
/// and `<think>`/`<thinking>` blocks (including an unclosed trailing one),
/// then trim leading whitespace once and trailing whitespace.
pub fn clean_assistant_content(content: &str) -> String {
  static MARKERS: OnceLock<Regex> = OnceLock::new();
  static THINK_CLOSED: OnceLock<Regex> = OnceLock::new();
  static THINK_OPEN_END: OnceLock<Regex> = OnceLock::new();

  let markers = MARKERS
    .get_or_init(|| Regex::new(r"<!--\s*(?:save-worthy|sources):[^\n]*?-->").unwrap());
  let think_closed = THINK_CLOSED
    .get_or_init(|| Regex::new(r"(?is)<think(?:ing)?>\s*.*?</think(?:ing)?>\s*").unwrap());
  let think_open_end =
    THINK_OPEN_END.get_or_init(|| Regex::new(r"(?is)<think(?:ing)?>\s*.*$").unwrap());

  let cleaned = markers.replace_all(content, "");
  let cleaned = think_closed.replace_all(&cleaned, "");
  let cleaned = think_open_end.replace_all(&cleaned, "");
  cleaned.trim_start().trim_end().to_string()
}

/// chat-save-to-wiki.ts L10-16: first visible line with `#` heading prefix
/// stripped, capped at 60 chars, defaulting to "Saved Query".
pub fn title_from_clean_content(clean: &str) -> String {
  static HEADING: OnceLock<Regex> = OnceLock::new();
  let heading = HEADING.get_or_init(|| Regex::new(r"^#+\s*").unwrap());

  clean
    .split('\n')
    .map(|line| heading.replace(line, "").trim().to_string())
    .find(|line| !line.is_empty())
    .map(|line| line.chars().take(60).collect::<String>())
    .unwrap_or_else(|| "Saved Query".to_string())
}

/// wiki-filename.ts L32-46: NFKC, whitespace to hyphens, keep Unicode
/// letters/digits/hyphens, collapse and trim hyphens, lowercase, 50 chars,
/// fallback "query".
pub fn make_query_slug(title: &str) -> String {
  static WHITESPACE: OnceLock<Regex> = OnceLock::new();
  static DISALLOWED: OnceLock<Regex> = OnceLock::new();
  static HYPHENS: OnceLock<Regex> = OnceLock::new();

  let whitespace = WHITESPACE.get_or_init(|| Regex::new(r"\s+").unwrap());
  let disallowed = DISALLOWED.get_or_init(|| Regex::new(r"[^\p{L}\p{N}-]").unwrap());
  let hyphens = HYPHENS.get_or_init(|| Regex::new(r"-+").unwrap());

  let normalized: String = title.nfkc().collect();
  let slug = whitespace.replace_all(normalized.trim(), "-");
  let slug = disallowed.replace_all(&slug, "");
  let slug = hyphens.replace_all(&slug, "-");
  let slug = slug
    .trim_start_matches('-')
    .trim_end_matches('-')
    .to_lowercase();
  let truncated: String = slug.chars().take(50).collect();
  if truncated.is_empty() {
    "query".to_string()
  } else {
    truncated
  }
}

#[derive(Debug, Clone)]
pub struct QueryFileName {
  pub slug: String,
  pub file_name: String,
  pub date: String,
  pub time: String,
}

/// wiki-filename.ts L50-66: `{slug}-{YYYY-MM-DD}-{HHMMSS}.md`, always UTC so
/// the same save yields the same name regardless of server timezone.
pub fn make_query_file_name(title: &str, now: OffsetDateTime) -> QueryFileName {
  let slug = make_query_slug(title);
  let date = format!(
    "{:04}-{:02}-{:02}",
    now.year(),
    u8::from(now.month()),
    now.day()
  );
  let time = format!("{:02}{:02}{:02}", now.hour(), now.minute(), now.second());
  QueryFileName {
    file_name: format!("{slug}-{date}-{time}.md"),
    slug,
    date,
    time,
  }
}

#[derive(Debug, Clone)]
pub struct SavedQueryPage {
  pub path: String,
  pub title: String,
  pub file_name: String,
}

/// chat-message.tsx L559-603: write the query page with frontmatter, insert a
/// wikilink under `## Queries` in index.md (creating file/section as needed),
/// and append to log.md.
pub fn save_query_page(
  root: &ProjectRoot,
  raw_content: &str,
) -> Result<SavedQueryPage, QuerySaveError> {
  save_query_page_at(root, raw_content, OffsetDateTime::now_utc())
}

pub fn save_query_page_at(
  root: &ProjectRoot,
  raw_content: &str,
  now: OffsetDateTime,
) -> Result<SavedQueryPage, QuerySaveError> {
  // index.md/log.md are read-modify-write; serialize concurrent saves within
  // this process (single-instance server — accepted limitation from the plan).
  static INDEX_LOG_LOCK: Mutex<()> = Mutex::new(());

  let clean = clean_assistant_content(raw_content);
  if clean.is_empty() {
    return Err(QuerySaveError::EmptyContent);
  }
  let title = title_from_clean_content(&clean);
  let named = make_query_file_name(&title, now);

  // chat-message.tsx L559-569: frontmatter block ends with a single newline
  // before the content, exactly as upstream's join("\n") produces.
  let frontmatter = format!(
    "---\ntype: query\ntitle: \"{}\"\ncreated: {}\ntags: []\n---\n",
    title.replace('"', "\\\""),
    named.date
  );
  let _guard = INDEX_LOG_LOCK
    .lock()
    .unwrap_or_else(|poisoned| poisoned.into_inner());

  // Same-second saves collide on the timestamped name; suffix instead of
  // silently overwriting the earlier page. Checked under the lock so
  // concurrent saves can't pick the same candidate.
  let file_name = dedupe_file_name(root, &named.file_name);
  let page_rel = format!("wiki/queries/{file_name}");
  let saved = save_wiki_page(root, &page_rel, &format!("{frontmatter}{clean}"))?;

  // chat-message.tsx L571-592: index.md insert under `## Queries`.
  let index_content = read_or(root, "wiki/index.md", "# Wiki Index\n\n## Queries\n");
  let link_target = file_name.trim_end_matches(".md");
  let entry = format!("- [[queries/{link_target}|{title}]]");
  let updated_index = if index_content.contains("## Queries") {
    match index_content.find("## Queries\n") {
      Some(position) => {
        let insert_at = position + "## Queries\n".len();
        format!(
          "{}{}\n{}",
          &index_content[..insert_at],
          entry,
          &index_content[insert_at..]
        )
      }
      // Upstream's regex replace of `## Queries\n` no-ops when the heading
      // sits at EOF without a newline; mirror that.
      None => index_content.clone(),
    }
  } else {
    format!("{}\n\n## Queries\n{entry}\n", index_content.trim_end())
  };
  save_wiki_page(root, "wiki/index.md", &updated_index)?;

  // chat-message.tsx L594-603: log.md append.
  let log_content = read_or(root, "wiki/log.md", "# Wiki Log\n\n");
  let log_entry = format!("- {}: Saved query page `{file_name}`\n", named.date);
  save_wiki_page(
    root,
    "wiki/log.md",
    &format!("{}\n{log_entry}", log_content.trim_end()),
  )?;

  Ok(SavedQueryPage {
    path: saved.path,
    title,
    file_name,
  })
}

fn dedupe_file_name(root: &ProjectRoot, file_name: &str) -> String {
  let exists = |name: &str| {
    root
      .safe_join(&format!("wiki/queries/{name}"))
      .is_ok_and(|path| path.exists())
  };
  if !exists(file_name) {
    return file_name.to_string();
  }
  let stem = file_name.trim_end_matches(".md");
  (2..)
    .map(|n| format!("{stem}-{n}.md"))
    .find(|candidate| !exists(candidate))
    .expect("unbounded suffix search")
}

fn read_or(root: &ProjectRoot, rel: &str, fallback: &str) -> String {
  root
    .safe_join(rel)
    .ok()
    .and_then(|path| fs::read_to_string(path).ok())
    .unwrap_or_else(|| fallback.to_string())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_now() -> OffsetDateTime {
    time::Date::from_calendar_date(2026, time::Month::April, 23)
      .unwrap()
      .with_hms(14, 30, 52)
      .unwrap()
      .assume_utc()
  }

  #[test]
  fn cleans_markers_and_think_blocks() {
    assert_eq!(
      clean_assistant_content("<!-- save-worthy: yes -->\nHello"),
      "Hello"
    );
    assert_eq!(
      clean_assistant_content("<think>internal</think>\nAnswer"),
      "Answer"
    );
    assert_eq!(
      clean_assistant_content("<thinking>a\nb</thinking>  Answer"),
      "Answer"
    );
    // Unclosed trailing think block is dropped entirely.
    assert_eq!(clean_assistant_content("Answer\n<think>trailing"), "Answer");
    assert_eq!(
      clean_assistant_content("<!--sources: [[a]]-->\n\n  Body  \n"),
      "Body"
    );
    assert_eq!(clean_assistant_content("<think>only thoughts"), "");
  }

  #[test]
  fn derives_title_from_first_visible_line() {
    assert_eq!(title_from_clean_content("## Heading\nbody"), "Heading");
    assert_eq!(title_from_clean_content("\n\n  plain line"), "plain line");
    assert_eq!(title_from_clean_content(""), "Saved Query");
    let long = "x".repeat(80);
    assert_eq!(title_from_clean_content(&long).chars().count(), 60);
  }

  #[test]
  fn slug_keeps_unicode_letters() {
    assert_eq!(make_query_slug("Hello World"), "hello-world");
    assert_eq!(make_query_slug("量子 计算 概述"), "量子-计算-概述");
    assert_eq!(make_query_slug("🚀🚀🚀"), "query");
    assert_eq!(make_query_slug(""), "query");
    assert_eq!(make_query_slug("--a--b--"), "a-b");
    // NFKC folds full-width forms.
    assert_eq!(make_query_slug("ＡＢＣ　１２３"), "abc-123");
    let long = "a".repeat(80);
    assert_eq!(make_query_slug(&long).chars().count(), 50);
  }

  #[test]
  fn file_name_is_slug_date_time() {
    let named = make_query_file_name("Demo Title", test_now());
    assert_eq!(named.slug, "demo-title");
    assert_eq!(named.date, "2026-04-23");
    assert_eq!(named.time, "143052");
    assert_eq!(named.file_name, "demo-title-2026-04-23-143052.md");
  }

  fn temp_root() -> (tempfile::TempDir, ProjectRoot) {
    let dir = tempfile::tempdir().unwrap();
    let root = ProjectRoot::new(dir.path()).unwrap();
    (dir, root)
  }

  #[test]
  fn saves_page_and_creates_index_and_log() {
    let (dir, root) = temp_root();
    let saved = save_query_page_at(
      &root,
      "# Demo Title\n\nBody text",
      test_now(),
    )
    .unwrap();
    assert_eq!(saved.title, "Demo Title");
    assert_eq!(saved.path, "wiki/queries/demo-title-2026-04-23-143052.md");

    let page = std::fs::read_to_string(dir.path().join(&saved.path)).unwrap();
    assert!(page.starts_with(
      "---\ntype: query\ntitle: \"Demo Title\"\ncreated: 2026-04-23\ntags: []\n---\n# Demo Title"
    ));

    let index = std::fs::read_to_string(dir.path().join("wiki/index.md")).unwrap();
    assert!(index.contains(
      "## Queries\n- [[queries/demo-title-2026-04-23-143052|Demo Title]]"
    ));

    let log = std::fs::read_to_string(dir.path().join("wiki/log.md")).unwrap();
    assert!(log.starts_with("# Wiki Log\n"));
    assert!(log.contains(
      "- 2026-04-23: Saved query page `demo-title-2026-04-23-143052.md`\n"
    ));
  }

  #[test]
  fn same_second_saves_produce_distinct_files() {
    let (dir, root) = temp_root();
    let first = save_query_page_at(&root, "# Demo Title\n\nfirst", test_now()).unwrap();
    let second = save_query_page_at(&root, "# Demo Title\n\nsecond", test_now()).unwrap();
    assert_eq!(first.file_name, "demo-title-2026-04-23-143052.md");
    assert_eq!(second.file_name, "demo-title-2026-04-23-143052-2.md");
    let first_body =
      std::fs::read_to_string(dir.path().join(&first.path)).unwrap();
    let second_body =
      std::fs::read_to_string(dir.path().join(&second.path)).unwrap();
    assert!(first_body.contains("first"));
    assert!(second_body.contains("second"));
    let index = std::fs::read_to_string(dir.path().join("wiki/index.md")).unwrap();
    assert!(index.contains("queries/demo-title-2026-04-23-143052-2|Demo Title"));
  }

  #[test]
  fn inserts_into_existing_queries_section() {
    let (dir, root) = temp_root();
    std::fs::create_dir_all(dir.path().join("wiki")).unwrap();
    std::fs::write(
      dir.path().join("wiki/index.md"),
      "# Wiki Index\n\n## Queries\n- [[queries/old|Old]]\n\n## Other\n",
    )
    .unwrap();
    save_query_page_at(&root, "New Entry", test_now()).unwrap();
    let index = std::fs::read_to_string(dir.path().join("wiki/index.md")).unwrap();
    assert!(index.contains(
      "## Queries\n- [[queries/new-entry-2026-04-23-143052|New Entry]]\n- [[queries/old|Old]]"
    ));
    assert!(index.contains("## Other"));
  }

  #[test]
  fn appends_section_when_index_lacks_queries_heading() {
    let (dir, root) = temp_root();
    std::fs::create_dir_all(dir.path().join("wiki")).unwrap();
    std::fs::write(dir.path().join("wiki/index.md"), "# Wiki Index\n\n## Pages\n").unwrap();
    save_query_page_at(&root, "Solo", test_now()).unwrap();
    let index = std::fs::read_to_string(dir.path().join("wiki/index.md")).unwrap();
    assert!(index.ends_with(
      "## Queries\n- [[queries/solo-2026-04-23-143052|Solo]]\n"
    ));
  }

  #[test]
  fn appends_to_existing_log() {
    let (dir, root) = temp_root();
    std::fs::create_dir_all(dir.path().join("wiki")).unwrap();
    std::fs::write(dir.path().join("wiki/log.md"), "# Wiki Log\n\n- old entry\n").unwrap();
    save_query_page_at(&root, "Logged", test_now()).unwrap();
    let log = std::fs::read_to_string(dir.path().join("wiki/log.md")).unwrap();
    assert!(log.contains("- old entry\n- 2026-04-23: Saved query page"));
  }

  #[test]
  fn rejects_empty_cleaned_content() {
    let (_dir, root) = temp_root();
    assert!(matches!(
      save_query_page_at(&root, "<think>nothing else", OffsetDateTime::now_utc()),
      Err(QuerySaveError::EmptyContent)
    ));
  }
}
