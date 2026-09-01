# AGENTS.md

## Project Identity

**Atmos Video** — video sharing & playback platform. Despite the directory name (`atmos-android`), this is **not** an Android project. It's a two-part monorepo: Rust/Axum backend + React/Vite webapp.

## Quick Commands

### Backend (from `backend/`)

```bash
cargo fmt --check          # format check
cargo clippy -- -D warnings # lint (warnings are errors)
cargo build --release      # build
cargo test                 # unit tests only (no DB needed)
DATABASE_URL="postgres://kuaile@localhost:5432/atmos_video" cargo test  # full tests (needs PostgreSQL)
cargo test -- --test-threads=1  # serial (for DB-dependent tests)
cargo test --test openapi_route_tests  # route↔OpenAPI consistency (no DB needed)
```

### Webapp (from `webapp/`)

```bash
npm install
npm run dev    # dev server at localhost:5173, proxies /videos /auth /admin etc. to :8082
npm run build  # tsc + vite build → dist/
npm test       # Vitest unit tests (src/test/)
```

## Critical Gotchas

### Migrations are auto-discovered

Migrations in `backend/migrations/` are auto-discovered at runtime. Files are sorted by filename and applied in order. Just drop a new `.sql` file into `migrations/` — no need to register it. The `_schema_migrations` table tracks which have been applied. Override the directory via `MIGRATIONS_DIR` env var (defaults to `$CARGO_MANIFEST_DIR/migrations/`).

### CI order matters

GitHub Actions (`.github/workflows/ci.yml`): `fmt → clippy → build → test`. Mirrors this locally before pushing.

### OpenAPI docs must stay in sync with routes

`backend/tests/openapi_route_tests.rs` asserts the OpenAPI spec (`src/openapi.rs`) matches every route registered in `app.rs` (bidirectional). **Any route added/removed/changed in `app.rs` requires a matching update in `src/openapi.rs`** (and the mirrored `registered_routes()` list in the test). Intentionally undocumented routes go into `INTENTIONALLY_OMITTED` with a reason. Runs offline, no DB needed.

### DB integration tests are gated behind DATABASE_URL

Tests in `backend/tests/` that need PostgreSQL (`http_integration.rs`, `integration_auth.rs`, `integration_videos.rs`, `integration_test_helpers.rs`) silently skip when `DATABASE_URL` is unset, so `cargo test` alone can look greener than it is. CI always runs them (postgres service container, `--test-threads=1`).

### Registration is disabled by default

`REGISTRATION_ENABLED=false` (default). Set to `true` via env to allow new user signups. Admin user must be created first.

### Auth tokens are NOT UUIDs

Tokens are 256-bit alphanumeric strings, 7-day expiry. Don't assume UUID format.

## Architecture (Backend)

```
main.rs → build_router(app.rs) → middleware → handlers → services → repositories → SQLx
```

Layering rule: **handlers never access the database directly** — they go through service → repository.

- Global middleware, execution order (outermost first): `security_headers` → `inject_state` → `resolve_tenant` → `request_id` → `request_log` → `TraceLayer` → `CompressionLayer` → CORS. Note `inject_state` must run before `request_log` (it needs `AppState` for user lookup)
- Route-specific: `bearer_auth` → `role_auth(N)` → `admin_auth` (layered per route group)
- Upload route has `DefaultBodyLimit::disable()`; upload and admin routes use 2-hour timeout, all other routes use 30-second timeout
- State (`AppState` in `src/state.rs`) is `Arc`-wrapped, injected into request extensions

## Architecture (Webapp)

React 18 SPA with lazy-loaded pages (`App.tsx`). Vite build output served by backend at `/webapp/`.

- `vite.config.ts` sets `base: '/webapp/'` — important for production asset paths
- Dev proxy targets `http://localhost:8082` for all API paths
- TypeScript strict mode: `noUnusedLocals`, `noUnusedParameters`, `noUncheckedIndexedAccess`
- All user-visible text is Chinese (zh-CN); i18n strings live in `src/locales/` (zh-CN / en-US)
- `npm test` runs Vitest suites in `src/test/`

## Environment Variables

The backend reads from `.env` (loaded by `dotenvy`). Key vars with non-obvious defaults:

| Variable | Default | Notes |
|---|---|---|
| `DATABASE_URL` | `postgres://kuaile@localhost:5432/atmos_video` | |
| `SERVER_PORT` | `8082` | |
| `PUBLIC_URL` | (required) | External-accessible base URL for share links, hotlink protection, and HTTPS redirects |
| `WEBAPP_ROOT` | `./webapp/dist` | Env var name is `WEBAPP_ROOT`, not `WEBA_ROOT` |
| `MEDIA_ROOT` | `./media` | |
| `MIGRATIONS_DIR` | `$CARGO_MANIFEST_DIR/migrations/` | Override for auto-discovered migrations |
| `REGISTRATION_ENABLED` | `false` | Controls public signup |
| `CORS_ORIGIN` | (empty) | Comma-separated allowed origins |
| `COOKIE_SECURE` | `true` | |
| `APP_ENV` | `production` | `production` enables HTTPS redirect + strict CORS; `development` relaxes |
| `ALLOW_FIRST_USER_ADMIN` | `false` | First registered user never auto-becomes admin by default |
| `UPLOAD_QUOTA_BYTES` | `53687091200` | Per-user storage quota (50 GB); `0` disables |
| `SMTP_*` | (empty) | SMTP creds — required for password reset / email verification |
| `REDIS_URL` | (empty) | Optional Redis; falls back to in-memory rate limiting/cache |
| `ADMIN_IP_WHITELIST` | (empty) | Comma-separated IPs allowed to hit `/admin/*`; empty = unrestricted |
| `TRUSTED_PROXY` | `0` | Trust `X-Forwarded-For`/`cf-connecting-ip` from any peer |
| `HASHID_SALT` | baked-in | Must be stable across restarts; set a random value in production |
| `RUST_LOG` | `info` | tracing-subscriber EnvFilter; GET/HEAD 请求日志在 `debug` 级别(需 `RUST_LOG=debug` 才可见),写操作与慢/错误请求在 `info`+ |

## Useful Files

- `backend/run_backend.command` — macOS helper: start/stop/restart backend with PostgreSQL auto-start
- `backend/src/db.rs:6-36` — migration auto-discovery (`migrations_dir_or_default`, `discover_migrations`)
- `backend/src/app.rs` — route definitions with middleware layers (global stack at the bottom of `build_router`; route groups: public → auth → video → playback → upload → admin → internal → docs)
- `backend/src/openapi.rs` + `backend/tests/openapi_route_tests.rs` — hand-written OpenAPI spec + route↔spec consistency test (keep in sync with `app.rs`)
- `backend/migrations/` — 40 auto-discovered migrations; latest is `040_search_suggest_trgm_and_recommendation_views.sql` (search-suggestion trigram index + recommendation views index)
- `backend/tests/` — DB-gated integration tests: `http_integration.rs`, `integration_auth.rs`, `integration_videos.rs`, `integration_test_helpers.rs`; offline tests: `openapi_route_tests.rs`, `service_content_tests.rs`, `service_media_tests.rs`, `service_misc_tests.rs`, `test_*.rs`
- `webapp/vite.config.ts` — dev proxy config and build settings
- `webapp/src/test/` — Vitest unit tests (`npm test`)
