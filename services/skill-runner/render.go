package main

import (
	"context"
	"fmt"
	"strings"
)

const (
	skillsRoot     = "/skills"
	inputSelection = "/work/input/selection.md"
	inputArgument  = "/work/input/argument.txt"
	codexConfig    = "/root/.codex/config.toml"
	promptPath     = "/work/PROMPT.md"
	outputPath     = "/work/out/deck.html"
)

// RenderRequest 是 POST /render 的 body(与 backend 的 serde snake_case 对齐)。
type RenderRequest struct {
	SkillID   string   `json:"skill_id"`
	Selection string   `json:"selection"`
	Argument  string   `json:"argument"`
	Provider  Provider `json:"provider"`
}

// RenderedDeck 是成功响应。
type RenderedDeck struct {
	DeckHTML string `json:"deck_html"`
}

// RenderError 带阶段标签,map 到 HTTP + backend 的 job error。
type RenderError struct {
	Stage   string // create | codex | timeout | output
	Message string
}

func (e *RenderError) Error() string {
	return fmt.Sprintf("%s: %s", e.Stage, e.Message)
}

// runRender 编排一次沙箱生命周期。无论成功失败都 kill(create 失败除外——没有沙箱)。
func runRender(ctx context.Context, factory SandboxFactory, templateID string, req RenderRequest) (RenderedDeck, error) {
	_ = templateID // 真实 factory 用它 Create;fake 忽略。
	sb, err := factory.Create(ctx)
	if err != nil {
		return RenderedDeck{}, &RenderError{Stage: "create", Message: err.Error()}
	}
	defer sb.Kill(context.Background())

	if err := sb.WriteFile(ctx, inputSelection, req.Selection); err != nil {
		return RenderedDeck{}, &RenderError{Stage: "create", Message: "write selection: " + err.Error()}
	}
	if err := sb.WriteFile(ctx, inputArgument, req.Argument); err != nil {
		return RenderedDeck{}, &RenderError{Stage: "create", Message: "write argument: " + err.Error()}
	}
	if err := sb.WriteFile(ctx, codexConfig, renderCodexConfig(req.Provider)); err != nil {
		return RenderedDeck{}, &RenderError{Stage: "create", Message: "write codex config: " + err.Error()}
	}

	cmd := fmt.Sprintf(
		`codex exec --skip-git-repo-check --dangerously-bypass-approvals-and-sandbox --cd %s/%s "$(cat %s)"`,
		skillsRoot, req.SkillID, promptPath,
	)
	res, err := sb.RunCommand(ctx, cmd, map[string]string{"PROVIDER_API_KEY": req.Provider.APIKey})
	if err != nil {
		if ctx.Err() == context.DeadlineExceeded {
			return RenderedDeck{}, &RenderError{Stage: "timeout", Message: "codex render timed out"}
		}
		return RenderedDeck{}, &RenderError{Stage: "codex", Message: err.Error()}
	}
	if res.ExitCode != 0 {
		return RenderedDeck{}, &RenderError{Stage: "codex", Message: summarize(res.Stderr)}
	}

	html, err := sb.ReadFile(ctx, outputPath)
	if err != nil {
		return RenderedDeck{}, &RenderError{Stage: "output", Message: "read output: " + err.Error()}
	}
	if strings.TrimSpace(html) == "" {
		return RenderedDeck{}, &RenderError{Stage: "output", Message: "codex produced empty output"}
	}
	return RenderedDeck{DeckHTML: html}, nil
}

// summarize 截断长 stderr,避免 job error 塞爆。
func summarize(s string) string {
	const max = 2000
	s = strings.TrimSpace(s)
	if len(s) > max {
		return s[:max] + "…(truncated)"
	}
	if s == "" {
		return "codex exited non-zero with no stderr"
	}
	return s
}
