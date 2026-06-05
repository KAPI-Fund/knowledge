# Phase Two Worker And Query Design

## Scope

This spec defines phase two, subproject 2.1:

- server-embedded task orchestration
- asynchronous project operations execution
- asynchronous Query/RAG execution
- provider abstraction with a single OpenAI-compatible implementation
- admin UI extensions for query, task monitoring, provider settings, and audit visibility

This spec does not include a standalone `knowledge-worker` process, streaming responses, DAG scheduling, or multi-provider execution.

## Goals

Phase one established the operational shell, persisted task summaries, audit logs, settings, and project-level admin pages. Phase two subproject 2.1 should turn those operational views into a real execution system.

The expected outcome is:

- `knowledge-server` accepts work requests and turns them into durable tasks
- a built-in worker loop executes those tasks
- task state survives restarts and re-enters the queue automatically
- Query/RAG no longer returns synchronously from the main query endpoint
- a project-level query console can submit async query jobs and render their results

## Non-Goals

This subproject intentionally excludes:

- extracting the worker into a dedicated process
- SSE or token streaming
- chat-style multi-turn conversations
- multiple provider implementations
- vector database adoption
- complex workflow dependencies between tasks

## Recommended Approach

Three approaches were considered.

### Approach 1: Server-Embedded Orchestrator

Run a scheduler and executor loop inside `knowledge-server`. The HTTP layer creates durable tasks in PostgreSQL, Redis provides wake-up signaling, and the in-process worker consumes queued tasks.

Pros:

- shortest path from current codebase to real execution
- minimal architecture churn
- directly compatible with the existing admin UI and persisted task model
- straightforward upgrade path to a future standalone worker

Cons:

- execution and API share one process boundary
- runtime scaling is coarser than a separate worker process

### Approach 2: Redis-First Queue

Use Redis as the source-of-truth queue and PostgreSQL only for task summaries and results.

Pros:

- fast queue semantics
- good fit for future high-throughput task execution

Cons:

- harder restart recovery and idempotency model
- state consistency becomes more complex than current needs justify

### Approach 3: Workflow Engine First

Introduce explicit task graphs, dependencies, and orchestration semantics from the start.

Pros:

- richest long-term orchestration model

Cons:

- significantly exceeds the scope of the next useful slice
- delays Query/RAG and real execution

### Recommendation

Use Approach 1.

It best matches the current repository state, the user-confirmed requirement for an embedded worker, and the need to ship a durable async Query/RAG path without introducing unnecessary queue infrastructure complexity.

## Architecture

The system should be organized into the following units:

- `HTTP ingress`
- `task store`
- `scheduler`
- `executor registry`
- `task executors`
- `provider abstraction`
- `result serializer`

### HTTP Ingress

The HTTP layer is responsible for:

- validating input and authorization
- creating tasks
- returning `taskId` and initial status
- exposing task listing, detail, retry, and cancel endpoints
- exposing query-task creation and result polling endpoints

It must not execute heavy work inline.

### Task Store

PostgreSQL is the source of truth for:

- task payload
- task status
- attempts
- lease metadata
- result payload
- error payload
- timing fields

Redis is not a source of truth. It is only used for:

- worker wake-up signaling
- short-lived coordination support
- future lightweight progress notification support

### Scheduler

The scheduler lives inside `knowledge-server` and is responsible for:

- startup recovery
- polling or wake-up-driven task acquisition
- lease assignment
- retry scheduling
- cancellation visibility

### Executor Registry

The executor registry maps a task type to a concrete executor. This keeps orchestration separate from task behavior and allows later extraction into a standalone worker without redesigning task semantics.

### Provider Abstraction

The Query/RAG executor depends on a provider port rather than a concrete vendor API. Phase two includes one OpenAI-compatible adapter, but the interface boundary should be stable enough to support future providers without changing task shape.

## Task Model

All project operations and Query/RAG jobs should use one durable task model.

### Task Types

Initial task types:

- `project.import_source`
- `project.rescan_sources`
- `project.delete_source`
- `project.ingest_source`
- `project.update_review`
- `query.answer`

### Task Fields

Each task should include at least:

- `id`
- `project_id`
- `task_type`
- `status`
- `payload`
- `result`
- `error`
- `attempt_count`
- `max_attempts`
- `created_by`
- `created_at`
- `started_at`
- `finished_at`
- `lease_owner`
- `lease_expires_at`
- `next_retry_at`

`payload`, `result`, and `error` should be JSONB fields so each executor can use structured data without schema churn.

### Status Model

The initial status set should be:

- `queued`
- `running`
- `succeeded`
- `failed`
- `cancelled`
- `retry_waiting`

### State Transitions

Allowed transitions:

- new task: `queued`
- acquired by worker: `queued -> running`
- success: `running -> succeeded`
- retryable failure: `running -> retry_waiting -> queued`
- terminal failure: `running -> failed`
- manual cancel: `queued/running/retry_waiting -> cancelled`
- manual retry: `failed/cancelled -> queued`
- restart recovery: `queued/running/retry_waiting -> queued`

The system should use lease-based recovery instead of trusting in-memory execution state.

## Execution Flow

### Standard Flow

1. Client calls an API endpoint.
2. `knowledge-server` validates permissions and input.
3. The API writes a `queued` task row in PostgreSQL.
4. The API records a corresponding audit event.
5. The API publishes a lightweight wake-up signal through Redis.
6. The in-process scheduler attempts atomic task acquisition.
7. The scheduler sets lease metadata and transitions the task to `running`.
8. The executor registry dispatches to a concrete executor.
9. The executor writes `result` or `error` and updates final state.
10. Clients poll task detail endpoints until the task is terminal.

### Atomic Acquisition

Task pickup must be done through atomic SQL. The implementation must not:

- select a candidate task in one query
- then update it in a second step

That pattern risks double execution as soon as concurrency increases.

## Restart Recovery

At startup, the scheduler scans all recoverable tasks:

- `queued`
- `running`
- `retry_waiting`

Recovery behavior:

- expired or previously running tasks are reset to `queued`
- queued tasks remain executable
- retry-waiting tasks re-enter the queue once `next_retry_at` is due

The user explicitly chose automatic re-enqueue on restart, so this system should treat process restarts as replay-safe by design.

## Idempotency Requirements

Because restart recovery re-enqueues unfinished work, executors must be idempotent enough to tolerate replay.

Minimum expectations:

- `project.import_source` should not corrupt files or produce inconsistent final state when replayed
- `project.ingest_source` should not append unbounded duplicate derived artifacts
- `query.answer` should overwrite or finalize its own task result only, without external side effects

Where exact idempotency is difficult, the executor should prefer deterministic overwrite semantics over append semantics.

## Query/RAG Contract

Query/RAG becomes fully asynchronous in phase two.

### API Shape

Primary creation endpoint:

- `POST /api/projects/:projectId/query-tasks`

Request fields:

- `query`
- `topK`
- optional `filters`

Response:

- `taskId`
- `status`

The phase-one synchronous query path should no longer be the primary contract for the admin UI.

### Query Task Payload

The `query.answer` payload should include at least:

- `query`
- `top_k`
- `provider_mode`
- `language`
- `requested_by`

### Query Task Result

The `result` field should contain:

- `answer`
- `citations`
- `context_summary`
- `model`
- `provider`
- `usage`
- `completed_at`

Each citation should include:

- `path`
- `title`
- `snippet`
- `score`

### Retrieval Strategy

Phase two should continue to use the existing project-local search logic for retrieval context. The LLM provider should generate an answer from that retrieved context instead of inventing sources.

This keeps the implementation aligned with the current repository behavior and minimizes architectural churn.

## Provider Abstraction

Phase two should introduce a narrow provider port:

- `ProviderClient::answer_query(request) -> ProviderAnswer`

Phase two should implement one concrete adapter:

- `OpenAiCompatibleProvider`

### Provider Configuration

Provider settings should support:

- `base_url`
- `api_key`
- `model`
- optional `embedding_model`
- `timeout_seconds`

The configuration layer should support future providers, but only one provider must be executable in this phase.

### Provider Error Normalization

Provider failures should be converted into a stable internal error shape:

- `code`
- `message`
- `retryable`
- `provider_status`

That error payload is what gets persisted to the task row and rendered by the admin UI.

## Retry Semantics

Retry behavior should be intentionally simple in this phase.

### Query Tasks

Retryable:

- provider 429
- provider 5xx
- transport failures
- timeouts

Non-retryable:

- invalid task payload
- missing provider configuration
- unsupported model selection

### Project Tasks

Retryable:

- transient lock conflicts
- recoverable external coordination failures

Non-retryable:

- missing source file
- malformed input payload
- invalid project-relative path

### Attempt Policy

Each task should carry:

- `attempt_count`
- `max_attempts`
- `next_retry_at`

When `attempt_count` exceeds `max_attempts`, the task becomes `failed`.

## Admin UI Changes

Phase two should extend the admin app in focused ways that directly support the new execution model.

### Query Workbench

Add a project-level route:

- `/projects/:projectId/query`

The page should provide:

- query input
- async submission
- current task polling
- result rendering
- citations rendering
- error rendering
- retry action

This is a task-oriented query console, not a synchronous chat UI.

### Tasks Page Enhancements

The tasks page should add:

- task type filtering
- status filtering
- attempt counters
- last error summary
- direct navigation for `query.answer` results
- retry for failed tasks
- cancel for queued or running tasks

### Settings Page Enhancements

The settings page should add provider configuration fields:

- provider mode
- base URL
- API key
- model
- timeout
- optional embedding model

The API key should not be returned in plaintext once stored. The UI should only show configured vs. not configured state.

### Audit Page Enhancements

The audit view should expose:

- query task created
- query task succeeded
- query task failed
- worker retried task
- worker recovered task on startup

## Error Handling

### API Layer

Synchronous HTTP errors should only represent request creation failure:

- authorization failure
- invalid payload
- missing provider configuration
- unsupported operation

If task creation succeeds, execution failures should appear through task state, not through the original HTTP response.

### Worker Layer

Worker failures should always persist structured error metadata and state transitions. No executor should fail silently.

### Provider Layer

Provider-specific errors must be normalized before persistence so the admin UI does not depend on vendor-specific response structures.

## Testing Strategy

Tests should remain focused on critical behavior.

### Rust Integration Tests

Cover:

- create and complete a `query.answer` task
- task polling returns final answer and citations
- missing provider configuration produces terminal failure
- retryable provider failure enters retry flow
- startup recovery re-enqueues `queued`, `running`, and `retry_waiting` tasks

### Rust Unit Tests

Cover:

- scheduler acquisition and lease behavior
- task state transitions
- provider adapter request and response mapping

### Admin Unit Tests

Cover:

- query page task submission and result polling
- tasks page retry and cancel actions
- provider settings write path

### Playwright Tests

Cover:

- configure provider
- submit async query task
- wait for completion
- render answer and citations
- inspect resulting task record

## Delivery Boundary

Phase two subproject 2.1 is complete when:

- `knowledge-server` runs an embedded worker loop
- project operations can be executed through durable tasks
- query requests create async `query.answer` tasks
- query results are persisted and retrievable through task detail
- the admin UI exposes a query workbench and enhanced task operations
- restart recovery re-queues unfinished tasks
- the provider abstraction exists and one OpenAI-compatible adapter is working

The following remain explicitly out of scope for this spec:

- standalone `knowledge-worker`
- SSE or token streaming
- multi-provider execution
- workflow graphs or DAG scheduling
- vector database adoption
- multi-turn chat memory
