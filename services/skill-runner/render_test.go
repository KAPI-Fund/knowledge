package main

import (
	"context"
	"errors"
	"testing"
)

func baseReq() RenderRequest {
	return RenderRequest{
		SkillID:   "guizang-ppt",
		Selection: "内容",
		Argument:  "swiss",
		Provider:  Provider{BaseURL: "https://api.x/v1", APIKey: "sk-1", Model: "gpt-5.4"},
	}
}

func TestRenderSuccess(t *testing.T) {
	sb := &fakeSandbox{
		runResult: CommandResult{ExitCode: 0},
		files:     map[string]string{"/work/out/deck.html": "<!DOCTYPE html><html>ok</html>"},
	}
	out, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq())
	if err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	if out.DeckHTML != "<!DOCTYPE html><html>ok</html>" {
		t.Fatalf("bad html: %q", out.DeckHTML)
	}
	if !sb.killed {
		t.Fatal("sandbox must be killed on success")
	}
	if sb.writes["/work/input/selection.md"] != "内容" {
		t.Fatalf("selection not written: %v", sb.writes)
	}
	if !contains(sb.writes["/root/.codex/config.toml"], `base_url = "https://api.x/v1"`) {
		t.Fatalf("codex config not written: %v", sb.writes["/root/.codex/config.toml"])
	}
}

func TestRenderCodexNonZeroExit(t *testing.T) {
	sb := &fakeSandbox{runResult: CommandResult{ExitCode: 1, Stderr: "model refused"}}
	_, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq())
	if err == nil {
		t.Fatal("expected error on non-zero exit")
	}
	re := asRenderError(t, err)
	if re.Stage != "codex" {
		t.Fatalf("stage = %q, want codex", re.Stage)
	}
	if !sb.killed {
		t.Fatal("sandbox must be killed on codex failure")
	}
}

func TestRenderMissingOutput(t *testing.T) {
	sb := &fakeSandbox{
		runResult:    CommandResult{ExitCode: 0},
		failReadPath: "/work/out/deck.html",
	}
	_, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq())
	if err == nil {
		t.Fatal("expected error on missing output")
	}
	if asRenderError(t, err).Stage != "output" {
		t.Fatalf("stage = %q, want output", asRenderError(t, err).Stage)
	}
	if !sb.killed {
		t.Fatal("sandbox must be killed on missing output")
	}
}

func TestRenderCreateFailure(t *testing.T) {
	_, err := runRender(context.Background(), &fakeFactory{createErr: errors.New("kvm down")}, "tpl-1", baseReq())
	if err == nil {
		t.Fatal("expected error on create failure")
	}
	if asRenderError(t, err).Stage != "create" {
		t.Fatalf("stage = %q, want create", asRenderError(t, err).Stage)
	}
}

func asRenderError(t *testing.T, err error) *RenderError {
	t.Helper()
	var re *RenderError
	if !errors.As(err, &re) {
		t.Fatalf("error is not *RenderError: %v", err)
	}
	return re
}
