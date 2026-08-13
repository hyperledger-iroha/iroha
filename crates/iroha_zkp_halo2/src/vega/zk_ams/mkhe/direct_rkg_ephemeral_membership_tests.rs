use core::cell::Cell;
use super::*;
use crate::vega::{
    MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1, derive_t256_generators_v1,
    sponge::keccak256,
    zk_ams::mkhe::{
        active::ZkAmsMkheActivePartySecretV1,
        active_exact_binding::mint_test_state_owned_collective_secret_binding_v1,
        direct_collective_eval_ceremony::ZkAmsMkheDirectEvaluatedKeyTargetV1,
        manifest::{ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1, release_profile_v1},
    },
};
const TEST_PROOF_BYTES_V1: usize = 1_447;
const TEST_CHUNK_WIRE_BYTES_V1: usize = 1_494;
struct StreamRandom {
    seed: Vec<u8>,
    counter: u64,
}
impl StreamRandom {
    fn new(seed: &[u8]) -> Self {
        Self {
            seed: seed.to_vec(),
            counter: 0,
        }
    }
}
impl MaskedRelaxedRandomSourceV1 for StreamRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let mut written = 0;
        while written < destination.len() {
            let mut frame = self.seed.clone();
            frame.extend_from_slice(&self.counter.to_be_bytes());
            let block = keccak256(&frame);
            let take = (destination.len() - written).min(block.len());
            destination[written..written + take].copy_from_slice(&block[..take]);
            written += take;
            self.counter = self.counter.wrapping_add(1);
        }
        Ok(())
    }
}
fn governed_fixture(
    label: &[u8],
) -> (
    ZkAmsMkheGovernedActiveRosterV1,
    VerifiedPersistentWitnessBindingSetV1,
    ZkAmsMkheDirectCeremonyContextV1,
) {
    let mut random = StreamRandom::new(label);
    let mut secrets = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map(|_| ZkAmsMkheActivePartySecretV1::generate(&mut random).expect("party secret"))
        .collect::<Vec<_>>();
    secrets.sort_by_key(|secret| secret.party().expect("party id"));
    let references: [&ZkAmsMkheActivePartySecretV1; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = secrets
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .expect("eight parties");
    let roster =
        ZkAmsMkheGovernedActiveRosterV1::new(97, references, &mut random).expect("governed roster");
    let transcript = keccak256(&[label, b".cpk-transcript"].concat());
    let collective_key = keccak256(&[label, b".collective-key"].concat());
    let security = keccak256(&[label, b".security"].concat());
    let shares: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = core::array::from_fn(|index| {
        let mut frame = label.to_vec();
        frame.extend_from_slice(b".share");
        frame.push(index as u8);
        keccak256(&frame)
    });
    let secret_bindings = (0..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1)
        .map(|index| {
            let mut point_label = label.to_vec();
            point_label.extend_from_slice(b".secret-commitments");
            point_label.push(index as u8);
            let commitments =
                derive_t256_generators_v1(&point_label, ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1)
                    .expect("secret commitment points")
                    .try_into()
                    .expect("eight secret commitments");
            mint_test_state_owned_collective_secret_binding_v1(
                &roster,
                security,
                transcript,
                index,
                shares[index],
                commitments,
            )
            .expect("test CPK binding")
        })
        .collect::<Vec<_>>();
    let binding_references: [&VerifiedPersistentWitnessBindingV1;
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = secret_bindings
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .expect("eight bindings");
    let bindings = VerifiedPersistentWitnessBindingSetV1::new(
        &roster,
        transcript,
        collective_key,
        shares,
        binding_references,
    )
    .expect("verified secret-binding set");
    let direct_context = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        7,
    )
    .expect("direct RKG context");
    (roster, bindings, direct_context)
}
fn coefficient_at(index: usize) -> i8 {
    match index % 3 {
        0 => -1,
        1 => 0,
        _ => 1,
    }
}
fn scalar_for_coefficient(coefficient: i8) -> Scalar {
    match coefficient {
        -1 => -Scalar::one(),
        0 => Scalar::zero(),
        1 => Scalar::one(),
        _ => unreachable!("test coefficient is ternary"),
    }
}
fn fake_chunk(
    context_digest: [u8; 32],
    ordinal: usize,
    commitment: Point,
) -> ZkAmsT256MembershipProofV1 {
    let mut proof = vec![ordinal as u8; TEST_PROOF_BYTES_V1];
    proof[..32].copy_from_slice(&context_digest);
    proof[32..34].copy_from_slice(&(ordinal as u16).to_be_bytes());
    let mut wire = Vec::with_capacity(TEST_CHUNK_WIRE_BYTES_V1);
    wire.extend_from_slice(b"ZMBP");
    wire.push(1);
    wire.push(ZkAmsT256MembershipBoundV1::One as u8);
    wire.extend_from_slice(&(ordinal as u16).to_be_bytes());
    wire.extend_from_slice(
        &u32::try_from(ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            .expect("fixed chunk length")
            .to_be_bytes(),
    );
    wire.extend_from_slice(
        &commitment
            .to_non_identity_wire_bytes()
            .expect("nonidentity commitment"),
    );
    wire.extend_from_slice(&(TEST_PROOF_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&proof);
    assert_eq!(wire.len(), TEST_CHUNK_WIRE_BYTES_V1);
    ZkAmsT256MembershipProofV1::from_wire_bytes_exact(&wire).expect("synthetic chunk")
}
fn fake_verify(
    context_digest: [u8; 32],
    ordinal: u16,
    chunk: &ZkAmsT256MembershipProofV1,
) -> Result<[u8; 32], ExactEightChunkMembershipErrorV1> {
    if chunk.bound() != ZkAmsT256MembershipBoundV1::One
        || chunk.proof_bytes().get(..32) != Some(context_digest.as_slice())
        || chunk.proof_bytes().get(32..34) != Some(ordinal.to_be_bytes().as_slice())
    {
        return Err(ExactEightChunkMembershipErrorV1::DigestMismatch);
    }
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.direct-rkg-ephemeral.test-transcript");
    hash.update(&context_digest);
    hash.update(&ordinal.to_be_bytes());
    hash.update(&chunk.to_wire_bytes());
    Ok(hash.finalize())
}
fn evidence_and_opening_fixture(
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
) -> (
    ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1,
    ZeroizingT256ScalarVecV1,
    [ZeroizingT256ScalarCopyV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) {
    let exact_context = context.to_exact().expect("exact context");
    let context_digest = exact_context.context_digest();
    let mut raw_blindings: [Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] =
        core::array::from_fn(|index| Scalar::from_u64(101 + index as u64));
    let chunks = core::array::from_fn(|chunk| {
        let start = chunk * ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1;
        let mut coefficients = (start..start + ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            .map(coefficient_at)
            .collect::<Vec<_>>();
        let commitment = commit_zk_ams_t256_membership_chunk_v1(
            ZkAmsT256MembershipBoundV1::One,
            &coefficients,
            &raw_blindings[chunk],
        )
        .expect("opening commitment");
        coefficients.fill(0);
        fake_chunk(context_digest, chunk, commitment)
    });
    let transcript_digests = core::array::from_fn(|index| {
        fake_verify(context_digest, index as u16, &chunks[index]).expect("test transcript")
    });
    let evidence = ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1::assemble_for_test(
        context,
        chunks,
        transcript_digests,
    )
    .expect("synthetic evidence");
    let mut u =
        ZeroizingT256ScalarVecV1::with_capacity(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1);
    for index in 0..ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 {
        let mut scalar = scalar_for_coefficient(coefficient_at(index));
        u.push(scalar);
        scalar.clear_secret();
    }
    let blindings =
        core::array::from_fn(|index| ZeroizingT256ScalarCopyV1::take(&mut raw_blindings[index]));
    assert!(raw_blindings.iter().all(|blinding| blinding.is_zero()));
    (evidence, u, blindings)
}
fn verified_source_fixture(
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
) -> (
    VerifiedRkgEphemeralMembershipSourceV1,
    ZeroizingT256ScalarVecV1,
    [ZeroizingT256ScalarCopyV1; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
) {
    let (evidence, u, blindings) = evidence_and_opening_fixture(context);
    let source = evidence
        .into_verified_with_for_test(fake_verify)
        .expect("exact-verifier source");
    (source, u, blindings)
}
#[test]
fn wrapper_wire_is_exact_role_separated_and_binds_every_axis() {
    let (roster, bindings, direct_context) = governed_fixture(b"rkg-ephemeral-wire");
    let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        &direct_context,
        2,
        41,
    )
    .expect("wrapper context");
    let (evidence, _u, _blindings) = evidence_and_opening_fixture(context);
    let wire = evidence.to_wire_bytes().expect("canonical wire");
    assert_eq!(wire.len(), 12_291);
    assert_eq!(&wire[..4], b"ZRME");
    assert_eq!(wire[4], 1);
    assert_eq!(wire[5], ZkAmsT256MembershipBoundV1::One as u8);
    assert_eq!(wire[6], 8);
    assert_eq!(
        ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1::from_wire_bytes_exact(context, &wire)
            .expect("decode")
            .to_wire_bytes()
            .expect("re-encode"),
        wire
    );
    for axis in 0..12 {
        let mut changed = context;
        match axis {
            0 => changed.profile_digest[0] ^= 1,
            1 => changed.roster_digest[0] ^= 1,
            2 => changed.key_material_digest[0] ^= 1,
            3 => changed.epoch += 1,
            4 => changed.cpk_transcript_digest[0] ^= 1,
            5 => changed.direct_context_digest[0] ^= 1,
            6 => changed.party_index ^= 1,
            7 => {
                let mut party = changed.party.to_bytes();
                party[0] ^= 1;
                changed.party = ZkAmsMkhePartyIdV1::new(party).expect("changed party");
            }
            8 => changed.evaluated_key_ordinal = 1,
            9 => changed.digit_index ^= 1,
            10 => changed.record_index += 1,
            11 => changed.secret_lineage_identity_digest[0] ^= 1,
            _ => unreachable!(),
        }
        changed.statement_digest = rkg_ephemeral_statement_digest_v1(changed);
        assert_ne!(changed.statement_digest(), context.statement_digest());
        assert!(
            ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1::from_wire_bytes_exact(changed, &wire)
                .is_err(),
            "wrapper axis {axis} accepted a stale wire"
        );
    }
    let trailing = [wire.as_slice(), &[0]].concat();
    for malformed in [&wire[..wire.len() - 1], trailing.as_slice()] {
        assert!(
            ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1::from_wire_bytes_exact(
                context, malformed
            )
            .is_err()
        );
    }
}
#[test]
fn only_exact_verification_mints_source_and_tampering_blocks_binding_mint() {
    let (roster, bindings, direct_context) = governed_fixture(b"rkg-ephemeral-source");
    let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        &direct_context,
        1,
        73,
    )
    .expect("wrapper context");
    let (mut source, _u, _blindings) = verified_source_fixture(context);
    assert_ne!(source.source_verification_digest(), [0; 32]);
    source.source_verification_digest[0] ^= 1;
    assert_eq!(
        mint_rkg_ephemeral_binding_from_verified_membership_v1(
            &roster,
            &bindings,
            &direct_context,
            1,
            73,
            source,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
    let galois = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index: 0 },
        7,
    )
    .expect("Galois context");
    assert!(
        ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            &roster, &bindings, &galois, 1, 73,
        )
        .is_err()
    );
    assert!(
        ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            &roster,
            &bindings,
            &direct_context,
            1,
            0,
        )
        .is_err()
    );
}
#[test]
fn retained_opening_is_move_only_round_scoped_and_rejects_non_rkg_consumers() {
    let (roster, bindings, direct_context) = governed_fixture(b"rkg-ephemeral-retained");
    let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        &direct_context,
        4,
        91,
    )
    .expect("wrapper context");
    let (source, u, blindings) = verified_source_fixture(context);
    let (opening, verifier_binding) = RetainedRkgEphemeralOpeningV1::from_verified_membership(
        &roster,
        &bindings,
        &direct_context,
        4,
        91,
        source,
        u,
        blindings,
    )
    .expect("retained opening");
    for round in [
        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne,
        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundTwo,
    ] {
        bindings
            .validate_rkg_ephemeral_binding_for_direct_context(
                &roster,
                &direct_context,
                4,
                &verifier_binding,
                round,
            )
            .expect("compact verifier binding");
        let observed = opening
            .with_borrowed_opening_for_round(
                &roster,
                &bindings,
                &direct_context,
                round,
                |u, blindings| {
                    assert_eq!(u.len(), 131_072);
                    assert_eq!(blindings.len(), 8);
                    assert!(blindings.iter().all(|blinding| !blinding.is_zero()));
                    keccak256(b"closure-ran")
                },
            )
            .expect("authorized opening use");
        assert_eq!(observed, keccak256(b"closure-ran"));
    }
    let closure_calls = Cell::new(0_u8);
    for round in [
        ZkAmsMkheDirectCeremonyRoundV1::RkgNormalize,
        ZkAmsMkheDirectCeremonyRoundV1::Galois,
    ] {
        assert_eq!(
            opening.with_borrowed_opening_for_round(
                &roster,
                &bindings,
                &direct_context,
                round,
                |_u, _blindings| closure_calls.set(closure_calls.get() + 1),
            ),
            Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
        );
    }
    assert_eq!(closure_calls.get(), 0);
    let debug = format!("{opening:?}");
    assert!(debug.contains("[REDACTED; 131072]"));
    assert!(debug.contains("[REDACTED; 8]"));
}
#[test]
fn compact_verifier_binding_rejects_another_valid_direct_digit_context() {
    let (roster, bindings, direct_context) = governed_fixture(b"rkg-ephemeral-context-replay");
    let wrapper_context =
        ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            &roster,
            &bindings,
            &direct_context,
            3,
            151,
        )
        .expect("C7 wrapper context");
    let (source, u, blindings) = verified_source_fixture(wrapper_context);
    let (_opening, verifier_binding) = RetainedRkgEphemeralOpeningV1::from_verified_membership(
        &roster,
        &bindings,
        &direct_context,
        3,
        151,
        source,
        u,
        blindings,
    )
    .expect("C7 verifier binding");
    assert_eq!(
        verifier_binding.source_context_digest(),
        direct_context.digest()
    );
    assert_eq!(
        verifier_binding.source_statement_digest(),
        wrapper_context.statement_digest()
    );
    let next_digit_context = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        8,
    )
    .expect("valid C8 direct context");
    assert_ne!(next_digit_context.digest(), direct_context.digest());
    assert_eq!(
        bindings.validate_rkg_ephemeral_binding_for_direct_context(
            &roster,
            &next_digit_context,
            3,
            &verifier_binding,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
    let next_record_context =
        ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            &roster,
            &bindings,
            &direct_context,
            3,
            152,
        )
        .expect("record-152 wrapper context");
    let (next_source, next_u, next_blindings) = verified_source_fixture(next_record_context);
    let (_next_opening, next_binding) = RetainedRkgEphemeralOpeningV1::from_verified_membership(
        &roster,
        &bindings,
        &direct_context,
        3,
        152,
        next_source,
        next_u,
        next_blindings,
    )
    .expect("record-152 verifier binding");
    assert_ne!(
        verifier_binding.source_statement_digest(),
        next_binding.source_statement_digest()
    );
    assert_ne!(
        verifier_binding.identity_digest(),
        next_binding.identity_digest()
    );
}
#[test]
fn retained_opening_recomputes_every_commitment_and_zeroizes_error_owners() {
    let (roster, bindings, direct_context) = governed_fixture(b"rkg-ephemeral-hostile-opening");
    let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        &direct_context,
        0,
        117,
    )
    .expect("wrapper context");
    let (source, mut u, blindings) = verified_source_fixture(context);
    u.as_mut_slice()[ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1 * 6] = Scalar::from_u64(2);
    let before_error = crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(matches!(
        RetainedRkgEphemeralOpeningV1::from_verified_membership(
            &roster,
            &bindings,
            &direct_context,
            0,
            117,
            source,
            u,
            blindings,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
    assert!(
        crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1() > before_error
    );
    let (source, mut short_u, blindings) = verified_source_fixture(context);
    short_u.clear_and_truncate(ZK_AMS_MKHE_EXACT_MEMBERSHIP_COEFFICIENTS_V1 - 1);
    assert!(matches!(
        RetainedRkgEphemeralOpeningV1::from_verified_membership(
            &roster,
            &bindings,
            &direct_context,
            0,
            117,
            source,
            short_u,
            blindings,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));
}
#[test]
fn retained_opening_rechecks_hostile_post_construction_mutation_before_closure() {
    let (roster, bindings, direct_context) = governed_fixture(b"rkg-ephemeral-hostile-retained");
    let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        &direct_context,
        6,
        129,
    )
    .expect("wrapper context");
    let (source, u, blindings) = verified_source_fixture(context);
    let (mut opening, _verifier_binding) = RetainedRkgEphemeralOpeningV1::from_verified_membership(
        &roster,
        &bindings,
        &direct_context,
        6,
        129,
        source,
        u,
        blindings,
    )
    .expect("retained opening");
    opening.u.as_mut_slice()[0].clear_secret();
    let closure_calls = Cell::new(0_u8);
    assert_eq!(
        opening.with_borrowed_opening_for_round(
            &roster,
            &bindings,
            &direct_context,
            ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne,
            |_u, _blindings| closure_calls.set(closure_calls.get() + 1),
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
    assert_eq!(closure_calls.get(), 0);
}
#[test]
fn source_and_opening_api_guards_stay_move_only_and_release_stays_closed() {
    let source = include_str!("direct_rkg_ephemeral_membership.rs");
    let verified = source
        .split("pub(super) struct VerifiedRkgEphemeralMembershipSourceV1")
        .nth(1)
        .expect("verified source")
        .split("fn verified_source_digest_v1")
        .next()
        .expect("verified source impl");
    assert!(!verified.contains("derive(Clone"));
    assert!(!verified.contains("impl Clone"));
    assert!(!verified.contains("from_wire_bytes"));
    assert!(!verified.contains("pub(super) fn new"));
    let opening = source
        .split("pub(super) struct RetainedRkgEphemeralOpeningV1")
        .nth(1)
        .expect("opening owner")
        .split("struct ZeroizingRkgEphemeralCoefficientChunkV1")
        .next()
        .expect("opening impl");
    assert!(!opening.contains("derive(Clone"));
    assert!(!opening.contains("impl Clone"));
    assert!(!opening.contains("pub(super) fn u("));
    assert!(!opening.contains("pub(super) fn blindings("));
    assert!(opening.contains("with_borrowed_opening_for_round"));
    assert!(opening.contains("RkgNormalize"));
    assert!(opening.contains("Galois"));
    let round_use = opening
        .split("pub(super) fn with_borrowed_opening_for_round")
        .nth(1)
        .expect("round-scoped opening use")
        .split("impl core::fmt::Debug")
        .next()
        .expect("round-scoped opening implementation");
    let context_validation = round_use
        .find("if expected != self.context")
        .expect("context validation");
    let binding_validation = round_use
        .find("bindings.validate_rkg_ephemeral_binding_for_direct_context")
        .expect("binding validation");
    let opening_validation = round_use
        .find("verify_retained_opening_commitments_v1")
        .expect("retained opening validation");
    assert!(context_validation < binding_validation);
    assert!(binding_validation < opening_validation);
    assert!(round_use.contains(
        "verify_retained_opening_commitments_v1(&self.binding, &self.u, &self.blindings)?;\n        Ok(use_opening("
    ));
    let binding_source = include_str!("active_exact_binding.rs");
    let binding_fields = binding_source
        .split("pub(super) struct VerifiedPersistentWitnessBindingV1")
        .nth(1)
        .expect("persistent binding")
        .split("impl VerifiedPersistentWitnessBindingV1")
        .next()
        .expect("persistent binding fields");
    assert!(binding_fields.contains("source_context_digest: [u8; 32]"));
    assert!(binding_fields.contains("source_statement_digest: [u8; 32]"));
    let binding_impl = binding_source
        .split("impl VerifiedPersistentWitnessBindingV1")
        .nth(1)
        .expect("persistent binding implementation")
        .split("/// Exact ordered eight-party set")
        .next()
        .expect("persistent binding implementation body");
    assert!(binding_impl.contains("source_context_digest: self.source_context_digest"));
    assert!(binding_impl.contains("source_statement_digest: self.source_statement_digest"));
    assert!(binding_impl.contains("self.source_context_digest != [0; 32]"));
    assert!(binding_impl.contains("self.source_statement_digest != [0; 32]"));
    assert!(binding_impl.contains("self.source_context_digest == [0; 32]"));
    assert!(binding_impl.contains("self.source_statement_digest == [0; 32]"));
    let ephemeral_mint = binding_source
        .split("pub(super) fn mint_rkg_ephemeral_binding_from_verified_membership_v1")
        .nth(1)
        .expect("RKG-ephemeral mint")
        .split("/// Test-only stand-in")
        .next()
        .expect("RKG-ephemeral mint implementation");
    let source_validation = ephemeral_mint
        .find("source.validate_against(expected_context)?")
        .expect("source validation");
    let context_retention = ephemeral_mint
        .find("let source_context_digest = expected_context.direct_context_digest()")
        .expect("source context retention");
    let statement_retention = ephemeral_mint
        .find("let source_statement_digest = expected_context.statement_digest()")
        .expect("source statement retention");
    assert!(source_validation < context_retention);
    assert!(context_retention < statement_retention);
    let direct_context_validation = binding_source
        .split("pub(super) fn validate_rkg_ephemeral_binding_for_direct_context")
        .nth(1)
        .expect("direct-context binding validation")
        .split("pub(super) fn bind_direct_relation_use")
        .next()
        .expect("direct-context binding implementation");
    assert!(direct_context_validation.contains("binding.record_index"));
    assert!(
        direct_context_validation.contains("binding.source_context_digest != context.digest()")
    );
    assert!(
        direct_context_validation
            .contains("binding.source_statement_digest != expected_context.statement_digest()")
    );
    let direct_use = binding_source
        .split("pub(super) fn bind_direct_relation_use")
        .nth(1)
        .expect("direct-use binding")
        .split("/// Exact direct-ceremony equation")
        .next()
        .expect("direct-use implementation");
    assert!(direct_use.contains("binding.source_context_digest != selector.context_digest"));
    assert!(direct_use.contains("ephemeral_source_statement_digest"));
    assert!(direct_use.contains("ephemeral_record_index"));
    let identity_hash = binding_source
        .split("fn verified_binding_identity_digest")
        .nth(1)
        .expect("binding identity hash")
        .split("fn verified_binding_verification_digest")
        .next()
        .expect("binding identity implementation");
    assert!(identity_hash.contains("binding.role == PersistentWitnessRoleV1::RkgEphemeral"));
    assert!(identity_hash.contains("binding.source_context_digest"));
    assert!(identity_hash.contains("binding.source_statement_digest"));
    let verification_hash = binding_source
        .split("fn verified_binding_verification_digest")
        .nth(1)
        .expect("binding verification hash")
        .split("fn verified_binding_set_root")
        .next()
        .expect("binding verification implementation");
    assert!(verification_hash.contains("binding.role == PersistentWitnessRoleV1::RkgEphemeral"));
    assert!(verification_hash.contains("binding.source_context_digest"));
    assert!(verification_hash.contains("binding.source_statement_digest"));
    let state =
        super::super::active_exact_binding::exact_binding_release_state_v1(&release_profile_v1())
            .expect("fail-closed audit");
    assert_eq!(state.blocker_mask, 0xfc);
    assert!(!state.release_available);
}
