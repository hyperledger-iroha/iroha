//! Read-only capture of the installed, stopped predecessor runtime.
//!
//! The selected configuration and the loaded unit are separate authorities: the
//! selector identifies configuration, while the installed unit identifies the
//! daemon actually selected by systemd. Neither is inferred from a prior plan.
use super::*;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

const HOST_PUBLIC_KEY: &str = "/etc/ssh/ssh_host_ed25519_key.pub";
const UNIT_PROPERTIES: &str = "LoadState,FragmentPath,DropInPaths,NeedDaemonReload,ActiveState,SubState,MainPID,ControlPID,Job";

/// Run on the approved Linux guest through its pinned SSH route, after stopping validators.
#[derive(clap::Args, Debug)]
pub(in super::super::super::super) struct CaptureDispatcherCurrentRuntime {
    /// SHA-256 of `ssh-ed25519 <base64>` from the independently approved host key.
    #[arg(long)]
    expected_host_identity_sha256: String,
    /// A fresh file in an existing root-owned mode0700 directory.
    #[arg(long)]
    output: PathBuf,
}

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

fn unit_properties(unit: &str) -> Result<BTreeMap<String, String>> {
    let bytes = super::super::super::run_host_command(
        super::super::super::SYSTEMCTL,
        &["show", "--all", &format!("--property={UNIT_PROPERTIES}"), unit],
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

fn require_unit(unit: &str, fragment: &str, running: bool) -> Result<()> {
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
    if running {
        need(
            fields.get("ActiveState").map(String::as_str) == Some("active")
                && fields.get("SubState").map(String::as_str) == Some("running")
                && fields
                    .get("MainPID")
                    .is_some_and(|pid| pid.parse::<u32>().is_ok_and(|pid| pid > 1)),
            "edge unit is not running",
        )
    } else {
        need(
            fields.get("ActiveState").map(String::as_str) == Some("inactive")
                && fields.get("SubState").map(String::as_str) == Some("dead")
                && fields.get("MainPID").map(String::as_str) == Some("0"),
            "validator unit must be stopped before runtime capture",
        )
    }
}

fn selected_release(slug: &str) -> Result<(String, String)> {
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
fn daemon_in_unit(bytes: &[u8], slug: &str) -> Result<(String, String)> {
    let text = std::str::from_utf8(bytes)?;
    let config = format!("/srv/taira/{slug}/current/config/config.toml");
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
    need(
        tail == format!("{config}', '--sora']"),
        "installed daemon argv differs",
    )?;
    let prefix = "/private/runtime/taira-public-reset/release-";
    let release = daemon
        .strip_prefix(prefix)
        .ok_or_else(|| eyre!("daemon is outside the installed update namespace"))?;
    let (commit, suffix) = release
        .split_once('-')
        .ok_or_else(|| eyre!("daemon update revision is absent"))?;
    validate_lower_hex("installed daemon revision", commit, 40)?;
    need(
        suffix.starts_with("update-")
            && suffix.ends_with("/bin/iroha3d_taira")
            && suffix["update-".len()..]
                .split('/')
                .next()
                .is_some_and(|operation| {
                    operation.len() == 32
                        && operation
                            .bytes()
                            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
                }),
        "installed daemon path is not a bounded update release",
    )?;
    validate_absolute_normal_path(Path::new(daemon), "installed daemon path")?;
    Ok((daemon.to_owned(), commit.to_owned()))
}

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

fn capture_validator(slug: &str, observed: &mut Observed) -> Result<ValidatorAdmittedReleaseV1> {
    let (release_root, commit) = selected_release(slug)?;
    let unit = format!("iroha3d-{slug}.service");
    let unit_path = format!("/etc/systemd/system/{unit}");
    require_unit(&unit, &unit_path, false)?;
    let unit_pin = observed.pin(Path::new(&unit_path), Some(0o644), 16 * 1024 * 1024)?;
    let (daemon, daemon_commit) = daemon_in_unit(&admission::read(&unit_pin)?, slug)?;
    let artifacts = vec![
        artifact(observed, "iroha3d", daemon.clone(), &daemon_commit)?,
        artifact(
            observed,
            "config",
            format!("{release_root}/config/config.toml"),
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
        artifact(observed, "validator_unit", unit_path, &daemon_commit)?,
    ];
    let state = format!("/var/lib/taira/{slug}");
    require_root_directory(Path::new(&state), true, "stopped validator state")?;
    let metadata = fs::symlink_metadata(state)?;
    let prior = ValidatorAdmittedReleaseV1 {
        commit,
        release_root,
        argv: vec![
            daemon,
            "--config".into(),
            format!("/srv/taira/{slug}/current/config/config.toml"),
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

fn capture_edge(observed: &mut Observed) -> Result<EdgeAdmittedReleaseV1> {
    let (release_root, commit) = selected_release("edge")?;
    require_unit("nginx.service", "/etc/systemd/system/nginx.service", true)?;
    let cli = observed.pin(
        &Path::new(&release_root).join("bin/iroha"),
        Some(0o755),
        MAX_BINARY,
    )?;
    let config = observed.pin(
        &Path::new(&release_root).join("taira.conf"),
        Some(0o640),
        MAX_BINARY,
    )?;
    let installed = observed.pin(
        Path::new("/etc/nginx/conf.d/taira.conf"),
        Some(0o640),
        MAX_BINARY,
    )?;
    need(
        installed.sha256 == config.sha256 && installed.size == config.size,
        "installed edge configuration differs from selected release",
    )?;
    let _unit = observed.pin(
        Path::new("/etc/systemd/system/nginx.service"),
        Some(0o644),
        MAX_BINARY,
    )?;
    Ok(EdgeAdmittedReleaseV1 {
        commit,
        release_root,
        cli_sha256: cli.sha256,
        config_sha256: config.sha256,
    })
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
            let validators = SLUGS[..4]
                .iter()
                .map(|slug| capture_validator(slug, &mut observed))
                .collect::<Result<Vec<_>>>()?;
            let edge = capture_edge(&mut observed)?;
            let runtime = CurrentRuntime {
                schema: "iroha.taira.dispatcher-current-runtime.v1".into(),
                host_identity_sha256: self.expected_host_identity_sha256.clone(),
                validators,
                edge,
            };
            validate_runtime(&runtime)?;
            let _roles = runtime_roles(&runtime, &mut observed)?;
            locks.revalidate()?;
            observed.revalidate()?;
            for slug in &SLUGS[..4] {
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
                require_unit(&unit, &format!("/etc/systemd/system/{unit}"), false)?;
            }
            need(
                selected_release("edge")?.0 == runtime.edge.release_root,
                "edge selector moved during capture",
            )?;
            require_unit("nginx.service", "/etc/systemd/system/nginx.service", true)?;
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
    fn installed_launcher_parses_exact_daemon_assignment() {
        let slug = "taira-validator-1";
        let commit = "a".repeat(40);
        let unit = format!(
            "[Service]\nExecStart=/usr/bin/python3 -c \"import os\\ncmd = ['/private/runtime/taira-public-reset/release-{commit}-update-{}/bin/iroha3d_taira', '--config', '/srv/taira/{slug}/current/config/config.toml', '--sora']\\nreserved_fds = (198, 199)\"\n",
            "b".repeat(32),
        );
        assert_eq!(daemon_in_unit(unit.as_bytes(), slug).unwrap().1, commit);
        assert!(daemon_in_unit(unit.replace("--sora", "--other").as_bytes(), slug).is_err());
        assert!(daemon_in_unit(format!("{unit}{unit}").as_bytes(), slug).is_err());
        assert!(daemon_in_unit(unit.replace("cmd =", "other =").as_bytes(), slug).is_err());
    }
}
