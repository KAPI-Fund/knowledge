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
