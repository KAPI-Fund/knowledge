ALTER TABLE system_settings ADD COLUMN provider_base_url TEXT;
ALTER TABLE system_settings ADD COLUMN provider_api_key TEXT;
ALTER TABLE system_settings ADD COLUMN provider_model TEXT;
ALTER TABLE system_settings ADD COLUMN provider_embedding_model TEXT;
ALTER TABLE system_settings ADD COLUMN provider_timeout_seconds BIGINT;
