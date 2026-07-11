package main

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"
)

// scriptSandbox 按命令内容路由返回值,exec 流程一次会跑多条命令。
type scriptSandbox struct {
	writes map[string]string
	run    func(ctx context.Context, cmd string) (CommandResult, error)
	killed bool
}

func (s *scriptSandbox) WriteFile(_ context.Context, path, content string) error {
	if s.writes == nil {
		s.writes = map[string]string{}
	}
	s.writes[path] = content
	return nil
}

func (s *scriptSandbox) RunCommand(ctx context.Context, cmd string, _ map[string]string, _ func(string)) (CommandResult, error) {
	return s.run(ctx, cmd)
}

func (s *scriptSandbox) ReadFile(_ context.Context, _ string) (string, error) {
	return "", nil
}

func (s *scriptSandbox) Kill(_ context.Context) error {
	s.killed = true
	return nil
}

type scriptFactory struct{ sb *scriptSandbox }

func (f *scriptFactory) Create(_ context.Context) (Sandbox, error) { return f.sb, nil }

func TestRunExecUploadsFilesRunsCommandAndReturnsChangedFiles(t *testing.T) {
	snapshots := 0
	sb := &scriptSandbox{}
	sb.run = func(_ context.Context, cmd string) (CommandResult, error) {
		switch {
		case strings.HasPrefix(cmd, "mkdir -p"):
			return CommandResult{ExitCode: 0}, nil
		case strings.Contains(cmd, "sha256sum"):
			snapshots++
			if snapshots == 1 {
				return CommandResult{ExitCode: 0, Stdout: "5 aaaa ./input.md\n"}, nil
			}
			return CommandResult{ExitCode: 0, Stdout: "5 aaaa ./input.md\n7 bbbb ./out.svg\n"}, nil
		case strings.Contains(cmd, "TOOBIG"):
			return CommandResult{ExitCode: 0, Stdout: base64.StdEncoding.EncodeToString([]byte("<svg/>\n"))}, nil
		case strings.Contains(cmd, "&& ( "):
			if !strings.Contains(cmd, "make-svg") {
				t.Fatalf("unexpected exec command: %s", cmd)
			}
			return CommandResult{ExitCode: 0, Stdout: "generated\n", Stderr: ""}, nil
		default:
			t.Fatalf("unexpected command: %s", cmd)
			return CommandResult{}, nil
		}
	}
	req := ExecRequest{
		Command:        "make-svg",
		TimeoutSeconds: 5,
		Files: []ExecFile{
			{Path: "input.md", ContentB64: base64.StdEncoding.EncodeToString([]byte("hello"))},
		},
	}
	out, err := runExec(context.Background(), &scriptFactory{sb: sb}, req)
	if err != nil {
		t.Fatal(err)
	}
	if sb.writes["/work/agent-workspace/input.md"] != "hello" {
		t.Fatalf("input file not uploaded: %#v", sb.writes)
	}
	if out.ExitCode == nil || *out.ExitCode != 0 || out.TimedOut {
		t.Fatalf("bad result: %+v", out)
	}
	if out.Stdout != "generated\n" {
		t.Fatalf("stdout: %q", out.Stdout)
	}
	if len(out.Files) != 1 || out.Files[0].Path != "out.svg" {
		t.Fatalf("changed files: %+v", out.Files)
	}
	decoded, _ := base64.StdEncoding.DecodeString(out.Files[0].ContentB64)
	if string(decoded) != "<svg/>\n" {
		t.Fatalf("decoded content: %q", decoded)
	}
	if !sb.killed {
		t.Fatal("sandbox not killed")
	}
}

func TestRunExecReportsTimeoutInsteadOfError(t *testing.T) {
	sb := &scriptSandbox{}
	sb.run = func(ctx context.Context, cmd string) (CommandResult, error) {
		if strings.Contains(cmd, "&& ( ") {
			<-ctx.Done()
			return CommandResult{}, ctx.Err()
		}
		return CommandResult{ExitCode: 0}, nil
	}
	start := time.Now()
	out, err := runExec(context.Background(), &scriptFactory{sb: sb}, ExecRequest{Command: "sleep 999", TimeoutSeconds: 1})
	if err != nil {
		t.Fatal(err)
	}
	if !out.TimedOut || out.ExitCode != nil {
		t.Fatalf("expected timeout result, got %+v", out)
	}
	if !strings.Contains(out.Stderr, "timed out after 1s") {
		t.Fatalf("stderr: %q", out.Stderr)
	}
	if time.Since(start) > 5*time.Second {
		t.Fatal("timeout did not bound the wait")
	}
}

func TestHandleExecRejectsWhenAtCapacity(t *testing.T) {
	s := newServer(&scriptFactory{sb: &scriptSandbox{}}, "tpl-1", 1)
	s.sem <- struct{}{} // 占满唯一并发槽

	req := httptest.NewRequest(http.MethodPost, "/exec", strings.NewReader(`{"command":"ls"}`))
	rec := httptest.NewRecorder()
	s.handleExec(rec, req)

	if rec.Code != http.StatusTooManyRequests {
		t.Fatalf("status=%d body=%s", rec.Code, rec.Body.String())
	}
	var errBody map[string]string
	if err := json.Unmarshal(rec.Body.Bytes(), &errBody); err != nil {
		t.Fatalf("body not json: %v", err)
	}
	if errBody["stage"] != "exec" || errBody["message"] == "" {
		t.Fatalf("body=%#v", errBody)
	}
	if len(s.sem) != 1 {
		t.Fatal("rejected request must not consume the held slot")
	}
}

func TestSafeExecRelPath(t *testing.T) {
	for _, bad := range []string{"", "/etc/passwd", "../x", "a/../../x", "..", `a\b`} {
		if safeExecRelPath(bad) {
			t.Fatalf("expected unsafe: %q", bad)
		}
	}
	for _, good := range []string{"a.md", "deck/index.html", "a/b/c.svg"} {
		if !safeExecRelPath(good) {
			t.Fatalf("expected safe: %q", good)
		}
	}
}

func TestChangedExecFilesDiffsAndSorts(t *testing.T) {
	before := map[string]string{"a": "1 x", "b": "2 y"}
	after := map[string]string{"a": "1 x", "b": "3 z", "c": "4 w"}
	got := changedExecFiles(before, after)
	if len(got) != 2 || got[0] != "b" || got[1] != "c" {
		t.Fatalf("got %v", got)
	}
}
