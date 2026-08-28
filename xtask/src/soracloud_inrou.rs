use std::{env, error::Error, path::Path, process::Command};
pub enum CommandMode {
    Portable,
}
pub fn run(mode: CommandMode) -> Result<(), Box<dyn Error>> {
    match mode {
        CommandMode::Portable => run_portable_local(),
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
            "--bin",
            "iroha3d",
            "build_inrou_user_data_projects_isolated_portable_block_mounts",
            "--",
            "--nocapture",
        ],
        &[],
    )?;
    if !cfg!(windows) {
        run_cargo_smoke_command(
            &[
                "test",
                "--locked",
                "-p",
                "irohad",
                "--bin",
                "iroha3d",
                "ensure_inrou_portable_root_disk_is_a_standalone_authenticated_copy",
                "--",
                "--nocapture",
            ],
            &[],
        )?;
        run_cargo_smoke_command(
            &[
                "test",
                "--locked",
                "-p",
                "irohad",
                "--bin",
                "iroha3d",
                "ensure_inrou_portable_lease_disks_create_reusable_raw_images",
                "--",
                "--nocapture",
            ],
            &[],
        )?;
    }
    run_cargo_smoke_command(
        &[
            "test",
            "--locked",
            "-p",
            "irohad",
            "--bin",
            "iroha3d",
            "inrou_portable_smoke_boots_debian_guest_and_serves_healthcheck",
            "--",
            "--ignored",
            "--nocapture",
        ],
        &[],
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
