# Code Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all issues identified in the code review — clippy/fmt errors, token lifecycle, dead code, security hardening.

**Architecture:** Minimal targeted fixes across handlers, middleware, services, and repositories. No structural changes.

**Tech Stack:** Rust / Axum / SQLx / PostgreSQL

---

### Task 1: Fix Clippy Lint Error and Formatting

**Files:**
- Modify: `src/handlers/admin.rs:148-152`
- Modify: `src/middleware/security.rs:63-66`

- [ ] **Step 1: Fix the clippy error in admin.rs**

In `src/handlers/admin.rs`, line 148, change:
```rust
if e.starts_with("duplicate:") || e.starts_with("重复") {
    error_response(StatusCode::CONFLICT, &e)
} else {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, &format!("上传失败: {}", e))
}
```
to:
```rust
if e.starts_with("duplicate:") || e.starts_with("重复") {
    error_response(StatusCode::CONFLICT, e)
} else {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("上传失败: {}", e),
    )
}
```

- [ ] **Step 2: Fix formatting in security.rs**

In `src/middleware/security.rs`, line 63-66, change:
```rust
let origins: Vec<HeaderValue> = origins
    .split(',')
    .map(|s| s.trim().parse::<HeaderValue>().expect("invalid CORS_ORIGIN"))
    .collect();
```
to:
```rust
let origins: Vec<HeaderValue> = origins
    .split(',')
    .map(|s| {
        s.trim()
            .parse::<HeaderValue>()
            .expect("invalid CORS_ORIGIN")
    })
    .collect();
```

- [ ] **Step 3: Run clippy and fmt to verify**

Run: `cargo clippy -- -D warnings && cargo fmt --check`
Expected: Both pass with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/handlers/admin.rs src/middleware/security.rs
git commit -m "fix: clippy lint error and formatting issues"
```

---

### Task 2: Token Lifecycle — Clean Up Old Tokens on Login

**Files:**
- Modify: `src/repositories/user_repo.rs`
- Modify: `src/services/auth_service.rs`

- [ ] **Step 1: Add delete_tokens_by_user_id to UserRepository**

In `src/repositories/user_repo.rs`, add a new method:
```rust
pub async fn delete_tokens_by_user_id(&self, user_id: i64) -> Result<u64, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM auth_tokens WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
    Ok(result.rows_affected())
}
```

- [ ] **Step 2: Clean old tokens before creating new one in login**

In `src/services/auth_service.rs`, in the `login` method, after the password verification succeeds (line 139), add a call to delete old tokens before creating the new one:

```rust
if !password::verify(&req.password, &user.password_hash)? {
    return Ok(AuthResponse {
        ok: false,
        token: None,
        error: Some("用户名或密码错误".into()),
    });
}

// Clean up old tokens for this user before creating a new one
let _ = self.user_repo.delete_tokens_by_user_id(user.id).await;

let token = self.user_repo.create_token(user.id).await?;
```

- [ ] **Step 3: Verify with cargo check**

Run: `cargo check`
Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src/repositories/user_repo.rs src/services/auth_service.rs
git commit -m "fix: clean up old tokens on login to prevent token accumulation"
```

---

### Task 3: Remove Duplicate RateLimiter Instance

**Files:**
- Modify: `src/app.rs`
- Modify: `src/state.rs`

- [ ] **Step 1: Reuse the AuthService's rate limiter in AppState**

In `src/app.rs`, the auth_service is created at line 57-62 with `RateLimiter::new()`, and AppState gets a separate `RateLimiter::new()` at line 78. Change AppState to reuse the same rate_limiter:

Current code:
```rust
let auth_service = AuthService::new(
    user_repo.clone(),
    playback_service.clone(),
    RateLimiter::new(),
    config.clone(),
);
```

Change to:
```rust
let rate_limiter = RateLimiter::new();
let auth_service = AuthService::new(
    user_repo.clone(),
    playback_service.clone(),
    rate_limiter.clone(),
    config.clone(),
);
```

And at line 78, change:
```rust
rate_limiter: RateLimiter::new(),
```
to:
```rust
rate_limiter,
```

- [ ] **Step 2: Verify with cargo check**

Run: `cargo check`
Expected: Compiles without errors.

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "fix: reuse single RateLimiter instance across AuthService and AppState"
```

---

### Task 4: Add Category Length Validation

**Files:**
- Modify: `src/handlers/admin.rs`

- [ ] **Step 1: Add category length check in add_external_video**

In `src/handlers/admin.rs`, in the `add_external_video` handler, after the title validation block (line 19-24), add:

```rust
if let Some(ref cat) = req.category {
    if cat.len() > 100 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "分类名称长度不能超过 100 个字符",
        ));
    }
}
```

- [ ] **Step 2: Add category length check in update_video**

In `src/handlers/admin.rs`, in the `update_video` handler, add validation before calling the service:

```rust
pub async fn update_video(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    SafeJson(req): SafeJson<VideoUpdateRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Some(ref title) = req.title {
        if title.trim().is_empty() || title.len() > 500 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "标题长度需在 1-500 个字符之间",
            ));
        }
    }
    if let Some(ref category) = req.category {
        if category.len() > 100 {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "分类名称长度不能超过 100 个字符",
            ));
        }
    }
```

Note: The `update_video` handler currently returns `Json<OkResponse>` without Result. It needs to be changed to return `Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)>` for the validation errors.

- [ ] **Step 3: Verify with cargo check**

Run: `cargo check`
Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src/handlers/admin.rs
git commit -m "fix: add category length validation (max 100 chars)"
```

---

### Task 5: Clean Up Dead Code

**Files:**
- Modify: `src/middleware/auth.rs`
- Modify: `src/services/video_service.rs`

- [ ] **Step 1: Remove unused #[allow(dead_code)] from AuthUser.id**

In `src/middleware/auth.rs`, line 117, remove the `#[allow(dead_code)]` attribute:
```rust
pub struct AuthUser {
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}
```

- [ ] **Step 2: Remove unused list_videos method from VideoService**

In `src/services/video_service.rs`, lines 21-25, remove the `#[allow(dead_code)]` and the entire `list_videos` method since `list_videos_paged` is the one actually used:

```rust
pub async fn list_videos_paged(
    ...
```

- [ ] **Step 3: Remove #[allow(dead_code)] from VideoRow fields**

In `src/repositories/video_repo.rs`, remove `#[allow(dead_code)]` from `file_hash`, `file_size`, `original_name`, and `created_at` fields. These fields are populated by SQLx and may be needed for future use, but the `#[allow(dead_code)]` can be removed if they are actually read somewhere, or the fields themselves removed if truly unused. Since they're part of `FromRow` mapping, keep the fields but remove the annotations — the fields ARE used by SQLx deserialization even if not read in Rust code directly.

Actually, `file_hash`, `file_size`, `original_name` are used in `find_existing_by_name_and_size_batch` indirectly (through the NameSize struct), and `created_at` is used in `user_repo.rs`. The `#[allow(dead_code)]` is needed because the struct fields are populated but not directly read in Rust code after deserialization. Leave these as-is — the annotations are correct.

- [ ] **Step 4: Verify with cargo check and clippy**

Run: `cargo clippy -- -D warnings && cargo check`
Expected: Passes without errors.

- [ ] **Step 5: Commit**

```bash
git add src/middleware/auth.rs src/services/video_service.rs
git commit -m "cleanup: remove dead code and unused annotations"
```

---

### Task 6: Final Verification

- [ ] **Step 1: Run full verification**

Run: `cargo fmt --check && cargo clippy -- -D warnings && cargo test`
Expected: All pass.

- [ ] **Step 2: Commit if any remaining fixes needed**
