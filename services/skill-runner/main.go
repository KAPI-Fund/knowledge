package main

import (
	"context"
	"encoding/json"
	"log"
	"net/http"
	"os"
	"time"
)

// renderTimeout 是 codex 整个 agentic 生成的墙钟上限(见 spec 第4节)。
const renderTimeout = 600 * time.Second

type server struct {
	factory    SandboxFactory
	templateID string
}

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

	start := time.Now()
	out, err := runRender(ctx, s.factory, s.templateID, req)
	if err != nil {
		re, ok := err.(*RenderError)
		stage, msg := "codex", err.Error()
		if ok {
			stage, msg = re.Stage, re.Message
		}
		status := http.StatusInternalServerError
		if stage == "timeout" {
			status = http.StatusGatewayTimeout
		}
		log.Printf("render failed skill=%s stage=%s dur=%s: %s", req.SkillID, stage, time.Since(start), msg)
		writeErr(w, status, stage, msg)
		return
	}
	log.Printf("render ok skill=%s dur=%s bytes=%d", req.SkillID, time.Since(start), len(out.DeckHTML))
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(out)
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
