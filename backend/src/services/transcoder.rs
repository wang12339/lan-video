use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Semaphore;

/// 转码器配置（由 `AppConfig::transcode_settings()` 构造，测试可用默认值）。
#[derive(Debug, Clone)]
pub struct TranscodeSettings {
    /// 单次 ffmpeg 转码超时。
    pub transcode_timeout: Duration,
    /// 单次 ffprobe 调用超时。
    pub ffprobe_timeout: Duration,
    /// 允许同时进行的 ffmpeg 转码数。
    pub concurrency: usize,
    /// 计算各分辨率输出大小上限时假设的源视频最大时长（秒）。
    pub max_duration_secs: u64,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
}

impl Default for TranscodeSettings {
    fn default() -> Self {
        Self {
            transcode_timeout: Duration::from_secs(3600),
            ffprobe_timeout: Duration::from_secs(30),
            concurrency: 1,
            max_duration_secs: 7200,
            ffmpeg_path: "ffmpeg".into(),
            ffprobe_path: "ffprobe".into(),
        }
    }
}

/// Max output size in bytes for a variant at the given resolution, derived
/// from the target bitrate and the maximum assumed source duration. A
/// malicious or pathological input cannot make ffmpeg fill the disk: any
/// output beyond this cap is deleted right after encoding.
fn max_variant_size_bytes(resolution: &str, max_duration_secs: u64) -> Option<u64> {
    let (_, _, bitrate_kbps) = resolution_params(resolution)?;
    Some(bitrate_kbps as u64 * 1000 / 8 * max_duration_secs)
}

/// ffmpeg stderr can be arbitrarily large (a hostile media file may produce
/// a flood of diagnostics); keep error messages bounded for logs.
fn truncate_stderr(stderr: &str) -> String {
    const MAX_CHARS: usize = 512;
    if stderr.chars().count() <= MAX_CHARS {
        stderr.to_string()
    } else {
        let truncated: String = stderr.chars().take(MAX_CHARS).collect();
        let dropped = stderr.chars().count() - MAX_CHARS;
        format!("{}… ({} more bytes not shown)", truncated, dropped)
    }
}

/// Shared width/height/bitrate mapping used by arg building and bitrate lookup.
fn resolution_params(resolution: &str) -> Option<(u32, u32, u32)> {
    match resolution {
        "2160p" => Some((3840, 2160, 8000)),
        "1080p" => Some((1920, 1080, 5000)),
        "720p" => Some((1280, 720, 2500)),
        "480p" => Some((854, 480, 1000)),
        "360p" => Some((640, 360, 600)),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoVariant {
    pub id: i32,
    pub video_id: i64,
    pub resolution: String,
    pub file_path: String,
    pub file_size: i64,
    pub bitrate: Option<i32>,
    pub codec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlsVariant {
    pub name: String,
    pub bitrate: u32,
    pub width: u32,
    pub height: u32,
    pub playlist_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HlsPlaylist {
    pub video_id: i64,
    pub master_url: String,
    pub variants: Vec<HlsVariant>,
}

#[derive(Debug, Clone)]
pub struct Transcoder {
    ffmpeg_path: String,
    ffprobe_path: String,
    output_dir: PathBuf,
    hls_dir: PathBuf,
    settings: TranscodeSettings,
    semaphore: Arc<Semaphore>,
}

/// 进程级「HLS 转码中」集合（video_id）。同一视频的 HLS 输出目录是固定的
/// `hls/{video_id}/`，重复请求若各起一个 ffmpeg，会并发覆盖同一目录下的
/// 播放列表与分片。
///
/// 这一层是**进程内**的快路径：它不需要网络往返，因此单实例部署（也是默认
/// 部署方式）零额外开销，且在 Redis 不可用时仍能保证同进程内只有一个任务。
/// 多实例部署下由 `try_begin_hls` 额外通过 Redis 分布式锁收口，见该函数。
static HLS_IN_FLIGHT: OnceLock<Mutex<HashSet<i64>>> = OnceLock::new();

fn hls_in_flight() -> &'static Mutex<HashSet<i64>> {
    HLS_IN_FLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Redis 分布式锁的存活时间上限。
///
/// 只在实例崩溃（guard 的 `Drop` 不会执行）时才会走到 TTL 过期，因此取值
/// 对齐 `/admin/videos/hls` 路由的 7200s 超时：正常情况下锁总是由 guard 精确
/// 释放，TTL 只是崩溃后的兜底，不该先于正常转码结束而抢走锁。
pub const HLS_LOCK_TTL_SECS: u64 = 7200;

/// Compare-and-delete：只删除自己持有的锁。
///
/// 不能用裸 `DEL`：如果本实例的锁已因 TTL 过期被回收、而另一实例重新抢到了
/// 同一把锁，无条件 `DEL` 会把别人的锁删掉。脚本保证「值仍是我的 token 才删」。
const HLS_UNLOCK_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('DEL', KEYS[1])
end
return 0
"#;

/// HLS 转码占用的 RAII 标记：drop（含 panic 展开）时自动释放 video_id。
///
/// 持有期间：进程内集合挡住同实例的并发请求，Redis 锁挡住跨实例的并发请求。
pub struct HlsInFlightGuard {
    video_id: i64,
    /// Some 时表示同时持有 Redis 锁，drop 时需要按 token 精确释放。
    redis: Option<(Arc<redis::aio::ConnectionManager>, String)>,
}

impl Drop for HlsInFlightGuard {
    fn drop(&mut self) {
        // 本地集合：锁中毒（持锁线程 panic）不应阻止释放，取回内部集合继续清理。
        let mut set = hls_in_flight()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        set.remove(&self.video_id);
        drop(set);

        // 分布式锁释放需要 await，而 `Drop` 不能 await，因此交给一个 detached
        // 任务。锁有 7200s TTL 兜底，最坏情况只是残留一个过期键。
        if let Some((conn, token)) = self.redis.take() {
            let key = hls_lock_key(self.video_id);
            let video_id = self.video_id;
            tokio::spawn(async move {
                let mut conn = conn.as_ref().clone();
                let released: Result<i64, _> = redis::Script::new(HLS_UNLOCK_SCRIPT)
                    .key(&key)
                    .arg(&token)
                    .invoke_async(&mut conn)
                    .await;
                if let Err(e) = released {
                    tracing::warn!(
                        video_id,
                        error = %e,
                        "failed to release HLS redis lock (will expire via TTL)"
                    );
                }
            });
        }
    }
}

fn hls_lock_key(video_id: i64) -> String {
    format!("hls:transcode:{video_id}")
}

/// Unique value stored in the Redis lock, so the release script can tell "my
/// lock" from "a lock another instance took after mine expired".
fn new_lock_token(video_id: i64) -> String {
    use rand::Rng;
    let mut buf = [0u8; 8];
    rand::thread_rng().fill(&mut buf);
    format!(
        "{}:{}-{}",
        std::process::id(),
        video_id,
        buf.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

impl Transcoder {
    pub fn new(media_root: &Path, settings: TranscodeSettings) -> Self {
        let output_dir = media_root.join("variants");
        let hls_dir = media_root.join("hls");
        if let Err(e) = std::fs::create_dir_all(&output_dir) {
            tracing::warn!(
                "failed to create variants directory {}: {}",
                output_dir.display(),
                e
            );
        }
        if let Err(e) = std::fs::create_dir_all(&hls_dir) {
            tracing::warn!(
                "failed to create HLS directory {}: {}",
                hls_dir.display(),
                e
            );
        }

        Transcoder {
            ffmpeg_path: settings.ffmpeg_path.clone(),
            ffprobe_path: settings.ffprobe_path.clone(),
            output_dir,
            hls_dir,
            semaphore: Arc::new(Semaphore::new(settings.concurrency.max(1))),
            settings,
        }
    }

    /// 暴露全局转码信号量：调用方（如 HLS 后台任务）可 `acquire_owned()`
    /// 以复用同一并发上限，避免绕开 `transcode()` 内的限流。
    pub fn semaphore(&self) -> Arc<Semaphore> {
        self.semaphore.clone()
    }

    /// 尝试声明开始 `video_id` 的 HLS 转码。该视频已有任务在途时返回
    /// `None`（调用方应返回 409），否则返回的 guard 持有期间其它请求都
    /// 会被拒绝，直到 guard 被 drop（任务结束或 panic）。
    ///
    /// 两级去重：
    /// 1. 进程内 `HashSet` —— 无网络开销，覆盖单实例（默认）部署与 Redis 不可用场景；
    /// 2. Redis `SET NX EX` —— 仅在第 1 步通过后执行，覆盖多实例部署，避免
    ///    两个实例同时向同一个 `hls/{video_id}/` 目录写 ffmpeg 输出。
    ///
    /// Redis 不可达时降级为仅进程内去重（多实例下可能重复转码，但不会阻塞
    /// 请求），并记警告——宁可浪费一次 CPU，也不要因为 Redis 抖动让 HLS 不可用。
    pub async fn try_begin_hls(
        &self,
        video_id: i64,
        redis: &crate::services::redis::SharedRedis,
    ) -> Option<HlsInFlightGuard> {
        // Level 1: local. Cheap and authoritative for this process.
        {
            let mut set = hls_in_flight()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !set.insert(video_id) {
                return None;
            }
        }

        // Level 2: distributed. Only now do we pay for a network round trip.
        let Some(conn) = redis.resolve() else {
            return Some(HlsInFlightGuard {
                video_id,
                redis: None,
            });
        };
        let token = new_lock_token(video_id);
        let key = hls_lock_key(video_id);
        let mut handle = conn.as_ref().clone();
        let acquired: Result<Option<String>, _> = redis::cmd("SET")
            .arg(&key)
            .arg(&token)
            .arg("NX")
            .arg("EX")
            .arg(HLS_LOCK_TTL_SECS)
            .query_async(&mut handle)
            .await;

        match acquired {
            // `SET NX` returns nil when the key already existed.
            Ok(Some(_)) => Some(HlsInFlightGuard {
                video_id,
                redis: Some((conn, token)),
            }),
            Ok(None) => {
                // Another instance is transcoding this video: undo the local
                // reservation so a later retry can succeed once it finishes.
                let mut set = hls_in_flight()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                set.remove(&video_id);
                None
            }
            Err(e) => {
                tracing::warn!(
                    video_id,
                    error = %e,
                    "HLS dedup: Redis unavailable, falling back to process-local only"
                );
                Some(HlsInFlightGuard {
                    video_id,
                    redis: None,
                })
            }
        }
    }

    pub async fn transcode(
        &self,
        video_id: i64,
        input_path: &Path,
        resolutions: Vec<String>,
    ) -> Result<Vec<VideoVariant>> {
        use tokio::task::JoinSet;

        // Deduplicate resolutions while preserving order — duplicate entries
        // would otherwise race on the same output file.
        let mut seen = HashSet::new();
        let resolutions: Vec<String> = resolutions
            .into_iter()
            .filter(|r| seen.insert(r.clone()))
            .collect();

        let mut join_set = JoinSet::new();

        for resolution in &resolutions {
            // Whitelist the resolution before it is interpolated into a
            // filesystem path: an unvalidated value could smuggle `/` or
            // `..` into `get_output_path` and escape the variants directory.
            resolution_params(resolution)
                .ok_or_else(|| anyhow!("Unsupported resolution: {}", resolution))?;
            let output_path = self.get_output_path(video_id, resolution);
            let args = self.build_ffmpeg_args(input_path, &output_path, resolution)?;

            tracing::info!(
                "Starting transcode: video_id={}, resolution={}, input={:?}, output={:?}",
                video_id,
                resolution,
                input_path,
                output_path
            );

            let ffmpeg_path = self.ffmpeg_path.clone();
            let resolution = resolution.clone();
            let output_path_clone = output_path.clone();
            let bitrate = self.get_bitrate(&resolution);
            let timeout = self.settings.transcode_timeout;
            let max_duration_secs = self.settings.max_duration_secs;
            let semaphore = self.semaphore.clone();

            join_set.spawn(async move {
                // Cap the number of concurrent ffmpeg processes.
                let _permit = match semaphore.acquire().await {
                    Ok(p) => p,
                    Err(_) => return Err(anyhow!("transcode semaphore closed")),
                };

                // Run ffmpeg as a child process with a hard timeout;
                // kill_on_drop guarantees the child is reaped (not orphaned)
                // if we abort the future, e.g. on timeout or when the
                // JoinSet is dropped after another resolution failed.
                let output = tokio::time::timeout(
                    timeout,
                    Command::new(&ffmpeg_path)
                        .args(&args)
                        .kill_on_drop(true)
                        .output(),
                )
                .await
                .map_err(|_| anyhow!("ffmpeg timed out after {:?}", timeout))?
                .context("Failed to execute ffmpeg")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("FFmpeg failed: {}", truncate_stderr(&stderr));
                }

                let metadata = tokio::fs::metadata(&output_path_clone).await?;
                let file_size = metadata.len() as i64;

                // Disk-quota guard: delete any output that blew past the
                // size cap so a pathological input cannot leave junk on disk
                // or bypass the per-user storage quota by multiplier.
                if let Some(limit) = max_variant_size_bytes(&resolution, max_duration_secs) {
                    if metadata.len() > limit {
                        let _ = tokio::fs::remove_file(&output_path_clone).await;
                        anyhow::bail!(
                            "Output exceeds size limit ({} bytes > {} bytes)",
                            metadata.len(),
                            limit
                        );
                    }
                }

                Ok::<_, anyhow::Error>(VideoVariant {
                    id: 0,
                    video_id,
                    resolution,
                    file_path: output_path_clone.to_string_lossy().to_string(),
                    file_size,
                    bitrate,
                    codec: "h264".to_string(),
                })
            });
        }

        let mut variants = Vec::new();
        while let Some(result) = join_set.join_next().await {
            variants.push(result??);
        }

        Ok(variants)
    }

    fn build_ffmpeg_args(
        &self,
        input: &Path,
        output: &Path,
        resolution: &str,
    ) -> Result<Vec<String>> {
        let (width, height, bitrate) = resolution_params(resolution)
            .ok_or_else(|| anyhow!("Unsupported resolution: {}", resolution))?;

        Ok(vec![
            "-i".to_string(),
            input.to_string_lossy().to_string(),
            "-vf".to_string(),
            format!("scale={}:{}", width, height),
            "-c:v".to_string(),
            "libx264".to_string(),
            "-preset".to_string(),
            "medium".to_string(),
            "-crf".to_string(),
            "23".to_string(),
            "-b:v".to_string(),
            format!("{}k", bitrate),
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "128k".to_string(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            "-y".to_string(),
            output.to_string_lossy().to_string(),
        ])
    }

    fn get_output_path(&self, video_id: i64, resolution: &str) -> PathBuf {
        self.output_dir
            .join(format!("{}_{}.mp4", video_id, resolution))
    }

    fn get_bitrate(&self, resolution: &str) -> Option<i32> {
        resolution_params(resolution).and_then(|(_, _, b)| i32::try_from(b).ok())
    }

    pub async fn get_video_info(&self, video_path: &Path) -> Result<VideoInfo> {
        let output = tokio::time::timeout(
            self.settings.ffprobe_timeout,
            Command::new(&self.ffprobe_path)
                .args([
                    "-v",
                    "quiet",
                    "-print_format",
                    "json",
                    "-show_format",
                    "-show_streams",
                    video_path.to_str().unwrap_or_default(),
                ])
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| anyhow!("ffprobe timed out"))?
        .context("Failed to execute ffprobe")?;

        if !output.status.success() {
            anyhow::bail!("FFprobe failed");
        }

        let info: VideoInfo =
            serde_json::from_slice(&output.stdout).context("Failed to parse FFprobe output")?;

        Ok(info)
    }

    /// Transcode video to HLS format with multiple bitrate variants
    pub async fn transcode_to_hls(&self, video_id: i64, input_path: &Path) -> Result<HlsPlaylist> {
        let video_hls_dir = self.hls_dir.join(video_id.to_string());
        tokio::fs::create_dir_all(&video_hls_dir).await?;

        let master_playlist_path = video_hls_dir.join("master.m3u8");

        // Define bitrate variants for adaptive streaming
        let variants = vec![
            ("720p", 2500, 1280, 720),
            ("480p", 1000, 854, 480),
            ("360p", 600, 640, 360),
        ];

        let mut variant_playlists = Vec::new();

        for (name, bitrate, width, height) in &variants {
            let variant_dir = video_hls_dir.join(name);
            tokio::fs::create_dir_all(&variant_dir).await?;

            let playlist_path = variant_dir.join("index.m3u8");
            let segment_pattern = variant_dir.join("segment_%03d.ts");

            let args = vec![
                "-i".to_string(),
                input_path.to_string_lossy().to_string(),
                "-vf".to_string(),
                format!("scale={}:{}", width, height),
                "-c:v".to_string(),
                "libx264".to_string(),
                "-preset".to_string(),
                "medium".to_string(),
                "-crf".to_string(),
                "23".to_string(),
                "-b:v".to_string(),
                format!("{}k", bitrate),
                "-c:a".to_string(),
                "aac".to_string(),
                "-b:a".to_string(),
                "128k".to_string(),
                "-f".to_string(),
                "hls".to_string(),
                "-hls_time".to_string(),
                "6".to_string(),
                "-hls_list_size".to_string(),
                "0".to_string(),
                "-hls_segment_filename".to_string(),
                segment_pattern.to_string_lossy().to_string(),
                "-y".to_string(),
                playlist_path.to_string_lossy().to_string(),
            ];

            let output = tokio::time::timeout(
                self.settings.transcode_timeout,
                Command::new(&self.ffmpeg_path)
                    .args(&args)
                    .kill_on_drop(true)
                    .output(),
            )
            .await
            .map_err(|_| anyhow!("HLS transcode timed out"))?
            .context("Failed to execute ffmpeg for HLS")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("HLS FFmpeg failed: {}", truncate_stderr(&stderr));
            }

            variant_playlists.push(HlsVariant {
                name: name.to_string(),
                bitrate: *bitrate,
                width: *width,
                height: *height,
                playlist_url: format!("/media/hls/{}/{}/index.m3u8", video_id, name),
            });
        }

        // Generate master playlist
        let master_content = self.generate_master_playlist(&variant_playlists);
        tokio::fs::write(&master_playlist_path, master_content).await?;

        Ok(HlsPlaylist {
            video_id,
            master_url: format!("/media/hls/{}/master.m3u8", video_id),
            variants: variant_playlists,
        })
    }

    fn generate_master_playlist(&self, variants: &[HlsVariant]) -> String {
        let mut content = String::from("#EXTM3U\n#EXT-X-VERSION:3\n\n");

        for variant in variants {
            content.push_str(&format!(
                "#EXT-X-STREAM-INF:BANDWIDTH={},RESOLUTION={}x{}\n{}\n\n",
                variant.bitrate * 1000,
                variant.width,
                variant.height,
                variant.playlist_url
            ));
        }

        content
    }

    pub async fn delete_variant(&self, video_id: i64, resolution: &str) -> Result<()> {
        // Unknown resolutions can never have a variant, so treat them as a
        // no-op. This also rejects path-traversal attempts smuggled through
        // the `resolution` parameter (it is embedded into a filesystem path).
        if resolution_params(resolution).is_none() {
            return Ok(());
        }
        let path = self.get_output_path(video_id, resolution);
        match tokio::fs::metadata(&path).await {
            Ok(_) => tokio::fs::remove_file(&path)
                .await
                .context("Failed to delete variant file")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).context("Failed to stat variant file"),
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoInfo {
    pub format: Option<FormatInfo>,
    pub streams: Option<Vec<StreamInfo>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FormatInfo {
    pub duration: Option<String>,
    pub size: Option<String>,
    pub bit_rate: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamInfo {
    pub codec_type: String,
    pub codec_name: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub bit_rate: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway transcoder rooted at a temp dir, for exercising the pure
    /// path/参数 helpers that would otherwise need a real ffmpeg run.
    fn test_transcoder() -> Transcoder {
        let dir = std::env::temp_dir().join(format!(
            "atmos-tx-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        Transcoder::new(&dir, TranscodeSettings::default())
    }

    /// Calls the real `get_output_path`. The previous version of this test
    /// rebuilt the path with `format!` inside the test body and never touched
    /// the function under test, so it could not fail.
    #[test]
    fn get_output_path_uses_the_variants_dir() {
        let tx = test_transcoder();
        let path = tx.get_output_path(42, "720p");
        assert_eq!(
            path,
            tx.output_dir.join("42_720p.mp4"),
            "variant path must be <output_dir>/<video_id>_<resolution>.mp4"
        );
        assert!(path.starts_with(&tx.output_dir));
        assert!(path.to_string_lossy().ends_with(".mp4"));
    }

    #[test]
    fn get_output_path_separates_videos_and_resolutions() {
        let tx = test_transcoder();
        let a = tx.get_output_path(1, "720p");
        let b = tx.get_output_path(1, "1080p");
        let c = tx.get_output_path(2, "720p");
        assert_ne!(a, b, "different resolutions must not collide");
        assert_ne!(a, c, "different videos must not collide");
    }

    /// Calls the real `get_bitrate`. The previous version recomputed the same
    /// `match` expression inside the test, which made it tautological.
    #[test]
    fn get_bitrate_matches_the_resolution_ladder() {
        let tx = test_transcoder();
        // (resolution, width, height, kbps) — the ladder is ordered
        // high-to-low, and bitrate must fall monotonically with it.
        let ladder = [
            ("2160p", 3840, 2160, 8000i32),
            ("1080p", 1920, 1080, 5000),
            ("720p", 1280, 720, 2500),
            ("480p", 854, 480, 1000),
            ("360p", 640, 360, 600),
        ];
        let mut previous: Option<i32> = None;
        for (resolution, w, h, expected_kbps) in ladder {
            let params = resolution_params(resolution)
                .unwrap_or_else(|| panic!("{resolution} must be a known resolution"));
            assert_eq!(params.0, w, "{resolution} width");
            assert_eq!(params.1, h, "{resolution} height");
            assert_eq!(params.2 as i32, expected_kbps, "{resolution} bitrate");
            assert_eq!(
                tx.get_bitrate(resolution),
                Some(expected_kbps),
                "get_bitrate({resolution})"
            );
            if let Some(prev) = previous {
                assert!(
                    expected_kbps < prev,
                    "bitrate must decrease down the ladder ({resolution})"
                );
            }
            previous = Some(expected_kbps);
        }
    }

    #[test]
    fn get_bitrate_rejects_unknown_resolutions() {
        let tx = test_transcoder();
        for bogus in [
            "",
            "unknown",
            "1080",
            "1080P", // case matters: the ladder is matched exactly
            "720p/../",
            "../../etc/passwd",
            "2160p.mp4",
        ] {
            assert_eq!(
                tx.get_bitrate(bogus),
                None,
                "{bogus:?} must not resolve to a bitrate"
            );
        }
    }

    /// `delete_variant` embeds `resolution` into a filesystem path, so an
    /// unrecognised value must be a no-op rather than a traversal primitive.
    #[tokio::test]
    async fn delete_variant_ignores_unknown_resolutions() {
        let tx = test_transcoder();
        for bogus in ["../../etc/passwd", "720p/../../../../tmp/x", "", "720P"] {
            tx.delete_variant(1, bogus)
                .await
                .expect("unknown resolutions must be a silent no-op");
        }
    }

    /// The disk-quota guard: bitrate × max duration bounds the output size, and
    /// an unknown resolution has no bound at all (so it must be rejected rather
    /// than defaulting to "unlimited").
    #[test]
    fn max_variant_size_scales_with_bitrate_and_duration() {
        let ten_minutes = 600u64;
        let eight_mbps =
            max_variant_size_bytes("2160p", ten_minutes).expect("2160p must have a size cap");
        let one_mbps = max_variant_size_bytes("360p", ten_minutes).expect("360p must have a cap");
        assert!(
            eight_mbps > one_mbps,
            "a higher bitrate must allow a larger file"
        );
        // 8000 kbps = 1_000_000 bytes/s, so 600s = 600_000_000 bytes.
        assert_eq!(eight_mbps, 8000 * 1000 / 8 * ten_minutes);
        // Doubling the duration doubles the cap.
        assert_eq!(
            max_variant_size_bytes("720p", ten_minutes * 2).unwrap(),
            max_variant_size_bytes("720p", ten_minutes).unwrap() * 2
        );
        assert_eq!(max_variant_size_bytes("nope", ten_minutes), None);
    }

    #[test]
    fn truncate_stderr_bounds_hostile_ffmpeg_output() {
        assert_eq!(truncate_stderr("short"), "short");
        // Multi-byte characters must not be split mid-codepoint.
        let long_cjk = "错".repeat(1000);
        let out = truncate_stderr(&long_cjk);
        assert!(
            out.chars().count() <= 512 + 40,
            "got {}",
            out.chars().count()
        );
        assert!(out.contains("more bytes not shown"));
    }
}
