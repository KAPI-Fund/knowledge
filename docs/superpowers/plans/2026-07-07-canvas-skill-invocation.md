# Canvas Skill Invocation System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a data-driven `/`-command skill system to the canvas chat, with the first real skill (`guizang-ppt`) rendering a selected node's content into a single-file HTML slide deck via a lightweight in-process LLM loop (no Node, no subprocess), shown in an `html` canvas node.

**Architecture:** A read-only `skills/` directory of `skill.toml` descriptors is scanned into an in-memory `Vec<SkillDescriptor>`. Chat dispatch queries the registry by command (the hardcoded `enum ChatCommand` is deleted; existing search/image/analyze become `runtime="builtin"` records). Async `llm-skill` runs enqueue a row on a new `canvas_skill_jobs` table and immediately place a `status:running` html node; a background worker (mirroring `spawn_scheduler`) runs a **bounded in-process LLM loop** — the guizang template + selected content are inlined into a prompt and the existing `OpenAiCompatibleProvider::complete_text` produces `deck.html` (reusing the same active `provider_connections` credential path as ingest, via streaming aggregation that already survives long generations). The deck is stored as a `text/html` asset and the result written back to the job row. The frontend polls `GET /api/canvas-skill-jobs/{id}` and patches the node — the backend never touches the canvas document (frontend is the sole writer).

**Why not Codex/Node:** guizang's real task is "content + fixed on-disk template → one HTML file", which needs no dynamic tool selection. Running a full agent runtime (Codex CLI + Node baked into the Rust image + the codebase's first subprocess subsystem) is disproportionate. The existing LLM client has no tool-calling, and adding streaming `tool_calls` parsing would be the *heaviest* non-Node option — conflicting with the lightweight goal. A bounded loop on the existing `complete_text` keeps the agent-style multi-turn flow and upstream prompt engineering with **zero client changes, zero Node, zero subprocess**.

**Tech Stack:** Rust (axum, sqlx/Postgres, tokio, async-stream), existing `OpenAiCompatibleProvider` (reqwest, streaming chat completions), React + TypeScript + react-query + shadcn/ui (Radix), React Flow canvas.

**Reference spec:** `docs/superpowers/specs/2026-07-07-canvas-skill-invocation-design.md`

---

## Conventions used in this plan

- Backend crate root: `crates/knowledge-server`. Run backend commands from `E:\Projects\Js\knowledge`.
- Frontend app root: `apps/admin`. Run frontend commands from `E:\Projects\Js\knowledge\apps\admin`.
- Backend tests: `cargo test -p knowledge-server --lib`. Frontend tests: `npx vitest run <path>`; types: `npx tsc --noEmit`.
- Standing directives (MEMORY.md): communicate in Chinese; compose shadcn/ui primitives (no hand-rolled UI); clean cutover with no legacy compat shims (delete `ChatCommand`, don't keep dual paths); cite upstream where logic is ported.
- **Verify names before editing:** each backend task lists the exact symbols it depends on. Confirm them against the current file (`resolve_active`, `ActiveConnection::from(...).provider()`, `OpenAiCompatibleProvider::complete_text`, `ProviderTextRequest { system_prompt, user_prompt }` → `ProviderTextResponse { text, usage }`, `NewAsset::new`, `insert_asset`, `asset_url`, `acquire_next_task` shape) before writing code — match the codebase, don't invent.
- **Confirmed facts (already read):** `crates/knowledge-server/src/providers/openai_compatible.rs` exposes `complete_text(ProviderTextRequest) -> Result<ProviderTextResponse, ProviderError>`, streams+aggregates so long generations survive the gateway's ~60s idle cut. `providers/mod.rs` re-exports `resolve_active`, `ActiveConnection`, `OpenAiCompatibleProvider`. **There is NO tool/function-calling** on the client — the loop must not assume it.

---

## Task 1: Migration — `canvas_skill_jobs` table

**Files:**
- Create: `crates/knowledge-server/migrations/0018_canvas_skill_jobs.sql`

- [ ] **Step 1: Write the migration**

```sql
-- Async jobs for canvas /skill invocations (llm-skill runtime). Independent of
-- project_tasks (which is project-bound); mirrors only the scheduler *pattern*
-- (lease + status machine), not that table.
CREATE TABLE canvas_skill_jobs (
    id                TEXT PRIMARY KEY NOT NULL DEFAULT (gen_random_uuid()::text),
    canvas_id         TEXT NOT NULL,
    node_id           TEXT NOT NULL,
    skill_id          TEXT NOT NULL,
    status            TEXT NOT NULL DEFAULT 'queued'
                          CHECK (status IN ('queued','running','done','error')),
    input             JSONB NOT NULL DEFAULT '{}'::jsonb,
    result            JSONB,
    error             JSONB,
    created_by        TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL,
    started_at        TEXT,
    finished_at       TEXT,
    lease_owner       TEXT,
    lease_expires_at  TEXT
);

CREATE INDEX canvas_skill_jobs_queued_idx
    ON canvas_skill_jobs (status, created_at)
    WHERE status IN ('queued', 'running');
```

- [ ] **Step 2: Verify it applies**

Run: `cargo test -p knowledge-server --lib`
Expected: migrations run on the test DB boot; suite still compiles/loads. (Later tasks add the code that uses this table; this task only proves the SQL parses/applies.)

- [ ] **Step 3: Commit**

```bash
git add crates/knowledge-server/migrations/0018_canvas_skill_jobs.sql
git commit -m "feat(canvas): add canvas_skill_jobs table"
```

---

## Task 2: Skill registry loader + `skill.toml` parsing

**Files:**
- Create: `crates/knowledge-server/src/skills/mod.rs`
- Create: `crates/knowledge-server/src/skills/descriptor.rs`
- Modify: `crates/knowledge-server/src/lib.rs` (add `pub mod skills;` to the module list)
- Confirm `toml` is a dependency: `crates/knowledge-server/Cargo.toml` (add `toml = "0.8"` if absent).

- [ ] **Step 1: Add `toml` dep if missing**

Check `crates/knowledge-server/Cargo.toml` `[dependencies]`. If `toml` is absent, add:
```toml
toml = "0.8"
```
Run: `cargo build -p knowledge-server` to fetch it. Expected: builds.

- [ ] **Step 2: Write the failing test for descriptor parsing**

Create `crates/knowledge-server/src/skills/descriptor.rs` with the type and a test module:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillRuntime {
    Builtin,
    /// In-process bounded LLM loop (guizang-style): inline template + content,
    /// call complete_text, produce a single HTML file. No Node, no subprocess.
    LlmSkill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputSource {
    Selection,
    Argument,
    None,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputSpec {
    pub source: InputSource,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub argument_hint: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputSpec {
    pub node_type: String,
    #[serde(default)]
    pub r#async: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillDescriptor {
    pub id: String,
    pub command: String,
    pub name: String,
    pub description: String,
    pub runtime: SkillRuntime,
    #[serde(default)]
    pub entry: Option<String>,
    pub input: InputSpec,
    pub output: OutputSpec,
    /// Absolute path to the skill directory on disk. Filled in by the loader,
    /// not present in skill.toml. Skipped during deserialization.
    #[serde(skip)]
    pub dir: std::path::PathBuf,
}

impl SkillDescriptor {
    pub fn parse_toml(source: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(source)
    }

    pub fn requires_selection(&self) -> bool {
        matches!(self.input.source, InputSource::Selection) && self.input.required
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUIZANG_TOML: &str = r#"
id          = "guizang-ppt"
command     = "ppt"
name        = "PPT 生成"
description = "把选中内容做成单文件 HTML 幻灯片"
runtime     = "llm-skill"
entry       = "SKILL.md"

[input]
source   = "selection"
required = true
argument_hint = "可选:风格 / 要求"

[output]
node_type = "html"
async     = true
"#;

    #[test]
    fn parses_guizang_descriptor() {
        let d = SkillDescriptor::parse_toml(GUIZANG_TOML).expect("parse");
        assert_eq!(d.id, "guizang-ppt");
        assert_eq!(d.command, "ppt");
        assert_eq!(d.runtime, SkillRuntime::LlmSkill);
        assert_eq!(d.entry.as_deref(), Some("SKILL.md"));
        assert_eq!(d.input.source, InputSource::Selection);
        assert!(d.input.required);
        assert_eq!(d.output.node_type, "html");
        assert!(d.output.r#async);
        assert!(d.requires_selection());
    }

    #[test]
    fn builtin_without_selection_does_not_require_selection() {
        let toml = r#"
id = "web-search"
command = "search"
name = "Search"
description = "Web search"
runtime = "builtin"

[input]
source = "argument"
required = false

[output]
node_type = "search"
async = false
"#;
        let d = SkillDescriptor::parse_toml(toml).expect("parse");
        assert_eq!(d.runtime, SkillRuntime::Builtin);
        assert!(!d.requires_selection());
    }
}
```

- [ ] **Step 3: Run the tests — expect fail then pass compilation**

First wire the module (Step 4) before running, since `descriptor.rs` isn't yet in the crate. After Step 4:
Run: `cargo test -p knowledge-server --lib skills::descriptor`
Expected: PASS (both tests).

- [ ] **Step 4: Write the loader in `skills/mod.rs`**

```rust
pub mod descriptor;

use std::path::Path;
use std::sync::Arc;

pub use descriptor::{InputSource, SkillDescriptor, SkillRuntime};

/// In-memory registry of skills scanned from the skills directory at startup.
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: Arc<Vec<SkillDescriptor>>,
}

impl SkillRegistry {
    pub fn new(skills: Vec<SkillDescriptor>) -> Self {
        Self { skills: Arc::new(skills) }
    }

    /// Scan `dir` for `*/skill.toml` files and parse each into a descriptor.
    /// Missing directory → empty registry (not an error): builtins are appended
    /// by the caller regardless.
    pub fn load_from_dir(dir: &Path) -> std::io::Result<Vec<SkillDescriptor>> {
        let mut out = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(err) => return Err(err),
        };
        for entry in entries {
            let entry = entry?;
            let sub = entry.path();
            if !sub.is_dir() {
                continue;
            }
            let toml_path = sub.join("skill.toml");
            if !toml_path.exists() {
                continue;
            }
            let source = std::fs::read_to_string(&toml_path)?;
            match SkillDescriptor::parse_toml(&source) {
                Ok(mut d) => {
                    d.dir = sub;
                    out.push(d);
                }
                Err(err) => {
                    tracing::error!(?toml_path, %err, "failed to parse skill.toml; skipping");
                }
            }
        }
        Ok(out)
    }

    pub fn all(&self) -> &[SkillDescriptor] {
        &self.skills
    }

    pub fn by_command(&self, command: &str) -> Option<&SkillDescriptor> {
        self.skills.iter().find(|s| s.command == command)
    }
}
```

- [ ] **Step 5: Register the module**

In `crates/knowledge-server/src/lib.rs` module list, add:
```rust
pub mod skills;
```

- [ ] **Step 6: Add a loader test**

Append to `skills/mod.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_dir_yields_empty() {
        let dir = std::path::Path::new("/nonexistent/skills/dir/xyz");
        let skills = SkillRegistry::load_from_dir(dir).expect("no error");
        assert!(skills.is_empty());
    }
}
```

- [ ] **Step 7: Verify**

Run: `cargo test -p knowledge-server --lib skills`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/knowledge-server/src/skills crates/knowledge-server/src/lib.rs crates/knowledge-server/Cargo.toml
git commit -m "feat(skills): skill.toml descriptor + registry loader"
```

---

## Task 3: Assemble the registry at startup (builtins + scanned) and store on AppState

**Files:**
- Modify: `crates/knowledge-server/src/skills/mod.rs` (add `builtin_descriptors()`)
- Modify: `crates/knowledge-server/src/lib.rs` (build registry in `bootstrap_state`, add to `AppState`)
- Modify: wherever `AppState` is defined (confirm: likely `crates/knowledge-server/src/state.rs` or `lib.rs`).

- [ ] **Step 1: Add builtin descriptors constructor with a test**

In `skills/mod.rs`, add:
```rust
use descriptor::{InputSpec, OutputSpec};

/// The three formerly-hardcoded chat commands, folded into the registry as
/// builtin records so dispatch is uniform (no per-skill if-branches).
pub fn builtin_descriptors() -> Vec<SkillDescriptor> {
    fn builtin(
        id: &str,
        command: &str,
        name: &str,
        description: &str,
        node_type: &str,
        source: InputSource,
    ) -> SkillDescriptor {
        SkillDescriptor {
            id: id.to_string(),
            command: command.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            runtime: SkillRuntime::Builtin,
            entry: None,
            input: InputSpec { source, required: false, argument_hint: None },
            output: OutputSpec { node_type: node_type.to_string(), r#async: false },
            dir: std::path::PathBuf::new(),
        }
    }
    vec![
        builtin("web-search", "search", "Web search", "Search the web and add a note", "search", InputSource::Argument),
        builtin("ai-image", "image", "Image", "Generate an image from a prompt", "ai_image", InputSource::Argument),
        builtin("ai-analyze", "analyze", "Analyze", "Analyze selected nodes", "ai_analyze", InputSource::Selection),
    ]
}
```

Add test:
```rust
#[test]
fn builtins_cover_the_three_legacy_commands() {
    let reg = SkillRegistry::new(builtin_descriptors());
    for cmd in ["search", "image", "analyze"] {
        let d = reg.by_command(cmd).unwrap_or_else(|| panic!("missing {cmd}"));
        assert_eq!(d.runtime, SkillRuntime::Builtin);
    }
}
```

- [ ] **Step 2: Build the registry in `bootstrap_state`**

In `lib.rs`, after the pool/migrate/seed lines, assemble the registry. Resolve the skills dir from an env var with a sensible default (the baked image path from Task 15):
```rust
    let skills_dir = std::env::var("KNOWLEDGE_SKILLS_DIR")
        .unwrap_or_else(|_| "/app/skills".to_string());
    let mut descriptors = skills::builtin_descriptors();
    descriptors.extend(skills::SkillRegistry::load_from_dir(std::path::Path::new(&skills_dir))?);
    let skill_registry = skills::SkillRegistry::new(descriptors);
```
Add `skill_registry` to the `AppState` construction (see Step 3).

- [ ] **Step 3: Add `skill_registry` field to `AppState`**

Locate the `AppState` struct (grep `struct AppState`). Add:
```rust
    pub skill_registry: crate::skills::SkillRegistry,
```
`SkillRegistry` is `Clone` (Arc inside), so it satisfies the `AppState: Clone` requirement. Wire the field in the constructor to the `skill_registry` built in Step 2.

- [ ] **Step 4: Verify**

Run: `cargo test -p knowledge-server --lib skills`
Then: `cargo check -p knowledge-server`
Expected: tests pass; crate compiles (dispatch not yet changed).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src
git commit -m "feat(skills): assemble registry (builtins + scanned) on AppState"
```

---

## Task 4: `GET /api/skills` — metadata-only listing

**Files:**
- Create: `crates/knowledge-server/src/skills/routes.rs`
- Modify: `crates/knowledge-server/src/skills/mod.rs` (add `pub mod routes;`)
- Modify: wherever the axum router is composed (grep `Router::new` / `.nest("/api"`). Confirm the auth layer used by other `/api` routes and apply the same.

- [ ] **Step 1: Write the failing test for metadata shape**

In `skills/routes.rs`:
```rust
use axum::{extract::State, Json};
use serde::Serialize;

use crate::skills::descriptor::{InputSource, SkillDescriptor};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub command: String,
    pub name: String,
    pub description: String,
    pub requires_selection: bool,
    pub argument_hint: Option<String>,
    pub output_node_type: String,
}

impl From<&SkillDescriptor> for SkillMetadata {
    fn from(d: &SkillDescriptor) -> Self {
        SkillMetadata {
            command: d.command.clone(),
            name: d.name.clone(),
            description: d.description.clone(),
            requires_selection: d.requires_selection(),
            argument_hint: d.input.argument_hint.clone(),
            output_node_type: d.output.node_type.clone(),
        }
    }
}

pub async fn list_skills(State(state): State<crate::AppState>) -> Json<Vec<SkillMetadata>> {
    let items = state
        .skill_registry
        .all()
        .iter()
        .map(SkillMetadata::from)
        .collect();
    Json(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::descriptor::{InputSpec, OutputSpec, SkillRuntime};

    fn llm_skill() -> SkillDescriptor {
        SkillDescriptor {
            id: "guizang-ppt".into(),
            command: "ppt".into(),
            name: "PPT".into(),
            description: "slides".into(),
            runtime: SkillRuntime::LlmSkill,
            entry: Some("SKILL.md".into()),
            input: InputSpec { source: InputSource::Selection, required: true, argument_hint: Some("hint".into()) },
            output: OutputSpec { node_type: "html".into(), r#async: true },
            dir: std::path::PathBuf::new(),
        }
    }

    #[test]
    fn metadata_hides_runtime_and_entry() {
        let meta = SkillMetadata::from(&llm_skill());
        let json = serde_json::to_string(&meta).unwrap();
        assert!(!json.contains("runtime"));
        assert!(!json.contains("entry"));
        assert!(!json.contains("SKILL.md"));
        assert!(json.contains("\"requiresSelection\":true"));
        assert!(json.contains("\"outputNodeType\":\"html\""));
    }
}
```

- [ ] **Step 2: Register module + run test**

Add `pub mod routes;` to `skills/mod.rs`.
Run: `cargo test -p knowledge-server --lib skills::routes`
Expected: PASS.

- [ ] **Step 3: Mount the route**

In the router composition, add (matching the existing `/api` nesting + auth middleware):
```rust
.route("/api/skills", axum::routing::get(crate::skills::routes::list_skills))
```
Confirm `AppState` is the router state type and the same auth layer as other `/api` GETs is applied.

- [ ] **Step 4: Verify**

Run: `cargo check -p knowledge-server` then `cargo test -p knowledge-server --lib skills`
Expected: compiles; tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/skills
git commit -m "feat(skills): GET /api/skills metadata endpoint"
```

---

## Task 5: `canvas_skill_jobs` store (create + lease + finish)

**Files:**
- Create: `crates/knowledge-server/src/canvas/skill_jobs.rs`
- Modify: `crates/knowledge-server/src/canvas/mod.rs` (add `pub mod skill_jobs;` — confirm the canvas module file name).

- [ ] **Step 1: Write the store**

Model the SQL on `tasks/store.rs` (`acquire_next_task` uses a CTE with `FOR UPDATE SKIP LOCKED`; confirm exact column list before writing). **Follow the TEXT convention (see Conventions section):** every id/timestamp is `TEXT`/`String`, ids come from `Uuid::new_v4().to_string()`, timestamps are Rust-computed RFC3339 strings — **no SQL `now()` / `make_interval()`**. `input`/`result`/`error` are `JSONB`, decoded as `serde_json::Value` (crate has sqlx `json` feature — the read side below relies on it, so the write side may bind `Value` directly). Create `skill_jobs.rs`:

```rust
use serde_json::Value;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SkillJob {
    pub id: String,
    pub canvas_id: String,
    pub node_id: String,
    pub skill_id: String,
    pub status: String,
    pub input: Value,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub created_by: String,
}

#[derive(Debug, Clone)]
pub struct NewSkillJob {
    pub canvas_id: String,
    pub node_id: String,
    pub skill_id: String,
    pub input: Value,
    pub created_by: String,
}

// Column tuple shared by acquire_next_job + get_job reads (matches the SELECT/RETURNING order).
type JobRow = (String, String, String, String, String, Value, Option<Value>, Option<Value>, String);

fn row_to_job(row: JobRow) -> SkillJob {
    let (id, canvas_id, node_id, skill_id, status, input, result, error, created_by) = row;
    SkillJob { id, canvas_id, node_id, skill_id, status, input, result, error, created_by }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .expect("format current time as RFC3339")
}

pub async fn create_job(pool: &PgPool, job: NewSkillJob) -> Result<String, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    // Match tasks/store.rs: serialize JSONB payloads to a String and cast with
    // `::jsonb` on the write side (the crate's sqlx build does not bind Value directly).
    let input = serde_json::to_string(&job.input).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        "INSERT INTO canvas_skill_jobs
           (id, canvas_id, node_id, skill_id, input, created_by, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7, $7)",
    )
    .bind(&id)
    .bind(&job.canvas_id)
    .bind(&job.node_id)
    .bind(&job.skill_id)
    .bind(&input)
    .bind(&job.created_by)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Lease the oldest queued job for `owner`, marking it running for `lease_seconds`.
pub async fn acquire_next_job(
    pool: &PgPool,
    owner: &str,
    lease_seconds: i64,
) -> Result<Option<SkillJob>, sqlx::Error> {
    let now = now_rfc3339();
    let lease_expires_at = OffsetDateTime::now_utc()
        .checked_add(time::Duration::seconds(lease_seconds))
        .expect("compute lease expiry")
        .format(&Rfc3339)
        .expect("format lease expiry as RFC3339");

    let row = sqlx::query_as::<_, JobRow>(
        "WITH next_job AS (
             SELECT id FROM canvas_skill_jobs
             WHERE status = 'queued'
             ORDER BY created_at
             FOR UPDATE SKIP LOCKED
             LIMIT 1
         )
         UPDATE canvas_skill_jobs j
         SET status = 'running',
             started_at = COALESCE(j.started_at, $2),
             updated_at = $2,
             lease_owner = $1,
             lease_expires_at = $3
         FROM next_job
         WHERE j.id = next_job.id
         RETURNING j.id, j.canvas_id, j.node_id, j.skill_id, j.status, j.input, j.result, j.error, j.created_by",
    )
    .bind(owner)
    .bind(&now)
    .bind(&lease_expires_at)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(row_to_job))
}

pub async fn complete_job(pool: &PgPool, id: &str, result: Value) -> Result<(), sqlx::Error> {
    let now = now_rfc3339();
    let result = serde_json::to_string(&result).unwrap_or_else(|_| "null".to_string());
    sqlx::query(
        "UPDATE canvas_skill_jobs
         SET status = 'done', result = $2::jsonb, error = NULL,
             finished_at = $3, updated_at = $3, lease_owner = NULL, lease_expires_at = NULL
         WHERE id = $1",
    )
    .bind(id)
    .bind(&result)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail_job(pool: &PgPool, id: &str, error: Value) -> Result<(), sqlx::Error> {
    let now = now_rfc3339();
    let error = serde_json::to_string(&error).unwrap_or_else(|_| "null".to_string());
    sqlx::query(
        "UPDATE canvas_skill_jobs
         SET status = 'error', error = $2::jsonb,
             finished_at = $3, updated_at = $3, lease_owner = NULL, lease_expires_at = NULL
         WHERE id = $1",
    )
    .bind(id)
    .bind(&error)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_job(pool: &PgPool, id: &str) -> Result<Option<SkillJob>, sqlx::Error> {
    let row = sqlx::query_as::<_, JobRow>(
        "SELECT id, canvas_id, node_id, skill_id, status, input, result, error, created_by
         FROM canvas_skill_jobs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(row_to_job))
}
```

- [ ] **Step 2: Register module**

Add `pub mod skill_jobs;` to the canvas module root (confirm filename — likely `canvas/mod.rs` or `canvas.rs`).

- [ ] **Step 3: Write an integration test (gated like existing DB tests)**

Follow the pattern used by existing store tests (they use a test pool helper). If the crate has a `#[sqlx::test]` or a shared test-pool fixture, add a round-trip test: `create_job` → `acquire_next_job` returns it as `running` → `complete_job` → `get_job` shows `done` with the result. If DB tests are gated/skipped in CI, mirror that gating exactly.

- [ ] **Step 4: Verify**

Run: `cargo test -p knowledge-server --lib canvas::skill_jobs`
Expected: PASS (or gated-skip consistent with other DB tests).

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas
git commit -m "feat(canvas): canvas_skill_jobs store (create/lease/finish)"
```

---

## Task 6: Registry-driven chat dispatch (delete `ChatCommand`, enqueue llm-skill jobs)

**Files:**
- Modify: `crates/knowledge-server/src/canvas/routes.rs`

**Context:** Today `chat_handler` parses `ChatCommand` via `strip_prefix` and branches to `run_search_skill` / `run_image_skill` / `build_analyze_node`, all inside one `async_stream::stream!` yielding a `node` event + `done`. We replace the parse+branch with a registry lookup by command. Builtins keep their existing sync bodies. `llm-skill` async skills validate input, create a job, yield a `running` html node, and finish the SSE immediately.

- [ ] **Step 1: Delete `enum ChatCommand` and `ChatCommand::parse`**

Remove the enum and its `parse` impl entirely. Add a small parser that splits the leading `/command` and the remaining argument text:

```rust
/// Split a chat message into (command, argument) when it starts with `/`.
/// `/ppt swiss style` -> (Some("ppt"), "swiss style"); `hello` -> (None, "hello").
fn parse_slash_command(message: &str) -> (Option<&str>, &str) {
    let trimmed = message.trim_start();
    let Some(rest) = trimmed.strip_prefix('/') else {
        return (None, message.trim());
    };
    match rest.split_once(char::is_whitespace) {
        Some((cmd, arg)) => (Some(cmd), arg.trim()),
        None => (Some(rest.trim()), ""),
    }
}
```

Add a unit test:
```rust
#[test]
fn parse_slash_command_splits_command_and_arg() {
    assert_eq!(parse_slash_command("/ppt swiss"), (Some("ppt"), "swiss"));
    assert_eq!(parse_slash_command("/ppt"), (Some("ppt"), ""));
    assert_eq!(parse_slash_command("no command"), (None, "no command"));
}
```

- [ ] **Step 2: Rewrite dispatch in `chat_handler`**

Inside the SSE stream, look up the command in `state.skill_registry`. Keep builtins pointed at their existing functions; branch async llm-skills to the new job path. Sketch (adapt to the exact existing stream body and event helpers `skill_node_event_name()` / `build_skill_node_done_payload`):

```rust
let (command, argument) = parse_slash_command(&request.message);

let descriptor = command.and_then(|c| state.skill_registry.by_command(c).cloned());

match descriptor {
    // No slash command → existing plain/chat behavior (unchanged).
    None => { /* existing Plain branch */ }

    Some(d) => {
        // Selection guard applies to any skill that requires it.
        if d.requires_selection() && request.selected_node_ids.is_empty() {
            yield sse_error("content empty");
            return;
        }

        match d.runtime {
            SkillRuntime::Builtin => match d.command.as_str() {
                "search"  => { /* existing run_search_skill(...) body */ }
                "image"   => { /* existing run_image_skill(...) body */ }
                "analyze" => { /* existing build_analyze_node(...) body */ }
                other => { yield sse_error(&format!("unknown builtin: {other}")); }
            },
            SkillRuntime::LlmSkill => {
                // 1. Gather selection text (requiresSelection already enforced).
                let selection_text = collect_selected_text(&state, &request).await;
                // 2. Create the job row.
                let node_id = new_node_id();
                let input = serde_json::json!({
                    "selection": selection_text,
                    "argument": argument,
                });
                let job_id = crate::canvas::skill_jobs::create_job(
                    &state.pool,
                    crate::canvas::skill_jobs::NewSkillJob {
                        canvas_id: request_canvas_id,
                        node_id: node_id.clone(),
                        skill_id: d.id.clone(),
                        input,
                        created_by: current_user_id,
                    },
                ).await.map_err(internal_error)?;
                // 3. Yield a running html node the frontend will place + poll.
                let node = serde_json::json!({
                    "type": "html",
                    "data": { "status": "running", "jobId": job_id.to_string() }
                });
                yield node_event(build_skill_node_done_payload(node, request.x, request.y));
                // 4. Done — SSE ends without waiting.
                yield done_event();
            }
        }
    }
}
```

Notes for the implementer:
- `collect_selected_text` should concatenate the text/markdown of the selected nodes — reuse whatever the existing `build_analyze_node` uses to read selected-node content; do not invent a new reader.
- `request_canvas_id` / `current_user_id`: pull from the handler's existing path param + auth extractor.
- `new_node_id()`: match the frontend's node-id format (the summary notes frontend assigns ids; if the backend must supply one, use `Uuid::new_v4().to_string()` and ensure the frontend accepts a server-provided id for the running node — see Task 11 Step 4).
- Keep `selected_node_ids` field name (`serde(rename = "selectedNodeIds")`) unchanged.

- [ ] **Step 3: Delete now-dead code**

Remove any `ChatCommand` references left in the file. `run_search_skill` / `run_image_skill` / `build_analyze_node` stay (called from the builtin arm).

- [ ] **Step 4: Verify**

Run: `cargo test -p knowledge-server --lib canvas`
Then: `cargo check -p knowledge-server`
Expected: parse test passes; crate compiles; existing canvas tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/knowledge-server/src/canvas/routes.rs
git commit -m "feat(canvas): registry-driven chat dispatch, delete ChatCommand"
```

---

## Task 7: Deck renderer (in-process bounded LLM loop — no Node, no subprocess)

**Files:**
- Create: `crates/knowledge-server/src/canvas/deck_renderer.rs`
- Modify: `crates/knowledge-server/src/canvas/mod.rs` (`pub mod deck_renderer;`)

**Context:** No subprocess, no tempfile, no new deps. Read the skill's `SKILL.md` guidance + its HTML template from the (read-only) `descriptor.dir`, inline both into a prompt, and call the existing `OpenAiCompatibleProvider::complete_text` (which already streams+aggregates so a long generation survives the gateway's ~60s idle cut — verified in `openai_compatible.rs`). Extract the HTML from the reply. The provider client's own request timeout (from the connection's `timeout_seconds`) bounds the call — no separate timeout wiring needed. `ProviderTextRequest`, `ProviderTextResponse`, `ProviderError`, `OpenAiCompatibleProvider` are all re-exported from `crate::providers`.

- [ ] **Step 1: Write the failing pure tests**

Create `crates/knowledge-server/src/canvas/deck_renderer.rs` with the pure helpers + tests first:

```rust
use std::path::Path;

use crate::providers::{OpenAiCompatibleProvider, ProviderError, ProviderTextRequest};
use crate::skills::SkillDescriptor;

#[derive(Debug)]
pub struct DeckOutcome {
    pub deck_html: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DeckError {
    #[error("skill file missing: {0}")]
    MissingFile(String),
    #[error("model returned no usable HTML")]
    NoHtml,
    #[error(transparent)]
    Provider(#[from] ProviderError),
}

/// System prompt = the skill's authoring guidance + the chosen HTML template,
/// both inlined. The model must return ONE self-contained HTML document.
pub fn build_system_prompt(skill_md: &str, template_html: &str) -> String {
    format!(
        "You are a slide-deck generator. Follow these authoring instructions exactly:\n\n\
         {skill_md}\n\n\
         Use this HTML template as the structural and visual baseline; keep its styling \
         and produce ONE self-contained HTML file (inline CSS/JS, no external assets):\n\n\
         <template>\n{template_html}\n</template>\n\n\
         Output ONLY the final HTML document, starting with <!DOCTYPE html>. Do not wrap \
         it in Markdown fences or add commentary."
    )
}

pub fn build_user_prompt(selection: &str, argument: &str) -> String {
    let requirements = if argument.trim().is_empty() {
        "No extra requirements.".to_string()
    } else {
        format!("Extra requirements: {}", argument.trim())
    };
    format!("Source content to turn into slides:\n\n{selection}\n\n{requirements}")
}

/// Strip an optional ```html … ``` fence and assert the payload looks like HTML.
pub fn extract_html(raw: &str) -> Result<String, DeckError> {
    let trimmed = raw.trim();
    let body = trimmed
        .strip_prefix("```html")
        .or_else(|| trimmed.strip_prefix("```"))
        .map(str::trim_start)
        .and_then(|rest| rest.strip_suffix("```"))
        .map(str::trim)
        .unwrap_or(trimmed);
    let lowered = body.to_ascii_lowercase();
    if lowered.starts_with("<!doctype") || lowered.contains("<html") {
        Ok(body.to_string())
    } else {
        Err(DeckError::NoHtml)
    }
}

fn read_skill_file(dir: &Path, name: &str) -> Result<String, DeckError> {
    std::fs::read_to_string(dir.join(name)).map_err(|_| DeckError::MissingFile(name.to_string()))
}

/// Read SKILL.md + template from the skill dir, run one completion, extract HTML.
pub async fn render_deck(
    descriptor: &SkillDescriptor,
    provider: &OpenAiCompatibleProvider,
    selection: &str,
    argument: &str,
) -> Result<DeckOutcome, DeckError> {
    let entry = descriptor.entry.as_deref().unwrap_or("SKILL.md");
    let skill_md = read_skill_file(&descriptor.dir, entry)?;
    // Design default: swiss template; fall back to the plain template.
    let template = read_skill_file(&descriptor.dir, "template-swiss.html")
        .or_else(|_| read_skill_file(&descriptor.dir, "template.html"))?;

    let response = provider
        .complete_text(ProviderTextRequest {
            system_prompt: build_system_prompt(&skill_md, &template),
            user_prompt: build_user_prompt(selection, argument),
        })
        .await?;

    let deck_html = extract_html(&response.text)?;
    Ok(DeckOutcome { deck_html })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_inlines_guidance_and_template() {
        let p = build_system_prompt("AUTHORING_RULES", "<div class=slide>");
        assert!(p.contains("AUTHORING_RULES"));
        assert!(p.contains("<div class=slide>"));
        assert!(p.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn user_prompt_handles_empty_and_present_argument() {
        assert!(build_user_prompt("content", "  ").contains("No extra requirements."));
        assert!(build_user_prompt("content", "swiss, dark").contains("Extra requirements: swiss, dark"));
    }

    #[test]
    fn extract_html_unwraps_fences_and_rejects_prose() {
        assert_eq!(
            extract_html("```html\n<!DOCTYPE html><html></html>\n```").unwrap(),
            "<!DOCTYPE html><html></html>"
        );
        assert_eq!(
            extract_html("<!doctype html><html>x</html>").unwrap(),
            "<!doctype html><html>x</html>"
        );
        assert!(matches!(extract_html("sorry, I can't do that"), Err(DeckError::NoHtml)));
    }
}
```

- [ ] **Step 2: Register module + run tests**

Add `pub mod deck_renderer;` to the canvas module root.
Run: `cargo test -p knowledge-server --lib canvas::deck_renderer`
Expected: PASS (three pure tests; no live LLM call).

- [ ] **Step 3: Verify compile**

Run: `cargo check -p knowledge-server`
Expected: compiles. (`ProviderTextRequest`/`ProviderTextResponse`/`ProviderError`/`OpenAiCompatibleProvider` are re-exported from `providers/mod.rs`; confirm the import path matches.)

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src/canvas/deck_renderer.rs crates/knowledge-server/src/canvas
git commit -m "feat(canvas): in-process deck renderer via complete_text"
```

---

## Task 8: Canvas skill worker (mirror `spawn_scheduler`) + concurrency cap + spawn in bootstrap

**Files:**
- Create: `crates/knowledge-server/src/canvas/skill_worker.rs`
- Modify: `crates/knowledge-server/src/canvas/mod.rs` (`pub mod skill_worker;`)
- Modify: `crates/knowledge-server/src/lib.rs` (`bootstrap_state`: spawn the worker alongside `spawn_scheduler`).

- [ ] **Step 1: Write the worker loop**

Model on `tasks/scheduler.rs::spawn_scheduler` (250ms poll, 30s lease). Add a global concurrency cap via a `tokio::sync::Semaphore` (max 2 concurrent deck-render jobs, so slow generations don't starve the pool):

```rust
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::canvas::skill_jobs;
use crate::skills::SkillRuntime;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const LEASE_SECONDS: i64 = 30;
const MAX_CONCURRENT: usize = 2;
const WORKER_OWNER: &str = "canvas-skill-worker";

pub fn spawn_skill_worker(state: crate::AppState) {
    let permits = Arc::new(Semaphore::new(MAX_CONCURRENT));
    tokio::spawn(async move {
        loop {
            // Only lease when a permit is free, so we never over-subscribe the LLM.
            let permit = match Arc::clone(&permits).acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };
            match skill_jobs::acquire_next_job(&state.pool, WORKER_OWNER, LEASE_SECONDS).await {
                Ok(Some(job)) => {
                    let state = state.clone();
                    tokio::spawn(async move {
                        run_one(state, job).await;
                        drop(permit); // release only after the job finishes
                    });
                }
                Ok(None) => {
                    drop(permit);
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
                Err(err) => {
                    tracing::error!(%err, "acquire_next_job failed");
                    drop(permit);
                    tokio::time::sleep(POLL_INTERVAL).await;
                }
            }
        }
    });
}

async fn run_one(state: crate::AppState, job: skill_jobs::SkillJob) {
    let result = execute(&state, &job).await;
    match result {
        Ok(value) => {
            if let Err(err) = skill_jobs::complete_job(&state.pool, job.id, value).await {
                tracing::error!(%err, job_id = %job.id, "complete_job failed");
            }
        }
        Err(message) => {
            let payload = serde_json::json!({ "message": message });
            if let Err(err) = skill_jobs::fail_job(&state.pool, job.id, payload).await {
                tracing::error!(%err, job_id = %job.id, "fail_job failed");
            }
        }
    }
}

async fn execute(state: &crate::AppState, job: &skill_jobs::SkillJob) -> Result<serde_json::Value, String> {
    let descriptor = state
        .skill_registry
        .all()
        .iter()
        .find(|s| s.id == job.skill_id && matches!(s.runtime, SkillRuntime::LlmSkill))
        .cloned()
        .ok_or_else(|| format!("unknown llm skill: {}", job.skill_id))?;

    let selection = job.input.get("selection").and_then(|v| v.as_str()).unwrap_or_default();
    let argument = job.input.get("argument").and_then(|v| v.as_str()).unwrap_or_default();

    // Reuse the same active-connection resolution as ingest.
    let connections = crate::providers::list_connections(&state.pool).await.map_err(|e| e.to_string())?;
    let active = crate::providers::resolve_active(&connections).ok_or("no active LLM connection")?;
    let provider = crate::providers::ActiveConnection::from(active).provider();

    let outcome = crate::canvas::deck_renderer::render_deck(
        &descriptor,
        &provider,
        selection,
        argument,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Store deck.html as a text/html asset owned by the job creator.
    let asset = crate::assets::store::NewAsset::new(
        job.created_by.clone(),
        "text/html".to_string(),
        outcome.deck_html.into_bytes(),
    );
    let asset_id = crate::assets::store::insert_asset(&state.pool, asset).await.map_err(|e| e.to_string())?;
    let url = crate::assets::store::asset_url(&asset_id);

    Ok(serde_json::json!({
        "assetId": asset_id,
        "url": url,
        "title": "PPT",
    }))
}
```

Implementer notes:
- `SkillJob` (Task 5) already carries `created_by: String` (baked in during T5 reconciliation); `render_job` receives `job: &skill_jobs::SkillJob`, so pass `job.created_by.clone()` into `NewAsset::new`. Confirm `NewAsset::new`'s owner param type is `String` (per `assets/store.rs`); if it takes `&str`, pass `&job.created_by` instead.
- `ActiveConnection::from(active).provider()` returns an `OpenAiCompatibleProvider` (same call `load_ingest_provider` uses). Pass `&provider` straight into `render_deck`; no raw-cred accessors needed.

- [ ] **Step 2: Spawn the worker in `bootstrap_state`**

Next to the existing `spawn_scheduler(state.clone())` (lib.rs ~line 50-52), add:
```rust
    crate::canvas::skill_worker::spawn_skill_worker(state.clone());
```
Add `pub mod skill_worker;` to the canvas module root.

- [ ] **Step 3: Verify**

Run: `cargo check -p knowledge-server` then `cargo test -p knowledge-server --lib`
Expected: compiles; suite passes.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src
git commit -m "feat(canvas): skill worker (lease loop + concurrency cap) + spawn"
```

---

## Task 9: `GET /api/canvas-skill-jobs/{id}` — poll endpoint

**Files:**
- Create/extend: `crates/knowledge-server/src/canvas/skill_jobs_routes.rs` (or add to `skill_jobs.rs`)
- Modify: router composition + canvas module root.

- [ ] **Step 1: Write the handler with ownership check**

```rust
use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::http::error::ApiError; // TEXT-convention codebase: this is the error type (see tasks/store.rs)

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillJobStatus {
    pub status: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
}

pub async fn get_skill_job(
    State(state): State<crate::AppState>,
    // confirm the auth extractor used elsewhere for current user id (user.id is a String)
    user: crate::auth::CurrentUser,
    Path(id): Path<String>,
) -> Result<Json<SkillJobStatus>, ApiError> {
    let job = crate::canvas::skill_jobs::get_job(&state.pool, &id)
        .await?
        .ok_or_else(|| ApiError::not_found("unknown job"))?;

    // Ownership: only the creator may poll. Adjust if canvas-level ACL is required.
    // Use the same not_found (never leak existence to non-owners).
    if job.created_by != user.id {
        return Err(ApiError::not_found("unknown job"));
    }

    Ok(Json(SkillJobStatus {
        status: job.status,
        result: job.result,
        error: job.error,
    }))
}
```

**Note:** confirm `ApiError`'s exact not-found constructor name/signature — `tasks/store.rs` uses `ApiError::bad_request("unknown task")` for a missing row, so if there is no `not_found`, use `ApiError::bad_request("unknown job")` for both branches (consistent non-leaking response). `get_job` returns `sqlx::Error` on the wire, so rely on the existing `impl From<sqlx::Error> for ApiError` (the `?` on the `.await`).

- [ ] **Step 2: Mount the route**

```rust
.route("/api/canvas-skill-jobs/{id}", axum::routing::get(crate::canvas::skill_jobs_routes::get_skill_job))
```
Match the axum path-param syntax the codebase already uses (`{id}` vs `:id`) and the same auth layer.

- [ ] **Step 3: Verify**

Run: `cargo check -p knowledge-server` then `cargo test -p knowledge-server --lib canvas`
Expected: compiles; tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/src
git commit -m "feat(canvas): GET /api/canvas-skill-jobs/{id} poll endpoint"
```

---

## Task 10: Bake guizang skill descriptor into the tree

**Files:**
- Create: `crates/knowledge-server/skills/guizang-ppt/skill.toml`
- Create: `crates/knowledge-server/skills/guizang-ppt/SKILL.md` (+ templates/references from upstream)

**Upstream:** https://github.com/op7418/guizang-ppt-skill — copy `SKILL.md`, `template.html`, `template-swiss.html`, and `references/` verbatim (per `feedback_copy_upstream`; do not rewrite the prompt engineering).

- [ ] **Step 1: Write the descriptor**

`crates/knowledge-server/skills/guizang-ppt/skill.toml`:
```toml
id          = "guizang-ppt"
command     = "ppt"
name        = "PPT 生成"
description = "把选中内容做成单文件 HTML 幻灯片"
runtime     = "llm-skill"
entry       = "SKILL.md"

[input]
source        = "selection"
required      = true
argument_hint = "可选:风格 / 要求"

[output]
node_type = "html"
async     = true
```

- [ ] **Step 2: Vendor the upstream skill files**

Copy `SKILL.md`, `template.html`, `template-swiss.html`, and `references/` from the upstream repo into `crates/knowledge-server/skills/guizang-ppt/` unchanged.

- [ ] **Step 3: Verify the loader picks it up (local run)**

Set `KNOWLEDGE_SKILLS_DIR=crates/knowledge-server/skills` and add a test that loads that directory and asserts a `ppt` command with `runtime=llm-skill` is present. (Use a path relative to `CARGO_MANIFEST_DIR` in the test.)
Run: `cargo test -p knowledge-server --lib skills`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/skills/guizang-ppt
git commit -m "feat(skills): vendor guizang-ppt skill + descriptor"
```

---

## Task 11: Frontend — `html` node type (zod enum, component, adapter, sizes)

**Files:**
- Modify: `apps/admin/src/features/canvas/types.ts` (add `"html"` to `canvasNodeSchema` enum)
- Create: `apps/admin/src/features/canvas/node-types/html.tsx`
- Modify: `apps/admin/src/features/canvas/canvas-board.tsx` (`nodeTypes` map)
- Modify: `apps/admin/src/features/canvas/page.tsx` (`NODE_SIZES`)

- [ ] **Step 1: Add `"html"` to the zod enum**

In `types.ts`:
```ts
type: z.enum(["note", "url", "kb", "ai_analyze", "ai_image", "search", "html"]),
```

- [ ] **Step 2: Write the html node component**

Create `node-types/html.tsx` (compose `NodeShell` like `search.tsx`; sandboxed iframe when done):
```tsx
import { NodeShell } from "./node-shell";

export interface HtmlNodeData {
  status: "running" | "done" | "error";
  jobId?: string;
  assetId?: string;
  url?: string;
  title?: string;
  error?: string;
}

export function HtmlNode({ data }: { data: HtmlNodeData }) {
  const status = data.status;
  return (
    <NodeShell
      title={data.title ?? "PPT"}
      status={status === "running" ? "running" : status === "error" ? "error" : "idle"}
    >
      {status === "done" && data.url ? (
        <div className="flex flex-col gap-2">
          <iframe
            sandbox=""
            src={data.url}
            title={data.title ?? "deck"}
            className="h-72 w-full rounded-md border border-border bg-white"
          />
          <a
            href={data.url}
            target="_blank"
            rel="noreferrer"
            className="text-xs text-muted-foreground underline"
          >
            在新标签打开
          </a>
        </div>
      ) : status === "error" ? (
        <p className="text-sm text-destructive">{data.error ?? "生成失败"}</p>
      ) : (
        <p className="text-sm text-muted-foreground">正在生成幻灯片…</p>
      )}
    </NodeShell>
  );
}
```
Confirm `NodeShell`'s exact prop names/status values against `node-shell.tsx` and match them.

- [ ] **Step 3: Register the adapter in `nodeTypes`**

In `canvas-board.tsx`, add `html` to the `nodeTypes` map mirroring the existing adapters (they inject `__cb`/`__model`/`__index`; the html node only needs `data`).

- [ ] **Step 4: Add `NODE_SIZES.html` in `page.tsx`**

Add an entry (e.g. `html: { width: 420, height: 340 }`) matching the iframe size.

- [ ] **Step 5: Verify types**

Run (CWD `apps/admin`): `npx tsc --noEmit`
Expected: clean (or only Task 12/13 pending references).

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/canvas/types.ts apps/admin/src/features/canvas/node-types/html.tsx apps/admin/src/features/canvas/canvas-board.tsx apps/admin/src/features/canvas/page.tsx
git commit -m "feat(canvas): html node type with sandboxed iframe"
```

---

## Task 12: Frontend — `GET /api/skills` query + `/` Command menu

**Files:**
- Modify: `apps/admin/src/features/shared/api.ts` (add `fetchSkills` + `skillMetadataSchema`)
- Create: `apps/admin/src/features/canvas/use-skills-query.ts`
- Modify: `apps/admin/src/features/canvas/chat-panel.tsx` (Command menu on leading `/`)
- Ensure shadcn `Command` primitive exists: `apps/admin/src/components/ui/command.tsx` (add via shadcn if absent — do NOT hand-roll).

- [ ] **Step 1: Add the API + schema**

In `api.ts`:
```ts
export const skillMetadataSchema = z.object({
  command: z.string(),
  name: z.string(),
  description: z.string(),
  requiresSelection: z.boolean(),
  argumentHint: z.string().nullable().optional(),
  outputNodeType: z.string(),
});
export type SkillMetadata = z.infer<typeof skillMetadataSchema>;

export async function fetchSkills(): Promise<SkillMetadata[]> {
  const res = await apiFetch("/api/skills"); // match the existing fetch helper name
  return z.array(skillMetadataSchema).parse(await res.json());
}
```

- [ ] **Step 2: react-query hook**

`use-skills-query.ts`:
```ts
import { useQuery } from "@tanstack/react-query";
import { fetchSkills } from "@/features/shared/api";

export function useSkillsQuery() {
  return useQuery({ queryKey: ["skills"], queryFn: fetchSkills, staleTime: 5 * 60_000 });
}
```

- [ ] **Step 3: Ensure the shadcn Command primitive is present**

If `apps/admin/src/components/ui/command.tsx` doesn't exist, add it via the shadcn CLI (`npx shadcn@latest add command`) — per `feedback_use_shadcn`, compose the primitive, don't build a custom menu.

- [ ] **Step 4: Wire the menu in `chat-panel.tsx`**

- Track the input value; when it starts with `/`, show a shadcn `Command` popover listing `useSkillsQuery().data`.
- Each item shows `name` + `description`; if `requiresSelection` and there is no current selection, disable it (or show "需选中节点") — the panel must know the current selection count (thread a `hasSelection`/`selectedCount` prop from `page.tsx`).
- On select, set the input to `/${command} ` and swap the placeholder to `argumentHint`.
- Submit path is unchanged (`streamCanvasChat`); it already carries `selectedNodeIds`.

- [ ] **Step 5: Verify**

Run (CWD `apps/admin`): `npx tsc --noEmit`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add apps/admin/src/features/canvas/chat-panel.tsx apps/admin/src/features/canvas/use-skills-query.ts apps/admin/src/features/shared/api.ts apps/admin/src/components/ui/command.tsx
git commit -m "feat(canvas): / command menu from GET /api/skills"
```

---

## Task 13: Frontend — job polling + node backfill

**Files:**
- Create: `apps/admin/src/features/canvas/use-skill-job-poll.ts`
- Modify: `apps/admin/src/features/canvas/page.tsx` (poll running html nodes; patch on done/error; frontend is sole writer)

**Context (spec §4):** the backend never writes the canvas document. The frontend polls each running html node's `jobId` and patches the node via the existing `patchNodeData`, then persists the canvas the same way other edits do.

- [ ] **Step 1: Add the fetch + schema in `api.ts`**

```ts
export const skillJobStatusSchema = z.object({
  status: z.enum(["queued", "running", "done", "error"]),
  result: z.object({ assetId: z.string(), url: z.string(), title: z.string().optional() }).nullish(),
  error: z.object({ message: z.string() }).nullish(),
});
export async function fetchSkillJob(id: string) {
  const res = await apiFetch(`/api/canvas-skill-jobs/${id}`);
  return skillJobStatusSchema.parse(await res.json());
}
```

- [ ] **Step 2: Poll hook**

`use-skill-job-poll.ts` — given a list of `{ nodeId, jobId }` for running html nodes, poll each (e.g. every 2s) until `done`/`error`, invoking a callback to patch the node. Use `useQuery` with `refetchInterval` per job, or a single interval effect. Stop polling once terminal.

- [ ] **Step 3: Wire into `page.tsx`**

- Derive running html nodes from canvas state (`node.type === "html" && node.data.status === "running" && node.data.jobId`).
- On `done`: `patchNodeData(nodeId, { status: "done", assetId, url, title })` then persist the canvas (same flush/save path as other edits).
- On `error`: `patchNodeData(nodeId, { status: "error", error: message })` then persist.
- On the SSE `node` event that first creates the running html node (from Task 6), place it via the existing `addSkillNode` chain so it gets a position + id.

- [ ] **Step 4: Verify**

Run (CWD `apps/admin`): `npx tsc --noEmit` and `npx vitest run src/features/canvas`
Expected: clean; canvas tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/admin/src/features/canvas/use-skill-job-poll.ts apps/admin/src/features/canvas/page.tsx apps/admin/src/features/shared/api.ts
git commit -m "feat(canvas): poll skill jobs and backfill html node"
```

---

## Task 14: Docker image — bake the read-only skills dir

The LLM loop runs in-process (no Node, no subprocess, no `@openai/codex`), so the image needs
**no new runtime dependencies** — only the read-only `skills/` baseline copied in and pointed at by
`KNOWLEDGE_SKILLS_DIR`.

**Files:**
- Modify: `crates/knowledge-server/Dockerfile` (or the repo's server Dockerfile — confirm path)
- Modify: `docker-compose.yml` (set `KNOWLEDGE_SKILLS_DIR`)

- [ ] **Step 1: Copy the skills dir into the runtime stage**

In the final runtime stage of the server image, copy the skills baseline and set the env var:
```dockerfile
# Bake the read-only skills baseline (registry source at runtime)
COPY crates/knowledge-server/skills /app/skills
ENV KNOWLEDGE_SKILLS_DIR=/app/skills
```
(Confirm the base image + copy paths; match the existing Dockerfile's WORKDIR and stage names.)

- [ ] **Step 2: compose env**

In `docker-compose.yml` server service, ensure `KNOWLEDGE_SKILLS_DIR=/app/skills` is set (or rely on
the `ENV` above). No new secret is added — the deck renderer reuses the active `provider_connections`
creds via `list_connections`/`resolve_active` at runtime.

- [ ] **Step 3: Build**

Run: `docker compose build server`
Expected: image builds; `/app/skills/guizang-ppt/skill.toml` is present (spot check:
`docker compose run --rm server ls /app/skills/guizang-ppt`).

- [ ] **Step 4: Commit**

```bash
git add crates/knowledge-server/Dockerfile docker-compose.yml
git commit -m "build(server): bake guizang skill dir into image"
```

---

## Task 15: Full verification + end-to-end acceptance

- [ ] **Backend** (from `E:\Projects\Js\knowledge`)
  - `cargo test -p knowledge-server --lib` — all pass
  - `cargo check -p knowledge-server` — clean

- [ ] **Frontend** (from `apps/admin`)
  - `npx vitest run` — all pass
  - `npx tsc --noEmit` — clean

- [ ] **End-to-end** (`docker compose build server admin && docker compose up -d`)
  - Select a note node → type `/ppt <要求>` → a `running` html node appears immediately → after the worker finishes, the node flips to `done` and the iframe renders the deck.
  - Select nothing → `/ppt` → chat shows `content empty`; no node created.
  - `/` menu lists all skills (including folded-in search/image/analyze); data comes from `GET /api/skills` and never exposes `runtime`/`entry`.
  - Add a new skill by dropping `skills/<id>/skill.toml` (one builtin-style or llm-skill descriptor) → it appears in the menu with zero routing-code changes.

---

## Self-review notes (author checklist run)

- **Spec coverage:** registry (T2–T3), `GET /api/skills` (T4), job table (T1) + store (T5), dispatch replacing `ChatCommand` (T6), in-process LLM deck renderer (T7) + lease-loop worker with concurrency cap (T8), poll endpoint (T9), guizang vendor (T10), html node (T11), `/` menu (T12), poll+backfill (T13), skills-dir bake (T14), e2e (T15) — every spec component maps to a task.
- **Type consistency:** `SkillDescriptor` fields (`input.source`, `input.required`, `output.node_type`, `output.async`) are used identically in T2/T3/T4/T7/T8; `SkillJob` gains `created_by` (flagged in T8 Step 1 with the instruction to backfill T5's struct + queries). `SkillMetadata` camelCase keys (`requiresSelection`, `argumentHint`, `outputNodeType`) match the frontend `skillMetadataSchema` in T12.
- **Known-verify points (call out, don't guess):** exact `AppState` location + auth extractor names; `providers::ActiveConnection::from(active).provider()` return type (`OpenAiCompatibleProvider`, same as `load_ingest_provider`); axum path-param syntax (`{id}` vs `:id`); `NodeShell` prop/status names; the `apiFetch` helper name in `api.ts`. Each is flagged inline in its task.
- **No-legacy:** `enum ChatCommand` + `ChatCommand::parse` deleted in T6 (no dual path), matching `feedback_no_legacy_compat`.
- **shadcn:** `/` menu uses the shadcn `Command` primitive (T12 Step 3), matching `feedback_use_shadcn`.
- **upstream:** guizang files vendored verbatim (T10), matching `feedback_copy_upstream`.
