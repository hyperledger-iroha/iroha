//! Actual finalized provider authority at each staging callback, with a deterministic clock.

use super::*;
use iroha_core::{
    smartcontracts::isi::sorafs_provider_admission::test_fixture::{
        NOW, ProviderAdmissionTestFixtureV1, key,
    },
    sumeragi::test_chain::Signers,
};
use iroha_data_model::sorafs::pin_registry::{
    ChunkerProfileHandle, ManifestDigest, ManifestRootCid, PinPolicy, StorageClass,
};
use iroha_model_base::metadata::Metadata;
use sorafs_manifest::capacity::{
    REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
    ReplicationOrderV1,
};
use std::num::NonZeroUsize;

struct Fixture {
    native: ProviderAdmissionTestFixtureV1,
    authorization: FinalizedProviderIngestAuthorizationV1,
}

impl Fixture {
    fn new() -> Self {
        let mut native = ProviderAdmissionTestFixtureV1::new_at(NOW);
        native.admit();
        let provider = native.provider();
        let publisher = AccountId::new(key(1).public_key().clone());
        let digest = ManifestDigest::new([0x51; 32]);
        let root = ManifestRootCid::from_blake3_digest([0x52; 32]).unwrap();
        let chunker = ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1f,
        };
        // Order and pin remain valid beyond admission expiry: their reader deadline cannot
        // stand in for the independently expiring, genuinely admitted provider envelope.
        let mut pin = PinManifestRecord::new(
            digest,
            root,
            chunker.clone(),
            [0x53; 32],
            [0x54; 32],
            4096,
            PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: NOW + 7201,
            },
            publisher.clone(),
            NOW + 1,
            None,
            None,
            Metadata::default(),
        );
        pin.approve(NOW + 1, Some([0x55; 32]));
        let canonical = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0x56; 32],
            manifest_cid: root.as_bytes().to_vec(),
            manifest_digest: *digest.as_bytes(),
            chunking_profile: chunker.to_handle(),
            target_replicas: 1,
            assignments: vec![ReplicationAssignmentV1 {
                provider_id: *provider.as_bytes(),
                slice_gib: 1,
                lane: None,
            }],
            issued_at: NOW + 1,
            deadline_at: NOW + 7200,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 99_000,
            },
            metadata: Vec::new(),
        };
        canonical.validate().unwrap();
        let order = ReplicationOrderRecord {
            order_id: ReplicationOrderId::new(canonical.order_id),
            manifest_digest: digest,
            manifest_root_cid: root,
            musubi_archive: None,
            issued_by: publisher,
            issued_epoch: canonical.issued_at,
            deadline_epoch: canonical.deadline_at,
            canonical_order: norito::encode_canonical(&canonical).unwrap(),
            assignment_revision: 1,
            provider_completions: Vec::new(),
            status: ReplicationOrderStatus::Pending,
        };
        native.commit(
            |tx| {
                tx.world.pin_manifests_mut_for_testing().insert(digest, pin);
                tx.world
                    .replication_orders_mut_for_testing()
                    .insert(order.order_id, order);
            },
            true,
        );
        let (height, hash) = {
            let view = native.state.view();
            (
                u64::try_from(view.height()).unwrap(),
                *view.latest_block_hash().unwrap().as_ref(),
            )
        };
        let authorization = FinalizedProviderIngestAuthorizationV1::from_finalized_state(
            height,
            hash,
            *provider.as_bytes(),
            canonical.order_id,
            *digest.as_bytes(),
            root.as_bytes().to_vec(),
            chunker.to_handle(),
            [0x53; 32],
            [0x54; 32],
            4096,
        )
        .unwrap();
        Self {
            native,
            authorization,
        }
    }

    fn head(&self) -> (u64, [u8; 32]) {
        let view = self.native.state.view();
        (
            u64::try_from(view.height()).unwrap(),
            *view.latest_block_hash().unwrap().as_ref(),
        )
    }
}

fn assert_refused(result: std::result::Result<(), StorageError>) {
    assert!(
        matches!(result, Err(StorageError::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied),
        "each callback must establish current native authority"
    );
}

#[test]
fn staged_publisher_callback_refuses_admission_expiry_at_unchanged_head() {
    let fixture = Fixture::new();
    let expiry = fixture.native.envelope().retention_epoch;
    let now = Cell::new(expiry - 1);
    let head = fixture.head();
    let (revision, mut current) = staged_publisher_authority_callback_v1(
        &fixture.native.state,
        &fixture.authorization,
        || Ok(now.get()),
    )
    .unwrap();
    assert_eq!(revision, 1);
    current().unwrap();
    // No elapsed-time sleep or new block: the old same-head 600-second cache accepts here.
    now.set(expiry);
    assert_refused(current());
    now.set(expiry + 1);
    assert_refused(current());
    assert_eq!(fixture.head(), head);
}

#[test]
fn staged_publisher_callback_rechecks_decode_refusal_and_exact_retry() {
    let fixture = Fixture::new();
    let head = fixture.head();
    let (_, mut current) = staged_publisher_authority_callback_v1(
        &fixture.native.state,
        &fixture.authorization,
        || Ok(NOW + 2),
    )
    .unwrap();
    current().unwrap();
    let unavailable = norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64);
    assert_refused(norito::with_decode_limits_scope(unavailable, &mut current));
    assert_eq!(fixture.head(), head);
    current().unwrap();
    assert_eq!(fixture.head(), head);
}

#[test]
fn staged_publisher_callback_rechecks_durable_finality_and_exact_retry() {
    let fixture = Fixture::new();
    let head = fixture.head();
    let chain = fixture.native.chain();
    let (_, original_qc) = chain.committed_body(head.0).unwrap().unwrap();
    let original_qc = norito::encode_canonical(&original_qc).unwrap();
    let (_, mut current) = staged_publisher_authority_callback_v1(
        &fixture.native.state,
        &fixture.authorization,
        || Ok(NOW + 2),
    )
    .unwrap();
    current().unwrap();
    chain.corrupt_local_quorum_for_test(head.0, Signers::BelowQuorum);
    assert_eq!(fixture.head(), head);
    assert_refused(current());
    // Restore precisely the original QC, without publishing a block or changing State.
    chain
        .kura()
        .corrupt_commit_certificate_for_testing(
            NonZeroUsize::new(usize::try_from(head.0).unwrap()).unwrap(),
            Some(original_qc),
        )
        .unwrap();
    current().unwrap();
    assert_eq!(fixture.head(), head);
}
