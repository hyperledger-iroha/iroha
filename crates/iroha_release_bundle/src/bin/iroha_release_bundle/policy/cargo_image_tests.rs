//! Genuine source/image custody and recursive Cargo controls; no release qualification.

use super::super::Tool;
use super::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn private(root: &Path, name: &str) -> PrivateDirectory {
    PrivateDirectory::open_or_create(root.join(name)).unwrap()
}

fn fixture_tool(root: &Path) -> Tool {
    let source = private(root, "installed");
    let path = source.path().join("cargo");
    fs::copy(std::env::current_exe().unwrap(), &path).unwrap();
    Tool::open(&path).unwrap()
}

fn observed_image(
    tool: &mut Tool,
    parent: &PrivateDirectory,
    name: &str,
    observe: impl FnMut(Step) -> io::Result<()>,
) -> io::Result<Arc<Image>> {
    publish(
        Arc::get_mut(&mut tool.original).unwrap(),
        tool.snapshot,
        &tool.evidence,
        parent,
        name,
        observe,
    )
}

#[test]
fn copied_cargo_is_a_distinct_exact_immutable_original() {
    let root = tempfile::tempdir().unwrap();
    let parent = private(root.path(), "published");
    let tool = fixture_tool(root.path());
    let source = tool.snapshot;
    let expected = tool.evidence.clone();
    let tool = tool.publish_cargo_image(&parent, "invocation").unwrap();
    let image = tool.cargo_image.as_ref().unwrap();
    assert_eq!(
        image.batch.entry_path(0).unwrap().file_name().unwrap(),
        IMAGE
    );
    assert_ne!(
        image.batch.entry_snapshot(0).unwrap().local_parts().1,
        source.local_parts().1
    );
    assert_ne!(
        image.executable.identity().unwrap(),
        image.batch.entry_identity(0).unwrap()
    );
    assert_eq!(tool.original.snapshot().unwrap(), source);
    let mut reader = image.batch.reader(0).unwrap();
    assert_eq!(
        super::super::digest_original(&mut reader, MAXIMUM as u64).unwrap(),
        expected.sha256
    );
    assert_eq!(tool.evidence().unwrap().bytes, expected.bytes);
    tool.revalidate().unwrap();
}

#[test]
fn existing_cargo_image_namespace_is_never_reused() {
    let root = tempfile::tempdir().unwrap();
    let parent = private(root.path(), "published");
    let mut tool = fixture_tool(root.path());
    let image = observed_image(&mut tool, &parent, "invocation", |_| Ok(())).unwrap();
    let id = image.batch.entry_identity(0).unwrap();
    assert!(observed_image(&mut tool, &parent, "invocation", |_| Ok(())).is_err());
    assert_eq!(image.batch.entry_identity(0).unwrap(), id);
    tool.revalidate().unwrap();
}

#[test]
fn cargo_copy_and_sealing_failures_preserve_claimed_names_and_source() {
    let root = tempfile::tempdir().unwrap();
    let parent = private(root.path(), "published");
    let mut tool = fixture_tool(root.path());
    for (name, selected) in [
        ("partial-copy", Step::CopyChunk(16 * 1024)),
        ("copy", Step::Copied),
        ("seal", Step::Sealing(FlatBatchSealStep::FileSealed(0))),
        ("final", Step::Authenticated),
        ("memory", Step::MemorySealed),
    ] {
        let error = observed_image(&mut tool, &parent, name, |step| {
            if step == selected {
                Err(io::Error::other("injected publication failure"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(error.to_string(), "injected publication failure");
        assert!(parent.path().join(name).join(IMAGE).is_file());
        assert_eq!(tool.original.snapshot().unwrap(), tool.snapshot);
        tool.revalidate().unwrap();
        assert!(observed_image(&mut tool, &parent, name, |_| Ok(())).is_err());
    }
}

#[test]
fn cargo_image_refuses_wrong_byte_and_extent_anchors() {
    let root = tempfile::tempdir().unwrap();
    let parent = private(root.path(), "published");
    let mut tool = fixture_tool(root.path());
    let expected = tool.evidence.clone();
    tool.evidence.sha256 = "0".repeat(64);
    let error = observed_image(&mut tool, &parent, "wrong-bytes", |_| Ok(())).unwrap_err();
    assert!(error.to_string().contains("copied bytes differ"));
    assert!(parent.path().join("wrong-bytes").join(IMAGE).is_file());
    tool.evidence = expected;
    tool.evidence.bytes -= 1;
    assert!(observed_image(&mut tool, &parent, "wrong-extent", |_| Ok(())).is_err());
    assert!(!parent.path().join("wrong-extent").exists());
    assert_eq!(tool.original.snapshot().unwrap(), tool.snapshot);
}

#[test]
fn cargo_source_drift_is_checked_after_an_initiating_publication_error() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = tempfile::tempdir().unwrap();
    let parent = private(root.path(), "published");
    let mut tool = fixture_tool(root.path());
    let source = root.path().join("installed/cargo");
    let error = observed_image(&mut tool, &parent, "drift", |step| {
        if step == Step::Copied {
            fs::set_permissions(&source, fs::Permissions::from_mode(0o600))?;
            return Err(io::Error::other("first copy observer error"));
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error.to_string(), "first copy observer error");
    assert!(tool.revalidate().is_err());
    assert!(parent.path().join("drift").join(IMAGE).exists());
}

#[test]
#[ignore = "original child invoked by cargo_image_wait_error_retains_both_originals"]
fn cargo_image_child() {
    use std::io::Write as _;
    let selected = std::env::var("CARGO").unwrap();
    assert!(selected.starts_with("/proc/self/fd/"));
    let image_name = std::env::current_exe().unwrap();
    assert!(
        image_name
            .to_string_lossy()
            .contains("memfd:iroha-retained-tool")
    );
    assert_ne!(image_name.file_stem().unwrap(), "cargo");
    assert_eq!(std::env::args_os().next().unwrap(), "/iroha-retained-tool");
    println!("cargo-image-ready");
    std::io::stdout().flush().unwrap();
    std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink()).unwrap();
    assert!(fs::metadata(selected).unwrap().is_file());
}

#[test]
fn cargo_image_wait_error_retains_both_originals() {
    use iroha_fs::ProcessStdio;
    use std::io::{BufRead as _, BufReader};
    let root = tempfile::tempdir().unwrap();
    let parent = private(root.path(), "published");
    let tool = fixture_tool(root.path())
        .publish_cargo_image(&parent, "invocation")
        .unwrap();
    let mut command = tool.command().unwrap();
    tool.bind(&mut command, "CARGO").unwrap();
    command
        .args([
            "--exact",
            "cli::policy::cargo_image::tests::cargo_image_child",
            "--ignored",
            "--nocapture",
        ])
        .stdin(ProcessStdio::piped())
        .stdout(ProcessStdio::piped());
    let mut child = command.spawn().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(stdout.read_line(&mut line).unwrap(), 0);
        if line.contains("cargo-image-ready") {
            break;
        }
    }
    let mut stdin = child.stdin.take();
    let mut observations = 0;
    let (status, first) = super::super::super::build::wait_terminal(|| {
        observations += 1;
        tool.revalidate().unwrap();
        if observations == 1 {
            assert!(child.try_wait().unwrap().is_none());
            Err(io::Error::other("injected wait error"))
        } else {
            drop(stdin.take());
            child.wait()
        }
    });
    assert!(status.success());
    assert_eq!(first.unwrap().to_string(), "injected wait error");
    assert_eq!(observations, 2);
    tool.revalidate().unwrap();
}

#[test]
fn sealed_cargo_descriptor_survives_ancestor_substitution_without_reselection() {
    let root = tempfile::tempdir().unwrap();
    let parent = private(root.path(), "published");
    let tool = fixture_tool(root.path())
        .publish_cargo_image(&parent, "invocation")
        .unwrap();
    let mut command = tool.command().unwrap();
    tool.bind(&mut command, "CARGO").unwrap();
    command
        .args([
            "--exact",
            "cli::policy::cargo_image::tests::cargo_image_child",
            "--ignored",
            "--nocapture",
        ])
        .stdin(iroha_fs::ProcessStdio::null());
    // Parent substitution after command creation cannot change either selected fd.
    let old = root.path().join("displaced");
    fs::rename(parent.path(), &old).unwrap();
    fs::create_dir(parent.path()).unwrap();
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("cargo-image-ready"));
    assert!(tool.revalidate().is_err());
    fs::remove_dir(parent.path()).unwrap();
    fs::rename(old, parent.path()).unwrap();
    // The ancestor can be restored without changing the sealed child snapshot;
    // exact descriptor selection, rather than final pathname equality, prevented
    // substitution during the launch window.
    tool.revalidate().unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn genuine_cargo_preserves_selected_original_through_two_recursive_builds() {
    genuine_recursion(Rebinding::None);
}

#[cfg(target_os = "linux")]
#[test]
fn genuine_recursive_cargo_memfd_rejects_named_leaf_execution_authority() {
    genuine_recursion(Rebinding::Anonymous);
}

#[cfg(target_os = "linux")]
#[test]
fn genuine_named_cargo_control_demonstrates_substitution_before_final_refusal() {
    genuine_recursion(Rebinding::NamedControl);
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rebinding {
    None,
    Anonymous,
    NamedControl,
}

#[cfg(target_os = "linux")]
fn genuine_recursion(rebinding: Rebinding) {
    use iroha_fs::ProcessStdio;
    use std::os::unix::fs::PermissionsExt as _;
    fn actual_tool(name: &str) -> PathBuf {
        // Test-only discovery, never release trust or source admission.
        let result = std::process::Command::new("rustup")
            .args(["which", name])
            .output()
            .unwrap();
        assert!(result.status.success(), "{result:?}");
        PathBuf::from(std::str::from_utf8(&result.stdout).unwrap().trim())
    }
    let root = tempfile::tempdir().unwrap();
    let parent = private(root.path(), "published");
    let cargo = Tool::open(&actual_tool("cargo"))
        .unwrap()
        .publish_cargo_image(&parent, "invocation")
        .unwrap();
    let rustc = Tool::open(&actual_tool("rustc")).unwrap();
    let versions = [(&cargo, "--version"), (&rustc, "-vV")].map(|(tool, argument)| {
        let mut command = tool.source_command().unwrap();
        command.arg(argument).stdin(ProcessStdio::null());
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    });
    let identity = cargo
        .cargo_image
        .as_ref()
        .unwrap()
        .executable
        .identity()
        .unwrap()
        .local_parts();
    let inode = u64::from_le_bytes(identity.2[..8].try_into().unwrap());
    let expected = format!("{}:{inode}", identity.1);
    for level in 0..3 {
        let project = root.path().join(format!("level{level}"));
        fs::create_dir_all(project.join("src")).unwrap();
        fs::write(
            project.join("Cargo.toml"),
            format!(
                "[package]\nname='level{level}'\nversion='0.0.0'\nedition='2024'\n[workspace]\n"
            ),
        )
        .unwrap();
        fs::write(
            project.join("Cargo.lock"),
            format!("version = 4\n[[package]]\nname='level{level}'\nversion='0.0.0'\n"),
        )
        .unwrap();
        fs::write(
            project.join("src/lib.rs"),
            "//! Recursive original-Cargo fixture.\n",
        )
        .unwrap();
        let next = if level < 2 {
            format!(
                r#"
            let next = root.join("level{}");
            let output = std::process::Command::new(&cargo)
                .args(["build", "--offline", "--locked", "-j1"])
                .current_dir(&next).env("CARGO_TARGET_DIR", next.join("target"))
                .output().unwrap();
            assert!(output.status.success(), "{{}}", String::from_utf8_lossy(&output.stderr));
        "#,
                level + 1
            )
        } else {
            String::new()
        };
        fs::write(project.join("build.rs"), format!(r##"//! Genuine recursive Cargo build-script assertion.
        use std::os::unix::fs::MetadataExt as _;
        fn main() {{
            let root = std::path::PathBuf::from(std::env::var_os("PROBE_ROOT").unwrap());
            let cargo = std::env::var("CARGO").unwrap();
            if {level} == 0 && std::env::var("REBIND_LEAF").unwrap() == "yes" {{
                use std::os::unix::fs::PermissionsExt as _;
                let named = root.join("published/invocation/cargo");
                std::fs::rename(&named, root.join("published/invocation/displaced-original")).unwrap();
                std::fs::write(&named, b"#!/bin/sh\nprintf substituted > \"$PROBE_ROOT/substituted-cargo-ran\"\nexit 67\n").unwrap();
                std::fs::set_permissions(&named, std::fs::Permissions::from_mode(0o700)).unwrap();
                let output = std::process::Command::new(&cargo).arg("--version").output().unwrap();
                assert!(output.status.success(), "substituted Cargo must never execute: {{output:?}}");
                assert_eq!(output.stdout, std::env::var("EXPECTED_CARGO_VERSION").unwrap().as_bytes());
            }}
            assert_eq!(cargo, std::env::var("EXPECTED_CARGO_FD").unwrap());
            assert!(cargo.starts_with("/proc/self/fd/"));
            let metadata = std::fs::metadata(&cargo).unwrap();
            assert_eq!(format!("{{}}:{{}}",metadata.dev(),metadata.ino()), std::env::var("EXPECTED_CARGO_ID").unwrap());
            for (key, argument) in [("CARGO", "--version"), ("RUSTC", "-vV")] {{
                let selected = std::env::var(key).unwrap();
                assert!(selected.starts_with("/proc/self/fd/"), "{{key}} lost descriptor selection");
                let output = std::process::Command::new(selected).arg(argument).output().unwrap();
                assert!(output.status.success(), "{{key}}: {{}}",String::from_utf8_lossy(&output.stderr));
                assert_eq!(output.stdout, std::env::var(format!("EXPECTED_{{key}}_VERSION")).unwrap().as_bytes());
            }}
            {next}
            std::fs::write(root.join("observed-{level}"), cargo).unwrap();
        }}
        "##)).unwrap();
    }
    let mut command = if rebinding == Rebinding::NamedControl {
        // Counterexample only: the rejected predecessor executes this renameable witness.
        cargo
            .cargo_image
            .as_ref()
            .unwrap()
            .batch
            .command(0)
            .unwrap()
    } else {
        cargo.command().unwrap()
    };
    command
        .args(["build", "--offline", "--locked", "-j1"])
        .current_dir(root.path().join("level0"))
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("CARGO_HOME", root.path().join("home"))
        .env("CARGO_TARGET_DIR", root.path().join("level0/target"))
        .env("PROBE_ROOT", root.path())
        .env(
            "REBIND_LEAF",
            if rebinding == Rebinding::None {
                "no"
            } else {
                "yes"
            },
        )
        .env("EXPECTED_CARGO_ID", expected)
        .env("EXPECTED_CARGO_VERSION", &versions[0])
        .env("EXPECTED_RUSTC_VERSION", &versions[1])
        .stdin(ProcessStdio::null());
    cargo.bind(&mut command, "CARGO").unwrap();
    rustc.bind(&mut command, "RUSTC").unwrap();
    let selected = command
        .get_envs()
        .find(|(k, _)| *k == "CARGO")
        .and_then(|(_, v)| v)
        .unwrap()
        .to_os_string();
    command.env("EXPECTED_CARGO_FD", &selected);
    if rebinding != Rebinding::None {
        // Mutate only the witness after both original command and descriptor binding
        // exist, before Cargo caches current_exe. The selected memfd has no such leaf.
        let directory = root.path().join("published/invocation");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        fs::rename(directory.join(IMAGE), directory.join("cargo")).unwrap();
    }
    let output = command.output().unwrap();
    if rebinding == Rebinding::NamedControl {
        assert!(!output.status.success());
        assert_eq!(
            fs::read(root.path().join("substituted-cargo-ran")).unwrap(),
            b"substituted"
        );
        assert!(!root.path().join("observed-0").exists());
        assert!(cargo.revalidate().is_err());
        rustc.revalidate().unwrap();
        return;
    }
    assert!(!root.path().join("substituted-cargo-ran").exists());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for level in 0..3 {
        assert_eq!(
            fs::read_to_string(root.path().join(format!("observed-{level}"))).unwrap(),
            selected.to_str().unwrap()
        );
    }
    if rebinding == Rebinding::None {
        cargo.revalidate().unwrap();
    } else {
        assert!(cargo.revalidate().is_err());
    }
    rustc.revalidate().unwrap();
}
