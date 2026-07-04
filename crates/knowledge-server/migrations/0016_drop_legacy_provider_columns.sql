-- Legacy flat provider/search config is superseded by provider_connections +
-- search_provider_configs. Drop provider_mode and all flat columns; UI is now the
-- only source of truth (no env seeding).
ALTER TABLE system_settings
  DROP COLUMN IF EXISTS provider_mode,
  DROP COLUMN IF EXISTS provider_base_url,
  DROP COLUMN IF EXISTS provider_api_key,
  DROP COLUMN IF EXISTS provider_model,
  DROP COLUMN IF EXISTS provider_embedding_model,
  DROP COLUMN IF EXISTS provider_timeout_seconds,
  DROP COLUMN IF EXISTS search_api_key,
  DROP COLUMN IF EXISTS serpapi_engine,
  DROP COLUMN IF EXISTS searxng_url,
  DROP COLUMN IF EXISTS searxng_categories,
  DROP COLUMN IF EXISTS ollama_search_url,
  DROP COLUMN IF EXISTS tavily_base_url,
  DROP COLUMN IF EXISTS serpapi_base_url;
