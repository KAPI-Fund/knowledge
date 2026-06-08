use std::path::Path;

use unicode_normalization::UnicodeNormalization;

pub const RAW_SOURCES_PREFIX: &str = "raw/sources/";
const RAW_SOURCES_MARKER: &str = "/raw/sources/";
const MAX_SOURCE_SUMMARY_SLUG_LENGTH: usize = 120;
const FALLBACK_SOURCE_PART: &str = "source";

pub fn source_identity_from_reference(reference: &str) -> String {
  let normalized = reference.replace('\\', "/").trim().trim_matches('/').to_string();
  let lower = normalized.to_lowercase();
  if lower.starts_with(RAW_SOURCES_PREFIX) {
    return normalized[RAW_SOURCES_PREFIX.len()..].to_string();
  }
  if let Some(index) = lower.find(RAW_SOURCES_MARKER) {
    return normalized[index + RAW_SOURCES_MARKER.len()..].to_string();
  }
  if !looks_like_absolute_path(&normalized) && normalized.contains('/') {
    return normalized;
  }
  Path::new(&normalized)
    .file_name()
    .and_then(|value| value.to_str())
    .unwrap_or(reference)
    .to_string()
}

pub fn source_reference_path(source_identity: &str) -> String {
  format!("raw/sources/{}", normalize_source_identity(source_identity))
}

pub fn source_file_name(source_identity: &str) -> String {
  Path::new(source_identity)
    .file_name()
    .and_then(|value| value.to_str())
    .unwrap_or(source_identity)
    .to_string()
}

pub fn source_file_stem(source_identity: &str) -> String {
  Path::new(source_identity)
    .file_stem()
    .and_then(|value| value.to_str())
    .unwrap_or(source_identity)
    .to_string()
}

pub fn source_summary_slug_from_identity(source_identity: &str) -> String {
  let normalized = normalize_source_identity(source_identity);
  let without_extension = if let Some((parent, file_name)) = normalized.rsplit_once('/') {
    format!("{parent}/{}", strip_source_extension(file_name))
  } else {
    strip_source_extension(&normalized).to_string()
  };
  let parts = without_extension
    .split('/')
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>();

  if parts.len() <= 1 {
    return parts.first().copied().unwrap_or(FALLBACK_SOURCE_PART).to_string();
  }

  let hash = stable_slug_hash(&normalized);
  let prefix = parts
    .iter()
    .map(|part| {
      let (readable, structural_length) = readable_slug_part(part);
      format!("{structural_length}-{readable}")
    })
    .collect::<Vec<_>>()
    .join("--");

  let full_slug = format!("{prefix}--{hash}");
  if full_slug.chars().count() <= MAX_SOURCE_SUMMARY_SLUG_LENGTH {
    return full_slug;
  }

  let readable_limit = MAX_SOURCE_SUMMARY_SLUG_LENGTH.saturating_sub(hash.chars().count() + 2);
  let readable_prefix = prefix
    .chars()
    .take(readable_limit)
    .collect::<String>()
    .trim_end_matches('-')
    .to_string();
  format!(
    "{}--{hash}",
    if readable_prefix.is_empty() {
      FALLBACK_SOURCE_PART
    } else {
      readable_prefix.as_str()
    }
  )
}

pub fn source_summary_path(source_identity: &str) -> String {
  format!(
    "wiki/sources/{}.md",
    source_summary_slug_from_identity(source_identity)
  )
}

pub fn source_checkpoint_stem(source_identity: &str) -> String {
  source_summary_slug_from_identity(source_identity)
}

fn normalize_source_identity(source_identity: &str) -> String {
  source_identity.replace('\\', "/").trim_matches('/').to_string()
}

fn looks_like_absolute_path(path: &str) -> bool {
  if path.is_empty() {
    return false;
  }
  if Path::new(path).is_absolute() {
    return true;
  }
  let bytes = path.as_bytes();
  if bytes.len() >= 3
    && bytes[1] == b':'
    && bytes[0].is_ascii_alphabetic()
    && matches!(bytes[2], b'/' | b'\\')
  {
    return true;
  }

  path.starts_with("//") || path.starts_with(r"\\")
}

fn strip_source_extension(file_name: &str) -> &str {
  match file_name.rsplit_once('.') {
    Some((prefix, _)) if !prefix.is_empty() => prefix,
    _ => file_name,
  }
}

fn readable_slug_part(part: &str) -> (String, usize) {
  let normalized = part.nfkc().collect::<String>();
  let trimmed = normalized.trim();
  let mut structural = String::new();
  let mut last_was_dash = false;

  for ch in trimmed.chars() {
    if ch.is_whitespace() {
      if !structural.is_empty() && !last_was_dash {
        structural.push('-');
        last_was_dash = true;
      }
      continue;
    }

    for lower in ch.to_lowercase() {
      if lower.is_alphanumeric() || lower == '-' {
        structural.push(lower);
        last_was_dash = lower == '-';
      }
    }
  }

  let structural = structural.trim_matches('-').to_string();
  let readable = collapse_repeated_hyphens(&structural);
  let readable = if readable.is_empty() {
    FALLBACK_SOURCE_PART.to_string()
  } else {
    readable
  };
  let structural_length = if structural.is_empty() {
    FALLBACK_SOURCE_PART.chars().count()
  } else {
    structural.chars().count()
  };
  (readable, structural_length.max(1))
}

fn collapse_repeated_hyphens(value: &str) -> String {
  let mut output = String::new();
  let mut last_was_dash = false;
  for ch in value.chars() {
    if ch == '-' {
      if !last_was_dash {
        output.push(ch);
      }
      last_was_dash = true;
    } else {
      output.push(ch);
      last_was_dash = false;
    }
  }
  output
}

fn stable_slug_hash(value: &str) -> String {
  let mut hash: u32 = 0x811c9dc5;
  for byte in value.as_bytes() {
    hash ^= u32::from(*byte);
    hash = hash.wrapping_mul(0x01000193);
  }
  radix36(hash)
}

fn radix36(mut value: u32) -> String {
  if value == 0 {
    return "0".to_string();
  }

  let mut output = Vec::new();
  while value > 0 {
    let digit = (value % 36) as u8;
    output.push(match digit {
      0..=9 => (b'0' + digit) as char,
      _ => (b'a' + (digit - 10)) as char,
    });
    value /= 36;
  }
  output.iter().rev().collect()
}
