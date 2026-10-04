//! Generated-profile HTTP refusals and exact original wallet recovery over real loopback HTTP.
//! Wallet preparation and once-only dispatch establish no native policy or activation evidence.

use super::{ManagedInitialReservePolicy, Original, journal};
use crate::managed::native_operation::test_support::{
    native_fixture::policy, wallet_http::WalletHttp,
};
use crate::managed::{
    PreparedLocalnet,
    native_operation::{
        MAX_CHECKPOINT_BYTES, now_ms, read_optional, require_empty, test_support::UnavailablePeers,
    },
};
use iroha_data_model::transaction::{FeePaymentIntent, SignedTransaction};
use iroha_wallet::operations::{AccountService, BoundedTransactionOptions, OperationStatus};
use std::{
    collections::BTreeMap,
    io,
    time::{Duration, Instant},
};

const FINALITY_PATH: &str = "/v1/bridge/finality/1";

fn fixture() -> (
    tempfile::TempDir,
    PreparedLocalnet,
    ManagedInitialReservePolicy,
    UnavailablePeers,
) {
    let temporary = tempfile::Builder::new()
        .prefix(".reserve-http-")
        .tempdir()
        .unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "reserve-http",
        &temporary.path().join("generation"),
        &ports,
        crate::localnet::LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let coordinator = ManagedInitialReservePolicy::open(&prepared).unwrap();
    drop(ports);
    let peers = UnavailablePeers::start(&prepared);
    (temporary, prepared, coordinator, peers)
}

#[test]
fn missing_reserve_original_recovery_performs_no_http_or_journal_creation() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared, mut coordinator, mut peers) = fixture();
    for _ in 0..2 {
        assert!(
            coordinator
                .recover(Instant::now() + Duration::from_secs(15))
                .is_err()
        );
        assert!(peers.requests.lock().unwrap().is_empty());
        assert_eq!(
            coordinator
                .authority
                .directory
                .open_child("set")
                .err()
                .expect("read-only recovery must not create an operation directory")
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(
            read_optional(
                &coordinator.authority.directory,
                "current-checkpoint.nrt",
                MAX_CHECKPOINT_BYTES,
            )
            .unwrap()
            .is_none()
        );
        drop(coordinator);
        coordinator = ManagedInitialReservePolicy::open(&prepared).unwrap();
    }
    peers.finish();
    assert!(peers.requests.lock().unwrap().is_empty());
}

#[test]
fn unavailable_reserve_finality_prevents_original_wallet_quote_and_dispatch() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared, mut coordinator, mut peers) = fixture();
    let policy = policy(&coordinator.authority);
    let utc_deadline = now_ms().unwrap() + 60_000;
    for _ in 0..2 {
        let options = BoundedTransactionOptions {
            fee_payment: FeePaymentIntent::authority(Vec::new(), None),
            max_total_fees: BTreeMap::new(),
            deadline: Instant::now() + Duration::from_secs(15),
        };
        let error = coordinator
            .set_initial(&policy, utc_deadline, &options)
            .err()
            .expect("missing original finality must refuse initial reserve preparation");
        assert!(
            error
                .to_string()
                .contains("cannot read original genesis result"),
            "refusal must precede wallet preparation: {error}"
        );
        let directory = coordinator.authority.directory.open_child("set").unwrap();
        require_empty(&directory).unwrap();
        assert_eq!(
            directory
                .open_child("transaction")
                .err()
                .expect("no wallet journal before authenticated finality")
                .kind(),
            io::ErrorKind::NotFound
        );
        assert!(
            read_optional(
                &coordinator.authority.directory,
                "current-checkpoint.nrt",
                MAX_CHECKPOINT_BYTES,
            )
            .unwrap()
            .is_none()
        );
        let observed_requests = peers.requests.lock().unwrap().len();
        assert!(
            coordinator
                .recover(Instant::now() + Duration::from_secs(15))
                .is_err()
        );
        assert_eq!(peers.requests.lock().unwrap().len(), observed_requests);
        require_empty(&directory).unwrap();
        drop(directory);
        drop(coordinator);
        coordinator = ManagedInitialReservePolicy::open(&prepared).unwrap();
    }
    peers.finish();
    let requests = peers.requests.lock().unwrap();
    for request in requests.iter() {
        assert_eq!(request.method, "GET", "no transaction or quote POST");
        assert!(
            matches!(
                request.path.as_str(),
                "/v1/node/capabilities" | FINALITY_PATH
            ),
            "no quote, transaction status, dispatch or reserve read before finality: {}",
            request.path
        );
    }
    for peer in 0..4 {
        for path in ["/v1/node/capabilities", FINALITY_PATH] {
            assert_eq!(
                requests
                    .iter()
                    .filter(|request| request.peer == peer && request.path == path)
                    .count(),
                2,
                "each reopened attempt must use the selected original peer {peer} at {path}"
            );
        }
    }
}

#[test]
fn original_request_wallet_reopens_without_resigning_or_duplicate_dispatch() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared, mut coordinator, mut unavailable) = fixture();
    unavailable.finish();
    let directory = coordinator.authority.directory.ensure_child("set").unwrap();
    let policy = policy(&coordinator.authority);
    let options = BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(Vec::new(), None),
        max_total_fees: BTreeMap::new(),
        deadline: Instant::now() + Duration::from_secs(180),
    };
    let original = Original {
        selection: coordinator.selection(&policy).unwrap(),
        policy,

        // Codec-only original intent. No coordinator advance, native proof or activation is
        // inferred from this intentionally invalid checkpoint or from the accepted local quote.
        checkpoint: vec![1],
    };
    let original = super::tests::retain_explicit_request(
        &coordinator,
        &directory,
        &original,
        now_ms().unwrap() + 600_000,
        &options,
    );
    let original_bytes = directory.read("original.nrt", 256 * 1024).unwrap();
    let path = original.directory().path().join("transaction");
    let mut http = WalletHttp::start_config(&coordinator.authority.config, path.clone());
    let account = AccountService::new(coordinator.authority.config.clone()).unwrap();
    let request = original.request(original.terms.signing_deadline(options.deadline).unwrap());
    assert_eq!(
        account
            .prepare_initial_reserve_policy(&request, &path)
            .unwrap()
            .status,
        OperationStatus::Prepared
    );
    let operation_bytes = std::fs::read(path.join("operation.json")).unwrap();
    let before_verify = http.requests.lock().unwrap().len();
    let signed = coordinator
        .verify_wallet(original.directory(), &original, options.deadline)
        .unwrap();
    assert_eq!(http.requests.lock().unwrap().len(), before_verify);
    let signed_wire = signed.encode_wire_v1().unwrap();
    assert!(!path.join("submission.json").exists());
    assert_eq!(
        account
            .resume_initial_reserve_policy(&path, &request)
            .unwrap()
            .status,
        OperationStatus::Absent
    );
    assert!(!path.join("submission.json").exists());
    drop(account);
    drop(directory);
    let mut marker = None;
    for _ in 0..2 {
        drop(coordinator);
        coordinator = ManagedInitialReservePolicy::open(&prepared).unwrap();
        let directory = coordinator.authority.directory.open_child("set").unwrap();
        let restored = journal::required_original(&directory).unwrap();
        let request = restored.request(Instant::now() + Duration::from_secs(300));
        assert_eq!(
            request.deadline_unix_ms,
            original.terms.signing_deadline_unix_ms
        );
        let before_verify = http.requests.lock().unwrap().len();
        assert_eq!(
            coordinator
                .verify_wallet(restored.directory(), &restored, request.options.deadline)
                .unwrap()
                .encode_wire_v1()
                .unwrap(),
            signed_wire
        );
        assert_eq!(http.requests.lock().unwrap().len(), before_verify);
        let account = AccountService::new(coordinator.authority.config.clone()).unwrap();
        assert_eq!(
            account
                .submit_initial_reserve_policy(&path, &request)
                .unwrap()
                .status,
            OperationStatus::Pending
        );
        assert_eq!(
            account
                .resume_initial_reserve_policy(&path, &request)
                .unwrap()
                .status,
            OperationStatus::Pending
        );
        let retained_marker = std::fs::read(path.join("submission.json")).unwrap();
        if let Some(previous) = &marker {
            assert_eq!(previous, &retained_marker);
        } else {
            marker = Some(retained_marker);
        }
        assert_eq!(
            std::fs::read(path.join("operation.json")).unwrap(),
            operation_bytes
        );
        assert_eq!(
            directory
                .read("original.nrt", 256 * 1024)
                .unwrap()
                .as_slice(),
            original_bytes.as_slice()
        );
    }
    http.finish();
    let requests = http.requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.target.path() == "/v1/fees/quote")
            .count(),
        1,
        "recovery never requotes or signs a replacement"
    );
    let submissions: Vec<_> = requests
        .iter()
        .filter(|request| {
            request.target.path() == iroha_torii_shared::route_catalog::pipeline::TRANSACTION.path()
        })
        .collect();
    assert_eq!(
        submissions.len(),
        1,
        "ambiguous HTTP failure is never redispatched"
    );
    assert_eq!(submissions[0].body, signed_wire);
    for request in requests
        .iter()
        .filter(|request| request.target.path() == "/v1/pipeline/transactions/status")
    {
        let hash = request
            .target
            .query_pairs()
            .find(|(key, _)| key == "hash")
            .unwrap()
            .1;
        assert_eq!(
            hash.parse::<iroha_crypto::HashOf<SignedTransaction>>()
                .unwrap(),
            signed.hash()
        );
    }
}
