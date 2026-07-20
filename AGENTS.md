# AGENTS.md

## Project Identity

**Atmos Video** — LAN video playback platform. Despite the directory name (`atmos-android`), this is **not** an Android project. It's a two-part monorepo: Rust/Axum backend + React/Vite webapp.

## Quick Commands

### Backend (from `backend/`)

```bash
cargo fmt --check          # format check
cargo clippy -- -D warnings # lint (warnings are errors)
cargo build --release      # build
cargo test                 # unit tests only (no DB needed)
DATABASE_URL="postgres://kuaile@localhost:5432/lan_video" cargo test  # full tests (needs PostgreSQL)
cargo test -- --test-threads=1  # serial (for DB-dependent tests)
```

### Webapp (from `webapp/`)

```bash
npm install
npm run dev    # dev server at localhost:5173, proxies /videos /auth /admin etc. to :8082
npm run build  # tsc + vite build → dist/
```

## Critical Gotchas

### Migrations are auto-discovered

Migrations in `backend/migrations/` are auto-discovered at runtime. Files are sorted by filename and applied in order. Just drop a new `.sql` file into `migrations/` — no need to register it. The `_schema_migrations` table tracks which have been applied. Override the directory via `MIGRATIONS_DIR` env var (defaults to `$CARGO_MANIFEST_DIR/migrations/`).

### CI order matters

GitHub Actions (`.github/workflows/ci.yml`): `fmt → clippy → build → test`. Mirrors this locally before pushing.

### Registration is disabled by default

`REGISTRATION_ENABLED=false` (default). Set to `true` via env to allow new user signups. Admin user must be created first.

### Auth tokens are NOT UUIDs

Tokens are 256-bit alphanumeric strings, 7-day expiry. Don't assume UUID format.

## Architecture (Backend)

```
main.rs → build_router(app.rs) → middleware → handlers → services → repositories → SQLx
```

Layering rule: **handlers never access the database directly** — they go through service → repository.

- Middleware stack (in order): `request_id` → `request_log` → `TraceLayer` → CORS → inject_state → `security_headers`
- Route-specific: `bearer_auth` → `role_auth(N)` → `admin_auth` (layered per route group)
- Upload route has `DefaultBodyLimit::disable()` and 2-hour timeout; all other routes use 30-second timeout
- State (`AppState` in `src/state.rs`) is `Arc`-wrapped, injected into request extensions

## Architecture (Webapp)

React 18 SPA with lazy-loaded pages (`App.tsx`). Vite build output served by backend at `/webapp/`.

- `vite.config.ts` sets `base: '/webapp/'` — important for production asset paths
- Dev proxy targets `http://localhost:8082` for all API paths
- TypeScript strict mode: `noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`
- All user-visible text is Chinese (zh-CN)

## Environment Variables

The backend reads from `.env` (loaded by `dotenvy`). Key vars with non-obvious defaults:

| Variable | Default | Notes |
|---|---|---|
| `DATABASE_URL` | `postgres://kuaile@localhost:5432/lan_video` | |
| `SERVER_PORT` | `8082` | |
| `PUBLIC_URL` | (empty) | External-accessible base URL for share links. If unset, share URLs are auto-built from the request's `Host` header |
| `WEBAPP_ROOT` | `./webapp/dist` | Env var name is `WEBAPP_ROOT`, not `WEBA_ROOT` |
| `MEDIA_ROOT` | `./media` | |
| `MIGRATIONS_DIR` | `$CARGO_MANIFEST_DIR/migrations/` | Override for auto-discovered migrations |
| `REGISTRATION_ENABLED` | `false` | Controls public signup |
| `CORS_ORIGIN` | `http://localhost:8082` | |
| `COOKIE_SECURE` | `false` | |
| `RUST_LOG` | `info` | tracing-subscriber EnvFilter |

## Useful Files

- `backend/run_backend.command` — macOS helper: start/stop/restart backend with PostgreSQL auto-start
- `backend/src/db.rs:6-36` — migration auto-discovery (`get_migrations_dir`, `discover_migrations`)
- `backend/src/app.rs:48-354` — full route definitions with middleware layers
- `webapp/vite.config.ts` — dev proxy config and build settings
