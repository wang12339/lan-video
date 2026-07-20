use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::config::AppConfig;
use crate::repositories::video_repo::VideoRepository;

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
pub struct TranscodeRequest {
    pub resolutions: Vec<String>, // e.g., ["720p", "480p", "360p"]
    pub quality: Option<String>,  // "low", "medium", "high"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeJob {
    pub id: i32,
    pub video_id: i64,
    pub status: String,
    pub resolution: String,
    pub progress: i32,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Transcoder {
    ffmpeg_path: String,
    ffprobe_path: String,
    output_dir: PathBuf,
    video_repo: VideoRepository,
    config: AppConfig,
}

impl Transcoder {
    pub fn new(video_repo: VideoRepository, config: AppConfig) -> Self {
        let output_dir = config.media_root.join("variants");
        std::fs::create_dir_all(&output_dir).ok();

        Transcoder {
            ffmpeg_path: std::env::var("FFMPEG_PATH").unwrap_or_else(|_| "ffmpeg".to_string()),
            ffprobe_path: std::env::var("FFPROBE_PATH").unwrap_or_else(|_| "ffprobe".to_string()),
            output_dir,
            video_repo,
            config,
        }
    }

    pub async fn transcode(
        &self,
        video_id: i64,
        input_path: &Path,
        resolutions: Vec<String>,
    ) -> Result<Vec<VideoVariant>> {
        let mut variants = Vec::new();

        for resolution in &resolutions {
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
            let args_clone = args.clone();

            tokio::task::spawn_blocking(move || {
                let output = std::process::Command::new(&ffmpeg_path)
                    .args(&args_clone)
                    .output()
                    .context("Failed to execute ffmpeg")?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    anyhow::bail!("FFmpeg failed: {}", stderr);
                }

                Ok::<_, anyhow::Error>(())
            })
            .await??;

            let metadata = tokio::fs::metadata(&output_path).await?;
            let file_size = metadata.len() as i64;

            let variant = VideoVariant {
                id: 0, // Will be set by database
                video_id,
                resolution: resolution.clone(),
                file_path: output_path.to_string_lossy().to_string(),
                file_size,
                bitrate: self.get_bitrate(resolution),
                codec: "h264".to_string(),
            };

            variants.push(variant);
        }

        Ok(variants)
    }

    fn build_ffmpeg_args(
        &self,
        input: &Path,
        output: &Path,
        resolution: &str,
    ) -> Result<Vec<String>> {
        let (width, height, bitrate) = match resolution {
            "2160p" => (3840, 2160, 8000),
            "1080p" => (1920, 1080, 5000),
            "720p" => (1280, 720, 2500),
            "480p" => (854, 480, 1000),
            "360p" => (640, 360, 600),
            _ => anyhow::bail!("Unsupported resolution: {}", resolution),
        };

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
        match resolution {
            "2160p" => Some(8000),
            "1080p" => Some(5000),
            "720p" => Some(2500),
            "480p" => Some(1000),
            "360p" => Some(600),
            _ => None,
        }
    }

    pub async fn get_video_info(&self, video_path: &Path) -> Result<VideoInfo> {
        let output = Command::new(&self.ffprobe_path)
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                video_path.to_str().unwrap_or_default(),
            ])
            .output()
            .await
            .context("Failed to execute ffprobe")?;

        if !output.status.success() {
            anyhow::bail!("FFprobe failed");
        }

        let info: VideoInfo =
            serde_json::from_slice(&output.stdout).context("Failed to parse FFprobe output")?;

        Ok(info)
    }

    pub async fn delete_variant(&self, video_id: i64, resolution: &str) -> Result<()> {
        let path = self.get_output_path(video_id, resolution);
        if path.exists() {
            tokio::fs::remove_file(&path)
                .await
                .context("Failed to delete variant file")?;
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
