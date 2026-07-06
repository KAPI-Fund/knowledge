-- Firecrawl-only web fetch provider config, mirroring search_provider +
-- search_provider_configs. Selector defaults to 'none' so a fresh install has no
-- fetch provider until one is set in the admin UI; the URL node returns a clear
-- "not configured" error until then.
ALTER TABLE system_settings ADD COLUMN fetch_provider TEXT NOT NULL DEFAULT 'none';
ALTER TABLE system_settings ADD COLUMN fetch_provider_configs JSONB NOT NULL DEFAULT '{}'::jsonb;
