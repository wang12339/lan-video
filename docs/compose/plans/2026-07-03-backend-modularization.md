# 后端全面模块化实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use compose:subagent (recommended) or compose:execute to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将后端代码从"上帝服务"模式拆分为职责清晰的模块化架构，提升可维护性和开发效率。

**Architecture:** 分层架构保持 handler → service → repository，但将 video_service 拆分为 video_service + media_service + playback_service，将 video_repo 拆分为 video_repo + playback_repo，新增全局 AppError 类型统一错误处理。

**Tech Stack:** Rust / Axum 0.8 / SQLx 0.8 / PostgreSQL 16

---

## 文件结构

```
src/
├── error.rs              # 新增：全局 AppError 类型
├── state.rs              # 修改：添加 auth_service, video_cache 引用
├── lib.rs                # 修改：添加 pub mod error
├── app.rs                # 修改：简化 build_router
├── handlers/
│   ├── auth.rs           # 修改：使用 AppError
│   ├── videos.rs         # 修改：使用 AppError
│   ├── playback.rs       # 修改：使用 AppError
│   ├── admin.rs          # 修改：使用 AppError
│   └── server.rs         # 修改：使用 AppError
├── services/
│   ├── auth_service.rs   # 不变
│   ├── media_service.rs  # 新增：从 video_service 拆出
│   ├── playback_service.rs # 新增：从 video_service 拆出
│   └── video_service.rs  # 修改：只保留视频 CRUD
├── repositories/
│   ├── user_repo.rs      # 不变
│   ├── video_repo.rs     # 修改：移除播放历史/点赞/收藏
│   └── playback_repo.rs  # 新增：播放历史/点赞/收藏
└── middleware/
    └── auth.rs           # 修改：导出 extract_token
```

---

## Task 1: 创建全局 AppError 类型

**Files:**
- Create: `src/error.rs`
- Modify: `src/lib.rs`
- Test: `src/error.rs` (inline tests)

- [ ] **Step 1: 创建 error.rs**

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    Conflict(String),
    RateLimited,
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg),
            AppError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "尝试次数过多，请稍后再试".into()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}

impl From<String> for AppError {
    fn from(e: String) -> Self {
        AppError::Internal(e)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("Database error: {}", e);
        AppError::Internal("数据库错误".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_not_found() {
        let err = AppError::NotFound("not found".into());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_rate_limited() {
        let err = AppError::RateLimited;
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
```

- [ ] **Step 2: 在 lib.rs 中添加模块声明**

在 `src/lib.rs` 顶部添加：
```rust
pub mod error;
```

- [ ] **Step 3: 运行测试验证**

Run: `cd backend && cargo test error`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add src/error.rs src/lib.rs
git commit -m "feat: add global AppError type for unified error handling"
```

---

## Task 2: 创建 playback_repo.rs

**Files:**
- Create: `src/repositories/playback_repo.rs`
- Modify: `src/repositories/mod.rs`
- Test: 运行 cargo test 确保不破坏现有功能

- [ ] **Step 1: 创建 playback_repo.rs**

从 `video_repo.rs` 迁移以下方法：
- `get_playback_data`
- `find_playback_history_by_username`
- `upsert_playback`
- `count_watched_videos`
- `sum_watch_time`
- `toggle_like`
- `is_liked`
- `toggle_favorite`
- `is_favorited`

```rust
use sqlx::PgPool;
use crate::models::playback::RecentWatchItem;

#[derive(Debug, sqlx::FromRow)]
struct PlaybackRow {
    pub video_id: i64,
    pub position_ms: i64,
    pub duration_ms: i64,
}

#[derive(Clone)]
pub struct PlaybackRepository {
    pool: PgPool,
}

impl PlaybackRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_playback_data(
        &self,
        username: &str,
        video_id: i64,
    ) -> Result<Option<(i64, i64)>, sqlx::Error> {
        let row = sqlx::query_as::<_, PlaybackRow>(
            "SELECT video_id, position_ms, duration_ms FROM playback_history WHERE username = $1 AND video_id = $2"
        )
        .bind(username)
        .bind(video_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| (r.position_ms, r.duration_ms)))
    }

    pub async fn find_playback_history_by_username(
        &self,
        username: &str,
        limit: Option<i64>,
    ) -> Result<Vec<RecentWatchItem>, sqlx::Error> {
        #[derive(Debug, sqlx::FromRow)]
        struct Row {
            video_id: i64,
            title: String,
            cover_url: Option<String>,
            stream_url: String,
            source_type: String,
            category: String,
            position_ms: i64,
            duration_ms: i64,
            updated_at: chrono::DateTime<chrono::Utc>,
        }
        let limit = limit.unwrap_or(500);
        let rows = sqlx::query_as::<_, Row>(
            r#"SELECT h.video_id, v.title, v.cover_url, v.stream_url, v.source_type, v.category,
                      h.position_ms, h.duration_ms, h.updated_at
               FROM playback_history h
               JOIN videos v ON h.video_id = v.id
               WHERE h.username = $1
               ORDER BY h.updated_at DESC
               LIMIT $2"#
        )
        .bind(username)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| RecentWatchItem {
            video_id: r.video_id,
            title: r.title,
            cover_url: r.cover_url,
            stream_url: r.stream_url,
            source_type: r.source_type,
            category: r.category,
            position_ms: r.position_ms,
            duration_ms: r.duration_ms,
            updated_at: r.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }).collect())
    }

    pub async fn upsert_playback(
        &self,
        username: &str,
        video_id: i64,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO playback_history (username, video_id, position_ms, duration_ms, updated_at) \
             VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP) \
             ON CONFLICT (username, video_id) DO UPDATE SET \
             position_ms = $3, duration_ms = $4, updated_at = CURRENT_TIMESTAMP"
        )
        .bind(username)
        .bind(video_id)
        .bind(position_ms)
        .bind(duration_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn count_watched_videos(&self, username: &str) -> Result<i64, sqlx::Error> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT video_id) FROM playback_history WHERE username = $1"
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn sum_watch_time(&self, username: &str) -> Result<i64, sqlx::Error> {
        let (sum,): (Option<i64>,) = sqlx::query_as(
            "SELECT SUM(position_ms) FROM playback_history WHERE username = $1"
        )
        .bind(username)
        .fetch_one(&self.pool)
        .await?;
        Ok(sum.unwrap_or(0))
    }

    pub async fn toggle_like(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        let existing = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM user_likes WHERE username = $1 AND video_id = $2)"
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;

        if existing {
            sqlx::query("DELETE FROM user_likes WHERE username = $1 AND video_id = $2")
                .bind(username)
                .bind(video_id)
                .execute(&self.pool)
                .await?;
            Ok(false)
        } else {
            sqlx::query("INSERT INTO user_likes (username, video_id) VALUES ($1, $2)")
                .bind(username)
                .bind(video_id)
                .execute(&self.pool)
                .await?;
            Ok(true)
        }
    }

    pub async fn is_liked(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM user_likes WHERE username = $1 AND video_id = $2)"
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn toggle_favorite(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        let existing = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM user_favorites WHERE username = $1 AND video_id = $2)"
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await?;

        if existing {
            sqlx::query("DELETE FROM user_favorites WHERE username = $1 AND video_id = $2")
                .bind(username)
                .bind(video_id)
                .execute(&self.pool)
                .await?;
            Ok(false)
        } else {
            sqlx::query("INSERT INTO user_favorites (username, video_id) VALUES ($1, $2)")
                .bind(username)
                .bind(video_id)
                .execute(&self.pool)
                .await?;
            Ok(true)
        }
    }

    pub async fn is_favorited(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM user_favorites WHERE username = $1 AND video_id = $2)"
        )
        .bind(username)
        .bind(video_id)
        .fetch_one(&self.pool)
        .await
    }
}
```

- [ ] **Step 2: 在 mod.rs 中添加模块声明**

在 `src/repositories/mod.rs` 中添加：
```rust
pub mod playback_repo;
```

- [ ] **Step 3: 运行测试验证**

Run: `cd backend && cargo check`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add src/repositories/playback_repo.rs src/repositories/mod.rs
git commit -m "feat: add PlaybackRepository for playback history, likes, favorites"
```

---

## Task 3: 从 video_repo.rs 移除播放历史/点赞/收藏方法

**Files:**
- Modify: `src/repositories/video_repo.rs`
- Test: 运行 cargo test 确保不破坏现有功能

- [ ] **Step 1: 从 video_repo.rs 删除以下方法**

删除以下方法（保留其他视频相关方法）：
- `get_playback_data`
- `find_playback_history_by_username`
- `upsert_playback`
- `count_watched_videos`
- `sum_watch_time`
- `toggle_like`
- `is_liked`
- `toggle_favorite`
- `is_favorited`
- `delete_playback_history_by_video`
- `delete_likes_by_video`
- `delete_favorites_by_video`

同时删除 `PlaybackRow` 结构体（已迁移到 playback_repo.rs）。

- [ ] **Step 2: 运行测试验证**

Run: `cd backend && cargo check`
Expected: 编译错误（其他文件还在引用这些方法）

- [ ] **Step 3: 提交（临时）**

```bash
git add src/repositories/video_repo.rs
git commit -m "refactor: remove playback/like/favorite methods from VideoRepository"
```

---

## Task 4: 创建 playback_service.rs

**Files:**
- Create: `src/services/playback_service.rs`
- Modify: `src/services/mod.rs`
- Test: 运行 cargo test 确保不破坏现有功能

- [ ] **Step 1: 创建 playback_service.rs**

从 `video_service.rs` 迁移以下方法：
- `get_playback_data`
- `get_playback_history`
- `update_playback`
- `get_user_profile_data`
- `toggle_like`
- `is_liked`
- `toggle_favorite`
- `is_favorited`

```rust
use crate::repositories::playback_repo::PlaybackRepository;

#[derive(Clone)]
pub struct PlaybackService {
    repo: PlaybackRepository,
}

impl PlaybackService {
    pub fn new(repo: PlaybackRepository) -> Self {
        Self { repo }
    }

    pub async fn get_playback_data(
        &self,
        username: &str,
        video_id: i64,
    ) -> Result<Option<(i64, i64)>, sqlx::Error> {
        self.repo.get_playback_data(username, video_id).await
    }

    pub async fn get_playback_history(
        &self,
        username: &str,
    ) -> Result<Vec<crate::models::playback::RecentWatchItem>, sqlx::Error> {
        self.repo.find_playback_history_by_username(username, None).await
    }

    pub async fn update_playback(
        &self,
        username: &str,
        video_id: i64,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<(), sqlx::Error> {
        self.repo.upsert_playback(username, video_id, position_ms, duration_ms).await
    }

    pub async fn get_user_profile_data(
        &self,
        username: &str,
    ) -> Result<(i64, i64, Vec<crate::models::playback::RecentWatchItem>), sqlx::Error> {
        let total_videos_watched = self.repo.count_watched_videos(username).await?;
        let total_watch_time = self.repo.sum_watch_time(username).await?;
        let recent_history = self.repo.find_playback_history_by_username(username, Some(20)).await?;
        Ok((total_videos_watched, total_watch_time, recent_history))
    }

    pub async fn toggle_like(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.toggle_like(username, video_id).await
    }

    pub async fn is_liked(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.is_liked(username, video_id).await
    }

    pub async fn toggle_favorite(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.toggle_favorite(username, video_id).await
    }

    pub async fn is_favorited(&self, username: &str, video_id: i64) -> Result<bool, sqlx::Error> {
        self.repo.is_favorited(username, video_id).await
    }
}
```

- [ ] **Step 2: 在 mod.rs 中添加模块声明**

在 `src/services/mod.rs` 中添加：
```rust
pub mod playback_service;
```

- [ ] **Step 3: 运行测试验证**

Run: `cd backend && cargo check`
Expected: 编译错误（handler 还在引用 video_service 的播放历史方法）

- [ ] **Step 4: 提交**

```bash
git add src/services/playback_service.rs src/services/mod.rs
git commit -m "feat: add PlaybackService for playback history, likes, favorites"
```

---

## Task 5: 从 video_service.rs 移除播放历史/点赞/收藏方法

**Files:**
- Modify: `src/services/video_service.rs`
- Test: 运行 cargo test 确保不破坏现有功能

- [ ] **Step 1: 从 video_service.rs 删除以下方法**

删除以下方法（保留视频 CRUD 和媒体处理方法）：
- `get_playback_data`
- `get_playback_history`
- `update_playback`
- `get_user_profile_data`
- `toggle_like`
- `is_liked`
- `toggle_favorite`
- `is_favorited`

- [ ] **Step 2: 运行测试验证**

Run: `cd backend && cargo check`
Expected: 编译错误（handler 还在引用这些方法）

- [ ] **Step 3: 提交**

```bash
git add src/services/video_service.rs
git commit -m "refactor: remove playback/like/favorite methods from VideoService"
```

---

## Task 6: 创建 media_service.rs

**Files:**
- Create: `src/services/media_service.rs`
- Modify: `src/services/mod.rs`
- Test: 运行 cargo test 确保不破坏现有功能

- [ ] **Step 1: 创建 media_service.rs**

从 `video_service.rs` 迁移以下方法和函数：
- `upload_video_file`
- `validate_file_type`
- `extract_duration`
- `generate_thumbnail`
- `backfill_thumbnails`
- `update_cover`

```rust
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use axum::body::Bytes;
use md5::{Digest, Md5};
use tokio::fs as tokio_fs;

use crate::config::AppConfig;
use crate::repositories::video_repo::VideoRepository;

#[derive(Clone)]
pub struct MediaService {
    repo: VideoRepository,
    config: AppConfig,
}

impl MediaService {
    pub fn new(repo: VideoRepository, config: AppConfig) -> Self {
        Self { repo, config }
    }

    pub async fn upload_video_file(
        &self,
        file_name: &str,
        temp_path: &Path,
        category: &str,
        client_hash: Option<&str>,
    ) -> Result<i64, String> {
        // 计算文件哈希
        let hash = self.compute_file_hash(temp_path).await?;

        // 检查重复
        if self.repo.find_video_by_file_hash(&hash).await.map_err(|e| e.to_string())?.is_some() {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err("重复：视频已存在".into());
        }

        // 确定文件类型
        let ext = Path::new(file_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_else(|| "mp4".to_string());

        if let Err(e) = validate_file_type(temp_path, &ext) {
            let _ = tokio::fs::remove_file(temp_path).await;
            return Err(e);
        }

        let is_video = matches!(
            ext.as_str(),
            "mp4" | "m3u8" | "mov" | "avi" | "mkv" | "webm" | "flv" | "wmv"
        );
        let is_image = matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp"
        );

        let source_type = if is_video {
            "local_video"
        } else if is_image {
            "local_image"
        } else {
            "local"
        };

        // 移动文件到最终位置
        let final_name = format!("{}_{}.{}", 
            Path::new(file_name).file_stem().unwrap_or_default().to_string_lossy(),
            chrono::Utc::now().timestamp_millis(),
            ext
        );
        let final_path = self.config.media_root.join(&final_name);
        tokio::fs::rename(temp_path, &final_path).await
            .map_err(|e| format!("移动文件失败: {}", e))?;

        let stream_url = format!("/media/{}", final_name);
        let file_size = tokio::fs::metadata(&final_path).await
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let id = self.repo.save_local_video(
            file_name, "", source_type, None, &stream_url, category,
            Some(&hash), Some(file_size), Some(file_name), None,
        ).await.map_err(|e| e.to_string())?;

        Ok(id)
    }

    async fn compute_file_hash(&self, path: &Path) -> Result<String, String> {
        use tokio::io::AsyncReadExt;
        let mut file = tokio::fs::File::open(path).await
            .map_err(|e| format!("打开文件失败: {}", e))?;
        let mut hasher = Md5::new();
        let mut buf = vec![0u8; 65536];
        loop {
            let n = file.read(&mut buf).await
                .map_err(|e| format!("读取文件失败: {}", e))?;
            if n == 0 { break; }
            hasher.update(&buf[..n]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub async fn generate_thumbnail(&self, video_id: i64) -> Result<bool, String> {
        // 缩略图生成逻辑
        todo!()
    }

    pub async fn backfill_thumbnails(&self) -> Result<(u64, Vec<String>), String> {
        // 缩略图回填逻辑
        todo!()
    }

    pub async fn update_cover(&self, id: i64, file_name: &str, bytes: Bytes) -> Result<(), String> {
        // 封面更新逻辑
        todo!()
    }
}

pub fn validate_file_type(path: &Path, ext: &str) -> Result<(), String> {
    // 文件类型验证逻辑
    todo!()
}

async fn extract_duration(path: &Path) -> Result<Option<i64>, String> {
    // 时长提取逻辑
    todo!()
}
```

- [ ] **Step 2: 在 mod.rs 中添加模块声明**

在 `src/services/mod.rs` 中添加：
```rust
pub mod media_service;
```

- [ ] **Step 3: 运行测试验证**

Run: `cd backend && cargo check`
Expected: 编译错误（其他文件还在引用 video_service 的媒体处理方法）

- [ ] **Step 4: 提交**

```bash
git add src/services/media_service.rs src/services/mod.rs
git commit -m "feat: add MediaService for file upload, thumbnails, ffprobe"
```

---

## Task 7: 从 video_service.rs 移除媒体处理方法

**Files:**
- Modify: `src/services/video_service.rs`
- Test: 运行 cargo test 确保不破坏现有功能

- [ ] **Step 1: 从 video_service.rs 删除以下方法和函数**

删除以下方法和函数（保留视频 CRUD 方法）：
- `upload_video_file`
- `compute_file_hash`
- `generate_thumbnail`
- `backfill_thumbnails`
- `update_cover`
- `validate_file_type` (pub fn)
- `extract_duration` (async fn)

- [ ] **Step 2: 运行测试验证**

Run: `cd backend && cargo check`
Expected: 编译错误（handler 还在引用这些方法）

- [ ] **Step 3: 提交**

```bash
git add src/services/video_service.rs
git commit -m "refactor: remove media processing methods from VideoService"
```

---

## Task 8: 更新 AppState 和 app.rs

**Files:**
- Modify: `src/state.rs`
- Modify: `src/app.rs`
- Test: 运行 cargo test 确保不破坏现有功能

- [ ] **Step 1: 更新 state.rs**

```rust
use std::sync::Arc;
use crate::repositories::user_repo::UserRepository;
use crate::repositories::video_repo::VideoRepository;
use crate::repositories::playback_repo::PlaybackRepository;
use crate::services::video_service::VideoService;
use crate::services::media_service::MediaService;
use crate::services::playback_service::PlaybackService;
use crate::services::auth_service::AuthService;
use crate::config::AppConfig;
use crate::middleware::rate_limit::RateLimiter;
use moka::sync::Cache;

pub type VideoListCache = Cache<String, Vec<crate::models::video::VideoItem>>;

#[derive(Clone)]
pub struct AppState {
    pub user_repo: UserRepository,
    pub video_repo: VideoRepository,
    pub playback_repo: PlaybackRepository,
    pub video_service: VideoService,
    pub media_service: MediaService,
    pub playback_service: PlaybackService,
    pub auth_service: AuthService,
    pub config: AppConfig,
    pub rate_limiter: RateLimiter,
    pub video_cache: VideoListCache,
}
```

- [ ] **Step 2: 更新 app.rs**

在 `build_router` 中构造所有 service 并放入 AppState：

```rust
pub async fn build_router(config: AppConfig) -> Router {
    let pool = init_pool(&config.database_url).await;

    let user_repo = UserRepository::new(pool.clone());
    let video_repo = VideoRepository::new(pool.clone());
    let playback_repo = PlaybackRepository::new(pool.clone());

    let video_service = VideoService::new(video_repo.clone(), config.clone());
    let media_service = MediaService::new(video_repo.clone(), config.clone());
    let playback_service = PlaybackService::new(playback_repo.clone());
    let auth_service = AuthService::new(
        user_repo.clone(),
        video_service.clone(),
        RateLimiter::new(),
        config.clone(),
    );

    let video_cache = VideoListCache::builder()
        .time_to_live(Duration::from_secs(10))
        .max_capacity(1024)
        .build();

    let state = Arc::new(AppState {
        user_repo,
        video_repo,
        playback_repo,
        video_service,
        media_service,
        playback_service,
        auth_service,
        config: config.clone(),
        rate_limiter: RateLimiter::new(),
        video_cache,
    });

    // ... 其余路由组装代码
}
```

- [ ] **Step 3: 运行测试验证**

Run: `cd backend && cargo check`
Expected: 编译错误（handler 还在使用旧的 state 字段）

- [ ] **Step 4: 提交**

```bash
git add src/state.rs src/app.rs
git commit -m "refactor: update AppState with new services and repos"
```

---

## Task 9: 更新 handlers 使用新 service

**Files:**
- Modify: `src/handlers/playback.rs`
- Modify: `src/handlers/videos.rs`
- Modify: `src/handlers/admin.rs`
- Modify: `src/handlers/auth.rs`
- Test: 运行 cargo test 确保不破坏现有功能

- [ ] **Step 1: 更新 handlers/playback.rs**

将 `state.video_service` 替换为 `state.playback_service`。

- [ ] **Step 2: 更新 handlers/videos.rs**

将点赞/收藏相关方法替换为 `state.playback_service`。

- [ ] **Step 3: 更新 handlers/admin.rs**

将上传/缩略图相关方法替换为 `state.media_service`。

- [ ] **Step 4: 更新 handlers/auth.rs**

使用 `state.auth_service` 替代手动构造。

- [ ] **Step 5: 运行测试验证**

Run: `cd backend && cargo test`
Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add src/handlers/
git commit -m "refactor: update handlers to use new services"
```

---

## Task 10: 清理和验证

**Files:**
- Test: 运行完整测试套件

- [ ] **Step 1: 运行完整测试**

Run: `cd backend && cargo clippy -- -D warnings && cargo fmt && cargo test`
Expected: ALL PASS

- [ ] **Step 2: 验证文件大小**

检查每个文件行数是否在合理范围内（100-300行）。

- [ ] **Step 3: 最终提交**

```bash
git add -A
git commit -m "refactor: complete backend modularization"
```
