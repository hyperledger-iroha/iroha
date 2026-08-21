//! Snapshot naming and storage-layout helpers.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) fn default_snapshot_slug() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    format!("snapshot-{}-{:03}", now.as_secs(), now.subsec_millis())
}
pub(super) const SNAPSHOT_LABEL_MAX_LEN: usize = 64;
pub(super) const SNAPSHOT_STORAGE_LAYOUT: &str = "kura-subdirectory-v1";
pub(super) fn sanitize_snapshot_label(label: &str) -> Option<String> {
    let mut sanitized = String::with_capacity(label.len().min(SNAPSHOT_LABEL_MAX_LEN));
    let mut previous_was_sep = true;
    for ch in label.chars() {
        match ch {
            'a'..='z' | '0'..='9' => {
                if sanitized.len() == SNAPSHOT_LABEL_MAX_LEN {
                    break;
                }
                sanitized.push(ch);
                previous_was_sep = false;
            }
            'A'..='Z' => {
                if sanitized.len() == SNAPSHOT_LABEL_MAX_LEN {
                    break;
                }
                sanitized.push(ch.to_ascii_lowercase());
                previous_was_sep = false;
            }
            '-' | '_' | ' ' | '.' => {
                if !previous_was_sep && sanitized.len() < SNAPSHOT_LABEL_MAX_LEN {
                    sanitized.push('-');
                    previous_was_sep = true;
                }
            }
            _ => {
                // Skip unsupported characters but continue scanning so alphanumeric
                // runs can still be recovered.
            }
        }
    }
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
