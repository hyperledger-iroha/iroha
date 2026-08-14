//! External qualification and committed-visibility tests for reputation publication.
use super::*;
#[test]
fn publication_construction_rejects_signer_handle_substitution() {
    let projector_root = TempDir::new().expect("projector root");
    let publication_root = TempDir::new().expect("publication root");
    let trust = trust_policy();
    let ingest = ingest_policy(&trust);
    let projector =
        Arc::new(ReputationIngestService::open(projector_root.path(), ingest).expect("projector"));
    let policy = ReputationPublicationPolicyV1::try_new(
        &trust,
        "signer-a",
        "dag-a",
        b"peer-a".to_vec(),
        SigningKey::from_bytes(&[0xB1; 32])
            .verifying_key()
            .to_bytes(),
        REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1,
    )
    .expect("publication policy");
    let signer_qualification = policy.threshold_signer_qualification();
    let dag_qualification = policy.governance_dag_qualification();
    let result = ReputationPublicationReconcilerV1::open(
        publication_root.path(),
        projector,
        trust,
        policy,
        Arc::new(NullThresholdSigner {
            handle: "signer-b".to_owned(),
            qualification: signer_qualification,
        }),
        Arc::new(NullGovernanceDag {
            handle: "dag-a".to_owned(),
            qualification: dag_qualification,
        }),
    );
    assert!(matches!(
        result,
        Err(ReputationRuntimeError::RuntimeBindingMismatch)
    ));
}
#[test]
fn governance_dag_qualification_binds_peer_identity_and_ed25519_key() {
    let first_key = SigningKey::from_bytes(&[0xB1; 32])
        .verifying_key()
        .to_bytes();
    let second_key = SigningKey::from_bytes(&[0xB2; 32])
        .verifying_key()
        .to_bytes();
    let expected =
        reputation_governance_dag_policy_digest_v1(b"peer-a", first_key).expect("policy digest");
    assert_eq!(
        expected,
        reputation_governance_dag_policy_digest_v1(b"peer-a", first_key)
            .expect("stable policy digest")
    );
    assert_ne!(
        expected,
        reputation_governance_dag_policy_digest_v1(b"peer-b", first_key)
            .expect("peer-bound policy digest")
    );
    assert_ne!(
        expected,
        reputation_governance_dag_policy_digest_v1(b"peer-a", second_key)
            .expect("key-bound policy digest")
    );
    assert!(matches!(
        reputation_governance_dag_policy_digest_v1(b"", first_key),
        Err(ReputationRuntimeError::InvalidRuntimePolicy)
    ));
    assert!(matches!(
        reputation_governance_dag_policy_digest_v1(b"peer-a", [0; 32]),
        Err(ReputationRuntimeError::InvalidRuntimePolicy)
    ));
}
#[test]
fn publication_construction_rejects_same_key_different_peer_qualification() {
    let projector_root = TempDir::new().expect("projector root");
    let publication_root = TempDir::new().expect("publication root");
    let trust = trust_policy();
    let projector = Arc::new(
        ReputationIngestService::open(projector_root.path(), ingest_policy(&trust))
            .expect("projector"),
    );
    let publisher_key = SigningKey::from_bytes(&[0xB1; 32])
        .verifying_key()
        .to_bytes();
    let policy = ReputationPublicationPolicyV1::try_new(
        &trust,
        "signer-a",
        "dag-a",
        b"peer-a".to_vec(),
        publisher_key,
        REPUTATION_RUNTIME_MIN_CHECKPOINT_BYTES_V1,
    )
    .expect("publication policy");
    let substituted_peer_qualification = ReputationRuntimeProviderQualificationV1::new(
        REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
        reputation_governance_dag_policy_digest_v1(b"peer-b", publisher_key)
            .expect("substituted peer policy digest"),
    );
    let result = ReputationPublicationReconcilerV1::open(
        publication_root.path(),
        projector,
        trust,
        policy.clone(),
        Arc::new(NullThresholdSigner {
            handle: "signer-a".to_owned(),
            qualification: policy.threshold_signer_qualification(),
        }),
        Arc::new(NullGovernanceDag {
            handle: "dag-a".to_owned(),
            qualification: substituted_peer_qualification,
        }),
    );
    assert!(matches!(
        result,
        Err(ReputationRuntimeError::RuntimeBindingMismatch)
    ));
    assert!(
        !publication_root
            .path()
            .join(REPUTATION_PUBLICATION_CHECKPOINT_FILE_NAME_V1)
            .exists(),
        "peer substitution must fail before publication state opens"
    );
}
#[test]
fn governance_readback_is_discarded_when_provider_qualification_drifts() {
    let projector_root = TempDir::new().expect("projector root");
    let publication_root = TempDir::new().expect("publication root");
    let trust = trust_policy();
    let projector = Arc::new(
        ReputationIngestService::open(projector_root.path(), ingest_policy(&trust))
            .expect("projector"),
    );
    let policy = publication_policy(&trust);
    let signed = signed_snapshot(&trust, [0xD8; 16], None, FINALIZED_AT_MS / 1_000);
    let block = governance_block_after(&signed, None);
    let readback = ReputationGovernanceDagReadbackV1 {
        version: REPUTATION_GOVERNANCE_DAG_READBACK_VERSION_V1,
        head: governance_head(std::slice::from_ref(&block)),
        inclusion_path: vec![block],
    };
    let pending = StoredReputationPublicationV1 {
        sequence: 1,
        material_digest: [0xD9; 32],
        signed_result_digest: signed_result_digest(&signed).expect("signed result digest"),
        signed_result: signed,
        governance_acknowledgement: None,
        governance_readback: None,
    };
    let request = governance_publication_request(&pending).expect("publication request");
    let reconciler = ReputationPublicationReconcilerV1::open(
        publication_root.path(),
        projector,
        trust,
        policy.clone(),
        Arc::new(NullThresholdSigner {
            handle: "signer-a".to_owned(),
            qualification: policy.threshold_signer_qualification(),
        }),
        Arc::new(DriftingGovernanceDag {
            handle: "dag-a".to_owned(),
            qualification: Mutex::new(policy.governance_dag_qualification()),
            readback,
        }),
    )
    .expect("open reconciler");
    assert!(matches!(
        reconciler.reconcile_governance_publication(&request),
        Err(ReputationRuntimeError::RuntimeBindingChanged)
    ));
    assert!(
        reconciler
            .pending_publication()
            .expect("read durable publication state")
            .is_none(),
        "a receipt returned during qualification drift must not become durable"
    );
}
#[test]
fn committed_projection_is_gated_idempotent_restart_safe_and_corruption_strict() {
    let projector_root = TempDir::new().expect("projector root");
    let publication_root = TempDir::new().expect("publication root");
    let trust = trust_policy();
    let projector = Arc::new(
        ReputationIngestService::open(projector_root.path(), ingest_policy(&trust))
            .expect("projector"),
    );
    let policy = publication_policy(&trust);
    let reconciler = open_publication_reconciler(
        publication_root.path(),
        Arc::clone(&projector),
        trust.clone(),
        policy.clone(),
    );
    assert_eq!(
        reconciler
            .committed_read_projection()
            .expect("empty projection"),
        ReputationCommittedReadProjectionV1::empty(
            policy.digest().expect("publication policy digest")
        )
    );
    let signed = signed_snapshot(&trust, [0xE1; 16], None, FINALIZED_AT_MS / 1_000);
    signed
        .verify(&trust, FINALIZED_AT_MS / 1_000)
        .expect("threshold-signed fixture");
    let delivery = signing_delivery(&signed);
    reconciler
        .validate_signed_result(&delivery, &signed)
        .expect("signed result binds exact delivery");
    reconciler
        .store_signed_result(&delivery, signed.clone())
        .expect("stage signed result");
    assert!(
        reconciler
            .committed_read_projection()
            .expect("staged projection")
            .latest
            .is_none(),
        "threshold signing alone must not enter the committed projection"
    );
    let digest = signed_result_digest(&signed).expect("signed result digest");
    let (acknowledgement, governance_readback) = governance_readback(
        &policy,
        delivery.sequence,
        delivery.material_digest,
        digest,
        &signed,
    );
    assert!(matches!(
        reconciler.complete_publication(acknowledgement),
        Err(ReputationRuntimeError::PublicationCheckpointConflict)
    ));
    assert!(
        reconciler
            .committed_read_projection()
            .expect("pre-readback projection")
            .latest
            .is_none(),
        "a structurally valid but unobserved acknowledgement must not publish"
    );
    reconciler
        .store_governance_readback(acknowledgement, governance_readback)
        .expect("store authoritative DAG readback");
    assert!(
        reconciler
            .committed_read_projection()
            .expect("DAG-staged projection")
            .latest
            .is_none(),
        "DAG readback must remain hidden until projector acknowledgement"
    );
    reconciler
        .complete_publication(acknowledgement)
        .expect("commit authoritative projection");
    let committed = reconciler
        .committed_read_projection()
        .expect("committed projection");
    assert_eq!(committed.events.len(), 1);
    assert_eq!(
        reconciler
            .committed_snapshot_by_id(signed.snapshot.snapshot_id)
            .expect("committed snapshot lookup"),
        Some(signed.snapshot.clone())
    );
    assert_eq!(
        committed
            .latest
            .as_ref()
            .expect("latest committed snapshot")
            .signed_result,
        signed
    );
    assert_eq!(committed.events[0].snapshot_id, signed.snapshot.snapshot_id);
    reconciler
        .complete_publication(acknowledgement)
        .expect("exact completion replay");
    assert_eq!(
        reconciler
            .committed_read_projection()
            .expect("idempotent projection"),
        committed,
        "an exact replay must not duplicate committed history"
    );
    drop(reconciler);
    let restored = open_publication_reconciler(
        publication_root.path(),
        Arc::clone(&projector),
        trust.clone(),
        policy.clone(),
    );
    assert_eq!(
        restored
            .committed_read_projection()
            .expect("restored projection"),
        committed
    );
    drop(restored);
    fs::write(
        publication_root
            .path()
            .join(REPUTATION_PUBLICATION_CHECKPOINT_FILE_NAME_V1),
        [0xFF, 0x00, 0x01],
    )
    .expect("write corrupt publication checkpoint");
    let signer_qualification = policy.threshold_signer_qualification();
    let dag_qualification = policy.governance_dag_qualification();
    assert!(matches!(
        ReputationPublicationReconcilerV1::open(
            publication_root.path(),
            projector,
            trust,
            policy,
            Arc::new(NullThresholdSigner {
                handle: "signer-a".to_owned(),
                qualification: signer_qualification,
            }),
            Arc::new(NullGovernanceDag {
                handle: "dag-a".to_owned(),
                qualification: dag_qualification,
            }),
        ),
        Err(ReputationRuntimeError::InvalidCheckpoint) | Err(ReputationRuntimeError::CheckpointIo)
    ));
}
