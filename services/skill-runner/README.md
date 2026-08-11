# skill-runner

HTTP sidecar between the Rust backend and CubeSandbox. On each render request it spins up a CubeSandbox micro-VM, writes input files, runs the `codex` CLI inside the VM to agentically generate an HTML deck, reads the output, and returns it to the caller. On error it returns a structured `{stage, message}` JSON body.

## Environment variables

| Variable | Required | Description |
|---|---|---|
| `CUBE_TEMPLATE_ID` | Yes | CubeSandbox VM template ID |
| `CUBE_API_URL` | Yes (via SDK) | CubeSandbox gateway URL; also accepted as `E2B_API_URL` |
| `CUBE_API_KEY` | Yes (via SDK) | API key; also accepted as `E2B_API_KEY` |
| `SKILL_RUNNER_ADDR` | No | Listen address (default `:4600`) |
| `SKILLS_VERSION` | No | Logged at startup for tracing |

`CUBE_API_URL` and `CUBE_API_KEY` are read directly by the SDK via `NewConfigFromEnv()`; the service itself does not reference them explicitly.

## API

### POST /render

Request body (`RenderRequest`):

```json
{
  "skill_id": "string",
  "selection": "string",
  "argument": "string",
  "provider": {
    "base_url": "string",
    "api_key": "string",
    "model": "string"
  }
}
```

Success response (`RenderedDeck`):

```json
{ "deck_html": "<html>..." }
```

Error response:

```json
{ "stage": "create|codex|timeout|output", "message": "description" }
```

HTTP status is 500 for most errors, 504 for timeout stage.

### GET /health

Returns `200 ok` when the process is up. No dependency checks.

## Local dev

```bash
go test ./...
```

## Build

```bash
docker build -t skill-runner .
```

Build image is `golang:1.25` (matches the `go 1.25.0` directive in `go.mod`). Final image is `gcr.io/distroless/static-debian12`.

## Deployment

See `docs/skill-runner/deploy.md`.
