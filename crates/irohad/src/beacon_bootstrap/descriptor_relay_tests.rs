//! Isolated native controls for descriptor isolation, custody and private logging.

use super::*;
use std::{
    fs,
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    os::{fd::FromRawFd as _, unix::fs::PermissionsExt as _},
    process::Command,
};

const CASE: &str = "IROHA_BEACON_DESCRIPTOR_RELAY_TEST";

fn isolated(case: &str) {
    let root = tempfile::Builder::new()
        .prefix(".beacon-relay-test-")
        .tempdir_in(std::env::current_dir().unwrap())
        .unwrap();
    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let module = module_path!().split_once("::").unwrap().1;
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("{module}::dense_parent"),
            "--ignored",
            "--nocapture",
        ])
        .env(CASE, case)
        .env(
            "IROHA_BEACON_DESCRIPTOR_RELAY_ROOT",
            root.path().canonicalize().unwrap(),
        )
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "case={case}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "isolate dense descriptor ownership from every other test"]
fn dense_parent() {
    let case = std::env::var(CASE).unwrap();
    let root =
        std::path::PathBuf::from(std::env::var_os("IROHA_BEACON_DESCRIPTOR_RELAY_ROOT").unwrap());
    let source = File::open("/dev/null").unwrap();
    let mut held = Vec::new();
    loop {
        let fd = rustix::io::fcntl_dupfd_cloexec(&source, 3).unwrap();
        let last = fd.as_raw_fd() >= 220;
        held.push(fd);
        if last {
            break;
        }
    }
    if let Some(number) = case.strip_prefix("occupied-") {
        let number = number.parse::<i32>().unwrap();
        let occupied = held.iter().find(|fd| fd.as_raw_fd() == number).unwrap();
        rustix::io::fcntl_setfd(occupied, rustix::io::FdFlags::empty()).unwrap();
    }
    let before = held
        .iter()
        .map(|fd| (fd.as_raw_fd(), rustix::io::fcntl_getfd(fd).unwrap()))
        .collect::<Vec<_>>();
    let module = module_path!().split_once("::").unwrap().1;
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            &format!("{module}::relay_child"),
            "--ignored",
            "--nocapture",
        ])
        .env(CASE, &case)
        .env("IROHA_BEACON_DESCRIPTOR_RELAY_ROOT", root.as_path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let expected_pid = child.id();
    let output = child.wait_with_output().unwrap();
    if case == "dispatch" {
        assert_eq!(
            output.status.code(),
            Some(75),
            "canonical missing-public-evidence rejection"
        );
        assert_eq!(
            fs::read(root.join("input")).unwrap(),
            b"one-shot-private-record"
        );
        assert!(
            fs::read_to_string(root.join("stderr.log"))
                .unwrap()
                .contains("beacon bootstrap bounded I/O failed")
        );
    } else {
        assert!(
            output.status.success(),
            "case={case}\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(root.join("pid-after")).unwrap(),
            expected_pid.to_string()
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("pid")).unwrap(),
        expected_pid.to_string()
    );
    assert_eq!(
        held.iter()
            .map(|fd| (fd.as_raw_fd(), rustix::io::fcntl_getfd(fd).unwrap()))
            .collect::<Vec<_>>(),
        before
    );
}

#[test]
fn dense_parent_descriptors_preserve_all_native_handoffs() {
    for case in ["key", "pending", "pending-current", "seat"] {
        isolated(case);
    }
}

fn rotation_arguments(root: &Path) -> Vec<std::ffi::OsString> {
    let hash = iroha_crypto::Hash::new(b"native-relay-control");
    let network = iroha_data_model::NetworkId::from_genesis_hash(
        iroha_crypto::HashOf::from_untyped_unchecked(hash),
    );
    vec![
        "--credential-max-memory-bytes".into(),
        "67108864".into(),
        "provision-rotation-seat".into(),
        "--selection-evidence".into(),
        root.join("missing-evidence").into_os_string(),
        "--network-id".into(),
        network.to_string().into(),
        "--chain-id".into(),
        "relay-control".into(),
        "--target-epoch".into(),
        "2".into(),
        "--transition-id".into(),
        hash.to_string().into(),
        "--signer-index".into(),
        "1".into(),
        "--key-fd".into(),
        "198".into(),
        "--public-fd".into(),
        "201".into(),
        "--finality-fd".into(),
        "202".into(),
        "--provider-handle".into(),
        "relay-control".into(),
        "--provider-revision".into(),
        "1".into(),
        "--attempt-root".into(),
        root.as_os_str().to_owned(),
    ]
}

#[test]
fn typed_dispatch_restores_logs_and_rejects_missing_evidence_before_private_consumption() {
    use clap::Parser as _;
    for option in ["--key-fd", "--public-fd", "--finality-fd"] {
        let mut arguments = rotation_arguments(Path::new("/unused-relay-test"));
        let index = arguments.iter().position(|arg| arg == option).unwrap();
        arguments[index + 1] = "203".into();
        let parsed = super::super::Args::try_parse_from(
            std::iter::once("beacon-bootstrap".into()).chain(arguments),
        )
        .unwrap();
        assert!(matches!(
            super::super::prepare_relay(&parsed.command),
            Err(Error::InvalidInput)
        ));
    }
    isolated("dispatch");
}

#[test]
fn surviving_unrelated_slot_is_refused_without_changing_it() {
    for case in [
        "occupied-198",
        "occupied-200",
        "occupied-201",
        "occupied-202",
    ] {
        isolated(case);
    }
}

#[test]
fn private_logs_refuse_preexisting_names_and_nonprivate_root() {
    for case in ["log-collision", "public-root"] {
        isolated(case);
    }
}

#[test]
#[ignore = "exact subprocess control; never mutate the test runner's standard descriptors"]
#[allow(
    unsafe_code,
    reason = "the isolated control takes each fixed descriptor transferred by the actual relay exactly once"
)]
fn relay_child() {
    let case = std::env::var(CASE).unwrap();
    let root =
        std::path::PathBuf::from(std::env::var_os("IROHA_BEACON_DESCRIPTOR_RELAY_ROOT").unwrap());
    fs::write(root.join("pid"), std::process::id().to_string()).unwrap();
    let saved = [
        rustix::stdio::stdin(),
        rustix::stdio::stdout(),
        rustix::stdio::stderr(),
    ]
    .map(|fd| rustix::io::fcntl_dupfd_cloexec(fd, 300).unwrap());
    let key_path = root.join("input");
    fs::write(&key_path, b"one-shot-private-record").unwrap();
    let key = File::options()
        .read(true)
        .write(true)
        .open(&key_path)
        .unwrap();
    rustix::stdio::dup2_stdin(&key).unwrap();
    if let Some(number) = case.strip_prefix("occupied-") {
        let number = number.parse::<i32>().unwrap();
        let fd = unsafe { BorrowedFd::borrow_raw(number) };
        let before = rustix::fs::fstat(fd).unwrap();
        let flags = rustix::io::fcntl_getfd(fd).unwrap();
        let error = match number {
            198 => prepare_key().unwrap_err(),
            200 => prepare_pending(true).unwrap_err(),
            201 | 202 => reserve(&[
                (rustix::stdio::stdin(), 198),
                (rustix::stdio::stdout(), 201),
                (rustix::stdio::stderr(), 202),
            ])
            .unwrap_err(),
            _ => unreachable!(),
        };
        assert_eq!(
            error.raw_os_error(),
            Some(rustix::io::Errno::BUSY.raw_os_error())
        );
        let after = rustix::fs::fstat(fd).unwrap();
        assert_eq!((before.st_dev, before.st_ino), (after.st_dev, after.st_ino));
        assert_eq!(rustix::io::fcntl_getfd(fd).unwrap(), flags);
    } else {
        let (public, mut public_writer) = std::io::pipe().unwrap();
        let (finality, mut finality_writer) = std::io::pipe().unwrap();
        public_writer.write_all(b"public-frame").unwrap();
        finality_writer.write_all(b"finality-frame").unwrap();
        drop(public_writer);
        drop(finality_writer);
        let seat = matches!(
            case.as_str(),
            "seat" | "dispatch" | "log-collision" | "public-root"
        );
        if seat {
            rustix::stdio::dup2_stdout(&public).unwrap();
            rustix::stdio::dup2_stderr(&finality).unwrap();
            if case == "dispatch" {
                let arguments = rotation_arguments(&root);
                super::super::dispatch_arguments(arguments.into_iter(), true);
                panic!("missing evidence must fail in the unchanged native verifier");
            }
            if case == "log-collision" {
                fs::write(root.join("stdout.log"), b"untouched").unwrap();
            }
            if case == "public-root" {
                fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
            }
            let result = prepare_seat(&root);
            if case != "seat" {
                assert!(result.is_err());
                // Failed reservations are dropped and private paths are never overwritten.
                assert!(
                    rustix::io::fcntl_dupfd_cloexec(&key, 198)
                        .is_ok_and(|fd| fd.as_raw_fd() == 198)
                );
                if case == "log-collision" {
                    assert_eq!(fs::read(root.join("stdout.log")).unwrap(), b"untouched");
                }
            } else {
                result.unwrap();
                for (fd, expected) in [
                    (201, b"public-frame".as_slice()),
                    (202, b"finality-frame".as_slice()),
                ] {
                    let mut file = unsafe { File::from_raw_fd(fd) };
                    assert_eq!(
                        rustix::fs::fcntl_getfl(&file).unwrap() & rustix::fs::OFlags::ACCMODE,
                        rustix::fs::OFlags::RDONLY
                    );
                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes).unwrap();
                    assert_eq!(bytes, expected);
                }
                rustix::io::write(rustix::stdio::stdout(), b"private stdout").unwrap();
                rustix::io::write(rustix::stdio::stderr(), b"private stderr").unwrap();
                for (name, expected) in [
                    ("stdout.log", b"private stdout".as_slice()),
                    ("stderr.log", b"private stderr".as_slice()),
                ] {
                    assert_eq!(fs::read(root.join(name)).unwrap(), expected);
                    assert_eq!(
                        fs::metadata(root.join(name)).unwrap().mode() & 0o7777,
                        0o600
                    );
                }
            }
        } else if case == "key" {
            prepare_key().unwrap();
        } else {
            let current = case == "pending-current";
            if current {
                rustix::stdio::dup2_stdout(&public).unwrap();
            }
            prepare_pending(current).unwrap();
            if current {
                let mut file = unsafe { File::from_raw_fd(200) };
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap();
                assert_eq!(bytes, b"public-frame");
            }
        }
        if !matches!(case.as_str(), "log-collision" | "public-root") {
            let mut file = unsafe { File::from_raw_fd(198) };
            assert!(
                rustix::io::fcntl_getfd(&file)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
            file.seek(SeekFrom::Start(0)).unwrap();
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, b"one-shot-private-record");
            file.set_len(0).unwrap();
            assert_eq!(fs::metadata(key_path).unwrap().len(), 0);
        }
    }
    fs::write(root.join("pid-after"), std::process::id().to_string()).unwrap();
    rustix::stdio::dup2_stdin(&saved[0]).unwrap();
    rustix::stdio::dup2_stdout(&saved[1]).unwrap();
    rustix::stdio::dup2_stderr(&saved[2]).unwrap();
}
