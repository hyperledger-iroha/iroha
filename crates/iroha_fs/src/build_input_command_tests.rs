//! Genuine tool-image controls for retained original selection, without release claims.

use super::*;
#[cfg(any(target_os = "linux", target_os = "android"))]
use crate::FileMetadata;
#[cfg(windows)]
use crate::ProcessStdio;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

static INSTALLER: OnceLock<PathBuf> = OnceLock::new();

fn images() -> &'static [Vec<u8>; 2] {
    static IMAGES: OnceLock<[Vec<u8>; 2]> = OnceLock::new();
    IMAGES.get_or_init(|| {
        let root = tempfile::tempdir().unwrap();
        let bytes = ["original", "competing"].map(|label| {
            let source = root.path().join(format!("{label}.rs"));
            let output = root
                .path()
                .join(format!("{label}{}", std::env::consts::EXE_SUFFIX));
            fs::write(
                &source,
                r#"//! Actual native original-selection probe.
use std::io::{self, Read as _, Write as _};
fn main() -> io::Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("install-image") => {
            let mut output=std::fs::File::create(std::env::args_os().nth(2).unwrap())?;
            io::copy(&mut io::stdin(), &mut output)?;
            #[cfg(unix)] {
                use std::os::unix::fs::PermissionsExt as _;
                output.set_permissions(std::fs::Permissions::from_mode(0o700))?;
            }
        }
        Some("argv0") => println!("@LABEL@\n{}", std::env::args().next().unwrap()),
        Some("launch-nested") => {
            println!("ready"); io::stdout().flush()?;
            let mut input=Vec::new(); io::stdin().read_to_end(&mut input)?;
            for key in ["NESTED_TOOL", "SECOND_TOOL"] {
                if let Some(path)=std::env::var_os(key) {
                    let output=std::process::Command::new(path).output()?;
                    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
                    io::stdout().write_all(&output.stdout)?;
                }
            }
        }
        #[cfg(unix)]
        Some("check-unrelated-inheritance") => {
            use std::os::unix::fs::MetadataExt as _;
            let expected=std::env::var("EXPECTED_ID").unwrap();
            let inherited=std::fs::metadata(std::env::var_os("NESTED_TOOL").unwrap())
                .is_ok_and(|metadata| format!("{}:{}", metadata.dev(), metadata.ino())==expected);
            assert!(!inherited, "unrelated launcher inherited the retained tool");
            println!("not inherited");
        }
        _ => println!("@LABEL@"),
    }
    Ok(())
}
"#
                .replace("@LABEL@", label),
            )
            .unwrap();
            let compiled = std::process::Command::new(
                std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),
            )
            .args(["--edition=2024", "-C", "debuginfo=0"])
            .arg(&source)
            .arg("-o")
            .arg(&output)
            .output()
            .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            fs::read(output).unwrap()
        });
        INSTALLER
            .set(
                root.path()
                    .join(format!("original{}", std::env::consts::EXE_SUFFIX)),
            )
            .unwrap();
        let _ = root.keep();
        bytes
    })
}

fn install(path: &Path, bytes: &[u8]) {
    use std::io::Write as _;
    let _ = images();
    // Only this actual fixture child owns a writable image handle. Parallel test
    // forks in the parent cannot inherit that writer and cause incidental ETXTBSY.
    let mut command = ProcessCommand::new(INSTALLER.get().unwrap()).unwrap();
    command
        .args([std::ffi::OsStr::new("install-image"), path.as_os_str()])
        .stdin(crate::ProcessStdio::Piped)
        .stdout(crate::ProcessStdio::Null)
        .stderr(crate::ProcessStdio::Piped);
    let mut child = command.spawn().unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(any(windows, target_os = "linux", target_os = "android"))]
#[test]
fn original_build_input_command_runs_genuine_image_without_changing_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let path = root
        .path()
        .join(format!("tool{}", std::env::consts::EXE_SUFFIX));
    install(&path, &images()[0]);
    fs::hard_link(&path, root.path().join("cargo-output-alias")).unwrap();
    let original = Arc::new(RetainedBuildInput::open(&path).unwrap());
    let snapshot = original.snapshot().unwrap();
    let output = original.command().unwrap().output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"original\n");
    assert!(output.stderr.is_empty());
    original.revalidate().unwrap();
    assert_eq!(original.snapshot().unwrap(), snapshot);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn renamed_and_restored_ancestor_cannot_redirect_the_original_tool_command() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("tools");
    let displaced = root.path().join("held-original-tools");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("tool");
    install(&path, &images()[0]);
    let original = Arc::new(RetainedBuildInput::open(&path).unwrap());
    let snapshot = original.snapshot().unwrap();
    let identity = FileMetadata::of(&fs::File::open(&path).unwrap())
        .unwrap()
        .identity();
    // Both commands are configured under the correct original name. The race is
    // after their initial admission and before the kernel chooses the executable.
    let mut retained_command = original.command().unwrap();
    retained_command.arg("argv0");
    let mut path_command = ProcessCommand::new(&path).unwrap();
    path_command.arg("argv0");
    fs::rename(&parent, &displaced).unwrap();
    fs::create_dir(&parent).unwrap();
    install(&path, &images()[1]);
    assert_ne!(
        FileMetadata::of(&fs::File::open(&path).unwrap())
            .unwrap()
            .identity(),
        identity
    );
    let redirected = path_command.output().unwrap();
    let selected = retained_command.output().unwrap();
    assert!(redirected.status.success());
    assert!(selected.status.success());
    assert_eq!(
        redirected.stdout,
        format!("competing\n{}\n", path.display()).as_bytes()
    );
    assert_eq!(
        selected.stdout,
        format!("original\n{}\n", path.display()).as_bytes()
    );
    fs::remove_dir_all(&parent).unwrap();
    fs::rename(&displaced, &parent).unwrap();
    assert_eq!(
        FileMetadata::of(&fs::File::open(&path).unwrap())
            .unwrap()
            .identity(),
        identity
    );
    // Restoring only the ancestor leaves the file snapshot unchanged. This is
    // exactly the race which surrounding before/after tool checks cannot exclude.
    original.revalidate().unwrap();
    assert_eq!(original.snapshot().unwrap(), snapshot);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn command_keeps_original_descriptor_alive_until_spawn_after_caller_drop() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tool");
    install(&path, &images()[0]);
    let original = Arc::new(RetainedBuildInput::open(&path).unwrap());
    let weak = Arc::downgrade(&original);
    let command = original.command().unwrap();
    drop(original);
    assert!(weak.upgrade().is_some());
    let output = command.output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"original\n");
    assert!(weak.upgrade().is_none());
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn scripts_and_changed_originals_do_not_gain_descriptor_launch() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tool");
    install(&path, b"#!/bin/sh\nprintf not-a-native-image\n");
    let script = Arc::new(RetainedBuildInput::open(&path).unwrap());
    assert!(script.command().is_err());
    drop(script);
    install(&path, &images()[0]);
    let original = Arc::new(RetainedBuildInput::open(&path).unwrap());
    install(&path, &images()[1]);
    assert!(original.command().is_err());
}

#[cfg(windows)]
#[test]
fn original_windows_tool_sharing_remains_live_through_command_execution() {
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("tools");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("tool.exe");
    install(&path, &images()[0]);
    let original = Arc::new(RetainedBuildInput::open(&path).unwrap());
    let snapshot = original.snapshot().unwrap();
    let mut command = original.command().unwrap();
    assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
    assert!(fs::rename(&parent, root.path().join("away")).is_err());
    assert!(fs::rename(&path, parent.join("away.exe")).is_err());
    command
        .arg("launch-nested")
        .stdin(ProcessStdio::Null)
        .stdout(ProcessStdio::Piped);
    command
        .env_retained_build_input("NESTED_TOOL", &original)
        .unwrap();
    let output = command.spawn().unwrap().wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"ready\noriginal\n");
    assert_eq!(original.snapshot().unwrap(), snapshot);
}

#[cfg(any(windows, target_os = "linux", target_os = "android"))]
#[test]
fn invalid_nested_tool_environment_names_are_refused_without_configuration() {
    let root = tempfile::tempdir().unwrap();
    let path = root
        .path()
        .join(format!("tool{}", std::env::consts::EXE_SUFFIX));
    install(&path, &images()[0]);
    let original = Arc::new(RetainedBuildInput::open(&path).unwrap());
    let mut command = original.command().unwrap();
    for key in ["", "KEY=VALUE", "KEY\0TAIL"] {
        assert_eq!(
            command
                .env_retained_build_input(key, &original)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(command.get_envs().count(), 0);
    }
    original.revalidate().unwrap();
}

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn nested_original_tools_survive_exec_and_ancestor_swap_without_parent_inheritance() {
    use crate::ProcessStdio;
    use std::io::{BufRead as _, Read as _};
    use std::os::unix::fs::MetadataExt as _;
    let root = tempfile::tempdir().unwrap();
    let parent = root.path().join("tools");
    let displaced = root.path().join("held-tools");
    fs::create_dir(&parent).unwrap();
    let path = parent.join("tool");
    install(&path, &images()[0]);
    let second_path = root.path().join("second-tool");
    install(&second_path, &images()[1]);
    let launcher = root.path().join("launcher");
    install(&launcher, &images()[0]);
    let original = Arc::new(RetainedBuildInput::open(&path).unwrap());
    let second = Arc::new(RetainedBuildInput::open(&second_path).unwrap());
    let snapshot = original.snapshot().unwrap();
    let mut retained = ProcessCommand::new(&launcher).unwrap();
    retained
        .arg("launch-nested")
        .stdin(ProcessStdio::Piped)
        .stdout(ProcessStdio::Piped)
        .kill_on_drop(true);
    retained
        .env_retained_build_input("NESTED_TOOL", &original)
        .unwrap();
    retained
        .env_retained_build_input("SECOND_TOOL", &second)
        .unwrap();
    let values = retained
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.unwrap().to_owned()))
        .collect::<Vec<_>>();
    assert_ne!(values[0].1, values[1].1);
    let descriptor_path = values
        .iter()
        .find(|(name, _)| name == "NESTED_TOOL")
        .unwrap()
        .1
        .clone();
    let metadata = fs::metadata(&path).unwrap();
    let unrelated = std::process::Command::new(&launcher)
        .arg("check-unrelated-inheritance")
        .env("NESTED_TOOL", &descriptor_path)
        .env(
            "EXPECTED_ID",
            format!("{}:{}", metadata.dev(), metadata.ino()),
        )
        .output()
        .unwrap();
    assert!(
        unrelated.status.success(),
        "{}",
        String::from_utf8_lossy(&unrelated.stderr)
    );
    assert_eq!(unrelated.stdout, b"not inherited\n");
    let mut ordinary = ProcessCommand::new(&launcher).unwrap();
    ordinary
        .arg("launch-nested")
        .env("NESTED_TOOL", &path)
        .stdin(ProcessStdio::Piped)
        .stdout(ProcessStdio::Piped)
        .kill_on_drop(true);
    let mut kept_child = retained.spawn().unwrap();
    let mut path_child = ordinary.spawn().unwrap();
    let mut kept_output = std::io::BufReader::new(kept_child.stdout.take().unwrap());
    let mut path_output = std::io::BufReader::new(path_child.stdout.take().unwrap());
    for output in [&mut kept_output, &mut path_output] {
        let mut ready = String::new();
        output.read_line(&mut ready).unwrap();
        assert_eq!(ready, "ready\n");
    }
    fs::rename(&parent, &displaced).unwrap();
    fs::create_dir(&parent).unwrap();
    install(&path, &images()[1]);
    drop(kept_child.stdin.take());
    drop(path_child.stdin.take());
    let mut selected = Vec::new();
    let mut redirected = Vec::new();
    kept_output.read_to_end(&mut selected).unwrap();
    path_output.read_to_end(&mut redirected).unwrap();
    assert!(kept_child.wait().unwrap().success());
    assert!(path_child.wait().unwrap().success());
    assert_eq!(selected, b"original\ncompeting\n");
    assert_eq!(redirected, b"competing\n");
    fs::remove_dir_all(&parent).unwrap();
    fs::rename(&displaced, &parent).unwrap();
    original.revalidate().unwrap();
    second.revalidate().unwrap();
    assert_eq!(original.snapshot().unwrap(), snapshot);
}

#[cfg(target_os = "linux")]
#[test]
fn fixed_descriptor_mapping_refuses_to_replace_an_owned_nested_tool() {
    use std::os::fd::AsFd as _;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("tool");
    install(&path, &images()[0]);
    let original = Arc::new(RetainedBuildInput::open(&path).unwrap());
    let snapshot = original.snapshot().unwrap();
    let mut command = original.command().unwrap();
    command
        .env_retained_build_input("NESTED_TOOL", &original)
        .unwrap();
    let target = command
        .get_envs()
        .find(|(key, _)| *key == "NESTED_TOOL")
        .unwrap()
        .1
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("/proc/self/fd/")
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let competing = fs::File::open(&path).unwrap();
    command
        .inherit_descriptors(&[(competing.as_fd(), target)])
        .unwrap();
    // The owned duplicate was allocated before std's private error channel. The
    // occupied-target refusal must reach the parent through that intact channel.
    let error = command.output().unwrap_err();
    assert_eq!(
        error.raw_os_error(),
        Some(rustix::io::Errno::BUSY.raw_os_error())
    );
    assert_eq!(original.snapshot().unwrap(), snapshot);
}

#[cfg(target_os = "linux")]
#[test]
fn actual_cargo_invokes_the_inherited_original_native_rustc() {
    fn actual_tool(name: &str) -> std::path::PathBuf {
        // Test-only toolchain discovery. Production receives explicit independently
        // admitted tools; this diagnostic confers no release toolchain provenance.
        let output = std::process::Command::new("rustup")
            .args(["which", name])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::path::PathBuf::from(std::str::from_utf8(&output.stdout).unwrap().trim())
    }
    let cargo = Arc::new(RetainedBuildInput::open(actual_tool("cargo")).unwrap());
    let rustc = Arc::new(RetainedBuildInput::open(actual_tool("rustc")).unwrap());
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::create_dir(root.path().join("home")).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname='nested_tool_probe'\nversion='0.0.0'\nedition='2024'\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        root.path().join("Cargo.lock"),
        "version = 4\n[[package]]\nname = \"nested_tool_probe\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(root.path().join("src/main.rs"), "//! Genuine Cargo-to-rustc control.\nfn main(){println!(\"original compiler reached\");}\n").unwrap();
    fs::write(root.path().join("build.rs"), r#"//! Actual Cargo descendant binding control.
fn main() {
    for (key, version) in [("RUSTC", "-vV")] {
        let selected=std::env::var(key).unwrap();
        assert!(selected.starts_with("/proc/self/fd/"), "{key} lost original descriptor selection: {selected}");
        let output=std::process::Command::new(&selected).arg(version).output().unwrap();
        assert!(output.status.success(), "{key}: {}", String::from_utf8_lossy(&output.stderr));
    }
}
"#).unwrap();
    let mut command = cargo.command().unwrap();
    command
        .args(["build", "--offline", "--locked"])
        .current_dir(root.path())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("CARGO_HOME", root.path().join("home"))
        .env("CARGO_TARGET_DIR", root.path().join("target"));
    command.env_retained_build_input("RUSTC", &rustc).unwrap();
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    cargo.revalidate().unwrap();
    rustc.revalidate().unwrap();
    let output = std::process::Command::new(root.path().join("target/debug/nested_tool_probe"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"original compiler reached\n");
}
