//! Recovery binds its original child cursor and independently selected pre-dispatch parent.

use super::*;
use crate::attachment::tests::Fixture;

#[test]
fn pending_checkpoint_and_exact_next_decision_survive_canonical_reopen() {
    let mut fixture = Fixture::new();
    fixture.parent_receipt();
    let pending = PendingOperation {
        kind: PendingKind::Registration,
        checkpoint: fixture.parent.checkpoint().encode_canonical().unwrap(),
        replay: None,
    };
    pending.validate(&fixture.identity).unwrap();
    assert_eq!(pending.target(&fixture.identity).unwrap().height, 1);
    assert_eq!(pending.journal_name().unwrap(), "registration");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("attachment");
    let mut store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    let mut record = store.record.clone();
    record.pending = Some(pending);
    store.publish(&record, PublishMode::Replace).unwrap();
    store.record = record;
    assert!(store.progress(None).pending);
    drop(store);
    let store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    assert!(store.record.pending.is_some());
    let previous = PrivateDataspaceAnchorState::from_authorized_registration(
        fixture.identity.registration.clone(),
    )
    .unwrap();
    let anchor = fixture.child_anchor();
    let mut pending = PendingOperation {
        kind: PendingKind::Anchor {
            previous: Box::new(previous),
            anchor: Box::new(anchor),
        },
        checkpoint: fixture.parent.checkpoint().encode_canonical().unwrap(),
        replay: None,
    };
    pending.validate(&fixture.identity).unwrap();
    assert_eq!(pending.target(&fixture.identity).unwrap().height, 2);
    assert_eq!(pending.journal_name().unwrap(), "anchor-2");
    let encoded = encode_bounded(&pending, MAX_RECORD_BYTES).unwrap();
    let reopened: PendingOperation = norito::decode_canonical_with_limits(
        &encoded,
        norito::canonical_decode_limits(encoded.len()),
    )
    .unwrap();
    assert!(reopened.kind == pending.kind);
    reopened.validate(&fixture.identity).unwrap();
    assert_eq!(reopened.target(&fixture.identity).unwrap().height, 2);
    if let PendingKind::Anchor { anchor, .. } = &mut pending.kind {
        **anchor = fixture.child_anchor(); // genuine QC, but a gap from the retained cursor
    }
    assert!(pending.validate(&fixture.identity).is_err());
    pending.checkpoint = fixture.child.checkpoint().encode_canonical().unwrap();
    assert!(pending.verifier(&fixture.identity).is_err());
}

#[test]
fn status_carrier_is_only_a_bounded_hint_and_elapsed_budget_fails_before_io() {
    let mut report = OperationReport {
        status: OperationStatus::Applied,
        data: norito::json!({"evidence": {"block_height": 2}}),
    };
    assert_eq!(carrier_height(&report).unwrap().get(), 2);
    for evidence in [
        norito::json!(null),
        norito::json!({"block_height": 0}),
        norito::json!({"block_height": 1}),
        norito::json!({"block_height": "2"}),
    ] {
        report.data = norito::json!({"evidence": evidence});
        assert!(carrier_height(&report).is_err());
    }
    assert!(require_deadline(Instant::now()).is_err());
    assert!(require_deadline(Instant::now() + std::time::Duration::from_secs(1)).is_ok());
}

#[test]
fn parent_context_and_certified_tip_are_required_before_preparation_or_dispatch() {
    use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
    use iroha_data_model::{
        sumeragi_finality::SumeragiFinalityProof, transaction::FeePaymentIntent,
    };
    use iroha_model_base::peer::PeerId;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Offline(AtomicUsize);
    impl crate::verify::finality::FinalitySource for Offline {
        type Error = std::io::Error;
        fn finality_proof(
            &self,
            _: NonZeroU64,
        ) -> std::result::Result<SumeragiFinalityProof, Self::Error> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(std::io::Error::other("offline fixture"))
        }
        fn latest_attestation(
            &self,
            _: &PeerId,
            _: &[u8; 32],
        ) -> std::result::Result<crate::verify::finality::FinalityAttestation, Self::Error>
        {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(std::io::Error::other("offline fixture"))
        }
    }
    let mut fixture = Fixture::new();
    fixture.parent_receipt();
    let dir = tempfile::tempdir().unwrap();
    let bootstrap = fixture.bootstrap();
    let mut parent = ParentFinalityStore::open(&dir.path().join("parent"), &bootstrap).unwrap();
    let mut store =
        AttachmentStore::open(&dir.path().join("attachment"), fixture.identity.clone()).unwrap();
    let key = KeyPair::from_seed(vec![47; 32], Algorithm::Ed25519);
    let config = Config::load_table(
        "attachment-tests.toml",
        toml::toml! {
            chain = (fixture.parent.chain_id())
            network_id = (fixture.parent.network_id().to_string())
            torii_url = "https://parent.example/"
            [account]
            chain_discriminant = 753
            public_key = (key.public_key().to_string())
            private_key = (ExposedPrivateKey(key.private_key().clone()).to_string())
        },
    )
    .unwrap();
    let options = BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(vec![], None),
        max_total_fees: std::collections::BTreeMap::default(),
        deadline: Instant::now() + std::time::Duration::from_secs(5),
    };
    let offline = Offline(AtomicUsize::new(0));
    let mut wrong = config.clone();
    wrong.torii_api_url = "https://foreign.example/".parse().unwrap();
    assert!(
        store
            .advance_parent(&wrong, &bootstrap, &mut parent, &offline, &options, None)
            .is_err()
    );
    assert_eq!(offline.0.load(Ordering::Relaxed), 0);
    assert!(
        store
            .advance_parent(&config, &bootstrap, &mut parent, &offline, &options, None)
            .is_err()
    );
    assert_eq!(offline.0.load(Ordering::Relaxed), 1);
    assert!(!store.progress(None).pending);
    assert!(!store.directory.path().join("transactions").exists());
    assert!(store.confirmed().is_none());
}

#[test]
fn expired_unsigned_parent_requests_remain_terminal_under_fresh_turn_budgets() {
    use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
    use iroha_data_model::{
        account::address::ChainDiscriminantGuard, sumeragi_finality::SumeragiFinalityProof,
        transaction::FeePaymentIntent,
    };
    use iroha_model_base::peer::PeerId;
    use std::time::Duration;

    // The wallet checks its schema's exact JSON field order, including unsigned
    // requests. Keep this expired input fixture typed rather than sorting its keys
    // through a generic JSON object; the production wallet remains its only reader.
    #[derive(norito::derive::JsonSerialize)]
    struct Terms {
        max_total_fees: Vec<norito::json::Value>,
        deadline_ms: u64,
    }
    #[derive(norito::derive::JsonSerialize)]
    #[norito(tag = "kind", content = "value", rename_all = "snake_case")]
    enum RequestKind<'a> {
        PrivateRootRegistration {
            alias: &'a str,
            expected_ownership_generation: u64,
            registration: &'a PrivateDataspaceRegistration,
            terms: &'a Terms,
        },
        PrivateRootAnchor {
            state: &'a PrivateDataspaceAnchorState,
            anchor: &'a PrivateDataspaceAnchor,
            terms: &'a Terms,
        },
    }
    #[derive(norito::derive::JsonSerialize)]
    struct Request<'a> {
        schema: &'a str,
        torii_url: &'a str,
        chain_id: &'a str,
        network_id: NetworkId,
        chain_discriminant: u16,
        account_id: &'a AccountId,
        operation: RequestKind<'a>,
        requested_fee: &'a FeePaymentIntent,
        deadline_ms: u64,
    }

    struct CertifiedTip(SumeragiFinalityProof);
    impl crate::verify::finality::FinalitySource for CertifiedTip {
        type Error = std::io::Error;
        fn finality_proof(&self, height: NonZeroU64) -> std::io::Result<SumeragiFinalityProof> {
            assert_eq!(height.get(), self.0.height());
            Ok(self.0.clone())
        }
        fn latest_attestation(
            &self,
            _: &PeerId,
            _: &[u8; 32],
        ) -> std::io::Result<crate::verify::finality::FinalityAttestation> {
            panic!("historical certificate refresh must not infer quorum readiness")
        }
        fn latest_finality_proof(
            &self,
            _: &PeerId,
            _: &[u8; 32],
        ) -> std::io::Result<SumeragiFinalityProof> {
            Ok(self.0.clone())
        }
    }

    for anchor in [false, true] {
        let mut fixture = Fixture::new();
        let (receipt, verifier) = fixture.parent_receipt();
        let root = tempfile::tempdir().unwrap();
        let bootstrap = fixture.bootstrap();
        let mut parent =
            ParentFinalityStore::open(&root.path().join("parent"), &bootstrap).unwrap();
        let mut store =
            AttachmentStore::open(&root.path().join("attachment"), fixture.identity.clone())
                .unwrap();
        let key = KeyPair::from_seed(vec![47; 32], Algorithm::Ed25519);
        let config = Config::load_table(
            "expired-parent-request.toml",
            toml::toml! {
                chain = (fixture.parent.chain_id())
                network_id = (fixture.parent.network_id().to_string())
                torii_url = "https://parent.example/"
                [account]
                chain_discriminant = 753
                public_key = (key.public_key().to_string())
                private_key = (ExposedPrivateKey(key.private_key().clone()).to_string())
            },
        )
        .unwrap();
        let _profile = ChainDiscriminantGuard::enter(config.account_chain_discriminant);
        let options = BoundedTransactionOptions {
            fee_payment: FeePaymentIntent::authority(vec![], None),
            max_total_fees: std::collections::BTreeMap::default(),
            deadline: Instant::now() + Duration::from_secs(30),
        };
        let terms = Terms {
            max_total_fees: vec![],
            deadline_ms: 1,
        };
        let kind = if anchor {
            store.confirm_with_verifier(receipt, &verifier).unwrap();
            let state = store.confirmed_child_state().unwrap().clone();
            let anchor = fixture.child_anchor();
            PendingKind::Anchor {
                previous: Box::new(state),
                anchor: Box::new(anchor),
            }
        } else {
            PendingKind::Registration
        };
        let pending = PendingOperation {
            kind,
            checkpoint: fixture.parent.checkpoint().encode_canonical().unwrap(),
            replay: None,
        };
        let journal = store
            .directory
            .ensure_child("transactions")
            .unwrap()
            .path()
            .join(pending.journal_name().unwrap());
        store.publish_pending(pending.clone()).unwrap();
        // A canonical, already expired unsigned request is enough to exercise recovery.
        // Its child certificates are genuine fixture certificates; no payload, signature,
        // dispatch or parent inclusion is fabricated by this local custody fixture.
        let operation = match &pending.kind {
            PendingKind::Registration => RequestKind::PrivateRootRegistration {
                alias: &fixture.identity.alias,
                expected_ownership_generation: fixture.identity.ownership_generation,
                registration: &fixture.identity.registration,
                terms: &terms,
            },
            PendingKind::Anchor { previous, anchor } => RequestKind::PrivateRootAnchor {
                state: previous,
                anchor,
                terms: &terms,
            },
        };
        let request = Request {
            schema: "iroha.wallet.native-preparation.v1",
            torii_url: config.torii_api_url.as_str(),
            chain_id: config.chain.as_str(),
            network_id: config.network_id,
            chain_discriminant: config.account_chain_discriminant,
            account_id: &config.account,
            operation,
            requested_fee: &options.fee_payment,
            deadline_ms: 1,
        };
        drop(iroha_operation_journal::Journal::create_preparation(&journal, &request).unwrap());
        let original = std::fs::read(journal.join("preparation.json")).unwrap();
        let pending = store
            .directory
            .read("attachment.nrt", MAX_RECORD_BYTES)
            .unwrap();
        let confirmed = store.confirmed();
        let source = CertifiedTip(fixture.parent.latest().clone());
        for seconds in [30, 60] {
            let fresh = BoundedTransactionOptions {
                deadline: Instant::now() + Duration::from_secs(seconds),
                ..options.clone()
            };
            let progress = store
                .advance_parent(&config, &bootstrap, &mut parent, &source, &fresh, None)
                .unwrap();
            assert_eq!(progress.transaction_status, Some(OperationStatus::Expired));
            assert_eq!(progress.confirmed, confirmed);
            assert!(progress.pending);
            assert_eq!(
                std::fs::read(journal.join("preparation.json")).unwrap(),
                original
            );
            assert_eq!(
                store
                    .directory
                    .read("attachment.nrt", MAX_RECORD_BYTES)
                    .unwrap(),
                pending
            );
            for record in [
                "payload.json",
                "operation.json",
                "submission.json",
                "applied.json",
                "retired.json",
            ] {
                assert!(
                    !journal.join(record).exists(),
                    "expired request created {record}"
                );
            }
        }
    }
}

#[test]
fn confirmed_receipt_and_pending_clear_are_one_durable_publication() {
    let mut fixture = Fixture::new();
    let original = fixture.parent.checkpoint().encode_canonical().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("attachment");
    let mut store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    store
        .publish_pending(PendingOperation {
            kind: PendingKind::Registration,
            checkpoint: original,
            replay: None,
        })
        .unwrap();
    let (receipt, verifier) = fixture.parent_receipt();
    let expected = store
        .confirm_completed_operation(receipt, &verifier)
        .unwrap();
    assert!(!store.progress(None).pending);
    assert_eq!(store.confirmed(), Some(expected));
    drop(store);
    let store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    assert!(!store.progress(None).pending);
    assert_eq!(store.confirmed(), Some(expected));
}

#[test]
fn failed_atomic_completion_preserves_old_receipt_and_pending_then_reopens_exact_work() {
    let mut fixture = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("attachment");
    let mut store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    let (first_receipt, first_verifier) = fixture.parent_receipt();
    store
        .confirm_with_verifier(first_receipt, &first_verifier)
        .unwrap();
    let old = store.confirmed();
    // Replaying the already registered identity is sufficient to test atomic completion
    // without inventing a child execution result or a fake parent finality capability.
    store
        .publish_pending(PendingOperation {
            kind: PendingKind::Registration,
            checkpoint: fixture.parent.checkpoint().encode_canonical().unwrap(),
            replay: None,
        })
        .unwrap();
    let retained_bytes = store
        .directory
        .read("attachment.nrt", MAX_RECORD_BYTES)
        .unwrap();
    let (receipt, verifier) = fixture.parent_receipt();
    std::fs::remove_file(path.join("attachment.nrt")).unwrap();
    std::fs::create_dir(path.join("attachment.nrt")).unwrap();
    assert!(
        store
            .confirm_completed_operation(receipt.clone(), &verifier)
            .is_err()
    );
    assert_eq!(store.confirmed(), old);
    assert!(store.progress(None).pending);
    assert!(store.revalidate().is_err());
    drop(store);
    std::fs::remove_dir(path.join("attachment.nrt")).unwrap();
    PrivateDirectory::open(&path)
        .unwrap()
        .write_atomic("attachment.nrt", &retained_bytes, PublishMode::CreateNew)
        .unwrap();
    let mut store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    assert_eq!(store.confirmed(), old);
    assert!(store.progress(None).pending);
    store
        .confirm_completed_operation(receipt, &verifier)
        .unwrap();
    assert!(!store.progress(None).pending);
}

#[test]
fn cancelled_attachment_refuses_new_parent_work_but_preserves_pending_reconciliation() {
    use iroha_crypto::{Algorithm, ExposedPrivateKey, KeyPair};
    use iroha_data_model::{
        sumeragi_finality::SumeragiFinalityProof, transaction::FeePaymentIntent,
    };
    use iroha_model_base::peer::PeerId;
    use std::cell::Cell;
    struct Offline(Cell<usize>);
    impl crate::verify::finality::FinalitySource for Offline {
        type Error = std::io::Error;
        fn finality_proof(
            &self,
            _: NonZeroU64,
        ) -> std::result::Result<SumeragiFinalityProof, Self::Error> {
            self.0.set(self.0.get() + 1);
            Err(std::io::Error::other("offline cancellation fixture"))
        }
        fn latest_attestation(
            &self,
            _: &PeerId,
            _: &[u8; 32],
        ) -> std::result::Result<crate::verify::finality::FinalityAttestation, Self::Error>
        {
            self.0.set(self.0.get() + 1);
            Err(std::io::Error::other("offline cancellation fixture"))
        }
    }
    struct NoLocalRead;
    impl PrivateRootSource for NoLocalRead {
        fn anchor(&self, _: NonZeroU64) -> Result<Option<PrivateDataspaceAnchor>> {
            panic!("cancelled new relay must not read a child successor")
        }
    }
    let mut fixture = Fixture::new();
    fixture.parent_receipt();
    let dir = tempfile::tempdir().unwrap();
    let bootstrap = fixture.bootstrap();
    let mut parent = ParentFinalityStore::open(&dir.path().join("parent"), &bootstrap).unwrap();
    let path = dir.path().join("attachment");
    let mut store = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    let signal = Arc::new(AtomicBool::new(true));
    store.bind_cancellation(Arc::clone(&signal)).unwrap();
    store.bind_cancellation(Arc::clone(&signal)).unwrap();
    assert!(
        store
            .bind_cancellation(Arc::new(AtomicBool::new(false)))
            .is_err()
    );
    let key = KeyPair::from_seed(vec![47; 32], Algorithm::Ed25519);
    let config = Config::load_table(
        "attachment-tests.toml",
        toml::toml! {
            chain = (fixture.parent.chain_id())
            network_id = (fixture.parent.network_id().to_string())
            torii_url = "https://parent.example/"
            [account]
            chain_discriminant = 753
            public_key = (key.public_key().to_string())
            private_key = (ExposedPrivateKey(key.private_key().clone()).to_string())
        },
    )
    .unwrap();
    let options = BoundedTransactionOptions {
        fee_payment: FeePaymentIntent::authority(vec![], None),
        max_total_fees: std::collections::BTreeMap::default(),
        deadline: Instant::now() + std::time::Duration::from_secs(10),
    };
    let source = Offline(Cell::new(0));
    assert!(matches!(
        store.advance_parent(&config, &bootstrap, &mut parent, &source, &options, None),
        Err(AttachmentError::Cancelled)
    ));
    assert!(matches!(
        store.relay_once(
            &NoLocalRead,
            &mut RelayParent {
                config: &config,
                bootstrap: &bootstrap,
                finality: &mut parent,
                source: &source,
                options: &options
            }
        ),
        Err(AttachmentError::Cancelled)
    ));
    assert_eq!(source.0.get(), 0);
    assert!(!store.progress(None).pending);
    assert!(!store.directory.path().join("transactions").exists());
    // Existing exact operation state still enters independent read-only reconciliation.
    store
        .publish_pending(PendingOperation {
            kind: PendingKind::Registration,
            checkpoint: fixture.parent.checkpoint().encode_canonical().unwrap(),
            replay: None,
        })
        .unwrap();
    let original = store
        .directory
        .read("attachment.nrt", MAX_RECORD_BYTES)
        .unwrap();
    assert!(!matches!(
        store.advance_parent(&config, &bootstrap, &mut parent, &source, &options, None),
        Ok(_) | Err(AttachmentError::Cancelled)
    ));
    assert_eq!(source.0.get(), 1);
    assert_eq!(
        store
            .directory
            .read("attachment.nrt", MAX_RECORD_BYTES)
            .unwrap(),
        original
    );
    assert!(!store.directory.path().join("transactions").exists());
    drop(store);
    let reopened = AttachmentStore::open(&path, fixture.identity.clone()).unwrap();
    assert_eq!(
        reopened
            .directory
            .read("attachment.nrt", MAX_RECORD_BYTES)
            .unwrap(),
        original
    );
    assert!(reopened.progress(None).pending);
    assert!(signal.load(Ordering::Acquire));
}
