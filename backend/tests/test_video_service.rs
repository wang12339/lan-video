#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// Integration tests for video_service functions
// Run with: cargo test --test test_video_service -- --nocapture

use atmos_video_backend::services::media_service::{
    is_safe_external_url, safe_media_path, sanitize_filename, validate_file_type,
};
use std::fs;
use std::path::PathBuf;

// ── safe_media_path tests ──

#[test]
fn test_safe_media_path_valid() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_valid");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("video.mp4"), b"fake video data").unwrap();

    // Canonicalize dir to handle macOS /tmp -> /private/tmp symlink
    let canonical_dir = dir.canonicalize().unwrap();
    let result = safe_media_path("/media/video.mp4", &canonical_dir);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), canonical_dir.join("video.mp4"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_safe_media_path_traversal_dotdot() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_traversal");
    let secret = dir.join("secret");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&secret).unwrap();
    fs::write(secret.join("passwords.txt"), b"secret data").unwrap();

    // Canonicalize dir to handle macOS /tmp -> /private/tmp symlink
    let canonical_dir = dir.canonicalize().unwrap();
    // Attempt path traversal with ..
    let result = safe_media_path("/media/../secret/passwords.txt", &canonical_dir);
    assert!(result.is_none(), "path traversal with .. should be blocked");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_safe_media_path_traversal_encoded() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_encoded");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let canonical_dir = dir.canonicalize().unwrap();

    // URL-encoded traversal attempt (%2e%2e = ..)
    let result = safe_media_path("/media/%2e%2e/etc/passwd", &canonical_dir);
    // This should either fail prefix strip or fail canonicalize
    assert!(result.is_none(), "encoded traversal should be blocked");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_safe_media_path_no_media_prefix() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_noprefix");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let canonical_dir = dir.canonicalize().unwrap();

    let result = safe_media_path("/other/video.mp4", &canonical_dir);
    assert!(
        result.is_none(),
        "missing /media/ prefix should return None"
    );

    let result = safe_media_path("video.mp4", &canonical_dir);
    assert!(result.is_none(), "no leading slash should return None");

    let result = safe_media_path("", &canonical_dir);
    assert!(result.is_none(), "empty string should return None");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_safe_media_path_nonexistent_file() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_nonexist");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let canonical_dir = dir.canonicalize().unwrap();

    let result = safe_media_path("/media/does_not_exist.mp4", &canonical_dir);
    assert!(
        result.is_none(),
        "nonexistent file should return None (canonicalize fails)"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_safe_media_path_subdirectory() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_subdir");
    let subdir = dir.join("subdir");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&subdir).unwrap();
    fs::write(subdir.join("nested.mp4"), b"fake data").unwrap();
    let canonical_dir = dir.canonicalize().unwrap();

    let result = safe_media_path("/media/subdir/nested.mp4", &canonical_dir);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), canonical_dir.join("subdir/nested.mp4"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_safe_media_path_empty_media_prefix() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_empty");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let canonical_dir = dir.canonicalize().unwrap();

    // Just "/media/" with nothing after
    let result = safe_media_path("/media/", &canonical_dir);
    // This resolves to media_root itself, which starts_with media_root — allowed
    // The behavior depends on whether media_root is considered a valid "file"
    // Since canonicalize on a directory succeeds and starts_with is true, it returns Some
    // But this is fine — the caller should check if it's a file
    assert!(result.is_some());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_safe_media_path_symlink_escape() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_symlink");
    let outside = std::env::temp_dir().join("atmos_test_safe_media_outside");
    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&dir).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.txt"), b"secret").unwrap();
    let canonical_dir = dir.canonicalize().unwrap();

    // Create symlink inside media_root pointing outside
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, dir.join("escape_link")).unwrap();
        let result = safe_media_path("/media/escape_link/secret.txt", &canonical_dir);
        assert!(result.is_none(), "symlink escape should be blocked");
    }

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(&outside);
}

// ── validate_file_type tests ──

fn create_temp_file(name: &str, data: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir();
    let path = dir.join(name);
    fs::write(&path, data).unwrap();
    path
}

#[test]
fn test_validate_file_type_valid_png() {
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE,
    ];
    let path = create_temp_file("test_validate_png.png", &png_bytes);
    assert!(validate_file_type(&path, "png").is_ok());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_valid_jpeg() {
    let jpeg_bytes: Vec<u8> = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00, 0xFF, 0xD9,
    ];
    let path = create_temp_file("test_validate_jpeg.jpg", &jpeg_bytes);
    assert!(validate_file_type(&path, "jpg").is_ok());
    assert!(validate_file_type(&path, "jpeg").is_ok());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_mismatched_extension() {
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE,
    ];
    let path = create_temp_file("test_validate_mismatch.jpg", &png_bytes);
    // PNG data with .jpg extension should fail
    assert!(validate_file_type(&path, "jpg").is_err());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_text_as_mp4() {
    let path = create_temp_file("test_validate_fake.mp4", b"this is not a video");
    assert!(validate_file_type(&path, "mp4").is_err());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_unknown_extension() {
    let path = create_temp_file("test_validate_unknown.xyz", b"some content");
    assert!(validate_file_type(&path, "xyz").is_err());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_gif() {
    // Minimal GIF89a header
    let gif_bytes: Vec<u8> = vec![
        0x47, 0x49, 0x46, 0x38, 0x39, 0x61, // GIF89a
        0x01, 0x00, 0x01, 0x00, // 1x1
        0x00, 0x00, 0x00, 0x21, // color table + extension introducer
        0xF9, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, // GCE
        0x2C, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, // image descriptor
        0x02, 0x02, 0x44, 0x01, 0x00, // image data
        0x3B, // trailer
    ];
    let path = create_temp_file("test_validate_gif.gif", &gif_bytes);
    assert!(validate_file_type(&path, "gif").is_ok());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_webp() {
    // Minimal WEBP header (RIFF....WEBP)
    let mut webp_bytes: Vec<u8> = vec![
        0x52, 0x49, 0x46, 0x46, // RIFF
        0x24, 0x00, 0x00, 0x00, // file size - 8
        0x57, 0x45, 0x42, 0x50, // WEBP
        0x56, 0x50, 0x38, 0x20, // VP8 chunk
        0x18, 0x00, 0x00, 0x00, // chunk size
    ];
    // Pad with zeros to make valid VP8 bitstream header
    webp_bytes.extend_from_slice(&[0u8; 24]);
    let path = create_temp_file("test_validate_webp.webp", &webp_bytes);
    assert!(validate_file_type(&path, "webp").is_ok());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_bmp() {
    // Minimal BMP header
    let mut bmp_bytes: Vec<u8> = vec![
        0x42, 0x4D, // BM signature
        0x36, 0x00, 0x00, 0x00, // file size
        0x00, 0x00, 0x00, 0x00, // reserved
        0x36, 0x00, 0x00, 0x00, // pixel data offset
    ];
    // DIB header (BITMAPINFOHEADER)
    bmp_bytes.extend_from_slice(&[
        0x28, 0x00, 0x00, 0x00, // header size
        0x01, 0x00, 0x00, 0x00, // width 1
        0x01, 0x00, 0x00, 0x00, // height 1
        0x01, 0x00, // planes
        0x18, 0x00, // bits per pixel (24)
        0x00, 0x00, 0x00, 0x00, // compression
        0x00, 0x00, 0x00, 0x00, // image size
        0x00, 0x00, 0x00, 0x00, // h resolution
        0x00, 0x00, 0x00, 0x00, // v resolution
        0x00, 0x00, 0x00, 0x00, // colors
        0x00, 0x00, 0x00, 0x00, // important colors
    ]);
    // 1 pixel (BGR)
    bmp_bytes.extend_from_slice(&[0x00, 0x00, 0xFF]);
    let path = create_temp_file("test_validate_bmp.bmp", &bmp_bytes);
    assert!(validate_file_type(&path, "bmp").is_ok());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_m3u8_always_allowed() {
    // m3u8 is always allowed (HLS playlist, no fixed magic bytes)
    let path = create_temp_file("test_validate_m3u8.m3u8", b"#EXTM3U\n#EXT-X-VERSION:3\n");
    assert!(validate_file_type(&path, "m3u8").is_ok());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_empty_file() {
    let path = create_temp_file("test_validate_empty.mp4", b"");
    // Empty file has no magic bytes — should fail
    assert!(validate_file_type(&path, "mp4").is_err());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_case_sensitivity() {
    // Extension matching is case-sensitive in the current implementation
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90,
        0x77, 0x53, 0xDE,
    ];
    let path = create_temp_file("test_validate_case.png", &png_bytes);
    // Lowercase should work
    assert!(validate_file_type(&path, "png").is_ok());
    // Uppercase extension should fail (not in the match)
    assert!(validate_file_type(&path, "PNG").is_err());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_text_as_image() {
    let path = create_temp_file("test_validate_fake_img.jpg", b"not an image at all");
    assert!(validate_file_type(&path, "jpg").is_err());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_safe_media_path_root_only() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_rootonly");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let canonical_dir = dir.canonicalize().unwrap();

    // "/media" without trailing slash
    let result = safe_media_path("/media", &canonical_dir);
    assert!(
        result.is_none(),
        "/media without trailing slash should not match prefix"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_safe_media_path_multiple_slashes() {
    let dir = std::env::temp_dir().join("atmos_test_safe_media_multislash");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("file.mp4"), b"data").unwrap();
    let canonical_dir = dir.canonicalize().unwrap();

    // "/media//file.mp4" — double slash
    let result = safe_media_path("/media//file.mp4", &canonical_dir);
    // strip_prefix("/media/") on "/media//file.mp4" yields "/file.mp4"
    // media_root.join("/file.mp4") on Unix replaces with /file.mp4 — outside media_root
    // This should be None (blocked)
    assert!(result.is_none(), "double slash path should be blocked");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_validate_file_type_valid_mp4() {
    // 最小合法 MP4 头：ftyp + isom 主品牌（infer 的 mp4 识别条件）
    let mp4_bytes: Vec<u8> = vec![
        0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70, // size + ftyp
        0x69, 0x73, 0x6F, 0x6D, 0x00, 0x00, 0x02, 0x00, // major brand: isom
        0x69, 0x73, 0x6F, 0x6D, 0x69, 0x73, 0x6F, 0x32, // compatible: isom iso2
        0x6D, 0x70, 0x34, 0x31, 0x00, 0x00, 0x00, 0x00, // compatible: mp41
    ];
    let path = create_temp_file("test_validate_mp4.mp4", &mp4_bytes);
    assert!(validate_file_type(&path, "mp4").is_ok());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_validate_file_type_mp4_rejects_quicktime_brand() {
    // ftyp 但主品牌是 qt（QuickTime）→ 应识别为 mov 而非 mp4
    let qt_bytes: Vec<u8> = vec![
        0x00, 0x00, 0x00, 0x14, 0x66, 0x74, 0x79, 0x70, // size + ftyp
        0x71, 0x74, 0x20, 0x20, 0x00, 0x00, 0x00, 0x00, // major brand: qt
        0x71, 0x74, 0x20, 0x20, // compatible: qt
    ];
    let path = create_temp_file("test_validate_qt.mp4", &qt_bytes);
    assert!(
        validate_file_type(&path, "mp4").is_err(),
        "QuickTime 品牌文件不应被当作 mp4 接受"
    );
    assert!(validate_file_type(&path, "mov").is_ok());
    let _ = fs::remove_file(&path);
}

#[test]
fn test_sanitize_filename_backslash_windows_traversal() {
    // Windows 风格路径穿越（反斜杠）→ 必须被替换为 '_'
    let result = sanitize_filename("..\\..\\evil.mp4");
    assert!(!result.contains('\\'), "反斜杠必须被清除");
    assert!(!result.contains('/'), "正斜杠必须被清除");
    assert_eq!(result, ".._.._evil.mp4");
}

#[test]
fn test_sanitize_filename_absolute_path() {
    // 绝对路径只保留最后一段
    assert_eq!(sanitize_filename("/etc/passwd"), "passwd");
    assert_eq!(sanitize_filename("../../etc/shadow"), "shadow");
}

#[test]
fn test_is_safe_external_url_rejects_non_http() {
    assert!(!is_safe_external_url("ftp://example.com/v.mp4"));
    assert!(!is_safe_external_url("file:///etc/passwd"));
    assert!(!is_safe_external_url("javascript:alert(1)"));
    // 无主机名
    assert!(!is_safe_external_url("http:///no-host.mp4"));
    assert!(!is_safe_external_url("https://"));
}

#[test]
fn test_is_safe_external_url_port_and_userinfo() {
    // 带端口与 userinfo 的合法外部地址
    assert!(is_safe_external_url("https://example.com:8443/v.mp4"));
    assert!(is_safe_external_url(
        "http://user:pass@example.com:8080/v.mp4"
    ));
    // userinfo 不能掩盖私有主机
    assert!(!is_safe_external_url(
        "http://user:pass@10.0.0.1:8080/v.mp4"
    ));
}
