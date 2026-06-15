ALTER TABLE system_settings ADD COLUMN search_provider TEXT NOT NULL DEFAULT 'none';
ALTER TABLE system_settings ADD COLUMN search_api_key TEXT;
ALTER TABLE system_settings ADD COLUMN serpapi_engine TEXT DEFAULT 'google';
ALTER TABLE system_settings ADD COLUMN searxng_url TEXT;
ALTER TABLE system_settings ADD COLUMN searxng_categories JSONB NOT NULL DEFAULT '["general"]'::jsonb;
ALTER TABLE system_settings ADD COLUMN ollama_search_url TEXT;
