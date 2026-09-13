//! 聊天室视频格式兼容处理。
//!
//! 用户上传的视频容器/编码五花八门（iPhone 的 .mov/HEVC、安卓的 .mkv 等），
//! 浏览器只能稳定播放 H.264/AAC 的 MP4 与 WebM。本模块负责：
//!   - 白名单校验输入扩展名；
//!   - ffprobe 探测时长（超限拒绝）与编码；
//!   - H.264 + AAC/MP3 直接 remux 成 MP4（秒级、不损质量）；
//!   - 其余（HEVC/VP9/AV1/其他容器）转码为 H.264/AAC MP4（限宽 1280）；
//!   - 输出大小上限兜底，避免异常输入把磁盘写满。
//!
//! WebM 保持原样（浏览器原生支持），不做转换。

use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

/// 允许上传的输入扩展名（真正格式仍由 magic bytes 校验兜底）。
pub const ALLOWED_INPUT_EXTS: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv", "avi"];

/// 聊天视频最长时长（秒）。
pub const MAX_DURATION_SECS: f64 = 300.0;
/// 转码后输出大小上限（转码可能让体积膨胀）。
pub const MAX_OUTPUT_BYTES: u64 = 200 * 1024 * 1024;
/// 单次 ffmpeg/ffprobe 调用超时。
const FFMPEG_TIMEOUT_SECS: u64 = 240;
const FFPROBE_TIMEOUT_SECS: u64 = 20;
/// 并发转码上限（与其他 ffmpeg 任务错开，防止拖垮机器）。
const MAX_CONCURRENT_TRANSCODES: usize = 2;

fn transcode_semaphore() -> &'static tokio::sync::Semaphore {
    static SEM: std::sync::OnceLock<tokio::sync::Semaphore> = std::sync::OnceLock::new();
    SEM.get_or_init(|| tokio::sync::Semaphore::new(MAX_CONCURRENT_TRANSCODES))
}

#[derive(Debug, Default)]
struct ProbeResult {
    duration_secs: Option<f64>,
    video_codec: Option<String>,
    audio_codec: Option<String>,
    has_audio: bool,
}

/// 用 ffprobe 读取时长与编解码信息。失败返回 Err（调用方决定是否降级）。
async fn probe(ffprobe: &str, input: &Path) -> Result<ProbeResult, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(FFPROBE_TIMEOUT_SECS),
        Command::new(ffprobe)
            .kill_on_drop(true)
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration:stream=codec_type,codec_name")
            .arg("-of")
            .arg("json")
            .arg(input)
            .output(),
    )
    .await
    .map_err(|_| format!("ffprobe 超时（{}s）", FFPROBE_TIMEOUT_SECS))?
    .map_err(|e| format!("ffprobe 不可用: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe 解析失败: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("")
        ));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("ffprobe 输出解析失败: {}", e))?;

    let mut result = ProbeResult {
        duration_secs: json
            .get("format")
            .and_then(|f| f.get("duration"))
            .and_then(|d| d.as_str())
            .and_then(|d| d.parse::<f64>().ok()),
        ..Default::default()
    };

    if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
        for stream in streams {
            match stream.get("codec_type").and_then(|t| t.as_str()) {
                Some("video") if result.video_codec.is_none() => {
                    result.video_codec = stream
                        .get("codec_name")
                        .and_then(|c| c.as_str())
                        .map(|s| s.to_ascii_lowercase());
                }
                Some("audio") => {
                    result.has_audio = true;
                    if result.audio_codec.is_none() {
                        result.audio_codec = stream
                            .get("codec_name")
                            .and_then(|c| c.as_str())
                            .map(|s| s.to_ascii_lowercase());
                    }
                }
                _ => {}
            }
        }
    }
    Ok(result)
}

/// MP4 容器可安全容纳、且浏览器普遍可播的音频编码。
fn audio_mp4_compatible(codec: Option<&str>) -> bool {
    matches!(codec, None | Some("aac") | Some("mp3"))
}

/// 把任意支持的输入转换为浏览器可播的 MP4。
///
/// `ext` 为小写输入扩展名（须在 [`ALLOWED_INPUT_EXTS`] 内）。
/// 返回 `Ok(Some(file_name))` 表示已生成转换后的文件（在 `out_dir` 下）；
/// `Ok(None)` 表示无需转换（WebM 原样使用）。
/// 失败返回用户可读的中文原因；失败时不留下半成品文件。
pub async fn prepare_video(
    input: &Path,
    ext: &str,
    out_dir: &Path,
    ffmpeg: &str,
    ffprobe: &str,
) -> Result<Option<String>, String> {
    let probe = match probe(ffprobe, input).await {
        Ok(p) => p,
        Err(probe_err) => {
            // ffprobe 不可用时 WebM 按原样保留（不阻断上传）；其余报错
            if ext == "webm" {
                return Ok(None);
            }
            tracing::warn!("chat video ffprobe failed: {}", probe_err);
            return Err("视频无法解析，请确认文件未损坏".into());
        }
    };

    // WebM 且编码为浏览器原生支持的 VP8/VP9/AV1 → 原样保留；
    // 否则（如 H.264 的 mkv 改名 webm）走下方转码兜底。
    if ext == "webm"
        && matches!(
            probe.video_codec.as_deref(),
            Some("vp8") | Some("vp9") | Some("av1")
        )
    {
        return Ok(None);
    }

    if let Some(dur) = probe.duration_secs {
        if dur > MAX_DURATION_SECS {
            return Err(format!(
                "视频过长（{} 分钟），聊天室视频最长 {} 分钟",
                (dur / 60.0).round() as u64,
                (MAX_DURATION_SECS / 60.0) as u64
            ));
        }
    }

    let is_h264 = probe.video_codec.as_deref() == Some("h264");
    let can_remux = is_h264 && audio_mp4_compatible(probe.audio_codec.as_deref());

    let out_name = format!("{}.mp4", uuid::Uuid::new_v4().simple());
    let out_path = out_dir.join(&out_name);

    // 限并发，避免同时转多个大文件拖垮机器
    let _permit = transcode_semaphore()
        .acquire()
        .await
        .map_err(|_| "转码任务繁忙，请稍后再试".to_string())?;

    let mut last_err = String::new();

    // 1) 优先 remux（不重新编码，秒级完成）
    if can_remux {
        let result = run_ffmpeg(
            ffmpeg,
            &[
                "-y",
                "-i",
                &input.to_string_lossy(),
                "-map",
                "0:v:0",
                "-map",
                "0:a:0?",
                "-c",
                "copy",
                "-movflags",
                "+faststart",
                &out_path.to_string_lossy(),
            ],
        )
        .await;
        match result {
            Ok(()) => return finalize(&out_path, &out_name).map(Some),
            Err(e) => {
                last_err = e;
                let _ = tokio::fs::remove_file(&out_path).await;
            }
        }
    }

    // 2) 转码为 H.264/AAC；限宽 1280 以防 4K 重编码过慢
    let result = run_ffmpeg(
        ffmpeg,
        &[
            "-y",
            "-i",
            &input.to_string_lossy(),
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-vf",
            "scale='min(1280,iw)':-2",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
            &out_path.to_string_lossy(),
        ],
    )
    .await;

    match result {
        Ok(()) => finalize(&out_path, &out_name).map(Some),
        Err(e) => {
            let _ = tokio::fs::remove_file(&out_path).await;
            tracing::warn!(
                "chat video transcode failed (remux_err: {}, encode_err: {})",
                last_err,
                e
            );
            Err("视频转码失败，请尝试其他格式".into())
        }
    }
}

/// 校验输出文件存在且未超上限，返回文件名。
fn finalize(out_path: &Path, out_name: &str) -> Result<String, String> {
    let meta = std::fs::metadata(out_path).map_err(|_| "转码输出缺失".to_string())?;
    if meta.len() == 0 {
        let _ = std::fs::remove_file(out_path);
        return Err("视频转码失败，请尝试其他格式".into());
    }
    if meta.len() > MAX_OUTPUT_BYTES {
        let _ = std::fs::remove_file(out_path);
        return Err("视频过大，请选择更短的片段".into());
    }
    Ok(out_name.to_string())
}

async fn run_ffmpeg(ffmpeg: &str, args: &[&str]) -> Result<(), String> {
    let output = tokio::time::timeout(
        Duration::from_secs(FFMPEG_TIMEOUT_SECS),
        Command::new(ffmpeg).kill_on_drop(true).args(args).output(),
    )
    .await
    .map_err(|_| format!("转码超时（{}s）", FFMPEG_TIMEOUT_SECS))?
    .map_err(|e| format!("ffmpeg 不可用: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr)
            .lines()
            .last()
            .unwrap_or("")
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_exts_cover_common_phone_formats() {
        for ext in ["mp4", "m4v", "mov", "webm", "mkv", "avi"] {
            assert!(ALLOWED_INPUT_EXTS.contains(&ext), "{ext} 应被接受");
        }
        assert!(!ALLOWED_INPUT_EXTS.contains(&"exe"));
        assert!(!ALLOWED_INPUT_EXTS.contains(&"gif"));
    }

    #[test]
    fn audio_mp4_compat_matrix() {
        assert!(audio_mp4_compatible(None));
        assert!(audio_mp4_compatible(Some("aac")));
        assert!(audio_mp4_compatible(Some("mp3")));
        assert!(!audio_mp4_compatible(Some("opus")));
        assert!(!audio_mp4_compatible(Some("vorbis")));
        assert!(!audio_mp4_compatible(Some("pcm_s16le")));
    }

    fn ffmpeg_available() -> bool {
        std::process::Command::new("ffmpeg")
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// 真实转换冒烟：mkv(H.264) → mp4（remux 路径）。无 ffmpeg 时跳过。
    #[tokio::test]
    async fn prepare_video_converts_mkv_to_mp4() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg not available, skipping");
            return;
        }
        let dir = std::env::temp_dir().join(format!("atmos_chatvid_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("in.mkv");
        // 生成 0.5s 的 H.264 mkv
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=s=160x120:d=0.5",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-y",
            ])
            .arg(&input)
            .status()
            .unwrap();
        if !status.success() {
            eprintln!("ffmpeg failed to generate fixture, skipping");
            std::fs::remove_dir_all(&dir).ok();
            return;
        }

        let out = prepare_video(&input, "mkv", &dir, "ffmpeg", "ffprobe")
            .await
            .expect("mkv 应转换为 mp4");
        let name = out.expect("非 webm 必须返回转换后的文件名");
        assert!(name.ends_with(".mp4"), "输出应为 mp4: {name}");
        let out_path = dir.join(&name);
        assert!(out_path.exists(), "输出文件应存在");
        assert!(std::fs::metadata(&out_path).unwrap().len() > 0);
        // 输出应为 H.264/AAC 的 mp4（浏览器可播）
        let probe = probe("ffprobe", &out_path).await.unwrap();
        assert_eq!(probe.video_codec.as_deref(), Some("h264"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// WebM 原样保留（不做转换）。
    #[tokio::test]
    async fn prepare_video_keeps_webm() {
        let dir = std::env::temp_dir();
        let fake = dir.join(format!("atmos_webm_{}.webm", std::process::id()));
        std::fs::write(&fake, b"not really webm").unwrap();
        let out = prepare_video(&fake, "webm", &dir, "ffmpeg", "ffprobe")
            .await
            .unwrap();
        assert!(out.is_none(), "webm 不应被转换");
        std::fs::remove_file(&fake).ok();
    }
}
