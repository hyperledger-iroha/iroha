use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json as serde_json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum Command {
    Summary(SummaryOptions),
}

#[derive(Debug)]
pub struct SummaryOptions {
    pub log_path: PathBuf,
    pub filter_wave: Option<String>,
    pub output: SummaryOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryOutput {
    Human,
    JsonStdout,
    JsonFile(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSerialize)]
pub struct WaveSummary {
    pub wave: String,
    pub total_events: usize,
    pub event_counts: BTreeMap<String, usize>,
    pub invites_total: usize,
    pub invites_open: usize,
    pub last_event_timestamp: Option<String>,
}

#[derive(Debug, Deserialize, JsonDeserialize)]
struct LogFile {
    version: u32,
    entries: Vec<LogEntry>,
}

#[derive(Debug, Deserialize, JsonDeserialize)]
#[allow(dead_code)]
struct LogEntry {
    wave: Option<String>,
    recipient: Option<String>,
    event: Option<String>,
    timestamp: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Default)]
struct RecipientState {
    has_invite: bool,
    is_closed: bool,
}

pub fn default_log_path() -> PathBuf {
    PathBuf::from("artifacts/docs_portal_preview/feedback_log.json")
}

pub fn run(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Summary(options) => run_summary(options),
    }
}

fn run_summary(options: SummaryOptions) -> Result<(), Box<dyn Error>> {
    let entries = load_log_entries(&options.log_path)?;
    let summary = summarize_entries(&entries, options.filter_wave.as_deref());
    match options.output {
        SummaryOutput::Human => {
            println!("{}", format_summary(&summary));
        }
        SummaryOutput::JsonStdout => {
            println!("{}", serde_json::to_json_pretty(&summary)?);
        }
        SummaryOutput::JsonFile(path) => {
            let rendered = serde_json::to_json_pretty(&summary)?;
            fs::write(path, rendered)?;
        }
    }
    Ok(())
}

fn load_log_entries(path: &Path) -> Result<Vec<LogEntry>, Box<dyn Error>> {
    let data = fs::read_to_string(path)?;
    let parsed: LogFile = serde_json::from_str(&data)?;
    if parsed.version != 1 {
        return Err(format!("unsupported log version {}", parsed.version).into());
    }
    Ok(parsed.entries)
}

fn summarize_entries(entries: &[LogEntry], filter_wave: Option<&str>) -> Vec<WaveSummary> {
    let mut waves: BTreeMap<String, WaveSummary> = BTreeMap::new();
    let mut recipient_state: BTreeMap<(String, String), RecipientState> = BTreeMap::new();

    for entry in entries {
        let Some(wave_label) = entry.wave.as_deref() else {
            continue;
        };
        if filter_wave.is_some_and(|filter| filter != wave_label) {
            continue;
        }
        let summary = waves
            .entry(wave_label.to_string())
            .or_insert_with(|| WaveSummary {
                wave: wave_label.to_string(),
                total_events: 0,
                event_counts: BTreeMap::new(),
                invites_total: 0,
                invites_open: 0,
                last_event_timestamp: None,
            });
        summary.total_events += 1;
        if let Some(event_name) = entry.event.as_deref() {
            *summary
                .event_counts
                .entry(event_name.to_string())
                .or_insert(0) += 1;
            track_recipient_state(
                &mut recipient_state,
                wave_label,
                event_name,
                entry.recipient.as_deref(),
            );
        }
        if let Some(ts) = entry.timestamp.as_ref()
            && summary
                .last_event_timestamp
                .as_ref()
                .map(|current| ts > current)
                .unwrap_or(true)
        {
            summary.last_event_timestamp = Some(ts.clone());
        }
    }

    for ((wave, _recipient), state) in &recipient_state {
        if let Some(summary) = waves.get_mut(wave)
            && state.has_invite
        {
            summary.invites_total += 1;
            if !state.is_closed {
                summary.invites_open += 1;
            }
        }
    }

    waves.into_values().collect()
}

fn track_recipient_state(
    recipient_state: &mut BTreeMap<(String, String), RecipientState>,
    wave: &str,
    event: &str,
    recipient: Option<&str>,
) {
    let recipient_label = recipient.unwrap_or("unknown");
    let key = (wave.to_string(), recipient_label.to_string());
    let state = recipient_state.entry(key).or_default();
    match event {
        "invite-sent" => {
            state.has_invite = true;
            state.is_closed = false;
        }
        "access-revoked" => {
            state.is_closed = true;
        }
        _ => {}
    }
}

pub fn format_summary(summary: &[WaveSummary]) -> String {
    if summary.is_empty() {
        return "No preview feedback entries found.".to_string();
    }
    let mut lines = Vec::new();
    for item in summary {
        lines.push(format!("Wave {}", item.wave));
        lines.push(format!("  total events: {}", item.total_events));
        lines.push(format!(
            "  invites logged: {} (open: {})",
            item.invites_total, item.invites_open
        ));
        let feedback = item
            .event_counts
            .get("feedback-submitted")
            .copied()
            .unwrap_or(0);
        let issues = item.event_counts.get("issue-opened").copied().unwrap_or(0);
        lines.push(format!("  feedback submissions: {feedback}"));
        lines.push(format!("  issues opened: {issues}"));
        if let Some(ts) = item.last_event_timestamp.as_ref() {
            lines.push(format!("  last event: {ts}"));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<LogEntry> {
        vec![
            LogEntry {
                wave: Some("preview-1".into()),
                recipient: Some("alice@example.com".into()),
                event: Some("invite-sent".into()),
                timestamp: Some("2026-03-01T12:00:00Z".into()),
                notes: None,
            },
            LogEntry {
                wave: Some("preview-1".into()),
                recipient: Some("alice@example.com".into()),
                event: Some("feedback-submitted".into()),
                timestamp: Some("2026-03-02T12:00:00Z".into()),
                notes: None,
            },
            LogEntry {
                wave: Some("preview-1".into()),
                recipient: Some("bob@example.com".into()),
                event: Some("invite-sent".into()),
                timestamp: Some("2026-03-03T12:00:00Z".into()),
                notes: None,
            },
            LogEntry {
                wave: Some("preview-1".into()),
                recipient: Some("bob@example.com".into()),
                event: Some("access-revoked".into()),
                timestamp: Some("2026-03-04T12:00:00Z".into()),
                notes: None,
            },
        ]
    }

    #[test]
    fn summarize_entries_counts_events() {
        let summary = summarize_entries(&sample_entries(), None);
        assert_eq!(summary.len(), 1);
        let wave = &summary[0];
        assert_eq!(wave.wave, "preview-1");
        assert_eq!(wave.total_events, 4);
        assert_eq!(wave.invites_total, 2);
        assert_eq!(wave.invites_open, 1);
        assert_eq!(
            wave.event_counts.get("feedback-submitted").copied(),
            Some(1)
        );
        assert_eq!(wave.event_counts.get("invite-sent").copied(), Some(2));
        assert_eq!(
            wave.last_event_timestamp.as_deref(),
            Some("2026-03-04T12:00:00Z")
        );
    }

    #[test]
    fn summarize_entries_filters_wave() {
        let mut entries = sample_entries();
        entries.push(LogEntry {
            wave: Some("preview-2".into()),
            recipient: Some("carol@example.com".into()),
            event: Some("invite-sent".into()),
            timestamp: Some("2026-03-05T12:00:00Z".into()),
            notes: None,
        });
        let summary = summarize_entries(&entries, Some("preview-2"));
        assert_eq!(summary.len(), 1);
        assert_eq!(summary[0].wave, "preview-2");
        assert_eq!(summary[0].total_events, 1);
    }

    #[test]
    fn format_summary_handles_empty() {
        let formatted = format_summary(&[]);
        assert!(formatted.contains("No preview feedback entries"));
    }
}
