use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct ExtractOptions {
  pub min_width: u32,
  pub min_height: u32,
  pub max_images: usize,
}

impl Default for ExtractOptions {
  fn default() -> Self {
    Self {
      min_width: 100,
      min_height: 100,
      max_images: 500,
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedImage {
  pub index: u32,
  pub mime_type: String,
  pub page: Option<u32>,
  pub width: u32,
  pub height: u32,
  pub data_base64: String,
  pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedImage {
  pub index: u32,
  pub mime_type: String,
  pub page: Option<u32>,
  pub width: u32,
  pub height: u32,
  pub rel_path: String,
  pub abs_path: String,
  pub sha256: String,
}

static PDFIUM: OnceLock<Result<pdfium_render::prelude::Pdfium, String>> = OnceLock::new();
static PDFIUM_LOCK: Mutex<()> = Mutex::new(());

pub fn extract_pdf_text(path: &Path) -> Result<String, String> {
  use pdfium_render::prelude::*;

  let _guard = lock_pdfium();
  let pdfium = pdfium()?;
  let doc = pdfium.load_pdf_from_file(path, None).map_err(|error| match error {
    PdfiumError::PdfiumLibraryInternalError(PdfiumInternalError::PasswordError) => {
      format!("PDF is password protected and cannot be read: '{}'", path.display())
    }
    _ => format!("Failed to open PDF '{}': {error}", path.display()),
  })?;

  let mut out = String::new();
  for (index, page) in doc.pages().iter().enumerate() {
    if !out.is_empty() {
      out.push_str("\n\n");
    }
    out.push_str(&format!("## Page {}\n\n", index + 1));
    let text = page
      .text()
      .map_err(|error| format!("Page {} text extraction failed: {error}", index + 1))?
      .all();
    out.push_str(&text);
    out.push('\n');
  }

  if out.trim().is_empty() {
    Ok("[Could not extract text from PDF]".to_string())
  } else {
    Ok(out)
  }
}

pub fn extract_pdf_images(
  path: &Path,
  options: &ExtractOptions,
) -> Result<Vec<ExtractedImage>, String> {
  use pdfium_render::prelude::*;

  let _guard = lock_pdfium();
  let pdfium = pdfium()?;
  let doc = pdfium
    .load_pdf_from_file(path, None)
    .map_err(|error| format!("Failed to open PDF '{}': {error}", path.display()))?;

  let mut images = Vec::new();
  let mut index: u32 = 0;

  'pages: for (page_index, page) in doc.pages().iter().enumerate() {
    for object in page.objects().iter() {
      let Some(image) = object.as_image_object() else {
        continue;
      };
      let dyn_img = match image.get_raw_image() {
        Ok(image) => image,
        Err(error) => {
          eprintln!(
            "[multimodal] page {} image read failed: {error}",
            page_index + 1
          );
          continue;
        }
      };

      let width = dyn_img.width();
      let height = dyn_img.height();
      if width < options.min_width || height < options.min_height {
        continue;
      }

      let mut png_bytes = Vec::new();
      if let Err(error) =
        dyn_img.write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
      {
        eprintln!(
          "[multimodal] page {} PNG encode failed: {error}",
          page_index + 1
        );
        continue;
      }

      index += 1;
      images.push(ExtractedImage {
        index,
        mime_type: "image/png".to_string(),
        page: Some((page_index + 1) as u32),
        width,
        height,
        data_base64: BASE64.encode(&png_bytes),
        sha256: sha256_hex(&png_bytes),
      });

      if images.len() >= options.max_images {
        break 'pages;
      }
    }
  }

  Ok(images)
}

pub fn extract_office_images(
  path: &Path,
  options: &ExtractOptions,
) -> Result<Vec<ExtractedImage>, String> {
  let file = fs::File::open(path).map_err(|error| format!("Failed to open '{}': {error}", path.display()))?;
  let mut archive =
    zip::ZipArchive::new(file).map_err(|error| format!("Failed to read zip '{}': {error}", path.display()))?;

  let is_pptx = archive
    .file_names()
    .any(|name| name == "ppt/presentation.xml" || name.starts_with("ppt/slides/slide"));
  let slide_map = if is_pptx {
    build_pptx_media_slide_map(&mut archive)
  } else {
    HashMap::new()
  };

  let media_indices = (0..archive.len())
    .filter(|index| {
      archive
        .by_index_raw(*index)
        .ok()
        .map(|file| is_media_path(file.name()))
        .unwrap_or(false)
    })
    .collect::<Vec<_>>();

  let mut images = Vec::new();
  let mut index: u32 = 0;

  for archive_index in media_indices {
    let mut entry = match archive.by_index(archive_index) {
      Ok(entry) => entry,
      Err(error) => {
        eprintln!("[multimodal] zip entry read failed: {error}");
        continue;
      }
    };

    let entry_name = entry.name().to_string();
    let Some(mime_type) = guess_mime_from_name(&entry_name) else {
      continue;
    };

    let mut bytes = Vec::with_capacity(entry.size() as usize);
    if let Err(error) = entry.read_to_end(&mut bytes) {
      eprintln!("[multimodal] read '{entry_name}' failed: {error}");
      continue;
    }

    let (width, height) = match image::load_from_memory(&bytes) {
      Ok(image) => (image.width(), image.height()),
      Err(error) => {
        eprintln!("[multimodal] decode '{entry_name}' failed: {error}");
        continue;
      }
    };

    if width < options.min_width || height < options.min_height {
      continue;
    }

    index += 1;
    images.push(ExtractedImage {
      index,
      mime_type,
      page: slide_map.get(&entry_name).copied().flatten(),
      width,
      height,
      data_base64: BASE64.encode(&bytes),
      sha256: sha256_hex(&bytes),
    });

    if images.len() >= options.max_images {
      break;
    }
  }

  Ok(images)
}

pub fn extract_and_save_source_images(
  path: &Path,
  dest_dir: &Path,
  rel_to: &Path,
  options: &ExtractOptions,
) -> Result<Vec<SavedImage>, String> {
  let extension = path
    .extension()
    .and_then(|value| value.to_str())
    .unwrap_or_default()
    .to_ascii_lowercase();

  let extracted = match extension.as_str() {
    "pdf" => extract_pdf_images(path, options)?,
    "pptx" | "docx" | "xlsx" | "xls" | "ods" => extract_office_images(path, options)?,
    _ => return Ok(Vec::new()),
  };

  save_extracted_images(&extracted, dest_dir, rel_to)
}

pub fn save_extracted_images(
  images: &[ExtractedImage],
  dest_dir: &Path,
  rel_to: &Path,
) -> Result<Vec<SavedImage>, String> {
  if !dest_dir.exists() {
    fs::create_dir_all(dest_dir)
      .map_err(|error| format!("create_dir_all '{}': {error}", dest_dir.display()))?;
  }

  let mut out = Vec::new();
  for image in images {
    let bytes = BASE64
      .decode(&image.data_base64)
      .map_err(|error| format!("failed to decode image bytes: {error}"))?;
    let file_name = format!("img-{}.{}", image.index, ext_for_mime(&image.mime_type));
    let abs = dest_dir.join(&file_name);
    fs::write(&abs, &bytes).map_err(|error| format!("write '{}': {error}", abs.display()))?;
    let rel = abs
      .strip_prefix(rel_to)
      .map(|path| path.to_string_lossy().replace('\\', "/"))
      .unwrap_or_else(|_| file_name.clone());

    out.push(SavedImage {
      index: image.index,
      mime_type: image.mime_type.clone(),
      page: image.page,
      width: image.width,
      height: image.height,
      rel_path: rel,
      abs_path: abs.to_string_lossy().to_string(),
      sha256: image.sha256.clone(),
    });
  }

  Ok(out)
}

pub fn build_image_markdown_section(
  images: &[SavedImage],
  captions_by_sha: Option<&HashMap<String, String>>,
) -> String {
  if images.is_empty() {
    return String::new();
  }

  let mut by_page: BTreeMap<String, Vec<&SavedImage>> = BTreeMap::new();
  for image in images {
    let key = image
      .page
      .map(|page| format!("Page {page}"))
      .unwrap_or_else(|| "Document".to_string());
    by_page.entry(key).or_default().push(image);
  }

  let mut lines = vec!["".to_string(), "".to_string(), "## Embedded Images".to_string(), "".to_string()];
  let mut page_keys = by_page.keys().cloned().collect::<Vec<_>>();
  page_keys.sort_by(|left, right| match (left.as_str(), right.as_str()) {
    ("Document", "Document") => std::cmp::Ordering::Equal,
    ("Document", _) => std::cmp::Ordering::Greater,
    (_, "Document") => std::cmp::Ordering::Less,
    _ => page_number(left).cmp(&page_number(right)),
  });

  for page_key in page_keys {
    lines.push(format!("### {page_key}"));
    lines.push(String::new());
    for image in by_page.get(&page_key).into_iter().flatten() {
      let caption = captions_by_sha
        .and_then(|map| map.get(&image.sha256))
        .cloned()
        .unwrap_or_default();
      let caption = sanitize_caption(&caption);
      lines.push(format!("![{caption}]({})", image.rel_path));
    }
    lines.push(String::new());
  }

  lines.join("\n")
}

fn page_number(value: &str) -> u32 {
  value
    .chars()
    .filter(|ch| ch.is_ascii_digit())
    .collect::<String>()
    .parse::<u32>()
    .unwrap_or(0)
}

fn sanitize_caption(value: &str) -> String {
  value.replace(['\r', '\n'], " ").replace(']', ")").trim().to_string()
}

fn ext_for_mime(mime: &str) -> &'static str {
  match mime {
    "image/png" => "png",
    "image/jpeg" => "jpg",
    "image/gif" => "gif",
    "image/webp" => "webp",
    "image/bmp" => "bmp",
    "image/tiff" => "tiff",
    _ => "bin",
  }
}

fn is_media_path(name: &str) -> bool {
  let lower = name.to_ascii_lowercase();
  lower.starts_with("ppt/media/")
    || lower.starts_with("word/media/")
    || lower.starts_with("xl/media/")
}

fn guess_mime_from_name(name: &str) -> Option<String> {
  let ext = Path::new(name)
    .extension()
    .and_then(|value| value.to_str())?
    .to_ascii_lowercase();
  match ext.as_str() {
    "png" => Some("image/png".to_string()),
    "jpg" | "jpeg" => Some("image/jpeg".to_string()),
    "gif" => Some("image/gif".to_string()),
    "webp" => Some("image/webp".to_string()),
    "bmp" => Some("image/bmp".to_string()),
    "tif" | "tiff" => Some("image/tiff".to_string()),
    _ => None,
  }
}

fn sha256_hex(bytes: &[u8]) -> String {
  let mut hasher = Sha256::new();
  hasher.update(bytes);
  format!("{:x}", hasher.finalize())
}

fn build_pptx_media_slide_map(
  archive: &mut zip::ZipArchive<fs::File>,
) -> HashMap<String, Option<u32>> {
  let rels_paths = archive
    .file_names()
    .filter(|name| name.starts_with("ppt/slides/_rels/slide") && name.ends_with(".xml.rels"))
    .map(str::to_string)
    .collect::<Vec<_>>();

  let mut out = HashMap::new();
  for rels_path in rels_paths {
    let slide_num = rels_path
      .strip_prefix("ppt/slides/_rels/slide")
      .and_then(|value| value.strip_suffix(".xml.rels"))
      .and_then(|value| value.parse::<u32>().ok());

    let mut entry = match archive.by_name(&rels_path) {
      Ok(entry) => entry,
      Err(_) => continue,
    };
    let mut xml = String::new();
    if entry.read_to_string(&mut xml).is_err() {
      continue;
    }

    let mut search_from = 0;
    while let Some(position) = xml[search_from..].find("Target=\"") {
      let start = search_from + position + "Target=\"".len();
      let Some(end_rel) = xml[start..].find('"') else {
        break;
      };
      let end = start + end_rel;
      let target = &xml[start..end];
      search_from = end + 1;
      if let Some(stripped) = target.strip_prefix("../") {
        let canonical = format!("ppt/{stripped}");
        if is_media_path(&canonical) {
          out.insert(canonical, slide_num);
        }
      }
    }
  }

  out
}

fn lock_pdfium() -> std::sync::MutexGuard<'static, ()> {
  PDFIUM_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn pdfium() -> Result<&'static pdfium_render::prelude::Pdfium, String> {
  PDFIUM
    .get_or_init(|| {
      use pdfium_render::prelude::Pdfium;

      let candidates = pdfium_candidate_paths();
      for path in &candidates {
        if let Ok(bindings) = Pdfium::bind_to_library(path) {
          return Ok(Pdfium::new(bindings));
        }
      }

      Pdfium::bind_to_system_library()
        .map(Pdfium::new)
        .map_err(|error| {
          format!(
            "Failed to locate Pdfium library. Tried: {}. Last error: {error}",
            if candidates.is_empty() {
              "(no candidates)".to_string()
            } else {
              candidates
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(", ")
            }
          )
        })
    })
    .as_ref()
    .map_err(|error| error.clone())
}

fn pdfium_candidate_paths() -> Vec<PathBuf> {
  let mut paths = Vec::new();
  if let Ok(value) = std::env::var("PDFIUM_DYNAMIC_LIB_PATH") {
    paths.push(PathBuf::from(value));
  }

  if let Ok(cwd) = std::env::current_dir() {
    push_pdfium_candidates(&mut paths, &cwd);
    if let Some(parent) = cwd.parent() {
      push_pdfium_candidates(&mut paths, parent);
      if let Some(grand_parent) = parent.parent() {
        push_pdfium_candidates(&mut paths, grand_parent);
      }
    }
  }

  if let Ok(exe) = std::env::current_exe()
    && let Some(parent) = exe.parent()
  {
    push_pdfium_candidates(&mut paths, parent);
    if let Some(grand_parent) = parent.parent() {
      push_pdfium_candidates(&mut paths, grand_parent);
    }
  }

  paths
}

fn push_pdfium_candidates(paths: &mut Vec<PathBuf>, base: &Path) {
  #[cfg(target_os = "windows")]
  {
    paths.push(base.join("pdfium").join("pdfium.dll"));
    paths.push(base.join("pdfium").join("libpdfium.dll"));
    paths.push(base.join("pdfium.dll"));
    paths.push(base.join("libpdfium.dll"));
  }

  #[cfg(target_os = "macos")]
  {
    paths.push(base.join("pdfium").join("libpdfium.dylib"));
    paths.push(base.join("libpdfium.dylib"));
  }

  #[cfg(target_os = "linux")]
  {
    paths.push(base.join("pdfium").join("libpdfium.so"));
    paths.push(base.join("libpdfium.so"));
  }
}

#[cfg(test)]
mod tests {
  use std::io::Write;

  use tempfile::tempdir;
  use zip::write::FileOptions;
  use zip::ZipWriter;

  use super::*;

  #[test]
  fn extract_options_defaults_match_plan() {
    let options = ExtractOptions::default();
    assert_eq!(options.min_width, 100);
    assert_eq!(options.min_height, 100);
    assert_eq!(options.max_images, 500);
  }

  #[test]
  fn build_image_markdown_section_uses_captions_by_hash() {
    let images = vec![SavedImage {
      index: 1,
      mime_type: "image/png".to_string(),
      page: Some(2),
      width: 10,
      height: 10,
      rel_path: "media/slug/img-1.png".to_string(),
      abs_path: "/tmp/img-1.png".to_string(),
      sha256: "abc".to_string(),
    }];
    let mut captions = HashMap::new();
    captions.insert("abc".to_string(), "Figure caption".to_string());
    let section = build_image_markdown_section(&images, Some(&captions));
    assert!(section.contains("## Embedded Images"));
    assert!(section.contains("### Page 2"));
    assert!(section.contains("![Figure caption](media/slug/img-1.png)"));
  }

  #[test]
  fn extract_office_images_reads_embedded_docx_media() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("vision.docx");
    fs::write(&path, build_minimal_docx_with_image()).unwrap();

    let images = extract_office_images(&path, &ExtractOptions::default()).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].mime_type, "image/png");
    assert_eq!(images[0].page, None);
  }

  fn build_minimal_docx_with_image() -> Vec<u8> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(cursor);
    let options: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(
      br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(
      br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(
      br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>Document with a figure.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#,
    )
    .unwrap();

    zip.start_file("word/media/image1.png", options).unwrap();
    zip.write_all(&sample_png_bytes()).unwrap();

    zip.finish().unwrap().into_inner()
  }

  fn sample_png_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(128, 128, image::Rgba([255, 0, 0, 255]));
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
      .write_to(&mut cursor, image::ImageFormat::Png)
      .unwrap();
    cursor.into_inner()
  }
}
