//! Read-only capture of the installed, stopped predecessor runtime.
//!
//! The selected configuration and the loaded unit are separate authorities: the
//! selector identifies configuration, while the installed unit identifies the
//! daemon actually selected by systemd. Neither is inferred from a prior plan.
// The root transaction runs on Linux only; other platforms compile these items solely for their
// unit tests, which do not reach every Linux entry point.
#![cfg_attr(all(test, not(target_os = "linux")), allow(dead_code))]
use super::*;
#[cfg(any(target_os = "linux", test))]
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[cfg(any(target_os = "linux", test))]
const HOST_PUBLIC_KEY: &str = "/etc/ssh/ssh_host_ed25519_key.pub";
#[cfg(any(target_os = "linux", test))]
const UNIT_PROPERTIES: &str = "LoadState,FragmentPath,DropInPaths,NeedDaemonReload,ActiveState,SubState,MainPID,ControlPID,Job";

/// Run on the approved Linux guest through its pinned SSH route, after stopping validators.
#[derive(clap::Args, Debug)]
pub(in super::super::super::super) struct CaptureDispatcherCurrentRuntime {
    /// SHA-256 of `ssh-ed25519 <base64>` from the independently approved host key.
    #[arg(long)]
    expected_host_identity_sha256: String,
    /// Required independently admitted MacStadium host pair, retained on the guest.
    #[arg(long)]
    host_pair: PathBuf,
    #[arg(long)]
    expected_host_pair_sha256: String,
    /// Independently signed native Mac capture; Linux never supplies edge observations.
    #[arg(long)]
    native_edge_capture: PathBuf,
    #[arg(long)]
    expected_native_edge_capture_sha256: String,
    #[arg(long)]
    expected_retained_inventory_sha256: String,
    #[arg(long)]
    expected_authorization_sha256: String,
    #[arg(long)]
    authorization_nonce: String,
    #[arg(long)]
    next_genesis_hash: String,
    /// A fresh file in an existing root-owned mode0700 directory.
    #[arg(long)]
    output: PathBuf,
}

#[cfg(any(target_os = "linux", test))]
fn host_identity(observed: &mut Observed) -> Result<String> {
    let key = observed.pin(Path::new(HOST_PUBLIC_KEY), Some(0o644), 16 * 1024)?;
    let bytes = admission::read(&key)?;
    let line = std::str::from_utf8(&bytes)?.trim_end_matches('\n');
    need(!line.contains('\n'), "SSH host key is not one line")?;
    let fields: Vec<&str> = line.split_ascii_whitespace().collect();
    need(
        fields.len() >= 2 && fields[0] == "ssh-ed25519",
        "approved SSH host key is not Ed25519",
    )?;
    Ok(sha256_hex(
        format!("{} {}", fields[0], fields[1]).as_bytes(),
    ))
}

#[cfg(any(target_os = "linux", test))]
fn unit_properties(unit: &str) -> Result<BTreeMap<String, String>> {
    let bytes = super::super::super::run_host_command(
        super::super::super::SYSTEMCTL,
        &[
            "show",
            "--all",
            &format!("--property={UNIT_PROPERTIES}"),
            unit,
        ],
        Instant::now() + Duration::from_secs(30),
    )?;
    let mut fields = BTreeMap::new();
    for line in std::str::from_utf8(&bytes)?.lines() {
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| eyre!("systemd returned a malformed unit property"))?;
        need(
            UNIT_PROPERTIES.split(',').any(|expected| expected == name)
                && fields.insert(name.to_owned(), value.to_owned()).is_none(),
            "systemd returned an unknown or repeated unit property",
        )?;
    }
    need(
        fields.len() == UNIT_PROPERTIES.split(',').count(),
        "systemd omitted a unit property",
    )?;
    Ok(fields)
}

#[cfg(any(target_os = "linux", test))]
fn require_unit(unit: &str, fragment: &str) -> Result<()> {
    let fields = unit_properties(unit)?;
    need(
        fields.get("LoadState").map(String::as_str) == Some("loaded")
            && fields.get("FragmentPath").map(String::as_str) == Some(fragment)
            && fields.get("DropInPaths").is_some_and(String::is_empty)
            && fields.get("NeedDaemonReload").map(String::as_str) == Some("no")
            && fields.get("ControlPID").map(String::as_str) == Some("0")
            && fields.get("Job").is_some_and(String::is_empty),
        "installed unit is not the exact loaded fragment",
    )?;
    need(
        fields.get("ActiveState").map(String::as_str) == Some("inactive")
            && fields.get("SubState").map(String::as_str) == Some("dead")
            && fields.get("MainPID").map(String::as_str) == Some("0"),
        "validator unit must be stopped before runtime capture",
    )
}

#[cfg(any(target_os = "linux", test))]
fn selected_release(slug: &str) -> Result<(String, String)> {
    need(
        SLUGS.contains(&slug),
        "guest selector must belong to a validator",
    )?;
    let service = format!("/srv/taira/{slug}");
    let selector = Path::new(&service).join("current");
    require_root_no_symlink_ancestors(&selector, "runtime selector")?;
    let metadata = fs::symlink_metadata(&selector)?;
    need(
        metadata.file_type().is_symlink() && metadata.uid() == 0,
        "unsafe runtime selector",
    )?;
    let release = fs::read_link(&selector)?;
    let root = Path::new(&service).join("releases");
    need(
        release.parent() == Some(root.as_path()),
        "runtime selector escaped its release root",
    )?;
    let commit = release
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| eyre!("selected release has no revision"))?;
    validate_lower_hex("selected configuration revision", commit, 40)?;
    require_root_directory(&release, false, "selected configuration release")?;
    Ok((release.to_string_lossy().into_owned(), commit.to_owned()))
}

/// Accept only the exact installed launcher assignment, without executing it.
#[cfg(any(target_os = "linux", test))]
fn daemon_in_unit(bytes: &[u8], slug: &str, selected_commit: &str) -> Result<InstalledDaemon> {
    let text = std::str::from_utf8(bytes)?;
    // systemd stores the launcher's Python newlines as literal `\n` escapes.
    let lines: Vec<&str> = text
        .split("\\n")
        .filter_map(|line| line.strip_prefix("cmd = ['"))
        .collect();
    need(
        lines.len() == 1,
        "installed unit must contain one daemon argv assignment",
    )?;
    let (daemon, tail) = lines[0]
        .split_once("', '--config', '")
        .ok_or_else(|| eyre!("installed unit daemon argv is malformed"))?;
    let config_name = ["config.toml", "beacon.toml"]
        .into_iter()
        .find(|name| tail == format!("/srv/taira/{slug}/current/config/{name}', '--sora']"))
        .ok_or_else(|| eyre!("installed daemon argv differs"))?;
    let stable = format!("/srv/taira/{slug}/current/bin/iroha3d_taira");
    if daemon == stable {
        validate_lower_hex("selected daemon revision", selected_commit, 40)?;
        return Ok(InstalledDaemon {
            argv0: daemon.to_owned(),
            artifact_path: format!(
                "/srv/taira/{slug}/releases/{selected_commit}/bin/iroha3d_taira"
            ),
            commit: selected_commit.to_owned(),
            config_name,
        });
    }
    let prefix = "/private/runtime/taira-public-reset/release-";
    let release = daemon
        .strip_prefix(prefix)
        .ok_or_else(|| eyre!("daemon is outside the installed update namespace"))?;
    let (commit, suffix) = release
        .split_once('-')
        .ok_or_else(|| eyre!("daemon update revision is absent"))?;
    validate_lower_hex("installed daemon revision", commit, 40)?;
    need(
        suffix
            .strip_prefix("update-")
            .and_then(|value| value.strip_suffix("/bin/iroha3d_taira"))
            .is_some_and(|operation| {
                operation.len() == 32
                    && operation
                        .bytes()
                        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
            }),
        "installed daemon path is not a bounded update release",
    )?;
    validate_absolute_normal_path(Path::new(daemon), "installed daemon path")?;
    Ok(InstalledDaemon {
        argv0: daemon.to_owned(),
        artifact_path: daemon.to_owned(),
        commit: commit.to_owned(),
        config_name,
    })
}

#[cfg(any(target_os = "linux", test))]
struct InstalledDaemon {
    argv0: String,
    artifact_path: String,
    commit: String,
    config_name: &'static str,
}

#[cfg(any(target_os = "linux", test))]
fn artifact(
    observed: &mut Observed,
    role: &str,
    path: String,
    source_commit: &str,
) -> Result<super::super::super::super::OccupiedArtifactV1> {
    let (mode, maximum) = super::super::super::super::artifact_role_policy(role)?;
    let pin = observed.pin(Path::new(&path), Some(u32::from(mode)), maximum)?;
    Ok(super::super::super::super::OccupiedArtifactV1 {
        role: role.to_owned(),
        path,
        sha256: pin.sha256,
        size: pin.size,
        mode,
        source_commit: source_commit.to_owned(),
    })
}

#[cfg(any(target_os = "linux", test))]
fn capture_validator(slug: &str, observed: &mut Observed) -> Result<ValidatorAdmittedReleaseV1> {
    let (release_root, commit) = selected_release(slug)?;
    let unit = format!("iroha3d-{slug}.service");
    let unit_path = format!("/etc/systemd/system/{unit}");
    require_unit(&unit, &unit_path)?;
    let unit_pin = observed.pin(Path::new(&unit_path), Some(0o644), 16 * 1024 * 1024)?;
    let daemon = daemon_in_unit(&admission::read(&unit_pin)?, slug, &commit)?;
    let artifacts = vec![
        artifact(observed, "iroha3d", daemon.artifact_path, &daemon.commit)?,
        artifact(
            observed,
            "config",
            format!("{release_root}/config/{}", daemon.config_name),
            &commit,
        )?,
        artifact(
            observed,
            "genesis",
            format!("{release_root}/genesis/genesis.json"),
            &commit,
        )?,
        artifact(
            observed,
            "genesis_hash",
            format!("{release_root}/genesis/genesis.sha256"),
            &commit,
        )?,
        artifact(observed, "validator_unit", unit_path, &daemon.commit)?,
    ];
    let state = format!("/var/lib/taira/{slug}");
    require_root_directory(Path::new(&state), true, "stopped validator state")?;
    let metadata = fs::symlink_metadata(state)?;
    let prior = ValidatorAdmittedReleaseV1 {
        commit,
        release_root,
        argv: vec![
            daemon.argv0,
            "--config".into(),
            format!("/srv/taira/{slug}/current/config/{}", daemon.config_name),
            "--sora".into(),
        ],
        artifacts,
        service_state: super::super::super::super::PriorValidatorServiceStateV1::Stopped(
            super::super::super::super::StoppedValidatorStateV1 {
                device: metadata.dev(),
                inode: metadata.ino(),
            },
        ),
    };
    occupied::validate_prior_binding(&prior, &format!("/srv/taira/{slug}"), &unit)?;
    Ok(prior)
}

#[cfg(any(target_os = "linux", test))]
fn capture_native_edge(
    command: &CaptureDispatcherCurrentRuntime,
    observed: &mut Observed,
) -> Result<(ResetHostPairV1, SignedNativeEdgeCaptureV1)> {
    for digest in [
        &command.expected_host_pair_sha256,
        &command.expected_native_edge_capture_sha256,
        &command.expected_retained_inventory_sha256,
        &command.expected_authorization_sha256,
    ] {
        require_lower_sha256(digest, "native runtime capture authority")?;
    }
    super::super::super::super::validate_canonical_iroha_hash(
        "native capture genesis hash",
        &command.next_genesis_hash,
    )?;
    super::super::super::super::validate_nonce(&command.authorization_nonce)?;
    let host_pair = observed.pin(&command.host_pair, None, 16 * 1024)?;
    let capture = observed.pin(&command.native_edge_capture, None, 64 * 1024)?;
    need(
        host_pair.sha256 == command.expected_host_pair_sha256
            && capture.sha256 == command.expected_native_edge_capture_sha256,
        "host pair or native capture public pin differs",
    )?;
    let hosts: ResetHostPairV1 = json::from_slice(&admission::read(&host_pair)?)?;
    let native_edge: SignedNativeEdgeCaptureV1 = json::from_slice(&admission::read(&capture)?)?;
    need(
        hosts.validator_guest.endpoint.host_identity_sha256
            == command.expected_host_identity_sha256,
        "capture host pair differs from independently approved guest route",
    )?;
    native_edge.verify_retained_join(
        &hosts,
        &command.expected_retained_inventory_sha256,
        &command.expected_authorization_sha256,
        &command.authorization_nonce,
        &command.next_genesis_hash,
    )?;
    let dispatcher = observed.pin(
        Path::new(&hosts.validator_guest.dispatcher_path),
        Some(0o755),
        MAX_BINARY,
    )?;
    let guard = observed.pin(
        &Path::new(&hosts.validator_guest.custody_root)
            .join(SLUGS[0])
            .join("guard.json"),
        Some(0o600),
        16 * 1024,
    )?;
    need(
        dispatcher.sha256 == hosts.validator_guest.dispatcher_sha256
            && guard.sha256 == hosts.validator_guest.guard_sha256,
        "native guest dispatcher or coordination guard differs from admitted current host custody",
    )?;
    Ok((hosts, native_edge))
}

impl CaptureDispatcherCurrentRuntime {
    /// Capture a private typed record; do not stop services or change selectors.
    pub(in super::super::super::super) fn run<W: Write>(&self, output: &mut W) -> Result<()> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = output;
            return Err(eyre!("dispatcher runtime capture requires Linux"));
        }
        #[cfg(target_os = "linux")]
        {
            need(rustix::process::geteuid().as_raw() == 0, "root is required")?;
            require_lower_sha256(
                &self.expected_host_identity_sha256,
                "approved host identity",
            )?;
            let locks = admission::locks_for_host(&self.expected_host_identity_sha256)?;
            let mut observed = Observed(Vec::new());
            need(
                host_identity(&mut observed)? == self.expected_host_identity_sha256,
                "guest SSH host identity differs from approved route",
            )?;
            let (hosts, native_edge) = capture_native_edge(self, &mut observed)?;
            let validators = SLUGS
                .iter()
                .map(|slug| capture_validator(slug, &mut observed))
                .collect::<Result<Vec<_>>>()?;
            let runtime = CurrentRuntime {
                schema: "iroha.taira.dispatcher-current-runtime.v1".into(),
                host_identity_sha256: self.expected_host_identity_sha256.clone(),
                hosts,
                validators,
                native_edge,
            };
            validate_runtime(&runtime)?;
            let _roles = runtime_roles(&runtime, &mut observed)?;
            locks.revalidate()?;
            observed.revalidate()?;
            for slug in &SLUGS {
                let (selected, _) = selected_release(slug)?;
                let index = slug
                    .strip_prefix("taira-validator-")
                    .unwrap()
                    .parse::<usize>()?
                    - 1;
                need(
                    selected == runtime.validators[index].release_root,
                    "validator selector moved during capture",
                )?;
                let unit = format!("iroha3d-{slug}.service");
                require_unit(&unit, &format!("/etc/systemd/system/{unit}"))?;
            }
            let bytes = json::to_vec(&runtime)?;
            require_root_no_symlink_ancestors(&self.output, "runtime capture output")?;
            super::super::super::super::inputs::write_new_private(&self.output, &bytes)?;
            writeln!(
                output,
                "{}",
                json::to_json(&norito::json!({
                    "schema": "iroha.taira.dispatcher-current-runtime-captured.v1",
                    "path": (self.output.to_string_lossy().as_ref()),
                    "sha256": (sha256_hex(&bytes)),
                    "validator_count": 4,
                    "host_pair_sha256": (runtime.hosts.digest()?),
                    "native_edge_capture_sha256": (self.expected_native_edge_capture_sha256),
                    "native_edge_captured_at_unix_ms": (runtime.native_edge.claims.captured_at_unix_ms),
                    "services_stopped": true,
                    "ledger_mutated": false,
                }))?
            )?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_capture_requires_independent_host_and_native_edge_inputs() {
        use clap::Parser as _;

        let arguments = vec![
            "iroha",
            "taira",
            "public-reset",
            "capture-dispatcher-current-runtime",
            "--expected-host-identity-sha256",
            "guest",
            "--host-pair",
            "/host-pair.json",
            "--expected-host-pair-sha256",
            "pair",
            "--native-edge-capture",
            "/native-edge-capture.json",
            "--expected-native-edge-capture-sha256",
            "capture",
            "--expected-retained-inventory-sha256",
            "inventory",
            "--expected-authorization-sha256",
            "authorization",
            "--authorization-nonce",
            "nonce",
            "--next-genesis-hash",
            "genesis",
            "--output",
            "/runtime.json",
        ];
        assert!(crate::Args::try_parse_from(arguments.clone()).is_ok());
        for required in [
            "--host-pair",
            "--expected-host-pair-sha256",
            "--native-edge-capture",
            "--expected-native-edge-capture-sha256",
            "--expected-retained-inventory-sha256",
            "--expected-authorization-sha256",
            "--authorization-nonce",
            "--next-genesis-hash",
        ] {
            let mut missing = arguments.clone();
            let index = missing.iter().position(|value| *value == required).unwrap();
            missing.drain(index..index + 2);
            assert!(crate::Args::try_parse_from(missing).is_err(), "{required}");
        }
    }

    #[test]
    fn installed_launcher_parses_exact_daemon_assignment() {
        let slug = "taira-validator-1";
        let commit = "a".repeat(40);
        let unit = format!(
            "[Service]\nExecStart=/usr/bin/python3 -c \"import os\\ncmd = ['/private/runtime/taira-public-reset/release-{commit}-update-{}/bin/iroha3d_taira', '--config', '/srv/taira/{slug}/current/config/config.toml', '--sora']\\nreserved_fds = (198, 199)\"\n",
            "b".repeat(32),
        );
        assert_eq!(
            daemon_in_unit(unit.as_bytes(), slug, &commit)
                .unwrap()
                .commit,
            commit
        );
        assert!(
            daemon_in_unit(unit.replace("--sora", "--other").as_bytes(), slug, &commit).is_err()
        );
        assert!(daemon_in_unit(format!("{unit}{unit}").as_bytes(), slug, &commit).is_err());
        assert!(
            daemon_in_unit(unit.replace("cmd =", "other =").as_bytes(), slug, &commit).is_err()
        );
        assert!(
            daemon_in_unit(
                unit.replace("/bin/iroha3d_taira", "/foreign/bin/iroha3d_taira")
                    .as_bytes(),
                slug,
                &commit
            )
            .is_err()
        );
    }

    #[test]
    fn installed_launcher_binds_native_beacon_config_and_selected_daemon() {
        let slug = "taira-validator-1";
        let commit = "a".repeat(40);
        let unit = format!(
            "[Service]\nExecStart=/usr/bin/python3 -c \"import os\\ncmd = ['/srv/taira/{slug}/current/bin/iroha3d_taira', '--config', '/srv/taira/{slug}/current/config/beacon.toml', '--sora']\\nreserved_fds = (198, 199)\"\n"
        );
        let parsed = daemon_in_unit(unit.as_bytes(), slug, &commit).unwrap();
        assert_eq!(
            parsed.argv0,
            format!("/srv/taira/{slug}/current/bin/iroha3d_taira")
        );
        assert_eq!(
            parsed.artifact_path,
            format!("/srv/taira/{slug}/releases/{commit}/bin/iroha3d_taira")
        );
        assert_eq!(parsed.commit, commit);
        assert_eq!(parsed.config_name, "beacon.toml");
        for foreign in [
            "foreign.toml",
            "../beacon.toml",
            "beacon.toml/other",
            "beacon.toml', '--config', '/tmp/foreign.toml",
        ] {
            assert!(
                daemon_in_unit(
                    unit.replace("beacon.toml", foreign).as_bytes(),
                    slug,
                    &commit
                )
                .is_err()
            );
        }
        assert!(
            daemon_in_unit(
                unit.replace(slug, "taira-validator-2").as_bytes(),
                slug,
                &commit
            )
            .is_err()
        );
        assert!(daemon_in_unit(unit.as_bytes(), slug, "not-a-revision").is_err());
    }
}
