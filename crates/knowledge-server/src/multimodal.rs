use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::providers::{
  OpenAiCompatibleProvider, ProviderContentBlock, ProviderError, ProviderMultimodalRequest,
};
use knowledge_core::project::multimodal::{build_image_markdown_section, SavedImage};

const IMAGE_CAPTION_PROMPT: &str = "Describe this image factually for a knowledge-base index. Include any visible text verbatim, chart axes and values, diagram structure, and key visual elements. Do not speculate or editorialize. Use 2 to 4 sentences. Output plain text only.";
const IMAGE_CAPTION_PROMPT_VERSION: &str = "v1";
const IMAGE_CAPTION_CACHE_TTL_SECS: u64 = 60 * 60 * 24 * 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionCacheEntry {
  pub caption: String,
}

pub fn image_caption_cache_key(hash: &str) -> String {
  format!("image-caption:{IMAGE_CAPTION_PROMPT_VERSION}:{hash}")
}

pub async fn caption_image(
  provider: &OpenAiCompatibleProvider,
  data_base64: &str,
  media_type: &str,
) -> Result<String, ProviderError> {
  let response = provider
    .complete_multimodal(ProviderMultimodalRequest {
      system_prompt: IMAGE_CAPTION_PROMPT.to_string(),
      content_blocks: vec![
        ProviderContentBlock::Text {
          text: "Caption this image.".to_string(),
        },
        ProviderContentBlock::Image {
          media_type: media_type.to_string(),
          data_base64: data_base64.to_string(),
        },
      ],
    })
    .await?;

  Ok(response.text.trim().to_string())
}

pub async fn load_cached_caption(
  cache: &crate::cache::CacheStore,
  hash: &str,
) -> anyhow::Result<Option<String>> {
  cache.get_string(&image_caption_cache_key(hash)).await
}

pub async fn save_cached_caption(
  cache: &crate::cache::CacheStore,
  hash: &str,
  caption: &str,
) -> anyhow::Result<()> {
  cache
    .set_string_ex(
      &image_caption_cache_key(hash),
      caption,
      IMAGE_CAPTION_CACHE_TTL_SECS,
    )
    .await
}

pub fn image_rel_path(source_slug: &str, index: u32, mime_type: &str) -> String {
  let ext = match mime_type {
    "image/png" => "png",
    "image/jpeg" => "jpg",
    "image/gif" => "gif",
    "image/webp" => "webp",
    "image/bmp" => "bmp",
    "image/tiff" => "tiff",
    _ => "bin",
  };
  format!("wiki/media/{source_slug}/img-{index}.{ext}")
}

pub fn caption_cache_key_path(project_root: &Path) -> PathBuf {
  project_root.join(".llm-wiki/image-caption-cache.json")
}

pub fn captions_by_sha_from_entries(entries: &[(String, String)]) -> HashMap<String, String> {
  entries
    .iter()
    .map(|(hash, caption)| (hash.clone(), caption.clone()))
    .collect()
}

pub fn inject_images_into_source_summary(
  root: &Path,
  summary_path: &str,
  source_identity: &str,
  summary_title: &str,
  images: &[SavedImage],
  captions_by_sha: &HashMap<String, String>,
) -> anyhow::Result<()> {
  if images.is_empty() {
    return Ok(());
  }

  let mut summary_images = images.to_vec();
  for image in &mut summary_images {
    if image.rel_path.starts_with("media/") {
      image.rel_path = format!("../{}", image.rel_path);
    }
  }

  let section = build_image_markdown_section(&summary_images, Some(captions_by_sha));
  if section.trim().is_empty() {
    return Ok(());
  }

  let path = root.join(summary_path);
  if let Some(parent) = path.parent() {
    fs::create_dir_all(parent)?;
  }

  let existing = fs::read_to_string(&path).unwrap_or_else(|_| {
    format!(
      "---\ntype: source\ntitle: {}\nsources: [\"{}\"]\n---\n\n# {}\n",
      summary_title, source_identity, summary_title
    )
  });

  let marker = "<!-- llm-wiki:embedded-images -->";
  let rendered_section = format!("\n\n{marker}\n{}\n{marker}\n", section.trim());
  let updated = if let Some(start) = existing.find(marker) {
    if let Some(end_rel) = existing[start + marker.len()..].find(marker) {
      let end = start + marker.len() + end_rel + marker.len();
      let mut value = String::new();
      value.push_str(existing[..start].trim_end());
      value.push_str(&rendered_section);
      value.push_str(existing[end..].trim_start());
      value
    } else {
      format!("{}{}", existing.trim_end(), rendered_section)
    }
  } else {
    format!("{}{}", existing.trim_end(), rendered_section)
  };

  fs::write(&path, updated)?;
  Ok(())
}
