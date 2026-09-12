//! Captured original identities at finalized archive and digest boundaries.
//!
//! The fixture contains original compiler metadata, not historical whole-frame goldens.
//! These tests use the existing archive fixtures; they do not supply finality authority.

use super::*;
use norito::{NoritoSchema, core::NoritoDeserialize, core::NoritoSerialize, json::Value};

fn observed<T: NoritoSchema>(identifier: &str, expected_directions: &[&str]) {
    let rows: Vec<Value> = norito::json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/core/finalized_query_frame_identity_observations.v1.json"
    )))
    .expect("immutable original compiler observations");
    assert_eq!(rows.len(), 49);
    let mut directions = BTreeSet::new();
    for row in rows
        .iter()
        .filter(|row| row.get("identifier").and_then(Value::as_str) == Some(identifier))
    {
        let field = |key| row.get(key).and_then(Value::as_str).expect("captured text");
        assert!(directions.insert(field("direction")));
        assert_eq!(T::nominal_name(), field("nominal"));
        assert_eq!(std::any::type_name::<T>(), field("nominal"));
        assert_eq!(T::frame_name(), field("root_hint"));
        let observed: Vec<u8> = field("schema_hash")
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(
            norito::schema::identity::frame_hash::<T>().as_slice(),
            observed
        );
    }
    assert_eq!(directions, expected_directions.iter().copied().collect());
}

fn roundtrip<T: NoritoSerialize + for<'de> NoritoDeserialize<'de>>(value: &T) {
    let frame = norito::encode_canonical(value).expect("canonical frame");
    let view = norito::core::from_bytes_view(&frame).expect("frame and checksum");
    assert_eq!(view.schema(), norito::schema::identity::frame_hash::<T>());
    let ambient = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let _ambient = norito::core::DecodeFlagsGuard::enter(ambient);
    assert_eq!(norito::encode_canonical(value).unwrap(), frame);
    let decoded: T = norito::decode_canonical(&frame).expect("typed roundtrip");
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), frame);
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    let mut wrong_root = frame.clone();
    wrong_root[6] ^= 1;
    assert!(norito::decode_canonical::<T>(&wrong_root).is_err());
    let mut corrupt = frame.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(norito::decode_canonical::<T>(&corrupt).is_err());
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    assert_eq!(norito::core::get_decode_flags(), ambient);
}

fn check<T: Clone + NoritoSerialize + for<'de> NoritoDeserialize<'de>>(name: &str, value: &T) {
    observed::<T>(name, &["serialize", "deserialize"]);
    roundtrip(value);
    roundtrip(&Option::<T>::None);
    roundtrip(&Some(value.clone()));
    roundtrip(&Vec::<T>::new());
    roundtrip(&vec![value.clone(), value.clone()]);
}

fn framed_digest<T: NoritoSerialize>(domain: &[u8], value: &T) -> [u8; 32] {
    let bytes = norito::to_bytes(value).expect("actual committed frame");
    let view = norito::core::from_bytes_view(&bytes).expect("frame and checksum");
    assert_eq!(view.schema(), norito::schema::identity::frame_hash::<T>());
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&bytes);
    *hasher.finalize().as_bytes()
}

#[test]
fn archive_fixture_pin_policy_respects_approval_lifecycle() {
    let valid = projection(7);
    valid.validate(bounds()).expect("valid archive fixture");
    for provider in &valid.providers {
        for archived in &provider.orders {
            let pin = &archived.pin_manifest;
            assert_eq!(pin.approved_epoch, Some(1));
            assert_eq!(
                pin.policy.retention_epoch,
                archived.replication_order.deadline_epoch
            );
            for invalid_retention in [0, pin.approved_epoch.unwrap()] {
                let mut invalid = pin.clone();
                invalid.policy.retention_epoch = invalid_retention;
                assert!(matches!(
                    validate_pin_manifest_lifecycle(&invalid),
                    Err(ProviderIngestFinalizedArchiveErrorV1::InvalidProjection {
                        reason: "pin-manifest lifecycle state is noncanonical"
                    })
                ));
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn captured_finalized_provider_archive_frames() {
    let directory = physical_tempdir().expect("archive directory");
    let archive = ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds())
        .expect("open archive");
    let first = projection(7);
    archive
        .insert(first.clone())
        .expect("publish actual record");
    let bytes = fs::read(archive.record_path(&first.key).unwrap()).expect("persisted record");
    let record: ProviderIngestFinalizedArchiveRecordV1 =
        decode_from_bytes_with_limits(&bytes, bounds().decode_limits().unwrap())
            .expect("bounded record");
    record.validate().expect("valid persisted record");
    assert_eq!(encode_bounded_record(&record, bounds()).unwrap(), bytes);
    check("ProviderIngestFinalizedArchiveKeyV1", &first.key);
    check("ProviderIngestFinalizedArchiveRecordV1", &record);
    check(
        "ProviderIngestFinalizedArchiveRecordMaterialV1",
        &record.material,
    );
    check(
        "ProviderIngestFinalizedProviderProjectionV1",
        &first.providers[0],
    );
    assert_eq!(
        record.record_digest,
        framed_digest(RECORD_DIGEST_DOMAIN_V1, &record.material)
    );
    assert_eq!(
        provider_state_root(&first.providers).unwrap(),
        framed_digest(STATE_ROOT_DOMAIN_V1, &first.providers)
    );
    let link = ProviderIngestFinalizedPrefixLinkV1 {
        previous_cumulative_digest: None,
        key: first.key,
        record_digest: record.record_digest,
    };
    check("ProviderIngestFinalizedPrefixLinkV1", &link);
    assert_eq!(
        canonical_domain_digest(PREFIX_DIGEST_DOMAIN_V1, &link).unwrap(),
        framed_digest(PREFIX_DIGEST_DOMAIN_V1, &link)
    );
    let page = archive
        .read_provider_page(&first.key, PROVIDER_A, None, 1)
        .unwrap();
    check("ProviderIngestFinalizedArchivePageV1", &page);
    check(
        "ProviderIngestFinalizedArchiveCursorV1",
        page.next_cursor.as_ref().expect("continuation"),
    );
    let (_, prepared, proposal) = prepared_compaction_for_test(&archive, first.key);
    prepared
        .checkpoint
        .validate(bounds())
        .expect("valid prepared checkpoint");
    check(
        "ProviderIngestFinalizedArchiveCheckpointV1",
        &prepared.checkpoint,
    );
    check(
        "ProviderIngestFinalizedArchiveCheckpointMaterialV1",
        &prepared.checkpoint.material,
    );
    check(
        "ProviderIngestFinalizedArchiveCompactionProposalMaterialV1",
        &proposal.material,
    );
    assert_eq!(
        encode_bounded_checkpoint(&prepared.checkpoint, bounds()).unwrap(),
        prepared.canonical_bytes
    );
    assert_eq!(
        prepared.checkpoint.checkpoint_digest,
        framed_digest(CHECKPOINT_DIGEST_DOMAIN_V1, &prepared.checkpoint.material)
    );
    let before = (
        archive_namespace_snapshot(&archive.records),
        archive_namespace_snapshot(&archive.checkpoints),
    );
    drop(archive);
    let reopened =
        ProviderIngestFinalizedArchiveV1::try_open(archive_root(&directory), bounds()).unwrap();
    assert_eq!(
        reopened
            .read_provider_page(&first.key, PROVIDER_A, None, 1)
            .unwrap(),
        page
    );
    assert_eq!(
        (
            archive_namespace_snapshot(&reopened.records),
            archive_namespace_snapshot(&reopened.checkpoints)
        ),
        before
    );
}

#[test]
fn captured_finalized_provider_approval_frames() {
    let authority = TestRetentionAuthority::new();
    let proposal = ProviderIngestFinalizedArchiveCompactionProposalV1::try_new(
        retention_fence(key(7), 1),
        [0xC3; 32],
        [0xC4; 32],
    )
    .unwrap();
    let approval = ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        authority.binding().qualification(),
        proposal,
        None,
        None,
    )
    .unwrap();
    check(
        "ProviderIngestFinalizedArchiveCompactionProposalMaterialV1",
        &approval.material.proposal.material,
    );
    check(
        "ProviderIngestFinalizedArchiveRetentionApprovalMaterialV1",
        &approval.material,
    );
    check(
        "ProviderIngestFinalizedArchiveRetentionApprovalRecordV1",
        &approval,
    );
    assert_eq!(
        approval.revision,
        framed_digest(RETENTION_APPROVAL_REVISION_DOMAIN_V1, &approval.material)
    );
    let bytes = approval.to_canonical_bytes().unwrap();
    assert_eq!(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&bytes)
            .unwrap(),
        approval
    );
    let mut forged = approval.clone();
    forged.material.sequence = 0;
    assert!(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(
            &norito::to_bytes(&forged).unwrap()
        )
        .is_err()
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        ProviderIngestFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&trailing)
            .is_err()
    );
}
