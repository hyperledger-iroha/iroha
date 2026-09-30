//! Current native consensus status and explicit runtime operator-key consumers.

use super::*;

#[cfg(unix)]
fn native_status_fixture() -> iroha_data_model::sumeragi::SumeragiStatus {
    use iroha_data_model::sumeragi::{PROTOCOL_VERSION, SumeragiFootprint, SumeragiStatus};
    let leader = KeyPair::try_from_seed(vec![7; 32], Algorithm::BlsNormal)
        .expect("native leader fixture key");
    let proxy_tail = KeyPair::try_from_seed(vec![8; 32], Algorithm::BlsNormal)
        .expect("native proxy-tail fixture key");
    SumeragiStatus {
        protocol_version: PROTOCOL_VERSION,
        config_fingerprint: CryptoHash::new(b"CLI native status fixture"),
        beacon_horizon: None,
        instance: [3; 32],
        height: 10,
        view: 2,
        stage: 1,
        leader: Some(leader.public_key().clone()),
        proxy_tail: Some(proxy_tail.public_key().clone()),
        high_qc_view: Some(1),
        level: 2,
        start_level: 0,
        t_retx_ms: 250,
        committed_height: 9,
        applied_height: 8,
        awaiting: false,
        signer: Some(leader.public_key().clone()),
        unanchored: false,
        abstaining: false,
        halted: None,
        footprint: SumeragiFootprint {
            pending_apply: 1,
            peers: 4,
            ..SumeragiFootprint::default()
        },
    }
}

#[cfg(unix)]
#[test]
#[allow(
    unsafe_code,
    reason = "the child-only pre_exec hook passes one retained read-only operator descriptor through exec using fcntl"
)]
fn sumeragi_summary_commands_against_torii_mock() {
    use std::{
        io::{Seek as _, SeekFrom},
        os::{
            fd::{AsRawFd as _, BorrowedFd},
            unix::process::CommandExt as _,
        },
    };
    use torii_mock_support::{
        SpawnError, TempDir, ToriiMockProcess, configure_sumeragi, write_client_config,
    };
    let mock = match ToriiMockProcess::spawn() {
        Ok(proc) => proc,
        Err(SpawnError::PythonUnavailable | SpawnError::PermissionDenied) => {
            eprintln!(
                "skipping sumeragi_summary_commands_against_torii_mock: mock server unavailable"
            );
            return;
        }
        Err(err) => panic!("failed to start Torii mock: {err}"),
    };
    let temp_dir = TempDir::new("sumeragi_summary").expect("temp dir");
    let config_path = temp_dir.path().join("client.toml");
    write_client_config(&config_path, mock.base_url()).expect("write config");
    let operator_key_file = tempfile::NamedTempFile::new().expect("private operator key file");
    let operator_key = fixture_key_pair(0xA7);
    fs::write(
        operator_key_file.path(),
        iroha_crypto::ExposedPrivateKey(operator_key.private_key().clone()).to_string(),
    )
    .expect("write canonical runtime operator key");
    let unsigned = command()
        .arg("--config")
        .arg(&config_path)
        .args(["ops", "sumeragi", "status"])
        .output()
        .expect("run operator read without its dedicated signer");
    assert!(!unsigned.status.success());
    assert!(unsigned.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&unsigned.stderr)
            .contains("operator signing key is required before request dispatch")
    );
    let status = native_status_fixture();
    let mut configuration = norito::json::Map::new();
    configuration.insert(
        "status".to_owned(),
        norito::json::to_value(&status).expect("typed native status JSON"),
    );
    configure_sumeragi(mock.base_url(), &Value::Object(configuration))
        .expect("configure canonical Sumeragi status");
    let mut inherited_operator_file =
        fs::File::open(operator_key_file.path()).expect("read-only operator descriptor");
    inherited_operator_file
        .seek(SeekFrom::Start(5))
        .expect("retain caller cursor");
    let operator_fd = inherited_operator_file.as_raw_fd();
    let assert_summary = |args: &[&str], expected: &str| {
        for inherited in [false, true] {
            let mut invocation = command();
            invocation.arg("--config").arg(&config_path);
            if inherited {
                invocation
                    .arg("--operator-private-key-fd")
                    .arg(operator_fd.to_string());
                // SAFETY: the read-only file remains open through child execution; this child-only
                // hook uses only fcntl and lends the descriptor without taking ownership.
                unsafe {
                    invocation.inner.pre_exec(move || {
                        let fd = BorrowedFd::borrow_raw(operator_fd);
                        let flags = rustix::io::fcntl_getfd(fd).map_err(io::Error::from)?;
                        rustix::io::fcntl_setfd(fd, flags & !rustix::io::FdFlags::CLOEXEC)
                            .map_err(io::Error::from)
                    });
                }
            } else {
                invocation
                    .arg("--operator-private-key-file")
                    .arg(operator_key_file.path());
            }
            let output = invocation
                .arg("--output-format")
                .arg("text")
                .args(args)
                .output()
                .unwrap_or_else(|err| panic!("failed to execute iroha {args:?}: {err}"));
            assert!(
                output.status.success(),
                "expected iroha {args:?} to succeed, stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert_eq!(
                stdout.trim_end(),
                expected,
                "unexpected summary for {args:?}, stdout: {stdout}"
            );
        }
    };
    let expected = format!(
        "height=10 view=2 stage=1 leader={} proxy_tail={} lock_view=1 committed=9 applied=8 awaiting=false signing=true halted=no",
        status.leader.as_ref().expect("fixture leader"),
        status.proxy_tail.as_ref().expect("fixture proxy tail"),
    );
    assert_summary(&["ops", "sumeragi", "status"], &expected);
    assert_eq!(
        inherited_operator_file
            .stream_position()
            .expect("caller descriptor remains open"),
        5
    );
}
#[test]
fn sumeragi_leader_subcommand_is_retired() {
    let rejected = command()
        .args(["ops", "sumeragi", "leader"])
        .output()
        .expect("reject the retired leader subcommand");
    // CLI argument failures use the canonical input-error exit code.
    assert_eq!(rejected.status.code(), Some(4));
    assert!(rejected.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("unrecognized subcommand 'leader'"),
        "retired command must fail during argument parsing: {}",
        String::from_utf8_lossy(&rejected.stderr),
    );
}
