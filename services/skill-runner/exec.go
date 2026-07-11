package main

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"path"
	"strings"
	"time"
)

const (
	// execWorkspace 是沙箱内的 Agent 工作区,与 backend 项目目录里的
	// agent-workspace 一一对应:请求带入的文件写到这里,命令也从这里跑。
	execWorkspace      = "/work/agent-workspace"
	execMaxTimeoutSecs = 30
	execMaxFileBytes   = 2 * 1024 * 1024
	execMaxInputFiles  = 100
	execMaxReturnFiles = 50
)

// ExecFile 是 backend <-> sidecar 之间的一个 workspace 文件(内容 base64,兼容二进制)。
type ExecFile struct {
	Path       string `json:"path"`
	ContentB64 string `json:"content_b64"`
}

// ExecRequest 是 POST /exec 的 body(serde snake_case 对齐)。
type ExecRequest struct {
	Command        string     `json:"command"`
	TimeoutSeconds int        `json:"timeout_seconds"`
	Files          []ExecFile `json:"files"`
}

// ExecResponse 是 POST /exec 的成功 body。Files 只含执行后新增/变更的文件。
type ExecResponse struct {
	ExitCode *int       `json:"exit_code"`
	Stdout   string     `json:"stdout"`
	Stderr   string     `json:"stderr"`
	TimedOut bool       `json:"timed_out"`
	Files    []ExecFile `json:"files"`
}

func (s *server) handleExec(w http.ResponseWriter, r *http.Request) {
	var req ExecRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeErr(w, http.StatusBadRequest, "exec", "bad request body: "+err.Error())
		return
	}
	req.Command = strings.TrimSpace(req.Command)
	if req.Command == "" {
		writeErr(w, http.StatusBadRequest, "exec", "command required")
		return
	}
	if req.TimeoutSeconds < 1 || req.TimeoutSeconds > execMaxTimeoutSecs {
		req.TimeoutSeconds = execMaxTimeoutSecs
	}
	if len(req.Files) > execMaxInputFiles {
		writeErr(w, http.StatusBadRequest, "exec", fmt.Sprintf("too many input files (max %d)", execMaxInputFiles))
		return
	}
	for _, f := range req.Files {
		if !safeExecRelPath(f.Path) {
			writeErr(w, http.StatusBadRequest, "exec", "unsafe file path: "+f.Path)
			return
		}
	}
	if !s.tryAcquire() {
		writeErr(w, http.StatusTooManyRequests, "exec", "skill-runner 并发已满,请稍后重试")
		return
	}
	defer s.release()

	start := time.Now()
	out, err := runExec(r.Context(), s.factory, req)
	if err != nil {
		log.Printf("exec failed dur=%s cmd=%q: %s", time.Since(start), truncateLine(req.Command), err)
		writeErr(w, http.StatusInternalServerError, "exec", err.Error())
		return
	}
	log.Printf("exec ok dur=%s timedOut=%v exit=%v files=%d cmd=%q",
		time.Since(start), out.TimedOut, out.ExitCode, len(out.Files), truncateLine(req.Command))
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(out)
}

// runExec 编排一次 exec 沙箱生命周期:写入文件→快照→执行→快照→回读变更文件。
func runExec(ctx context.Context, factory SandboxFactory, req ExecRequest) (ExecResponse, error) {
	sb, err := factory.Create(ctx)
	if err != nil {
		return ExecResponse{}, fmt.Errorf("create sandbox: %w", err)
	}
	defer sb.Kill(context.Background())

	if _, err := sb.RunCommand(ctx, "mkdir -p "+shellQuote(execWorkspace), nil, nil); err != nil {
		return ExecResponse{}, fmt.Errorf("prepare workspace: %w", err)
	}
	for _, f := range req.Files {
		content, err := base64.StdEncoding.DecodeString(f.ContentB64)
		if err != nil {
			return ExecResponse{}, fmt.Errorf("decode %s: %w", f.Path, err)
		}
		target := path.Join(execWorkspace, f.Path)
		if err := sb.WriteFile(ctx, target, string(content)); err != nil {
			return ExecResponse{}, fmt.Errorf("write %s: %w", f.Path, err)
		}
	}

	before, err := snapshotExecWorkspace(ctx, sb)
	if err != nil {
		return ExecResponse{}, fmt.Errorf("snapshot workspace: %w", err)
	}

	execCtx, cancel := context.WithTimeout(ctx, time.Duration(req.TimeoutSeconds)*time.Second)
	defer cancel()
	cmd := "cd " + shellQuote(execWorkspace) + " && ( " + req.Command + " )"
	res, runErr := sb.RunCommand(execCtx, cmd, nil, nil)

	out := ExecResponse{Files: []ExecFile{}}
	if runErr != nil {
		if execCtx.Err() == context.DeadlineExceeded {
			out.TimedOut = true
			out.Stderr = fmt.Sprintf("Command timed out after %ds", req.TimeoutSeconds)
		} else {
			return ExecResponse{}, fmt.Errorf("run command: %w", runErr)
		}
	} else {
		exit := res.ExitCode
		out.ExitCode = &exit
		out.Stdout = res.Stdout
		out.Stderr = res.Stderr
	}

	after, err := snapshotExecWorkspace(ctx, sb)
	if err != nil {
		return ExecResponse{}, fmt.Errorf("snapshot workspace after exec: %w", err)
	}
	for _, rel := range changedExecFiles(before, after) {
		if len(out.Files) >= execMaxReturnFiles {
			break
		}
		content, ok, err := readExecFileB64(ctx, sb, rel)
		if err != nil {
			return ExecResponse{}, fmt.Errorf("read generated file %s: %w", rel, err)
		}
		if !ok {
			continue
		}
		out.Files = append(out.Files, ExecFile{Path: rel, ContentB64: content})
	}
	return out, nil
}

// snapshotExecWorkspace 返回 workspace 内文件相对路径 → "size hash" 签名。
func snapshotExecWorkspace(ctx context.Context, sb Sandbox) (map[string]string, error) {
	cmd := "cd " + shellQuote(execWorkspace) + ` && find . -type f -exec sh -c 'for f; do printf "%s %s %s\n" "$(wc -c < "$f")" "$(sha256sum "$f" | cut -d" " -f1)" "$f"; done' _ {} + 2>/dev/null; true`
	res, err := sb.RunCommand(ctx, cmd, nil, nil)
	if err != nil {
		return nil, err
	}
	files := map[string]string{}
	for _, line := range strings.Split(res.Stdout, "\n") {
		line = strings.TrimSpace(line)
		parts := strings.SplitN(line, " ", 3)
		if len(parts) != 3 {
			continue
		}
		rel := strings.TrimPrefix(parts[2], "./")
		files[rel] = parts[0] + " " + parts[1]
	}
	return files, nil
}

func changedExecFiles(before, after map[string]string) []string {
	var changed []string
	for rel, sig := range after {
		if before[rel] != sig {
			changed = append(changed, rel)
		}
	}
	// map 遍历无序;固定顺序便于测试与回传稳定。
	for i := 1; i < len(changed); i++ {
		for j := i; j > 0 && changed[j] < changed[j-1]; j-- {
			changed[j], changed[j-1] = changed[j-1], changed[j]
		}
	}
	return changed
}

// readExecFileB64 回读单个变更文件;超过 execMaxFileBytes 的跳过(ok=false)。
// 用 `base64 | tr -d '\n'` 而非 base64 -w0,兼容 busybox。
func readExecFileB64(ctx context.Context, sb Sandbox, rel string) (string, bool, error) {
	target := path.Join(execWorkspace, rel)
	cmd := fmt.Sprintf(
		`s=$(wc -c < %[1]s 2>/dev/null || echo 0); if [ "$s" -gt %[2]d ]; then echo TOOBIG; else base64 %[1]s | tr -d '\n'; fi`,
		shellQuote(target), execMaxFileBytes,
	)
	res, err := sb.RunCommand(ctx, cmd, nil, nil)
	if err != nil {
		return "", false, err
	}
	content := strings.TrimSpace(res.Stdout)
	if content == "TOOBIG" {
		return "", false, nil
	}
	return content, true, nil
}

// safeExecRelPath 拒绝绝对路径与目录逃逸;必须落在 workspace 内。
func safeExecRelPath(rel string) bool {
	if rel == "" || strings.HasPrefix(rel, "/") || strings.Contains(rel, "\\") {
		return false
	}
	cleaned := path.Clean(rel)
	if cleaned == "." || cleaned == ".." || strings.HasPrefix(cleaned, "../") {
		return false
	}
	return true
}

func shellQuote(s string) string {
	return "'" + strings.ReplaceAll(s, "'", `'\''`) + "'"
}
