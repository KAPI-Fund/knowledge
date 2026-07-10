package main

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"os"
	"sync"
	"time"
)

// renderTimeout 是 codex 整个 agentic 生成的墙钟上限(见 spec 第4节)。
const renderTimeout = 600 * time.Second

type server struct {
	factory    SandboxFactory
	templateID string
}

// handleRender 以 NDJSON 流式响应:执行期间写 progress 行,结束写 done/error 终态行。
// 流开始前的校验错误仍用非 200 JSON(writeErr)。
func (s *server) handleRender(w http.ResponseWriter, r *http.Request) {
	var req RenderRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeErr(w, http.StatusBadRequest, "create", "bad request body: "+err.Error())
		return
	}
	if req.SkillID == "" {
		writeErr(w, http.StatusBadRequest, "create", "skill_id required")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), renderTimeout)
	defer cancel()

	w.Header().Set("Content-Type", "application/x-ndjson")
	w.WriteHeader(http.StatusOK)
	flusher, _ := w.(http.Flusher)

	start := time.Now()
	var mu sync.Mutex
	closed := false
	// writeLine 序列化 emit/心跳/终态的并发写;final 置 closed,handler 返回后
	// 心跳 goroutine 不得再碰 ResponseWriter。
	writeLine := func(v any, final bool) {
		mu.Lock()
		defer mu.Unlock()
		if closed {
			return
		}
		if final {
			closed = true
		}
		_ = json.NewEncoder(w).Encode(v)
		if flusher != nil {
			flusher.Flush()
		}
	}
	emit := func(stage, message string) {
		writeLine(map[string]any{
			"type":      "progress",
			"stage":     stage,
			"message":   message,
			"elapsed_s": int(time.Since(start).Seconds()),
		}, false)
	}

	hbCtx, hbCancel := context.WithCancel(ctx)
	defer hbCancel()
	go func() {
		t := time.NewTicker(10 * time.Second)
		defer t.Stop()
		for {
			select {
			case <-hbCtx.Done():
				return
			case <-t.C:
				emit("codex", "仍在生成…")
			}
		}
	}()

	out, err := runRender(ctx, s.factory, s.templateID, req, emit)
	hbCancel()
	if err != nil {
		re, ok := err.(*RenderError)
		stage, msg := "codex", err.Error()
		if ok {
			stage, msg = re.Stage, re.Message
		}
		log.Printf("render failed skill=%s stage=%s dur=%s: %s", req.SkillID, stage, time.Since(start), msg)
		writeLine(map[string]string{"type": "error", "stage": stage, "message": msg}, true)
		return
	}
	log.Printf("render ok skill=%s dur=%s bytes=%d", req.SkillID, time.Since(start), len(out.DeckHTML))
	writeLine(map[string]string{"type": "done", "deck_html": out.DeckHTML}, true)
}

func writeErr(w http.ResponseWriter, status int, stage, message string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(map[string]string{"stage": stage, "message": message})
}

func (s *server) handleHealth(w http.ResponseWriter, _ *http.Request) {
	w.WriteHeader(http.StatusOK)
	_, _ = w.Write([]byte("ok"))
}

func main() {
	addr := envOr("SKILL_RUNNER_ADDR", ":4600")
	templateID := os.Getenv("CUBE_TEMPLATE_ID")
	if templateID == "" {
		log.Fatal("CUBE_TEMPLATE_ID is required")
	}
	factory, err := newCubeFactory(templateID)
	if err != nil {
		log.Fatalf("init cube factory: %v", err)
	}
	if v := os.Getenv("SKILLS_VERSION"); v != "" {
		log.Printf("skill-runner starting; skills version=%s template=%s", v, templateID)
	}
	s := &server{factory: factory, templateID: templateID}
	mux := http.NewServeMux()
	mux.HandleFunc("/render", s.handleRender)
	mux.HandleFunc("/health", s.handleHealth)
	log.Printf("skill-runner listening on %s", addr)
	log.Fatal(http.ListenAndServe(addr, mux))
}

func envOr(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}
