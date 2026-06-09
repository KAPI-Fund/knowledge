pub fn sanitize_ingested_file_content(content: &str) -> String {
  let stripped = strip_outer_code_fence(content);
  let stripped = strip_frontmatter_key_prefix(&stripped);
  let stripped = add_missing_opening_frontmatter_fence(&stripped);
  repair_wikilink_lists_in_frontmatter(&stripped)
}

fn strip_outer_code_fence(content: &str) -> String {
  let normalized = content.trim();
  for opener in ["```yaml\n", "```md\n", "```markdown\n", "```\n"] {
    if let Some(body) = normalized.strip_prefix(opener)
      && let Some(inner) = body.strip_suffix("\n```")
    {
      return inner.to_string();
    }
  }
  content.to_string()
}

fn strip_frontmatter_key_prefix(content: &str) -> String {
  if let Some(rest) = content.strip_prefix("frontmatter:\n")
    && rest.starts_with("---\n")
  {
    return rest.to_string();
  }
  content.to_string()
}

fn add_missing_opening_frontmatter_fence(content: &str) -> String {
  if content.trim_start().starts_with("---") {
    return content.to_string();
  }

  let trimmed = content.trim_start_matches('\n').trim_start_matches('\r');
  let mut lines = trimmed.lines();
  let Some(first) = lines.next() else {
    return content.to_string();
  };
  if !matches_frontmatter_key(first) {
    return content.to_string();
  }

  let search_lines = trimmed.lines().take(30).collect::<Vec<_>>();
  if search_lines.iter().skip(1).any(|line| line.trim() == "---") {
    format!("---\n{trimmed}")
  } else {
    content.to_string()
  }
}

fn repair_wikilink_lists_in_frontmatter(content: &str) -> String {
  let Some((frontmatter, body, newline)) = split_frontmatter(content) else {
    return content.to_string();
  };

  let repaired = frontmatter
    .lines()
    .map(|line| {
      let Some((key, value)) = line.split_once(':') else {
        return line.to_string();
      };
      let trimmed = value.trim();
      if !trimmed.contains("]], [[") {
        return line.to_string();
      }
      let items = trimmed
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(", ");
      format!("{}: [{items}]", key.trim())
    })
    .collect::<Vec<_>>()
    .join(newline);

  format!("---{newline}{repaired}{newline}---{newline}{body}")
}

fn matches_frontmatter_key(line: &str) -> bool {
  matches!(
    line.trim_start(),
    value if value.starts_with("type:")
      || value.starts_with("title:")
      || value.starts_with("created:")
      || value.starts_with("updated:")
      || value.starts_with("tags:")
      || value.starts_with("related:")
      || value.starts_with("sources:")
  )
}

fn split_frontmatter(content: &str) -> Option<(String, String, &'static str)> {
  let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };
  let lines = content.split(newline).collect::<Vec<_>>();
  if lines.first().copied() != Some("---") {
    return None;
  }
  let end_index = lines
    .iter()
    .enumerate()
    .skip(1)
    .find_map(|(index, line)| if *line == "---" { Some(index) } else { None })?;
  let frontmatter = lines[1..end_index].join(newline);
  let body = lines[end_index + 1..].join(newline);
  Some((frontmatter, body, newline))
}

#[cfg(test)]
mod tests {
  use super::sanitize_ingested_file_content;

  #[test]
  fn returns_clean_content_unchanged() {
    let input = "---\ntype: entity\ntitle: Foo\n---\n\n# Foo\n\nbody";
    assert_eq!(sanitize_ingested_file_content(input), input);
  }

  #[test]
  fn strips_yaml_wrapped_document() {
    let input = "```yaml\n---\ntype: entity\ntitle: Accumulibacter\n---\n\n# Body\n```";
    assert_eq!(
      sanitize_ingested_file_content(input),
      "---\ntype: entity\ntitle: Accumulibacter\n---\n\n# Body"
    );
  }

  #[test]
  fn strips_frontmatter_key_prefix() {
    let input = "frontmatter:\n---\ntype: entity\ntitle: LSTM\n---\n\n# Body";
    assert_eq!(
      sanitize_ingested_file_content(input),
      "---\ntype: entity\ntitle: LSTM\n---\n\n# Body"
    );
  }

  #[test]
  fn repairs_missing_opening_frontmatter_fence() {
    let input = "\n\ntype: entity\ntitle: \"Foo: Bar\"\nsources: [foo.pdf]\n---\n\n# Foo\n\nBody";
    assert_eq!(
      sanitize_ingested_file_content(input),
      "---\ntype: entity\ntitle: \"Foo: Bar\"\nsources: [foo.pdf]\n---\n\n# Foo\n\nBody"
    );
  }

  #[test]
  fn repairs_invalid_wikilink_list_inside_frontmatter() {
    let input = "---\ntype: entity\nrelated: [[a]], [[b]], [[c]]\n---\n\nbody";
    assert_eq!(
      sanitize_ingested_file_content(input),
      "---\ntype: entity\nrelated: [\"[[a]]\", \"[[b]]\", \"[[c]]\"]\n---\n\nbody"
    );
  }

  #[test]
  fn does_not_touch_body_wikilink_text() {
    let input = "---\ntype: x\n---\n\nrelated: [[a]], [[b]] in body prose";
    assert_eq!(sanitize_ingested_file_content(input), input);
  }
}
