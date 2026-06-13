use std::collections::{BTreeMap, BTreeSet};

const BODY_SHRINK_THRESHOLD: f32 = 0.7;
const UNION_FIELDS: &[&str] = &["sources", "tags", "related"];
const LOCKED_FIELDS: &[&str] = &["type", "title", "created"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageMergePlan {
  Final(String),
  NeedsProvider {
    existing_content: String,
    array_merged: String,
  },
}

#[derive(Debug, Clone, Default)]
struct ParsedFrontmatter {
  fields: BTreeMap<String, String>,
  body: String,
}

pub fn prepare_page_merge(new_content: &str, existing_content: Option<&str>) -> PageMergePlan {
  let Some(existing_content) = existing_content else {
    return PageMergePlan::Final(new_content.to_string());
  };

  if new_content == existing_content {
    return PageMergePlan::Final(existing_content.to_string());
  }

  let array_merged = merge_array_fields_into_content(new_content, Some(existing_content), UNION_FIELDS);
  let old_parsed = parse_frontmatter(existing_content);
  let merged_parsed = parse_frontmatter(&array_merged);
  if old_parsed.body.trim() == merged_parsed.body.trim() {
    return PageMergePlan::Final(array_merged);
  }

  PageMergePlan::NeedsProvider {
    existing_content: existing_content.to_string(),
    array_merged,
  }
}

pub fn finalize_page_merge(
  existing_content: &str,
  array_merged: &str,
  llm_output: Option<&str>,
  today: &str,
) -> String {
  let Some(llm_output) = llm_output else {
    return array_merged.to_string();
  };

  let old_parsed = parse_frontmatter(existing_content);
  let array_merged_parsed = parse_frontmatter(array_merged);
  let llm_parsed = parse_frontmatter(llm_output);
  if llm_parsed.fields.is_empty() {
    return array_merged.to_string();
  }

  let old_body_len = old_parsed.body.len();
  let new_body_len = array_merged_parsed.body.len();
  let llm_body_len = llm_parsed.body.len();
  let min_threshold = (old_body_len.max(new_body_len) as f32) * BODY_SHRINK_THRESHOLD;
  if (llm_body_len as f32) < min_threshold {
    return array_merged.to_string();
  }

  let mut final_content = llm_output.to_string();
  for field in LOCKED_FIELDS {
    if let Some(value) = old_parsed.fields.get(*field)
      && !value.trim().is_empty()
    {
      final_content = set_frontmatter_scalar(&final_content, field, value);
    }
  }
  final_content = merge_array_fields_into_content(&final_content, Some(array_merged), UNION_FIELDS);
  set_frontmatter_scalar(&final_content, "updated", today)
}

pub fn build_page_merge_prompts(
  existing_content: &str,
  incoming_content: &str,
  source_file_name: &str,
) -> (String, String) {
  let system_prompt = [
    "You are merging two versions of the same wiki page into one coherent document.",
    "Both versions describe the same entity or concept; one is already on disk,",
    "the other was just generated from a different source document.",
    "",
    "Output one merged version that:",
    "- Preserves every factual claim from both versions",
    "- Eliminates redundancy when both versions state the same fact",
    "- Reorganizes sections so the structure is logical for the merged topic",
    "- Uses consistent markdown structure",
    "- Keeps [[wikilink]] references intact",
    "",
    "Output requirements:",
    "- The first character of your response must be `-`",
    "- Output the complete file: YAML frontmatter plus body",
    "- No preamble, no analysis prose",
    "- The caller will overwrite `sources`, `tags`, `related`, and `updated` deterministically",
  ]
  .join("\n");

  let user_prompt = [
    "## Existing version on disk",
    "",
    existing_content,
    "",
    "---",
    "",
    &format!("## Newly generated version (from {source_file_name})"),
    "",
    incoming_content,
    "",
    "---",
    "",
    "Now output the merged file. Start with `---` on the first line.",
  ]
  .join("\n");

  (system_prompt, user_prompt)
}

fn parse_frontmatter(content: &str) -> ParsedFrontmatter {
  let Some((frontmatter_lines, rest, _newline)) = split_frontmatter_lines_owned(content) else {
    return ParsedFrontmatter {
      fields: BTreeMap::new(),
      body: content.to_string(),
    };
  };

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

  ParsedFrontmatter { fields, body: rest }
}

pub(crate) fn merge_array_fields_into_content(
  new_content: &str,
  existing_content: Option<&str>,
  fields: &[&str],
) -> String {
  let Some(existing_content) = existing_content else {
    return new_content.to_string();
  };
  if !existing_content.starts_with("---") || !new_content.starts_with("---") {
    return new_content.to_string();
  }

  let mut result = new_content.to_string();
  let mut changed = false;

  for field in fields {
    let old_values = parse_frontmatter_array(existing_content, field);
    if old_values.is_empty() {
      continue;
    }

    let new_values = parse_frontmatter_array(&result, field);
    let merged = merge_frontmatter_lists(&old_values, &new_values);
    if merged == new_values {
      continue;
    }

    result = write_frontmatter_array(&result, field, &merged);
    changed = true;
  }

  if changed { result } else { new_content.to_string() }
}

pub(crate) fn parse_frontmatter_array(content: &str, field_name: &str) -> Vec<String> {
  let Some((frontmatter_lines, _, _)) = split_frontmatter_lines_owned(content) else {
    return Vec::new();
  };

  let field_prefix = format!("{field_name}:");
  let mut index = 0usize;
  while index < frontmatter_lines.len() {
    let line = &frontmatter_lines[index];
    let trimmed = line.trim();
    if !trimmed.starts_with(&field_prefix) {
      index += 1;
      continue;
    }

    let remainder = trimmed[field_prefix.len()..].trim();
    if remainder.starts_with('[') && remainder.ends_with(']') {
      return split_inline_array(&remainder[1..remainder.len() - 1]);
    }

    if !remainder.is_empty() {
      return Vec::new();
    }

    let mut values = Vec::new();
    index += 1;
    while index < frontmatter_lines.len() {
      let next_line = &frontmatter_lines[index];
      let next_trimmed = next_line.trim();
      if next_trimmed.is_empty() {
        index += 1;
        continue;
      }
      if !(next_line.starts_with(' ') || next_line.starts_with('\t')) {
        break;
      }
      if let Some(value) = next_trimmed.strip_prefix("- ") {
        values.push(unquote_inline_array_item(value.trim()));
      }
      index += 1;
    }
    return values;
  }

  Vec::new()
}

pub(crate) fn write_frontmatter_array(content: &str, field_name: &str, values: &[String]) -> String {
  let Some((frontmatter_lines, rest, newline)) = split_frontmatter_lines_owned(content) else {
    return content.to_string();
  };

  let replacement = format!(
    "{field_name}: [{}]",
    values
      .iter()
      .map(|value| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")))
      .collect::<Vec<_>>()
      .join(", ")
  );
  let field_prefix = format!("{field_name}:");
  let mut rewritten = Vec::new();
  let mut replaced = false;
  let mut index = 0usize;

  while index < frontmatter_lines.len() {
    let line = &frontmatter_lines[index];
    let trimmed = line.trim();
    if trimmed.starts_with(&field_prefix) {
      rewritten.push(replacement.clone());
      replaced = true;
      index += 1;

      if trimmed[field_prefix.len()..].trim().is_empty() {
        while index < frontmatter_lines.len() {
          let next_line = &frontmatter_lines[index];
          let next_trimmed = next_line.trim();
          if next_trimmed.is_empty() || next_line.starts_with(' ') || next_line.starts_with('\t') {
            index += 1;
            continue;
          }
          break;
        }
      }
      continue;
    }

    rewritten.push(line.clone());
    index += 1;
  }

  if !replaced {
    rewritten.push(replacement);
  }

  if rest.is_empty() {
    format!("---{newline}{}{newline}---", rewritten.join(newline))
  } else {
    format!("---{newline}{}{newline}---{newline}{rest}", rewritten.join(newline))
  }
}

fn merge_frontmatter_lists(existing: &[String], incoming: &[String]) -> Vec<String> {
  let mut merged = Vec::new();
  let mut seen = BTreeSet::new();

  for value in existing.iter().chain(incoming.iter()) {
    let key = value.to_lowercase();
    if seen.insert(key) {
      merged.push(value.clone());
    }
  }

  merged
}

pub(crate) fn set_frontmatter_scalar(content: &str, field_name: &str, value: &str) -> String {
  let Some((frontmatter_lines, rest, newline)) = split_frontmatter_lines_owned(content) else {
    return content.to_string();
  };

  let field_prefix = format!("{field_name}:");
  let replacement = format!("{field_name}: {value}");
  let mut rewritten = Vec::new();
  let mut replaced = false;

  for line in &frontmatter_lines {
    let trimmed = line.trim();
    if trimmed.starts_with(&field_prefix) {
      rewritten.push(replacement.clone());
      replaced = true;
    } else {
      rewritten.push(line.clone());
    }
  }

  if !replaced {
    rewritten.push(replacement);
  }

  if rest.is_empty() {
    format!("---{newline}{}{newline}---", rewritten.join(newline))
  } else {
    format!("---{newline}{}{newline}---{newline}{rest}", rewritten.join(newline))
  }
}

pub(crate) fn split_frontmatter_lines_owned(content: &str) -> Option<(Vec<String>, String, &'static str)> {
  let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
  let lines = content.split(newline).collect::<Vec<_>>();
  if lines.first().copied() != Some("---") {
    return None;
  }

  let end_index = lines.iter().enumerate().skip(1).find_map(|(index, line)| {
    if *line == "---" {
      Some(index)
    } else {
      None
    }
  })?;

  let frontmatter_lines = lines[1..end_index]
    .iter()
    .map(|line| (*line).to_string())
    .collect::<Vec<_>>();
  let rest = lines[end_index + 1..].join(newline);
  Some((frontmatter_lines, rest, newline))
}

fn unquote_inline_array_item(value: &str) -> String {
  value.trim().trim_matches('"').trim_matches('\'').to_string()
}

fn split_inline_array(body: &str) -> Vec<String> {
  let mut values = Vec::new();
  let mut current = String::new();
  let mut quote = None;
  let mut escaped = false;

  for ch in body.chars() {
    if escaped {
      current.push(ch);
      escaped = false;
      continue;
    }
    if quote == Some('"') && ch == '\\' {
      escaped = true;
      continue;
    }
    if quote.is_none() && matches!(ch, '"' | '\'') {
      quote = Some(ch);
      continue;
    }
    if quote == Some(ch) {
      quote = None;
      continue;
    }
    if ch == ',' && quote.is_none() {
      let value = current.trim();
      if !value.is_empty() {
        values.push(unquote_inline_array_item(value));
      }
      current.clear();
      continue;
    }
    current.push(ch);
  }

  let value = current.trim();
  if !value.is_empty() {
    values.push(unquote_inline_array_item(value));
  }

  values
}

#[cfg(test)]
mod tests {
  use super::{build_page_merge_prompts, finalize_page_merge, prepare_page_merge, PageMergePlan};

  fn page(frontmatter: &str, body: &str) -> String {
    format!("---\n{frontmatter}\n---\n\n{body}")
  }

  #[test]
  fn prepare_page_merge_skips_provider_for_new_page_or_identical_content() {
    assert_eq!(
      prepare_page_merge(&page("type: concept\ntitle: Foo", "body"), None),
      PageMergePlan::Final(page("type: concept\ntitle: Foo", "body"))
    );

    let content = page("type: concept\ntitle: Foo", "body");
    assert_eq!(
      prepare_page_merge(&content, Some(&content)),
      PageMergePlan::Final(content)
    );
  }

  #[test]
  fn prepare_page_merge_skips_provider_when_only_array_fields_change() {
    let existing = page("type: concept\ntitle: Foo\nsources: [\"a.md\"]", "same body");
    let incoming = page("type: concept\ntitle: Foo\nsources: [\"b.md\"]", "same body");

    let PageMergePlan::Final(final_content) = prepare_page_merge(&incoming, Some(&existing)) else {
      panic!("expected final content");
    };

    assert!(final_content.contains("sources: [\"a.md\", \"b.md\"]"));
    assert!(final_content.contains("same body"));
  }

  #[test]
  fn finalize_page_merge_preserves_locked_fields_and_unions_arrays() {
    let existing = page(
      "type: concept\ntitle: Attention Mechanism\ncreated: 2026-06-08\ntags: [transformers]\nsources: [\"attention.md\"]",
      "Old body",
    );
    let incoming = page(
      "type: concept\ntitle: Attention Mechanism\ncreated: 2026-06-09\ntags: [optimization]\nsources: [\"attention-optimizations.md\"]",
      "New body",
    );
    let PageMergePlan::NeedsProvider {
      existing_content,
      array_merged,
    } = prepare_page_merge(&incoming, Some(&existing))
    else {
      panic!("expected provider merge");
    };

    let merged = finalize_page_merge(
      &existing_content,
      &array_merged,
      Some(&page(
        "type: concept\ntitle: Renamed\ncreated: 2026-06-09\ntags: [optimization]\nsources: [\"attention-optimizations.md\"]",
        "Old body\n\nNew body",
      )),
      "2026-06-30",
    );

    assert!(merged.contains("title: Attention Mechanism"));
    assert!(merged.contains("created: 2026-06-08"));
    assert!(merged.contains("updated: 2026-06-30"));
    assert!(merged.contains("sources: [\"attention.md\", \"attention-optimizations.md\"]"));
    assert!(merged.contains("tags: [\"transformers\", \"optimization\"]"));
    assert!(merged.contains("Old body"));
    assert!(merged.contains("New body"));
  }

  #[test]
  fn finalize_page_merge_falls_back_when_llm_output_is_invalid_or_too_small() {
    let existing = page("type: concept\ntitle: Foo\nsources: [\"a.md\"]", &"old ".repeat(80));
    let incoming = page("type: concept\ntitle: Foo\nsources: [\"b.md\"]", &"new ".repeat(80));
    let PageMergePlan::NeedsProvider {
      existing_content,
      array_merged,
    } = prepare_page_merge(&incoming, Some(&existing))
    else {
      panic!("expected provider merge");
    };

    let missing_frontmatter =
      finalize_page_merge(&existing_content, &array_merged, Some("no frontmatter"), "2026-06-30");
    assert_eq!(missing_frontmatter, array_merged);

    let too_small = finalize_page_merge(
      &existing_content,
      &array_merged,
      Some(&page("type: concept\ntitle: Foo", "tiny")),
      "2026-06-30",
    );
    assert_eq!(too_small, array_merged);
  }

  #[test]
  fn build_page_merge_prompts_include_existing_and_incoming_versions() {
    let (system, user) =
      build_page_merge_prompts("old content", "new content", "doc-B.pdf");
    assert!(system.contains("merging two versions"));
    assert!(user.contains("old content"));
    assert!(user.contains("new content"));
    assert!(user.contains("doc-B.pdf"));
  }
}
