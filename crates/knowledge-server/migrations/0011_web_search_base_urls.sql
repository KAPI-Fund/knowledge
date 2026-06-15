ALTER TABLE system_settings ADD COLUMN tavily_base_url TEXT NOT NULL DEFAULT 'https://api.tavily.com';
ALTER TABLE system_settings ADD COLUMN serpapi_base_url TEXT NOT NULL DEFAULT 'https://serpapi.com';
