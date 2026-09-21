//! Read-only physical inventory of Kura storage and declared lane metadata.
//!
//! Configured aliases do not identify lane instances or authorize retirement.
//! This survey never opens Kura, follows symbolic links, or moves storage files.
use eyre::{Context, Result, eyre};
use iroha_config::parameters::actual::LaneConfig;
use iroha_core::kura::Kura;
use norito::{derive::JsonSerialize, json};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone)]
pub struct LaneMaintenanceOptions {
    pub config_path: PathBuf,
}
#[derive(Debug, Clone, JsonSerialize)]
pub struct LaneMaintenanceReport {
    pub store_root: String,
    pub declared_lanes: Vec<DeclaredLane>,
    pub canonical_blocks: PathReport,
    pub canonical_merge_log: PathReport,
    pub instance_blocks: Vec<PathReport>,
    pub instance_merge_scaffolds: Vec<PathReport>,
    pub unclassified_entries: Vec<PathReport>,
}
#[derive(Debug, Clone, JsonSerialize)]
pub struct DeclaredLane {
    pub lane_id: u32,
    pub dataspace_id: u64,
    pub alias: String,
    pub slug: String,
}
#[derive(Debug, Clone, JsonSerialize)]
pub struct PathReport {
    pub path: String,
    pub exists: bool,
    pub kind: &'static str,
    pub size_bytes: u64,
}
pub fn run(options: LaneMaintenanceOptions) -> Result<LaneMaintenanceReport> {
    let cfg = super::load_actual_config(&options.config_path)?;
    let store_root = cfg.kura.store_dir.resolve_relative_path();
    let report = inspect_lanes(&store_root, &cfg.nexus.lane_config)?;
    let rendered_value = json::to_value(&report)?;
    println!("{}", json::to_string_pretty(&rendered_value)?);
    Ok(report)
}
fn inspect_lanes(store_root: &Path, lanes: &LaneConfig) -> Result<LaneMaintenanceReport> {
    // Validate the root before inspecting children so a linked store cannot
    // make the otherwise non-following namespace walk traverse another tree.
    directory_present(store_root)?;
    let (canonical_blocks, canonical_merge_log) = Kura::canonical_storage_paths(store_root);
    let blocks_root = store_root.join("blocks");
    let merge_root = store_root.join("merge_ledger");
    let mut unclassified_entries = inventory_entries(&blocks_root, &["canonical", "instances"])?;
    unclassified_entries.extend(inventory_entries(
        &merge_root,
        &["canonical.log", "instances"],
    )?);
    Ok(LaneMaintenanceReport {
        store_root: store_root.display().to_string(),
        declared_lanes: lanes
            .entries()
            .iter()
            .map(|entry| DeclaredLane {
                lane_id: entry.lane_id.as_u32(),
                dataspace_id: entry.dataspace_id.as_u64(),
                alias: entry.alias.clone(),
                slug: entry.slug.clone(),
            })
            .collect(),
        canonical_blocks: PathReport::from_path(canonical_blocks)?,
        canonical_merge_log: PathReport::from_path(canonical_merge_log)?,
        instance_blocks: inventory_entries(&blocks_root.join("instances"), &[])?,
        instance_merge_scaffolds: inventory_entries(&merge_root.join("instances"), &[])?,
        unclassified_entries,
    })
}
fn directory_present(root: &Path) -> Result<bool> {
    let metadata = match fs::symlink_metadata(root) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).wrap_err_with(|| format!("inspect {}", root.display())),
    };
    if !metadata.is_dir() {
        return Err(eyre!(
            "inventory namespace {} must be a directory, not a symbolic link or other file",
            root.display()
        ));
    }
    Ok(true)
}
fn inventory_entries(root: &Path, excluded: &[&str]) -> Result<Vec<PathReport>> {
    if !directory_present(root)? {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(root).wrap_err_with(|| format!("read {}", root.display()))? {
        let entry = entry?;
        if excluded.iter().any(|name| entry.file_name() == *name) {
            continue;
        }
        entries.push(PathReport::from_path(entry.path())?);
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}
impl PathReport {
    fn from_path(path: PathBuf) -> Result<Self> {
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => {
                return Err(error).wrap_err_with(|| format!("inspect {}", path.display()));
            }
        };
        let kind = match metadata.as_ref() {
            None => "missing",
            Some(metadata) if metadata.file_type().is_symlink() => "symlink",
            Some(metadata) if metadata.is_dir() => "directory",
            Some(metadata) if metadata.is_file() => "file",
            Some(_) => "other",
        };
        Ok(Self {
            path: path.display().to_string(),
            exists: metadata.is_some(),
            kind,
            size_bytes: if metadata.is_some() {
                byte_len(&path)?
            } else {
                0
            },
        })
    }
}
fn byte_len(path: &Path) -> Result<u64> {
    let mut total = 0_u64;
    let mut pending = vec![path.to_owned()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            for entry in fs::read_dir(&path)? {
                pending.push(entry?.path());
            }
        } else if metadata.is_file() || metadata.file_type().is_symlink() {
            total = total
                .checked_add(metadata.len())
                .ok_or_else(|| eyre!("inventory byte count overflow at {}", path.display()))?;
        }
    }
    Ok(total)
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

    fn lane_cfg(alias: &str) -> LaneConfig {
        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![
                LaneMetadata::default(),
                LaneMetadata {
                    id: LaneId::from(1),
                    alias: alias.to_owned(),
                    ..LaneMetadata::default()
                },
            ],
        )
        .expect("catalog");
        LaneConfig::from_catalog(&catalog)
    }

    #[test]
    fn inventories_canonical_and_exact_instance_paths_without_alias_authority() {
        let temp = tempdir().expect("tmpdir");
        let store = temp.path();
        let (canonical_blocks, canonical_merge) = Kura::canonical_storage_paths(store);
        fs::create_dir_all(&canonical_blocks).expect("canonical blocks");
        fs::write(canonical_blocks.join("blocks.data"), b"canonical").expect("block data");
        fs::create_dir_all(canonical_merge.parent().unwrap()).expect("canonical merge parent");
        fs::write(&canonical_merge, b"merge").expect("canonical merge");
        // A locator constructs test paths only; inventory does not infer active
        // ownership from these values or from their filename hash.
        let identity = LaneStorageIdentity::new(
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"network"))),
            LaneId::from(1),
            DataSpaceId::new(0),
            Hash::new(b"instance"),
            7,
        );
        let instance_blocks = identity.blocks_dir(store);
        let instance_merge = identity.merge_log_path(store);
        fs::create_dir_all(&instance_blocks).expect("instance blocks");
        fs::write(instance_blocks.join("receipt.norito"), b"receipt").expect("receipt");
        fs::create_dir_all(instance_merge.parent().unwrap()).expect("instance merge parent");
        fs::write(&instance_merge, []).expect("empty geometry scaffold");
        let unclassified_blocks = store.join("blocks/lane_999_old");
        let unclassified_merge = store.join("merge_ledger/lane_999_old_merge.log");
        fs::create_dir_all(&unclassified_blocks).expect("unclassified blocks");
        fs::write(&unclassified_merge, b"unknown").expect("unclassified merge");

        let report = inspect_lanes(store, &lane_cfg("Alpha")).expect("inventory");
        assert_eq!(report.declared_lanes.len(), 2);
        assert_eq!(
            report.canonical_blocks.path,
            canonical_blocks.display().to_string()
        );
        assert_eq!(report.canonical_blocks.size_bytes, 9);
        assert_eq!(
            report.canonical_merge_log.path,
            canonical_merge.display().to_string()
        );
        assert_eq!(report.canonical_merge_log.size_bytes, 5);
        assert_eq!(report.instance_blocks.len(), 1);
        assert_eq!(
            report.instance_blocks[0].path,
            instance_blocks.display().to_string()
        );
        assert_eq!(report.instance_blocks[0].size_bytes, 7);
        assert_eq!(report.instance_merge_scaffolds.len(), 1);
        assert_eq!(
            report.instance_merge_scaffolds[0].path,
            instance_merge.display().to_string()
        );
        assert_eq!(report.unclassified_entries.len(), 2);
        assert!(unclassified_blocks.is_dir());
        assert_eq!(fs::read(&unclassified_merge).unwrap(), b"unknown");
        assert!(!store.join("retired").exists());

        let renamed = inspect_lanes(store, &lane_cfg("Renamed")).expect("renamed catalog");
        assert_eq!(renamed.declared_lanes[1].alias, "Renamed");
        assert_eq!(
            renamed.instance_blocks[0].path,
            report.instance_blocks[0].path
        );
        assert_eq!(
            renamed.instance_merge_scaffolds[0].path,
            report.instance_merge_scaffolds[0].path
        );
        let json = json::to_value(&report).expect("report JSON");
        for unsupported in ["active", "retired", "compacted"] {
            assert!(json.get(unsupported).is_none());
        }
    }

    #[test]
    fn run_reads_existing_config_and_storage_without_writing_files() {
        let temp = tempdir().expect("tmpdir");
        let store = temp.path().join("store");
        let (blocks, merge) = Kura::canonical_storage_paths(&store);
        fs::create_dir_all(&blocks).expect("canonical blocks");
        fs::create_dir_all(merge.parent().unwrap()).expect("merge directory");
        fs::write(blocks.join("blocks.data"), b"original block data").expect("block data");
        fs::write(&merge, b"original merge data").expect("merge data");
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../crates/iroha_config/tests/fixtures/base.toml")
            .canonicalize()
            .expect("existing configuration fixture");
        let config_path = temp.path().join("config.toml");
        fs::write(
            &config_path,
            format!("extends = {base:?}\n[kura]\nstore_dir = {store:?}\n"),
        )
        .expect("inventory configuration");
        let image = || {
            walkdir::WalkDir::new(temp.path())
                .into_iter()
                .map(|entry| {
                    let entry = entry.expect("inspect test directory");
                    let bytes = entry
                        .file_type()
                        .is_file()
                        .then(|| fs::read(entry.path()).expect("read original fixture bytes"));
                    (entry.path().to_owned(), bytes)
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        };
        let before = image();
        let report = run(LaneMaintenanceOptions { config_path }).expect("read-only report");
        assert_eq!(report.canonical_blocks.path, blocks.display().to_string());
        assert_eq!(report.canonical_blocks.size_bytes, 19);
        assert_eq!(report.canonical_merge_log.size_bytes, 19);
        assert_eq!(image(), before, "no report, archive, or storage mutation");
    }

    #[test]
    fn absent_store_is_reported_without_creating_storage() {
        let temp = tempdir().expect("tmpdir");
        let store = temp.path().join("absent");
        let report = inspect_lanes(&store, &lane_cfg("Alpha")).expect("missing storage");
        assert!(!report.canonical_blocks.exists);
        assert_eq!(report.canonical_blocks.kind, "missing");
        assert!(!report.canonical_merge_log.exists);
        assert!(report.instance_blocks.is_empty());
        assert!(report.instance_merge_scaffolds.is_empty());
        assert!(report.unclassified_entries.is_empty());
        assert!(!store.exists());
    }

    #[cfg(unix)]
    #[test]
    fn inventory_reports_symlinks_without_following_them_and_rejects_linked_namespaces() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tmpdir");
        let store = temp.path();
        let instances = store.join("blocks/instances");
        fs::create_dir_all(&instances).expect("instances");
        let cycle = instances.join("cycle");
        symlink(&instances, &cycle).expect("cycle symlink");
        let broken = instances.join("broken");
        symlink("missing", &broken).expect("broken symlink");
        let report = inspect_lanes(store, &lane_cfg("Alpha")).expect("symlink inventory");
        assert_eq!(report.instance_blocks.len(), 2);
        assert!(report.instance_blocks[0].path < report.instance_blocks[1].path);
        for entry in &report.instance_blocks {
            assert_eq!(entry.kind, "symlink");
            assert!(entry.exists);
            assert_eq!(
                entry.size_bytes,
                fs::symlink_metadata(&entry.path).unwrap().len()
            );
        }
        let merge_root = store.join("merge_ledger");
        fs::create_dir_all(&merge_root).expect("merge namespace");
        symlink(&instances, merge_root.join("instances")).expect("linked namespace");
        let error = inspect_lanes(store, &lane_cfg("Alpha")).expect_err("refuse linked namespace");
        assert!(error.to_string().contains("must be a directory"));
        assert!(cycle.is_symlink());
        assert!(broken.is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn inventory_rejects_a_linked_store_root_and_does_not_follow_nested_links() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tmpdir");
        let outside = tempdir().expect("outside directory");
        fs::write(outside.path().join("retained.norito"), b"outside evidence")
            .expect("outside evidence");
        let linked_store = temp.path().join("linked-store");
        symlink(outside.path(), &linked_store).expect("linked store");
        let error =
            inspect_lanes(&linked_store, &lane_cfg("Alpha")).expect_err("refuse linked store root");
        assert!(error.to_string().contains("must be a directory"));

        let store = temp.path().join("store");
        let (blocks, _) = Kura::canonical_storage_paths(&store);
        fs::create_dir_all(&blocks).expect("canonical blocks");
        fs::write(blocks.join("blocks.data"), b"canonical").expect("block data");
        let linked_payload = blocks.join("external");
        symlink(outside.path(), &linked_payload).expect("linked payload");
        let report = inspect_lanes(&store, &lane_cfg("Alpha")).expect("nested symlink inventory");
        assert_eq!(
            report.canonical_blocks.size_bytes,
            9 + fs::symlink_metadata(&linked_payload).unwrap().len()
        );
        assert_eq!(
            fs::read(outside.path().join("retained.norito")).unwrap(),
            b"outside evidence"
        );
        assert!(linked_payload.is_symlink());
    }
}
