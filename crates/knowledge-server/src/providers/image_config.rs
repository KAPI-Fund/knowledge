use crate::app::state::AppState;
use crate::http::error::ApiError;
use crate::providers::OpenAiCompatibleProvider;

/// Resolved image-generation configuration (OpenAI-compatible endpoint).
#[derive(Debug, Clone)]
pub struct ImageConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub size: String,
    pub timeout_seconds: i64,
}

impl ImageConfig {
    pub fn provider(&self) -> OpenAiCompatibleProvider {
        OpenAiCompatibleProvider::new(
            self.base_url.clone(),
            self.api_key.clone(),
            self.model.clone(),
            self.timeout_seconds,
        )
    }
}

/// Pure: validate the raw image columns. Requires base_url + model; defaults the
/// size and timeout. Returns a bad_request error mirroring the old provider-
/// incomplete failure so callers surface a clear "configure the image provider".
pub fn image_config_from_row(
    base_url: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    size: Option<String>,
    timeout_seconds: Option<i64>,
) -> Result<ImageConfig, ApiError> {
    let base_url = base_url.unwrap_or_default();
    let model = model.unwrap_or_default();
    if base_url.trim().is_empty() || model.trim().is_empty() {
        return Err(ApiError::bad_request("image provider is not configured"));
    }
    let size = size.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| "1024x1024".to_string());
    Ok(ImageConfig {
        base_url,
        api_key: api_key.unwrap_or_default(),
        model,
        size,
        // Image generation is slow (multi-MB base64 payloads, tens of seconds to a
        // few minutes), so default the request timeout to 5 minutes.
        timeout_seconds: timeout_seconds.unwrap_or(300),
    })
}

pub async fn load_image_config(state: &AppState) -> Result<ImageConfig, ApiError> {
    let (base_url, api_key, model, size, timeout_seconds) =
        sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>)>(
            "SELECT image_base_url, image_api_key, image_model, image_size, image_timeout_seconds
             FROM system_settings WHERE id = 1",
        )
        .fetch_one(&state.pool)
        .await
        .map_err(ApiError::from)?;
    image_config_from_row(base_url, api_key, model, size, timeout_seconds)
}

const GPT_IMAGE_SIZES: &[&str] = &["1024x1024", "1024x1536", "1536x1024"];
const DALLE3_SIZES: &[&str] = &["1024x1024", "1024x1792", "1792x1024"];
const DALLE2_SIZES: &[&str] = &["256x256", "512x512", "1024x1024"];

/// The fixed size set a known OpenAI image model family accepts, or `None` for an
/// unrecognized model. Mirrors the admin UI's `allowedSizesForModel`, but returns
/// `None` (rather than a catch-all list) for unknown models so a custom endpoint's
/// bespoke size is left unrestricted server-side.
fn allowed_sizes_for_model(model: &str) -> Option<&'static [&'static str]> {
    let m = model.trim().to_lowercase();
    if m.starts_with("gpt-image") {
        Some(GPT_IMAGE_SIZES)
    } else if m.starts_with("dall-e-3") || m.starts_with("dalle-3") {
        Some(DALLE3_SIZES)
    } else if m.starts_with("dall-e-2") || m.starts_with("dalle-2") {
        Some(DALLE2_SIZES)
    } else {
        None
    }
}

/// Reject a size a known model family does not support (e.g. dall-e-3 + 512x512),
/// so an incompatible combination can't be persisted only to fail at canvas
/// runtime. Unknown models and blank sizes pass through (validated/defaulted
/// elsewhere).
pub fn validate_image_size(model: &str, size: &str) -> Result<(), ApiError> {
    let size = size.trim();
    if size.is_empty() {
        return Ok(());
    }
    if let Some(allowed) = allowed_sizes_for_model(model) {
        if !allowed.contains(&size) {
            return Err(ApiError::bad_request(format!(
                "image size {size:?} is not supported by model {:?}; allowed: {}",
                model.trim(),
                allowed.join(", ")
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_row_requires_base_url_and_model() {
        assert!(image_config_from_row(None, None, Some("m".into()), None, None).is_err());
        assert!(image_config_from_row(Some("u".into()), None, None, None, None).is_err());
        assert!(image_config_from_row(Some("".into()), None, Some("m".into()), None, None).is_err());
    }

    #[test]
    fn from_row_defaults_size_and_timeout() {
        let cfg = image_config_from_row(
            Some("https://api.example.com".into()),
            None,
            Some("gpt-image-1".into()),
            None,
            None,
        )
        .unwrap();
        assert_eq!(cfg.size, "1024x1024");
        assert_eq!(cfg.timeout_seconds, 300);
        assert_eq!(cfg.api_key, "");
    }

    #[test]
    fn from_row_keeps_explicit_size() {
        let cfg = image_config_from_row(
            Some("https://api.example.com".into()),
            Some("k".into()),
            Some("gpt-image-1".into()),
            Some("512x512".into()),
            Some(90),
        )
        .unwrap();
        assert_eq!(cfg.size, "512x512");
        assert_eq!(cfg.timeout_seconds, 90);
    }

    #[test]
    fn validate_image_size_enforces_known_families() {
        // gpt-image-1 supports 1536x1024 but not 512x512.
        assert!(validate_image_size("gpt-image-1", "1536x1024").is_ok());
        assert!(validate_image_size("gpt-image-1", "512x512").is_err());
        // dall-e-3 supports 1792x1024 but not 512x512.
        assert!(validate_image_size("dall-e-3", "1792x1024").is_ok());
        assert!(validate_image_size("dall-e-3", "512x512").is_err());
        // dall-e-2 supports 512x512 but not 1536x1024.
        assert!(validate_image_size("dall-e-2", "512x512").is_ok());
        assert!(validate_image_size("dall-e-2", "1536x1024").is_err());
    }

    #[test]
    fn validate_image_size_passes_unknown_model_and_blank_size() {
        // Custom endpoints (unknown model) are left unrestricted.
        assert!(validate_image_size("my-custom-model", "999x999").is_ok());
        // Blank size is validated/defaulted elsewhere, not here.
        assert!(validate_image_size("dall-e-3", "").is_ok());
        assert!(validate_image_size("dall-e-3", "   ").is_ok());
    }
}
