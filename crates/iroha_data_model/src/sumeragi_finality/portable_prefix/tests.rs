//! Genuine certificate/row acquisition, substitution, boundary and refusal regression tests.

use super::*;
use crate::sumeragi_finality::test_fixtures::NativeFinalityFixture;
use iroha_allocation::AllocationBudget;
use iroha_sumeragi::availability::{AvailabilitySource, BodyRestoration, PayloadBytes};

fn reader(fixture: &NativeFinalityFixture) -> PortablePrefixVerifierV1 {
    let selected = SumeragiFinalityVerifier::new(
        fixture.genesis(),
        fixture.chain_id(),
        fixture.genesis_proof().committee.clone(),
    )
    .unwrap();
    let genesis = decode_framed_signed_block(&fixture.genesis_proof().block_wire).unwrap();
    PortablePrefixVerifierV1::new(
        &selected,
        genesis.commit_certificate().unwrap().result_preimage(),
    )
    .unwrap()
}

fn proof_from_original(
    reader: &PortablePrefixVerifierV1,
    proof: &SumeragiFinalityProof,
    parent: BlockHeader,
) -> PortablePrefixFrameV1 {
    // Test-only native source construction: authenticate the original fixture payload under
    // its exact genuine signed manifest before lending its immutable bytes to export.
    let block = decode_framed_signed_block(&proof.block_wire).unwrap();
    let original = block.commit_certificate().unwrap();
    let certificate = SumeragiCommitCertificateV1 {
        consensus_header: original.consensus_header().to_vec(),
        commit_qc: original.commit_qc().to_vec(),
        result_preimage: original.result_preimage().to_vec(),
    };
    let mut acquisition = reader
        .acquisition(certificate, original.availability().to_vec(), parent)
        .unwrap();
    let prepared = &acquisition.prepared;
    let source = AvailabilitySource::new(
        prepared.header.instance,
        prepared.header.height,
        prepared.core_hash,
        prepared.config.clone(),
    )
    .unwrap();
    let mut payload = Vec::new();
    block.write_resultless_proposal_wire(&mut payload).unwrap();
    let budget = AllocationBudget::new(64 * 1024 * 1024);
    let body = BodyRestoration::new(
        source,
        prepared.header.clone(),
        prepared.availability.clone(),
        PayloadBytes::from_untrusted(payload).unwrap(),
    )
    .complete(&budget, &prepared.crypto)
    .unwrap_or_else(|_| panic!("original signed payload custody"));
    acquisition.acquire_from_body(&body).unwrap();
    assert!(acquisition.requested_row().unwrap().is_none());
    assert_eq!(acquisition.proof().unwrap().executed_parent_header, parent);
    acquisition
        .into_proof()
        .unwrap_or_else(|_| panic!("complete original row prefix"))
}

fn resign(qc: &mut Qc, signers: &[usize]) {
    let mut keys: Vec<_> = (1..=4)
        .map(|seed| iroha_crypto::KeyPair::from_seed(vec![seed; 32], Algorithm::BlsNormal))
        .collect();
    keys.sort_by_key(|key| key.public_key().try_to_bytes().unwrap().1.to_vec());
    qc.signers =
        iroha_sumeragi::types::Bitmap::from_indices(4, signers.iter().map(|index| *index as u32))
            .unwrap();
    let shares: Vec<_> = signers
        .iter()
        .map(|index| consensus::sign_preimage(keys[*index].private_key(), &qc.preimage()).unwrap())
        .collect();
    qc.agg_sig = AggregateSignature(consensus::aggregate(&shares).unwrap());
}

#[test]
fn portable_prefix_certifies_published_parent_state_and_exact_undo_headers() {
    let mut fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let h2 = fixture.latest().clone();
    let mut verifier = reader(&fixture);
    let first = proof_from_original(&verifier, &h2, fixture.genesis().header());
    let genesis = verifier.verify(&first).unwrap();
    assert_eq!(genesis.checkpoint().header(), &fixture.genesis().header());
    assert!(genesis.checkpoint().core_header().is_none());
    assert!(genesis.predecessor().is_none());
    let block = fixture.block_with_submitted_work(fixture.next_header());
    let h3 = fixture.certify(block);
    let mut second = proof_from_original(&verifier, &h3, h2.block_header);
    // The synthetic execution fixture deliberately distinguishes published P_H from X_H.
    // Real BLS Commit signatures bind the modified original result; no exporter key does.
    let published: KeyedStateRoot = Hash::new(b"published state after H2 publication").into();
    let (header, mut result) = CertifiedResultV1::decode(
        &second.certificate.result_preimage,
        CertifiedResultScope::TairaGlobal,
    )
    .unwrap()
    .into_parts();
    result.execution.parent_world_state_root = published;
    second.certificate.result_preimage =
        CertifiedResultV1::new(header, result, CertifiedResultScope::TairaGlobal)
            .unwrap()
            .preimage()
            .unwrap();
    let mut qc: Qc = norito::decode_canonical(&second.certificate.commit_qc).unwrap();
    qc.result = result_of_preimage(&second.certificate.result_preimage).unwrap();
    resign(&mut qc, &[0, 1, 2]);
    second.certificate.commit_qc = norito::encode_canonical(&qc).unwrap();
    let bytes = second.to_canonical_bytes().unwrap();
    let roundtrip = PortablePrefixFrameV1::decode_canonical(&bytes).unwrap();
    assert_eq!(roundtrip, second);
    let checkpoint = verifier.verify(&roundtrip).unwrap();
    assert_eq!(checkpoint.network_id(), fixture.network_id());
    assert_eq!(checkpoint.chain_id(), fixture.chain_id());
    assert_eq!(checkpoint.checkpoint().header(), &h2.block_header);
    assert_eq!(
        checkpoint.predecessor().unwrap().header(),
        &fixture.genesis().header()
    );
    assert_eq!(checkpoint.published_root(), published);
    assert_ne!(
        checkpoint.published_root(),
        checkpoint
            .checkpoint()
            .certified_result()
            .commitment()
            .execution
            .world_state_root
    );
    assert_eq!(checkpoint.successor_header().height, 3);
    assert_eq!(checkpoint.successor_core_hash(), qc.block_hash);
    assert_eq!(checkpoint.successor_result_hash(), qc.result);
    assert_eq!(verifier.height(), 3);
    assert!(
        verifier.verify(&roundtrip).is_err(),
        "replay cannot advance the prefix"
    );
}

#[test]
fn portable_prefix_authenticates_exact_parent_da_bundles_and_refuses_substitutions() {
    use crate::da::{
        commitment::{DaCommitmentRecord, DaProofScheme},
        ingest::{
            DaIngestAuthorizationV1, DaIngestSignatureV1, DaPinScopeAuthorizationV1, DaPinScopeV1,
        },
        pin_intent::DaPinIntent,
        prelude::RetentionPolicy,
        types::{BlobDigest, StorageTicketId},
    };
    use crate::sorafs::pin_registry::ManifestDigest;
    use iroha_crypto::{KeyPair, Signature};
    use iroha_model_base::topology::LaneId;

    let mut fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let h2 = fixture.latest().clone();
    let mut verifier = reader(&fixture);
    verifier
        .verify(&proof_from_original(
            &verifier,
            &h2,
            fixture.genesis().header(),
        ))
        .unwrap();
    let key = KeyPair::from_seed(vec![0xD9; 32], Algorithm::Ed25519);
    let ticket = StorageTicketId::new([0x66; 32]);
    let manifest = ManifestDigest::new([0x22; 32]);
    let commitments = DaCommitmentBundle::new(vec![DaCommitmentRecord::new(
        LaneId::SINGLE,
        1,
        1,
        BlobDigest::new([0x11; 32]),
        manifest,
        DaProofScheme::MerkleSha256,
        Hash::prehashed([0x33; 32]),
        Some(Hash::prehashed([0x55; 32])),
        RetentionPolicy::default(),
        ticket,
        Signature::try_new(key.private_key(), b"original DA acknowledgement").unwrap(),
    )]);
    let mut authorization = DaIngestAuthorizationV1 {
        network_id: NetworkId::from_genesis_hash(fixture.genesis().hash()),
        owner: crate::account::AccountId::new(key.public_key().clone()),
        lane_id: LaneId::SINGLE,
        epoch: 1,
        sequence: 1,
        payload_hash: BlobDigest::new([0x11; 32]),
        payload_bytes: 1,
        request_content_hash: Hash::prehashed([0xDC; 32]),
        signatures: Vec::new(),
    };
    authorization.signatures.push(DaIngestSignatureV1 {
        signer: key.public_key().clone(),
        signature: Signature::try_new(key.private_key(), &authorization.signing_digest()).unwrap(),
    });
    let scope = DaPinScopeV1::new(&authorization, ticket, manifest, Some("portable-da".into()));
    let pins = DaPinIntentBundle::new(vec![DaPinIntent::new(
        authorization,
        DaPinScopeAuthorizationV1::try_sign(scope, &key).unwrap(),
    )]);
    let mut parent = fixture.block_with_submitted_work(fixture.next_header());
    parent.set_da_commitments(Some(commitments.clone()));
    parent.set_da_pin_intents(Some(pins.clone()));
    // Changing proposal DA clears its previous execution result. Reattach the explicit
    // synthetic success row only after the final DA commitments have been selected.
    NativeFinalityFixture::install_network_results(&mut parent, vec![Ok(Vec::new())]);
    let h3 = fixture.certify(parent);
    verifier
        .verify(&proof_from_original(&verifier, &h3, h2.block_header))
        .unwrap();
    let block = fixture.block_with_submitted_work(fixture.next_header());
    let h4 = fixture.certify(block);
    let original = proof_from_original(&verifier, &h4, h3.block_header)
        .with_parent_da_bundles(Some(commitments.clone()), Some(pins.clone()));
    for mutation in 0..6 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.parent_da_commitments = None,
            1 => changed.parent_da_pin_intents = None,
            2 => changed.parent_da_commitments = Some(DaCommitmentBundle::default()),
            3 => {
                changed.parent_da_pin_intents.as_mut().unwrap().intents[0].alias =
                    Some("foreign".into())
            }
            4 => changed.parent_da_pin_intents.as_mut().unwrap().version += 1,
            5 => {
                let mut records = commitments.commitments().to_vec();
                records[0].client_blob_id = BlobDigest::new([0xAB; 32]);
                changed.parent_da_commitments = Some(DaCommitmentBundle::new(records));
            }
            _ => unreachable!(),
        }
        assert!(verifier.verify(&changed).is_err(), "mutation {mutation}");
        assert_eq!(verifier.height(), 3);
    }
    let bytes = original.to_canonical_bytes().unwrap();
    let roundtrip = PortablePrefixFrameV1::decode_canonical(&bytes).unwrap();
    assert_eq!(roundtrip, original);
    let refusal = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 1, usize::MAX),
        || authenticate_parent_da(&roundtrip),
    )
    .unwrap_err();
    assert!(matches!(refusal, FinalityReadError::DecodeResource(_)));
    assert_eq!(verifier.height(), 3);
    let sealed = verifier.verify(&roundtrip).unwrap();
    assert_eq!(sealed.checkpoint().da_commitments(), Some(&commitments));
    assert_eq!(sealed.checkpoint().da_pin_intents(), Some(&pins));
    assert!(sealed.predecessor().unwrap().da_commitments().is_none());
    assert!(sealed.predecessor().unwrap().da_pin_intents().is_none());
}

#[test]
fn portable_prefix_rejects_source_coordinates_missing_extra_and_corrupt_rows() {
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let proof = proof_from_original(
        &reader(&fixture),
        fixture.latest(),
        fixture.genesis().header(),
    );
    for mutation in 0..9 {
        let mut changed = proof.clone();
        match mutation {
            0 => changed.rows.clear(),
            1 => changed.rows.push(changed.rows.last().unwrap().clone()),
            2 => changed.rows[0].bytes[0] ^= 1,
            3 => changed.rows[0].height += 1,
            4 => changed.rows[0].instance.0[0] ^= 1,
            5 => changed.rows[0].block_hash.0[0] ^= 1,
            6 => changed.rows[0].index += 1,
            7 => {
                changed.rows[0].bytes.pop();
            }
            8 => changed.executed_parent_header.creation_time_ms += 1,
            _ => unreachable!(),
        }
        let mut verifier = reader(&fixture);
        assert!(verifier.verify(&changed).is_err(), "mutation {mutation}");
        assert_eq!(verifier.height(), 1);
        verifier.verify(&proof).unwrap();
    }
    for bytes in [
        Vec::new(),
        vec![0; MAX_PORTABLE_PREFIX_FRAME_BYTES + 1],
        [proof.to_canonical_bytes().unwrap(), vec![0]].concat(),
    ] {
        assert!(PortablePrefixFrameV1::decode_canonical(&bytes).is_err());
    }
}

#[test]
fn portable_prefix_exact_quorum_and_instance_fail_without_advancing() {
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let proof = proof_from_original(
        &reader(&fixture),
        fixture.latest(),
        fixture.genesis().header(),
    );
    for signers in [vec![0, 1], vec![0, 1, 2, 3]] {
        let mut changed = proof.clone();
        let mut qc: Qc = norito::decode_canonical(&changed.certificate.commit_qc).unwrap();
        resign(&mut qc, &signers);
        changed.certificate.commit_qc = norito::encode_canonical(&qc).unwrap();
        let mut verifier = reader(&fixture);
        assert!(
            verifier.verify(&changed).is_err(),
            "genuine BLS requires exactly n-f"
        );
        assert_eq!(verifier.height(), 1);
    }
    let mut changed = proof.clone();
    let mut header: CoreHeader =
        norito::decode_canonical(&changed.certificate.consensus_header).unwrap();
    header.instance.0[0] ^= 1;
    changed.certificate.consensus_header = norito::encode_canonical(&header).unwrap();
    assert!(reader(&fixture).verify(&changed).is_err());
    let other = NativeFinalityFixture::start_with_explicit_parameters("portable-other-chain");
    assert!(reader(&other).verify(&proof).is_err());
}

#[test]
fn portable_prefix_walks_every_boundary_with_original_historical_authority() {
    let (fixture, chain) =
        NativeFinalityFixture::short_npos_boundary_chain_with_explicit_parameters(4);
    let mut verifier = reader(&fixture);
    let mut frames = Vec::new();
    for pair in chain.windows(2) {
        let frame = proof_from_original(&verifier, &pair[1], pair[0].block_header);
        let sealed = verifier.verify(&frame).unwrap();
        assert_eq!(sealed.checkpoint().header(), &pair[0].block_header);
        assert_eq!(
            sealed.checkpoint().certified_result().commitment().height,
            pair[0].height()
        );
        frames.push(frame);
    }
    assert_eq!(verifier.height(), 4);
    assert_eq!(
        verifier
            .tip
            .certified_result
            .commitment()
            .schedule
            .current
            .authorization
            .epoch,
        1
    );
    assert!(
        reader(&fixture).verify(&frames[2]).is_err(),
        "successor committee cannot skip its certified boundary"
    );
    let mut verifier = reader(&fixture);
    verifier.verify(&frames[0]).unwrap();
    let mut corrupt = frames[1].clone();
    let mut qc: Qc = norito::decode_canonical(&corrupt.certificate.commit_qc).unwrap();
    qc.agg_sig.0[0] ^= 1;
    corrupt.certificate.commit_qc = norito::encode_canonical(&qc).unwrap();
    assert!(verifier.verify(&corrupt).is_err());
    assert!(verifier.verify(&frames[2]).is_err());
    assert_eq!(verifier.height(), 2);
    verifier.verify(&frames[1]).unwrap();
    verifier.verify(&frames[2]).unwrap();
}

#[test]
fn portable_prefix_local_refusal_retains_original_row_and_unadvanced_verifier() {
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let mut verifier = reader(&fixture);
    let frame = proof_from_original(&verifier, fixture.latest(), fixture.genesis().header());
    let mut acquisition = verifier
        .acquisition(
            frame.certificate.clone(),
            frame.availability.clone(),
            frame.executed_parent_header,
        )
        .unwrap();
    let expected = acquisition.requested_row().unwrap();
    let row = frame.rows[0].clone();
    let pointer = row.bytes.as_ptr();
    let (retained, refusal) = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, usize::MAX),
        || acquisition.push(row),
    )
    .unwrap_err();
    assert!(
        matches!(refusal, FinalityReadError::DecodeResource(ref error) if error.local_refusal().is_some())
    );
    assert_eq!(retained.bytes.as_ptr(), pointer);
    assert_eq!(acquisition.requested_row().unwrap(), expected);
    assert!(acquisition.proof().is_err());
    acquisition.push(retained).unwrap();
    for row in frame.rows.iter().skip(1) {
        acquisition.push(row.clone()).unwrap();
    }
    let original = acquisition.proof().unwrap();
    let refusal = norito::core::with_decode_limits_scope(
        norito::core::DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, usize::MAX),
        || verifier.verify(original),
    )
    .unwrap_err();
    assert!(matches!(refusal, FinalityReadError::DecodeResource(_)));
    assert_eq!(verifier.height(), 1);
    verifier.verify(original).unwrap();
    assert_eq!(verifier.height(), 2);
}
