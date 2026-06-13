use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::page_merge::{
  merge_array_fields_into_content, parse_frontmatter_array, set_frontmatter_scalar,
  split_frontmatter_lines_owned, write_frontmatter_array,
};
use super::root::{ProjectRoot, ProjectRootError};

/// Ported from upstream_llm_wiki/src/lib/dedup.ts (EntitySummary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySummary {
  pub slug: String,
  pub path: String,
  pub page_type: String,
  pub title: String,
  pub description: Option<String>,
  pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupPage {
  pub slug: String,
  pub path: String,
  pub content: String,
}

pub fn extract_entity_summary(relative_path: &str, content: &str) -> Option<EntitySummary> {
  let (frontmatter_lines, body, _newline) = split_frontmatter_lines_owned(content)?;

  let mut fields = BTreeMap::new();
  for line in &frontmatter_lines {
    let Some((key, value)) = line.split_once(':') else {
      continue;
    };
    let normalized = value.trim().trim_matches('"').trim_matches('\'').trim();
    if normalized.is_empty() {
      continue;
    }
    fields.insert(key.trim().to_string(), normalized.to_string());
  }

  let slug = slug_from_path(relative_path);
  let page_type = fields
    .get("type")
    .cloned()
    .unwrap_or_else(|| "unknown".to_string());
  let title = fields.get("title").cloned().unwrap_or_else(|| slug.clone());
  let description = fields
    .get("description")
    .cloned()
    .or_else(|| first_body_paragraph(&body))
    .map(|value| truncate_chars(&value, 200));
  let tags = parse_frontmatter_array(content, "tags");

  Some(EntitySummary {
    slug,
    path: relative_path.to_string(),
    page_type,
    title,
    description,
    tags,
  })
}

pub fn slug_from_path(path: &str) -> String {
  let base = path.rsplit('/').next().unwrap_or(path);
  base.strip_suffix(".md").unwrap_or(base).to_string()
}

fn first_body_paragraph(body: &str) -> Option<String> {
  body
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .find(|line| !line.starts_with('#') && !line.starts_with('|'))
    .map(str::to_string)
}

fn truncate_chars(value: &str, max: usize) -> String {
  if value.chars().count() <= max {
    return value.to_string();
  }
  let mut truncated = value.chars().take(max - 1).collect::<String>();
  truncated.push('…');
  truncated
}

/// Walk wiki/entities + wiki/concepts (upstream's extract scope), sorted by path.
pub fn collect_entity_pages(root: &ProjectRoot) -> Result<Vec<DedupPage>, ProjectRootError> {
  let mut pages = Vec::new();
  for subdir in ["wiki/concepts", "wiki/entities"] {
    let dir = root.safe_join(subdir)?;
    if dir.exists() {
      collect_markdown_pages(root.as_path(), &dir, &mut pages)?;
    }
  }
  pages.sort_by(|left, right| left.path.cmp(&right.path));
  Ok(pages)
}

/// Every .md under wiki/ — upstream's MergeRequest.otherWikiPages source.
pub fn collect_all_wiki_pages(root: &ProjectRoot) -> Result<Vec<DedupPage>, ProjectRootError> {
  let mut pages = Vec::new();
  let wiki_root = root.safe_join("wiki")?;
  if wiki_root.exists() {
    collect_markdown_pages(root.as_path(), &wiki_root, &mut pages)?;
  }
  pages.sort_by(|left, right| left.path.cmp(&right.path));
  Ok(pages)
}

fn collect_markdown_pages(
  project_root: &Path,
  dir: &Path,
  pages: &mut Vec<DedupPage>,
) -> Result<(), ProjectRootError> {
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();
    if entry.file_type()?.is_dir() {
      collect_markdown_pages(project_root, &path, pages)?;
      continue;
    }
    if path.extension().and_then(|value| value.to_str()) != Some("md") {
      continue;
    }
    let relative_path = path
      .strip_prefix(project_root)
      .unwrap_or(&path)
      .to_string_lossy()
      .replace('\\', "/");
    let content = fs::read_to_string(&path)?;
    pages.push(DedupPage {
      slug: slug_from_path(&relative_path),
      path: relative_path,
      content,
    });
  }
  Ok(())
}

/// Verbatim from upstream_llm_wiki/src/lib/dedup.ts DETECTOR_SYSTEM_PROMPT.
pub const DETECTOR_SYSTEM_PROMPT: &str = r#"You are a wiki maintenance assistant. You will receive a list of entity / concept pages from a wiki. Identify groups of slugs that likely refer to the same underlying topic under different names — for example:

- Same name in two languages (English vs Chinese, etc.)
- Plural vs singular form (e.g. "dpao" vs "dpaos")
- Abbreviation vs full form (e.g. "vfa" vs "volatile-fatty-acids")
- Synonyms in the same language
- The same proper noun spelled differently

Output ONLY valid JSON. No prose, no markdown fences, no explanation outside the JSON. The schema is:

{
  "groups": [
    {
      "slugs": ["slug-a", "slug-b"],
      "reason": "Both refer to X; first is English, second is Chinese.",
      "confidence": "high"
    }
  ]
}

Rules:
- Only include groups of 2 or more slugs from the input list.
- "high" = clearly the same entity, only naming differs.
- "medium" = likely the same but context-dependent.
- "low" = uncertain; user should review carefully.
- Never invent slugs that aren't in the input.
- If no duplicates exist, output {"groups": []}.
- Pages of different `type` (e.g. an entity and a concept) usually should NOT be grouped — only group across types when they're unambiguously the same thing."#;

/// Verbatim from upstream_llm_wiki/src/lib/dedup.ts MERGER_SYSTEM_PROMPT.
pub const MERGER_SYSTEM_PROMPT: &str = r#"You are a wiki maintenance assistant. You will be given several wiki pages that all describe the same entity or concept under different names. Merge them into a single coherent wiki page.

Output the COMPLETE merged file (frontmatter + body). The first character of your response MUST be "-" (the opening of "---"). No preamble, no explanation outside the file.

Rules:
- Preserve every distinct factual claim from every input page.
- Eliminate redundancy (don't say the same thing twice across sections).
- Reorganize sections so the structure is logical for the unified topic, not a concatenation of inputs.
- Use [[wikilink]] syntax in the body where the inputs did.
- Frontmatter: keep the standard fields (type, title, created, updated, tags, related, sources). The caller will overwrite sources / tags / related / updated with deterministic unions afterward — your job is to produce a sensible body and reasonable frontmatter shape.
- Pick the most descriptive title. If the inputs use different languages, prefer the language that matches the majority of the body content."#;

const FIELDS_TO_UNION: &[&str] = &["sources", "tags", "related"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroupCandidate {
  pub slugs: Vec<String>,
  pub reason: String,
  pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupRewrite {
  pub path: String,
  pub new_content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupBackupEntry {
  pub path: String,
  pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupMergeOutcome {
  pub canonical_content: String,
  pub canonical_path: String,
  pub rewrites: Vec<DedupRewrite>,
  pub pages_to_delete: Vec<String>,
  pub backup: Vec<DedupBackupEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum DedupMergeError {
  #[error("canonical slug \"{0}\" is not in the group")]
  CanonicalNotInGroup(String),
  #[error("dedup merge requires at least 2 pages in the group")]
  GroupTooSmall,
}

/// Ported from dedup.ts buildDetectorUserMessage.
pub fn build_detector_user_message(summaries: &[EntitySummary]) -> String {
  let lines = summaries
    .iter()
    .map(|summary| {
      let tag_part = if summary.tags.is_empty() {
        String::new()
      } else {
        format!(" [{}]", summary.tags.join(", "))
      };
      let desc_part = summary
        .description
        .as_deref()
        .map(|description| format!(" — {description}"))
        .unwrap_or_default();
      let title = serde_json::to_string(&summary.title)
        .unwrap_or_else(|_| format!("\"{}\"", summary.title));
      format!(
        "- type={}, slug={}, title={}{}{}",
        summary.page_type, summary.slug, title, tag_part, desc_part
      )
    })
    .collect::<Vec<_>>();
  format!(
    "## Wiki pages to scan ({} entries)\n\n{}\n\nReturn duplicate groups as JSON only.",
    summaries.len(),
    lines.join("\n")
  )
}

/// Ported from dedup.ts parseDetectorResponse — tolerant, returns [] on any failure.
pub fn parse_detector_response(raw: &str) -> Vec<DuplicateGroupCandidate> {
  let Some(json_text) = extract_first_json_object(raw) else {
    return Vec::new();
  };
  let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_text) else {
    return Vec::new();
  };
  let Some(groups_raw) = parsed.get("groups").and_then(serde_json::Value::as_array) else {
    return Vec::new();
  };

  let mut out = Vec::new();
  for group in groups_raw {
    let slugs = group
      .get("slugs")
      .and_then(serde_json::Value::as_array)
      .map(|values| {
        values
          .iter()
          .filter_map(serde_json::Value::as_str)
          .map(str::to_string)
          .collect::<Vec<_>>()
      })
      .unwrap_or_default();
    if slugs.len() < 2 {
      continue;
    }
    let reason = group
      .get("reason")
      .and_then(serde_json::Value::as_str)
      .unwrap_or_default()
      .to_string();
    let confidence = match group.get("confidence").and_then(serde_json::Value::as_str) {
      Some("high") => "high",
      Some("medium") => "medium",
      _ => "low",
    }
    .to_string();
    out.push(DuplicateGroupCandidate { slugs, reason, confidence });
  }
  out
}

/// Ported from dedup.ts extractFirstJsonObject — balanced-brace scan.
fn extract_first_json_object(text: &str) -> Option<&str> {
  let start = text.find('{')?;
  let mut depth = 0usize;
  let mut in_string = false;
  let mut escape = false;
  for (index, ch) in text[start..].char_indices() {
    if escape {
      escape = false;
      continue;
    }
    match ch {
      '\\' => escape = true,
      '"' => in_string = !in_string,
      '{' if !in_string => depth += 1,
      '}' if !in_string => {
        depth -= 1;
        if depth == 0 {
          return Some(&text[start..start + index + ch.len_utf8()]);
        }
      }
      _ => {}
    }
  }
  None
}

/// Ported from dedup.ts normalizeGroupKey — lowercased, sorted, comma-joined.
pub fn normalize_group_key(slugs: &[String]) -> String {
  let mut keys = slugs.iter().map(|slug| slug.to_lowercase()).collect::<Vec<_>>();
  keys.sort();
  keys.join(",")
}

/// Ported from the filter chain in dedup.ts detectDuplicateGroups.
pub fn filter_detected_groups(
  groups: Vec<DuplicateGroupCandidate>,
  valid_slugs: &BTreeSet<String>,
  not_duplicates: &[Vec<String>],
) -> Vec<DuplicateGroupCandidate> {
  let not_dup_keys = not_duplicates
    .iter()
    .map(|group| normalize_group_key(group))
    .collect::<BTreeSet<_>>();

  groups
    .into_iter()
    .map(|mut group| {
      group.slugs.retain(|slug| valid_slugs.contains(slug));
      group
    })
    .filter(|group| group.slugs.len() >= 2)
    .filter(|group| !not_dup_keys.contains(&normalize_group_key(&group.slugs)))
    .collect()
}

/// Ported from dedup.ts buildMergerUserMessage.
pub fn build_merger_user_message(group: &[DedupPage]) -> String {
  let sections = group
    .iter()
    .enumerate()
    .map(|(index, page)| {
      format!("## Page {} (slug: {})\n\n{}\n", index + 1, page.slug, page.content)
    })
    .collect::<Vec<_>>();
  let canonical_hint = group.first().map(|page| page.slug.clone()).unwrap_or_default();
  [
    format!(
      "These {} wiki pages have been confirmed by the user to describe the same topic.",
      group.len()
    ),
    format!(
      "Merge them into a single coherent page (the canonical slug will be \"{canonical_hint}\" or whichever the caller chose)."
    ),
    String::new(),
    sections.join("\n---\n\n"),
    String::new(),
    "Now output the merged file. First character must be `-`.".to_string(),
  ]
  .join("\n")
}

/// Ported from dedup.ts rewriteCrossReferences. Returns Some(new_content)
/// only when something changed (upstream compares strings at the call site).
pub fn rewrite_cross_references(
  content: &str,
  slug_redirects: &BTreeMap<String, String>,
) -> Option<String> {
  let mut out = rewrite_wikilinks(content, slug_redirects);

  let existing = parse_frontmatter_array(&out, "related");
  if !existing.is_empty() {
    let rewritten = existing
      .iter()
      .map(|slug| slug_redirects.get(slug).cloned().unwrap_or_else(|| slug.clone()))
      .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut unique = Vec::new();
    for slug in rewritten {
      if seen.insert(slug.to_lowercase()) {
        unique.push(slug);
      }
    }
    if unique != existing {
      out = write_frontmatter_array(&out, "related", &unique);
    }
  }

  if out == content { None } else { Some(out) }
}

/// [[slug]] and [[slug|alias]] forms — exact slug match on the target portion.
fn rewrite_wikilinks(content: &str, slug_redirects: &BTreeMap<String, String>) -> String {
  let mut out = String::with_capacity(content.len());
  let mut rest = content;
  while let Some(start) = rest.find("[[") {
    let Some(end_offset) = rest[start + 2..].find("]]") else {
      break;
    };
    let inner = &rest[start + 2..start + 2 + end_offset];
    let (target, alias) = match inner.split_once('|') {
      Some((target, alias)) => (target, Some(alias)),
      None => (inner, None),
    };
    out.push_str(&rest[..start]);
    match slug_redirects.get(target) {
      Some(new_slug) => {
        out.push_str("[[");
        out.push_str(new_slug);
        if let Some(alias) = alias {
          out.push('|');
          out.push_str(alias);
        }
        out.push_str("]]");
      }
      None => out.push_str(&rest[start..start + 2 + end_offset + 2]),
    }
    rest = &rest[start + 2 + end_offset + 2..];
  }
  out.push_str(rest);
  out
}

/// Ported from dedup.ts mergeDuplicateGroup, minus the LLM call (the server
/// executor calls the provider and passes llm_output in).
pub fn compute_dedup_merge(
  group: &[DedupPage],
  canonical_slug: &str,
  other_wiki_pages: &[DedupPage],
  llm_output: &str,
  today: &str,
) -> Result<DedupMergeOutcome, DedupMergeError> {
  let canonical = group
    .iter()
    .find(|page| page.slug == canonical_slug)
    .ok_or_else(|| DedupMergeError::CanonicalNotInGroup(canonical_slug.to_string()))?;
  if group.len() < 2 {
    return Err(DedupMergeError::GroupTooSmall);
  }

  let mut merged = llm_output.to_string();
  for page in group {
    merged = merge_array_fields_into_content(&merged, Some(&page.content), FIELDS_TO_UNION);
  }
  merged = set_frontmatter_scalar(&merged, "updated", today);

  let mut slug_redirects = BTreeMap::new();
  for page in group {
    if page.slug != canonical_slug {
      slug_redirects.insert(page.slug.clone(), canonical_slug.to_string());
    }
  }

  let mut rewrites = Vec::new();
  for page in other_wiki_pages {
    if let Some(new_content) = rewrite_cross_references(&page.content, &slug_redirects) {
      rewrites.push(DedupRewrite {
        path: page.path.clone(),
        new_content,
      });
    }
  }

  let mut backup = group
    .iter()
    .map(|page| DedupBackupEntry {
      path: page.path.clone(),
      content: page.content.clone(),
    })
    .collect::<Vec<_>>();
  for rewrite in &rewrites {
    if let Some(original) = other_wiki_pages.iter().find(|page| page.path == rewrite.path) {
      backup.push(DedupBackupEntry {
        path: original.path.clone(),
        content: original.content.clone(),
      });
    }
  }

  let pages_to_delete = group
    .iter()
    .filter(|page| page.slug != canonical_slug)
    .map(|page| page.path.clone())
    .collect::<Vec<_>>();

  Ok(DedupMergeOutcome {
    canonical_content: merged,
    canonical_path: canonical.path.clone(),
    rewrites,
    pages_to_delete,
    backup,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_entity_summary_reads_frontmatter_fields() {
    let content = "---\ntype: entity\ntitle: Volatile Fatty Acids\ndescription: Short-chain fatty acids.\ntags: [chemistry, metabolism]\n---\n\n# VFA\n\nBody text here.\n";
    let summary = extract_entity_summary("wiki/entities/vfa.md", content).unwrap();
    assert_eq!(summary.slug, "vfa");
    assert_eq!(summary.path, "wiki/entities/vfa.md");
    assert_eq!(summary.page_type, "entity");
    assert_eq!(summary.title, "Volatile Fatty Acids");
    assert_eq!(summary.description.as_deref(), Some("Short-chain fatty acids."));
    assert_eq!(summary.tags, vec!["chemistry".to_string(), "metabolism".to_string()]);
  }

  #[test]
  fn extract_entity_summary_returns_none_without_frontmatter() {
    assert!(extract_entity_summary("wiki/entities/foo.md", "# Foo\n\nNo frontmatter.").is_none());
  }

  #[test]
  fn extract_entity_summary_falls_back_to_first_body_paragraph() {
    let content = "---\ntype: concept\ntitle: Attention\n---\n\n# Attention\n\n| a | b |\n\nFocuses computation on relevant tokens.\n";
    let summary = extract_entity_summary("wiki/concepts/attention.md", content).unwrap();
    assert_eq!(
      summary.description.as_deref(),
      Some("Focuses computation on relevant tokens.")
    );
  }

  #[test]
  fn extract_entity_summary_truncates_long_descriptions() {
    let long_line = "x".repeat(300);
    let content = format!("---\ntype: concept\ntitle: Foo\n---\n\n{long_line}\n");
    let summary = extract_entity_summary("wiki/concepts/foo.md", &content).unwrap();
    let description = summary.description.unwrap();
    assert_eq!(description.chars().count(), 200);
    assert!(description.ends_with('…'));
  }

  #[test]
  fn extract_entity_summary_defaults_title_and_type() {
    let content = "---\ncreated: 2026-06-12\n---\n\nBody.\n";
    let summary = extract_entity_summary("wiki/entities/some-slug.md", content).unwrap();
    assert_eq!(summary.title, "some-slug");
    assert_eq!(summary.page_type, "unknown");
    assert!(summary.tags.is_empty());
  }

  #[test]
  fn collect_entity_pages_walks_entities_and_concepts_only() {
    let temp = tempfile::tempdir().unwrap();
    let root = crate::project::root::ProjectRoot::new(temp.path()).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/entities")).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/concepts/nested")).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/sources")).unwrap();
    std::fs::write(temp.path().join("wiki/entities/b.md"), "b").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/a.md"), "a").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/nested/c.md"), "c").unwrap();
    std::fs::write(temp.path().join("wiki/concepts/skip.txt"), "no").unwrap();
    std::fs::write(temp.path().join("wiki/sources/s.md"), "s").unwrap();

    let pages = collect_entity_pages(&root).unwrap();
    let paths = pages.iter().map(|page| page.path.as_str()).collect::<Vec<_>>();
    assert_eq!(
      paths,
      vec!["wiki/concepts/a.md", "wiki/concepts/nested/c.md", "wiki/entities/b.md"]
    );
    assert_eq!(pages[0].slug, "a");
    assert_eq!(pages[0].content, "a");
  }

  #[test]
  fn collect_all_wiki_pages_includes_every_markdown_file() {
    let temp = tempfile::tempdir().unwrap();
    let root = crate::project::root::ProjectRoot::new(temp.path()).unwrap();
    std::fs::create_dir_all(temp.path().join("wiki/sources")).unwrap();
    std::fs::write(temp.path().join("wiki/index.md"), "index").unwrap();
    std::fs::write(temp.path().join("wiki/sources/s.md"), "s").unwrap();

    let pages = collect_all_wiki_pages(&root).unwrap();
    let paths = pages.iter().map(|page| page.path.as_str()).collect::<Vec<_>>();
    assert_eq!(paths, vec!["wiki/index.md", "wiki/sources/s.md"]);
  }

  #[test]
  fn build_detector_user_message_lists_pages_with_tags_and_description() {
    let summaries = vec![
      EntitySummary {
        slug: "vfa".to_string(),
        path: "wiki/entities/vfa.md".to_string(),
        page_type: "entity".to_string(),
        title: "VFA".to_string(),
        description: Some("Short-chain fatty acids.".to_string()),
        tags: vec!["chemistry".to_string()],
      },
      EntitySummary {
        slug: "volatile-fatty-acids".to_string(),
        path: "wiki/entities/volatile-fatty-acids.md".to_string(),
        page_type: "entity".to_string(),
        title: "Volatile Fatty Acids".to_string(),
        description: None,
        tags: Vec::new(),
      },
    ];
    let message = build_detector_user_message(&summaries);
    assert!(message.starts_with("## Wiki pages to scan (2 entries)\n\n"));
    assert!(message.contains(
      "- type=entity, slug=vfa, title=\"VFA\" [chemistry] — Short-chain fatty acids."
    ));
    assert!(message.contains("- type=entity, slug=volatile-fatty-acids, title=\"Volatile Fatty Acids\""));
    assert!(message.ends_with("\n\nReturn duplicate groups as JSON only."));
  }

  #[test]
  fn parse_detector_response_extracts_json_from_noise() {
    let raw = "Sure, here you go:\n```json\n{\"groups\":[{\"slugs\":[\"a\",\"b\"],\"reason\":\"same\",\"confidence\":\"high\"}]}\n```\nLet me know!";
    let groups = parse_detector_response(raw);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].slugs, vec!["a".to_string(), "b".to_string()]);
    assert_eq!(groups[0].reason, "same");
    assert_eq!(groups[0].confidence, "high");
  }

  #[test]
  fn parse_detector_response_tolerates_garbage_and_bad_entries() {
    assert!(parse_detector_response("no json here").is_empty());
    assert!(parse_detector_response("{not valid json").is_empty());
    assert!(parse_detector_response("{\"groups\": \"nope\"}").is_empty());
    let raw = "{\"groups\":[{\"slugs\":[\"only-one\"]},{\"slugs\":[\"a\",\"b\"],\"confidence\":\"certain\"}]}";
    let groups = parse_detector_response(raw);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].confidence, "low");
    assert_eq!(groups[0].reason, "");
  }

  #[test]
  fn filter_detected_groups_drops_invented_small_and_whitelisted() {
    let valid_slugs = ["a", "b", "c"]
      .iter()
      .map(|slug| slug.to_string())
      .collect::<std::collections::BTreeSet<_>>();
    let groups = vec![
      DuplicateGroupCandidate {
        slugs: vec!["a".to_string(), "invented".to_string(), "b".to_string()],
        reason: "r1".to_string(),
        confidence: "high".to_string(),
      },
      DuplicateGroupCandidate {
        slugs: vec!["c".to_string(), "invented".to_string()],
        reason: "r2".to_string(),
        confidence: "medium".to_string(),
      },
      DuplicateGroupCandidate {
        slugs: vec!["B".to_string(), "a".to_string()],
        reason: "r3".to_string(),
        confidence: "low".to_string(),
      },
    ];
    let not_duplicates = vec![vec!["b".to_string(), "a".to_string()]];
    let filtered = filter_detected_groups(groups, &valid_slugs, &not_duplicates);
    assert!(filtered.is_empty());

    let surviving = filter_detected_groups(
      vec![DuplicateGroupCandidate {
        slugs: vec!["a".to_string(), "c".to_string()],
        reason: "keep".to_string(),
        confidence: "high".to_string(),
      }],
      &valid_slugs,
      &not_duplicates,
    );
    assert_eq!(surviving.len(), 1);
    assert_eq!(surviving[0].slugs, vec!["a".to_string(), "c".to_string()]);
  }

  #[test]
  fn normalize_group_key_is_case_insensitive_and_sorted() {
    assert_eq!(
      normalize_group_key(&["B".to_string(), "a".to_string()]),
      "a,b"
    );
  }

  #[test]
  fn rewrite_cross_references_rewrites_wikilinks_and_related() {
    let mut redirects = std::collections::BTreeMap::new();
    redirects.insert("attention".to_string(), "attention-mechanism".to_string());

    let content = "---\ntype: source\ntitle: Paper\nrelated: [attention, transformer]\n---\n\nSee [[attention]] and [[attention|the attention idea]] and [[transformer]].\n";
    let rewritten = rewrite_cross_references(content, &redirects).unwrap();
    assert!(rewritten.contains("See [[attention-mechanism]] and [[attention-mechanism|the attention idea]] and [[transformer]]."));
    assert!(rewritten.contains("related: [\"attention-mechanism\", \"transformer\"]"));
  }

  #[test]
  fn rewrite_cross_references_dedups_related_case_insensitively() {
    let mut redirects = std::collections::BTreeMap::new();
    redirects.insert("attn".to_string(), "Attention".to_string());

    let content = "---\ntype: concept\ntitle: Foo\nrelated: [attention, attn]\n---\n\nBody.\n";
    let rewritten = rewrite_cross_references(content, &redirects).unwrap();
    assert!(rewritten.contains("related: [\"attention\"]"));
  }

  #[test]
  fn rewrite_cross_references_returns_none_when_unchanged() {
    let mut redirects = std::collections::BTreeMap::new();
    redirects.insert("missing".to_string(), "other".to_string());
    let content = "---\ntype: concept\ntitle: Foo\n---\n\nNo links here.\n";
    assert!(rewrite_cross_references(content, &redirects).is_none());
  }

  #[test]
  fn build_merger_user_message_lists_group_pages() {
    let group = vec![
      DedupPage {
        slug: "attention".to_string(),
        path: "wiki/concepts/attention.md".to_string(),
        content: "content-a".to_string(),
      },
      DedupPage {
        slug: "attention-mechanism".to_string(),
        path: "wiki/concepts/attention-mechanism.md".to_string(),
        content: "content-b".to_string(),
      },
    ];
    let message = build_merger_user_message(&group);
    assert!(message.starts_with("These 2 wiki pages have been confirmed by the user to describe the same topic."));
    assert!(message.contains("## Page 1 (slug: attention)\n\ncontent-a"));
    assert!(message.contains("## Page 2 (slug: attention-mechanism)\n\ncontent-b"));
    assert!(message.contains("the canonical slug will be \"attention\""));
    assert!(message.ends_with("Now output the merged file. First character must be `-`."));
  }

  #[test]
  fn compute_dedup_merge_unions_frontmatter_and_rewrites_references() {
    let group = vec![
      DedupPage {
        slug: "attention".to_string(),
        path: "wiki/concepts/attention.md".to_string(),
        content: "---\ntype: concept\ntitle: Attention\nsources: [\"a.md\"]\ntags: [transformers]\nrelated: [transformer]\n---\n\nOld attention body.\n".to_string(),
      },
      DedupPage {
        slug: "attention-mechanism".to_string(),
        path: "wiki/concepts/attention-mechanism.md".to_string(),
        content: "---\ntype: concept\ntitle: Attention Mechanism\nsources: [\"b.md\"]\n---\n\nMechanism body.\n".to_string(),
      },
    ];
    let other_pages = vec![DedupPage {
      slug: "transformer-paper".to_string(),
      path: "wiki/sources/transformer-paper.md".to_string(),
      content: "---\ntype: source\ntitle: Transformer Paper\nrelated: [attention]\n---\n\nSee [[attention]] for details.\n".to_string(),
    }];
    let llm_output = "---\ntype: concept\ntitle: Attention Mechanism\nsources: [\"b.md\"]\n---\n\nMerged body covering both.\n";

    let outcome = compute_dedup_merge(
      &group,
      "attention-mechanism",
      &other_pages,
      llm_output,
      "2026-06-12",
    )
    .unwrap();

    assert_eq!(outcome.canonical_path, "wiki/concepts/attention-mechanism.md");
    assert!(outcome.canonical_content.contains("Merged body covering both."));
    assert!(outcome.canonical_content.contains("sources: [\"b.md\", \"a.md\"]"));
    assert!(outcome.canonical_content.contains("tags: [\"transformers\"]"));
    assert!(outcome.canonical_content.contains("related: [\"transformer\"]"));
    assert!(outcome.canonical_content.contains("updated: 2026-06-12"));

    assert_eq!(outcome.pages_to_delete, vec!["wiki/concepts/attention.md".to_string()]);

    assert_eq!(outcome.rewrites.len(), 1);
    assert_eq!(outcome.rewrites[0].path, "wiki/sources/transformer-paper.md");
    assert!(outcome.rewrites[0].new_content.contains("See [[attention-mechanism]] for details."));
    assert!(outcome.rewrites[0].new_content.contains("related: [\"attention-mechanism\"]"));

    let backup_paths = outcome.backup.iter().map(|entry| entry.path.as_str()).collect::<Vec<_>>();
    assert_eq!(
      backup_paths,
      vec![
        "wiki/concepts/attention.md",
        "wiki/concepts/attention-mechanism.md",
        "wiki/sources/transformer-paper.md"
      ]
    );
    assert!(outcome.backup[2].content.contains("See [[attention]] for details."));
  }

  #[test]
  fn compute_dedup_merge_rejects_bad_canonical_or_small_group() {
    let page = DedupPage {
      slug: "a".to_string(),
      path: "wiki/concepts/a.md".to_string(),
      content: "---\ntype: concept\ntitle: A\n---\n\nBody.\n".to_string(),
    };
    let err = compute_dedup_merge(&[page.clone(), page.clone()], "missing", &[], "---\n---\n", "2026-06-12")
      .unwrap_err();
    assert!(matches!(err, DedupMergeError::CanonicalNotInGroup(_)));

    let err = compute_dedup_merge(&[page], "a", &[], "---\n---\n", "2026-06-12").unwrap_err();
    assert!(matches!(err, DedupMergeError::GroupTooSmall));
  }
}
