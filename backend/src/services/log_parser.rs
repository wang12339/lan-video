use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::models::admin::LogEntry;

/// Parsed result from a log file: the file name and its entries.
pub struct ParsedLog {
    pub file_name: String,
    pub entries: Vec<LogEntry>,
}

/// Discover log files in `dir`, sorted by filename descending (most recent first).
pub fn discover_log_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.contains(".log") && !name.starts_with('.')
        })
        .map(|e| e.path())
        .collect();

    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    files
}

/// Parse a single JSON log file, reading from the tail for large files.
/// Returns entries in reverse-chronological order (most recent first).
/// Stops early once `needed` entries are collected (needed = limit + offset).
pub fn parse_log_file(path: &Path, needed: usize) -> Vec<LogEntry> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut reader = BufReader::new(file);

    // For large files, seek to ~1MB from the end
    let seek_pos = if file_size > 1_000_000 {
        SeekFrom::End(-1_000_000)
    } else {
        SeekFrom::Start(0)
    };
    if reader.seek(seek_pos).is_err() {
        return Vec::new();
    }

    // Read all lines from the seek point
    let mut all_lines: Vec<String> = Vec::new();
    for line_result in reader.lines() {
        match line_result {
            Ok(line) => all_lines.push(line),
            Err(_) => break,
        }
    }

    // Process lines in reverse (most recent first)
    let mut entries: Vec<LogEntry> = Vec::new();
    for line in all_lines.iter().rev() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(entry) = parse_log_line(line) {
            entries.push(entry);
            if entries.len() >= needed {
                break;
            }
        }
    }

    entries
}

/// Parse a single JSON log line into a LogEntry, applying static-resource filtering.
fn parse_log_line(line: &str) -> Option<LogEntry> {
    let parsed: serde_json::Value = serde_json::from_str(line).ok()?;

    let level = parsed
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("INFO")
        .to_uppercase();

    let fields = parsed.get("fields").and_then(|v| v.as_object());

    let message = fields
        .and_then(|f| f.get("message"))
        .or_else(|| parsed.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Filter out static resource requests
    let path = fields
        .and_then(|f| f.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if path.starts_with("/webapp/") || path.starts_with("/media/") || path == "/health" {
        return None;
    }

    let timestamp = parsed
        .get("timestamp")
        .and_then(|v| v.as_str())
        .or_else(|| parsed.get("time").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let request_id = parsed
        .get("span")
        .and_then(|s| s.get("request_id"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            fields
                .and_then(|f| f.get("request_id"))
                .and_then(|v| v.as_str())
        })
        .map(String::from);

    Some(LogEntry {
        timestamp,
        level,
        message,
        method: fields
            .and_then(|f| f.get("method"))
            .and_then(|v| v.as_str())
            .map(String::from),
        path: fields
            .and_then(|f| f.get("path"))
            .and_then(|v| v.as_str())
            .map(String::from),
        status: fields
            .and_then(|f| f.get("status"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u16),
        duration_ms: fields
            .and_then(|f| f.get("duration_ms"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u128),
        request_id,
        user: fields
            .and_then(|f| f.get("user"))
            .and_then(|v| v.as_str())
            .map(String::from),
        video_id: fields
            .and_then(|f| f.get("video_id"))
            .and_then(|v| v.as_i64()),
        error: fields
            .and_then(|f| f.get("error"))
            .and_then(|v| v.as_str())
            .map(String::from),
        action: fields
            .and_then(|f| f.get("action"))
            .and_then(|v| v.as_str())
            .map(String::from),
        target: fields
            .and_then(|f| f.get("target"))
            .and_then(|v| v.as_str())
            .map(String::from),
        page: fields
            .and_then(|f| f.get("page"))
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

/// Filter entries by level and search term.
/// Filters against message, method, path, and user fields.
pub fn filter_entries(
    entries: Vec<LogEntry>,
    level: Option<&str>,
    search: Option<&str>,
) -> Vec<LogEntry> {
    let level_upper = level.map(|l| l.to_uppercase());
    let search_lower = search.map(|s| s.to_lowercase());

    entries
        .into_iter()
        .filter(|e| {
            // Level filter
            if let Some(ref lvl) = level_upper {
                if e.level != *lvl {
                    return false;
                }
            }
            // Search filter — check message, method, path, user
            if let Some(ref q) = search_lower {
                let in_msg = e.message.to_lowercase().contains(q.as_str());
                let in_method = e
                    .method
                    .as_deref()
                    .map(|m| m.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                let in_path = e
                    .path
                    .as_deref()
                    .map(|p| p.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                let in_user = e
                    .user
                    .as_deref()
                    .map(|u| u.to_lowercase().contains(q.as_str()))
                    .unwrap_or(false);
                if !(in_msg || in_method || in_path || in_user) {
                    return false;
                }
            }
            true
        })
        .collect()
}
