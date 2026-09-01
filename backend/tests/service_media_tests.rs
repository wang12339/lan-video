//! 媒体服务 / 转码器 纯逻辑单元测试（集成测试形式）
//!
//! 覆盖 backend/src/services/media_service.rs 与 transcoder.rs 中
//! 不依赖 PostgreSQL 与真实 FFmpeg/ffprobe 的纯逻辑部分：
//!   - 文件类型 magic bytes 检测（infer_image / validate_file_type）
//!   - 文件名清理 sanitize_filename（路径穿越 / 控制字符 / 超长中文名）
//!   - 安全媒体路径校验 safe_media_path（../ 穿越 / 绝对路径 / 符号链接）
//!   - 外部 URL 校验 is_safe_external_url（内网 IP / IPv6 / localhost / userinfo）
//!   - 转码分辨率参数（通过 mock 二进制验证 build_ffmpeg_args 产生的参数）
//!   - ffprobe 视频信息 JSON 解析（get_video_info）
//!
//! 运行：cargo test --test service_media_tests
//!
//! 需要真实 PostgreSQL / FFmpeg 的用例统一用 #[ignore] 标注：
//!   DATABASE_URL=... cargo test --test service_media_tests -- --ignored

use atmos_video_backend::services::media_service::{
    infer_image, is_safe_external_url, safe_media_path, sanitize_filename, validate_file_type,
};
use atmos_video_backend::services::transcoder::{FormatInfo, Transcoder, VideoInfo};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

// ══════════════════════════════════════════════════════════════════════
// 测试辅助
// ══════════════════════════════════════════════════════════════════════

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 生成进程内唯一的临时目录名（并行测试之间互不干扰）。
fn unique_dir(prefix: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("{}_pid{}_n{}", prefix, std::process::id(), n))
}

/// 写入唯一临时文件并返回路径。
fn unique_file(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

// ══════════════════════════════════════════════════════════════════════
// 一、infer_image —— magic bytes 文件类型检测
// ══════════════════════════════════════════════════════════════════════

#[test]
fn test_infer_image_jpeg_full() {
    let bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];
    assert_eq!(infer_image(&bytes), Some(("jpg", "image/jpeg")));
}

#[test]
fn test_infer_image_jpeg_minimal() {
    // 恰好 3 字节（FF D8 FF）即可识别
    assert_eq!(
        infer_image(&[0xFF, 0xD8, 0xFF]),
        Some(("jpg", "image/jpeg"))
    );
}

#[test]
fn test_infer_image_jpeg_too_short() {
    assert_eq!(infer_image(&[0xFF, 0xD8]), None);
}

#[test]
fn test_infer_image_png() {
    let bytes = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    assert_eq!(infer_image(&bytes), Some(("png", "image/png")));
}

#[test]
fn test_infer_image_gif87a() {
    assert_eq!(infer_image(b"GIF87a"), Some(("gif", "image/gif")));
}

#[test]
fn test_infer_image_gif89a() {
    assert_eq!(infer_image(b"GIF89a\x00\x00"), Some(("gif", "image/gif")));
}

#[test]
fn test_infer_image_gif_5_bytes() {
    assert_eq!(infer_image(b"GIF89"), None);
}

#[test]
fn test_infer_image_webp() {
    // RIFF(4) + size(4) + "WEBP"(4)
    assert_eq!(
        infer_image(b"RIFF\x00\x00\x00\x00WEBP"),
        Some(("webp", "image/webp"))
    );
}

#[test]
fn test_infer_image_webp_11_bytes() {
    // 不足 12 字节不能判定
    assert_eq!(infer_image(b"RIFF\x00\x00\x00\x00WEB"), None);
}

#[test]
fn test_infer_image_webp_wrong_tag() {
    assert_eq!(infer_image(b"RIFF\x00\x00\x00\x00WEBX"), None);
}

#[test]
fn test_infer_image_webp_lowercase_riff() {
    // magic 区分大小写，小写 "riff" 不识别
    assert_eq!(infer_image(b"riff\x00\x00\x00\x00WEBP"), None);
}

#[test]
fn test_infer_image_bmp() {
    assert_eq!(
        infer_image(b"BM\x00\x00\x00\x00"),
        Some(("bmp", "image/bmp"))
    );
}

#[test]
fn test_infer_image_bmp_single_byte() {
    assert_eq!(infer_image(b"B"), None);
}

#[test]
fn test_infer_image_empty() {
    assert_eq!(infer_image(b""), None);
}

#[test]
fn test_infer_image_unknown_content() {
    assert_eq!(infer_image(b"plain text, not an image"), None);
}

// ══════════════════════════════════════════════════════════════════════
// 二、sanitize_filename —— 文件名清理
// ══════════════════════════════════════════════════════════════════════

#[test]
fn test_sanitize_filename_normal() {
    assert_eq!(sanitize_filename("my video 01.mp4"), "my video 01.mp4");
}

#[test]
fn test_sanitize_filename_unix_path_traversal() {
    // Path::file_name 只保留最后一个路径分量，../ 直接丢失
    assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
    assert_eq!(sanitize_filename("/etc/passwd"), "passwd");
    assert_eq!(sanitize_filename("a/b/c/evil.mp4"), "evil.mp4");
}

#[test]
fn test_sanitize_filename_windows_backslash_traversal() {
    // Unix 上反斜杠不是分隔符，file_name 不生效；由字符循环替换为 _
    assert_eq!(sanitize_filename("..\\..\\evil.mp4"), ".._.._evil.mp4");
}

#[test]
fn test_sanitize_filename_windows_absolute() {
    // Windows 绝对路径：整体保留后把反斜杠替换为下划线，不含分隔符
    assert_eq!(
        sanitize_filename("C:\\Windows\\system32\\evil.exe"),
        "C:_Windows_system32_evil.exe"
    );
}

#[test]
fn test_sanitize_filename_control_chars_replaced() {
    for (name, expected) in [
        ("evil\nfile.mp4", "evil_file.mp4"),
        ("evil\rfile.mp4", "evil_file.mp4"),
        ("evil\tfile.mp4", "evil_file.mp4"),
        ("evil\x1bfile.mp4", "evil_file.mp4"), // ESC
        ("evil\x07file.mp4", "evil_file.mp4"), // BEL
        ("evil\x7ffile.mp4", "evil_file.mp4"), // DEL
    ] {
        assert_eq!(sanitize_filename(name), expected, "输入: {name:?}");
    }
}

#[test]
fn test_sanitize_filename_nul_byte() {
    assert_eq!(sanitize_filename("a\0b.mp4"), "a_b.mp4");
    assert!(!sanitize_filename("a\0b.mp4").contains('\0'));
}

#[test]
fn test_sanitize_filename_only_control_chars() {
    // 控制字符全部替换为下划线；结果非空，不触发 fallback
    assert_eq!(sanitize_filename("\n\r\t\x07"), "____");
}

#[test]
fn test_sanitize_filename_whitespace_kept() {
    // 空格不是控制字符，原样保留（记录当前行为）
    assert_eq!(sanitize_filename("   "), "   ");
}

#[test]
fn test_sanitize_filename_empty_falls_back() {
    assert_eq!(sanitize_filename(""), "video.mp4");
}

#[test]
fn test_sanitize_filename_root_path_falls_back() {
    // "/" 无文件分量 → fallback
    assert_eq!(sanitize_filename("/"), "video.mp4");
}

#[test]
fn test_sanitize_filename_dotdot_falls_back() {
    // Path::new("..") 的 file_name 为 None → fallback
    assert_eq!(sanitize_filename(".."), "video.mp4");
    assert_eq!(sanitize_filename("a/.."), "video.mp4");
}

#[test]
fn test_sanitize_filename_three_dots_kept() {
    // "..." 是普通文件名分量，原样保留（记录当前行为）
    assert_eq!(sanitize_filename("..."), "...");
}

#[test]
fn test_sanitize_filename_ascii_cap_at_200() {
    let long = "a".repeat(201);
    let result = sanitize_filename(&long);
    assert_eq!(result.len(), 200);
}

#[test]
fn test_sanitize_filename_exactly_200_unchanged() {
    let name = "a".repeat(200);
    assert_eq!(sanitize_filename(&name), name);
}

#[test]
fn test_sanitize_filename_short_name_unchanged() {
    let name = "clip.mp4";
    assert_eq!(sanitize_filename(name), name);
}

#[test]
fn test_sanitize_filename_long_chinese_no_panic() {
    // 超长中文名：truncate 必须回退到字符边界，不得 panic，
    // 且结果必须是合法 UTF-8、不超过 200 字节
    let long: String = "视".repeat(300) + ".mp4";
    let result = sanitize_filename(&long);
    assert!(result.len() <= 200, "len = {}", result.len());
    assert!(result.is_char_boundary(result.len()));
    assert_eq!(result.chars().last(), Some('视'));
    assert!(String::from_utf8(result.as_bytes().to_vec()).is_ok());
}

#[test]
fn test_sanitize_filename_long_mixed_utf8_boundary() {
    // 混合单字节/多字节字符，保证截断点落在字符边界
    let long: String = "a".repeat(50) + &"视".repeat(60); // 50 + 180 = 230 字节
    let result = sanitize_filename(&long);
    assert!(result.len() <= 200);
    assert!(result.is_char_boundary(result.len()));
}

#[test]
fn test_sanitize_filename_never_contains_separators() {
    // 任何输入经过清理后都不含 / 或 \，可安全用作单一路径分量
    let inputs = [
        "../../../etc/passwd",
        "C:\\Windows\\evil.exe",
        "/very/deep/traversal.mp4",
        "\\..\\..\\",
    ];
    for input in inputs {
        let result = sanitize_filename(input);
        assert!(!result.contains('/'), "输入 {input:?} → {result:?}");
        assert!(!result.contains('\\'), "输入 {input:?} → {result:?}");
    }
}

// ══════════════════════════════════════════════════════════════════════
// 三、safe_media_path —— 安全媒体路径校验（../ 穿越 / 符号链接）
// ══════════════════════════════════════════════════════════════════════

fn make_media_root() -> PathBuf {
    let root = unique_dir("safe_media_root");
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("video.mp4"), b"data").unwrap();
    root
}

#[test]
fn test_safe_media_path_basic() {
    let root = make_media_root();
    let result = safe_media_path("/media/video.mp4", &root);
    assert!(result.is_some(), "应解析到合法路径");
    assert_eq!(result.unwrap().file_name().unwrap(), "video.mp4");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_safe_media_path_subdir() {
    let root = make_media_root();
    let result = safe_media_path("/media/sub/../video.mp4", &root);
    // "sub/.." 被规范化后仍在 root 内 → 允许
    assert!(result.is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_safe_media_path_double_slash() {
    // Unix 上 "//video.mp4" 是绝对路径，Path::join 会丢弃 base 直接拼出
    // "/video.mp4"，文件不存在 → canonicalize 失败 → None（行为安全）。
    // 注意：源码注释声称双斜杠会被 canonicalize 规范化后放行，实际在
    // Unix 上因 join 的绝对路径语义直接拒绝 —— 记录实际行为。
    let root = make_media_root();
    let result = safe_media_path("/media//video.mp4", &root);
    assert!(result.is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_safe_media_path_root_itself() {
    // "/media/" 空相对路径解析到 root 本身 → 允许（记录当前行为）
    let root = make_media_root();
    let result = safe_media_path("/media/", &root);
    assert!(result.is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_safe_media_path_traversal_blocked() {
    let root = make_media_root();
    // 目标必须已存在才能被 canonicalize 并做前缀校验；若穿越目标存在则越界
    let escape = unique_dir("escape_target");
    std::fs::create_dir_all(&escape).unwrap();
    std::fs::write(escape.join("secret.txt"), b"s").unwrap();
    // 从 root 穿到父目录再进入 escape 目录
    let parent = root.parent().unwrap();
    let target = escape
        .strip_prefix(parent)
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(safe_media_path(&format!("/media/../{target}/secret.txt"), &root).is_none());
    assert!(safe_media_path("/media/../../..", &root).is_none());
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&escape);
}

#[test]
fn test_safe_media_path_deep_traversal_escapes() {
    let root = make_media_root();
    // 反复 ../ 必然超出 root → None
    let url = "/media/sub/../../../../../../../../etc/passwd";
    assert!(safe_media_path(url, &root).is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_safe_media_path_missing_file() {
    let root = make_media_root();
    assert!(safe_media_path("/media/does_not_exist.mp4", &root).is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_safe_media_path_wrong_prefix() {
    let root = make_media_root();
    assert!(safe_media_path("media/video.mp4", &root).is_none());
    assert!(safe_media_path("/other/video.mp4", &root).is_none());
    assert!(safe_media_path("", &root).is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_safe_media_path_root_missing() {
    // media_root 本身不存在 → canonicalize 失败 → None
    let root = unique_dir("missing_root");
    assert!(safe_media_path("/media/video.mp4", &root).is_none());
}

#[cfg(unix)]
#[test]
fn test_safe_media_path_symlink_escape_blocked() {
    // 符号链接指向 root 之外 → canonicalize 解析后越界 → None
    let root = make_media_root();
    let outside = unique_dir("symlink_outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.txt"), b"s").unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("link.mp4")).unwrap();
    assert!(safe_media_path("/media/link.mp4", &root).is_none());
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&outside);
}

#[cfg(unix)]
#[test]
fn test_safe_media_path_symlink_inside_allowed() {
    // 符号链接指向 root 内部 → 规范化后仍在 root 内 → 允许
    let root = make_media_root();
    std::os::unix::fs::symlink(root.join("video.mp4"), root.join("alias.mp4")).unwrap();
    assert!(safe_media_path("/media/alias.mp4", &root).is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_safe_media_path_result_is_within_root() {
    // 防御性断言：返回值永远在 media_root 的规范化前缀之内
    let root = make_media_root();
    let canonical_root = root.canonicalize().unwrap();
    for url in [
        "/media/video.mp4",
        "/media/sub/../video.mp4",
        "/media//video.mp4",
    ] {
        if let Some(path) = safe_media_path(url, &root) {
            assert!(
                path.starts_with(&canonical_root),
                "{url} 解析结果 {path:?} 越出 root"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

// ══════════════════════════════════════════════════════════════════════
// 四、validate_file_type —— 扩展名与 magic bytes 一致性校验
// ══════════════════════════════════════════════════════════════════════

#[test]
fn test_validate_m3u8_valid() {
    let dir = unique_dir("m3u8_valid");
    let path = unique_file(
        &dir,
        "playlist.m3u8",
        b"#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:5.0,\nseg1.ts\n#EXT-X-ENDLIST\n",
    );
    assert!(validate_file_type(&path, "m3u8").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_m3u8_leading_whitespace_ok() {
    // trim_start 容忍前导空白
    let dir = unique_dir("m3u8_ws");
    let path = unique_file(&dir, "ws.m3u8", b"  \t#EXTM3U\n#EXTINF:3,\na.ts\n");
    assert!(validate_file_type(&path, "m3u8").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_m3u8_bom_rejected() {
    // BOM (\u{feff}) 不是空白，trim_start 不清除 → 拒绝（记录当前行为）
    let dir = unique_dir("m3u8_bom");
    let path = unique_file(&dir, "bom.m3u8", "\u{feff}#EXTM3U\n".as_bytes());
    assert!(validate_file_type(&path, "m3u8").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_m3u8_missing_header() {
    let dir = unique_dir("m3u8_nohead");
    let path = unique_file(&dir, "bad.m3u8", b"#EXTINF:5.0,\na.ts\n");
    let err = validate_file_type(&path, "m3u8").unwrap_err();
    assert!(err.contains("#EXTM3U"), "错误信息: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_m3u8_oversize() {
    // 超过 1 MiB 上限 → 拒绝
    let dir = unique_dir("m3u8_big");
    let mut content = Vec::with_capacity(1_048_577);
    content.extend_from_slice(b"#EXTM3U\n");
    content.resize(1_048_577, b'x');
    let path = unique_file(&dir, "big.m3u8", &content);
    let err = validate_file_type(&path, "m3u8").unwrap_err();
    assert!(err.contains("过大"), "错误信息: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_m3u8_script_tag_rejected() {
    for needle in [
        "<script>alert(1)</script>",
        "<SCRIPT>",
        "<iframe src=x>",
        "javascript:alert(1)",
        "<?xml version=\"1.0\"?>",
        "<html>",
    ] {
        let dir = unique_dir("m3u8_inject");
        let content = format!("#EXTM3U\n#EXTINF:5.0,\n{needle}\n");
        let path = unique_file(&dir, "evil.m3u8", content.as_bytes());
        assert!(
            validate_file_type(&path, "m3u8").is_err(),
            "应拒绝注入: {needle}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn test_validate_m3u8_empty() {
    let dir = unique_dir("m3u8_empty");
    let path = unique_file(&dir, "empty.m3u8", b"");
    assert!(validate_file_type(&path, "m3u8").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_png_ok() {
    let dir = unique_dir("vt_png");
    let path = unique_file(&dir, "a.png", PNG_BYTES);
    assert!(validate_file_type(&path, "png").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_png_as_jpg_rejected() {
    let dir = unique_dir("vt_png_jpg");
    let path = unique_file(&dir, "a.jpg", PNG_BYTES);
    let err = validate_file_type(&path, "jpg").unwrap_err();
    assert!(err.contains("不匹配"), "错误信息: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_jpeg_ok() {
    let dir = unique_dir("vt_jpg");
    let path = unique_file(&dir, "a.jpg", JPEG_BYTES);
    assert!(validate_file_type(&path, "jpg").is_ok());
    assert!(validate_file_type(&path, "jpeg").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_jpeg_as_png_rejected() {
    let dir = unique_dir("vt_jpg_png");
    let path = unique_file(&dir, "a.png", JPEG_BYTES);
    assert!(validate_file_type(&path, "png").is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_webp_ok() {
    let dir = unique_dir("vt_webp");
    let path = unique_file(&dir, "a.webp", b"RIFF\x10\x00\x00\x00WEBPVP8 ");
    assert!(validate_file_type(&path, "webp").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_mp4_isom_ok() {
    let dir = unique_dir("vt_mp4");
    let path = unique_file(&dir, "a.mp4", MP4_ISOM_BYTES);
    assert!(validate_file_type(&path, "mp4").is_ok());
    // mp4 容器同时接受 m4v 扩展名（同一容器格式）
    assert!(validate_file_type(&path, "m4v").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_m4v_real_file_accepted() {
    // infer 0.19 将 brand "M4V " 判定为 mime "video/x-m4v"，validate_file_type
    // 对 "m4v" 同时接受 video/mp4 与 video/x-m4v。
    let dir = unique_dir("vt_m4v");
    let path = unique_file(&dir, "a.m4v", M4V_BYTES);
    assert!(validate_file_type(&path, "m4v").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_mov_ok() {
    let dir = unique_dir("vt_mov");
    let path = unique_file(&dir, "a.mov", MOV_BYTES);
    assert!(validate_file_type(&path, "mov").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_avi_ok() {
    let dir = unique_dir("vt_avi");
    let path = unique_file(&dir, "a.avi", AVI_BYTES);
    assert!(validate_file_type(&path, "avi").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_webm_video_ok() {
    let dir = unique_dir("vt_webmv");
    let path = unique_file(&dir, "a.webm", WEBM_BYTES);
    assert!(validate_file_type(&path, "webm").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_flv_ok() {
    let dir = unique_dir("vt_flv");
    let path = unique_file(&dir, "a.flv", FLV_BYTES);
    assert!(validate_file_type(&path, "flv").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_wmv_ok() {
    let dir = unique_dir("vt_wmv");
    let path = unique_file(&dir, "a.wmv", WMV_BYTES);
    assert!(validate_file_type(&path, "wmv").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_mkv_ok() {
    let dir = unique_dir("vt_mkv");
    let path = unique_file(&dir, "a.mkv", MKV_BYTES);
    assert!(validate_file_type(&path, "mkv").is_ok());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_text_as_mp4_rejected() {
    let dir = unique_dir("vt_fakemp4");
    let path = unique_file(&dir, "fake.mp4", b"this is not a video");
    let err = validate_file_type(&path, "mp4").unwrap_err();
    assert!(
        err.contains("不匹配") || err.contains("无法识别"),
        "错误信息: {err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_unknown_extension() {
    // 字节内容可识别（PNG）但扩展名不在白名单 → "不匹配"分支
    let dir = unique_dir("vt_unknown");
    let path = unique_file(&dir, "a.xyz", PNG_BYTES);
    let err = validate_file_type(&path, "xyz").unwrap_err();
    assert!(
        err.contains("无法识别") || err.contains("不匹配"),
        "错误信息: {err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_missing_file() {
    let dir = unique_dir("vt_missing");
    let err = validate_file_type(&dir.join("nope.png"), "png").unwrap_err();
    assert!(err.contains("无法读取"), "错误信息: {err}");
}

// 各种格式的最小 magic bytes 样本
const PNG_BYTES: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE,
];
const JPEG_BYTES: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00, 0x01,
    0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
];
const MP4_ISOM_BYTES: &[u8] = b"\x00\x00\x00\x20ftypisom\x00\x00\x02\x00isomiso2mp41";
const M4V_BYTES: &[u8] = b"\x00\x00\x00\x20ftypM4V \x00\x00\x00\x00M4V mp42";
const MOV_BYTES: &[u8] = b"\x00\x00\x00\x20ftypqt  \x00\x00\x02\x00qt  ";
const AVI_BYTES: &[u8] = b"RIFF\x10\x00\x00\x00AVI LIST";
const WEBM_BYTES: &[u8] = &[
    0x1A, 0x45, 0xDF, 0xA3, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
const FLV_BYTES: &[u8] = b"FLV\x01\x05\x00\x00\x00\x09";
const WMV_BYTES: &[u8] = &[
    0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0x00, 0x00, 0x00,
];
const MKV_BYTES: &[u8] = &[
    0x1A, 0x45, 0xDF, 0xA3, 0x93, 0x42, 0x82, 0x88, 0x6D, 0x61, 0x74, 0x72, 0x6F, 0x73, 0x6B, 0x61,
];

// ══════════════════════════════════════════════════════════════════════
// 五、is_safe_external_url —— 外部流 URL 安全校验
// ══════════════════════════════════════════════════════════════════════

#[test]
fn test_safe_url_https_ok() {
    assert!(is_safe_external_url("https://example.com/video.mp4"));
    assert!(is_safe_external_url("https://example.com"));
}

#[test]
fn test_safe_url_http_ok() {
    assert!(is_safe_external_url("http://example.com/v.mp4"));
}

#[test]
fn test_safe_url_public_with_port() {
    assert!(is_safe_external_url("https://example.com:8443/v.mp4"));
    assert!(is_safe_external_url("https://8.8.8.8:443/x"));
}

#[test]
fn test_safe_url_trailing_dot_public_ok() {
    // DNS 根点：example.com. == example.com
    assert!(is_safe_external_url("http://example.com./v.mp4"));
}

#[test]
fn test_safe_url_localhost_blocked() {
    assert!(!is_safe_external_url("http://localhost/"));
    assert!(!is_safe_external_url("http://localhost:8082/admin"));
    assert!(!is_safe_external_url("http://LOCALHOST:8082/admin")); // 大小写不敏感
}

#[test]
fn test_safe_url_localhost_suffix_blocked() {
    // RFC 6761: *.localhost 解析到回环
    assert!(!is_safe_external_url("http://foo.localhost/v.mp4"));
    assert!(!is_safe_external_url("http://my-app.localhost/"));
}

#[test]
fn test_safe_url_loopback_blocked() {
    assert!(!is_safe_external_url("http://127.0.0.1/admin"));
    assert!(!is_safe_external_url("http://127.0.0.1:8080/"));
    // 127.0.0.0/8 整段都是回环
    assert!(!is_safe_external_url("http://127.0.0.2/admin"));
    assert!(!is_safe_external_url("http://127.8.8.8/"));
    assert!(!is_safe_external_url("http://127.255.255.255/"));
    // 尾点形式同样被拒绝
    assert!(!is_safe_external_url("http://127.0.0.1./admin"));
}

#[test]
fn test_safe_url_unspecified_blocked() {
    assert!(!is_safe_external_url("http://0.0.0.0/"));
    assert!(!is_safe_external_url("http://0.0.0.0:80/"));
}

#[test]
fn test_safe_url_link_local_blocked() {
    assert!(!is_safe_external_url("http://169.254.0.1/"));
}

#[test]
fn test_safe_url_cloud_metadata_blocked() {
    assert!(!is_safe_external_url(
        "http://169.254.169.254/latest/meta-data/"
    ));
    assert!(!is_safe_external_url(
        "http://100.100.100.200/latest/meta-data/"
    )); // 阿里云
}

#[test]
fn test_safe_url_private_10_blocked() {
    assert!(!is_safe_external_url("http://10.0.0.1/"));
    assert!(!is_safe_external_url("http://10.255.255.255/"));
}

#[test]
fn test_safe_url_private_172_boundary() {
    // 172.16.0.0/12 拒绝；172.32 是公网，放行
    assert!(!is_safe_external_url("http://172.16.0.1/"));
    assert!(!is_safe_external_url("http://172.31.255.255/"));
    assert!(is_safe_external_url("http://172.32.0.1/"));
}

#[test]
fn test_safe_url_private_192_blocked() {
    assert!(!is_safe_external_url("http://192.168.1.1/"));
    assert!(!is_safe_external_url("http://192.168.255.255/"));
}

#[test]
fn test_safe_url_documentation_blocked() {
    // RFC 5737 文档网段
    assert!(!is_safe_external_url("http://192.0.2.1/"));
    assert!(!is_safe_external_url("http://198.51.100.1/"));
    assert!(!is_safe_external_url("http://203.0.113.1/"));
}

#[test]
fn test_safe_url_multicast_blocked() {
    assert!(!is_safe_external_url("http://224.0.0.1/"));
    assert!(!is_safe_external_url("http://239.255.255.250/"));
}

#[test]
fn test_safe_url_broadcast_blocked() {
    assert!(!is_safe_external_url("http://255.255.255.255/"));
}

#[test]
fn test_safe_url_public_ip_allowed() {
    assert!(is_safe_external_url("http://8.8.8.8/"));
    assert!(is_safe_external_url("http://1.1.1.1/"));
    assert!(is_safe_external_url("http://114.114.114.114/"));
}

#[test]
fn test_safe_url_ipv6_loopback_blocked() {
    assert!(!is_safe_external_url("http://[::1]/"));
    assert!(!is_safe_external_url("http://[::1]:8080/"));
}

#[test]
fn test_safe_url_ipv6_unspecified_blocked() {
    assert!(!is_safe_external_url("http://[::]/"));
}

#[test]
fn test_safe_url_ipv6_link_local_blocked() {
    assert!(!is_safe_external_url("http://[fe80::1]/"));
}

#[test]
fn test_safe_url_ipv6_unique_local_blocked() {
    // fc00::/7（ULA）
    assert!(!is_safe_external_url("http://[fc00::1]/"));
    assert!(!is_safe_external_url("http://[fd12:3456:789a::1]/"));
}

#[test]
fn test_safe_url_ipv6_documentation_blocked() {
    assert!(!is_safe_external_url("http://[2001:db8::1]/"));
}

#[test]
fn test_safe_url_ipv6_multicast_blocked() {
    assert!(!is_safe_external_url("http://[ff02::1]/"));
}

#[test]
fn test_safe_url_ipv4_mapped_private_blocked() {
    assert!(!is_safe_external_url("http://[::ffff:127.0.0.1]/admin"));
    assert!(!is_safe_external_url("http://[::ffff:192.168.1.1]/"));
    assert!(!is_safe_external_url("http://[::ffff:10.0.0.1]/"));
}

#[test]
fn test_safe_url_ipv4_mapped_public_allowed() {
    assert!(is_safe_external_url("http://[::ffff:8.8.8.8]/"));
}

#[test]
fn test_safe_url_ipv6_global_allowed() {
    assert!(is_safe_external_url("http://[2606:4700:4700::1111]/"));
    assert!(is_safe_external_url("http://[2001:4860:4860::8888]:80/"));
}

#[test]
fn test_safe_url_ipv6_with_port() {
    assert!(!is_safe_external_url("http://[fe80::1]:8080/"));
    assert!(!is_safe_external_url("http://[2001:db8::1]:8080/"));
}

#[test]
fn test_safe_url_malformed_ipv6_no_bracket() {
    // "http://::1/" 无方括号 → host 被解析为空 → 拒绝
    assert!(!is_safe_external_url("http://::1/"));
}

#[test]
fn test_safe_url_ipv6_unclosed_bracket() {
    assert!(!is_safe_external_url("http://[::1:8080/")); // 无闭合 ]
}

#[test]
fn test_safe_url_userinfo_cannot_hide_private() {
    assert!(!is_safe_external_url("http://user:pass@127.0.0.1/admin"));
    assert!(!is_safe_external_url("http://user@localhost:8082/"));
    assert!(!is_safe_external_url("http://user@169.254.169.254/"));
    assert!(!is_safe_external_url("http://evil@10.0.0.1/"));
}

#[test]
fn test_safe_url_userinfo_public_ok() {
    assert!(is_safe_external_url("http://user:pass@example.com/v.mp4"));
    assert!(is_safe_external_url("http://user@8.8.8.8/v"));
}

#[test]
fn test_safe_url_double_at_rejected() {
    // "http://a@b@127.0.0.1/" 中 userinfo 出现第二个 '@'（RFC 3986 禁止），
    // 浏览器会按 userinfo="a@b"、host=127.0.0.1 连接 → 必须拒绝。
    assert!(!is_safe_external_url("http://a@b@127.0.0.1/admin"));
}

#[test]
fn test_safe_url_decimal_ip_rejected() {
    // 十进制 IP（2130706433 = 127.0.0.1）无法被 IpAddr 解析，但浏览器
    // 对纯数字 host 按 IPv4 解析 → 必须拒绝（防 SSRF 绕过）。
    assert!(!is_safe_external_url("http://2130706433/admin"));
    assert!(!is_safe_external_url("http://0x7f000001/admin"));
    assert!(!is_safe_external_url("http://127.1/admin"));
}

#[test]
fn test_safe_url_non_http_scheme_rejected() {
    assert!(!is_safe_external_url("ftp://example.com/file"));
    assert!(!is_safe_external_url("file:///etc/passwd"));
    assert!(!is_safe_external_url("javascript://example.com"));
    assert!(!is_safe_external_url("rtmp://example.com/live"));
}

#[test]
fn test_safe_url_uppercase_scheme_rejected() {
    // scheme 匹配区分大小写 → 大写 HTTP 被拒绝（记录当前行为）
    assert!(!is_safe_external_url("HTTP://example.com/"));
}

#[test]
fn test_safe_url_no_scheme_rejected() {
    assert!(!is_safe_external_url("not a url"));
    assert!(!is_safe_external_url("example.com/v.mp4"));
    assert!(!is_safe_external_url("//example.com/v.mp4"));
}

#[test]
fn test_safe_url_empty_rejected() {
    assert!(!is_safe_external_url(""));
}

#[test]
fn test_safe_url_empty_host_rejected() {
    assert!(!is_safe_external_url("https://"));
    assert!(!is_safe_external_url("http:///path"));
    assert!(!is_safe_external_url("https:///"));
}

#[test]
fn test_safe_url_protocol_only_rejected() {
    assert!(!is_safe_external_url("://"));
}

// ══════════════════════════════════════════════════════════════════════
// 六、Transcoder —— mock 二进制驱动的参数构建与流程验证
//
// 原理：Transcoder::new 通过 TranscodeSettings 指定 ffmpeg / ffprobe 二进制
// 路径。我们提供一个共享 mock 目录，其中的 ffmpeg / ffprobe 是 shell 脚本，
// 行为由“输入文件名”决定（进程间无状态，天然无竞态）。
// ══════════════════════════════════════════════════════════════════════

fn mock_bin_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("atmos_mock_bin_pid{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // 输出真 JSON（正常）或按输入文件名切换行为
    std::fs::write(
        dir.join("ffprobe"),
        r#"#!/bin/sh
in=""
for a in "$@"; do in="$a"; done
d=$(dirname "$in")
printf '%s\n' "$@" > "$d/ffprobe_args.log"
base=$(basename "$in")
case "$base" in
  bad_exit*) echo "mock probe failure" >&2; exit 3 ;;
  bad_json*) echo "{{{ this is not json" ;;
  slow*) sleep 5 ;;
  *) cat <<'EOF'
{"format":{"duration":"12.345","size":"1048576","bit_rate":"700000"},"streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"bit_rate":"600000"},{"codec_type":"audio","codec_name":"aac","width":null,"height":null,"bit_rate":"128000"}]}
EOF
  ;;
esac
"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("ffmpeg"),
        r#"#!/bin/sh
in=""
out=""
i=0
for a in "$@"; do
  i=$((i+1))
  if [ "$i" = "2" ]; then in="$a"; fi
  out="$a"
done
printf '%s\n' "$@" > "$out.args"
ibase=$(basename "$in")
case "$ibase" in
  ffmpeg_fail*) echo "mock ffmpeg failure" >&2; exit 1 ;;
  ffmpeg_slow*) sleep 5 ;;
  ffmpeg_nooutput*) exit 0 ;;
esac
printf 'mock-output' > "$out"
"#,
    )
    .unwrap();
    set_executable(&dir.join("ffprobe"));
    set_executable(&dir.join("ffmpeg"));
    dir
}

fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).unwrap();
}

/// 设置 mock 二进制路径并构造 Transcoder。所有 mock 测试共享同一目录。
fn mock_transcoder_with_settings(
    media_root: &Path,
    mut settings: atmos_video_backend::services::transcoder::TranscodeSettings,
) -> Transcoder {
    let bin = mock_bin_dir();
    settings.ffmpeg_path = bin.join("ffmpeg").to_string_lossy().into_owned();
    settings.ffprobe_path = bin.join("ffprobe").to_string_lossy().into_owned();
    Transcoder::new(media_root, settings)
}

fn mock_transcoder(media_root: &Path) -> Transcoder {
    mock_transcoder_with_settings(
        media_root,
        atmos_video_backend::services::transcoder::TranscodeSettings::default(),
    )
}

fn temp_media_root() -> PathBuf {
    let root = unique_dir("tx_media");
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn read_args_log(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| l.to_string())
        .collect()
}

// ── transcode：ffmpeg 参数构建（build_ffmpeg_args）与流程 ──

#[tokio::test]
async fn test_transcode_720p_builds_expected_args() {
    let root = temp_media_root();
    let input = unique_file(&root, "input_720p.mp4", b"x");
    let tx = mock_transcoder(&root);
    let variants = tx
        .transcode(42, &input, vec!["720p".to_string()])
        .await
        .unwrap();
    assert_eq!(variants.len(), 1);
    let v = &variants[0];
    assert_eq!(v.resolution, "720p");
    assert_eq!(v.video_id, 42);
    assert_eq!(v.bitrate, Some(2500));
    assert_eq!(v.codec, "h264");
    assert_eq!(v.file_size, 11, "mock 输出字节数");
    let output = root.join("variants/42_720p.mp4");
    assert_eq!(v.file_path, output.to_string_lossy());

    // 验证 build_ffmpeg_args 生成的完整参数序列
    let args = read_args_log(&output.with_extension("mp4.args"));
    let expected = vec![
        "-i".to_string(),
        input.to_string_lossy().to_string(),
        "-vf".to_string(),
        "scale=1280:720".to_string(),
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "23".to_string(),
        "-b:v".to_string(),
        "2500k".to_string(),
        "-c:a".to_string(),
        "aac".to_string(),
        "-b:a".to_string(),
        "128k".to_string(),
        "-movflags".to_string(),
        "+faststart".to_string(),
        "-y".to_string(),
        output.to_string_lossy().to_string(),
    ];
    assert_eq!(args, expected);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_transcode_all_resolutions_param_map() {
    let root = temp_media_root();
    let input = unique_file(&root, "input_all.mp4", b"x");
    let tx = mock_transcoder(&root);
    let resolutions = ["2160p", "1080p", "720p", "480p", "360p"];
    let variants = tx
        .transcode(
            7,
            &input,
            resolutions.iter().map(|s| s.to_string()).collect(),
        )
        .await
        .unwrap();

    // resolution → (宽x高, 码率 kbps)
    let expected: Vec<(u32, u32, i32)> = vec![
        (3840, 2160, 8000),
        (1920, 1080, 5000),
        (1280, 720, 2500),
        (854, 480, 1000),
        (640, 360, 600),
    ];
    assert_eq!(variants.len(), 5);
    for (v, (w, h, br)) in variants.iter().zip(expected.iter()) {
        assert_eq!(v.bitrate, Some(*br), "分辨率 {}", v.resolution);
        let args = read_args_log(&root.join(format!("variants/7_{}.mp4.args", v.resolution)));
        let res = &v.resolution;
        assert!(args.contains(&format!("scale={w}:{h}")), "{res} scale");
        assert!(args.contains(&format!("{br}k")), "{res} bitrate");
        assert!(args.contains(&"-movflags".to_string()));
        assert!(args.contains(&"+faststart".to_string()));
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_transcode_dedups_resolutions() {
    let root = temp_media_root();
    let input = unique_file(&root, "input_dedup.mp4", b"x");
    let tx = mock_transcoder(&root);
    let variants = tx
        .transcode(
            9,
            &input,
            vec![
                "720p".to_string(),
                "720p".to_string(),
                "1080p".to_string(),
                "720p".to_string(),
            ],
        )
        .await
        .unwrap();
    assert_eq!(variants.len(), 2);
    // 变体目录里只有去重后的 2 个输出文件
    let produced: Vec<_> = std::fs::read_dir(root.join("variants"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".mp4") && !n.ends_with(".args"))
        .collect();
    assert_eq!(produced.len(), 2, "输出文件: {produced:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_transcode_unsupported_resolution() {
    let root = temp_media_root();
    let input = unique_file(&root, "input_bad.mp4", b"x");
    let tx = mock_transcoder(&root);
    let err = tx
        .transcode(1, &input, vec!["800p".to_string()])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Unsupported resolution: 800p"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_transcode_resolution_case_sensitive() {
    let root = temp_media_root();
    let input = unique_file(&root, "input_case.mp4", b"x");
    let tx = mock_transcoder(&root);
    let err = tx
        .transcode(1, &input, vec!["1080P".to_string()])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("Unsupported resolution"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_transcode_empty_resolutions() {
    let root = temp_media_root();
    let input = unique_file(&root, "input_empty.mp4", b"x");
    let tx = mock_transcoder(&root);
    let variants = tx.transcode(1, &input, vec![]).await.unwrap();
    assert!(variants.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_transcode_ffmpeg_failure_propagates() {
    let root = temp_media_root();
    let input = unique_file(&root, "ffmpeg_fail.mp4", b"x");
    let tx = mock_transcoder(&root);
    let err = tx
        .transcode(1, &input, vec!["720p".to_string()])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("FFmpeg failed"), "{err}");
    assert!(err.contains("mock ffmpeg failure"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_transcode_ffmpeg_timeout_kills_child() {
    let root = temp_media_root();
    let input = unique_file(&root, "ffmpeg_slow.mp4", b"x");
    let mut settings = atmos_video_backend::services::transcoder::TranscodeSettings::default();
    settings.transcode_timeout = std::time::Duration::from_secs(1);
    let tx = mock_transcoder_with_settings(&root, settings);
    let err = tx
        .transcode(1, &input, vec!["720p".to_string()])
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("ffmpeg timed out"), "{err}");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_transcode_ffmpeg_no_output_file_errors() {
    let root = temp_media_root();
    let input = unique_file(&root, "ffmpeg_nooutput.mp4", b"x");
    let tx = mock_transcoder(&root);
    let result = tx.transcode(1, &input, vec!["720p".to_string()]).await;
    assert!(result.is_err(), "mock 未产出文件时应报错");
    let _ = std::fs::remove_dir_all(&root);
}

// ── get_video_info：ffprobe 输出解析 ──

#[tokio::test]
async fn test_get_video_info_parses_fields() {
    let dir = unique_dir("probe_ok");
    std::fs::create_dir_all(&dir).unwrap();
    let input = unique_file(&dir, "sample_probe.mp4", b"x");
    let tx = mock_transcoder(std::path::Path::new(&dir));
    let info: VideoInfo = tx.get_video_info(&input).await.unwrap();

    let format = info.format.expect("format 应存在");
    assert_eq!(format.duration.as_deref(), Some("12.345"));
    assert_eq!(format.size.as_deref(), Some("1048576"));
    assert_eq!(format.bit_rate.as_deref(), Some("700000"));

    let streams = info.streams.expect("streams 应存在");
    assert_eq!(streams.len(), 2);
    assert_eq!(streams[0].codec_type, "video");
    assert_eq!(streams[0].codec_name.as_deref(), Some("h264"));
    assert_eq!(streams[0].width, Some(1920));
    assert_eq!(streams[0].height, Some(1080));
    assert_eq!(streams[0].bit_rate.as_deref(), Some("600000"));
    assert_eq!(streams[1].codec_type, "audio");
    assert_eq!(streams[1].codec_name.as_deref(), Some("aac"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_get_video_info_passes_expected_args() {
    let dir = unique_dir("probe_args");
    std::fs::create_dir_all(&dir).unwrap();
    let input = unique_file(&dir, "args_probe.mp4", b"x");
    let tx = mock_transcoder(std::path::Path::new(&dir));
    tx.get_video_info(&input).await.unwrap();
    let log = dir.join("ffprobe_args.log");
    let args = read_args_log(&log);
    assert_eq!(
        args,
        vec![
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ]
        .into_iter()
        .map(|s| s.to_string())
        .chain(std::iter::once(input.to_string_lossy().to_string()))
        .collect::<Vec<String>>()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_get_video_info_probe_failure() {
    let dir = unique_dir("probe_fail");
    std::fs::create_dir_all(&dir).unwrap();
    let input = unique_file(&dir, "bad_exit.mp4", b"x");
    let tx = mock_transcoder(std::path::Path::new(&dir));
    let err = tx.get_video_info(&input).await.unwrap_err().to_string();
    assert!(err.contains("FFprobe failed"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_get_video_info_invalid_json() {
    let dir = unique_dir("probe_json");
    std::fs::create_dir_all(&dir).unwrap();
    let input = unique_file(&dir, "bad_json.mp4", b"x");
    let tx = mock_transcoder(std::path::Path::new(&dir));
    let err = tx.get_video_info(&input).await.unwrap_err().to_string();
    assert!(err.contains("Failed to parse FFprobe output"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn test_get_video_info_timeout() {
    let dir = unique_dir("probe_slow");
    std::fs::create_dir_all(&dir).unwrap();
    let input = unique_file(&dir, "slow.mp4", b"x");
    let mut settings = atmos_video_backend::services::transcoder::TranscodeSettings::default();
    settings.ffprobe_timeout = std::time::Duration::from_secs(1);
    let tx = mock_transcoder_with_settings(std::path::Path::new(&dir), settings);
    let err = tx.get_video_info(&input).await.unwrap_err().to_string();
    assert!(err.contains("ffprobe timed out"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── delete_variant：分辨率白名单与文件删除 ──

#[tokio::test]
async fn test_delete_variant_unknown_resolution_noop() {
    let root = temp_media_root();
    let tx = mock_transcoder(&root);
    for res in ["800p", "1080P", "720p60", "unknown"] {
        let result = tx.delete_variant(1, res).await;
        assert!(result.is_ok(), "未知分辨率 {res} 应视为 no-op");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_delete_variant_traversal_resolution_noop() {
    // 分辨率被嵌入文件路径；越权/穿越类分辨率必须被白名单挡下
    let root = temp_media_root();
    let tx = mock_transcoder(&root);
    for res in [
        "../evil",
        "..",
        "/etc/passwd",
        "../../etc/passwd",
        "720p.mp4",
    ] {
        let result = tx.delete_variant(1, res).await;
        assert!(result.is_ok(), "非法分辨率 {res} 应 no-op");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_delete_variant_deletes_existing_file() {
    let root = temp_media_root();
    let tx = mock_transcoder(&root);
    let target = root.join("variants/7_480p.mp4");
    std::fs::write(&target, b"data").unwrap();
    tx.delete_variant(7, "480p").await.unwrap();
    assert!(!target.exists(), "文件应被删除");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn test_delete_variant_missing_file_ok() {
    let root = temp_media_root();
    let tx = mock_transcoder(&root);
    // NotFound 不视为错误
    assert!(tx.delete_variant(99, "1080p").await.is_ok());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn test_transcoder_new_creates_variants_dir() {
    let root = temp_media_root();
    let _ = mock_transcoder(&root);
    assert!(root.join("variants").is_dir());
    let _ = std::fs::remove_dir_all(&root);
}

// ══════════════════════════════════════════════════════════════════════
// 七、#[ignore] —— 需要真实 PostgreSQL / FFmpeg 的用例
//
// 运行方式：
//   DATABASE_URL=postgres://kuaile@localhost:5432/atmos_video \
//     cargo test --test service_media_tests -- --ignored
// ══════════════════════════════════════════════════════════════════════

/// 完整上传流程：需要 PostgreSQL（DATABASE_URL）与本地文件系统。
/// 使用最小的 ftyp 头伪造 mp4 通过 magic 校验；结束后级联删除数据行与媒体文件。
#[tokio::test]
async fn upload_video_flow_with_database() {
    let Some(_) = std::env::var("DATABASE_URL").ok() else {
        eprintln!("DATABASE_URL not set, skipping");
        return;
    };
    use atmos_video_backend::config::AppConfig;
    use atmos_video_backend::repositories::video_repo::VideoRepository;
    use atmos_video_backend::services::media_service::MediaService;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://kuaile@localhost:5432/atmos_video".to_string());
    let pool = sqlx::PgPool::connect(&db_url)
        .await
        .expect("连接数据库失败");
    let repo = VideoRepository::new(pool);
    let root = temp_media_root();
    let config = AppConfig {
        database_url: db_url,
        server_port: 8082,
        public_url: "http://localhost:8082".to_string(),
        media_root: root.clone(),
        webapp_root: root.join("webapp"),
        log_dir: root.join("logs"),
        data_dir: root.join("data"),
        registration_enabled: Arc::new(AtomicBool::new(false)),
        cors_origin: String::new(),
        cookie_secure: false,
        smtp_host: String::new(),
        smtp_port: 0,
        smtp_username: String::new(),
        smtp_password: String::new(),
        smtp_from: String::new(),
        redis_url: String::new(),
        admin_ip_whitelist: Vec::new(),
        upload_quota_bytes: 0,
        db_max_connections: 100,
        db_min_connections: 2,
        migrations_dir: None,
        sentry_dsn: String::new(),
        sentry_environment: "production".into(),
        app_env: "test".into(),
        allow_first_user_admin: false,
        trusted_proxy: false,
        hashid_salt: String::new(),
        transcode_timeout_secs: 3600,
        ffprobe_timeout_secs: 30,
        transcode_concurrency: 1,
        transcode_max_duration_secs: 7200,
        ffmpeg_path: "ffmpeg".into(),
        ffprobe_path: "ffprobe".into(),
    };
    let svc = MediaService::new(repo.clone(), config);

    // upload_video_file 写入 videos.uploader_id（FK→users），因此先创建
    // 一个真实用户，避免依赖库中既有用户 id。
    let username = format!("upload_flow_{}", std::process::id());
    let pool = repo.pool();
    let (uploader_id,): (i64,) = sqlx::query_as(
        "INSERT INTO users (username, password_hash, approved, role, tenant_id) \
         VALUES ($1, 'x', true, 1, 1) RETURNING id",
    )
    .bind(&username)
    .fetch_one(pool)
    .await
    .expect("创建上传用户失败");

    let tmp = unique_dir("upload_tmp");
    let tmp_file = unique_file(&tmp, "集成测试.mp4", MP4_ISOM_BYTES);
    let id = svc
        .upload_video_file(1, "集成测试.mp4", &tmp_file, "test", uploader_id, None)
        .await
        .expect("上传失败");
    assert!(id > 0);

    // 清理：删除 DB 行 + 落盘文件
    let deleted = repo.delete_video_cascade(1, id).await.expect("清理失败");
    assert!(deleted);
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(uploader_id)
        .execute(pool)
        .await
        .expect("清理用户失败");
    for entry in std::fs::read_dir(&root).unwrap().flatten() {
        let _ = std::fs::remove_file(entry.path());
    }
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// 真实 ffprobe 解析一个程序生成的 WAV 文件：需要 ffprobe 在 PATH 上。
/// 不依赖 ffmpeg，可离线验证 ffprobe 集成。
#[tokio::test]
#[ignore = "需要真实 ffprobe 可执行文件"]
async fn get_video_info_real_ffprobe_wav() {
    let dir = unique_dir("real_probe");
    std::fs::create_dir_all(&dir).unwrap();
    // 0.5 秒 8kHz 单声道 16-bit 静音 WAV
    let sample_rate = 8000u32;
    let samples = sample_rate / 2;
    let data_len = samples * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // fmt 块大小
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // 单声道
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(&vec![0u8; data_len as usize]);
    let path = unique_file(&dir, "silence.wav", &wav);

    let tx = Transcoder::new(std::path::Path::new(&dir), Default::default());
    let info = tx.get_video_info(&path).await.expect("ffprobe 解析失败");
    let duration = info
        .format
        .and_then(|f: FormatInfo| f.duration)
        .expect("应能读出 duration");
    let dur: f64 = duration.parse().expect("duration 应可解析为数字");
    assert!((dur - 0.5).abs() < 0.05, "期望约 0.5s，实际 {dur}s");
    let _ = std::fs::remove_dir_all(&dir);
}
