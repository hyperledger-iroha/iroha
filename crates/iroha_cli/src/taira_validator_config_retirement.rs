//! Exact retired-table removal for the root-owned routine Taira updater.
//!
//! Preparation never replaces a live config. Each stopped install is one atomic sibling rename;
//! a retained original permits explicit pre-start recovery after a partial cohort install. A
//! start-intent permanently closes restore authority. No legacy node schema is reintroduced.

use super::*;
#[cfg(any(target_os = "linux", test))]
use crate::taira_config_retirement_evidence::LIMIT;
#[cfg(target_os = "linux")]
use crate::taira_config_retirement_evidence::{
    PREFIX, exact_fields, field, rows, same_renamed_file, stamp_value, text, validate_receipt,
    validate_retirement_records,
};
#[cfg(any(target_os = "linux", test))]
use zeroize::Zeroizing;

#[cfg(target_os = "linux")]
const PREPARED: &str = "config-retirement-prepared.json";
#[cfg(target_os = "linux")]
const INSTALLED: &str = "config-retirement-installed.json";
#[cfg(target_os = "linux")]
const RESTORED: &str = "config-retirement-restored.json";

/// Native configuration retirement without loading client credentials.
#[derive(Debug, clap::Args)]
pub(crate) struct RetireValidatorConfig {
    /// Prepare private siblings, install while stopped, or restore before any startup intent.
    #[arg(long, value_enum)]
    action: Action,
    /// Inherited bounded root-private request descriptor owned by the live updater.
    #[arg(long, value_parser = clap::value_parser!(u32).range(3..=65535))]
    request_fd: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Action {
    Prepare,
    Install,
    Restore,
}

#[cfg(target_os = "linux")]
#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct Request {
    schema: String,
    operation_directory: String,
    owner: maintenance::MaintenanceOwner,
}

impl RetireValidatorConfig {
    /// Execute with inherited updater custody and emit only the retained public receipt.
    pub(crate) fn run_without_client_config(&self, mut output: impl Write) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            ensure!(
                rustix::process::geteuid().as_raw() == 0,
                "config retirement requires root"
            );
            let bytes = crate::client_config::read_inherited_private_file(
                self.request_fd,
                16384,
                "config retirement request",
            )?;
            let request: Request = json::from_slice(&bytes)?;
            ensure!(
                request.schema == format!("{PREFIX}.request.v1"),
                "config retirement request schema differs"
            );
            let request = maintenance::MaintenanceRequest {
                schema: "taira.stopped-owner-maintenance.request.v1".into(),
                operation_directory: request.operation_directory,
                owner: request.owner,
            };
            let receipt = run(self.action, &request)?;
            output.write_all(&json::to_vec(&receipt)?)?;
            output.write_all(b"\n")?;
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = &mut output;
            Err(eyre!("config retirement requires Linux"))
        }
    }
}

/// Remove only the retired table. Every other TOML value is preserved exactly.
#[cfg(any(target_os = "linux", test))]
fn project(bytes: &[u8]) -> Result<(Zeroizing<Vec<u8>>, bool)> {
    ensure!(
        !bytes.is_empty() && bytes.len() as u64 <= LIMIT,
        "config retirement input bound differs"
    );
    let mut table: toml::Table = toml::from_str(
        std::str::from_utf8(bytes).map_err(|_| eyre!("config retirement input is not UTF-8"))?,
    )
    .map_err(|_| eyre!("config retirement input is not TOML"))?;
    let result = (|| {
        ensure!(
            !table.contains_key("extends") && !table.contains_key("profile"),
            "config retirement forbids inherited configuration"
        );
        let changed = match table.get_mut("zk") {
            Some(value) => {
                let zk = value
                    .as_table_mut()
                    .ok_or_else(|| eyre!("config retirement zk is not a table"))?;
                if let Some(value) = zk.get("halo2") {
                    ensure!(value.is_table(), "retired zk.halo2 is not a table");
                }
                if let Some(value) = zk.remove("halo2") {
                    let mut retired = toml::Table::new();
                    retired.insert("retired".into(), value);
                    crate::soracloud::zeroize_taira_toml_table(&mut retired);
                    true
                } else {
                    false
                }
            }
            None => false,
        };
        let output = if changed {
            let rendered = Zeroizing::new(
                toml::to_string(&table).map_err(|_| eyre!("cannot encode retired config"))?,
            );
            Zeroizing::new(rendered.as_bytes().to_vec())
        } else {
            Zeroizing::new(bytes.to_vec())
        };
        ensure!(
            output.len() as u64 <= LIMIT,
            "config retirement output bound differs"
        );
        Ok((output, changed))
    })();
    crate::soracloud::zeroize_taira_toml_table(&mut table);
    result
}

#[cfg(target_os = "linux")]
fn validate_config(bytes: &[u8], path: &Path) -> Result<()> {
    use iroha_config::node_config::{NodeConfigOptions, NodeFile, open_node_config};
    let table: toml::Table = toml::from_str(std::str::from_utf8(bytes)?)
        .map_err(|_| eyre!("projected config is not TOML"))?;
    // The actual node schema validates the projection; unknown remaining keys stay errors.
    let reader = open_node_config(
        NodeFile::Verified {
            path: path.into(),
            table,
        },
        NodeConfigOptions { sora: true },
    )
    .map_err(|_| eyre!("projected validator config cannot load"))?;
    let (user, _) = reader
        .read()
        .map_err(|_| eyre!("projected validator config does not match current schema"))?;
    user.parse()
        .map_err(|_| eyre!("projected validator config fails runtime validation"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn config_paths(plan: &json::Value, role: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let (source, staged, original) =
        crate::taira_config_retirement_evidence::config_paths(plan, role)?;
    Ok((source.into(), staged.into(), original.into()))
}

#[cfg(target_os = "linux")]
fn stamp(path: &Path) -> Result<json::Value> {
    use std::os::unix::fs::MetadataExt as _;
    require_root_no_symlink_ancestors(path, "retirement config")?;
    let m = fs::symlink_metadata(path)?;
    ensure!(m.is_file(), "retirement config is not a regular file");
    let ns = |s: i64, n: i64| {
        u64::try_from(i128::from(s) * 1_000_000_000 + i128::from(n))
            .map_err(|_| eyre!("config retirement timestamp differs"))
    };
    let value = json::to_value(&vec![
        m.dev(),
        m.ino(),
        u64::from(m.mode()),
        u64::from(m.uid()),
        u64::from(m.gid()),
        m.nlink(),
        m.len(),
        ns(m.mtime(), m.mtime_nsec())?,
        ns(m.ctime(), m.ctime_nsec())?,
    ])?;
    stamp_value(&value)?;
    Ok(value)
}

#[cfg(target_os = "linux")]
fn private_bytes(path: &Path) -> Result<Zeroizing<Vec<u8>>> {
    require_root_no_symlink_ancestors(path, "retirement private config")?;
    let pinned = pin_owner_private_file(path, "retirement private config")?;
    let bytes = Zeroizing::new(read_pinned_bytes(
        path,
        "retirement private config",
        pinned.file,
        &pinned.snapshot,
        LIMIT,
    )?);
    stamp(path)?;
    Ok(bytes)
}

#[cfg(target_os = "linux")]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    require_root_no_symlink_ancestors(path, "retirement output")?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("retirement output lacks parent"))?;
    require_root_directory(parent, false, "retirement output parent")?;
    let before = fs::symlink_metadata(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    require_root_no_symlink_ancestors(path, "retirement output")?;
    let after = fs::symlink_metadata(parent)?;
    ensure!(
        (before.dev(), before.ino(), before.uid(), before.mode())
            == (after.dev(), after.ino(), after.uid(), after.mode()),
        "retirement output parent changed"
    );
    temporary
        .persist_noclobber(path)
        .map_err(|_| eyre!("cannot publish fresh config retirement output"))?;
    File::open(parent)?.sync_all()?;
    ensure!(
        private_bytes(path)?.as_slice() == bytes,
        "retirement output publication differs"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_receipt(directory: &Path, name: &str) -> Result<json::Value> {
    json::from_slice(&maintenance::public_bytes(&directory.join(name), LIMIT)?)
        .map_err(|_| eyre!("config retirement receipt is invalid"))
}

#[cfg(target_os = "linux")]
fn vacant(
    plan: &json::Value,
    scope: &maintenance::MaintenanceScope,
    deadline: Instant,
) -> Result<()> {
    let units = field(plan, "units")?
        .as_array()
        .ok_or_else(|| eyre!("retirement units missing"))?;
    for (row, role) in units.iter().zip(super::super::VALIDATOR_SLUGS) {
        let unit = format!("iroha3d-{role}.service");
        let fragment = Path::new("/etc/systemd/system").join(&unit);
        require_root_no_symlink_ancestors(&fragment, "retirement unit")?;
        let (file, snap) = open_pinned_regular(&fragment, "retirement unit")?;
        ensure!(
            snap.uid == 0 && snap.mode & 0o022 == 0,
            "retirement unit custody differs"
        );
        let bytes = read_pinned_bytes(&fragment, "retirement unit", file, &snap, LIMIT)?;
        ensure!(
            bytes == BASE64.decode(text(row, "before")?)?
                || bytes == BASE64.decode(text(row, "after")?)?,
            "retirement unit differs from plan"
        );
        let loaded = run_host_command(
            SYSTEMCTL,
            &[
                "show",
                "--all",
                "--property=FragmentPath",
                "--property=DropInPaths",
                "--property=NeedDaemonReload",
                &unit,
            ],
            deadline,
        )?;
        let text_loaded = std::str::from_utf8(&loaded)?;
        let props = text_loaded
            .lines()
            .map(|line| {
                line.split_once('=')
                    .ok_or_else(|| eyre!("retirement loaded-unit evidence differs"))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        ensure!(
            props.len() == 3
                && props.get("FragmentPath").copied() == fragment.to_str()
                && props.get("DropInPaths").copied() == Some("")
                && matches!(props.get("NeedDaemonReload").copied(), Some("yes" | "no")),
            "retirement loaded unit differs"
        );
        let evidence = run_host_command(
            SYSTEMCTL,
            &[
                "show",
                "--all",
                "--property=ActiveState",
                "--property=SubState",
                "--property=MainPID",
                "--property=ControlPID",
                "--property=ControlGroup",
                "--property=Job",
                &unit,
            ],
            deadline,
        )?;
        if let Some(group) = validate_vacant_unit_evidence(&evidence, true)? {
            require_root_directory(&group, false, "retirement cgroup")?;
            ensure!(
                rustix::fs::statfs(&group)?.f_type as u64 == 0x6367_7270
                    && fs::read_to_string(group.join("cgroup.events"))?
                        .lines()
                        .filter(|s| s.starts_with("populated "))
                        .collect::<Vec<_>>()
                        == ["populated 0"],
                "retirement cgroup is not empty"
            );
        }
    }
    require_no_live_path_references(&scope.vacant_paths(), deadline)
}

#[cfg(target_os = "linux")]
fn run(action: Action, request: &maintenance::MaintenanceRequest) -> Result<json::Value> {
    let directory = Path::new(&request.operation_directory);
    let deadline = Instant::now() + Duration::from_secs(120);
    let plan_bytes = maintenance::public_bytes(&directory.join("intent.json"), 8 * LIMIT)?;
    let plan: json::Value = json::from_slice(&plan_bytes)?;
    let scope = maintenance::maintenance_scope(request, &plan)?;
    ensure!(
        std::env::current_exe()? == scope.candidate_cli
            && crate::compiled_build_identity()?.release_source_commit()? == text(&plan, "commit")?,
        "retirement executable differs from planned candidate"
    );
    let previous_daemons = maintenance::PreviousDaemonsGuard::admit(&scope, deadline)?;
    let check = || -> Result<()> {
        maintenance::verify_owner(request, &scope, deadline, action == Action::Restore)?;
        ensure!(
            maintenance::public_bytes(&directory.join("intent.json"), 8 * LIMIT)? == plan_bytes,
            "retirement plan changed"
        );
        previous_daemons.revalidate(deadline)?;
        for role in super::super::VALIDATOR_SLUGS {
            let (source, _, _) = config_paths(&plan, role)?;
            let target = source
                .parent()
                .and_then(Path::parent)
                .ok_or_else(|| eyre!("retirement target lacks release"))?;
            let selector = PathBuf::from(format!("/srv/taira/{role}/current"));
            require_root_no_symlink_ancestors(&selector, "retirement selector")?;
            ensure!(
                fs::symlink_metadata(&selector)?.uid() == 0 && fs::read_link(&selector)? == target,
                "retirement config selector changed"
            );
        }
        if action != Action::Prepare {
            vacant(&plan, &scope, deadline)?;
        }
        previous_daemons.revalidate(deadline)
    };
    check()?;
    if action == Action::Install {
        let checkpoints = read_receipt(directory, "checkpoint-stopped.json")?;
        let checkpoints = checkpoints
            .as_array()
            .ok_or_else(|| eyre!("retirement stopped checkpoints missing"))?;
        ensure!(
            checkpoints.len() == 4,
            "retirement requires all stopped checkpoints"
        );
        for (row, role) in checkpoints.iter().zip(super::super::VALIDATOR_SLUGS) {
            ensure!(
                text(row, "role")? == role
                    && field(row, "cohort_stopped")?.as_bool() == Some(true)
                    && field(row, "checkpoint_height")?
                        .as_u64()
                        .is_some_and(|height| height > 0),
                "retirement checkpoint ordering or stopped proof differs"
            );
        }
        let maintenance = read_receipt(directory, "stopped-owner-maintenance-result.json")?;
        ensure!(
            text(&maintenance, "schema")? == "taira.stopped-owner-maintenance.result.v1"
                && text(&maintenance, "operation")? == scope.operation
                && field(&maintenance, "all_four_stopped_owners_clean")?.as_bool() == Some(true),
            "retirement stopped-owner maintenance is incomplete"
        );
    }
    let name = match action {
        Action::Prepare => PREPARED,
        Action::Install => INSTALLED,
        Action::Restore => RESTORED,
    };
    // A receipt is immutable. An interrupted mutation can be continued only using the same
    // preparation and exact retained inode/content identities, never reconstructed authority.
    ensure!(
        !directory.join(RESTORED).try_exists()?,
        "retirement already restored; fresh update required"
    );
    if action != Action::Restore {
        ensure!(
            !directory.join(name).try_exists()?,
            "retirement action already completed"
        );
    }
    let mut result_rows = Vec::new();
    if action == Action::Prepare {
        for role in super::super::VALIDATOR_SLUGS {
            check()?;
            let (path, staged, original) = config_paths(&plan, role)?;
            let before = stamp(&path)?;
            let input = private_bytes(&path)?;
            let (output, changed) = project(&input)?;
            validate_config(&output, &path)?;
            ensure!(
                stamp(&path)? == before,
                "retirement source changed during preparation"
            );
            write_private(&original, &input)?;
            write_private(&staged, &output)?;
            ensure!(
                stamp(&path)? == before,
                "retirement source changed during publication"
            );
            result_rows.push(norito::json!({"role":role, "source_path":(path.to_string_lossy().into_owned()), "staged_path":(staged.to_string_lossy().into_owned()), "original_path":(original.to_string_lossy().into_owned()), "changed":changed, "source_sha256":(sha256_hex(&input)), "output_sha256":(sha256_hex(&output)), "before_stamp":before, "staged_stamp":(stamp(&staged)?)}));
        }
    } else {
        let prepared = read_receipt(directory, PREPARED)?;
        validate_receipt(&prepared, &plan, "prepared")?;
        // Admit every slot before the first replacement. Each replacement rechecks custody.
        for row in rows(&prepared)? {
            let restored = if action == Action::Restore {
                restore_intent(directory, row, &scope.operation)?
            } else {
                None
            };
            validate_private_relation(row, false, restored.as_ref())?;
        }
        for row in rows(&prepared)? {
            check()?;
            let restored = if action == Action::Restore {
                restore_intent(directory, row, &scope.operation)?
            } else {
                None
            };
            validate_private_relation(row, false, restored.as_ref())?;
            let source = Path::new(text(row, "source_path")?);
            let staged = Path::new(text(row, "staged_path")?);
            let current = stamp(source)?;
            let old = current == *field(row, "before_stamp")?;
            let installed = same_renamed_file(&current, field(row, "staged_stamp")?)?;
            let already_restored = match restored.as_ref() {
                Some(stamp) => same_renamed_file(&current, stamp)?,
                None => false,
            };
            ensure!(
                old || installed || already_restored,
                "retirement target has unknown custody"
            );
            if action == Action::Install && field(row, "changed")?.as_bool() == Some(true) && old {
                ensure!(
                    stamp(staged)? == *field(row, "staged_stamp")?,
                    "retirement staged custody changed"
                );
                check()?;
                ensure!(
                    stamp(source)? == current && stamp(staged)? == *field(row, "staged_stamp")?,
                    "retirement custody changed before install"
                );
                fs::rename(staged, source)?;
                File::open(source.parent().unwrap())?.sync_all()?;
            } else if action == Action::Restore && installed {
                let original = private_bytes(Path::new(text(row, "original_path")?))?;
                let restore = source.with_file_name(format!(
                    ".{}.{}.retirement-restore",
                    source.file_name().unwrap().to_string_lossy(),
                    scope.operation
                ));
                if let Some(expected) = restored.as_ref() {
                    ensure!(
                        stamp(&restore)? == *expected
                            && private_bytes(&restore)?.as_slice() == original.as_slice(),
                        "retirement restore staging changed"
                    );
                } else {
                    write_private(&restore, &original)?;
                    let intent = norito::json!({"schema":(format!("{PREFIX}.restore-intent.v1")), "operation":(scope.operation.clone()), "role":(text(row,"role")?), "source_path":(text(row,"source_path")?), "source_sha256":(text(row,"source_sha256")?), "restore_path":(restore.to_string_lossy().into_owned()), "restore_stamp":(stamp(&restore)?)});
                    write_private(
                        &directory.join(format!(
                            "config-retirement-restore-{}.json",
                            text(row, "role")?
                        )),
                        &json::to_vec(&intent)?,
                    )?;
                }
                check()?;
                ensure!(
                    stamp(source)? == current,
                    "retirement target changed before restore"
                );
                fs::rename(&restore, source)?;
                File::open(source.parent().unwrap())?.sync_all()?;
            }
            check()?;
            let mut result = row.clone();
            result.as_object_mut().unwrap().insert(
                if action == Action::Install {
                    "installed_stamp"
                } else {
                    "restored_stamp"
                }
                .into(),
                stamp(source)?,
            );
            result_rows.push(result);
        }
    }
    check()?;
    let kind = match action {
        Action::Prepare => "prepared",
        Action::Install => "installed",
        Action::Restore => "restored",
    };
    let receipt = norito::json!({"schema":(format!("{PREFIX}.{kind}.v1")), "operation":(scope.operation.clone()), "source_commit":(text(&plan,"commit")?), "network_id":(text(&plan,"network_id")?), "rows":result_rows});
    validate_receipt(&receipt, &plan, kind)?;
    write_private(&directory.join(name), &json::to_vec(&receipt)?)?;
    Ok(receipt)
}

/// A durable per-slot restore intent binds the replacement inode before the rename.
#[cfg(target_os = "linux")]
fn restore_intent(
    directory: &Path,
    row: &json::Value,
    operation: &str,
) -> Result<Option<json::Value>> {
    let name = format!("config-retirement-restore-{}.json", text(row, "role")?);
    match fs::symlink_metadata(directory.join(&name)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
        Ok(_) => {}
    }
    let intent = read_receipt(directory, &name)?;
    exact_fields(
        &intent,
        &[
            "schema",
            "operation",
            "role",
            "source_path",
            "source_sha256",
            "restore_path",
            "restore_stamp",
        ],
    )?;
    let source = Path::new(text(row, "source_path")?);
    let path = source.with_file_name(format!(
        ".{}.{}.retirement-restore",
        source.file_name().unwrap().to_string_lossy(),
        operation
    ));
    ensure!(
        text(&intent, "schema")? == format!("{PREFIX}.restore-intent.v1")
            && text(&intent, "operation")? == operation
            && text(&intent, "role")? == text(row, "role")?
            && text(&intent, "source_path")? == text(row, "source_path")?
            && text(&intent, "source_sha256")? == text(row, "source_sha256")?
            && text(&intent, "restore_path")? == path.to_string_lossy(),
        "retirement restore intent binding differs"
    );
    stamp_value(field(&intent, "restore_stamp")?)?;
    Ok(Some(field(&intent, "restore_stamp")?.clone()))
}

#[cfg(target_os = "linux")]
fn validate_private_relation(
    row: &json::Value,
    require_installed: bool,
    restored: Option<&json::Value>,
) -> Result<()> {
    let original = private_bytes(Path::new(text(row, "original_path")?))?;
    ensure!(
        sha256_hex(&original) == text(row, "source_sha256")?,
        "retirement original digest differs"
    );
    let (projected, changed) = project(&original)?;
    ensure!(
        Some(changed) == field(row, "changed")?.as_bool()
            && sha256_hex(&projected) == text(row, "output_sha256")?,
        "retirement semantic relation differs"
    );
    let source = Path::new(text(row, "source_path")?);
    let before = stamp(source)?;
    let current = private_bytes(source)?;
    let is_old =
        before == *field(row, "before_stamp")? && current.as_slice() == original.as_slice();
    let is_new = same_renamed_file(&before, field(row, "staged_stamp")?)?
        && current.as_slice() == projected.as_slice();
    if require_installed {
        ensure!(
            before == *field(row, "installed_stamp")? && (if changed { is_new } else { is_old }),
            "installed retirement config differs"
        );
    } else {
        let is_restored = match restored {
            Some(expected) => {
                same_renamed_file(&before, expected)? && current.as_slice() == original.as_slice()
            }
            None => false,
        };
        ensure!(
            is_old || is_new || is_restored,
            "retirement target is neither exact old nor projected config"
        );
        if is_old {
            let staged = Path::new(text(row, "staged_path")?);
            ensure!(
                stamp(staged)? == *field(row, "staged_stamp")?
                    && private_bytes(staged)?.as_slice() == projected.as_slice(),
                "retirement staged config differs"
            );
        }
    }
    ensure!(
        stamp(source)? == before,
        "retirement config changed during verification"
    );
    Ok(())
}

/// Independently consume retained private configs and verify the exact installed projection.
#[cfg(target_os = "linux")]
pub(crate) fn verify_installed_retirement(
    prepared: &json::Value,
    installed: &json::Value,
    plan: &json::Value,
) -> Result<()> {
    validate_retirement_records(prepared, installed, plan)?;
    for row in rows(installed)? {
        validate_private_relation(row, true, None)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "taira_validator_config_retirement_tests.rs"]
mod tests;
