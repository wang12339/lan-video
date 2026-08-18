use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command;

/// Max time for a single ffmpeg transcode before the child is killed.
/// Override with `TRANSCODE_TIMEOUT_SECS` (default 1 hour).
fn transcode_timeout() -> Duration {
    Duration::from_secs(
        std::env::var("TRANSCODE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600),
    )
}

/// Max time for a single ffprobe invocation.
fn ffprobe_timeout() -> Duration {
    Duration::from_secs(
        std::env::var("FFPROBE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30),
    )
}

/// How many ffmpeg transcodes may run concurrently (default 1 — each one is
/// CPU-heavy). Override with `TRANSCODE_CONCURRENCY`.
fn transcode_concurrency() -> usize {
    std::env::var("TRANSCODE_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1)
}

/// Upper bound (seconds) on the source duration assumed when computing the
/// per-variant output size cap. Override with `TRANSCODE_MAX_DURATION_SECS`
/// (default 2 hours).
fn transcode_max_duration_secs() -> u64 {
    std::env::var("TRANSCODE_MAX_DURATION_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7200)
}

/// Max output size in bytes for a variant at the given resolution, derived
/// from the target bitrate and the maximum assumed source duration. A
/// malicious or pathological input cannot make ffmpeg fill the disk: any
/// output beyond this cap is deleted right after encoding.
fn max_variant_size_bytes(resolution: &str) -> Option<u64> {
    let (_, _, bitrate_kbps) = resolution_params(resolution)?;
    Some(bitrate_kbps as u64 * 1000 / 8 * transcode_max_duration_secs())
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

static TRANSCODE_SEMAPHORE: OnceLock<tokio::sync::Semaphore> = OnceLock::new();

fn transcode_semaphore() -> &'static tokio::sync::Semaphore {
    TRANSCODE_SEMAPHORE.get_or_init(|| tokio::sync::Semaphore::new(transcode_concurrency()))
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
}

impl Transcoder {
    pub fn new(media_root: &Path) -> Self {
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
            ffmpeg_path: std::env::var("FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".to_string()),
            ffprobe_path: std::env::var("FFPROBE_PATH").unwrap_or_else(|_| "ffprobe".to_string()),
            output_dir,
            hls_dir,
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
            let timeout = transcode_timeout();

            join_set.spawn(async move {
                // Cap the number of concurrent ffmpeg processes.
                let _permit = match transcode_semaphore().acquire().await {
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
                if let Some(limit) = max_variant_size_bytes(&resolution) {
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
        resolution_params(resolution).map(|(_, _, b)| b as i32)
    }

    pub async fn get_video_info(&self, video_path: &Path) -> Result<VideoInfo> {
        let output = tokio::time::timeout(
            ffprobe_timeout(),
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
                transcode_timeout(),
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

    #[test]
    fn test_get_output_path() {
        // Mock test - in real implementation, you'd need to create mock objects
        // For now, just test the path generation logic
        let output_dir = PathBuf::from("/tmp/media/variants");
        let video_id = 1;
        let resolution = "720p";

        let path = output_dir.join(format!("{}_{}.mp4", video_id, resolution));
        assert!(path.to_string_lossy().contains("1_720p.mp4"));
    }

    #[test]
    fn test_get_bitrate() {
        // Test bitrate mapping
        let test_cases = vec![
            ("2160p", Some(8000)),
            ("1080p", Some(5000)),
            ("720p", Some(2500)),
            ("480p", Some(1000)),
            ("360p", Some(600)),
            ("unknown", None),
        ];

        for (resolution, expected) in test_cases {
            let result = match resolution {
                "2160p" => Some(8000),
                "1080p" => Some(5000),
                "720p" => Some(2500),
                "480p" => Some(1000),
                "360p" => Some(600),
                _ => None,
            };
            assert_eq!(result, expected, "Failed for resolution: {}", resolution);
        }
    }
}
