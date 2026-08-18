use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use tracing::info;

use crate::config::AppConfig;
use crate::models::video::{FileCheckItem, VideoItem};
use crate::repositories::video_repo::{LocalVideoValues, VideoRepository};

#[derive(Clone)]
pub struct VideoService {
    repo: VideoRepository,
    config: AppConfig,
}

impl VideoService {
    pub fn new(repo: VideoRepository, config: AppConfig) -> Self {
        Self { repo, config }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_videos_paged(
        &self,
        page: i64,
        size: i64,
        query: Option<&str>,
        source_type: Option<&str>,
        category: Option<&str>,
        username: Option<&str>,
        uploader_id: Option<i64>,
        sort: Option<&str>,
    ) -> Result<(Vec<VideoItem>, i64), sqlx::Error> {
        // Run the count and the page query concurrently instead of serially —
        // both hit the pool, so this roughly halves list latency.
        let (total, rows) = tokio::try_join!(
            self.repo
                .count_all(query, source_type, category, uploader_id),
            self.repo.find_all_paged(
                page,
                size,
                query,
                source_type,
                category,
                username,
                uploader_id,
                sort,
            ),
        )?;
        let items: Vec<VideoItem> = rows.into_iter().map(VideoItem::from).collect();
        Ok((items, total))
    }

    pub async fn get_video(&self, id: i64) -> Result<Option<VideoItem>, sqlx::Error> {
        let row = self.repo.find_by_id(id).await?;
        Ok(row.map(VideoItem::from))
    }

    pub async fn add_external_video(
        &self,
        title: &str,
        description: Option<&str>,
        category: Option<&str>,
        stream_url: &str,
        cover_url: Option<&str>,
        uploader_id: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let desc = description.unwrap_or("");
        let cat = category.unwrap_or("general");
        let id = self
            .repo
            .save_external_video(title, desc, cat, cover_url, stream_url, uploader_id)
            .await?;
        Ok(id)
    }

    pub async fn check_existing_hashes(
        &self,
        hashes: Vec<String>,
    ) -> Result<Vec<String>, sqlx::Error> {
        self.repo.find_existing_hashes(&hashes).await
    }

    pub async fn check_existing_files(
        &self,
        files: &[FileCheckItem],
    ) -> Result<HashSet<usize>, sqlx::Error> {
        // Single batch query instead of N+1
        let pairs: Vec<(String, i64)> = files.iter().map(|f| (f.name.clone(), f.size)).collect();
        let existing_pairs = self
            .repo
            .find_existing_by_name_and_size_batch(&pairs)
            .await?;

        let mut existing = HashSet::new();
        for (i, pair) in pairs.iter().enumerate() {
            if existing_pairs.contains(pair) {
                existing.insert(i);
            }
        }
        Ok(existing)
    }

    pub async fn scan_media_directory(&self, category: &str) -> Result<i64, sqlx::Error> {
        const MAX_SCAN_FILES: usize = 5000;
        let existing_urls = self.repo.find_all_local_file_names().await?;
        let media_root = self.config.media_root.clone();
        let video_exts: HashSet<&str> =
            ["mp4", "m3u8", "mov", "avi", "mkv", "webm", "flv", "wmv"].into();
        let image_exts: HashSet<&str> = ["jpg", "jpeg", "png", "webp", "gif", "bmp"].into();

        // Offload blocking FS discovery + MD5 hashing to the blocking thread pool.
        // Uses streaming read to avoid loading entire files into memory.
        #[derive(Clone)]
        struct FileCandidate {
            file_name: String,
            stream_url: String,
            source_type: &'static str,
            file_hash: String,
            file_size: i64,
        }

        let candidates: Vec<FileCandidate> = tokio::task::spawn_blocking(move || {
            let mut entries = match std::fs::read_dir(&media_root) {
                Ok(e) => e,
                Err(_) => return vec![],
            };
            let mut out = Vec::new();
            while let Some(entry) = entries.next().and_then(|e| e.ok()) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let file_name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n.to_string(),
                    None => continue,
                };
                let stream_url = format!("/media/{}", file_name);
                // Skip files already in database — no need to hash
                if existing_urls.contains(&stream_url) {
                    continue;
                }
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_default();
                let source_type = if video_exts.contains(ext.as_str()) {
                    "local_video"
                } else if image_exts.contains(ext.as_str()) {
                    "local_image"
                } else {
                    continue;
                };
                // Stream-read file to compute MD5 without loading into memory
                let file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(_) => continue,
                };
                let file_size = match file.metadata() {
                    Ok(m) => m.len() as i64,
                    Err(_) => continue,
                };
                let mut hasher = Sha256::new();
                let mut buf = [0u8; 65536];
                let mut reader = file;
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => hasher.update(&buf[..n]),
                        Err(_) => break,
                    }
                }
                let file_hash = format!("{:x}", hasher.finalize());
                out.push(FileCandidate {
                    file_name,
                    stream_url,
                    source_type,
                    file_hash,
                    file_size,
                });
                // Stop hashing as soon as we've reached the insert cap —
                // otherwise a huge directory gets fully hashed for nothing.
                if out.len() >= MAX_SCAN_FILES {
                    break;
                }
            }
            out
        })
        .await
        .map_err(|e| {
            tracing::error!("scan directory panicked: {}", e);
            sqlx::Error::Protocol("scan thread panicked".into())
        })?;

        const BATCH_SIZE: usize = 500;
        let mut added = 0i64;
        let mut batch: Vec<LocalVideoValues<'_>> = Vec::with_capacity(BATCH_SIZE);

        for cand in &candidates {
            if added as usize >= MAX_SCAN_FILES {
                info!("Scan limit reached ({} files)", MAX_SCAN_FILES);
                break;
            }
            batch.push((
                &cand.file_name,
                "",
                cand.source_type,
                None,
                if cand.source_type == "local_image" {
                    Some(&cand.stream_url)
                } else {
                    None
                },
                &cand.stream_url,
                category,
                Some(&cand.file_hash),
                Some(cand.file_size),
                Some(&cand.file_name),
            ));
            added += 1;

            if batch.len() >= BATCH_SIZE {
                let n = self.repo.batch_save_local_videos(&batch).await?;
                info!("Batch inserted {} videos", n);
                batch.clear();
            }
        }
        if !batch.is_empty() {
            let n = self.repo.batch_save_local_videos(&batch).await?;
            info!("Batch inserted {} videos", n);
        }
        if added > 0 {
            info!("Scanned and added {} new files", added);
        }
        Ok(added)
    }

    pub async fn delete_video(&self, id: i64) -> Result<bool, String> {
        let video = self.repo.find_by_id(id).await.map_err(|e| e.to_string())?;
        let Some(v) = video else {
            return Ok(false);
        };
        // The DB cascade is the source of truth; only touch the filesystem if
        // it succeeds (otherwise we'd orphan a DB row pointing at a deleted
        // file).
        let deleted = self
            .repo
            .delete_video_cascade(id)
            .await
            .map_err(|e| e.to_string())?;
        // Best-effort physical cleanup for stream/cover/thumb files in a
        // single blocking task (previously three sequential spawn_blocking
        // calls plus sync canonicalize() calls on the async runtime).
        let media_root = self.config.media_root.clone();
        let urls = vec![
            v.stream_url,
            v.cover_url.unwrap_or_default(),
            v.thumb_url.unwrap_or_default(),
        ];
        let cleanup = tokio::task::spawn_blocking(move || {
            for url in &urls {
                if let Some(path) =
                    crate::services::media_service::safe_media_path(url, &media_root)
                {
                    if path.exists() {
                        match std::fs::remove_file(&path) {
                            Ok(_) => info!(file = %path.display(), "deleted media file"),
                            Err(e) => tracing::warn!(
                                file = %path.display(),
                                error = %e,
                                "failed to delete media file"
                            ),
                        }
                    }
                }
            }
        });
        if let Err(e) = cleanup.await {
            tracing::warn!("media cleanup task panicked: {}", e);
        }
        Ok(deleted)
    }

    pub async fn delete_videos(&self, ids: &[i64]) -> Result<u64, String> {
        // Load all video info in a single query
        let videos = self
            .repo
            .find_all_by_ids(ids)
            .await
            .map_err(|e| e.to_string())?;

        // Batch delete from DB in a single transaction (source of truth)
        let deleted = self
            .repo
            .batch_delete_videos(ids)
            .await
            .map_err(|e| e.to_string())?;

        // Best-effort physical file cleanup in ONE blocking task instead of
        // up to 3×500 sequential spawn_blocking calls with per-file await.
        let media_root = self.config.media_root.clone();
        let cleanup = tokio::task::spawn_blocking(move || {
            for v in &videos {
                for url in [
                    v.stream_url.as_str(),
                    v.cover_url.as_deref().unwrap_or(""),
                    v.thumb_url.as_deref().unwrap_or(""),
                ] {
                    if let Some(path) =
                        crate::services::media_service::safe_media_path(url, &media_root)
                    {
                        if path.exists() {
                            match std::fs::remove_file(&path) {
                                Ok(_) => info!(file = %path.display(), "deleted media file"),
                                Err(e) => tracing::warn!(
                                    file = %path.display(),
                                    error = %e,
                                    "failed to delete media file"
                                ),
                            }
                        }
                    }
                }
            }
        });
        if let Err(e) = cleanup.await {
            tracing::warn!("media cleanup task panicked: {}", e);
        }
        Ok(deleted)
    }

    pub async fn update_video(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        category: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let rows = self
            .repo
            .update_video(id, title, description, category)
            .await?;
        Ok(rows > 0)
    }

    pub async fn increment_views(&self, id: i64) -> Result<(), sqlx::Error> {
        self.repo.increment_views(id).await
    }
}
