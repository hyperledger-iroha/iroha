//! Nexus lane storage maintenance helpers.
//!
//! Surveys Kura lane segments and optionally archives retired lanes so auditors
//! can keep disk usage and evidence bundles tidy.

use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use eyre::{Context, Result};
use iroha_config::parameters::actual::LaneConfig;
use norito::{derive::JsonSerialize, json};

#[derive(Debug, Clone)]
pub struct LaneMaintenanceOptions {
    pub config_path: PathBuf,
    pub json_output: PathBuf,
    pub compact_retired: bool,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct LaneMaintenanceReport {
    pub store_root: String,
    pub active: Vec<LaneStorageEntry>,
    pub retired: Vec<RetiredSegment>,
    pub compacted: Vec<CompactionAction>,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct LaneStorageEntry {
    pub lane_id: u32,
    pub dataspace_id: u64,
    pub alias: String,
    pub slug: String,
    pub blocks: PathReport,
    pub merge_log: PathReport,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct PathReport {
    pub path: String,
    pub exists: bool,
    pub kind: &'static str,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct RetiredSegment {
    pub path: String,
    pub kind: &'static str,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct CompactionAction {
    pub from: String,
    pub to: String,
}

pub fn run(options: LaneMaintenanceOptions) -> Result<LaneMaintenanceReport> {
    let cfg = super::load_actual_config(&options.config_path)?;
    let store_root = cfg.kura.store_dir.resolve_relative_path();
    let lane_cfg = cfg.nexus.lane_config.clone();
    let report = inspect_lanes(&store_root, &lane_cfg, options.compact_retired)?;

    let rendered_value = json::to_value(&report)?;
    let rendered = format!("{}\n", json::to_string_pretty(&rendered_value)?);
    if options.json_output == Path::new("-") {
        println!("{rendered}");
    } else {
        if let Some(parent) = options.json_output.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!("failed to create parent directory {}", parent.display())
            })?;
        }
        fs::write(&options.json_output, rendered).wrap_err_with(|| {
            format!(
                "failed to write lane maintenance report to {}",
                options.json_output.display()
            )
        })?;
    }

    Ok(report)
}

fn inspect_lanes(
    store_root: &Path,
    lanes: &LaneConfig,
    compact_retired: bool,
) -> Result<LaneMaintenanceReport> {
    let blocks_root = store_root.join("blocks");
    let merge_root = store_root.join("merge_ledger");

    let mut active_entries = Vec::new();
    let mut active_blocks = BTreeSet::new();
    let mut active_merges = BTreeSet::new();

    for entry in lanes.entries() {
        active_blocks.insert(entry.kura_segment.clone());
        active_merges.insert(format!("{}.log", entry.merge_segment));
        active_entries.push(LaneStorageEntry {
            lane_id: entry.lane_id.as_u32(),
            dataspace_id: entry.dataspace_id.as_u64(),
            alias: entry.alias.clone(),
            slug: entry.slug.clone(),
            blocks: PathReport::from_path(entry.blocks_dir(store_root), "blocks_dir"),
            merge_log: PathReport::from_path(entry.merge_log_path(store_root), "merge_log"),
        });
    }

    let mut retired = Vec::new();
    retired.extend(retired_blocks(&blocks_root, &active_blocks)?);
    retired.extend(retired_merges(&merge_root, &active_merges)?);

    let mut compacted = Vec::new();
    if compact_retired && !retired.is_empty() {
        compacted.extend(compact_retired_segments(
            store_root,
            retired.iter().map(|seg| seg.path.clone()),
        )?);
    }

    Ok(LaneMaintenanceReport {
        store_root: store_root.display().to_string(),
        active: active_entries,
        retired,
        compacted,
    })
}

fn retired_blocks(root: &Path, active: &BTreeSet<String>) -> Result<Vec<RetiredSegment>> {
    let mut retired = Vec::new();
    if !root.exists() {
        return Ok(retired);
    }

    for entry in fs::read_dir(root).wrap_err("failed to read Kura blocks directory")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("lane_") || active.contains(&name) {
            continue;
        }
        let path = entry.path();
        retired.push(RetiredSegment {
            path: path.display().to_string(),
            kind: "blocks_dir",
            size_bytes: byte_len(&path)?,
        });
    }
    Ok(retired)
}

fn retired_merges(root: &Path, active: &BTreeSet<String>) -> Result<Vec<RetiredSegment>> {
    let mut retired = Vec::new();
    if !root.exists() {
        return Ok(retired);
    }

    for entry in fs::read_dir(root).wrap_err("failed to read Kura merge_ledger directory")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("lane_") || active.contains(&name) {
            continue;
        }
        let path = entry.path();
        retired.push(RetiredSegment {
            path: path.display().to_string(),
            kind: "merge_log",
            size_bytes: byte_len(&path)?,
        });
    }
    Ok(retired)
}

fn compact_retired_segments(
    store_root: &Path,
    retired_paths: impl IntoIterator<Item = String>,
) -> Result<Vec<CompactionAction>> {
    let mut actions = Vec::new();
    let retired_root = store_root.join("retired");
    let retired_blocks = retired_root.join("blocks");
    let retired_merges = retired_root.join("merge_ledger");
    fs::create_dir_all(&retired_blocks)
        .wrap_err_with(|| format!("failed to create {}", retired_blocks.display()))?;
    fs::create_dir_all(&retired_merges)
        .wrap_err_with(|| format!("failed to create {}", retired_merges.display()))?;

    for raw_path in retired_paths {
        let source = PathBuf::from(&raw_path);
        let file_name = source
            .file_name()
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| OsString::from("retired_lane"));
        let target_root = if raw_path.ends_with(".log") {
            &retired_merges
        } else {
            &retired_blocks
        };
        let dest = unique_archive_path(target_root, &file_name);
        fs::rename(&source, &dest).wrap_err_with(|| {
            format!(
                "failed to archive retired lane segment from {} to {}",
                source.display(),
                dest.display()
            )
        })?;
        actions.push(CompactionAction {
            from: source.display().to_string(),
            to: dest.display().to_string(),
        });
    }

    Ok(actions)
}

fn unique_archive_path(root: &Path, file_name: &OsString) -> PathBuf {
    let mut candidate = root.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let base = file_name.to_string_lossy();
    let (stem, ext) = split_stem_ext(&base);
    let mut counter = 1u32;
    loop {
        let mut name = OsString::from(format!("{stem}_{counter}"));
        if let Some(ext) = ext {
            name.push(ext);
        }
        candidate = root.join(&name);
        if !candidate.exists() {
            return candidate;
        }
        counter = counter.saturating_add(1);
    }
}

fn split_stem_ext(name: &str) -> (&str, Option<&str>) {
    if let Some(idx) = name.rfind('.') {
        let (stem, ext) = name.split_at(idx);
        (stem, Some(ext))
    } else {
        (name, None)
    }
}

impl PathReport {
    fn from_path(path: PathBuf, kind: &'static str) -> Self {
        let exists = path.exists();
        let size_bytes = byte_len(&path).unwrap_or(0);
        Self {
            path: path.display().to_string(),
            exists,
            kind,
            size_bytes,
        }
    }
}

fn byte_len(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let meta = fs::metadata(path)?;
    if meta.is_file() {
        return Ok(meta.len());
    }
    if meta.is_dir() {
        let mut total = 0u64;
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            total = total.saturating_add(byte_len(&entry.path())?);
        }
        return Ok(total);
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use iroha_data_model::nexus::{LaneCatalog, LaneConfig as LaneMetadata, LaneId};
    use tempfile::tempdir;

    use super::*;

    fn lane_cfg() -> LaneConfig {
        let lane_count = NonZeroU32::new(3).expect("non-zero lane count");
        let lane0 = LaneMetadata::default();
        let lane1 = LaneMetadata {
            id: LaneId::from(1),
            alias: "Alpha".to_string(),
            ..LaneMetadata::default()
        };
        let lane2 = LaneMetadata {
            id: LaneId::from(2),
            alias: "Beta".to_string(),
            ..LaneMetadata::default()
        };
        let catalog = LaneCatalog::new(lane_count, vec![lane0, lane1, lane2]).expect("catalog");
        LaneConfig::from_catalog(&catalog)
    }

    #[test]
    fn reports_active_and_retired_segments() {
        let lanes = lane_cfg();
        let temp = tempdir().expect("tmpdir");
        let store = temp.path();
        let lane0 = lanes.entry(LaneId::from(0)).expect("lane 0");
        let lane1 = lanes.entry(LaneId::from(1)).expect("lane 1");

        fs::create_dir_all(lane0.blocks_dir(store)).expect("lane0 blocks");
        fs::create_dir_all(lane1.blocks_dir(store)).expect("lane1 blocks");
        let merge0 = lane0.merge_log_path(store);
        fs::create_dir_all(merge0.parent().unwrap()).expect("merge dir");
        fs::write(&merge0, b"merge0").expect("merge log");

        let retired_blocks = store.join("blocks").join("lane_999_old");
        fs::create_dir_all(&retired_blocks).expect("retired blocks");
        let retired_merge = store.join("merge_ledger").join("lane_999_old_merge.log");
        fs::create_dir_all(retired_merge.parent().unwrap()).expect("retired merge dir");
        fs::write(&retired_merge, b"old").expect("retired merge log");

        let report = inspect_lanes(store, &lanes, false).expect("report");
        assert_eq!(report.active.len(), 3);
        assert_eq!(report.retired.len(), 2);
        assert!(report.compacted.is_empty());
        assert_eq!(report.store_root, store.display().to_string());
    }

    #[test]
    fn compacts_retired_segments_into_archive() {
        let lanes = lane_cfg();
        let temp = tempdir().expect("tmpdir");
        let store = temp.path();
        let retired_blocks = store.join("blocks").join("lane_010_old");
        fs::create_dir_all(&retired_blocks).expect("retired");
        let retired_merge = store.join("merge_ledger").join("lane_010_old_merge.log");
        fs::create_dir_all(retired_merge.parent().unwrap()).expect("merge dir");
        fs::write(&retired_merge, b"old").expect("retired merge log");

        let report = inspect_lanes(store, &lanes, true).expect("report");
        assert_eq!(report.retired.len(), 2);
        assert_eq!(report.compacted.len(), 2);
        let retired_root = store.join("retired");
        assert!(
            retired_root
                .join("blocks")
                .read_dir()
                .unwrap()
                .next()
                .is_some()
        );
        assert!(
            retired_root
                .join("merge_ledger")
                .read_dir()
                .unwrap()
                .next()
                .is_some()
        );
    }
}
