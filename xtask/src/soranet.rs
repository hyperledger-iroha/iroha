use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use serde::Serialize;
use soranet_relay::constant_rate::{self, CONSTANT_RATE_CELL_BYTES, ConstantRateProfileSpec};
use thiserror::Error;

pub fn default_fixture_dir(workspace_root: &Path) -> PathBuf {
    workspace_root
        .join("tests")
        .join("interop")
        .join("soranet")
        .join("capabilities")
}

pub fn generate_capability_fixtures(output: &Path) -> Result<(), Box<dyn Error>> {
    soranet_handshake_harness::generate_capability_fixtures(output)
        .map_err(Box::<dyn Error>::from)?;
    Ok(())
}

pub fn verify_fixtures(output: &Path) -> Result<(), Box<dyn Error>> {
    soranet_handshake_harness::verify_fixtures(output).map_err(Box::<dyn Error>::from)?;
    Ok(())
}

const DEFAULT_TICK_VALUES: [f64; 5] = [5.0, 7.5, 10.0, 15.0, 20.0];

#[derive(Debug, Clone)]
pub enum ConstantRateSelection {
    All,
    Named(String),
}

#[derive(Debug, Clone, Copy)]
pub enum ConstantRateOutputFormat {
    Table,
    Json,
    Markdown,
}

#[derive(Debug, Clone)]
pub struct ConstantRateProfileOptions {
    pub selection: ConstantRateSelection,
    pub include_tick_table: bool,
    pub tick_values_ms: Vec<f64>,
}

#[derive(Debug, Error)]
pub enum ConstantRateProfileError {
    #[error("unknown constant-rate profile `{0}`")]
    UnknownProfile(String),
    #[error("tick values must be greater than zero")]
    InvalidTick,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConstantRateProfileSummary {
    pub name: &'static str,
    pub description: &'static str,
    pub tick_millis: f64,
    pub cell_payload_bytes: u32,
    pub lane_cap: u16,
    pub dummy_lane_floor: u16,
    pub per_lane_payload_mbps: f64,
    pub max_payload_mbps: f64,
    pub ceiling_payload_mbps: f64,
    pub ceiling_percent: f64,
    pub recommended_uplink_mbps: f64,
    pub neighbor_cap: u16,
    pub auto_disable_threshold_percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TickBandwidthEntry {
    pub tick_millis: f64,
    pub cells_per_sec: f64,
    pub payload_kib_per_sec: f64,
    pub payload_mbps: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConstantRateProfileReport {
    pub profiles: Vec<ConstantRateProfileSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tick_bandwidth: Option<Vec<TickBandwidthEntry>>,
}

pub fn build_constant_rate_report(
    options: &ConstantRateProfileOptions,
) -> Result<ConstantRateProfileReport, ConstantRateProfileError> {
    let specs: Vec<ConstantRateProfileSpec> = match &options.selection {
        ConstantRateSelection::All => constant_rate::all_profiles().to_vec(),
        ConstantRateSelection::Named(name) => {
            let spec = constant_rate::profile_by_name(name)
                .ok_or_else(|| ConstantRateProfileError::UnknownProfile(name.clone()))?;
            vec![spec]
        }
    };

    let profiles = specs
        .into_iter()
        .map(|spec| ConstantRateProfileSummary {
            name: spec.name,
            description: spec.description,
            tick_millis: spec.tick_millis,
            cell_payload_bytes: CONSTANT_RATE_CELL_BYTES,
            lane_cap: spec.lane_cap,
            dummy_lane_floor: spec.dummy_lane_floor,
            per_lane_payload_mbps: per_lane_payload_mbps(spec.tick_millis),
            max_payload_mbps: per_lane_payload_mbps(spec.tick_millis) * f64::from(spec.lane_cap),
            ceiling_payload_mbps: spec.recommended_uplink_mbps * spec.ceiling_fraction,
            ceiling_percent: spec.ceiling_fraction * 100.0,
            recommended_uplink_mbps: spec.recommended_uplink_mbps,
            neighbor_cap: spec.neighbor_cap,
            auto_disable_threshold_percent: spec.auto_disable_threshold_percent,
        })
        .collect();

    let tick_bandwidth = if options.include_tick_table {
        let ticks = if options.tick_values_ms.is_empty() {
            DEFAULT_TICK_VALUES.to_vec()
        } else {
            options.tick_values_ms.clone()
        };
        let entries = ticks
            .into_iter()
            .map(TickBandwidthEntry::try_from_tick)
            .collect::<Result<Vec<_>, _>>()?;
        Some(entries)
    } else {
        None
    };

    Ok(ConstantRateProfileReport {
        profiles,
        tick_bandwidth,
    })
}

impl TickBandwidthEntry {
    fn try_from_tick(tick_ms: f64) -> Result<Self, ConstantRateProfileError> {
        if tick_ms <= 0.0 {
            return Err(ConstantRateProfileError::InvalidTick);
        }
        let cells_per_sec = 1000.0 / tick_ms;
        let payload_bytes_per_sec = cells_per_sec * f64::from(CONSTANT_RATE_CELL_BYTES);
        Ok(Self {
            tick_millis: tick_ms,
            cells_per_sec,
            payload_kib_per_sec: payload_bytes_per_sec / 1024.0,
            payload_mbps: payload_bytes_per_sec * 8.0 / 1_000_000.0,
        })
    }
}

fn per_lane_payload_mbps(tick_ms: f64) -> f64 {
    let cells_per_sec = 1000.0 / tick_ms;
    let payload_bits_per_sec = cells_per_sec * f64::from(CONSTANT_RATE_CELL_BYTES) * 8.0;
    payload_bits_per_sec / 1_000_000.0
}

pub fn format_profile_table(profiles: &[ConstantRateProfileSummary]) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(vec![
        "Profile".to_string(),
        "Tick (ms)".to_string(),
        "Cell (B)".to_string(),
        "Lanes".to_string(),
        "Dummy floor".to_string(),
        "Per-lane Mbps".to_string(),
        "Ceiling Mbps".to_string(),
        "Ceiling % uplink".to_string(),
        "Recommended uplink (Mbps)".to_string(),
        "Neighbor cap".to_string(),
        "Auto-disable %".to_string(),
    ]);
    for profile in profiles {
        rows.push(vec![
            profile.name.to_string(),
            format!("{:.1}", profile.tick_millis),
            profile.cell_payload_bytes.to_string(),
            profile.lane_cap.to_string(),
            profile.dummy_lane_floor.to_string(),
            format!("{:.2}", profile.per_lane_payload_mbps),
            format!("{:.2}", profile.ceiling_payload_mbps),
            format!("{:.0}", profile.ceiling_percent),
            format!("{:.1}", profile.recommended_uplink_mbps),
            profile.neighbor_cap.to_string(),
            format!("{:.0}", profile.auto_disable_threshold_percent),
        ]);
    }
    render_table(&rows)
}

pub fn format_tick_table(entries: &[TickBandwidthEntry]) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    rows.push(vec![
        "Tick (ms)".to_string(),
        "Cells/sec".to_string(),
        "Payload KiB/sec".to_string(),
        "Payload Mbps".to_string(),
    ]);
    for entry in entries {
        rows.push(vec![
            format!("{:.1}", entry.tick_millis),
            format!("{:.2}", entry.cells_per_sec),
            format!("{:.2}", entry.payload_kib_per_sec),
            format!("{:.2}", entry.payload_mbps),
        ]);
    }
    render_table(&rows)
}

fn render_table(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let column_count = rows[0].len();
    let mut widths = vec![0usize; column_count];
    for row in rows {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(cell.len());
        }
    }
    let mut output = String::new();
    let total_width: usize = widths.iter().sum::<usize>() + (column_count - 1) * 2;
    for (index, row) in rows.iter().enumerate() {
        for (col, cell) in row.iter().enumerate() {
            output.push_str(cell);
            if col + 1 < column_count {
                let padding = widths[col].saturating_sub(cell.len()) + 2;
                output.push_str(&" ".repeat(padding));
            }
        }
        output.push('\n');
        if index == 0 {
            output.push_str(&"-".repeat(total_width));
            output.push('\n');
        }
    }
    output
}

impl fmt::Display for ConstantRateOutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table => write!(f, "table"),
            Self::Json => write!(f, "json"),
            Self::Markdown => write!(f, "markdown"),
        }
    }
}

pub fn format_profile_markdown_table(profiles: &[ConstantRateProfileSummary]) -> String {
    let mut output = String::new();
    output.push_str("| Profile | Tick (ms) | Cell (B) | Lanes | Dummy floor | Per-lane Mbps | Ceiling Mbps | Ceiling % | Recommended uplink (Mbps) | Neighbor cap | Auto-disable % |\n");
    output.push_str("|---------|-----------|----------|-------|-------------|----------------|--------------|-----------|---------------------------|--------------|----------------|\n");
    for profile in profiles {
        output.push_str(&format!(
            "| {} | {:.1} | {} | {} | {} | {:.2} | {:.2} | {:.0} | {:.1} | {} | {:.0} |\n",
            profile.name,
            profile.tick_millis,
            profile.cell_payload_bytes,
            profile.lane_cap,
            profile.dummy_lane_floor,
            profile.per_lane_payload_mbps,
            profile.ceiling_payload_mbps,
            profile.ceiling_percent,
            profile.recommended_uplink_mbps,
            profile.neighbor_cap,
            profile.auto_disable_threshold_percent,
        ));
    }
    output
}

pub fn format_tick_markdown_table(entries: &[TickBandwidthEntry]) -> String {
    let mut output = String::new();
    output.push_str("| Tick (ms) | Cells/sec | Payload KiB/sec | Payload Mbps |\n");
    output.push_str("|-----------|-----------|-----------------|--------------|\n");
    for entry in entries {
        output.push_str(&format!(
            "| {:.1} | {:.2} | {:.2} | {:.2} |\n",
            entry.tick_millis, entry.cells_per_sec, entry.payload_kib_per_sec, entry.payload_mbps,
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_constant_rate_report_for_core_profile() {
        let options = ConstantRateProfileOptions {
            selection: ConstantRateSelection::Named("core".into()),
            include_tick_table: true,
            tick_values_ms: vec![5.0],
        };
        let report = build_constant_rate_report(&options).expect("report");
        assert_eq!(report.profiles.len(), 1);
        let summary = &report.profiles[0];
        assert_eq!(summary.name, "core");
        assert!((summary.per_lane_payload_mbps - 1.6384).abs() < 1e-6);
        let entries = report.tick_bandwidth.expect("entries");
        let tick = entries.first().expect("tick");
        assert!((tick.payload_mbps - 1.6384).abs() < 1e-6);
    }

    #[test]
    fn format_profile_table_renders_headers() {
        let options = ConstantRateProfileOptions {
            selection: ConstantRateSelection::All,
            include_tick_table: false,
            tick_values_ms: Vec::new(),
        };
        let report = build_constant_rate_report(&options).expect("report");
        let table = format_profile_table(&report.profiles);
        assert!(table.contains("Profile"));
        assert!(table.contains("core"));
        assert!(table.contains("home"));
    }

    #[test]
    fn format_tick_table_renders_entries() {
        let entries = vec![
            TickBandwidthEntry::try_from_tick(5.0).expect("tick"),
            TickBandwidthEntry::try_from_tick(10.0).expect("tick"),
        ];
        let table = format_tick_table(&entries);
        assert!(table.contains("Tick (ms)"));
        assert!(table.contains("5.0"));
        assert!(table.contains("10.0"));
    }

    #[test]
    fn format_profile_markdown_table_renders_headers() {
        let options = ConstantRateProfileOptions {
            selection: ConstantRateSelection::All,
            include_tick_table: false,
            tick_values_ms: Vec::new(),
        };
        let report = build_constant_rate_report(&options).expect("report");
        let markdown = format_profile_markdown_table(&report.profiles);
        assert!(markdown.contains("| Profile | Tick (ms) |"));
        assert!(markdown.contains("| core | 5.0 |"));
    }

    #[test]
    fn format_tick_markdown_table_renders_entries() {
        let entries = vec![
            TickBandwidthEntry::try_from_tick(5.0).expect("tick"),
            TickBandwidthEntry::try_from_tick(7.5).expect("tick"),
        ];
        let markdown = format_tick_markdown_table(&entries);
        assert!(markdown.contains("| Tick (ms) | Cells/sec |"));
        assert!(markdown.contains("| 5.0 | 200.00 |"));
        assert!(markdown.contains("| 7.5 | 133.33 |"));
    }
}
