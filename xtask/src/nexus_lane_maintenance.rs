//! Read-only inventory of Kura's canonical lane-instance storage namespaces.
//!
//! Configured lane labels cannot determine authenticated active or retired
//! incarnations. Filesystem observations here grant no retirement, recovery or
//! archival authority; Core owns those transitions and their retained evidence.
use eyre::{Context, Result, eyre};
use iroha_config::parameters::actual::LaneConfig;
use norito::{derive::JsonSerialize, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct LaneMaintenanceOptions {
    pub config_path: PathBuf,
    pub json_output: PathBuf,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct LaneMaintenanceReport {
    pub store_root: String,
    pub configured_lanes: Vec<ConfiguredLane>,
    pub observed_instances: Vec<InstanceStorageEntry>,
    pub unrecognized_entries: Vec<UnrecognizedEntry>,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct ConfiguredLane {
    pub lane_id: u32,
    pub dataspace_id: u64,
    pub alias: String,
    pub slug: String,
}

/// Observed opaque storage key; no authenticated identity tuple is inferred.
#[derive(Debug, Clone, JsonSerialize)]
pub struct InstanceStorageEntry {
    pub storage_key: String,
    pub blocks: Option<PathReport>,
    pub merge_scaffold: Option<PathReport>,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct PathReport {
    pub path: String,
    pub kind: &'static str,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, JsonSerialize)]
pub struct UnrecognizedEntry {
    pub path: String,
    pub reason: &'static str,
}

pub fn run(options: LaneMaintenanceOptions) -> Result<LaneMaintenanceReport> {
    let cfg = super::load_actual_config(&options.config_path)?;
    let store_root = cfg.kura.store_dir.resolve_relative_path();
    let report = inspect_lanes(&store_root, &cfg.nexus.lane_config)?;
    let rendered_value = json::to_value(&report)?;
    let rendered = format!("{}\n", json::to_string_pretty(&rendered_value)?);
    if options.json_output == Path::new("-") {
        print!("{rendered}");
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

fn inspect_lanes(store_root: &Path, lanes: &LaneConfig) -> Result<LaneMaintenanceReport> {
    let configured_lanes = lanes
        .entries()
        .iter()
        .map(|entry| ConfiguredLane {
            lane_id: entry.lane_id.as_u32(),
            dataspace_id: entry.dataspace_id.as_u64(),
            alias: entry.alias.clone(),
            slug: entry.slug.clone(),
        })
        .collect();
    let mut instances = BTreeMap::new();
    let mut unrecognized_entries = Vec::new();
    if directory_present(store_root)? {
        // These are the current Core-owned namespace roots, not paths derived
        // from config aliases. Only observed canonical opaque keys are joined.
        for (namespace, blocks) in [("blocks", true), ("merge_ledger", false)] {
            let parent = store_root.join(namespace);
            if directory_present(&parent)? {
                inspect_namespace(
                    &parent.join("instances"),
                    blocks,
                    &mut instances,
                    &mut unrecognized_entries,
                )?;
            }
        }
    }
    unrecognized_entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(LaneMaintenanceReport {
        store_root: store_root.display().to_string(),
        configured_lanes,
        observed_instances: instances.into_values().collect(),
        unrecognized_entries,
    })
}

fn directory_present(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(true),
        Ok(_) => Err(eyre!("expected a real directory at {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).wrap_err_with(|| format!("failed to inspect {}", path.display())),
    }
}

fn storage_key(name: &str, blocks: bool) -> Option<&str> {
    let key = if blocks {
        name
    } else {
        name.strip_suffix(".log")?
    };
    (key.len() == 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(key)
}

fn inspect_namespace(
    root: &Path,
    blocks: bool,
    instances: &mut BTreeMap<String, InstanceStorageEntry>,
    unrecognized: &mut Vec<UnrecognizedEntry>,
) -> Result<()> {
    if !directory_present(root)? {
        return Ok(());
    }
    for entry in
        fs::read_dir(root).wrap_err_with(|| format!("failed to read {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let key = name.to_str().and_then(|name| storage_key(name, blocks));
        let kind = entry.file_type()?;
        let Some(key) = key.filter(|_| {
            if blocks {
                kind.is_dir()
            } else {
                kind.is_file()
            }
        }) else {
            unrecognized.push(UnrecognizedEntry {
                path: path.display().to_string(),
                reason: "non-canonical instance name or entry type",
            });
            continue;
        };
        let observed = PathReport {
            path: path.display().to_string(),
            kind: if blocks {
                "blocks_dir"
            } else {
                "merge_scaffold"
            },
            size_bytes: byte_len(&path)?,
        };
        let instance = instances
            .entry(key.to_owned())
            .or_insert_with(|| InstanceStorageEntry {
                storage_key: key.to_owned(),
                blocks: None,
                merge_scaffold: None,
            });
        if blocks {
            instance.blocks = Some(observed);
        } else {
            instance.merge_scaffold = Some(observed);
        }
    }
    Ok(())
}

fn byte_len(path: &Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if metadata.is_dir() {
        let mut total = 0u64;
        for entry in fs::read_dir(path)? {
            total = total
                .checked_add(byte_len(&entry?.path())?)
                .ok_or_else(|| eyre!("storage size overflow at {}", path.display()))?;
        }
        return Ok(total);
    }
    Err(eyre!("unsupported storage entry at {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::kura::LaneStorageIdentity;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        nexus::{LaneCatalog, LaneConfig as LaneMetadata},
    };
    use iroha_model_base::topology::{DataSpaceId, LaneId};
    use std::num::NonZeroU32;
    use tempfile::tempdir;

    fn lane_cfg() -> LaneConfig {
        let catalog = LaneCatalog::new(
            NonZeroU32::new(3).unwrap(),
            vec![
                LaneMetadata::default(),
                LaneMetadata {
                    id: LaneId::from(1),
                    alias: "Alpha".to_owned(),
                    ..LaneMetadata::default()
                },
                LaneMetadata {
                    id: LaneId::from(2),
                    alias: "Beta".to_owned(),
                    ..LaneMetadata::default()
                },
            ],
        )
        .unwrap();
        LaneConfig::from_catalog(&catalog)
    }

    fn identity(incarnation: &[u8]) -> LaneStorageIdentity {
        LaneStorageIdentity::new(
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"inventory-network",
            ))),
            LaneId::from(1),
            DataSpaceId::new(0),
            Hash::new(incarnation),
            3,
        )
    }

    #[test]
    fn reports_configured_labels_and_canonical_instances_without_retirement_classification() {
        let temp = tempdir().unwrap();
        let store = temp.path();
        let first = identity(b"first");
        let second = identity(b"second");
        fs::create_dir_all(first.blocks_dir(store)).unwrap();
        fs::create_dir_all(second.blocks_dir(store)).unwrap();
        let merge = first.merge_log_path(store);
        fs::create_dir_all(merge.parent().unwrap()).unwrap();
        fs::write(&merge, b"").unwrap();
        fs::write(first.blocks_dir(store).join("retained.norito"), b"evidence").unwrap();
        let report = inspect_lanes(store, &lane_cfg()).unwrap();
        assert_eq!(report.configured_lanes.len(), 3);
        assert_eq!(report.observed_instances.len(), 2);
        assert!(report.unrecognized_entries.is_empty());
        assert_eq!(report.store_root, store.display().to_string());
        let first_entry = report
            .observed_instances
            .iter()
            .find(|entry| entry.merge_scaffold.is_some())
            .unwrap();
        assert_eq!(
            first_entry.blocks.as_ref().unwrap().path,
            first.blocks_dir(store).display().to_string()
        );
        assert_eq!(first_entry.blocks.as_ref().unwrap().size_bytes, 8);
        assert_eq!(
            first_entry.merge_scaffold.as_ref().unwrap().path,
            merge.display().to_string()
        );
        assert_eq!(first_entry.merge_scaffold.as_ref().unwrap().size_bytes, 0);
        let json = json::to_value(&report).unwrap();
        for retired_field in ["active", "retired", "compacted"] {
            assert!(json.get(retired_field).is_none());
        }
    }

    #[test]
    fn survey_preserves_every_observed_instance_and_creates_no_archive() {
        let temp = tempdir().unwrap();
        let store = temp.path();
        let instance = identity(b"unknown-to-config");
        let blocks = instance.blocks_dir(store);
        let merge = instance.merge_log_path(store);
        fs::create_dir_all(&blocks).unwrap();
        fs::create_dir_all(merge.parent().unwrap()).unwrap();
        fs::write(blocks.join("retained.norito"), b"do not move").unwrap();
        fs::write(&merge, b"").unwrap();
        let first =
            inspect_lanes(store, &LaneConfig::from_catalog(&LaneCatalog::default())).unwrap();
        let second = inspect_lanes(store, &lane_cfg()).unwrap();
        assert_eq!(
            json::to_value(&first.observed_instances).unwrap(),
            json::to_value(&second.observed_instances).unwrap()
        );
        assert_eq!(
            fs::read(blocks.join("retained.norito")).unwrap(),
            b"do not move"
        );
        assert!(merge.is_file());
        assert!(!store.join("retired").exists());
    }

    #[test]
    fn instance_names_require_exact_canonical_lowercase_digest_and_log_suffix() {
        let key = "abcdef0123456789".repeat(4);
        assert_eq!(storage_key(&key, true), Some(key.as_str()));
        let log = format!("{key}.log");
        assert_eq!(storage_key(&log, false), Some(key.as_str()));
        for invalid in [
            key.to_uppercase(),
            key[..63].to_owned(),
            format!("{key}0"),
            format!("{key}.log"),
            "../outside".to_owned(),
        ] {
            assert!(storage_key(&invalid, true).is_none());
        }
        assert!(storage_key(&key, false).is_none());
        assert!(storage_key(&format!("{key}.LOG"), false).is_none());
    }

    #[test]
    fn unrecognized_entries_remain_reported_and_untouched() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("blocks/instances");
        fs::create_dir_all(&root).unwrap();
        let unknown = root.join("unidentified");
        fs::write(&unknown, b"retain me").unwrap();
        let report = inspect_lanes(temp.path(), &lane_cfg()).unwrap();
        assert!(report.observed_instances.is_empty());
        assert_eq!(report.unrecognized_entries.len(), 1);
        assert_eq!(
            report.unrecognized_entries[0].path,
            unknown.display().to_string()
        );
        assert_eq!(fs::read(unknown).unwrap(), b"retain me");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_namespace_and_nested_payload_are_not_traversed() {
        let temp = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("blocks")).unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("blocks/instances")).unwrap();
        assert!(inspect_lanes(temp.path(), &lane_cfg()).is_err());
        fs::remove_file(temp.path().join("blocks/instances")).unwrap();
        let blocks = identity(b"nested").blocks_dir(temp.path());
        fs::create_dir_all(&blocks).unwrap();
        std::os::unix::fs::symlink(outside.path(), blocks.join("external")).unwrap();
        assert!(inspect_lanes(temp.path(), &lane_cfg()).is_err());
        assert!(outside.path().read_dir().unwrap().next().is_none());
    }
}
