//! Deliberately isolated release-parameter KAT for the Phase-III terminal proof.
//!
//! This test is ignored in ordinary unit-test runs because it synthesizes and
//! proves the complete canonical admission relation at the maximum eight-fold
//! batch size.  A release engineer must run it in an externally resource-
//! contained process, archive the emitted digest and resource record, and only
//! then pin that digest in the production implementation certificate.

use super::*;
use std::sync::Arc;

use crate::vega::{
    circuit::CircuitAssignment,
    masked_relaxed::{
        MAX_MASKED_RELAXED_STRICT_INSTANCES_V1, MaskedRelaxedRandomErrorV1,
        MaskedRelaxedRandomSourceV1, MaskedRelaxedStreamConfigV1,
        precompute_masked_relaxed_stream_v1,
    },
    sponge::Keccak256,
};
use hex_literal::hex;

use super::super::super::{
    ZK_AMS_ACTION_INDEX_V1, ZkAmsAdmissionPublicInputV1, ZkAmsAdmissionRelationWitnessV1,
    ZkAmsProofContextV1,
};
use super::super::phase23_encrypted::{
    ZkAmsPhase23AccumulatorShapeV1, zk_ams_phase23_release_map_manifest_v1,
};

const RELEASE_TERMINAL_KAT_DOMAIN_V1: &[u8] = b"iroha.zk-ams.v1.phase3.release-terminal-kat";
const RELEASE_TERMINAL_NEGATIVE_CASE_COUNT_V1: u32 = 21;

#[derive(Clone)]
struct ReleaseKatRandom {
    seed: [u8; 32],
    counter: u64,
}

impl ReleaseKatRandom {
    fn new() -> Self {
        Self {
            seed: keccak256(b"iroha.zk-ams.v1.phase3.release-terminal-kat-random"),
            counter: 0,
        }
    }
}

impl MaskedRelaxedRandomSourceV1 for ReleaseKatRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        for (chunk_index, chunk) in destination.chunks_mut(32).enumerate() {
            let mut frame = Vec::with_capacity(48);
            frame.extend_from_slice(&self.seed);
            frame.extend_from_slice(&self.counter.to_be_bytes());
            frame.extend_from_slice(
                &u64::try_from(chunk_index)
                    .map_err(|_| MaskedRelaxedRandomErrorV1::Unavailable)?
                    .to_be_bytes(),
            );
            let block = keccak256(&frame);
            chunk.copy_from_slice(&block[..chunk.len()]);
        }
        self.counter = self
            .counter
            .checked_add(1)
            .ok_or(MaskedRelaxedRandomErrorV1::Unavailable)?;
        Ok(())
    }
}

fn release_proof_context() -> ZkAmsProofContextV1<'static> {
    ZkAmsProofContextV1 {
        chain_id: b"taira-zk-ams-phase3-release-kat",
        genesis_hash: [0x11; 32],
        action_index: ZK_AMS_ACTION_INDEX_V1,
        statement_digest: [0x12; 32],
        parameter_id: [0x13; 32],
        parameter_digest: [0x14; 32],
        verifier_digest: [0x15; 32],
        statement_schema_digest: [0x16; 32],
        engine_manifest_digest: [0x17; 32],
        generator_digest: [0x18; 32],
    }
}

fn release_admission_public() -> ZkAmsAdmissionPublicInputV1 {
    ZkAmsAdmissionPublicInputV1 {
        issuer_key_x: hex!("8e533b6fa0bf7b4625bb30667c01fb607ef9f8b8a80fef5b300628703187b2a3"),
        issuer_key_y: hex!("73eb1dbde03318366d069f83a6f5900053c73633cb041b21c55e1a86c1f400b4"),
        issuer_key_prefix: 0x02,
        issuer_id: [0x31; 32],
        policy_id: [0x35; 32],
        issuer_policy_record_digest: [0x32; 32],
        registry_id: [0x33; 32],
        registry_record_digest: [0x34; 32],
        policy_digest: [0x36; 32],
        phc_hash: hex!("9383ba61dc82dee66ba0210e99a86d9bc45c6ed62c717a111239991e347a3edd"),
        seed_public_key: [0x51; 32],
        prior_registry_root: [0x37; 32],
        next_registry_root: hex!(
            "84e0c6b4ab07ab28b71ad3828e3896e68aa821816c413bba257082df1238a586"
        ),
        current_registry_epoch: 9,
        next_registry_epoch: 10,
        batch_size: 1,
        anchor_index: 0,
    }
}

fn release_admission_assignment(shape: Arc<Shape>) -> CircuitAssignment {
    let public = release_admission_public();
    let subject_commitment = [0x41; 32];
    let credential_nonce = [0x61; 32];
    let signature_r = hex!("3ed113b7883b4c590638379db0c21cda16742ed0255048bf433391d374bc21d1");
    let signature_s = hex!("06d6d7ac6abd44d90dbdf7da0a16796a7228576114ad79a8e8d5ba374fb6a016");
    let recovery_x = signature_r;
    let recovery_y = hex!("9099209accc4c8a224c843afa4f4c68a090d04da5e9889dae2f8eefce82a3740");
    let witness = ZkAmsAdmissionRelationWitnessV1::new(
        &subject_commitment,
        &credential_nonce,
        &signature_r,
        &signature_s,
        &recovery_x,
        &recovery_y,
    )
    .expect("fixed nonzero release witness");
    let assignment = super::super::super::synthesize_admission_with_shape(public, &witness, shape)
        .expect("fixed release assignment must synthesize");
    assignment
        .shape
        .validate_strict_assignment(&assignment.witness, &assignment.public_inputs)
        .expect("fixed release assignment must satisfy the canonical relation");
    assignment
}

fn materialized_digest_for_release_kat(
    materialized: &ZkAmsPhase23MaterializedAccumulatorsV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.phase23.materialized");
    hash.update(&[materialized.version]);
    hash.update(&materialized.profile_digest);
    hash.update(&materialized.roster_digest);
    hash.update(&materialized.transcript_digest);
    hash.update(&materialized.batch_id);
    hash.update(&materialized.ordered_batch_input_digest);
    hash.update(&[materialized.fold_count]);
    for length in [
        materialized.shape.x,
        1,
        materialized.shape.e,
        materialized.shape.r_e,
        materialized.shape.w,
        materialized.shape.r_w,
    ] {
        hash.update(&length.to_be_bytes());
    }
    for family in [
        materialized.x.as_slice(),
        materialized.u.as_slice(),
        materialized.e.as_slice(),
        materialized.r_e.as_slice(),
        materialized.w.as_slice(),
        materialized.r_w.as_slice(),
    ] {
        for value in family {
            hash.update(&value.to_be_bytes());
        }
    }
    hash.finalize()
}

fn encode_mutated_relation(
    governed_count: usize,
    proof: &[u8],
    mutate: impl FnOnce(&mut crate::vega::masked_relaxed::MaskedRelaxedProofWireV1),
) -> Vec<u8> {
    let mut relation =
        super::super::super::decode_zk_ams_admission_relation_wire_v1(governed_count, proof)
            .expect("positive KAT proof must decode exactly");
    mutate(&mut relation);
    super::super::super::encode_zk_ams_admission_relation_wire_v1(relation)
        .expect("bounded structural mutation must remain encodable")
}

#[test]
#[ignore = "release-parameter max-fold proof; run in the isolated release resource harness"]
fn release_terminal_max_fold_kat_emits_candidate_digest() {
    let map_manifest =
        zk_ams_phase23_release_map_manifest_v1().expect("canonical release map manifest");
    let terminal_profile = build_terminal_profile(TerminalRelationSourceV1::CanonicalRelease)
        .expect("canonical terminal profile");
    let shape = Arc::clone(&terminal_profile.shape);
    let release_public_inputs = release_admission_public()
        .to_scalars()
        .expect("fixed release public input must be canonical");
    let strict_public_inputs = vec![release_public_inputs; MAX_MASKED_RELAXED_STRICT_INSTANCES_V1];
    let governed_inputs = strict_public_inputs
        .iter()
        .map(|public_inputs| {
            public_inputs
                .iter()
                .copied()
                .map(Scalar::to_be_bytes)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let ordered_batch_input_digest =
        zk_ams_phase3_ordered_public_inputs_digest_v1(&governed_inputs)
            .expect("maximum governed input digest");
    let context = ZkAmsPhase3TerminalContextV1::new(
        release_profile_v1()
            .digest()
            .expect("release profile digest"),
        [0x21; 32],
        1,
        [0x22; 32],
        [0x23; 32],
        ordered_batch_input_digest,
        terminal_profile.nifs_verifier_digest,
    )
    .expect("release terminal context");
    let governed =
        ZkAmsPhase3GovernedBatchV1::new(context, governed_inputs).expect("maximum governed batch");
    let proof_context = release_proof_context();
    let context_frame = terminal_composition_context_frame(&proof_context, context, &governed)
        .expect("canonical terminal composition context frame");
    let precomputation = precompute_masked_relaxed_stream_v1(
        MaskedRelaxedStreamConfigV1::new(
            super::super::super::COMPOSITION_DOMAIN_V1,
            &context_frame,
            super::super::super::COMMITMENT_KEY_LABEL_V1,
            Arc::clone(&shape),
            &strict_public_inputs,
            1,
        ),
        |_| Ok(release_admission_assignment(Arc::clone(&shape))),
        &mut ReleaseKatRandom::new(),
    )
    .expect("maximum release masked-relaxed precomputation");
    let mask = batch_anchor_from_instance(context, &precomputation.mask_instance)
        .expect("release mask anchor");
    let strict_witness_commitments = precomputation
        .strict_instances
        .iter()
        .map(|instance| commitment_to_wire(&instance.witness_commitment))
        .collect::<Result<Vec<_>, _>>()
        .expect("release strict commitment wires");
    let cross_term_commitments = precomputation
        .folds
        .iter()
        .map(|fold| commitment_to_wire(&fold.cross_term_commitment))
        .collect::<Result<Vec<_>, _>>()
        .expect("release cross-term commitment wires");
    let history = ZkAmsPhase3FoldHistoryV1::new(
        context,
        mask,
        strict_witness_commitments,
        cross_term_commitments,
    )
    .expect("maximum release public fold history");
    let accumulator_shape = ZkAmsPhase23AccumulatorShapeV1::new(
        u32::try_from(precomputation.folded_instance.public_inputs.len())
            .expect("release x count fits u32"),
        u32::try_from(precomputation.folded_witness().error.len())
            .expect("release error count fits u32"),
        u32::try_from(precomputation.folded_witness().error_blindings.len())
            .expect("release error blinding count fits u32"),
        u32::try_from(precomputation.folded_witness().values.len())
            .expect("release witness count fits u32"),
        u32::try_from(precomputation.folded_witness().witness_blindings.len())
            .expect("release witness blinding count fits u32"),
    )
    .expect("release accumulator shape");
    let fold_count =
        u8::try_from(precomputation.strict_instances.len()).expect("maximum fold count fits u8");
    let (instance, witness) = precomputation.into_folded_opening();
    let RelaxedInstance {
        public_inputs,
        relaxation,
        ..
    } = instance;
    let RelaxedWitness {
        values,
        witness_blindings,
        error,
        error_blindings,
    } = witness;
    let mut materialized = ZkAmsPhase23MaterializedAccumulatorsV1 {
        version: 1,
        profile_digest: context.profile_digest,
        roster_digest: context.roster_digest,
        transcript_digest: context.transcript_digest,
        batch_id: context.batch_id,
        ordered_batch_input_digest: context.ordered_batch_input_digest,
        fold_count,
        shape: accumulator_shape,
        x: public_inputs,
        u: vec![relaxation],
        e: error,
        r_e: error_blindings,
        w: values,
        r_w: witness_blindings,
        digest: [0; 32],
    };
    materialized.digest = materialized_digest_for_release_kat(&materialized);
    validate_materialized_accumulators_v1(&materialized)
        .expect("release materialized accumulator validation");
    let materialized_digest = materialized.digest;

    let output = prove_terminal_inner(
        &proof_context,
        context,
        &governed,
        &history,
        materialized,
        TerminalRelationSourceV1::CanonicalRelease,
    )
    .expect("release terminal proof");
    assert!(output.proof_bytes.len() <= ZK_AMS_PHASE3_MAX_TERMINAL_PROOF_BYTES_V1);
    let receipt = verify_terminal_inner(
        &proof_context,
        context,
        &governed,
        &output.batch_anchor,
        TerminalRelationSourceV1::CanonicalRelease,
        &output.proof_bytes,
    )
    .expect("release terminal verification");
    let relation = super::super::super::decode_zk_ams_admission_relation_wire_v1(
        MAX_MASKED_RELAXED_STRICT_INSTANCES_V1,
        &output.proof_bytes,
    )
    .expect("release proof canonical decode");
    assert_eq!(
        super::super::super::encode_zk_ams_admission_relation_wire_v1(relation)
            .expect("release proof canonical re-encode"),
        output.proof_bytes
    );

    let verify = |proof_context: &ZkAmsProofContextV1<'_>,
                  context: ZkAmsPhase3TerminalContextV1,
                  governed: &ZkAmsPhase3GovernedBatchV1,
                  anchor: &ZkAmsPhase3BatchAnchorV1,
                  proof: &[u8]| {
        verify_terminal_inner(
            proof_context,
            context,
            governed,
            anchor,
            TerminalRelationSourceV1::CanonicalRelease,
            proof,
        )
        .is_err()
    };
    let mut rejected = Vec::with_capacity(RELEASE_TERMINAL_NEGATIVE_CASE_COUNT_V1 as usize);
    for index in [
        0,
        output.proof_bytes.len() / 2,
        output.proof_bytes.len() - 1,
    ] {
        let mut corrupt = output.proof_bytes.clone();
        corrupt[index] ^= 1;
        rejected.push(verify(
            &proof_context,
            context,
            &governed,
            &output.batch_anchor,
            &corrupt,
        ));
    }
    for length in [0, 1, output.proof_bytes.len() - 1] {
        rejected.push(verify(
            &proof_context,
            context,
            &governed,
            &output.batch_anchor,
            &output.proof_bytes[..length],
        ));
    }
    let mut extended = output.proof_bytes.clone();
    extended.push(0);
    rejected.push(verify(
        &proof_context,
        context,
        &governed,
        &output.batch_anchor,
        &extended,
    ));
    let mut bad_anchor = output.batch_anchor.clone();
    bad_anchor.digest[0] ^= 1;
    rejected.push(verify(
        &proof_context,
        context,
        &governed,
        &bad_anchor,
        &output.proof_bytes,
    ));
    let mut bad_context = context;
    bad_context.batch_id[0] ^= 1;
    rejected.push(verify(
        &proof_context,
        bad_context,
        &governed,
        &output.batch_anchor,
        &output.proof_bytes,
    ));
    for mutation in 0..4 {
        let mut rebound_context = context;
        match mutation {
            0 => rebound_context.roster_digest[0] ^= 1,
            1 => rebound_context.epoch += 1,
            2 => rebound_context.transcript_digest[0] ^= 1,
            3 => rebound_context.batch_id[0] ^= 1,
            _ => unreachable!(),
        }
        rebound_context.digest = terminal_context_digest(rebound_context);
        let mut rebound_governed = governed.clone();
        rebound_governed.context_digest = rebound_context.digest;
        rebound_governed.digest = governed_batch_digest(&rebound_governed)
            .expect("rebound governed batch digest must fit release bounds");
        let mut rebound_anchor = output.batch_anchor.clone();
        rebound_anchor.context_digest = rebound_context.digest;
        rebound_anchor.digest = batch_anchor_digest(&rebound_anchor)
            .expect("rebound batch anchor digest must fit release bounds");
        rejected.push(verify(
            &proof_context,
            rebound_context,
            &rebound_governed,
            &rebound_anchor,
            &output.proof_bytes,
        ));
    }
    let mut bad_governed = governed.clone();
    bad_governed.digest[0] ^= 1;
    rejected.push(verify(
        &proof_context,
        context,
        &bad_governed,
        &output.batch_anchor,
        &output.proof_bytes,
    ));
    let mut bad_proof_context = proof_context;
    bad_proof_context.statement_digest[0] ^= 1;
    rejected.push(verify(
        &bad_proof_context,
        context,
        &governed,
        &output.batch_anchor,
        &output.proof_bytes,
    ));
    let mut wrong_verifier_context = context;
    wrong_verifier_context.nifs_verifier_digest[0] ^= 1;
    wrong_verifier_context.digest = terminal_context_digest(wrong_verifier_context);
    rejected.push(verify(
        &proof_context,
        wrong_verifier_context,
        &governed,
        &output.batch_anchor,
        &output.proof_bytes,
    ));
    for malformed in [
        encode_mutated_relation(
            governed.strict_public_inputs.len(),
            &output.proof_bytes,
            |proof| {
                proof.strict_instance_count -= 1;
            },
        ),
        encode_mutated_relation(
            governed.strict_public_inputs.len(),
            &output.proof_bytes,
            |proof| {
                proof.strict_witness_commitments.pop();
            },
        ),
        encode_mutated_relation(
            governed.strict_public_inputs.len(),
            &output.proof_bytes,
            |proof| {
                proof.strict_witness_commitments.swap(0, 1);
            },
        ),
        encode_mutated_relation(
            governed.strict_public_inputs.len(),
            &output.proof_bytes,
            |proof| {
                proof.cross_term_commitments.pop();
            },
        ),
        encode_mutated_relation(
            governed.strict_public_inputs.len(),
            &output.proof_bytes,
            |proof| {
                proof.cross_term_commitments.swap(0, 1);
            },
        ),
    ] {
        rejected.push(verify(
            &proof_context,
            context,
            &governed,
            &output.batch_anchor,
            &malformed,
        ));
    }
    assert_eq!(
        rejected.len(),
        RELEASE_TERMINAL_NEGATIVE_CASE_COUNT_V1 as usize
    );
    assert!(rejected.iter().all(|rejected| *rejected));

    let mut kat = Keccak256::new();
    kat.update(RELEASE_TERMINAL_KAT_DOMAIN_V1);
    kat.update(&context.digest);
    kat.update(&map_manifest.digest());
    kat.update(&terminal_profile.nifs_verifier_digest);
    kat.update(&governed.digest);
    kat.update(&history.digest);
    kat.update(&materialized_digest);
    kat.update(&output.batch_anchor.digest);
    kat.update(
        &u64::try_from(output.proof_bytes.len())
            .expect("proof length fits u64")
            .to_be_bytes(),
    );
    kat.update(&output.proof_bytes);
    kat.update(&receipt.digest());
    kat.update(&RELEASE_TERMINAL_NEGATIVE_CASE_COUNT_V1.to_be_bytes());
    for result in rejected {
        kat.update(&[result.into()]);
    }
    let digest = kat.finalize();
    assert_ne!(digest, [0; 32]);
    eprintln!(
        "ZK-AMS Phase-III release terminal KAT digest={} proof_bytes={} folds={} variables={} constraints={} public_inputs={} negatives={}",
        hex::encode(digest),
        output.proof_bytes.len(),
        MAX_MASKED_RELAXED_STRICT_INSTANCES_V1,
        shape.variable_count(),
        shape.constraint_count(),
        shape.public_input_count(),
        RELEASE_TERMINAL_NEGATIVE_CASE_COUNT_V1,
    );
}

#[test]
fn release_terminal_kat_keeps_strict_assignments_streamed() {
    let source = include_str!("terminal_release_kat.rs");
    assert!(!source.contains(concat!("vec![base_", "assignment;")));
    assert!(!source.contains(concat!("precompute_masked_relaxed_", "v1(")));
    assert!(source.contains("precompute_masked_relaxed_stream_v1("));
    assert!(source.contains("release_admission_assignment(Arc::clone(&shape))"));
    assert!(source.contains("validate_strict_assignment"));
    assert!(source.contains("let shape = Arc::clone(&terminal_profile.shape);"));
    assert!(!source.contains(concat!("zk_ams_phase23_release_", "maps_v1")));
}
