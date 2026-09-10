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

#[cfg(unix)]
#[test]
fn captured_finalized_reputation_archive_frames() {
    let directory = tempdir().expect("archive directory");
    let archive = open_archive(&directory, bounds());
    let projection = sample_projection(7, [0x71; 32]);
    archive
        .insert(projection.clone())
        .expect("publish actual anchor");
    let path = archive.record_path(&projection.key).unwrap();
    let anchor = archive
        .load_anchor_at(&path, Some(&projection.key))
        .expect("load actual anchor");
    anchor.validate_standalone().expect("valid anchor");
    check("ReputationFinalizedProjectionV1", &projection);
    check("ReputationFinalizedArchiveKeyV1", &projection.key);
    check("PersistedReputationFinalizedAnchorV1", &anchor);
    check("ReputationFinalizedAnchorManifestV1", &anchor.manifest);
    check("ReputationFinalizedAnchorDeltaV1", &anchor.delta);
    assert_eq!(
        anchor.manifest_digest,
        framed_digest(MANIFEST_DIGEST_DOMAIN_V1, &anchor.manifest)
    );
    assert_eq!(
        anchor.delta_digest,
        framed_digest(DELTA_DIGEST_DOMAIN_V1, &anchor.delta)
    );
    let material = ReputationFinalizedAnchorDigestMaterialV1 {
        version: anchor.version,
        manifest_digest: anchor.manifest_digest,
        delta_digest: anchor.delta_digest,
    };
    // This signing/digest material deliberately has no decoder.
    observed::<ReputationFinalizedAnchorDigestMaterialV1>(
        "ReputationFinalizedAnchorDigestMaterialV1",
        &["serialize"],
    );
    assert_eq!(
        anchor.anchor_digest().unwrap(),
        framed_digest(ANCHOR_DIGEST_DOMAIN_V1, &material)
    );
    for (frame, hash) in [
        (
            norito::encode_canonical(&material).unwrap(),
            norito::schema::identity::frame_hash::<ReputationFinalizedAnchorDigestMaterialV1>(),
        ),
        (
            norito::encode_canonical(&Some(material)).unwrap(),
            norito::schema::identity::frame_hash::<Option<ReputationFinalizedAnchorDigestMaterialV1>>(
            ),
        ),
        (
            norito::encode_canonical(&vec![material, material]).unwrap(),
            norito::schema::identity::frame_hash::<Vec<ReputationFinalizedAnchorDigestMaterialV1>>(
            ),
        ),
    ] {
        assert_eq!(
            norito::core::from_bytes_view(&frame).unwrap().schema(),
            hash
        );
    }
    let policy =
        PersistedReputationAuthorityPolicyV1::try_new(projection.authority_policy.clone()).unwrap();
    policy.validate().expect("valid persisted policy");
    check("PersistedReputationAuthorityPolicyV1", &policy);
    let (checkpoint, bytes, _) = test_checkpoint_artifact(&archive, &projection.key);
    checkpoint.validate_standalone().expect("valid checkpoint");
    check(
        "PersistedReputationFinalizedVirtualBaseCheckpointV1",
        &checkpoint,
    );
    check(
        "ReputationFinalizedVirtualBaseCheckpointV1",
        &checkpoint.checkpoint,
    );
    check(
        "ReputationCheckpointValidationSummaryV1",
        &checkpoint.checkpoint.validation_summary,
    );
    assert_eq!(
        encode_bounded_artifact(&checkpoint, bounds()).unwrap(),
        bytes
    );
    assert_eq!(
        checkpoint.checkpoint.validation_summary_digest,
        framed_digest(
            CHECKPOINT_VALIDATION_DIGEST_DOMAIN_V1,
            &checkpoint.checkpoint.validation_summary
        )
    );
    let persisted = fs::read(&path).unwrap();
    assert_eq!(
        encode_bounded_artifact(&anchor, bounds()).unwrap(),
        persisted
    );
    drop(archive);
    let reopened = open_archive(&directory, bounds());
    assert_eq!(
        reopened
            .load_anchor_at(&path, Some(&projection.key))
            .unwrap(),
        anchor
    );
    assert_eq!(fs::read(&path).unwrap(), persisted);
}

#[test]
fn captured_finalized_reputation_approval_frames() {
    let authority = TestRetentionAuthority::new();
    let proposal = retention_test_proposal(7, 0x31);
    let approval = ReputationFinalizedArchiveRetentionApprovalRecordV1::try_new(
        1,
        authority.binding().qualification(),
        proposal,
        None,
        None,
    )
    .unwrap();
    check(
        "ReputationFinalizedArchiveCompactionProposalMaterialV1",
        &approval.material.proposal.material,
    );
    check(
        "ReputationFinalizedArchiveRetentionApprovalMaterialV1",
        &approval.material,
    );
    check(
        "ReputationFinalizedArchiveRetentionApprovalRecordV1",
        &approval,
    );
    assert_eq!(
        approval.revision,
        framed_digest(RETENTION_APPROVAL_REVISION_DOMAIN_V1, &approval.material)
    );
    let bytes = approval.to_canonical_bytes().unwrap();
    assert_eq!(
        ReputationFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&bytes).unwrap(),
        approval
    );
    let mut forged = approval.clone();
    forged.material.sequence = 0;
    assert!(
        ReputationFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(
            &norito::to_bytes(&forged).unwrap()
        )
        .is_err()
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert!(
        ReputationFinalizedArchiveRetentionApprovalRecordV1::from_canonical_bytes(&trailing)
            .is_err()
    );
}
