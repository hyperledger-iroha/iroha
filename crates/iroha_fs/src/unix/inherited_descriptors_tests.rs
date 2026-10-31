//! Actual fixed-descriptor transfer and launch-channel collision controls.

use super::*;
use std::{
    fs::{self, File},
    io::Read as _,
    os::fd::AsFd as _,
    path::PathBuf,
    process::Stdio,
};

const CASE: &str = "IROHA_FD_TRANSFER_TEST_CASE";

fn run_isolated(case: &str) {
    let module = module_path!().split_once("::").unwrap().1;
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("{module}::descriptor_subprocess_control"),
            "--ignored",
            "--nocapture",
        ])
        .env(CASE, case)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "case={case}\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixed(source: &File, descriptor: RawFd) -> OwnedFd {
    let owned = rustix::io::fcntl_dupfd_cloexec(source, descriptor).unwrap();
    assert_eq!(
        owned.as_raw_fd(),
        descriptor,
        "isolated slot is unavailable"
    );
    owned
}

fn native_probe(directory: &std::path::Path) -> PathBuf {
    let source = directory.join("descriptor_probe.rs");
    let binary = directory.join("descriptor_probe");
    fs::write(
        &source,
        br#"//! Actual native child reading its inherited fixed descriptors.
use std::{io::Read as _, os::fd::FromRawFd as _};
fn main() {
    println!("cwd={}", std::env::current_dir().unwrap().display());
    for arg in std::env::args().skip(1) {
        let fd: i32 = arg.parse().unwrap();
        // The parent explicitly transferred this descriptor into this exec'd child.
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        let mut data = Vec::new(); file.read_to_end(&mut data).unwrap();
        print!("{fd}="); for byte in data { print!("{byte:02x}"); } println!();
    }
}
"#,
    )
    .unwrap();
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args(["--edition=2024", "-C", "debuginfo=0"])
        .arg(source)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    binary
}

fn input(root: &std::path::Path, name: &str, bytes: &[u8]) -> File {
    let path = root.join(name);
    fs::write(&path, bytes).unwrap();
    File::open(path).unwrap()
}

fn actual_permuted_transfer() {
    let root = tempfile::tempdir().unwrap();
    let executable = native_probe(root.path());
    let first = input(root.path(), "first", b"first\0");
    let second = input(root.path(), "second", b"second\n");
    let third = input(root.path(), "third", b"third");
    let originals = [fixed(&first, 198), fixed(&second, 201), fixed(&third, 202)];
    let mut command = Command::new(executable);
    command.args(["198", "201", "202"]).current_dir(root.path());
    inherit_descriptors(
        &mut command,
        &[
            (originals[1].as_fd(), 198),
            (originals[2].as_fd(), 201),
            (originals[0].as_fd(), 202),
        ],
    )
    .unwrap();
    // Source numbers overlap all targets, but every independent input is already
    // retained above all three slots. Dropping originals cannot change selected bytes.
    drop(originals);
    drop((first, second, third));
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "cwd={}\n198=7365636f6e640a\n201=7468697264\n202=666972737400\n",
            root.path().display()
        )
    );
}

fn occupied_target_preserves_flags_and_original() {
    let root = tempfile::tempdir().unwrap();
    let source = input(root.path(), "source", b"selected-source");
    let other = input(root.path(), "other", b"occupied-original");
    let occupied = fixed(&other, 198);
    let flags = rustix::io::fcntl_getfd(&occupied).unwrap();
    assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
    assert_eq!(
        require_unused(198).unwrap_err().raw_os_error(),
        Some(rustix::io::Errno::BUSY.raw_os_error())
    );
    assert_eq!(rustix::io::fcntl_getfd(&occupied).unwrap(), flags);
    let mut command = Command::new(root.path().join("missing-executable"));
    inherit_descriptors(&mut command, &[(source.as_fd(), 198)]).unwrap();
    assert_eq!(
        command.spawn().unwrap_err().raw_os_error(),
        Some(rustix::io::Errno::BUSY.raw_os_error())
    );
    let mut original = File::from(occupied);
    let mut bytes = Vec::new();
    original.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"occupied-original");
}

fn post_probe_occupation_is_not_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let source = input(root.path(), "source", b"selected-source");
    let other = input(root.path(), "other", b"intervening-original");
    let source = rustix::io::fcntl_dupfd_cloexec(source, 203).unwrap();
    require_unused(198).unwrap();
    let occupied = fixed(&other, 198);
    assert_eq!(
        install_exact(&source, 198).unwrap_err().raw_os_error(),
        Some(rustix::io::Errno::BUSY.raw_os_error())
    );
    assert!(
        rustix::io::fcntl_getfd(&occupied)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    let mut original = File::from(occupied);
    let mut bytes = Vec::new();
    original.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"intervening-original");
}

fn real_exec_failure_keeps_error_channel() {
    let root = tempfile::tempdir().unwrap();
    let source = input(root.path(), "source", b"selected-source");
    let mut command = Command::new(root.path().join("missing-executable"));
    inherit_descriptors(&mut command, &[(source.as_fd(), 198)]).unwrap();
    drop(source);
    assert_eq!(command.spawn().unwrap_err().kind(), io::ErrorKind::NotFound);
}

fn actual_exec_error_channel_collision() {
    let root = tempfile::tempdir().unwrap();
    let source = input(root.path(), "source", b"selected-source");
    let mut command = Command::new(root.path().join("missing-executable"));
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    inherit_descriptors(&mut command, &[(source.as_fd(), 128)]).unwrap();
    // Fill every low slot without closing or replacing unrelated descriptors. The
    // next two native allocations are127/128. Rust's fork-based launch allocates its
    // private exec-error pair there;128 is its still-live child write endpoint.
    let mut fillers = Vec::new();
    loop {
        let next = rustix::io::fcntl_dupfd_cloexec(&source, 3).unwrap();
        if next.as_raw_fd() == 127 {
            drop(next);
            break;
        }
        assert!(next.as_raw_fd() < 127);
        fillers.push(next);
    }
    require_unused(127).unwrap();
    require_unused(128).unwrap();
    assert_eq!(
        command.spawn().unwrap_err().raw_os_error(),
        Some(rustix::io::Errno::BUSY.raw_os_error()),
        "the private exec-error channel must report the occupied-target refusal"
    );
    drop(fillers);
}

#[test]
fn original_drop_permuted_fixed_descriptors_and_cwd_reach_actual_child() {
    run_isolated("transfer");
}

#[test]
fn occupied_destination_preserves_cloexec_bytes_and_reports_launch_error() {
    run_isolated("occupied");
}

#[test]
fn post_probe_destination_race_never_clobbers_an_original() {
    run_isolated("intervening");
}

#[test]
fn actual_missing_executable_remains_not_found() {
    run_isolated("exec-error");
}

#[test]
fn actual_private_exec_error_channel_collision_is_refused() {
    run_isolated("channel-collision");
}

#[test]
fn invalid_mapping_is_rejected_before_configuration() {
    let file = File::open(std::env::current_exe().unwrap()).unwrap();
    for target in [RawFd::MIN, -1, 0, 1, 2, MAX_DESTINATION + 1, RawFd::MAX] {
        let mut command = Command::new("unused");
        assert_eq!(
            inherit_descriptors(&mut command, &[(file.as_fd(), target)])
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    for mappings in [
        vec![(file.as_fd(), 198), (file.as_fd(), 198)],
        vec![(file.as_fd(), 198); MAX_MAPPINGS + 1],
    ] {
        assert_eq!(
            inherit_descriptors(&mut Command::new("unused"), &mappings)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    inherit_descriptors(&mut Command::new("unused"), &[]).unwrap();
}

#[test]
#[ignore = "subprocess body; explicitly launched by each descriptor control"]
fn descriptor_subprocess_control() {
    match std::env::var(CASE).unwrap().as_str() {
        "transfer" => actual_permuted_transfer(),
        "occupied" => occupied_target_preserves_flags_and_original(),
        "intervening" => post_probe_occupation_is_not_overwritten(),
        "exec-error" => real_exec_failure_keeps_error_channel(),
        "channel-collision" => actual_exec_error_channel_collision(),
        other => panic!("unknown descriptor subprocess case {other}"),
    }
}
