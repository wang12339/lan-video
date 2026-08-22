use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;
use tracing::info;

use crate::config::AppConfig;
use crate::models::video::{FileCheckItem, VideoItem};
use crate::repositories::video_repo::{LocalVideoValues, VideoRepository};
use crate::util::error::ServiceError;

#[derive(Clone)]
pub struct VideoService {
    repo: VideoRepository,
    config: AppConfig,
}

impl VideoService {
    /// 创建视频服务实例
    ///
    /// # 参数
    /// - `repo`: 视频数据仓库
    /// - `config`: 应用配置
    pub fn new(repo: VideoRepository, config: AppConfig) -> Self {
        Self { repo, config }
    }

    /// 分页查询视频列表
    ///
    /// # 参数
    /// - `page`: 页码，从1开始
    /// - `size`: 每页数量
    /// - `query`: 可选搜索关键词（模糊匹配标题/描述）
    /// - `source_type`: 可选来源类型筛选（local_video/local_image/external）
    /// - `category`: 可选分类筛选
    /// - `username`: 可选用户名筛选
    /// - `uploader_id`: 可选上传者ID筛选
    /// - `sort`: 可选排序方式
    ///
    /// # 返回
    /// - `Ok((Vec<VideoItem>, i64))`: 视频列表和总记录数
    /// - `Err(ServiceError)`: 数据库查询失败
    ///
    /// # 性能
    /// - 计数查询与分页查询并行执行，减少约50%延迟
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
    ) -> Result<(Vec<VideoItem>, i64), ServiceError> {
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

    /// 根据ID获取视频详情
    ///
    /// # 参数
    /// - `id`: 视频ID
    ///
    /// # 返回
    /// - `Ok(Some(VideoItem))`: 视频存在时返回视频信息
    /// - `Ok(None)`: 视频不存在
    /// - `Err(ServiceError)`: 数据库查询失败
    pub async fn get_video(&self, id: i64) -> Result<Option<VideoItem>, ServiceError> {
        let row = self.repo.find_by_id(id).await?;
        Ok(row.map(VideoItem::from))
    }

    /// 添加外部视频
    ///
    /// # 参数
    /// - `title`: 视频标题
    /// - `description`: 可选视频描述
    /// - `category`: 可选分类（默认"general"）
    /// - `stream_url`: 视频流地址
    /// - `cover_url`: 可选封面图片地址
    /// - `uploader_id`: 可选上传者ID
    ///
    /// # 返回
    /// - `Ok(i64)`: 新增视频的ID
    /// - `Err(ServiceError)`: 数据库插入失败
    pub async fn add_external_video(
        &self,
        title: &str,
        description: Option<&str>,
        category: Option<&str>,
        stream_url: &str,
        cover_url: Option<&str>,
        uploader_id: Option<i64>,
    ) -> Result<i64, ServiceError> {
        let desc = description.unwrap_or("");
        let cat = category.unwrap_or("general");
        let id = self
            .repo
            .save_external_video(title, desc, cat, cover_url, stream_url, uploader_id)
            .await?;
        Ok(id)
    }

    /// 检查已存在的文件哈希
    ///
    /// # 参数
    /// - `hashes`: 待检查的文件哈希列表
    ///
    /// # 返回
    /// - `Ok(Vec<String>)`: 数据库中已存在的哈希列表
    /// - `Err(ServiceError)`: 数据库查询失败
    pub async fn check_existing_hashes(
        &self,
        hashes: Vec<String>,
    ) -> Result<Vec<String>, ServiceError> {
        Ok(self.repo.find_existing_hashes(&hashes).await?)
    }

    /// 检查已存在的文件（按文件名和大小）
    ///
    /// # 参数
    /// - `files`: 待检查的文件列表
    ///
    /// # 返回
    /// - `Ok(HashSet<usize>)`: 已存在文件的索引集合
    /// - `Err(ServiceError)`: 数据库查询失败
    ///
    /// # 性能
    /// - 使用批量查询代替 N+1 查询
    pub async fn check_existing_files(
        &self,
        files: &[FileCheckItem],
    ) -> Result<HashSet<usize>, ServiceError> {
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

    /// 扫描媒体目录并导入新文件
    ///
    /// # 参数
    /// - `category`: 导入文件的分类
    ///
    /// # 返回
    /// - `Ok(i64)`: 新增文件数量
    /// - `Err(ServiceError)`: 数据库操作失败
    ///
    /// # 功能
    /// - 扫描 media_root 目录下的视频和图片文件
    /// - 计算文件SHA256哈希用于去重
    /// - 跳过数据库中已存在的文件
    /// - 批量插入新文件（每批500条）
    ///
    /// # 限制
    /// - 最多扫描5000个文件
    /// - 支持格式：mp4, m3u8, mov, avi, mkv, webm, flv, wmv, jpg, jpeg, png, webp, gif, bmp
    pub async fn scan_media_directory(&self, category: &str) -> Result<i64, ServiceError> {
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
            ServiceError::internal("扫描目录线程异常")
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

    /// 删除单个视频
    ///
    /// # 参数
    /// - `id`: 视频ID
    ///
    /// # 返回
    /// - `Ok(true)`: 视频已删除
    /// - `Ok(false)`: 视频不存在
    /// - `Err(ServiceError)`: 删除失败（数据库或文件系统错误）
    ///
    /// # 行为
    /// - 优先从数据库级联删除（source of truth）
    /// - 删除成功后尝试清理物理文件（stream/cover/thumb）
    /// - 文件清理失败不影响数据库删除结果
    pub async fn delete_video(&self, id: i64) -> Result<bool, ServiceError> {
        let video = self.repo.find_by_id(id).await?;
        let Some(v) = video else {
            return Ok(false);
        };
        // The DB cascade is the source of truth; only touch the filesystem if
        // it succeeds (otherwise we'd orphan a DB row pointing at a deleted
        // file).
        let deleted = self.repo.delete_video_cascade(id).await?;
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

    /// 批量删除视频
    ///
    /// # 参数
    /// - `ids`: 待删除视频的ID列表
    ///
    /// # 返回
    /// - `Ok(u64)`: 实际删除的视频数量
    /// - `Err(ServiceError)`: 删除失败（数据库或文件系统错误）
    ///
    /// # 行为
    /// - 单次查询加载所有视频信息
    /// - 单次事务批量删除数据库记录
    /// - 批量清理物理文件（单个阻塞任务处理所有文件）
    pub async fn delete_videos(&self, ids: &[i64]) -> Result<u64, ServiceError> {
        // Load all video info in a single query
        let videos = self.repo.find_all_by_ids(ids).await?;

        // Batch delete from DB in a single transaction (source of truth)
        let deleted = self.repo.batch_delete_videos(ids).await?;

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

    /// 更新视频信息
    ///
    /// # 参数
    /// - `id`: 视频ID
    /// - `title`: 可选新标题
    /// - `description`: 可选新描述
    /// - `category`: 可选新分类
    ///
    /// # 返回
    /// - `Ok(true)`: 视频已更新
    /// - `Ok(false)`: 视频不存在
    /// - `Err(ServiceError)`: 数据库更新失败
    pub async fn update_video(
        &self,
        id: i64,
        title: Option<&str>,
        description: Option<&str>,
        category: Option<&str>,
    ) -> Result<bool, ServiceError> {
        let rows = self
            .repo
            .update_video(id, title, description, category)
            .await?;
        Ok(rows > 0)
    }

    /// 递增视频播放次数
    ///
    /// # 参数
    /// - `id`: 视频ID
    ///
    /// # 返回
    /// - `Ok(())`: 更新成功
    /// - `Err(ServiceError)`: 数据库更新失败
    pub async fn increment_views(&self, id: i64) -> Result<(), ServiceError> {
        self.repo.increment_views(id).await.map_err(ServiceError::from)
    }
}
