# Knowledge Local Docker Stack

## Start

```bash
docker compose up --build
```

## URLs

- Admin UI: `http://127.0.0.1:4173`
- Backend API: `http://127.0.0.1:4001`
- Postgres: `127.0.0.1:55432`
- Redis: `127.0.0.1:56379`

## Default Login

- Username: `admin`
- Password: `secret-password`

## Real Provider

Set `KNOWLEDGE_PROVIDER_API_KEY` before starting the stack if you want live provider-backed query and ingest flows.

## Stop

```bash
docker compose down -v
```
