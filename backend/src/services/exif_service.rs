//! EXIF 元数据解析。
//!
//! 供图片上传后回填 `videos.exif_*` 列使用：只解析元数据，任何失败（不支持的
//! 文件类型、损坏数据、完全没有 EXIF）都返回 `None`，由调用方决定回退行为。
//! `parse_exif_file` 是同步阻塞 API，异步调用方应放入 `spawn_blocking`。
//!
//! 注意：EXIF 的拍摄时间不带时区，这里按 UTC 归一化存入 `taken_at`。

use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::Path;

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use exif::{In, Tag, Value};

/// 单个字符串字段的最大长度（字符数），避免异常 EXIF 撑大数据库列。
const MAX_STRING_LEN: usize = 200;

/// 从图片中解析出的 EXIF 字段（缺失的为 `None`）。
#[derive(Debug, Clone, Default)]
pub struct ParsedExif {
    /// 拍摄时间（DateTimeOriginal 优先，无则 DateTimeDigitized/DateTime）。
    pub taken_at: Option<DateTime<Utc>>,
    /// GPS 纬度（十进制度，南纬为负）。
    pub lat: Option<f64>,
    /// GPS 经度（十进制度，西经为负）。
    pub lon: Option<f64>,
    /// 相机：Make 与 Model 去重拼接，如 "Apple iPhone 15 Pro"。
    pub camera: Option<String>,
    /// 镜头型号。
    pub lens: Option<String>,
    /// 光圈 FNumber，如 1.8。
    pub aperture: Option<f64>,
    /// 快门速度，如 "1/500"。
    pub shutter: Option<String>,
    /// ISO 感光度。
    pub iso: Option<i64>,
    /// 焦距（毫米）。
    pub focal_length: Option<f64>,
    /// EXIF Orientation 原值（1..=8）。
    pub orientation: Option<i32>,
}

/// 解析磁盘上的图片文件。
///
/// 同步阻塞；JPEG 只读取 EXIF 段而非整个文件。异步调用方请使用
/// `tokio::task::spawn_blocking`。
pub fn parse_exif_file(path: &Path) -> Option<ParsedExif> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
    Some(extract(&exif))
}

/// 解析内存中的图片字节。
///
/// 支持 kamadak-exif 的容器范围（JPEG/TIFF/PNG/HEIF/WebP 等）；
/// 不支持或解析失败返回 `None`。
pub fn parse_exif_bytes(bytes: &[u8]) -> Option<ParsedExif> {
    let mut cursor = Cursor::new(bytes);
    let exif = exif::Reader::new().read_from_container(&mut cursor).ok()?;
    Some(extract(&exif))
}

fn extract(exif: &exif::Exif) -> ParsedExif {
    ParsedExif {
        taken_at: parse_taken_at(exif),
        lat: parse_gps_coord(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef),
        lon: parse_gps_coord(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef),
        camera: combine_camera(
            parse_ascii_tag(exif, Tag::Make),
            parse_ascii_tag(exif, Tag::Model),
        ),
        lens: parse_ascii_tag(exif, Tag::LensModel),
        aperture: parse_rational_tag(exif, Tag::FNumber),
        shutter: parse_shutter(exif),
        iso: parse_iso(exif),
        focal_length: parse_rational_tag(exif, Tag::FocalLength),
        orientation: parse_orientation(exif),
    }
}

fn parse_taken_at(exif: &exif::Exif) -> Option<DateTime<Utc>> {
    for tag in [Tag::DateTimeOriginal, Tag::DateTimeDigitized, Tag::DateTime] {
        let Some(field) = exif.get_field(tag, In::PRIMARY) else {
            continue;
        };
        let Value::Ascii(parts) = &field.value else {
            continue;
        };
        let Some(raw) = parts.first() else {
            continue;
        };
        let Some(raw) = raw.split(|&b| b == 0).next() else {
            continue;
        };
        if let Some(utc) = exif::DateTime::from_ascii(raw)
            .ok()
            .and_then(|parsed| exif_datetime_to_utc(&parsed))
        {
            return Some(utc);
        }
    }
    None
}

fn exif_datetime_to_utc(dt: &exif::DateTime) -> Option<DateTime<Utc>> {
    let date = NaiveDate::from_ymd_opt(i32::from(dt.year), u32::from(dt.month), u32::from(dt.day))?;
    let time = NaiveTime::from_hms_opt(
        u32::from(dt.hour),
        u32::from(dt.minute),
        u32::from(dt.second),
    )?;
    Some(Utc.from_utc_datetime(&NaiveDateTime::new(date, time)))
}

/// 解析 GPS 度分秒有理数并换算为十进制度，按 ref 的 N/S/E/W 决定符号。
fn parse_gps_coord(exif: &exif::Exif, coord_tag: Tag, ref_tag: Tag) -> Option<f64> {
    let field = exif.get_field(coord_tag, In::PRIMARY)?;
    let Value::Rational(parts) = &field.value else {
        return None;
    };
    let degrees = parts.first()?.to_f64();
    let minutes = parts.get(1).map(|r| r.to_f64()).unwrap_or(0.0);
    let seconds = parts.get(2).map(|r| r.to_f64()).unwrap_or(0.0);
    if !degrees.is_finite() || !minutes.is_finite() || !seconds.is_finite() {
        return None;
    }
    let magnitude = degrees + minutes / 60.0 + seconds / 3600.0;
    if !(0.0..=180.0).contains(&magnitude) {
        return None;
    }
    let negative = exif
        .get_field(ref_tag, In::PRIMARY)
        .and_then(|f| ascii_from_value(&f.value))
        .is_some_and(|reference| {
            reference.eq_ignore_ascii_case("S") || reference.eq_ignore_ascii_case("W")
        });
    Some(if negative { -magnitude } else { magnitude })
}

fn parse_ascii_tag(exif: &exif::Exif, tag: Tag) -> Option<String> {
    ascii_from_value(&exif.get_field(tag, In::PRIMARY)?.value)
}

fn ascii_from_value(value: &Value) -> Option<String> {
    let Value::Ascii(parts) = value else {
        return None;
    };
    sanitize_bytes(parts.first()?)
}

/// 去掉 NUL 后的内容、trim 空白并截断到 [`MAX_STRING_LEN`] 字符；空串视为缺失。
fn sanitize_bytes(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    let text = text.split('\0').next()?.trim();
    if text.is_empty() {
        return None;
    }
    Some(text.chars().take(MAX_STRING_LEN).collect())
}

fn combine_camera(make: Option<String>, model: Option<String>) -> Option<String> {
    match (make, model) {
        (Some(make), Some(model)) => {
            let combined = if model_mentions_make(&make, &model) {
                model
            } else {
                format!("{make} {model}")
            };
            truncate_owned(combined)
        }
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

/// Model 已包含 Make 时不重复拼接（如 Make=Apple / Model=Apple iPhone 15 Pro，
/// 或 Make=NIKON CORPORATION / Model=NIKON D850）。
fn model_mentions_make(make: &str, model: &str) -> bool {
    let make_lower = make.to_lowercase();
    let model_lower = model.to_lowercase();
    if model_lower.starts_with(&make_lower) {
        return true;
    }
    make_lower
        .split_whitespace()
        .next()
        .is_some_and(|first_word| model_lower.starts_with(first_word))
}

fn truncate_owned(text: String) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    if text.chars().count() <= MAX_STRING_LEN {
        return Some(text);
    }
    Some(text.chars().take(MAX_STRING_LEN).collect())
}

fn parse_rational_tag(exif: &exif::Exif, tag: Tag) -> Option<f64> {
    rational_f64(&exif.get_field(tag, In::PRIMARY)?.value)
}

fn rational_f64(value: &Value) -> Option<f64> {
    let raw = match value {
        Value::Rational(parts) => parts.first()?.to_f64(),
        Value::SRational(parts) => parts.first()?.to_f64(),
        _ => return None,
    };
    if raw.is_finite() {
        Some(raw)
    } else {
        None
    }
}

fn parse_shutter(exif: &exif::Exif) -> Option<String> {
    let field = exif.get_field(Tag::ExposureTime, In::PRIMARY)?;
    let Value::Rational(parts) = &field.value else {
        return None;
    };
    format_exposure_time(parts.first()?)
}

/// 曝光时间 < 1 秒输出约分后的 "1/500"；>= 1 秒输出小数秒，如 "2"、"1.3"。
fn format_exposure_time(rational: &exif::Rational) -> Option<String> {
    if rational.num == 0 || rational.denom == 0 {
        return None;
    }
    if rational.num < rational.denom {
        let divisor = gcd(rational.num, rational.denom);
        return Some(format!(
            "{}/{}",
            rational.num / divisor,
            rational.denom / divisor
        ));
    }
    let seconds = f64::from(rational.num) / f64::from(rational.denom);
    if !seconds.is_finite() {
        return None;
    }
    Some(trim_decimal(seconds))
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a.max(1)
}

fn trim_decimal(value: f64) -> String {
    let mut text = format!("{value:.4}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}

fn parse_iso(exif: &exif::Exif) -> Option<i64> {
    for tag in [Tag::PhotographicSensitivity, Tag::ISOSpeed] {
        let Some(value) = exif
            .get_field(tag, In::PRIMARY)
            .and_then(|field| field.value.get_uint(0))
        else {
            continue;
        };
        if value > 0 {
            return Some(i64::from(value));
        }
    }
    None
}

fn parse_orientation(exif: &exif::Exif) -> Option<i32> {
    let value = exif
        .get_field(Tag::Orientation, In::PRIMARY)?
        .value
        .get_uint(0)?;
    if (1..=8).contains(&value) {
        Some(value as i32)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const TYPE_ASCII: u16 = 2;
    const TYPE_SHORT: u16 = 3;
    const TYPE_LONG: u16 = 4;
    const TYPE_RATIONAL: u16 = 5;

    fn push_entry(buf: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value: [u8; 4]) {
        buf.extend_from_slice(&tag.to_le_bytes());
        buf.extend_from_slice(&ty.to_le_bytes());
        buf.extend_from_slice(&count.to_le_bytes());
        buf.extend_from_slice(&value);
    }

    fn push_rationals(buf: &mut Vec<u8>, values: &[(u32, u32)]) {
        for (num, denom) in values {
            buf.extend_from_slice(&num.to_le_bytes());
            buf.extend_from_slice(&denom.to_le_bytes());
        }
    }

    /// 构造最小合法小端 TIFF：IFD0（Make/Model/Orientation + Exif/GPS 指针）
    /// + Exif IFD + GPS IFD，值数据区紧随其后。
    fn build_tiff() -> Vec<u8> {
        let make = b"TestMake\0";
        let model = b"TestModel\0";
        let datetime_original = b"2024:05:20 14:30:45\0";
        let lens_model = b"TestLens 24-70\0";
        // GPS refs 只有 2 字节（<=4），按 TIFF 规范内联在 value 字段中，
        // 不写偏移；否则读取方会把偏移字节当作 ASCII 内容。
        let lat_ref_inline = [b'S', 0, 0, 0];
        let lon_ref_inline = [b'W', 0, 0, 0];

        const IFD0_COUNT: u32 = 5;
        const EXIF_COUNT: u32 = 6;
        const GPS_COUNT: u32 = 4;

        let ifd0_off = 8u32;
        let ifd0_size = 2 + IFD0_COUNT * 12 + 4;
        let exif_ifd_off = ifd0_off + ifd0_size;
        let exif_ifd_size = 2 + EXIF_COUNT * 12 + 4;
        let gps_ifd_off = exif_ifd_off + exif_ifd_size;
        let gps_ifd_size = 2 + GPS_COUNT * 12 + 4;
        let data_off = gps_ifd_off + gps_ifd_size;

        let mut data: Vec<u8> = Vec::new();
        let make_off = data_off + data.len() as u32;
        data.extend_from_slice(make);
        let model_off = data_off + data.len() as u32;
        data.extend_from_slice(model);
        let datetime_off = data_off + data.len() as u32;
        data.extend_from_slice(datetime_original);
        let lens_off = data_off + data.len() as u32;
        data.extend_from_slice(lens_model);
        let lat_off = data_off + data.len() as u32;
        push_rationals(&mut data, &[(33, 1), (52, 1), (30, 1)]);
        let lon_off = data_off + data.len() as u32;
        push_rationals(&mut data, &[(151, 1), (12, 1), (0, 1)]);
        let exposure_off = data_off + data.len() as u32;
        push_rationals(&mut data, &[(1, 500)]);
        let fnumber_off = data_off + data.len() as u32;
        push_rationals(&mut data, &[(18, 10)]);
        let focal_off = data_off + data.len() as u32;
        push_rationals(&mut data, &[(50, 1)]);

        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II");
        tiff.extend_from_slice(&42u16.to_le_bytes());
        tiff.extend_from_slice(&ifd0_off.to_le_bytes());

        tiff.extend_from_slice(&(IFD0_COUNT as u16).to_le_bytes());
        push_entry(
            &mut tiff,
            0x010F,
            TYPE_ASCII,
            make.len() as u32,
            make_off.to_le_bytes(),
        );
        push_entry(
            &mut tiff,
            0x0110,
            TYPE_ASCII,
            model.len() as u32,
            model_off.to_le_bytes(),
        );
        push_entry(&mut tiff, 0x0112, TYPE_SHORT, 1, 6u32.to_le_bytes());
        push_entry(&mut tiff, 0x8769, TYPE_LONG, 1, exif_ifd_off.to_le_bytes());
        push_entry(&mut tiff, 0x8825, TYPE_LONG, 1, gps_ifd_off.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());

        tiff.extend_from_slice(&(EXIF_COUNT as u16).to_le_bytes());
        push_entry(
            &mut tiff,
            0x829A,
            TYPE_RATIONAL,
            1,
            exposure_off.to_le_bytes(),
        );
        push_entry(
            &mut tiff,
            0x829D,
            TYPE_RATIONAL,
            1,
            fnumber_off.to_le_bytes(),
        );
        push_entry(&mut tiff, 0x8827, TYPE_SHORT, 1, 100u32.to_le_bytes());
        push_entry(
            &mut tiff,
            0x9003,
            TYPE_ASCII,
            datetime_original.len() as u32,
            datetime_off.to_le_bytes(),
        );
        push_entry(&mut tiff, 0x920A, TYPE_RATIONAL, 1, focal_off.to_le_bytes());
        push_entry(
            &mut tiff,
            0xA434,
            TYPE_ASCII,
            lens_model.len() as u32,
            lens_off.to_le_bytes(),
        );
        tiff.extend_from_slice(&0u32.to_le_bytes());

        tiff.extend_from_slice(&(GPS_COUNT as u16).to_le_bytes());
        push_entry(&mut tiff, 0x0001, TYPE_ASCII, 2, lat_ref_inline);
        push_entry(&mut tiff, 0x0002, TYPE_RATIONAL, 3, lat_off.to_le_bytes());
        push_entry(&mut tiff, 0x0003, TYPE_ASCII, 2, lon_ref_inline);
        push_entry(&mut tiff, 0x0004, TYPE_RATIONAL, 3, lon_off.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes());

        tiff.extend_from_slice(&data);
        tiff
    }

    #[test]
    fn parses_minimal_tiff_exif_and_gps() {
        let parsed = parse_exif_bytes(&build_tiff()).unwrap();
        let expected = chrono::Utc
            .with_ymd_and_hms(2024, 5, 20, 14, 30, 45)
            .unwrap();
        assert_eq!(parsed.taken_at, Some(expected));
        assert!((parsed.lat.unwrap() - (-33.875)).abs() < 1e-9);
        assert!((parsed.lon.unwrap() - (-151.2)).abs() < 1e-9);
        assert_eq!(parsed.camera.as_deref(), Some("TestMake TestModel"));
        assert_eq!(parsed.lens.as_deref(), Some("TestLens 24-70"));
        assert!((parsed.aperture.unwrap() - 1.8).abs() < 1e-9);
        assert_eq!(parsed.shutter.as_deref(), Some("1/500"));
        assert_eq!(parsed.iso, Some(100));
        assert!((parsed.focal_length.unwrap() - 50.0).abs() < 1e-9);
        assert_eq!(parsed.orientation, Some(6));
    }

    #[test]
    fn invalid_bytes_return_none() {
        assert!(parse_exif_bytes(&[]).is_none());
        assert!(parse_exif_bytes(b"not an image at all").is_none());
        assert!(parse_exif_bytes(&build_tiff()[..20]).is_none());
    }

    #[test]
    fn formats_exposure_time() {
        let one_over = exif::Rational {
            num: 10,
            denom: 500,
        };
        assert_eq!(format_exposure_time(&one_over).as_deref(), Some("1/50"));
        let fast = exif::Rational { num: 1, denom: 500 };
        assert_eq!(format_exposure_time(&fast).as_deref(), Some("1/500"));
        let seconds = exif::Rational { num: 2, denom: 1 };
        assert_eq!(format_exposure_time(&seconds).as_deref(), Some("2"));
        let decimal = exif::Rational { num: 13, denom: 10 };
        assert_eq!(format_exposure_time(&decimal).as_deref(), Some("1.3"));
        assert_eq!(
            format_exposure_time(&exif::Rational { num: 0, denom: 100 }),
            None
        );
        assert_eq!(
            format_exposure_time(&exif::Rational { num: 1, denom: 0 }),
            None
        );
    }

    #[test]
    fn combines_camera_without_duplicating_make() {
        assert_eq!(
            combine_camera(Some("Apple".into()), Some("Apple iPhone 15 Pro".into())).as_deref(),
            Some("Apple iPhone 15 Pro")
        );
        assert_eq!(
            combine_camera(Some("NIKON CORPORATION".into()), Some("NIKON D850".into())).as_deref(),
            Some("NIKON D850")
        );
        assert_eq!(
            combine_camera(Some("Canon".into()), Some("EOS R5".into())).as_deref(),
            Some("Canon EOS R5")
        );
        assert_eq!(combine_camera(None, None), None);
    }

    #[test]
    fn sanitizes_and_truncates_strings() {
        assert_eq!(sanitize_bytes(b"  hello\0tail").as_deref(), Some("hello"));
        assert_eq!(sanitize_bytes(b"   "), None);
        assert_eq!(sanitize_bytes(&[0u8, 0, 0]), None);
        assert_eq!(
            sanitize_bytes(&vec![b'A'; 300]).unwrap().chars().count(),
            200
        );
    }
}
