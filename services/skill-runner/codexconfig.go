package main

import "fmt"

// Provider 是 backend 注入、codex 要连的 OpenAI 兼容连接。
type Provider struct {
	BaseURL string `json:"base_url"`
	APIKey  string `json:"api_key"`
	Model   string `json:"model"`
}

// renderCodexConfig 生成写入沙箱 ~/.codex/config.toml 的内容。
// api_key 不写这里——它经 PROVIDER_API_KEY 环境变量注入(见 render.go)。
func renderCodexConfig(p Provider) string {
	return fmt.Sprintf(`model = "%s"
model_provider = "knowledge"

[model_providers.knowledge]
name = "knowledge"
base_url = "%s"
env_key = "PROVIDER_API_KEY"
wire_api = "chat"
requires_openai_auth = false
`, p.Model, p.BaseURL)
}
