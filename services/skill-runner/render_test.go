package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"strings"
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
	out, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq(), nil)
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
	_, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq(), nil)
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
	_, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq(), nil)
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
	_, err := runRender(context.Background(), &fakeFactory{createErr: errors.New("kvm down")}, "tpl-1", baseReq(), nil)
	if err == nil {
		t.Fatal("expected error on create failure")
	}
	if asRenderError(t, err).Stage != "create" {
		t.Fatalf("stage = %q, want create", asRenderError(t, err).Stage)
	}
}

func TestRenderEmitsProgress(t *testing.T) {
	sb := &fakeSandbox{
		runResult:   CommandResult{ExitCode: 0},
		stdoutLines: []string{"thinking about slides", "writing deck.html"},
		files:       map[string]string{"/work/out/deck.html": "<!DOCTYPE html>"},
	}
	var events []string
	emit := func(stage, message string) {
		events = append(events, stage+"|"+message)
	}
	if _, err := runRender(context.Background(), &fakeFactory{sb: sb}, "tpl-1", baseReq(), emit); err != nil {
		t.Fatalf("unexpected err: %v", err)
	}
	want := []string{
		"create|正在创建沙箱",
		"create|正在写入输入",
		"codex|codex 开始生成",
		"codex|thinking about slides",
		"codex|writing deck.html",
	}
	if len(events) != len(want) {
		t.Fatalf("events=%#v", events)
	}
	for i := range want {
		if events[i] != want[i] {
			t.Fatalf("event[%d]=%q, want %q", i, events[i], want[i])
		}
	}
}

func TestHandleRenderStreamsNDJSON(t *testing.T) {
	sb := &fakeSandbox{
		runResult:   CommandResult{ExitCode: 0},
		stdoutLines: []string{"slide 1 done"},
		files:       map[string]string{"/work/out/deck.html": "<!DOCTYPE html><html>deck</html>"},
	}
	s := newServer(&fakeFactory{sb: sb}, "tpl-1", 4)

	body, _ := json.Marshal(baseReq())
	req := httptest.NewRequest(http.MethodPost, "/render", bytes.NewReader(body))
	rec := httptest.NewRecorder()
	s.handleRender(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status=%d body=%s", rec.Code, rec.Body.String())
	}
	if ct := rec.Header().Get("Content-Type"); ct != "application/x-ndjson" {
		t.Fatalf("content-type=%q", ct)
	}
	lines := strings.Split(strings.TrimSpace(rec.Body.String()), "\n")
	if len(lines) < 2 {
		t.Fatalf("expected progress + done lines, got %#v", lines)
	}
	var first map[string]any
	if err := json.Unmarshal([]byte(lines[0]), &first); err != nil {
		t.Fatalf("first line not json: %v", err)
	}
	if first["type"] != "progress" || first["stage"] != "create" {
		t.Fatalf("first=%#v", first)
	}
	var last map[string]any
	if err := json.Unmarshal([]byte(lines[len(lines)-1]), &last); err != nil {
		t.Fatalf("last line not json: %v", err)
	}
	if last["type"] != "done" || last["deck_html"] != "<!DOCTYPE html><html>deck</html>" {
		t.Fatalf("last=%#v", last)
	}
}

func TestHandleRenderStreamsErrorLine(t *testing.T) {
	sb := &fakeSandbox{runResult: CommandResult{ExitCode: 1, Stderr: "model refused"}}
	s := newServer(&fakeFactory{sb: sb}, "tpl-1", 4)

	body, _ := json.Marshal(baseReq())
	req := httptest.NewRequest(http.MethodPost, "/render", bytes.NewReader(body))
	rec := httptest.NewRecorder()
	s.handleRender(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status=%d", rec.Code)
	}
	lines := strings.Split(strings.TrimSpace(rec.Body.String()), "\n")
	var last map[string]any
	if err := json.Unmarshal([]byte(lines[len(lines)-1]), &last); err != nil {
		t.Fatalf("last line not json: %v", err)
	}
	if last["type"] != "error" || last["stage"] != "codex" || last["message"] != "model refused" {
		t.Fatalf("last=%#v", last)
	}
}

func TestHandleRenderRejectsBadRequestBeforeStream(t *testing.T) {
	s := newServer(&fakeFactory{sb: &fakeSandbox{}}, "tpl-1", 4)
	req := httptest.NewRequest(http.MethodPost, "/render", strings.NewReader(`{"skill_id":""}`))
	rec := httptest.NewRecorder()
	s.handleRender(rec, req)
	if rec.Code != http.StatusBadRequest {
		t.Fatalf("status=%d", rec.Code)
	}
}

func TestHandleRenderRejectsWhenAtCapacity(t *testing.T) {
	s := newServer(&fakeFactory{sb: &fakeSandbox{}}, "tpl-1", 1)
	s.sem <- struct{}{} // 占满唯一并发槽

	body, _ := json.Marshal(baseReq())
	req := httptest.NewRequest(http.MethodPost, "/render", bytes.NewReader(body))
	rec := httptest.NewRecorder()
	s.handleRender(rec, req)

	if rec.Code != http.StatusTooManyRequests {
		t.Fatalf("status=%d body=%s", rec.Code, rec.Body.String())
	}
	var errBody map[string]string
	if err := json.Unmarshal(rec.Body.Bytes(), &errBody); err != nil {
		t.Fatalf("body not json: %v", err)
	}
	if errBody["stage"] != "create" || errBody["message"] == "" {
		t.Fatalf("body=%#v", errBody)
	}

	// 释放槽后必须恢复受理。
	<-s.sem
	sb := &fakeSandbox{
		runResult: CommandResult{ExitCode: 0},
		files:     map[string]string{"/work/out/deck.html": "<!DOCTYPE html>"},
	}
	s.factory = &fakeFactory{sb: sb}
	req = httptest.NewRequest(http.MethodPost, "/render", bytes.NewReader(body))
	rec = httptest.NewRecorder()
	s.handleRender(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("status after release=%d", rec.Code)
	}
	if len(s.sem) != 0 {
		t.Fatal("slot not released after render")
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
