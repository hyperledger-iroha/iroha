use super::*;
use crate::vega::bulletproof_t256::{
    ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1, ZkAmsT256MembershipBoundV1,
    commit_zk_ams_t256_membership_chunk_v1,
};
use crate::vega::{
    MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1, derive_t256_generators_v1,
    sponge::keccak256,
    zk_ams::mkhe::{
        active::ZkAmsMkheActivePartySecretV1,
        active_exact_binding::{
            VerifiedPersistentWitnessBindingV1, mint_test_state_owned_collective_secret_binding_v1,
        },
        collective::{
            ZkAmsMkheCollectivePartyStateV1, generate_zk_ams_mkhe_collective_party_state_v1,
        },
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

pub(in crate::vega::zk_ams::mkhe) fn creator_state_fixture(
    label: &[u8],
) -> (
    ZkAmsMkheGovernedActiveRosterV1,
    VerifiedPersistentWitnessBindingSetV1,
    ZkAmsMkheCollectivePartyStateV1,
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
    let shares: [[u8; 32]; ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1] = core::array::from_fn(|index| {
        if index == 0 {
            return [0; 32];
        }
        let mut frame = label.to_vec();
        frame.extend_from_slice(b".share");
        frame.push(index as u8);
        keccak256(&frame)
    });
    let (mut state, share) = generate_zk_ams_mkhe_collective_party_state_v1(
        &roster,
        transcript,
        0,
        &secrets[0],
        &mut random,
    )
    .expect("party state");
    let mut shares = shares;
    shares[0] = share.digest();
    let (cached, verifier) = state
        .test_state_owned_cpk_bindings_v1(&roster, &share)
        .expect("state-owned CPK bindings");
    state
        .admit_verified_cpk_binding(&roster, &share, cached)
        .expect("state-owned CPK admission");
    let security = state.security_certificate_digest_internal();
    let secret_bindings = core::iter::once(verifier)
        .chain((1..ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1).map(|index| {
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
        }))
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
    (roster, bindings, state)
}

fn governed_fixture(
    label: &[u8],
) -> (
    ZkAmsMkheGovernedActiveRosterV1,
    VerifiedPersistentWitnessBindingSetV1,
) {
    let (roster, bindings, _) = creator_state_fixture(label);
    (roster, bindings)
}

pub(in crate::vega::zk_ams::mkhe) fn creator_replacement_binding(
    roster: &ZkAmsMkheGovernedActiveRosterV1,
    state: &ZkAmsMkheCollectivePartyStateV1,
) -> VerifiedPersistentWitnessBindingV1 {
    let commitments = derive_t256_generators_v1(b"replacement", 8)
        .expect("replacement points")
        .try_into()
        .expect("eight replacement points");
    mint_test_state_owned_collective_secret_binding_v1(
        roster,
        state.security_certificate_digest_internal(),
        state.transcript_digest(),
        0,
        state.public_share_digest(),
        commitments,
    )
    .expect("replacement binding")
}

fn coefficient_at(index: usize) -> i8 {
    match index % 3 {
        0 => -1,
        1 => 0,
        _ => 1,
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

pub(in crate::vega::zk_ams::mkhe) fn creator_evidence(
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
    coefficients: &[i8],
    blindings: &[Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1],
    mismatch: bool,
) -> Result<
    ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1,
    ZkAmsMkheDirectRkgEphemeralMembershipErrorV1,
> {
    let context_digest = context.to_exact()?.context_digest();
    let mut commitments: [Point; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] =
        core::array::from_fn(|index| {
            let start = index * ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1;
            commit_zk_ams_t256_membership_chunk_v1(
                ZkAmsT256MembershipBoundV1::One,
                &coefficients[start..start + ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1],
                &blindings[index],
            )
            .expect("opening commitment")
        });
    if mismatch {
        commitments[7] = -commitments[7];
    }
    let chunks =
        core::array::from_fn(|index| fake_chunk(context_digest, index, commitments[index]));
    let transcripts = core::array::from_fn(|index| {
        fake_verify(context_digest, index as u16, &chunks[index]).expect("test transcript")
    });
    ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1::assemble_for_test(context, chunks, transcripts)
}

fn evidence_fixture(
    context: ZkAmsMkheDirectRkgEphemeralMembershipContextV1,
) -> ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1 {
    let exact_context = context.to_exact().expect("exact context");
    let context_digest = exact_context.context_digest();
    let blindings: [Scalar; ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1] =
        core::array::from_fn(|index| Scalar::from_u64(101 + index as u64));
    let chunks = core::array::from_fn(|chunk| {
        let start = chunk * ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1;
        let mut coefficients = (start..start + ZK_AMS_MEMBERSHIP_CHUNK_COEFFICIENTS_V1)
            .map(coefficient_at)
            .collect::<Vec<_>>();
        let commitment = commit_zk_ams_t256_membership_chunk_v1(
            ZkAmsT256MembershipBoundV1::One,
            &coefficients,
            &blindings[chunk],
        )
        .expect("opening commitment");
        coefficients.fill(0);
        fake_chunk(context_digest, chunk, commitment)
    });
    let transcript_digests = core::array::from_fn(|index| {
        fake_verify(context_digest, index as u16, &chunks[index]).expect("test transcript")
    });
    ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1::assemble_for_test(
        context,
        chunks,
        transcript_digests,
    )
    .expect("synthetic evidence")
}

#[test]
fn wrapper_wire_is_exact_role_separated_and_binds_canonical_context() {
    let (roster, bindings) = governed_fixture(b"rkg-ephemeral-wire");
    let direct_context = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        7,
    )
    .expect("direct context");
    let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        &direct_context,
        2,
    )
    .expect("wrapper context");
    assert_eq!(context.record_index(), 59);
    let evidence = evidence_fixture(context);
    assert_eq!(evidence.context(), context);
    assert_eq!(evidence.commitments().len(), 8);
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
        assert!(
            ZkAmsMkheDirectRkgEphemeralMembershipEvidenceV1::from_wire_bytes_exact(changed, &wire)
                .is_err(),
            "wrapper axis {axis} accepted a stale wire"
        );
    }
}

#[test]
fn canonical_record_schedule_is_nonzero_unique_and_round_neutral() {
    let (roster, bindings) = governed_fixture(b"rkg-ephemeral-records");
    for (digit, party, expected) in [(0, 0, 1), (0, 7, 8), (37, 0, 297), (37, 7, 304)] {
        let direct_context = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
            &roster,
            &bindings,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
            digit,
        )
        .expect("direct context");
        let context = ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            &roster,
            &bindings,
            &direct_context,
            party,
        )
        .expect("membership context");
        assert_eq!(context.record_index(), expected);
    }
    assert_eq!(release_profile_v1().gadget_digits, 38);
    assert_eq!(canonical_rkg_ephemeral_record_index_v1(1, 0, 0), None);
    assert_eq!(canonical_rkg_ephemeral_record_index_v1(0, 38, 0), None);
    assert_eq!(canonical_rkg_ephemeral_record_index_v1(0, 0, 8), None);
}

#[test]
fn non_relinearization_context_is_rejected_without_a_record_override() {
    let (roster, bindings) = governed_fixture(b"rkg-ephemeral-target");
    let galois = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Galois { schedule_index: 0 },
        7,
    )
    .expect("Galois context");
    assert!(
        ZkAmsMkheDirectRkgEphemeralMembershipContextV1::from_verified_binding_set(
            &roster, &bindings, &galois, 1,
        )
        .is_err()
    );
}

#[test]
fn membership_only_authority_corridor_is_removed_and_release_stays_closed() {
    let source = include_str!("direct_rkg_ephemeral_membership.rs");
    for forbidden in [
        "pub(super) fn into_verified(",
        "into_verified_with_for_test",
        "VerifiedRkgEphemeralMembershipSourceV1",
        "RetainedRkgEphemeralOpeningV1",
        "with_borrowed_opening_for_round",
        "mint_rkg_ephemeral_binding_from_verified_membership_v1",
        "ZeroizingT256ScalarVecV1",
    ] {
        assert!(
            !source.contains(forbidden),
            "authority corridor remains: {forbidden}"
        );
    }
    let context_constructor = source
        .split("pub(super) fn from_verified_binding_set(")
        .nth(1)
        .expect("membership context constructor")
        .split("fn validate(self)")
        .next()
        .expect("constructor boundary");
    assert!(!context_constructor.contains("record_index: u32"));
    assert!(context_constructor.contains("canonical_rkg_ephemeral_record_index_v1("));

    let binding_source = include_str!("active_exact_binding.rs");
    assert!(!binding_source.contains("fn mint_rkg_ephemeral_binding_from_verified_membership_v1"));
    let validation = binding_source
        .split("pub(super) fn validate_rkg_ephemeral_binding_for_direct_context")
        .nth(1)
        .expect("future direct-context validator")
        .split("pub(super) fn bind_direct_relation_use")
        .next()
        .expect("validator boundary");
    assert!(validation.contains("binding.record_index != expected_context.record_index()"));
    assert!(!validation.contains("party_index,\n                binding.record_index"));
    assert!(binding_source.contains("fn verified_ephemeral_binding_fixture("));

    let creator_source = include_str!("collective/party_local_rkg_ephemeral_v1.rs");
    assert!(creator_source.contains("R: MaskedRelaxedRandomSourceV1 + ProofRandomSource"));
    assert!(
        creator_source.contains("StateOwnedDirectRkgEphemeralMembershipPrecursorV1 { membership }")
    );
    for forbidden in [
        "VerifiedRkgEphemeralMembershipSourceV1",
        "RetainedRkgEphemeralOpeningV1",
        "mint_rkg_ephemeral_binding_from_verified_membership_v1",
        "PersistentDirectRelationUseSelectorV1",
        "VerificationReceiptV1",
        "AdmissionV1",
    ] {
        assert!(!creator_source.contains(forbidden));
    }
    let state =
        super::super::active_exact_binding::exact_binding_release_state_v1(&release_profile_v1())
            .expect("fail-closed audit");
    assert_eq!(state.blocker_mask, 0xfd);
    assert!(!state.external_commitment_provenance_certified);
    assert!(!state.full_basis_mrep_crs_certified);
    assert!(!state.membership_argument_of_knowledge_certified);
    assert!(!state.membership_zero_knowledge_certified);
    assert!(!state.composite_rom_forking_certified);
    assert!(!state.full_ceremony_10_336_instance_composition_certified);
    assert!(!state.release_available);
}
