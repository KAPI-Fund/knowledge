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
        timeout_seconds: timeout_seconds.unwrap_or(60),
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
        assert_eq!(cfg.timeout_seconds, 60);
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
}
