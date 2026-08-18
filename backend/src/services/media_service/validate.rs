//! 媒体文件校验与路径安全的纯工具函数。
//!
//! 从 `media_service.rs` 拆分而来：本模块只包含不依赖 `MediaService`
//! 的纯函数（magic-bytes 类型校验、路径穿越防护、外部 URL 白名单、
//! 文件名清洗、临时文件清扫），便于独立测试与复用。

use std::path::{Path, PathBuf};
use std::time::Duration;

/// 上传临时文件（`.upload_*`）最长存活时间：超过该时长未写入的视为放弃的
/// 上传，由后台任务清扫（续传每次写入都会刷新 mtime，进行中的上传不会被误删）。
pub const UPLOAD_TEMP_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// 分类名称最大长度（按 Unicode 字符计）。
pub const MAX_CATEGORY_CHARS: usize = 32;

/// Hard timeout for a single ffprobe invocation.
const FFPROBE_TIMEOUT_SECS: u64 = 15;

/// client's Content-Type. Returns (extension, mime) on success.
pub fn infer_image(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    // JPEG: FF D8 FF
    if bytes.len() >= 3 && bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(("jpg", "image/jpeg"));
    }
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if bytes.len() >= 8 && bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(("png", "image/png"));
    }
    // GIF: 47 49 46 38 (37|39) 61
    if bytes.len() >= 6 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Some(("gif", "image/gif"));
    }
    // WebP: "RIFF" .... "WEBP"
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(("webp", "image/webp"));
    }
    // BMP: "BM"
    if bytes.len() >= 2 && bytes.starts_with(b"BM") {
        return Some(("bmp", "image/bmp"));
    }
    None
}

pub fn safe_media_path(url: &str, media_root: &Path) -> Option<PathBuf> {
    let relative = url.strip_prefix("/media/")?;
    let path = media_root.join(relative);
    // Canonicalize to resolve any ".." components, then verify prefix
    let canonical = path.canonicalize().ok()?;
    let canonical_root = media_root.canonicalize().ok()?;
    if canonical.starts_with(&canonical_root) {
        Some(canonical)
    } else {
        tracing::warn!("Path traversal blocked: {:?}", path);
        None
    }
}

pub(super) async fn extract_duration(path: &std::path::Path) -> Result<Option<i64>, String> {
    let path_str = path.to_string_lossy().to_string();
    let output = tokio::time::timeout(
        Duration::from_secs(FFPROBE_TIMEOUT_SECS),
        tokio::process::Command::new("ffprobe")
            .kill_on_drop(true)
            .arg("-v")
            .arg("error")
            .arg("-show_entries")
            .arg("format=duration")
            .arg("-of")
            .arg("default=noprint_wrappers=1:nokey=1")
            .arg(&path_str)
            .output(),
    )
    .await
    .map_err(|_| format!("ffprobe timed out after {}s", FFPROBE_TIMEOUT_SECS))?
    .map_err(|e| format!("ffprobe not found: {}", e))?;

    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let dur_secs = stdout
        .trim()
        .parse::<f64>()
        .map_err(|e| format!("parse duration: {}", e))?;
    Ok(Some((dur_secs * 1000.0) as i64))
}

/// 用 magic bytes 验证文件类型，防止伪装扩展名的恶意上传
pub fn validate_file_type(path: &std::path::Path, ext: &str) -> Result<(), String> {
    // m3u8 (HLS playlist) is plain text with no fixed magic bytes — validate
    // the full content structure. SECURITY (H-05): the previous
    // implementation only checked the `#EXTM3U` prefix, so a malicious
    // payload could be smuggled after a one-line header. We now enforce:
    //   - starts with #EXTM3U
    //   - no embedded HTML/script tags (defence in depth against XSS-via-HLS)
    //   - file size is bounded
    if ext == "m3u8" {
        const MAX_M3U8_BYTES: u64 = 1_048_576; // 1 MiB
        let metadata =
            std::fs::metadata(path).map_err(|e| format!("无法读取 m3u8 元数据: {}", e))?;
        if metadata.len() > MAX_M3U8_BYTES {
            return Err(format!("m3u8 文件过大: 限制 {} 字节", MAX_M3U8_BYTES));
        }
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("无法读取 m3u8 文件: {}", e))?;
        let trimmed = content.trim_start();
        if !trimmed.starts_with("#EXTM3U") {
            return Err("m3u8 文件格式无效: 缺少 #EXTM3U 头部".to_string());
        }
        // Reject HTML-ish or script-ish content. We don't try to parse
        // the playlist — just block obvious injection attempts.
        let lower = content.to_ascii_lowercase();
        for needle in [
            "<script",
            "</script",
            "<iframe",
            "<object",
            "<embed",
            "<?xml",
            "<!doctype",
            "<html",
            "javascript:",
        ] {
            if lower.contains(needle) {
                return Err(format!("m3u8 文件包含禁止内容: {}", needle));
            }
        }
        return Ok(());
    }

    let kind = infer::get_from_path(path)
        .map_err(|e| format!("无法读取文件类型: {}", e))?
        .ok_or_else(|| format!("无法识别的文件类型: {}", ext))?;

    let mime_type = kind.mime_type();
    let is_valid = match ext {
        "mp4" => mime_type.starts_with("video/mp4"),
        "m4v" => {
            // infer 0.19 reports the Apple "M4V " brand as `video/x-m4v`,
            // not `video/mp4` — accept both for genuine .m4v files.
            mime_type == "video/x-m4v" || mime_type.starts_with("video/mp4")
        }
        "mov" => mime_type == "video/quicktime",
        "avi" => mime_type == "video/x-msvideo",
        "mkv" => mime_type == "video/x-matroska",
        "webm" => mime_type == "video/webm",
        "flv" => mime_type == "video/x-flv",
        "wmv" => mime_type == "video/x-ms-wmv",
        "jpg" | "jpeg" => mime_type.starts_with("image/jpeg"),
        "png" => mime_type == "image/png",
        "webp" => mime_type.starts_with("image/webp"),
        "gif" => mime_type == "image/gif",
        "bmp" => mime_type == "image/bmp",
        _ => false,
    };

    if !is_valid {
        return Err(format!(
            "文件类型不匹配: 扩展名 .{} 但实际 MIME 类型为 {}",
            ext, mime_type
        ));
    }
    Ok(())
}

/// 校验上传/编辑的 category 值（SECURITY L-03）：直接进入 DB 列与日志，
/// 需限制长度并拒绝控制字符/路径分隔符，防止日志伪造与超长值触发 DB 报错。
/// 长度按 Unicode 字符计（与 Postgres VARCHAR 语义一致，中文分类不受限）。
pub fn validate_category(category: &str) -> Result<(), String> {
    if category.chars().count() > MAX_CATEGORY_CHARS {
        return Err(format!(
            "分类名称长度不能超过 {} 个字符",
            MAX_CATEGORY_CHARS
        ));
    }
    if category
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err("分类名称包含非法字符".to_string());
    }
    Ok(())
}

/// spawn_blocking 侧的临时文件清扫实现（独立成纯函数便于测试）。
/// 对 mtime 无法读取（部分文件系统）或时间异常（时钟回拨）的文件保守跳过。
pub(super) fn sweep_upload_temps_blocking(root: &Path, ttl: Duration) -> Result<usize, String> {
    let now = std::time::SystemTime::now();
    let mut removed = 0usize;
    let entries = std::fs::read_dir(root)
        .map_err(|e| format!("扫描媒体目录失败 ({}): {}", root.display(), e))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(".upload_") || name.starts_with("cover_tmp_")) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let stale = meta
            .modified()
            .map(|m| now.duration_since(m).map(|age| age >= ttl).unwrap_or(false))
            .unwrap_or(false);
        if !stale {
            continue;
        }
        match std::fs::remove_file(entry.path()) {
            Ok(()) => {
                removed += 1;
                tracing::info!(file = %name, "removed stale upload temp file");
            }
            Err(e) => tracing::warn!(
                file = %name,
                "failed to remove stale upload temp file: {}",
                e
            ),
        }
    }
    Ok(removed)
}

/// Strip path separators and control characters from a user-supplied filename.
/// SECURITY (A08-04): the result is suitable for use as a DB column value,
/// for log emission, and as a single path component.
pub fn sanitize_filename(name: &str) -> String {
    // Keep only the trailing path component (drops any "../" the client
    // smuggles in via the multipart `filename` field).
    let base = std::path::Path::new(name)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "video.mp4".to_string());
    let mut out = String::with_capacity(base.len());
    for c in base.chars() {
        if c.is_control() || c == '/' || c == '\\' || c == '\0' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    // Cap length to 200 bytes so an attacker can't blow up DB rows / log lines.
    // String::truncate panics if the index isn't on a char boundary, and a
    // multi-byte filename (e.g. Chinese) can easily cross the 200th byte —
    // walk back to the nearest boundary instead.
    if out.len() > 200 {
        let mut end = 200;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
    }
    if out.is_empty() {
        out.push_str("video.mp4");
    }
    out
}

/// Validate an external stream URL: must be http(s), must NOT resolve to a
/// loopback, link-local, private, or otherwise non-routable address. Defence
/// in depth even though we don't fetch the URL server-side (browsers do).
pub fn is_safe_external_url(url: &str) -> bool {
    let scheme_end = match url.find("://") {
        Some(i) => i + 3,
        None => return false,
    };
    let scheme = &url[..scheme_end - 3];
    if !matches!(scheme, "http" | "https") {
        return false;
    }
    let after = &url[scheme_end..];
    // Reject credentials: a browser would authenticate against the host
    // after '@' and ignore everything before it, but a naive host
    // extraction wouldn't — so drop the userinfo part entirely. A second
    // literal '@' is itself forbidden in userinfo (RFC 3986) and turns the
    // URL into a parsing-ambiguity attack (`http://a@b@127.0.0.1/`), so it
    // is rejected outright instead of half-parsed.
    if after.bytes().filter(|&b| b == b'@').count() > 1 {
        return false;
    }
    let after = match after.find('@') {
        Some(at) => &after[at + 1..],
        None => after,
    };
    // host[:port][/path...]
    let host_end = if after.starts_with('[') {
        // IPv6 literal — the first ':' belongs to the address itself, so
        // scan up to the closing bracket instead.
        match after.find(']') {
            Some(close) => close + 1,
            None => return false, // malformed IPv6 literal
        }
    } else {
        after.find([':', '/']).unwrap_or(after.len())
    };
    let mut host = &after[..host_end];
    if host.is_empty() {
        return false;
    }
    // Strip a trailing dot (DNS root: "example.com." == "example.com") and
    // IPv6 brackets before matching/parsing.
    if let Some(inner) = host.strip_prefix('[') {
        host = inner.strip_suffix(']').unwrap_or(inner);
    }
    host = host.trim_end_matches('.');
    let host_lower = host.to_ascii_lowercase();
    let blocked = [
        "localhost",
        "127.0.0.1",
        "::1",
        "0.0.0.0",
        "169.254.169.254",
        "metadata.google.internal",
        "metadata.goog",
        "100.100.100.200",
    ];
    if blocked.iter().any(|b| host_lower == *b) {
        return false;
    }
    // RFC 6761: names ending in ".localhost" resolve to loopback.
    if host_lower.ends_with(".localhost") {
        return false;
    }
    // Reject IPv4 literals written in non-canonical numeric forms — a single
    // decimal integer ("2130706433"), a hex integer ("0x7f000001"), or
    // dotted variants ("127.1", "0177.0.0.1"). `std::net::IpAddr` refuses to
    // parse them but browsers resolve them to a real address, so a strict
    // parser alone would let e.g. 127.0.0.1 through in disguise.
    // NOTE: the check runs only AFTER `IpAddr` parsing so canonical forms
    // (e.g. "8.8.8.8") fall through to the normal disallow-list.
    if let Ok(ip) = host_lower.parse::<std::net::IpAddr>() {
        return !is_disallowed_ip(&ip);
    }
    if is_noncanonical_ip_literal(&host_lower) {
        return false;
    }
    true
}

/// True if `part` is a numeric literal: decimal digits, or `0x`/`0X`
/// hexadecimal digits. Empty strings are not literals.
fn is_numeric_component(part: &str) -> bool {
    if part.is_empty() {
        return false;
    }
    if let Some(hex) = part.strip_prefix("0x").or_else(|| part.strip_prefix("0X")) {
        return !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit());
    }
    part.bytes().all(|b| b.is_ascii_digit())
}

/// True if `host` is an IPv4 address written in a non-canonical form that
/// `std::net::IpAddr` cannot parse but browsers still resolve: a single
/// decimal/hex integer, or a dotted string of such literals (each component
/// may be decimal, hex or leading-zero octal — all digits, so caught here).
fn is_noncanonical_ip_literal(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    !parts.is_empty() && parts.iter().all(|p| is_numeric_component(p))
}

fn is_disallowed_ip(ip: &std::net::IpAddr) -> bool {
    use std::net::IpAddr::*;
    match ip {
        V4(v4) => {
            v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_private()
                || v4.is_multicast()
        }
        V6(v6) => {
            // v4-mapped addresses (e.g. ::ffff:127.0.0.1) are equivalent to
            // their IPv4 counterpart — re-check them as IPv4.
            if let Some(v4) = v6.to_ipv4() {
                return is_disallowed_ip(&std::net::IpAddr::V4(v4));
            }
            let seg = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || v6.is_unicast_link_local() // fe80::/10
                || (seg[0] & 0xfe00) == 0xfc00 // fc00::/7 unique-local
                || (seg[0] == 0x2001 && seg[1] == 0x0db8) // 2001:db8::/32 doc range
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_basic() {
        assert_eq!(sanitize_filename("video.mp4"), "video.mp4");
    }

    #[test]
    fn test_sanitize_filename_strips_path_separators() {
        // `Path::file_name` extracts just the last component
        let result = sanitize_filename("../../etc/passwd");
        assert!(!result.contains('/'));
        assert!(!result.contains(".."));
        assert_eq!(result, "passwd");
    }

    #[test]
    fn test_sanitize_filename_strips_control_chars() {
        let result = sanitize_filename("evil\nfile.mp4");
        assert!(!result.contains('\n'));
        assert!(result.contains("evil") && result.contains("file.mp4"));
    }

    #[test]
    fn test_sanitize_filename_strips_nul() {
        let result = sanitize_filename("evil\0file.mp4");
        assert!(!result.contains('\0'));
    }

    #[test]
    fn test_sanitize_filename_length_capped() {
        let long = "a".repeat(500) + ".mp4";
        let result = sanitize_filename(&long);
        assert!(result.len() <= 200 + 4); // 200 chars + ".mp4"
    }

    #[test]
    fn test_sanitize_filename_empty_fallback() {
        assert_eq!(sanitize_filename(""), "video.mp4");
    }

    #[test]
    fn test_sanitize_filename_only_control_chars() {
        let result = sanitize_filename("\n\r\t");
        assert!(result.len() == 3 || result.contains("video"));
    }

    #[test]
    fn test_infer_image_jpeg() {
        let bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46];
        let (ext, mime) = infer_image(&bytes).unwrap();
        assert_eq!(ext, "jpg");
        assert_eq!(mime, "image/jpeg");
    }

    #[test]
    fn test_infer_image_png() {
        let bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let (ext, mime) = infer_image(&bytes).unwrap();
        assert_eq!(ext, "png");
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn test_infer_image_gif() {
        let bytes = b"GIF89a...";
        let (ext, mime) = infer_image(bytes).unwrap();
        assert_eq!(ext, "gif");
        assert_eq!(mime, "image/gif");
    }

    #[test]
    fn test_infer_image_webp() {
        let bytes = b"RIFF\x00\x00\x00\x00WEBP";
        let (ext, mime) = infer_image(bytes).unwrap();
        assert_eq!(ext, "webp");
        assert_eq!(mime, "image/webp");
    }

    #[test]
    fn test_infer_image_bmp() {
        let bytes = b"BM\x00\x00\x00\x00\x00\x00";
        let (ext, mime) = infer_image(bytes).unwrap();
        assert_eq!(ext, "bmp");
        assert_eq!(mime, "image/bmp");
    }

    #[test]
    fn test_infer_image_empty() {
        assert!(infer_image(&[]).is_none());
    }

    #[test]
    fn test_infer_image_too_short() {
        assert!(infer_image(b"GIF").is_none());
    }

    #[test]
    fn test_infer_image_unknown() {
        assert!(infer_image(b"this is definitely not an image").is_none());
    }

    #[test]
    fn test_is_safe_external_url_https() {
        assert!(is_safe_external_url("https://example.com/video.mp4"));
    }

    #[test]
    fn test_is_safe_external_url_blocked_localhost() {
        assert!(!is_safe_external_url("http://localhost:8082/admin"));
    }

    #[test]
    fn test_is_safe_external_url_blocked_loopback() {
        assert!(!is_safe_external_url("http://127.0.0.1/admin"));
    }

    #[test]
    fn test_is_safe_external_url_blocked_private_ip() {
        assert!(!is_safe_external_url("http://192.168.1.1/admin"));
        assert!(!is_safe_external_url("http://10.0.0.1/admin"));
        assert!(!is_safe_external_url("http://172.16.0.1/admin"));
    }

    #[test]
    fn test_is_safe_external_url_blocked_cloud_metadata() {
        assert!(!is_safe_external_url(
            "http://169.254.169.254/latest/meta-data/"
        ));
    }

    #[test]
    fn test_is_safe_external_url_no_scheme() {
        assert!(!is_safe_external_url("ftp://example.com/file"));
    }

    #[test]
    fn test_is_safe_external_url_invalid_url() {
        assert!(!is_safe_external_url("not a url"));
    }

    #[test]
    fn test_validate_file_type_png() {
        // Minimal valid PNG: 8-byte signature + IHDR chunk
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // chunk length
            0x49, 0x48, 0x44, 0x52, // "IHDR"
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1 pixel
            0x08, 0x02, 0x00, 0x00, 0x00, // bit depth + color type + compression etc
            0x90, 0x77, 0x53, 0xDE, // CRC
        ];
        let dir = std::env::temp_dir();
        let path = dir.join("test_valid_png.png");
        std::fs::write(&path, &png_bytes).unwrap();
        assert!(validate_file_type(&path, "png").is_ok());
        assert!(validate_file_type(&path, "jpg").is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_validate_file_type_jpeg() {
        // Minimal valid JPEG: SOI + EOI markers
        let jpeg_bytes: Vec<u8> = vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00,
            0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
        ];
        let dir = std::env::temp_dir();
        let path = dir.join("test_valid_jpeg.jpg");
        std::fs::write(&path, &jpeg_bytes).unwrap();
        assert!(validate_file_type(&path, "jpg").is_ok());
        assert!(validate_file_type(&path, "png").is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_validate_file_type_rejects_text_as_mp4() {
        // Plain text file pretending to be mp4
        let dir = std::env::temp_dir();
        let path = dir.join("fake.mp4");
        std::fs::write(&path, b"this is not a video file").unwrap();
        assert!(validate_file_type(&path, "mp4").is_err());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_validate_file_type_unknown_extension() {
        let dir = std::env::temp_dir();
        let path = dir.join("file.xyz");
        std::fs::write(&path, b"some content").unwrap();
        assert!(validate_file_type(&path, "xyz").is_err());
        std::fs::remove_file(&path).unwrap();
    }

    // ──────────────────── 补充测试（与原工作区版本对齐） ────────────────────

    #[test]
    fn test_infer_image_gif_short_header() {
        // 长度不足 6 字节的 GIF 头不得被识别（防止前缀伪造）
        assert_eq!(infer_image(b"GIF8"), None);
        assert_eq!(infer_image(b"GIF87"), None);
        assert_eq!(infer_image(b"GIF89"), None);
    }

    #[test]
    fn test_infer_image_bmp_short_header() {
        assert_eq!(infer_image(b""), None);
        assert_eq!(infer_image(b"B"), None);
        // "BM" 恰好 2 字节 → 合法 BMP
        assert_eq!(infer_image(b"BM"), Some(("bmp", "image/bmp")));
    }

    #[test]
    fn test_sanitize_filename_multibyte_truncation() {
        // 多字节字符（中文）跨越 200 字节边界时，截断必须回退到字符边界，
        // 否则 String::truncate 会 panic
        let name = "中".repeat(100); // 300 字节
        let out = sanitize_filename(&name);
        assert!(
            out.len() <= 200,
            "截断后必须不超过 200 字节，实际 {}",
            out.len()
        );
        assert!(out.is_char_boundary(out.len()), "截断点必须是字符边界");
        assert!(out.chars().all(|c| !c.is_control()));
    }

    #[test]
    fn test_is_safe_external_url_ipv6_loopback_bracketed() {
        assert!(!is_safe_external_url("http://[::1]/"));
        assert!(!is_safe_external_url("https://[::1]:8080/video.mp4"));
    }

    #[test]
    fn test_is_safe_external_url_ipv4_mapped_v6() {
        // ::ffff:127.0.0.1 等价于 IPv4 回环，必须被拒绝
        assert!(!is_safe_external_url("http://[::ffff:127.0.0.1]/"));
        // ::ffff:8.8.8.8 是公网地址，允许
        assert!(is_safe_external_url("http://[::ffff:8.8.8.8]/"));
    }

    #[test]
    fn test_is_safe_external_url_v6_link_local() {
        assert!(!is_safe_external_url("http://[fe80::1]/"));
        assert!(!is_safe_external_url("http://[fe80::1:0]/"));
    }

    #[test]
    fn test_is_safe_external_url_userinfo_does_not_bypass() {
        // userinfo 不能绕过回环检查：浏览器会以 @ 后的主机为准
        assert!(!is_safe_external_url("http://user@127.0.0.1/"));
        assert!(!is_safe_external_url("http://user:pass@localhost/x"));
        assert!(is_safe_external_url("http://user@8.8.8.8/"));
    }

    #[test]
    fn test_is_safe_external_url_rejects_double_userinfo() {
        // 两个 @ 属于解析歧义攻击，直接拒绝
        assert!(!is_safe_external_url("http://a@b@127.0.0.1/"));
        assert!(!is_safe_external_url("http://a@b@example.com/"));
    }

    #[test]
    fn test_is_safe_external_url_rejects_noncanonical_ip_literals() {
        // 浏览器可解析但 std::net::IpAddr 拒绝的非规范 IPv4 写法
        assert!(!is_safe_external_url("http://2130706433/"), "十进制整数");
        assert!(!is_safe_external_url("http://0x7f000001/"), "十六进制整数");
        assert!(!is_safe_external_url("http://127.1/"), "省略段");
        assert!(!is_safe_external_url("http://0177.0.0.1/"), "八进制");
        assert!(!is_safe_external_url("http://127.0.0.1.0/"));
        assert!(
            is_safe_external_url("http://8.8.8.8/"),
            "规范公网 IP 应放行"
        );
    }

    #[test]
    fn test_is_safe_external_url_localhost_suffix() {
        // RFC 6761：*.localhost 一律解析到回环
        assert!(!is_safe_external_url("http://evil.localhost/"));
        assert!(!is_safe_external_url("http://foo.bar.localhost:8080/x"));
    }

    #[test]
    fn test_is_safe_external_url_trailing_dot() {
        // 尾点与 canonical 形式等价："127.0.0.1." 必须被拒绝
        assert!(!is_safe_external_url("http://127.0.0.1./"));
        assert!(!is_safe_external_url("http://localhost./"));
    }

    #[test]
    fn test_safe_media_path_double_slash_cannot_escape() {
        // "/media//x" 中 relative 以 '/' 开头，join 得到绝对路径 ——
        // canonicalize 后前缀检查必须拒绝
        let dir = std::env::temp_dir().join(format!("atmos_smp_7b8faa36_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.mp4");
        std::fs::write(&file, b"x").unwrap();

        assert_eq!(
            safe_media_path("/media/a.mp4", &dir),
            file.canonicalize().ok(),
            "规范路径应解析成功"
        );
        assert_eq!(
            safe_media_path("/media//a.mp4", &dir),
            None,
            "双斜杠必须被拒绝"
        );
        assert_eq!(
            safe_media_path("/media/../etc/passwd", &dir),
            None,
            "路径穿越必须被拒绝"
        );
        assert_eq!(safe_media_path("/media/nonexistent.mp4", &dir), None);
        assert_eq!(safe_media_path("http://evil/x", &dir), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_validate_file_type_m4v() {
        // 标准 M4V ftyp box（major brand "M4V "）→ infer 报告 video/x-m4v
        let m4v_bytes = [
            0x00, 0x00, 0x00, 0x18, b'f', b't', b'y', b'p', b'M', b'4', b'V', b' ', 0x00, 0x00,
            0x00, 0x00, b'M', b'4', b'V', b' ', 0x00, 0x00, 0x00, 0x00,
        ];
        let dir = std::env::temp_dir();
        let path = dir.join("sample.m4v");
        std::fs::write(&path, &m4v_bytes).unwrap();
        assert!(
            validate_file_type(&path, "m4v").is_ok(),
            "M4V brand 必须通过 .m4v 校验"
        );
        assert!(
            validate_file_type(&path, "mp4").is_err(),
            "M4V brand 不能冒充 .mp4"
        );
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn test_validate_category_basic() {
        assert!(validate_category("电影").is_ok());
        assert!(validate_category("a").is_ok());
        assert!(validate_category(&"a".repeat(32)).is_ok());
    }

    #[test]
    fn test_validate_category_length_limit() {
        assert!(validate_category(&"a".repeat(33)).is_err());
        assert!(validate_category(&"中".repeat(33)).is_err());
    }

    #[test]
    fn test_validate_category_rejects_illegal_chars() {
        assert!(validate_category("a/b").is_err());
        assert!(validate_category("a\\b").is_err());
        assert!(validate_category("a\nb").is_err());
        assert!(validate_category("a\tb").is_err());
    }

    #[test]
    fn test_sweep_upload_temps_removes_only_stale() {
        let dir = std::env::temp_dir().join(format!("atmos_sweep_4bfb9007_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let old = std::time::SystemTime::now()
            .checked_sub(Duration::from_secs(2 * 24 * 3600))
            .unwrap();
        let new = std::time::SystemTime::now();

        // 过期临时文件（应删除）
        for name in [".upload_stale", "cover_tmp_stale"] {
            let p = dir.join(name);
            std::fs::write(&p, b"x").unwrap();
            let ft = std::fs::FileTimes::new().set_modified(old);
            std::fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_times(ft)
                .unwrap();
        }
        // 新近临时文件（保留）
        let fresh = dir.join(".upload_fresh");
        std::fs::write(&fresh, b"x").unwrap();
        let ft = std::fs::FileTimes::new().set_modified(new);
        std::fs::File::options()
            .write(true)
            .open(&fresh)
            .unwrap()
            .set_times(ft)
            .unwrap();
        // 普通文件（不受影响）
        std::fs::write(dir.join("real.mp4"), b"x").unwrap();

        let removed = sweep_upload_temps_blocking(&dir, UPLOAD_TEMP_TTL).unwrap();
        assert_eq!(removed, 2, "只应删除两个过期临时文件");
        assert!(!dir.join(".upload_stale").exists());
        assert!(!dir.join("cover_tmp_stale").exists());
        assert!(dir.join(".upload_fresh").exists(), "新近临时文件必须保留");
        assert!(dir.join("real.mp4").exists(), "普通文件必须保留");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_sweep_upload_temps_idempotent() {
        let dir =
            std::env::temp_dir().join(format!("atmos_sweep2_bc440ddd_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".upload_x"), b"x").unwrap();
        assert_eq!(
            sweep_upload_temps_blocking(&dir, UPLOAD_TEMP_TTL).unwrap(),
            0
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_sweep_upload_temps_missing_root_is_err() {
        let dir =
            std::env::temp_dir().join(format!("atmos_nonexistent_c1c3af7b_{}", std::process::id()));
        assert!(sweep_upload_temps_blocking(&dir, UPLOAD_TEMP_TTL).is_err());
    }
}
