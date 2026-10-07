//! Read-only build-identity transition after an actual preserved-state Taira update.
//! Original deployment intent and genesis/peer/config authority are never rewritten.

use super::*;
use base64::Engine as _;
use iroha_crypto::Hash;

const BASE: &str = "/private/runtime/taira-public-reset";
const RECORDS: [&str; 7] = [
    "intent.json",
    "result.json",
    "retained-entry.json",
    "after.json",
    "checkpoint-stopped.json",
    "checkpoint-restored.json",
    "cohort-ready.json",
];

fn field<'a>(value: &'a json::Value, name: &str) -> Result<&'a json::Value> {
    value
        .get(name)
        .ok_or_else(|| eyre!("runtime update evidence field missing: {name}"))
}
fn text<'a>(value: &'a json::Value, name: &str) -> Result<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| eyre!("runtime update text field differs: {name}"))
}
fn rows(value: &json::Value) -> Result<&Vec<json::Value>> {
    value
        .as_array()
        .ok_or_else(|| eyre!("runtime update cohort is not an array"))
}
fn flag(value: &json::Value, name: &str, expected: bool) -> Result<()> {
    require(
        field(value, name)?.as_bool() == Some(expected),
        "runtime update completion claim differs",
    )
}
fn number(value: &json::Value, name: &str) -> Result<u64> {
    field(value, name)?
        .as_u64()
        .ok_or_else(|| eyre!("runtime update integer field differs: {name}"))
}

fn authenticated_replay_height(events: &json::Value) -> Result<u64> {
    let events = rows(events)?;
    require(events.len() <= 64, "runtime replay event bound exceeded")?;
    let mut failed = Vec::new();
    let mut begun = Vec::new();
    let mut completed = Vec::new();
    for event in events {
        let message = text(event, "message")?;
        require(
            ![
                "creating an empty state",
                "Snapshot restore is disabled",
                "emergency Fast",
                "Successfully loaded the state from a snapshot",
                "Validated snapshot block hashes against Kura",
            ]
            .iter()
            .any(|value| message.contains(value)),
            "runtime replay has a foreign or mixed startup mode",
        )?;
        if message.contains("Failed to load state snapshot") {
            failed.push(event);
        }
        if message.contains(
            "Kura retains the configured-primary replay floor; rebuilding state from blocks",
        ) {
            begun.push(event);
        }
        if message.contains("Sumeragi rebuilt the state from genesis and Kura") {
            completed.push(event);
        }
    }
    require(
        failed.len() == 1 && begun.len() == 1 && completed.len() == 1,
        "runtime native replay sequence differs",
    )?;
    require(
        text(failed[0], "message")?.split_whitespace().last()
            == Some("error=NativeExecutionReplayRequired")
            && number(failed[0], "time_us")? > 0
            && number(failed[0], "time_us")? <= number(begun[0], "time_us")?
            && number(begun[0], "time_us")? <= number(completed[0], "time_us")?,
        "runtime native replay policy or event order differs",
    )?;
    let heights = text(completed[0], "message")?
        .split_whitespace()
        .filter_map(|word| word.strip_prefix("height="))
        .collect::<Vec<_>>();
    require(
        heights.len() == 1
            && !heights[0].is_empty()
            && heights[0].bytes().all(|byte| byte.is_ascii_digit()),
        "runtime native replay completion height missing",
    )?;
    let height = heights[0].parse()?;
    require(height > 0, "runtime native replay completion is empty")?;
    Ok(height)
}

fn verify_restored_record(
    restored: &json::Value,
    stopped: &json::Value,
    after: &json::Value,
    source: &str,
    network: NetworkId,
    modern: bool,
) -> Result<bool> {
    if field(restored, "native_strict_checkpoint_verified")?.as_bool() == Some(true) {
        require(
            ![
                "proof_kind",
                "native_authenticated_kura_replay_verified",
                "retained_kura_tip",
                "source_commit",
                "network_id",
            ]
            .iter()
            .any(|key| restored.get(*key).is_some()),
            "Strict restore receipt cannot claim native replay",
        )?;
        return Ok(true);
    }
    require(modern, "legacy runtime requires native Strict restoration")?;
    flag(restored, "native_strict_checkpoint_verified", false)?;
    flag(restored, "native_authenticated_kura_replay_verified", true)?;
    require(
        text(restored, "proof_kind")? == "native_authenticated_kura_replay"
            && text(restored, "source_commit")? == source
            && text(restored, "network_id")? == network.to_string()
            && field(restored, "retained_kura_tip")? == field(stopped, "kura_tip")?,
        "runtime native replay retained identity differs",
    )?;
    let height = authenticated_replay_height(field(restored, "events")?)?;
    require(
        number(restored, "restored_height")? == height
            && height >= number(field(stopped, "kura_tip")?, "height")?
            && number(field(after, "public")?, "height")? >= height,
        "runtime native replay retained height differs",
    )?;
    let ready = field(after, "cohort_observation")?;
    flag(ready, "ready", true)?;
    flag(ready, "public_fresh", true)?;
    Ok(false)
}

/// Pure receipt joins are tested independently of privileged filesystem observation.
fn validate_records(
    records: &BTreeMap<String, json::Value>,
    operation: &str,
    network: NetworkId,
    source: &str,
    original: &DeploymentTrustV1,
    version: &str,
) -> Result<()> {
    let plan = &records["intent.json"];
    let result = &records["result.json"];
    let deployment = field(plan, "deployment")?;
    require(
        matches!(
            text(plan, "schema")?,
            "taira.daemon-update.plan.v1" | "taira.daemon-update.plan.v2"
        ) && text(result, "schema")? == text(plan, "schema")?.replace(".plan.", ".result.")
            && text(plan, "operation")? == operation
            && text(plan, "commit")? == source
            && text(result, "commit")? == source
            && text(plan, "network_id")? == network.to_string()
            && text(result, "network_id")? == network.to_string()
            && text(deployment, "network_id")? == network.to_string()
            && text(deployment, "runtime_root")? == BASE
            && text(deployment, "config_root")? == "/srv/taira"
            && text(deployment, "state_root")? == "/var/lib/taira",
        "runtime update source, network or retained roots differ",
    )?;
    for name in [
        "runtime_update_complete",
        "state_preserved",
        "cohort_processes_verified_after_final_observation",
        "all_own_retained_tips_verified_after_final_observation",
    ] {
        flag(result, name, true)?;
    }
    let modern = text(plan, "schema")? == "taira.daemon-update.plan.v2";
    let mut recovery_modes = Vec::new();
    for name in [
        "secret_contents_read",
        "transaction_submission",
        "python_transaction_submission",
    ] {
        flag(plan, name, false)?;
    }
    require(
        plan.get("failed_start").is_none(),
        "runtime verification does not reinterpret failed-start ancestry",
    )?;
    let previous = text(field(deployment, "current")?, "commit")?;
    require(
        previous != source
            && previous.len() == 40
            && previous
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "runtime update predecessor source differs",
    )?;
    let prior_fingerprint = Hash::new([version.as_bytes(), previous.as_bytes()].concat());
    require(
        original.peers.len() == 4
            && original
                .peers
                .iter()
                .all(|p| p.build_fingerprint == prior_fingerprint),
        "original peer build pins do not match the retained predecessor",
    )?;
    let units = rows(field(plan, "units")?)?;
    let artifacts = rows(field(plan, "artifacts")?)?;
    require(
        units.len() == 4 && artifacts.len() == 3,
        "runtime update artifact or cohort count differs",
    )?;
    for (artifact, expected) in artifacts.iter().zip(["iroha3d_taira", "iroha", "kagami"]) {
        require(
            text(artifact, "name")? == expected
                && text(artifact, "sha256")?.len() == 64
                && field(artifact, "size")?
                    .as_u64()
                    .is_some_and(|n| n > 1_000_000 && n < 1024 * 1024 * 1024),
            "runtime update artifact identity differs",
        )?;
    }
    let final_rows = rows(field(&records["cohort-ready.json"], "observations")?)?;
    for name in [
        "retained-entry.json",
        "after.json",
        "checkpoint-stopped.json",
        "checkpoint-restored.json",
    ] {
        require(
            rows(&records[name])?.len() == 4,
            "runtime update retained cohort count differs",
        )?;
    }
    require(
        final_rows.len() == 4,
        "runtime update final cohort count differs",
    )?;
    for index in 0..4 {
        let role = format!("taira-validator-{}", index + 1);
        let before = &rows(&records["retained-entry.json"])?[index];
        let after = &rows(&records["after.json"])?[index];
        let stopped = &rows(&records["checkpoint-stopped.json"])?[index];
        let restored = &rows(&records["checkpoint-restored.json"])?[index];
        for row in [
            &units[index],
            before,
            after,
            stopped,
            restored,
            &final_rows[index],
        ] {
            require(
                text(row, "role")? == role,
                "runtime update role ordering differs",
            )?;
        }
        let before_unit =
            base64::engine::general_purpose::STANDARD.decode(text(&units[index], "before")?)?;
        let after_unit =
            base64::engine::general_purpose::STANDARD.decode(text(&units[index], "after")?)?;
        let current = field(deployment, "current")?;
        let old =
            if current.get("kind").and_then(json::Value::as_str) == Some("observed-installation") {
                text(field(field(current, "daemons")?, &role)?, "command")?
            } else {
                text(current, "daemon")?
            };
        require(!old.is_empty(), "runtime predecessor daemon is empty")?;
        let positions: Vec<_> = before_unit
            .windows(old.len())
            .enumerate()
            .filter_map(|(i, bytes)| (bytes == old.as_bytes()).then_some(i))
            .collect();
        require(
            !old.is_empty()
                && positions.len() == 1
                && before_unit.len() <= 128 * 1024
                && digest(&before_unit) == text(&units[index], "before_sha256")?
                && digest(&after_unit) == text(&units[index], "after_sha256")?,
            "runtime unit transition digest differs",
        )?;
        let pos = positions[0];
        let candidate = format!("{BASE}/release-{source}-{operation}/bin/iroha3d_taira");
        let expected = [
            &before_unit[..pos],
            candidate.as_bytes(),
            &before_unit[pos + old.len()..],
        ]
        .concat();
        require(
            expected == after_unit,
            "runtime unit transition exceeds the daemon argument",
        )?;
        flag(stopped, "cohort_stopped", true)?;
        let strict = verify_restored_record(restored, stopped, after, source, network, modern)?;
        if !strict {
            let final_ready = field(&final_rows[index], "cohort_observation")?;
            flag(final_ready, "ready", true)?;
            flag(final_ready, "public_fresh", true)?;
        }
        recovery_modes.push(if strict {
            "native_strict_restore"
        } else {
            "native_authenticated_kura_replay"
        });
        require(
            number(restored, "restored_height")? >= number(stopped, "checkpoint_height")?
                && number(stopped, "checkpoint_height")? > 0,
            "runtime update did not restore its retained checkpoint",
        )?;
        for key in ["config_stamp", "state_root_identity", "current_target"] {
            require(
                field(before, key)? == field(after, key)?
                    && field(after, key)? == field(&final_rows[index], key)?,
                "runtime update changed retained config or state custody",
            )?;
        }
        for row in [after, &final_rows[index]] {
            let public = field(row, "public")?;
            require(
                text(public, "commit")? == source
                    && text(public, "network_id")? == network.to_string()
                    && number(public, "height")? >= number(field(stopped, "kura_tip")?, "height")?
                    && number(field(stopped, "kura_tip")?, "height")? > 0,
                "runtime update final public identity or retained height differs",
            )?;
        }
        for key in ["MainPID", "InvocationID", "NRestarts"] {
            require(
                field(field(after, "systemd")?, key)?
                    == field(field(&final_rows[index], "systemd")?, key)?,
                "runtime update final process differs",
            )?;
        }
    }
    let strict = recovery_modes
        .iter()
        .all(|mode| *mode == "native_strict_restore");
    flag(result, "retained_native_snapshot_verified", strict)?;
    if modern {
        flag(result, "retained_native_state_verified", true)?;
        flag(result, "historical_genesis_replay_supported", !strict)?;
        let modes = rows(field(result, "native_recovery_modes")?)?;
        require(
            modes.len() == 4
                && modes
                    .iter()
                    .zip(&recovery_modes)
                    .all(|(value, mode)| value.as_str() == Some(*mode)),
            "runtime completion recovery mode differs",
        )?;
    }
    Ok(())
}

fn verify_artifact(artifact: &json::Value, actual_sha256: &str, actual_size: u64) -> Result<()> {
    require(
        text(artifact, "sha256")? == actual_sha256 && number(artifact, "size")? == actual_size,
        "runtime artifact digest or size differs",
    )
}

fn verify_process(
    observed: &BTreeMap<String, String>,
    expected: &json::Value,
    fragment: &Path,
) -> Result<()> {
    for (key, expected) in [
        ("LoadState", "loaded"),
        ("ActiveState", "active"),
        ("SubState", "running"),
        ("ControlPID", "0"),
        ("DropInPaths", ""),
        ("NeedDaemonReload", "no"),
        ("Job", ""),
    ] {
        require(
            observed.get(key).map(String::as_str) == Some(expected),
            "runtime process is not the exact healthy cohort",
        )?;
    }
    require(
        observed.get("FragmentPath").map(String::as_str) == fragment.to_str(),
        "runtime systemd fragment differs",
    )?;
    for key in ["MainPID", "InvocationID", "NRestarts"] {
        require(
            observed.get(key).map(String::as_str) == Some(text(expected, key)?),
            "runtime process identity changed",
        )?;
    }
    Ok(())
}

pub(super) struct Verified {
    pub(super) trust: DeploymentTrustV1,
    pub(super) receipt: json::Value,
    #[cfg(target_os = "linux")]
    held: Vec<PublicInput>,
    #[cfg(target_os = "linux")]
    records: BTreeMap<String, json::Value>,
    #[cfg(target_os = "linux")]
    directory: PathBuf,
    #[cfg(target_os = "linux")]
    binaries: Vec<(PathBuf, File, fs::Metadata)>,
}

#[cfg(target_os = "linux")]
fn root_path(path: &Path, directory: bool) -> Result<fs::Metadata> {
    use std::os::unix::fs::MetadataExt as _;
    require(
        path.is_absolute() && path.canonicalize()? == path,
        "runtime evidence path is indirect",
    )?;
    for ancestor in path.ancestors().skip(1) {
        let meta = fs::symlink_metadata(ancestor)?;
        require(
            meta.is_dir() && meta.uid() == 0 && meta.mode() & 0o022 == 0,
            "runtime evidence ancestor custody differs",
        )?;
    }
    let meta = fs::symlink_metadata(path)?;
    require(
        meta.uid() == 0
            && meta.mode() & 0o022 == 0
            && if directory {
                meta.is_dir()
            } else {
                meta.is_file() && meta.nlink() == 1
            },
        "runtime evidence owner or file kind differs",
    )?;
    Ok(meta)
}

#[cfg(unix)]
fn metadata_value(meta: &fs::Metadata) -> Result<json::Value> {
    use std::os::unix::fs::MetadataExt as _;
    let timestamp = |seconds: i64, nanos: i64| -> Result<u64> {
        let value = i128::from(seconds) * 1_000_000_000 + i128::from(nanos);
        u64::try_from(value)
            .map_err(|_| eyre!("runtime metadata timestamp is outside the supported range"))
    };
    Ok(json::to_value(&vec![
        meta.dev(),
        meta.ino(),
        u64::from(meta.mode()),
        u64::from(meta.uid()),
        u64::from(meta.gid()),
        meta.nlink(),
        meta.len(),
        timestamp(meta.mtime(), meta.mtime_nsec())?,
        timestamp(meta.ctime(), meta.ctime_nsec())?,
    ])?)
}

#[cfg(target_os = "linux")]
fn process(role: &str) -> Result<BTreeMap<String, String>> {
    let result = std::process::Command::new("/usr/bin/systemctl").args(["show", "--all",
        "--property=LoadState,FragmentPath,DropInPaths,NeedDaemonReload,ActiveState,SubState,MainPID,ControlPID,InvocationID,NRestarts,Job",
        &format!("iroha3d-{role}.service")]).output()?;
    require(
        result.status.success() && result.stdout.len() <= 65536,
        "runtime process observation failed",
    )?;
    let mut values = BTreeMap::new();
    for line in std::str::from_utf8(&result.stdout)?.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| eyre!("invalid runtime process observation"))?;
        require(
            values.insert(key.into(), value.into()).is_none(),
            "duplicate runtime process observation",
        )?;
    }
    require(
        values.len() == 11,
        "runtime process observation fields differ",
    )?;
    Ok(values)
}

impl Verified {
    pub(super) fn admit(
        path: &Path,
        original: &DeploymentTrustV1,
        network: NetworkId,
    ) -> Result<Self> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (path, original, network);
            eyre::bail!("runtime update verification requires the actual Linux validator guest");
        }
        #[cfg(target_os = "linux")]
        {
            use std::io::Read as _;
            use std::os::unix::fs::MetadataExt as _;
            require(
                rustix::process::geteuid().as_raw() == 0,
                "runtime update verification requires root metadata custody",
            )?;
            let operation = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| eyre!("invalid runtime update path"))?;
            require(
                path.parent() == Some(Path::new(BASE))
                    && operation.strip_prefix("update-").is_some_and(|s| {
                        s.len() == 32
                            && s.bytes()
                                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    }),
                "runtime update must select one exact retained operation",
            )?;
            require(
                root_path(path, true)?.mode() & 0o7777 == 0o700,
                "runtime operation must remain root-private",
            )?;
            let mut held = Vec::new();
            let mut records = BTreeMap::new();
            let mut hashes = BTreeMap::new();
            for name in RECORDS {
                let selected = path.join(name);
                require(
                    root_path(&selected, false)?.mode() & 0o7777 == 0o600,
                    "runtime receipt must remain root-private",
                )?;
                let input = PublicInput::read(&selected)?;
                hashes.insert(name.to_owned(), digest(&input.bytes));
                records.insert(name.to_owned(), json::from_slice(&input.bytes)?);
                held.push(input);
            }
            let identity = crate::compiled_build_identity()?;
            let source = identity.release_source_commit()?;
            validate_records(
                &records,
                operation,
                network,
                source,
                original,
                identity.version(),
            )?;
            let release = Path::new(BASE).join(format!("release-{source}-{operation}/bin"));
            require(
                std::env::current_exe()? == release.join("iroha"),
                "runtime verifier is not the actual prepared candidate CLI",
            )?;
            let mut binaries = Vec::new();
            for artifact in rows(field(&records["intent.json"], "artifacts")?)? {
                let path = release.join(text(artifact, "name")?);
                let snapshot = root_path(&path, false)?;
                require(
                    snapshot.len() == field(artifact, "size")?.as_u64().unwrap_or(0)
                        && snapshot.mode() & 0o111 != 0,
                    "runtime artifact size differs",
                )?;
                let mut file = File::from(rustix::fs::open(
                    &path,
                    rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW,
                    rustix::fs::Mode::empty(),
                )?);
                let mut hash = Sha256::new();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = file.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    hash.update(&buffer[..n]);
                }
                verify_artifact(artifact, &hex::encode(hash.finalize()), snapshot.len())?;
                require(
                    same_file_snapshot(&snapshot, &file.metadata()?)
                        && same_file_snapshot(&snapshot, &root_path(&path, false)?),
                    "runtime artifact digest or custody differs",
                )?;
                binaries.push((path, file, snapshot));
            }
            let mut trust = original.clone();
            for peer in &mut trust.peers {
                peer.build_fingerprint = identity.build_fingerprint();
            }
            let receipt = norito::json!({"schema":"iroha.dataspace-runtime-update-verification.v1",
                "operation_directory":(path.to_string_lossy().into_owned()), "source_commit":source,
                "receipt_sha256":hashes, "original_trust_sha256":(digest(&json::to_vec(original)?)),
                "effective_trust_sha256":(digest(&json::to_vec(&trust)?)), "chain_write_performed":false});
            let value = Self {
                trust,
                receipt,
                held,
                records,
                directory: path.into(),
                binaries,
            };
            value.revalidate()?;
            Ok(value)
        }
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        #[cfg(not(target_os = "linux"))]
        {
            eyre::bail!("runtime update verification requires Linux");
        }
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt as _;
            for input in &self.held {
                input.revalidate()?;
            }
            for (path, file, snapshot) in &self.binaries {
                require(
                    same_file_snapshot(snapshot, &file.metadata()?)
                        && same_file_snapshot(snapshot, &root_path(path, false)?),
                    "runtime artifact custody changed",
                )?;
            }
            for name in ["failure.json", "rollback.json"] {
                require(
                    fs::symlink_metadata(self.directory.join(name))
                        .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
                    "runtime update has a failure or rollback marker",
                )?;
            }
            let plan = &self.records["intent.json"];
            let deployment = field(plan, "deployment")?;
            let release = text(deployment, "config_release")?;
            require(
                release.len() == 40 && release.bytes().all(|b| b.is_ascii_hexdigit()),
                "runtime retained config revision differs",
            )?;
            let filename = deployment
                .get("config_filename")
                .and_then(json::Value::as_str)
                .unwrap_or("config.toml");
            require(
                matches!(filename, "config.toml" | "beacon.toml"),
                "runtime config filename differs",
            )?;
            let daemon = &self.binaries[0].0;
            let final_rows = rows(field(&self.records["cohort-ready.json"], "observations")?)?;
            for index in 0..4 {
                let role = format!("taira-validator-{}", index + 1);
                let unit = &rows(field(plan, "units")?)?[index];
                let after = &final_rows[index];
                let fragment = PathBuf::from(format!("/etc/systemd/system/iroha3d-{role}.service"));
                root_path(&fragment, false)?;
                let raw = PublicInput::read(&fragment)?;
                require(
                    raw.bytes
                        == base64::engine::general_purpose::STANDARD
                            .decode(text(unit, "after")?)?
                        && digest(&raw.bytes) == text(unit, "after_sha256")?,
                    "installed runtime unit differs",
                )?;
                let target = PathBuf::from(format!("/srv/taira/{role}/releases/{release}"));
                let selector = PathBuf::from(format!("/srv/taira/{role}/current"));
                require(
                    fs::symlink_metadata(&selector)?.uid() == 0
                        && fs::read_link(&selector)? == target
                        && text(after, "current_target")? == target.to_string_lossy(),
                    "runtime config selector changed",
                )?;
                require(
                    metadata_value(&root_path(&target.join("config").join(filename), false)?)?
                        == *field(after, "config_stamp")?,
                    "retained runtime config metadata changed",
                )?;
                let state = root_path(&PathBuf::from(format!("/var/lib/taira/{role}")), true)?;
                require(
                    json::to_value(&vec![state.dev(), state.ino()])?
                        == *field(after, "state_root_identity")?,
                    "retained state root changed",
                )?;
                let observed = process(&role)?;
                verify_process(&observed, field(after, "systemd")?, &fragment)?;
                let pid: u32 = observed["MainPID"].parse()?;
                require(pid > 0, "runtime PID missing")?;
                let proc = PathBuf::from(format!("/proc/{pid}"));
                let expected = format!(
                    "{}\0--config\0{}/config/{filename}\0--sora\0",
                    daemon.display(),
                    selector.display()
                );
                require(
                    fs::read_link(proc.join("exe"))? == *daemon
                        && fs::read(proc.join("cmdline"))? == expected.as_bytes(),
                    "runtime executable or process arguments differ",
                )?;
                raw.revalidate()?;
                require(
                    process(&role)? == observed,
                    "runtime process changed during verification",
                )?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
pub(super) fn test_transition_trust() -> DeploymentTrustV1 {
    // Receipt-join tests do not execute or authenticate genesis. Full finality
    // authentication remains the existing production verifier's responsibility.
    let key = |seed| {
        iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519)
            .unwrap()
    };
    DeploymentTrustV1 {
        chain: "fc56984b-2be7-431d-840e-21514d1883f0".into(),
        account_chain_discriminant: 369,
        genesis_public_key: key(9).public_key().clone(),
        genesis_signed_wire_hex: String::new(),
        peers: (1..=4)
            .map(|seed| finality::PeerV1 {
                torii_origin: format!("http://127.0.0.1:{}/", 8080 + u16::from(seed)),
                peer_id: iroha_model_base::peer::PeerId::new(key(seed).public_key().clone()),
                node_fingerprint: Hash::new([seed]),
                build_fingerprint: Hash::new([1]),
                config_fingerprint: Hash::new([2]),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "2222222222222222222222222222222222222222";
    const PREVIOUS: &str = "1111111111111111111111111111111111111111";
    const VERSION: &str = "2.0.0-rc.2.0";

    fn fixture() -> (
        BTreeMap<String, json::Value>,
        String,
        NetworkId,
        DeploymentTrustV1,
    ) {
        let network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            Hash::new("receipt-join-fixture"),
        ));
        let mut trust = test_transition_trust();
        for peer in &mut trust.peers {
            peer.build_fingerprint = Hash::new(format!("{VERSION}{PREVIOUS}"));
        }
        let operation = format!("update-{}", "a".repeat(32));
        let old = format!("{BASE}/previous/bin/iroha3d_taira");
        let candidate = format!("{BASE}/release-{SOURCE}-{operation}/bin/iroha3d_taira");
        let mut units = Vec::new();
        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut stopped = Vec::new();
        let mut restored = Vec::new();
        for index in 1..=4 {
            let role = format!("taira-validator-{index}");
            let raw = format!(
                "ExecStart={old} --config /srv/taira/{role}/current/config/beacon.toml --sora"
            );
            let changed = raw.replace(&old, &candidate);
            units.push(norito::json!({"role":(role.clone()),"before":(base64::engine::general_purpose::STANDARD.encode(raw.as_bytes())),
                "before_sha256":(digest(raw.as_bytes())),"after":(base64::engine::general_purpose::STANDARD.encode(changed.as_bytes())),
                "after_sha256":(digest(changed.as_bytes()))}));
            let row = norito::json!({"role":(role.clone()), "config_stamp":[1,2,3],"state_root_identity":[1,2],"current_target":"retained",
                "public":{"commit":SOURCE,"network_id":(network.to_string()),"height":12},
                "systemd":{"MainPID":"123","InvocationID":"actual","NRestarts":"0"}});
            before.push(row.clone());
            after.push(row);
            stopped.push(norito::json!({"role":(role.clone()),"cohort_stopped":true,"checkpoint_height":12,"kura_tip":{"height":12}}));
            restored.push(norito::json!({"role":role,"native_strict_checkpoint_verified":true,"restored_height":12}));
        }
        let artifacts: Vec<_> = ["iroha3d_taira", "iroha", "kagami"]
            .into_iter()
            .map(|name| norito::json!({"name":name,"sha256":("a".repeat(64)),"size":2_000_000}))
            .collect();
        let intent = norito::json!({"schema":"taira.daemon-update.plan.v2","operation":(operation.clone()),"commit":SOURCE,
            "network_id":(network.to_string()),"secret_contents_read":false,"transaction_submission":false,
            "python_transaction_submission":false,"units":units,"artifacts":artifacts,
            "deployment":{"runtime_root":BASE,"config_root":"/srv/taira","state_root":"/var/lib/taira",
                "network_id":(network.to_string()),"current":{"commit":PREVIOUS,"daemon":old}}});
        let result = norito::json!({"schema":"taira.daemon-update.result.v2","commit":SOURCE,"network_id":(network.to_string()),
            "runtime_update_complete":true,"state_preserved":true,"retained_native_snapshot_verified":true,
            "retained_native_state_verified":true,"historical_genesis_replay_supported":false,
            "native_recovery_modes":["native_strict_restore","native_strict_restore","native_strict_restore","native_strict_restore"],
            "cohort_processes_verified_after_final_observation":true,"all_own_retained_tips_verified_after_final_observation":true});
        (
            BTreeMap::from([
                ("intent.json".into(), intent),
                ("result.json".into(), result),
                ("retained-entry.json".into(), norito::json!(before)),
                ("after.json".into(), norito::json!(after.clone())),
                (
                    "cohort-ready.json".into(),
                    norito::json!({"observations":after}),
                ),
                ("checkpoint-stopped.json".into(), norito::json!(stopped)),
                ("checkpoint-restored.json".into(), norito::json!(restored)),
            ]),
            operation,
            network,
            trust,
        )
    }

    #[test]
    fn runtime_transition_joins_truthful_replay_cohort_completion() {
        let (mut records, operation, network, trust) = fixture();
        let events = norito::json!([
            {"time_us":1,"message":"Failed to load state snapshot error=NativeExecutionReplayRequired"},
            {"time_us":2,"message":"Kura retains the configured-primary replay floor; rebuilding state from blocks"},
            {"time_us":3,"message":"Sumeragi rebuilt the state from genesis and Kura height=12"}
        ]);
        for index in 0..4 {
            let stopped = rows(&records["checkpoint-stopped.json"]).unwrap()[index].clone();
            let recovered = records
                .get_mut("checkpoint-restored.json")
                .unwrap()
                .get_mut(index)
                .unwrap()
                .as_object_mut()
                .unwrap();
            recovered.insert(
                "native_strict_checkpoint_verified".into(),
                norito::json!(false),
            );
            recovered.insert(
                "native_authenticated_kura_replay_verified".into(),
                norito::json!(true),
            );
            recovered.insert(
                "proof_kind".into(),
                norito::json!("native_authenticated_kura_replay"),
            );
            recovered.insert("source_commit".into(), norito::json!(SOURCE));
            recovered.insert("network_id".into(), norito::json!(network.to_string()));
            recovered.insert(
                "retained_kura_tip".into(),
                field(&stopped, "kura_tip").unwrap().clone(),
            );
            recovered.insert("events".into(), events.clone());
            records
                .get_mut("after.json")
                .unwrap()
                .get_mut(index)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(
                    "cohort_observation".into(),
                    norito::json!({"ready":true,"public_fresh":true}),
                );
            records
                .get_mut("cohort-ready.json")
                .unwrap()
                .get_mut("observations")
                .unwrap()
                .get_mut(index)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(
                    "cohort_observation".into(),
                    norito::json!({"ready":true,"public_fresh":true}),
                );
        }
        let result = records
            .get_mut("result.json")
            .unwrap()
            .as_object_mut()
            .unwrap();
        result.insert(
            "retained_native_snapshot_verified".into(),
            norito::json!(false),
        );
        result.insert("retained_native_state_verified".into(), norito::json!(true));
        result.insert(
            "historical_genesis_replay_supported".into(),
            norito::json!(true),
        );
        result.insert(
            "native_recovery_modes".into(),
            json::to_value(&vec!["native_authenticated_kura_replay"; 4]).unwrap(),
        );
        validate_records(&records, &operation, network, SOURCE, &trust, VERSION).unwrap();
        for (name, value) in [
            ("retained_native_snapshot_verified", norito::json!(true)),
            ("retained_native_state_verified", norito::json!(false)),
            ("historical_genesis_replay_supported", norito::json!(false)),
            (
                "native_recovery_modes",
                norito::json!(["native_strict_restore"]),
            ),
        ] {
            let mut bad = records.clone();
            bad.get_mut("result.json")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(name.into(), value);
            assert!(validate_records(&bad, &operation, network, SOURCE, &trust, VERSION).is_err());
        }
    }

    #[test]
    fn runtime_transition_native_replay_requires_exact_policy_prefix_and_ready_identity() {
        let (_, _, network, _) = fixture();
        let events = norito::json!([
            {"time_us":1,"message":"Failed to load state snapshot; checking whether Kura can rebuild from an empty state error=NativeExecutionReplayRequired"},
            {"time_us":2,"message":"Kura retains the configured-primary replay floor; rebuilding state from blocks"},
            {"time_us":3,"message":"Sumeragi rebuilt the state from genesis and Kura height=12 instance=abc"}
        ]);
        let tip = norito::json!({"height":12,"hash":("a".repeat(64))});
        let stopped = norito::json!({"checkpoint_height":12,"kura_tip":(tip.clone())});
        let after = norito::json!({"public":{"height":12},"cohort_observation":{"ready":true,"public_fresh":true}});
        let restored = norito::json!({"native_strict_checkpoint_verified":false,
            "native_authenticated_kura_replay_verified":true,"proof_kind":"native_authenticated_kura_replay",
            "source_commit":SOURCE,"network_id":(network.to_string()),"retained_kura_tip":tip,
            "restored_height":12,"events":events});
        assert!(
            !verify_restored_record(&restored, &stopped, &after, SOURCE, network, true).unwrap()
        );
        assert!(
            verify_restored_record(&restored, &stopped, &after, SOURCE, network, false).is_err()
        );
        for (name, value) in [
            ("native_strict_checkpoint_verified", norito::json!(true)),
            ("source_commit", norito::json!(PREVIOUS)),
            ("network_id", norito::json!("foreign")),
            (
                "retained_kura_tip",
                norito::json!({"height":12,"hash":("b".repeat(64))}),
            ),
            ("restored_height", norito::json!(11)),
            (
                "native_authenticated_kura_replay_verified",
                norito::json!(false),
            ),
        ] {
            let mut bad = restored.clone();
            bad.as_object_mut().unwrap().insert(name.into(), value);
            assert!(verify_restored_record(&bad, &stopped, &after, SOURCE, network, true).is_err());
        }
        let events = field(&restored, "events").unwrap();
        for (index, name, value) in [
            (
                0usize,
                "message",
                norito::json!("Failed to load state snapshot error=NotFound"),
            ),
            (1usize, "time_us", norito::json!(4)),
            (
                2usize,
                "message",
                norito::json!("Sumeragi rebuilt the state from genesis and Kura height=0"),
            ),
            (2usize, "message", norito::json!("emergency Fast")),
        ] {
            let mut bad = events.clone();
            bad.get_mut(index)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(name.into(), value);
            assert!(authenticated_replay_height(&bad).is_err());
        }
        for suffix in [
            "height=12 height=12",
            "height=12 height=bad",
            "height=12,",
            "height=+12",
            "height=",
            "height=１２",
            "",
        ] {
            let mut bad = events.clone();
            bad.get_mut(2).unwrap().as_object_mut().unwrap().insert(
                "message".into(),
                norito::json!(format!(
                    "Sumeragi rebuilt the state from genesis and Kura {suffix}"
                )),
            );
            assert!(authenticated_replay_height(&bad).is_err());
        }
        for key in ["ready", "public_fresh"] {
            let mut bad = after.clone();
            bad.get_mut("cohort_observation")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), norito::json!(false));
            assert!(
                verify_restored_record(&restored, &stopped, &bad, SOURCE, network, true).is_err()
            );
        }
    }

    #[test]
    fn runtime_transition_strict_completion_rejects_contradictory_mode_flags() {
        let (records, operation, network, trust) = fixture();
        validate_records(&records, &operation, network, SOURCE, &trust, VERSION).unwrap();
        for (key, value) in [
            ("retained_native_state_verified", norito::json!(false)),
            ("historical_genesis_replay_supported", norito::json!(true)),
            (
                "native_recovery_modes",
                json::to_value(&vec!["native_authenticated_kura_replay"; 4]).unwrap(),
            ),
        ] {
            let mut bad = records.clone();
            bad.get_mut("result.json")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(key.into(), value);
            assert!(validate_records(&bad, &operation, network, SOURCE, &trust, VERSION).is_err());
        }
    }

    #[test]
    fn runtime_transition_requires_completed_exact_source_and_preserved_native_cohort() {
        let (records, operation, network, trust) = fixture();
        validate_records(&records, &operation, network, SOURCE, &trust, VERSION).unwrap();
        for (record, field_name, value) in [
            (
                "result.json",
                "runtime_update_complete",
                norito::json!(false),
            ),
            ("result.json", "state_preserved", norito::json!(false)),
            (
                "result.json",
                "retained_native_snapshot_verified",
                norito::json!(false),
            ),
            ("result.json", "commit", norito::json!(PREVIOUS)),
            ("intent.json", "commit", norito::json!(PREVIOUS)),
            ("result.json", "network_id", norito::json!("foreign")),
            ("intent.json", "network_id", norito::json!("foreign")),
            ("intent.json", "transaction_submission", norito::json!(true)),
        ] {
            let mut changed = records.clone();
            changed
                .get_mut(record)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .insert(field_name.into(), value);
            assert!(
                validate_records(&changed, &operation, network, SOURCE, &trust, VERSION).is_err(),
                "{record}/{field_name}"
            );
        }
        let mut changed = trust.clone();
        changed.peers[0].build_fingerprint = Hash::new("foreign");
        assert!(
            validate_records(&records, &operation, network, SOURCE, &changed, VERSION).is_err()
        );
    }

    #[test]
    fn runtime_transition_rejects_rewritten_units_config_and_process_history() {
        let (records, operation, network, trust) = fixture();
        for (record, parent, name, value) in [
            ("after.json", None, "config_stamp", norito::json!([9, 9, 9])),
            (
                "checkpoint-restored.json",
                None,
                "native_strict_checkpoint_verified",
                norito::json!(false),
            ),
            (
                "checkpoint-restored.json",
                None,
                "restored_height",
                norito::json!(11),
            ),
            (
                "after.json",
                Some("systemd"),
                "MainPID",
                norito::json!("999"),
            ),
            ("after.json", Some("public"), "height", norito::json!(11)),
        ] {
            let mut changed = records.clone();
            let row = changed.get_mut(record).unwrap().get_mut(0usize).unwrap();
            let target = if let Some(parent) = parent {
                row.get_mut(parent).unwrap()
            } else {
                row
            };
            target.as_object_mut().unwrap().insert(name.into(), value);
            assert!(
                validate_records(&changed, &operation, network, SOURCE, &trust, VERSION).is_err(),
                "{record}/{name}"
            );
        }
        let mut changed = records.clone();
        let unit = changed
            .get_mut("intent.json")
            .unwrap()
            .get_mut("units")
            .unwrap()
            .get_mut(0usize)
            .unwrap();
        let raw = base64::engine::general_purpose::STANDARD
            .decode(text(unit, "after").unwrap())
            .unwrap();
        let altered = [raw, b" --unsafe-option".to_vec()].concat();
        *unit.get_mut("after").unwrap() =
            norito::json!(base64::engine::general_purpose::STANDARD.encode(&altered));
        *unit.get_mut("after_sha256").unwrap() = norito::json!(digest(&altered));
        assert!(validate_records(&changed, &operation, network, SOURCE, &trust, VERSION).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn runtime_transition_metadata_is_nine_unsigned_json_fields() {
        use std::os::unix::fs::MetadataExt as _;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("public-receipt.json");
        fs::write(&path, b"{}\n").unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let value = metadata_value(&metadata).unwrap();
        let fields = value.as_array().unwrap();
        assert_eq!(fields.len(), 9);
        assert!(fields.iter().all(|value| value.as_u64().is_some()));
        assert_eq!(fields[0].as_u64(), Some(metadata.dev()));
        assert_eq!(fields[1].as_u64(), Some(metadata.ino()));
        assert_eq!(fields[2].as_u64(), Some(u64::from(metadata.mode())));
        assert_eq!(fields[6].as_u64(), Some(3));
        let encoded = json::to_vec(&value).unwrap();
        assert_eq!(json::from_slice::<json::Value>(&encoded).unwrap(), value);
    }

    #[test]
    fn runtime_transition_checks_actual_artifact_and_live_process() {
        let artifact = norito::json!({"sha256":("a".repeat(64)),"size":2_000_000});
        verify_artifact(&artifact, &"a".repeat(64), 2_000_000).unwrap();
        assert!(verify_artifact(&artifact, &"b".repeat(64), 2_000_000).is_err());
        assert!(verify_artifact(&artifact, &"a".repeat(64), 2_000_001).is_err());
        let path = Path::new("/etc/systemd/system/iroha3d-taira-validator-1.service");
        let observed = BTreeMap::from(
            [
                ("LoadState", "loaded"),
                ("ActiveState", "active"),
                ("SubState", "running"),
                ("ControlPID", "0"),
                ("DropInPaths", ""),
                ("NeedDaemonReload", "no"),
                ("Job", ""),
                ("MainPID", "123"),
                ("InvocationID", "actual"),
                ("NRestarts", "0"),
                ("FragmentPath", path.to_str().unwrap()),
            ]
            .map(|(key, value)| (key.to_owned(), value.to_owned())),
        );
        let expected = norito::json!({"MainPID":"123","InvocationID":"actual","NRestarts":"0"});
        verify_process(&observed, &expected, path).unwrap();
        for (key, value) in [
            ("MainPID", "999"),
            ("InvocationID", "changed"),
            ("NRestarts", "1"),
            ("NeedDaemonReload", "yes"),
            ("ActiveState", "failed"),
            ("DropInPaths", "/unapproved.conf"),
        ] {
            let mut changed = observed.clone();
            changed.insert(key.into(), value.into());
            assert!(verify_process(&changed, &expected, path).is_err(), "{key}");
        }
    }
}
