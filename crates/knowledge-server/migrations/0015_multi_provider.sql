-- Multi-provider configuration: a preset connection list for the LLM
-- capability, plus independent embedding / image blocks and a per-provider
-- web-search config map on the singleton system_settings row.

-- 1. LLM preset connection list. Exactly one row is active (enforced in the
--    service layer via a single UPDATE that sets is_active = (id = $target)).
CREATE TABLE provider_connections (
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_key TEXT,
    model TEXT NOT NULL,
    timeout_seconds BIGINT,
    is_active BOOLEAN NOT NULL DEFAULT false,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_provider_connections_sort ON provider_connections (sort_order);

-- 2. Seed a "Default" active connection from the legacy flat provider_* columns,
--    but only when a base_url and model are actually configured. gen_random_uuid
--    is available via pgcrypto in this database; cast to text for the TEXT PK.
INSERT INTO provider_connections
    (id, label, base_url, api_key, model, timeout_seconds, is_active, sort_order, created_at, updated_at)
SELECT
    gen_random_uuid()::text,
    'Default',
    provider_base_url,
    provider_api_key,
    provider_model,
    provider_timeout_seconds,
    true,
    0,
    to_char(now() AT TIME ZONE 'utc', 'YYYY-MM-DD"T"HH24:MI:SS"Z"'),
    to_char(now() AT TIME ZONE 'utc', 'YYYY-MM-DD"T"HH24:MI:SS"Z"')
FROM system_settings
WHERE id = 1
  AND provider_base_url IS NOT NULL AND btrim(provider_base_url) <> ''
  AND provider_model    IS NOT NULL AND btrim(provider_model)    <> '';

-- 3. Independent embedding block.
ALTER TABLE system_settings ADD COLUMN embedding_enabled BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE system_settings ADD COLUMN embedding_base_url TEXT;
ALTER TABLE system_settings ADD COLUMN embedding_api_key TEXT;
ALTER TABLE system_settings ADD COLUMN embedding_model TEXT;
ALTER TABLE system_settings ADD COLUMN embedding_timeout_seconds BIGINT;

-- 4. Seed embedding from the legacy provider_* columns; enabled iff an
--    embedding model was configured.
UPDATE system_settings
SET embedding_base_url = provider_base_url,
    embedding_api_key = provider_api_key,
    embedding_model = provider_embedding_model,
    embedding_timeout_seconds = provider_timeout_seconds,
    embedding_enabled = (provider_embedding_model IS NOT NULL AND btrim(provider_embedding_model) <> '')
WHERE id = 1;

-- 5. Net-new image-generation block (OpenAI-compatible /v1/images/generations).
--    image_model is intentionally left NULL: the admin must choose an image
--    model (the legacy provider_model is a text/chat model and is the bug we fix).
ALTER TABLE system_settings ADD COLUMN image_base_url TEXT;
ALTER TABLE system_settings ADD COLUMN image_api_key TEXT;
ALTER TABLE system_settings ADD COLUMN image_model TEXT;
ALTER TABLE system_settings ADD COLUMN image_size TEXT NOT NULL DEFAULT '1024x1024';
ALTER TABLE system_settings ADD COLUMN image_timeout_seconds BIGINT;

UPDATE system_settings
SET image_base_url = provider_base_url,
    image_api_key = provider_api_key,
    image_timeout_seconds = provider_timeout_seconds
WHERE id = 1;

-- 6. Per-provider web-search config map so switching the active provider keeps
--    each provider's key/fields. Seeded from the flat search_* columns; the
--    active provider's key is placed under its own block, other providers keep
--    only their non-secret fields. jsonb_strip_nulls drops absent keys.
ALTER TABLE system_settings ADD COLUMN search_provider_configs JSONB NOT NULL DEFAULT '{}'::jsonb;

UPDATE system_settings
SET search_provider_configs = jsonb_strip_nulls(jsonb_build_object(
    'tavily',  jsonb_build_object(
        'apiKey',  CASE WHEN search_provider = 'tavily'  THEN search_api_key END,
        'baseUrl', tavily_base_url),
    'serpapi', jsonb_build_object(
        'apiKey',  CASE WHEN search_provider = 'serpapi' THEN search_api_key END,
        'engine',  serpapi_engine,
        'baseUrl', serpapi_base_url),
    'searxng', jsonb_build_object(
        'url',        searxng_url,
        'categories', searxng_categories),
    'ollama',  jsonb_build_object(
        'apiKey', CASE WHEN search_provider = 'ollama' THEN search_api_key END,
        'url',    ollama_search_url)
))
WHERE id = 1;
