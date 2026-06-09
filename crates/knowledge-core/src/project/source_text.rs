use std::fs;
use std::io::Read;
use std::path::Path;

use calamine::{open_workbook_auto, Data, Reader};
use office_oxide::Document;
use thiserror::Error;

use crate::project::multimodal::extract_pdf_text as extract_pdf_text_multimodal;

const OFFICE_EXTS: &[&str] = &["doc", "docx", "pptx", "xls", "xlsx", "odt", "ods", "odp"];

#[derive(Debug, Error)]
pub enum SourceTextError {
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[error("{0}")]
  Parse(String),
  #[error("unsupported source format: {0}")]
  UnsupportedFormat(String),
}

pub fn read_source_text(path: &Path) -> Result<String, SourceTextError> {
  let extension = path
    .extension()
    .and_then(|value| value.to_str())
    .unwrap_or_default()
    .to_ascii_lowercase();

  if extension == "pdf" {
    return extract_pdf_text_multimodal(path).map_err(SourceTextError::Parse);
  }

  if OFFICE_EXTS.contains(&extension.as_str()) {
    return extract_office_text(path, &extension);
  }

  fs::read_to_string(path).map_err(SourceTextError::Io)
}

fn extract_office_text(path: &Path, extension: &str) -> Result<String, SourceTextError> {
  if matches!(extension, "xlsx" | "xls" | "ods") {
    return extract_spreadsheet(path);
  }

  if extension == "docx" {
    return extract_docx_with_library(path);
  }

  if extension == "doc" {
    return extract_doc_with_office_oxide(path);
  }

  let file = fs::File::open(path)?;
  let mut archive = zip::ZipArchive::new(file).map_err(|error| {
    SourceTextError::Parse(format!(
      "failed to read zip archive '{}': {error}",
      path.display()
    ))
  })?;

  match extension {
    "pptx" => extract_pptx_markdown(&mut archive),
    "odt" | "odp" => extract_odf_text(&mut archive),
    other => Err(SourceTextError::UnsupportedFormat(other.to_string())),
  }
}

fn extract_doc_with_office_oxide(path: &Path) -> Result<String, SourceTextError> {
  let document = Document::open(path).map_err(|error| {
    SourceTextError::Parse(format!("failed to parse DOC '{}': {error}", path.display()))
  })?;
  let markdown = document.to_markdown();
  let text = if markdown.trim().is_empty() {
    document.plain_text()
  } else {
    markdown
  };

  if text.trim().is_empty() {
    Ok("[Document: no extractable text found in .doc file]".to_string())
  } else {
    Ok(text)
  }
}

fn extract_docx_with_library(path: &Path) -> Result<String, SourceTextError> {
  let bytes = fs::read(path)?;
  let document = match docx_rs::read_docx(&bytes) {
    Ok(document) => document,
    Err(_) => {
      let file = fs::File::open(path)?;
      let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        SourceTextError::Parse(format!(
          "failed to read DOCX archive '{}': {error}",
          path.display()
        ))
      })?;
      return extract_docx_markdown(&mut archive);
    }
  };

  let mut result = String::new();

  for child in document.document.children {
    match child {
      docx_rs::DocumentChild::Paragraph(paragraph) => {
        let mut paragraph_text = String::new();
        let mut is_heading = false;
        let mut heading_level: u8 = 1;

        if let Some(style) = &paragraph.property.style {
          let style_value = &style.val;
          if style_value.contains("Heading") || style_value.contains("heading") {
            is_heading = true;
            for ch in style_value.chars() {
              if ch.is_ascii_digit() {
                heading_level = ch.to_digit(10).unwrap_or(1) as u8;
                break;
              }
            }
          }
        }

        let is_list = paragraph.property.numbering_property.is_some();

        for child in &paragraph.children {
          if let docx_rs::ParagraphChild::Run(run) = child {
            let is_bold = run.run_property.bold.is_some();
            let is_italic = run.run_property.italic.is_some();

            for run_child in &run.children {
              if let docx_rs::RunChild::Text(text) = run_child {
                let value = &text.text;
                if is_bold && is_italic {
                  paragraph_text.push_str(&format!("***{value}***"));
                } else if is_bold {
                  paragraph_text.push_str(&format!("**{value}**"));
                } else if is_italic {
                  paragraph_text.push_str(&format!("*{value}*"));
                } else {
                  paragraph_text.push_str(value);
                }
              }
            }
          }
        }

        let text = paragraph_text.trim().to_string();
        if text.is_empty() {
          continue;
        }

        if is_heading {
          let prefix = "#".repeat(heading_level as usize);
          result.push_str(&format!("{prefix} {text}\n\n"));
        } else if is_list {
          result.push_str(&format!("- {text}\n"));
        } else {
          result.push_str(&text);
          result.push_str("\n\n");
        }
      }
      docx_rs::DocumentChild::Table(table) => {
        let mut rows: Vec<Vec<String>> = Vec::new();
        for row in &table.rows {
          let docx_rs::TableChild::TableRow(table_row) = row;
          let mut cells: Vec<String> = Vec::new();
          for cell in &table_row.cells {
            let docx_rs::TableRowChild::TableCell(table_cell) = cell;
            let mut cell_text = String::new();
            for child in &table_cell.children {
              if let docx_rs::TableCellContent::Paragraph(paragraph) = child {
                for paragraph_child in &paragraph.children {
                  if let docx_rs::ParagraphChild::Run(run) = paragraph_child {
                    for run_child in &run.children {
                      if let docx_rs::RunChild::Text(text) = run_child {
                        cell_text.push_str(&text.text);
                      }
                    }
                  }
                }
              }
            }
            cells.push(cell_text.trim().replace('|', "\\|"));
          }
          rows.push(cells);
        }
        if !rows.is_empty() {
          let max_columns = rows.iter().map(|row| row.len()).max().unwrap_or(0);
          for (index, row) in rows.iter().enumerate() {
            let mut padded = row.clone();
            padded.resize(max_columns, String::new());
            result.push_str("| ");
            result.push_str(&padded.join(" | "));
            result.push_str(" |\n");
            if index == 0 {
              result.push('|');
              for _ in 0..max_columns {
                result.push_str(" --- |");
              }
              result.push('\n');
            }
          }
          result.push('\n');
        }
      }
      _ => {}
    }
  }

  if result.trim().is_empty() {
    let file = fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
      SourceTextError::Parse(format!(
        "failed to read DOCX archive '{}': {error}",
        path.display()
      ))
    })?;
    extract_docx_markdown(&mut archive)
  } else {
    Ok(result)
  }
}

fn read_zip_file(archive: &mut zip::ZipArchive<fs::File>, name: &str) -> Option<String> {
  let mut file = archive.by_name(name).ok()?;
  let mut content = String::new();
  file.read_to_string(&mut content).ok()?;
  Some(content)
}

fn decode_xml_entities(text: &str) -> String {
  text
    .replace("&amp;", "&")
    .replace("&lt;", "<")
    .replace("&gt;", ">")
    .replace("&quot;", "\"")
    .replace("&apos;", "'")
    .replace("&#10;", "\n")
    .replace("&#13;", "")
}

fn extract_docx_markdown(
  archive: &mut zip::ZipArchive<fs::File>,
) -> Result<String, SourceTextError> {
  let xml = read_zip_file(archive, "word/document.xml")
    .ok_or_else(|| SourceTextError::Parse("no document.xml found".to_string()))?;

  let mut result = String::new();
  let mut index = 0usize;
  let chars = xml.chars().collect::<Vec<_>>();
  let length = chars.len();

  let mut paragraph_text = String::new();
  let mut is_heading = false;
  let mut heading_level: u8 = 1;
  let mut is_bold = false;
  let mut is_italic = false;
  let mut in_table = false;
  let mut table_row: Vec<String> = Vec::new();
  let mut table_cell_text = String::new();
  let mut in_cell = false;
  let mut is_first_table_row = true;
  let mut in_list_item = false;

  while index < length {
    if chars[index] == '<' {
      index += 1;
      let is_closing = index < length && chars[index] == '/';
      if is_closing {
        index += 1;
      }

      let mut tag_name = String::new();
      while index < length && chars[index] != '>' && chars[index] != ' ' && chars[index] != '/' {
        tag_name.push(chars[index]);
        index += 1;
      }

      let mut tag_content = String::new();
      while index < length && chars[index] != '>' {
        tag_content.push(chars[index]);
        index += 1;
      }
      if index < length {
        index += 1;
      }

      match tag_name.as_str() {
        "w:p" if !is_closing => {
          paragraph_text.clear();
          is_heading = false;
          in_list_item = false;
        }
        "w:p" if is_closing => {
          let text = paragraph_text.trim().to_string();
          if !text.is_empty() {
            if in_table && in_cell {
              table_cell_text = text;
            } else if is_heading {
              let prefix = "#".repeat(heading_level as usize);
              result.push_str(&format!("{prefix} {text}\n\n"));
            } else if in_list_item {
              result.push_str(&format!("- {text}\n"));
            } else {
              result.push_str(&text);
              result.push_str("\n\n");
            }
          }
          paragraph_text.clear();
        }
        "w:pStyle" if !is_closing => {
          if tag_content.contains("Heading") || tag_content.contains("heading") {
            is_heading = true;
            if let Some(position) = tag_content.find("Heading") {
              let suffix = &tag_content[position + 7..];
              if let Some(ch) = suffix.chars().next()
                && ch.is_ascii_digit()
              {
                heading_level = ch.to_digit(10).unwrap_or(1) as u8;
              }
            }
          }
          if tag_content.contains("ListParagraph") || tag_content.contains("listParagraph") {
            in_list_item = true;
          }
        }
        "w:b"
          if !is_closing
            && !tag_content.contains("w:val=\"0\"")
            && !tag_content.contains("w:val=\"false\"") =>
        {
          is_bold = true;
        }
        "w:i"
          if !is_closing
            && !tag_content.contains("w:val=\"0\"")
            && !tag_content.contains("w:val=\"false\"") =>
        {
          is_italic = true;
        }
        "w:r" if is_closing => {
          is_bold = false;
          is_italic = false;
        }
        "w:t" if !is_closing => {
          let mut text = String::new();
          while index < length {
            if chars[index] == '<' {
              break;
            }
            text.push(chars[index]);
            index += 1;
          }
          let decoded = decode_xml_entities(&text);
          if is_bold && is_italic {
            paragraph_text.push_str(&format!("***{decoded}***"));
          } else if is_bold {
            paragraph_text.push_str(&format!("**{decoded}**"));
          } else if is_italic {
            paragraph_text.push_str(&format!("*{decoded}*"));
          } else {
            paragraph_text.push_str(&decoded);
          }
        }
        "w:tbl" if !is_closing => {
          in_table = true;
          is_first_table_row = true;
        }
        "w:tbl" if is_closing => {
          in_table = false;
          result.push('\n');
        }
        "w:tr" if !is_closing => {
          table_row.clear();
        }
        "w:tr" if is_closing => {
          if !table_row.is_empty() {
            result.push_str("| ");
            result.push_str(&table_row.join(" | "));
            result.push_str(" |\n");
            if is_first_table_row {
              result.push('|');
              for _ in &table_row {
                result.push_str(" --- |");
              }
              result.push('\n');
              is_first_table_row = false;
            }
          }
        }
        "w:tc" if !is_closing => {
          in_cell = true;
          table_cell_text.clear();
        }
        "w:tc" if is_closing => {
          table_row.push(table_cell_text.trim().to_string());
          in_cell = false;
          table_cell_text.clear();
        }
        _ => {}
      }
    } else {
      index += 1;
    }
  }

  if result.trim().is_empty() {
    Ok("[Could not extract structured text from DOCX]".to_string())
  } else {
    Ok(result)
  }
}

fn extract_pptx_markdown(
  archive: &mut zip::ZipArchive<fs::File>,
) -> Result<String, SourceTextError> {
  let mut slide_names = (0..archive.len())
    .filter_map(|index| archive.by_index(index).ok().map(|file| file.name().to_string()))
    .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
    .collect::<Vec<_>>();

  slide_names.sort_by(|left, right| {
    let left_number = left
      .trim_start_matches("ppt/slides/slide")
      .trim_end_matches(".xml")
      .parse::<u32>()
      .unwrap_or(0);
    let right_number = right
      .trim_start_matches("ppt/slides/slide")
      .trim_end_matches(".xml")
      .parse::<u32>()
      .unwrap_or(0);
    left_number.cmp(&right_number)
  });

  let mut result = String::new();

  for (index, slide_name) in slide_names.iter().enumerate() {
    let Some(xml) = read_zip_file(archive, slide_name) else {
      continue;
    };

    result.push_str(&format!("## Slide {}\n\n", index + 1));

    let mut paragraphs = Vec::new();
    for paragraph_part in xml.split("<a:p") {
      let mut paragraph_text = String::new();
      for text_part in paragraph_part.split("<a:t") {
        if let Some(close_position) = text_part.find("</a:t>")
          && let Some(open_position) = text_part.find('>')
          && open_position < close_position
        {
          let text = &text_part[open_position + 1..close_position];
          paragraph_text.push_str(&decode_xml_entities(text));
        }
      }
      let trimmed = paragraph_text.trim().to_string();
      if !trimmed.is_empty() {
        paragraphs.push(trimmed);
      }
    }

    if let Some(title) = paragraphs.first() {
      result.push_str(&format!("**{title}**\n\n"));
      for paragraph in paragraphs.iter().skip(1) {
        result.push_str(&format!("- {paragraph}\n"));
      }
    }
    result.push('\n');
  }

  if result.trim().is_empty() {
    Ok("[Could not extract text from PPTX]".to_string())
  } else {
    Ok(result)
  }
}

fn extract_spreadsheet(path: &Path) -> Result<String, SourceTextError> {
  let mut workbook = open_workbook_auto(path).map_err(|error| {
    SourceTextError::Parse(format!(
      "failed to open spreadsheet '{}': {error}",
      path.display()
    ))
  })?;

  let mut result = String::new();
  let sheet_names = workbook.sheet_names().to_vec();

  for sheet_name in &sheet_names {
    if let Ok(range) = workbook.worksheet_range(sheet_name) {
      if range.is_empty() {
        continue;
      }

      if sheet_names.len() > 1 {
        result.push_str(&format!("## {sheet_name}\n\n"));
      }

      let mut rows = Vec::new();
      let mut max_columns = 0usize;
      for row in range.rows() {
        let cells = row
          .iter()
          .map(|cell| match cell {
            Data::Empty => String::new(),
            Data::String(value) => value.clone(),
            Data::Float(value) => {
              if *value == (*value as i64) as f64 {
                format!("{}", *value as i64)
              } else {
                format!("{value:.2}")
              }
            }
            Data::Int(value) => value.to_string(),
            Data::Bool(value) => value.to_string(),
            Data::DateTime(value) => format!("{value}"),
            Data::DateTimeIso(value) => value.clone(),
            Data::DurationIso(value) => value.clone(),
            Data::Error(value) => format!("ERR:{value:?}"),
          })
          .collect::<Vec<_>>();

        max_columns = max_columns.max(cells.len());
        rows.push(cells);
      }

      if rows.is_empty() || max_columns == 0 {
        continue;
      }

      for (index, row) in rows.iter().enumerate() {
        let mut padded = row.clone();
        padded.resize(max_columns, String::new());
        let escaped = padded
          .iter()
          .map(|value| value.replace('|', "\\|"))
          .collect::<Vec<_>>();
        result.push_str("| ");
        result.push_str(&escaped.join(" | "));
        result.push_str(" |\n");

        if index == 0 {
          result.push('|');
          for _ in 0..max_columns {
            result.push_str(" --- |");
          }
          result.push('\n');
        }
      }
      result.push('\n');
    }
  }

  if result.trim().is_empty() {
    Ok("[Could not extract data from spreadsheet]".to_string())
  } else {
    Ok(result)
  }
}

fn extract_odf_text(
  archive: &mut zip::ZipArchive<fs::File>,
) -> Result<String, SourceTextError> {
  let xml = read_zip_file(archive, "content.xml")
    .ok_or_else(|| SourceTextError::Parse("no content.xml found".to_string()))?;

  let mut result = String::new();
  let mut in_tag = false;
  for ch in xml.chars() {
    match ch {
      '<' => in_tag = true,
      '>' => {
        in_tag = false;
        result.push(' ');
      }
      _ if !in_tag => result.push(ch),
      _ => {}
    }
  }

  let cleaned = decode_xml_entities(&result);
  let lines = cleaned
    .lines()
    .map(str::trim)
    .filter(|line| !line.is_empty())
    .collect::<Vec<_>>();

  if lines.is_empty() {
    Ok("[Could not extract text from OpenDocument file]".to_string())
  } else {
    Ok(lines.join("\n\n"))
  }
}

#[cfg(test)]
mod tests {
  use std::io::Write;

  use tempfile::tempdir;
  use zip::write::FileOptions;
  use zip::ZipWriter;

  use super::read_source_text;

  #[test]
  fn read_source_text_extracts_minimal_docx_text() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("attention.docx");
    std::fs::write(&path, build_minimal_docx("Attention from DOCX.")).unwrap();

    let text = read_source_text(&path).unwrap();
    assert!(text.contains("Attention from DOCX."), "{text}");
  }

  #[test]
  fn read_source_text_routes_pdf_to_multimodal_path() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("paper.pdf");
    std::fs::write(&path, b"%PDF-1.4\n").unwrap();

    let error = read_source_text(&path).unwrap_err();
    match error {
      super::SourceTextError::Parse(message) => {
        assert!(message.contains("Failed to open PDF") || message.contains("Pdfium"));
      }
      other => panic!("unexpected error: {other:?}"),
    }
  }

  fn build_minimal_docx(text: &str) -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let options: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip
      .write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
      )
      .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip
      .write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
      )
      .unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip
      .write_all(
        format!(
          r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>{text}</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#
        )
        .as_bytes(),
      )
      .unwrap();

    zip.finish().unwrap().into_inner()
  }
}
