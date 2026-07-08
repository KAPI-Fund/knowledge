module github.com/knowledge/skill-runner

go 1.25.0

require github.com/tencentcloud/CubeSandbox/sdk/go v0.0.0-20260707100957-9b61ca278db4

// Patched: startProcess must envelope the Connect streaming request body.
// Upstream sends bare JSON, which this envd rejects. See third_party/cubesandbox-sdk/envd.go.
replace github.com/tencentcloud/CubeSandbox/sdk/go => ./third_party/cubesandbox-sdk
