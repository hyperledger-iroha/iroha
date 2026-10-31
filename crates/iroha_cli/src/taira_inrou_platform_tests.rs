//! Unsupported coordinator operations refuse before authority or filesystem work.

use super::*;
use iroha_i18n::Localizer;

struct UnusedContext;

impl RunContext for UnusedContext {
    fn config(&self) -> &Config {
        panic!("unsupported coordinator must not access client authority")
    }

    fn transaction_metadata(&self) -> Option<&Metadata> {
        panic!("unsupported coordinator must not access transaction metadata")
    }

    fn input_instructions(&self) -> bool {
        panic!("unsupported coordinator must not consume instructions")
    }

    fn output_instructions(&self) -> bool {
        panic!("unsupported coordinator must not publish instructions")
    }

    fn i18n(&self) -> &Localizer {
        panic!("unsupported coordinator must not render output")
    }

    fn print_data<T: JsonSerialize + ?Sized>(&mut self, _data: &T) -> Result<()> {
        panic!("unsupported coordinator must not publish output")
    }

    fn println(&mut self, _data: impl std::fmt::Display) -> Result<()> {
        panic!("unsupported coordinator must not publish output")
    }
}

fn require_unsupported(error: eyre::Report) {
    assert_eq!(
        error.downcast_ref::<std::io::Error>().unwrap().kind(),
        std::io::ErrorKind::Unsupported
    );
    assert!(error.to_string().contains("Unix host coordinator"));
}

#[test]
fn native_inrou_stage_refuses_before_authority_or_artifact_creation() {
    let temporary = tempfile::tempdir().unwrap();
    let stage = temporary.path().join("absent-stage");
    let validators = temporary.path().join("validators");
    fs::create_dir(&validators).unwrap();
    let sentinel = validators.join("peer0.toml");
    fs::write(&sentinel, b"unchanged validator configuration").unwrap();
    let command = InrouStage {
        mode: InrouCanaryMode::Deploy,
        container: temporary.path().join("container.json"),
        service: temporary.path().join("service.json"),
        bundle_file: temporary.path().join("bundle.tgz"),
        sorafs_retention_epoch: NonZeroU64::new(2_000_000_000).unwrap(),
        placement_targets: Vec::new(),
        stage_dir: stage.clone(),
        bind_validator_config_dir: validators,
        json: true,
    };
    require_unsupported(command.run(&mut UnusedContext).unwrap_err());
    assert!(!stage.exists());
    assert_eq!(
        fs::read(sentinel).unwrap(),
        b"unchanged validator configuration"
    );
}

#[test]
fn native_inrou_canary_actions_refuse_before_authority_or_transport() {
    let temporary = tempfile::tempdir().unwrap();
    let stage = temporary.path().join("absent-stage");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    for action in 0..3 {
        let nonce = "n".repeat(32);
        let command = InrouCanary {
            probe_scope: InrouProbeScope::Candidate,
            public_root: format!("http://{}", listener.local_addr().unwrap()),
            stage_dir: stage.clone(),
            mode: InrouCanaryMode::Deploy,
            operation: InrouCanaryOperation::GuestPin,
            authorization_sha256: "ab".repeat(32),
            authorization_nonce: nonce.clone(),
            mutation_phase: "pre_edge".to_owned(),
            idempotency_key: write_canary_child_idempotency_key(
                &nonce,
                "pre_edge",
                "inrou_guest_pin",
            ),
            execution_expires_at_unix_ms: u64::MAX,
            prepare_envelope: action == 0,
            prepared_output_fd: (action == 0).then_some(3),
            submit_prepared_envelope_fd: (action == 1).then_some(3),
            recover_prepared_envelope_fd: (action == 2).then_some(3),
            prerequisite_envelope_fd: (action == 0).then_some(4),
            timeout_secs: 1,
            json: true,
        };
        require_unsupported(command.run(&mut UnusedContext).unwrap_err());
        assert!(!stage.exists());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
