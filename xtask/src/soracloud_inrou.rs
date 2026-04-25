use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

pub enum CommandMode {
    Portable,
    Firecracker,
    MixedHost { inventory: PathBuf },
}

#[derive(Debug, Deserialize)]
struct MixedHostInventory {
    portable_host: RemoteSmokeHost,
    firecracker_host: RemoteSmokeHost,
    proxy_only_host: RemoteSmokeHost,
    status_gate: Option<RemoteSmokeCommand>,
}

#[derive(Debug, Deserialize)]
struct RemoteSmokeHost {
    ssh_target: String,
    repo_path: PathBuf,
    command: Option<String>,
    env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct RemoteSmokeCommand {
    ssh_target: String,
    repo_path: PathBuf,
    command: String,
    env: Option<BTreeMap<String, String>>,
}

pub fn run(mode: CommandMode) -> Result<(), Box<dyn Error>> {
    match mode {
        CommandMode::Portable => run_portable_local(),
        CommandMode::Firecracker => run_local_script("run_inrou_linux_kvm_smoke.sh"),
        CommandMode::MixedHost { inventory } => run_mixed_host(&inventory),
    }
}

fn run_portable_local() -> Result<(), Box<dyn Error>> {
    require_portable_guest_asset_env()?;
    run_cargo_smoke_command(
        &[
            "test",
            "--locked",
            "-p",
            "irohad",
            "--features",
            "embedded-soracloud-runtime",
            "--bin",
            "irohad",
            "build_inrou_user_data_projects_portable_block_mounts_and_allowlist_overlay",
            "--",
            "--nocapture",
        ],
        &[("IROHA_RUN_IGNORED", "1"), ("IROHA_INROU_PORTABLE", "1")],
    )?;
    if !cfg!(windows) {
        run_cargo_smoke_command(
            &[
                "test",
                "--locked",
                "-p",
                "irohad",
                "--features",
                "embedded-soracloud-runtime",
                "--bin",
                "irohad",
                "ensure_inrou_portable_root_disk_uses_qcow2_overlay_with_backing_file",
                "--",
                "--nocapture",
            ],
            &[("IROHA_RUN_IGNORED", "1"), ("IROHA_INROU_PORTABLE", "1")],
        )?;
    }
    run_cargo_smoke_command(
        &[
            "test",
            "--locked",
            "-p",
            "irohad",
            "--features",
            "embedded-soracloud-runtime",
            "--bin",
            "irohad",
            "inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck",
            "--",
            "--ignored",
            "--nocapture",
        ],
        &[("IROHA_RUN_IGNORED", "1"), ("IROHA_INROU_PORTABLE", "1")],
    )?;
    Ok(())
}

fn require_portable_guest_asset_env() -> Result<(), Box<dyn Error>> {
    for name in [
        "IROHA_INROU_PORTABLE_KERNEL_IMAGE",
        "IROHA_INROU_PORTABLE_ROOTFS_IMAGE",
    ] {
        let value = env::var(name).map_err(|_| portable_guest_asset_hint(name))?;
        if !Path::new(&value).is_file() {
            return Err(format!(
                "{name} must point to an existing file, got {value}. {}",
                portable_guest_asset_prepare_hint()
            )
            .into());
        }
    }
    if let Ok(value) = env::var("IROHA_INROU_PORTABLE_INITRD_IMAGE")
        && !value.trim().is_empty()
        && !Path::new(&value).is_file()
    {
        return Err(format!(
            "IROHA_INROU_PORTABLE_INITRD_IMAGE must point to an existing file, got {value}. {}",
            portable_guest_asset_prepare_hint()
        )
        .into());
    }
    Ok(())
}

fn portable_guest_asset_hint(name: &str) -> String {
    format!(
        "{name} must point to an existing file. {}",
        portable_guest_asset_prepare_hint()
    )
}

fn portable_guest_asset_prepare_hint() -> &'static str {
    "Prepare Debian genericcloud guest assets with `eval \"$(python3 scripts/ci/prepare_inrou_portable_guest_assets.py --print-env)\"`."
}

fn run_local_script(script_name: &str) -> Result<(), Box<dyn Error>> {
    let script_path = crate::workspace_root().join("scripts/ci").join(script_name);
    if !script_path.is_file() {
        return Err(format!("missing smoke script `{}`", script_path.display()).into());
    }
    let status = Command::new("bash").arg(&script_path).status()?;
    if status.success() {
        return Ok(());
    }
    Err(format!("{} failed with status {status}", script_path.display()).into())
}

fn run_cargo_smoke_command(args: &[&str], env: &[(&str, &str)]) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("cargo");
    command.args(args).current_dir(crate::workspace_root());
    for (key, value) in env {
        command.env(key, value);
    }
    let status = command.status()?;
    if status.success() {
        return Ok(());
    }
    Err(format!("cargo {} failed with status {status}", args.join(" ")).into())
}

fn run_mixed_host(inventory_path: &Path) -> Result<(), Box<dyn Error>> {
    let inventory_raw = fs::read_to_string(inventory_path)?;
    let inventory: MixedHostInventory = toml::from_str(&inventory_raw)?;

    run_remote_host_command(
        "portable_host",
        &inventory.portable_host.ssh_target,
        &inventory.portable_host.repo_path,
        inventory
            .portable_host
            .command
            .as_deref()
            .unwrap_or("cargo xtask soracloud-inrou-smoke portable"),
        inventory.portable_host.env.as_ref(),
    )?;
    run_remote_host_command(
        "firecracker_host",
        &inventory.firecracker_host.ssh_target,
        &inventory.firecracker_host.repo_path,
        inventory
            .firecracker_host
            .command
            .as_deref()
            .unwrap_or("cargo xtask soracloud-inrou-smoke firecracker"),
        inventory.firecracker_host.env.as_ref(),
    )?;
    run_remote_host_command(
        "proxy_only_host",
        &inventory.proxy_only_host.ssh_target,
        &inventory.proxy_only_host.repo_path,
        inventory
            .proxy_only_host
            .command
            .as_deref()
            .unwrap_or(
                "cargo check -p irohad --features embedded-soracloud-runtime --bin irohad --message-format short",
            ),
        inventory.proxy_only_host.env.as_ref(),
    )?;
    if let Some(status_gate) = inventory.status_gate.as_ref() {
        run_remote_host_command(
            "status_gate",
            &status_gate.ssh_target,
            &status_gate.repo_path,
            &status_gate.command,
            status_gate.env.as_ref(),
        )?;
    }
    Ok(())
}

fn run_remote_host_command(
    label: &str,
    ssh_target: &str,
    repo_path: &Path,
    command: &str,
    env: Option<&BTreeMap<String, String>>,
) -> Result<(), Box<dyn Error>> {
    let env_prefix = env
        .map(|vars| {
            vars.iter()
                .map(|(key, value)| format!("{key}={}", shell_single_quote(value)))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|value| !value.is_empty())
        .map(|value| format!("{value} "))
        .unwrap_or_default();
    let remote_command = format!(
        "set -euo pipefail && cd {} && {}{}",
        shell_single_quote(&repo_path.display().to_string()),
        env_prefix,
        command
    );
    let status = Command::new("ssh")
        .arg(ssh_target)
        .arg(remote_command)
        .status()?;
    if status.success() {
        return Ok(());
    }
    Err(format!("{label} remote smoke command failed with status {status}").into())
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}
