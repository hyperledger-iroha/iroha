//! Genuine native certificates and explicit mock custody exercise online-prefix publication.
use super::*;
use crate::kagemusha_wallet_state_v1::tests::{Wallet, fixture, synthetic_payout_wallet};
use iroha_data_model::sumeragi_finality::{
    WorldStateElementKindV1, WorldStateSnapshotEntryV1, test_fixtures::NativeFinalityFixture,
    world_state_value_hash_v1,
};
fn wallet(native: &NativeFinalityFixture) -> Wallet {
    let mut scheme: KagemushaWalletSchemeV1 = fixture("KagemushaWalletSchemeV1");
    scheme.network_id = *native.network_id().as_bytes();
    synthetic_payout_wallet(scheme, native.chain_id().to_owned())
}
// This is an explicit recursive-proof workload substitute, never a qualified source grant.
// The production ingress, native certificates, paired selection and cleanup are unchanged.
fn ingest(
    w: &mut Wallet,
    genesis: &SumeragiFinalityVerifier,
    original: &[u8],
) -> Result<LedgerProgressV1, Error> {
    w.ingest_ledger_transition(
        genesis,
        original,
        |wallet, manifest| {
            manifest
                .recursive_checkpoint
                .map(|address| {
                    let bytes = wallet.archive.read_object(
                        &address,
                        crate::kagemusha_wallet_finality_v1::HISTORY_ORIGINAL_MAX_BYTES_V1,
                    )?;
                    archive::decode::<u64>(&bytes)
                })
                .transpose()
        },
        |next| *next,
        |_, _, block| {
            let next = block
                .height()
                .checked_add(1)
                .ok_or(Error::Invalid("test height"))?;
            Ok((next, archive::encode(&next)?))
        },
    )
}
fn certify(native: &mut NativeFinalityFixture) -> Vec<u8> {
    let block = native.block_with_submitted_work(native.next_header());
    archive::encode(&native.certify(block)).unwrap()
}
fn restart(wallet: Wallet) -> Wallet {
    Coordinator::new(
        wallet.custody,
        wallet.archive,
        wallet.proofs,
        wallet.scheme_id,
        wallet.wallet_id,
    )
    .unwrap()
}
#[test]
fn enrollment_cannot_publish_a_ledger_checkpoint_before_bootstrap() {
    let native = NativeFinalityFixture::start("wallet-ledger-before-bootstrap");
    let mut w = crate::kagemusha_wallet_state_v1::tests::wallet();
    assert!(matches!(
        ingest(
            &mut w,
            &native.verifier(),
            &archive::encode(native.genesis_proof()).unwrap()
        ),
        Err(Error::NoHead)
    ));
    assert!(w.custody.archive_checkpoint().unwrap().is_none());
    assert!(w.manifest().unwrap().1.indexed.is_none());
}
#[test]
fn paired_publication_requires_recursive_success_and_restorable_selected_original() {
    let mut native = NativeFinalityFixture::start("wallet-ledger-paired-transition");
    let genesis = native.verifier();
    let mut w = wallet(&native);
    let first = archive::encode(native.genesis_proof()).unwrap();
    let selected = ingest(&mut w, &genesis, &first).unwrap();
    let (root, manifest) = w.manifest().unwrap();
    assert!(manifest.ledger_checkpoint.is_some());
    let recursive = manifest.recursive_checkpoint.unwrap();
    // Exact same-decision retry restores the selected pair without another proof workload.
    let retry = w
        .ingest_ledger_transition(
            &genesis,
            &first,
            |_, _| Ok(Some(2_u64)),
            |next| *next,
            |_, _, _| panic!("same-decision retry must not prove again"),
        )
        .unwrap();
    assert_eq!(retry, selected);
    let second = certify(&mut native);
    let error = w
        .ingest_ledger_transition(
            &genesis,
            &second,
            |_, _| Ok(Some(2_u64)),
            |next| *next,
            |_, _, _| {
                Err(Error::ArtifactsUnavailable(
                    "explicit test recursive workload",
                ))
            },
        )
        .unwrap_err();
    assert!(matches!(error, Error::ArtifactsUnavailable(_)));
    assert_eq!(w.manifest().unwrap().0, root);
    assert_eq!(
        w.ledger_progress_selected(&genesis).unwrap(),
        Some(selected)
    );
    w.archive.remove(ArchiveKey::Object(recursive)).unwrap();
    assert!(matches!(
        ingest(&mut w, &genesis, &first),
        Err(Error::WitnessLost(_))
    ));
    assert_eq!(w.manifest().unwrap().0, root);
}
#[test]
fn recursive_height_or_decode_failure_precedes_new_original_publication() {
    let mut native = NativeFinalityFixture::start("wallet-ledger-prefix-mutations");
    let genesis = native.verifier();
    let mut w = wallet(&native);
    let first = archive::encode(native.genesis_proof()).unwrap();
    ingest(&mut w, &genesis, &first).unwrap();
    let second = certify(&mut native);
    let (root, mut manifest) = w.manifest().unwrap();
    let marker = archive::encode(&99_u64).unwrap();
    let marker_address =
        crate::kagemusha_wallet_advance_v1::kagemusha_wallet_archive_object_digest_v1(&marker);
    assert!(
        w.archive
            .get(ArchiveKey::Object(marker_address), marker.len())
            .unwrap()
            .is_none()
    );
    let error = w
        .ingest_ledger_transition(
            &genesis,
            &second,
            |_, _| Ok(Some(99_u64)),
            |next| *next,
            |_, _, _| panic!("wrong restored height must refuse before proving"),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        Error::WitnessLost("recursive native height binding")
    ));
    let error = w
        .ingest_ledger_transition(
            &genesis,
            &second,
            |_, _| Ok(Some(2_u64)),
            |next| *next,
            |_, _, _| Ok((99, marker.clone())),
        )
        .unwrap_err();
    assert!(matches!(error, Error::Proof("recursive successor height")));
    assert_eq!(w.manifest().unwrap().0, root);
    assert!(
        w.archive
            .get(ArchiveKey::Object(marker_address), marker.len())
            .unwrap()
            .is_none()
    );
    // Explicit mock-custody corruption selects a content-addressed but noncanonical prefix.
    // Restoration must fail before the successor proof step or native tip can advance.
    manifest.recursive_checkpoint = Some(
        w.archive
            .write_object(
                b"malformed",
                crate::kagemusha_wallet_finality_v1::HISTORY_ORIGINAL_MAX_BYTES_V1,
            )
            .unwrap(),
    );
    w.publish_manifest(root, &manifest).unwrap();
    let corrupted_root = w.manifest().unwrap().0;
    assert!(matches!(
        ingest(&mut w, &genesis, &second),
        Err(Error::WitnessLost(_))
    ));
    assert_eq!(w.manifest().unwrap().0, corrupted_root);
    assert_eq!(
        w.ledger_progress_selected(&genesis)
            .unwrap()
            .unwrap()
            .height,
        1
    );
}
#[test]
fn ledger_original_bounds_refuse_empty_trailing_oversized_and_checkpoint_frames() {
    assert!(proof_original(&[]).is_err());
    assert!(payout_original(&[]).is_err());
    assert!(payout_original(&vec![0; PAYOUT_RECORD_MAX_BYTES_V1 + 1]).is_err());
    let native = NativeFinalityFixture::start("ledger-original-bound");
    let mut raw = archive::encode(native.genesis_proof()).unwrap();
    assert!(proof_original(&raw).is_ok());
    raw.push(0);
    assert!(matches!(proof_original(&raw), Err(Error::Invalid(_))));
    assert!(matches!(payout_original(&raw), Err(Error::Invalid(_))));
    assert!(proof_original(&native.checkpoint().encode_canonical().unwrap()).is_err());
    assert!(proof_original(&vec![0; LEDGER_PROOF_MAX_BYTES_V1 + 1]).is_err());
}
#[test]
fn ledger_continuity_exact_retry_foreign_root_and_selected_original_loss() {
    let mut native = NativeFinalityFixture::start("wallet-ledger-continuity");
    let genesis = native.verifier();
    let mut w = wallet(&native);
    assert_eq!(w.ledger_progress_selected(&genesis).unwrap(), None);
    let first = archive::encode(native.genesis_proof()).unwrap();
    let second = certify(&mut native);
    assert!(ingest(&mut w, &genesis, &second).is_err());
    let one = ingest(&mut w, &genesis, &first).unwrap();
    assert_eq!(one.height, 1);
    assert_eq!(ingest(&mut w, &genesis, &first).unwrap(), one);
    let mut alternate: SumeragiFinalityProof = archive::decode(&first).unwrap();
    alternate.block_wire[0] ^= 1;
    assert!(ingest(&mut w, &genesis, &archive::encode(&alternate).unwrap()).is_err());
    let selected_checkpoint = w.manifest().unwrap().1.ledger_checkpoint;
    let mut foreign = NativeFinalityFixture::start("foreign-ledger");
    assert!(ingest(&mut w, &genesis, &certify(&mut foreign)).is_err());
    assert_eq!(w.ledger_progress_selected(&genesis).unwrap(), Some(one));
    assert_eq!(
        w.manifest().unwrap().1.ledger_checkpoint,
        selected_checkpoint
    );
    let two = ingest(&mut w, &genesis, &second).unwrap();
    assert_eq!(two.height, 2);
    assert!(ingest(&mut w, &genesis, &first).is_err());
    let mut w = restart(w);
    assert_eq!(w.ledger_progress_selected(&genesis).unwrap(), Some(two));
    assert!(w.ledger_progress_selected(&foreign.verifier()).is_err());
    let (_, manifest) = w.manifest().unwrap();
    assert!(manifest.ledger_retired.is_none());
    w.archive
        .remove(ArchiveKey::Object(manifest.ledger_checkpoint.unwrap()))
        .unwrap();
    assert!(matches!(
        w.ledger_progress_selected(&genesis),
        Err(Error::WitnessLost(_))
    ));
    assert!(ingest(&mut w, &genesis, &second).is_err());
}
#[test]
fn ledger_fresh_and_same_height_ingress_reject_wrong_committee_and_nonexact_quorums() {
    use iroha_data_model::block::{CommitCertificate, decode_framed_signed_block};
    use iroha_sumeragi::{message::Qc, types::Bitmap};
    let mut native = NativeFinalityFixture::start("wallet-ledger-quorum");
    let genesis = native.verifier();
    let original = certify(&mut native);
    let proof: SumeragiFinalityProof = archive::decode(&original).unwrap();
    let mut malformed = Vec::new();
    let mut foreign = proof.clone();
    foreign.committee[0].proof_of_possession[0] ^= 1;
    malformed.push(archive::encode(&foreign).unwrap());
    for count in [2u32, 4] {
        let mut wrong = proof.clone();
        let mut block = decode_framed_signed_block(&wrong.block_wire).unwrap();
        let certificate = block.commit_certificate().unwrap();
        let mut qc: Qc = norito::decode_canonical(certificate.commit_qc()).unwrap();
        qc.signers = Bitmap::from_indices(4, 0..count).unwrap();
        block.set_commit_certificate(Some(CommitCertificate::from_untrusted_parts(
            certificate.consensus_header().to_vec(),
            norito::encode_canonical(&qc).unwrap(),
            certificate.result_preimage().to_vec(),
            certificate.availability().to_vec(),
        )));
        wrong.block_wire = block.encode_wire().unwrap();
        malformed.push(archive::encode(&wrong).unwrap());
    }
    let mut w = wallet(&native);
    let genesis_progress = ingest(
        &mut w,
        &genesis,
        &archive::encode(native.genesis_proof()).unwrap(),
    )
    .unwrap();
    for raw in &malformed {
        assert!(ingest(&mut w, &genesis, raw).is_err());
        assert_eq!(
            w.ledger_progress_selected(&genesis).unwrap(),
            Some(genesis_progress)
        );
    }
    let selected = ingest(&mut w, &genesis, &original).unwrap();
    for raw in &malformed {
        assert!(ingest(&mut w, &genesis, raw).is_err());
        assert_eq!(
            w.ledger_progress_selected(&genesis).unwrap(),
            Some(selected)
        );
    }
    assert_eq!(ingest(&mut w, &genesis, &original).unwrap(), selected);
}
#[test]
fn ledger_uncertain_publication_reconciles_selected_tip_and_bounds_cleanup() {
    for after in [false, true] {
        let mut native = NativeFinalityFixture::start("wallet-ledger-uncertain");
        let genesis = native.verifier();
        let mut w = wallet(&native);
        let first = archive::encode(native.genesis_proof()).unwrap();
        w.custody.fail_publication = Some(after);
        assert!(ingest(&mut w, &genesis, &first).is_err());
        let mut w = restart(w);
        assert_eq!(
            w.ledger_progress_selected(&genesis).unwrap().is_some(),
            after
        );
        let manifest = w.manifest().unwrap().1;
        assert_eq!(manifest.ledger_checkpoint.is_some(), after);
        assert_eq!(manifest.recursive_checkpoint.is_some(), after);
        let one = ingest(&mut w, &genesis, &first).unwrap();
        assert_eq!(one.height, 1);
        let second = certify(&mut native);
        w.archive.fail_remove = true;
        assert!(ingest(&mut w, &genesis, &second).is_err());
        let (_, manifest) = w.manifest().unwrap();
        assert!(manifest.ledger_retired.is_some());
        assert_ne!(manifest.ledger_checkpoint, manifest.ledger_retired);
        let mut w = restart(w);
        assert_eq!(
            w.ledger_progress_selected(&genesis)
                .unwrap()
                .unwrap()
                .height,
            2
        );
        w.custody.fail_publication = Some(after);
        assert!(ingest(&mut w, &genesis, &second).is_err());
        // The old object was already removed. A lost cleanup selection cannot make its
        // absence corrupt the still-selected new checkpoint, and cleanup remains idempotent.
        let mut w = restart(w);
        assert_eq!(ingest(&mut w, &genesis, &second).unwrap().height, 2);
        assert!(w.manifest().unwrap().1.ledger_retired.is_none());
        assert!(w.manifest().unwrap().1.recursive_retired.is_none());
    }
}
#[test]
fn payout_originals_require_selected_tip_exact_world_and_row_before_custody_release() {
    let mut native = NativeFinalityFixture::start("wallet-ledger-payout");
    let genesis = native.verifier();
    let mut w = wallet(&native);
    crate::kagemusha_wallet_state_v1::fee_claims::tests::seed_ledger_acknowledgement(&mut w);
    let payout = KagemushaWalletPayoutRecordV1 {
        key: KagemushaWalletPayoutKeyV1::Fee([2; 32]),
        source: [3; 32],
        amount: 17,
        transaction: [5; 32],
    };
    let world = WorldStateSnapshotV1 {
        schema_hash: iroha_crypto::Hash::new(b"native payout ingress schema"),
        entries: vec![WorldStateSnapshotEntryV1 {
            field_id: "world.kagemusha_wallet_ledger".into(),
            kind: WorldStateElementKindV1::Table,
            key_hash: Some(world_state_value_hash_v1(&payout.key.ledger_key(w.scheme_id)).unwrap()),
            value_hash: world_state_value_hash_v1(&norito::to_bytes(&payout).unwrap()).unwrap(),
        }],
    };
    let world_bytes = archive::encode(&world).unwrap();
    let payout_bytes = archive::encode(&payout).unwrap();
    assert!(
        w.acknowledge_payout_originals(&genesis, [2; 32], &world_bytes, &payout_bytes)
            .is_err()
    );
    ingest(
        &mut w,
        &genesis,
        &archive::encode(native.genesis_proof()).unwrap(),
    )
    .unwrap();
    let block = native.block_with_submitted_work(native.next_header());
    let original =
        archive::encode(&native.certify_with_world_root(block, world.root().unwrap())).unwrap();
    ingest(&mut w, &genesis, &original).unwrap();
    let mut foreign = payout;
    foreign.amount += 1;
    assert!(
        w.acknowledge_payout_originals(
            &genesis,
            [2; 32],
            &world_bytes,
            &archive::encode(&foreign).unwrap()
        )
        .is_err()
    );
    let mut wrong_world = world;
    wrong_world.schema_hash = iroha_crypto::Hash::new(b"wrong");
    assert!(
        w.acknowledge_payout_originals(
            &genesis,
            [2; 32],
            &archive::encode(&wrong_world).unwrap(),
            &payout_bytes
        )
        .is_err()
    );
    assert!(
        w.archive
            .get(ArchiveKey::FeeClaim([2; 32]), 128)
            .unwrap()
            .is_some()
    );
    for after in [false, true] {
        w.custody.fail_publication = Some(after);
        assert!(
            w.acknowledge_payout_originals(&genesis, [2; 32], &world_bytes, &payout_bytes)
                .is_err()
        );
        assert!(
            w.archive
                .get(ArchiveKey::FeeClaim([2; 32]), 128)
                .unwrap()
                .is_some()
        );
        w = restart(w);
    }
    w.acknowledge_payout_originals(&genesis, [2; 32], &world_bytes, &payout_bytes)
        .unwrap();
    assert_eq!(
        w.archive.get(ArchiveKey::FeeClaim([2; 32]), 128).unwrap(),
        None
    );
    let mut w = restart(w);
    w.acknowledge_payout_originals(&genesis, [2; 32], &world_bytes, &payout_bytes)
        .unwrap();
    assert!(
        w.acknowledge_payout_originals(&genesis, [9; 32], &world_bytes, &payout_bytes)
            .is_err()
    );
}
