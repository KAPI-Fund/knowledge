package main

import "testing"

func TestRenderCodexConfig(t *testing.T) {
	cfg := renderCodexConfig(Provider{
		BaseURL: "https://api.x/v1",
		Model:   "gpt-5.4",
	})
	for _, want := range []string{
		`model = "gpt-5.4"`,
		`model_provider = "knowledge"`,
		`base_url = "https://api.x/v1"`,
		`env_key = "PROVIDER_API_KEY"`,
		`wire_api = "chat"`,
		`requires_openai_auth = false`,
	} {
		if !contains(cfg, want) {
			t.Fatalf("config missing %q\n---\n%s", want, cfg)
		}
	}
	// api_key 绝不能写进 config.toml。
	if contains(cfg, "api_key") {
		t.Fatalf("api_key must not appear in config.toml:\n%s", cfg)
	}
}

func contains(haystack, needle string) bool {
	return len(haystack) >= len(needle) && (func() bool {
		for i := 0; i+len(needle) <= len(haystack); i++ {
			if haystack[i:i+len(needle)] == needle {
				return true
			}
		}
		return false
	})()
}
