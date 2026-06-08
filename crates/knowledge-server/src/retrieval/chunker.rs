#[derive(Debug, Clone)]
pub struct Chunk {
  pub index: u32,
  pub text: String,
  pub heading_path: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ChunkingOptions {
  pub target_chars: usize,
  pub overlap_chars: usize,
}

impl Default for ChunkingOptions {
  fn default() -> Self {
    Self {
      target_chars: 1_000,
      overlap_chars: 200,
    }
  }
}

pub fn chunk_markdown(content: &str, options: ChunkingOptions) -> Vec<Chunk> {
  let stripped = strip_frontmatter(content);
  if stripped.trim().is_empty() {
    return Vec::new();
  }

  let mut chunks = Vec::new();
  let mut next_index = 0u32;

  for section in split_sections(&stripped) {
    let section_chunks = chunk_section(&section.text, &section.heading_path, options, &mut next_index);
    chunks.extend(section_chunks);
  }

  chunks
}

#[derive(Debug, Clone)]
struct Section {
  heading_path: String,
  text: String,
}

fn strip_frontmatter(content: &str) -> String {
  if !(content.starts_with("---\n") || content.starts_with("---\r\n")) {
    return content.to_string();
  }

  let normalized = content.replace("\r\n", "\n");
  let mut lines = normalized.lines();
  if lines.next() != Some("---") {
    return content.to_string();
  }

  let mut offset = 4usize;
  for line in lines {
    offset += line.len() + 1;
    if line == "---" {
      return normalized[offset..].to_string();
    }
  }

  content.to_string()
}

fn split_sections(content: &str) -> Vec<Section> {
  let mut sections = Vec::new();
  let mut current_lines = Vec::new();
  let mut headings: Vec<(usize, String)> = Vec::new();
  let mut in_fence = false;

  for line in content.lines() {
    if line.starts_with("```") || line.starts_with("~~~") {
      in_fence = !in_fence;
      current_lines.push(line.to_string());
      continue;
    }

    if !in_fence
      && let Some((level, title)) = parse_heading(line)
    {
      flush_section(&mut sections, &mut current_lines, &headings);
      headings.retain(|(existing, _)| *existing < level);
      headings.push((level, title.to_string()));
    }

    current_lines.push(line.to_string());
  }

  flush_section(&mut sections, &mut current_lines, &headings);
  sections
}

fn flush_section(
  sections: &mut Vec<Section>,
  current_lines: &mut Vec<String>,
  headings: &[(usize, String)],
) {
  let text = current_lines.join("\n");
  if text.trim().is_empty() {
    current_lines.clear();
    return;
  }

  let heading_path = headings
    .iter()
    .map(|(level, title)| format!("{} {}", "#".repeat(*level), title))
    .collect::<Vec<_>>()
    .join(" > ");

  sections.push(Section { heading_path, text });
  current_lines.clear();
}

fn parse_heading(line: &str) -> Option<(usize, &str)> {
  let trimmed = line.trim();
  let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
  if hashes == 0 || hashes > 6 {
    return None;
  }
  let title = trimmed[hashes..].trim();
  if title.is_empty() {
    return None;
  }
  Some((hashes, title))
}

fn chunk_section(
  text: &str,
  heading_path: &str,
  options: ChunkingOptions,
  next_index: &mut u32,
) -> Vec<Chunk> {
  if text.len() <= options.target_chars {
    let chunk = Chunk {
      index: *next_index,
      text: text.trim().to_string(),
      heading_path: heading_path.to_string(),
    };
    *next_index += 1;
    return if chunk.text.is_empty() { Vec::new() } else { vec![chunk] };
  }

  let mut pieces = split_paragraphs(text);
  if pieces.len() == 1 {
    pieces = split_sentences(text, options.target_chars);
  }

  let mut output = Vec::new();
  let mut current = String::new();

  for piece in pieces {
    if current.is_empty() {
      current.push_str(piece.trim());
      continue;
    }

    let candidate = format!("{current}\n\n{}", piece.trim());
    if candidate.len() <= options.target_chars {
      current = candidate;
      continue;
    }

    push_chunk(&mut output, heading_path, next_index, &current);
    current = apply_overlap(&current, piece.trim(), options.overlap_chars);
  }

  if !current.trim().is_empty() {
    push_chunk(&mut output, heading_path, next_index, &current);
  }

  output
}

fn push_chunk(chunks: &mut Vec<Chunk>, heading_path: &str, next_index: &mut u32, text: &str) {
  let trimmed = text.trim();
  if trimmed.is_empty() {
    return;
  }

  chunks.push(Chunk {
    index: *next_index,
    text: trimmed.to_string(),
    heading_path: heading_path.to_string(),
  });
  *next_index += 1;
}

fn split_paragraphs(text: &str) -> Vec<String> {
  text
    .split("\n\n")
    .map(str::trim)
    .filter(|piece| !piece.is_empty())
    .map(str::to_string)
    .collect()
}

fn split_sentences(text: &str, target_chars: usize) -> Vec<String> {
  let mut pieces = Vec::new();
  let mut current = String::new();

  for sentence in text.split_inclusive(['.', '!', '?', '。', '！', '？']) {
    let trimmed = sentence.trim();
    if trimmed.is_empty() {
      continue;
    }

    if current.is_empty() {
      current.push_str(trimmed);
      continue;
    }

    let candidate = format!("{current} {trimmed}");
    if candidate.len() <= target_chars {
      current = candidate;
      continue;
    }

    pieces.push(current);
    current = trimmed.to_string();
  }

  if current.is_empty() {
    let mut hard = Vec::new();
    let chars = text.chars().collect::<Vec<_>>();
    for slice in chars.chunks(target_chars.max(1)) {
      hard.push(slice.iter().collect::<String>());
    }
    return hard;
  }

  pieces.push(current);
  pieces
}

fn apply_overlap(previous: &str, next_piece: &str, overlap_chars: usize) -> String {
  if overlap_chars == 0 || previous.is_empty() {
    return next_piece.to_string();
  }

  let mut tail = previous.chars().rev().take(overlap_chars).collect::<Vec<_>>();
  tail.reverse();
  let overlap = tail.into_iter().collect::<String>();
  format!("{overlap}\n\n{next_piece}")
}
