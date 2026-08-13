// Test body included from the streaming child module so production source stays bounded.
use super::super::super::super::{MaskedRelaxedRandomErrorV1, MaskedRelaxedRandomSourceV1};
use super::super::super::{AuthenticationSecret, PlaintextModulus};
use super::super::{
    DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1, decryption_split_manifest_digest,
    decryption_transient_zeroized_drop_count_v1, reset_decryption_transient_zeroized_drop_count_v1,
    sparse_negacyclic_mul_small, sparse_negacyclic_mul_wide,
};
use super::*;
use crate::vega::sponge::{keccak256, shake256};

const TEST_MODULI: [u64; 2] = [2_013_265_921, 1_811_939_329];
const TEST_ROOTS: [u64; 2] = [1_400_279_418, 677_356_115];
const TEST_PROVIDER_ID: [u8; 32] = [0x51; 32];
const TEST_SNAPSHOT_ID: [u8; 32] = [0x61; 32];
const TEST_DRIFTED_SNAPSHOT_ID: [u8; 32] = [0x62; 32];

fn test_profile() -> BgvProfile {
    BgvProfile {
        profile_id: [0xd8; 32],
        ring_degree: 8,
        moduli: &TEST_MODULI,
        negacyclic_roots: &TEST_ROOTS,
        plaintext_modulus: PlaintextModulus::Tiny(17),
        error_eta: 2,
        hybrid_rns_decomposition: false,
        gadget_base_log: 8,
        gadget_digits: 8,
        max_ciphertext_bytes: 1 << 20,
        max_evaluated_key_bytes: 16 << 20,
        max_round_bytes: 16 << 20,
        max_share_bytes: 4 << 20,
        max_workspace_bytes: 16 << 20,
        max_work_units: 1 << 20,
    }
}

struct KatRandom {
    state: [u8; 32],
    counter: u64,
}

impl KatRandom {
    fn new(label: &[u8]) -> Self {
        Self {
            state: keccak256(label),
            counter: 0,
        }
    }
}

impl MaskedRelaxedRandomSourceV1 for KatRandom {
    fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
        let mut cursor = 0;
        while cursor != destination.len() {
            let mut frame = Vec::with_capacity(40);
            frame.extend_from_slice(&self.state);
            frame.extend_from_slice(&self.counter.to_be_bytes());
            let block = shake256(&frame, 64);
            let take = (destination.len() - cursor).min(block.len());
            destination[cursor..cursor + take].copy_from_slice(&block[..take]);
            cursor += take;
            self.counter = self.counter.wrapping_add(1);
        }
        Ok(())
    }
}

#[derive(Clone)]
struct TestProvider {
    pointer: ZkAmsMkheDirectObjectPointerV1,
    bytes: Vec<u8>,
    snapshot_identity: [u8; 32],
    snapshot_calls: usize,
    read_calls: usize,
    short_read_at: Option<usize>,
    drift_snapshot_at: Option<usize>,
    mutate_payload_at: Option<usize>,
}

impl TestProvider {
    fn new(kind: ZkAmsMkheDirectObjectKindV1, bytes: Vec<u8>) -> Self {
        Self {
            pointer: ZkAmsMkheDirectObjectPointerV1::from_payload(kind, &bytes)
                .expect("bounded test payload has a canonical pointer"),
            bytes,
            snapshot_identity: TEST_SNAPSHOT_ID,
            snapshot_calls: 0,
            read_calls: 0,
            short_read_at: None,
            drift_snapshot_at: None,
            mutate_payload_at: None,
        }
    }
}

impl ZkAmsMkheDirectObjectReadAtProviderV1 for TestProvider {
    fn provider_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        Ok(TEST_PROVIDER_ID)
    }

    fn snapshot_identity(&mut self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        self.snapshot_calls += 1;
        if self
            .drift_snapshot_at
            .is_some_and(|call| self.snapshot_calls >= call)
        {
            Ok(TEST_DRIFTED_SNAPSHOT_ID)
        } else {
            Ok(self.snapshot_identity)
        }
    }

    fn object_len(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
    ) -> Result<u64, ZkAmsMkheErrorV1> {
        if pointer != self.pointer {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        u64::try_from(self.bytes.len()).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    }

    fn read_at(
        &mut self,
        pointer: ZkAmsMkheDirectObjectPointerV1,
        absolute_offset: u64,
        destination: &mut [u8],
    ) -> Result<usize, ZkAmsMkheErrorV1> {
        if pointer != self.pointer {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        self.read_calls += 1;
        let start =
            usize::try_from(absolute_offset).map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        if self.mutate_payload_at == Some(self.read_calls) {
            let byte = self
                .bytes
                .get_mut(start)
                .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
            *byte ^= 0x80;
        }
        let copied = if self.short_read_at == Some(self.read_calls) {
            destination.len().saturating_sub(1)
        } else {
            destination.len()
        };
        let end = start
            .checked_add(copied)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        let source = self
            .bytes
            .get(start..end)
            .ok_or(ZkAmsMkheErrorV1::InvalidWireEncoding)?;
        destination[..copied].copy_from_slice(source);
        Ok(copied)
    }
}

fn test_payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| (index as u8).wrapping_mul(29).wrapping_add(7))
        .collect()
}

fn release_test_roster_v1(epoch: u64) -> ZkAmsMkheGovernedRosterWireV1 {
    let parties = core::array::from_fn(|index| {
        let mut bytes = [0_u8; 32];
        bytes[31] = u8::try_from(index + 1).expect("release party index");
        ZkAmsMkhePartyIdV1::new(bytes).expect("nonzero release party")
    });
    ZkAmsMkheGovernedRosterWireV1::new(
        release_profile_v1()
            .digest()
            .expect("release profile digest"),
        epoch,
        parties,
    )
    .expect("ordered release roster")
}

fn valid_release_proof_bytes() -> Vec<u8> {
    let profile = release_profile_v1();
    let evidence = derive_decryption_resource_evidence(&profile).expect("release evidence");
    let mut bytes =
        vec![0_u8; usize::try_from(evidence.proof_payload_bytes).expect("proof length")];
    bytes[..4].copy_from_slice(&DECRYPTION_PROOF_TAG_V1);
    bytes[4] = MKHE_VERSION_V1;
    bytes[5..7].copy_from_slice(&evidence.wide_response_coefficient_bytes.to_be_bytes());
    bytes[7..11].copy_from_slice(
        &u32::try_from(profile.ring_degree)
            .expect("release degree is canonical")
            .to_be_bytes(),
    );
    bytes[11..43].fill(0x5a);
    let count = u32::try_from(profile.ring_degree)
        .expect("release degree is canonical")
        .to_be_bytes();
    bytes[43..47].copy_from_slice(&count);
    bytes[47..51].copy_from_slice(&count);
    bytes[51..55].copy_from_slice(&count);
    bytes
}

fn signed_manifest(
    label: &[u8],
    party_index: u8,
    context_byte: u8,
) -> (ZkAmsMkheDecryptionTransportManifestV1, DecryptionBindingV1) {
    let profile = release_profile_v1();
    let evidence = derive_decryption_resource_evidence(&profile).expect("release evidence");
    let mut random = KatRandom::new(label);
    let secret = AuthenticationSecret::generate(&mut random).expect("authentication secret");
    let party = secret.party_id().expect("authentication party");
    let binding = DecryptionBindingV1 {
        profile_digest: [0x11; 32],
        roster_digest: [0x22; 32],
        epoch: 7,
        transcript_digest: [0x33; 32],
        ciphertext_digest: [context_byte; 32],
        key_context_digest: [0x55; 32],
        statement_binding_digest: [0x66; 32],
        ciphertext_record_index: 9,
        sample_index: 11,
        party_index,
        party,
        level: 1,
    };
    let polynomial = ZkAmsMkheDecryptionTransportPointerV1 {
        kind: ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial,
        payload_bytes: evidence.split_polynomial_object_bytes,
        payload_blake3: [context_byte.wrapping_add(1); 32],
    };
    let proof = ZkAmsMkheDecryptionTransportPointerV1 {
        kind: ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope,
        payload_bytes: evidence.split_proof_envelope_bytes,
        payload_blake3: [context_byte.wrapping_add(2); 32],
    };
    let digest = decryption_split_manifest_digest(&binding, polynomial, proof)
        .expect("canonical manifest digest");
    let authentication = ArtifactAuthentication::sign(
        DECRYPTION_SPLIT_MANIFEST_AUTH_DOMAIN_V1,
        digest,
        &secret,
        &mut random,
    )
    .expect("manifest signature");
    let manifest = ZkAmsMkheDecryptionTransportManifestV1::new(
        binding.clone(),
        polynomial,
        proof,
        authentication,
    )
    .expect("signed manifest");
    (manifest, binding)
}

#[test]
fn residency_evidence_is_phase_exact_and_cannot_claim_release() {
    let evidence = zk_ams_mkhe_decryption_streaming_residency_evidence_v1()
        .expect("source-derived streaming evidence");
    assert_eq!(evidence.native_rns_polynomial_bytes, 39_845_888);
    assert_eq!(evidence.rns_limb_bytes, 1_048_576);
    assert_eq!(size_of::<ZkAmsMkheStreamingCollectiveCiphertextV1>(), 656);
    assert_eq!(size_of::<ZkAmsMkheDirectObjectPointerV1>(), 80);
    assert_eq!(size_of::<ZkAmsMkheDirectObjectReadReceiptV1>(), 248);
    assert_eq!(size_of::<ZkAmsMkheDirectObjectPublicationReceiptV1>(), 704);
    assert_eq!(evidence.ciphertext_input_bytes, 104_016);
    assert_eq!(evidence.aggregate_bytes, 39_845_888);
    assert_eq!(evidence.proof_view_backing_bytes, 33_030_199);
    assert_eq!(evidence.manifest_preflight_bytes, 3_984);
    assert_eq!(evidence.direct_read_buffer_bytes, 8_192);
    assert_eq!(evidence.sparse_challenge_bytes, 131_072);
    assert_eq!(evidence.manifest_preflight_peak_bytes, 108_000);
    assert_eq!(evidence.proof_load_peak_bytes, 74_040_855);
    assert_eq!(evidence.public_input_hash_peak_bytes, 74_171_927);
    assert_eq!(evidence.public_key_commitment_peak_bytes, 77_317_655);
    assert_eq!(evidence.share_commitment_peak_bytes, 77_317_655);
    assert_eq!(evidence.crt_decode_peak_bytes, 44_148_192);
    assert_eq!(evidence.enumerated_verifier_peak_bytes, 77_317_655);
    assert_eq!(evidence.governed_workspace_ceiling_bytes, 167_772_160);
    assert_eq!(evidence.maximum_full_rns_polynomials, 1);
    assert_eq!(evidence.maximum_rns_limb_buffers, 4);
    assert_eq!(evidence.party_b_passes, 2);
    assert_eq!(evidence.decryption_share_passes, 2);
    assert_eq!(evidence.ciphertext_constant_passes, 10);
    assert_eq!(evidence.ciphertext_linear_passes, 17);
    assert_eq!(evidence.native_reference_lower_bound_bytes, 358_612_992);
    assert_eq!(
        evidence.compact_authority_construction_lower_bound_bytes,
        122_683_434
    );
    assert_eq!(evidence.compact_authority_aggregate_bytes, 39_845_888);
    assert_eq!(
        evidence.compact_authority_absorbed_share_rns_bytes,
        79_691_776
    );
    assert_eq!(evidence.compact_authority_share_proof_bytes, 2_097_194);
    assert_eq!(evidence.compact_authority_limb_scratch_bytes, 1_048_576);
    assert_eq!(
        evidence.compact_authority_enumerated_peak_bytes,
        122_683_434
    );
    assert!(!evidence.compact_authority_cas_backend_residency_enumerated);
    assert!(evidence.compact_authority_enumerated_ceiling_met);
    assert_eq!(evidence.compact_authority_party_b_source_passes, 1);
    assert_eq!(evidence.compact_authority_publication_readback_passes, 2);
    assert_eq!(evidence.compact_authority_context_digest_read_passes, 2);
    assert_eq!(size_of::<SignedWideV1>(), 264);
    assert_eq!(size_of::<SparseChallengeTermV1>(), 16);
    assert_eq!(evidence.staged_prover_party_state_witness_bytes, 2_097_152);
    assert_eq!(evidence.staged_prover_smudge_witness_bytes, 34_603_008);
    assert_eq!(evidence.staged_prover_small_mask_bytes, 2_097_152);
    assert_eq!(evidence.staged_prover_wide_mask_bytes, 34_603_008);
    assert_eq!(evidence.staged_prover_sparse_challenge_bytes, 131_072);
    assert_eq!(evidence.staged_prover_sparse_challenge_terms_bytes, 320);
    assert_eq!(evidence.staged_prover_limb_scratch_bytes, 4_194_304);
    assert_eq!(evidence.staged_prover_direct_io_buffer_bytes, 8_192);
    assert_eq!(evidence.staged_prover_manifest_bytes, 498);
    assert_eq!(evidence.staged_prover_common_a_context_bytes, 156);
    assert_eq!(
        evidence.staged_prover_common_a_limb_frame_scratch_bytes,
        158
    );
    assert_eq!(
        evidence.staged_prover_share_construction_peak_bytes,
        41_006_828
    );
    assert_eq!(
        evidence.staged_prover_public_input_hash_peak_bytes,
        37_861_258
    );
    assert_eq!(evidence.staged_prover_commitment_peak_bytes, 77_707_146);
    assert_eq!(evidence.staged_prover_proof_write_peak_bytes, 73_644_076);
    assert_eq!(
        evidence.staged_prover_self_verification_peak_bytes,
        39_565_569
    );
    assert_eq!(evidence.staged_prover_enumerated_peak_bytes, 77_707_146);
    assert_eq!(
        evidence
            .governed_workspace_ceiling_bytes
            .checked_sub(evidence.staged_prover_enumerated_peak_bytes),
        Some(90_065_014)
    );
    assert!(!evidence.staged_prover_cas_backend_residency_enumerated);
    assert!(evidence.staged_prover_enumerated_ceiling_met);
    assert_eq!(evidence.staged_prover_component_source_passes, 1);
    assert_eq!(evidence.staged_prover_maximum_rejection_attempts, 120);
    assert_eq!(evidence.staged_prover_inner_rejection_attempts, 128);
    assert_eq!(evidence.staged_prover_maximum_ring_multiplications, 243);
    assert_eq!(
        evidence.staged_prover_ring_multiplication_work_units,
        21_785_739_264
    );
    assert_eq!(evidence.staged_prover_rng_byte_budget, 5_000_000_000);
    assert_eq!(
        evidence.staged_prover_first_candidate_rng_bytes,
        3_994_026_176
    );
    assert_eq!(
        evidence.staged_prover_common_a_candidate_budget,
        1_500_000_000
    );
    assert_eq!(
        evidence.staged_prover_first_candidate_common_a_candidates,
        607_649_792
    );
    assert_eq!(
        evidence.staged_prover_common_a_xof_byte_budget,
        12_000_000_000
    );
    assert_eq!(
        evidence.staged_prover_first_candidate_common_a_xof_bytes,
        4_861_198_336
    );
    assert_eq!(
        evidence.staged_prover_common_a_residue_output_work_units,
        607_649_792
    );
    assert_eq!(evidence.staged_prover_common_a_prepare_validation_passes, 1);
    assert_eq!(evidence.staged_prover_common_a_limb_derivations, 4_636);
    assert_eq!(evidence.staged_prover_common_a_frame_work_units, 732_488);
    assert_eq!(
        evidence.staged_prover_immutable_object_scan_work_units,
        5_279_074_909
    );
    assert_eq!(
        evidence.staged_prover_ciphertext_preflight_scan_work_units,
        79_692_080
    );
    assert_eq!(
        evidence.staged_prover_ciphertext_preflight_hash_work_units,
        79_691_947
    );
    assert_eq!(
        evidence.staged_prover_ciphertext_preflight_work_units,
        159_384_027
    );
    assert_eq!(
        evidence.staged_prover_transcript_hash_work_units,
        9_841_935_664
    );
    assert_eq!(evidence.staged_prover_transcript_fork_work_units, 41_624);
    assert_eq!(evidence.staged_prover_response_work_units, 13_278_880_301);
    assert_eq!(
        evidence.staged_prover_semantic_replay_work_units,
        1_539_047_424
    );
    assert_eq!(evidence.staged_prover_total_work_units, 69_492_485_649);
    assert_eq!(
        release_profile_v1()
            .max_work_units
            .checked_sub(evidence.staged_prover_total_work_units),
        Some(30_507_514_351)
    );
    assert!(evidence.staged_prover_work_ceiling_met);
    assert_eq!(evidence.staged_prover_party_b_read_passes, 2);
    assert_eq!(evidence.staged_prover_share_immutable_read_passes, 4);
    assert_eq!(evidence.staged_prover_proof_immutable_read_passes, 3);
    assert_eq!(evidence.staged_prover_ciphertext_constant_read_passes, 1);
    assert_eq!(evidence.staged_prover_ciphertext_linear_read_passes, 123);
    assert!(evidence.enumerated_verifier_ceiling_met);
    assert!(evidence.staged_prover_output_implemented);
    assert_eq!(evidence.staged_prover_release_kat_digest, [0; 32]);
    assert!(evidence.bounded_compact_authority_construction_implemented);
    assert_eq!(evidence.implementation_blocker_count, 0);
    assert_eq!(
        evidence.implementation_blockers,
        [
            ZkAmsMkheDecryptionStreamingBlockerV1::NoBlocker,
            ZkAmsMkheDecryptionStreamingBlockerV1::NoBlocker,
        ]
    );
    assert_eq!(
        evidence.authenticated_peak_residency_digest,
        ZK_AMS_MKHE_DECRYPTION_STREAMING_RESIDENCY_CERTIFICATE_DIGEST_V1
    );
    assert_eq!(evidence.authenticated_peak_residency_digest, [0; 32]);
    assert!(!evidence.release_certified);
    assert_ne!(evidence.evidence_digest, [0; 32]);

    let mut forged = evidence;
    forged.release_certified = true;
    assert_eq!(forged.validate(), Err(ZkAmsMkheErrorV1::InvalidProfile));
}

#[test]
fn compact_ciphertext_axes_keep_record_and_sample_indices_distinct_and_bounded() {
    let roster = release_test_roster_v1(41);
    let maximum_samples = super::super::super::manifest::zk_ams_mkhe_release_manifest_v1()
        .expect("release manifest")
        .max_samples_per_secret_epoch;
    let valid = DecryptionCiphertextAxesV1 {
        profile_digest: roster.profile_digest(),
        roster_digest: roster.roster_digest(),
        epoch: roster.epoch(),
        transcript_digest: [0x31; 32],
        ciphertext_digest: [0x41; 32],
        ciphertext_record_index: 7,
        sample_index: 11,
        level: 0,
    };
    valid
        .validate_for_roster_v1(&roster)
        .expect("distinct record and sample axes remain valid");
    let key_context_digest = [0x51; 32];
    let mut legacy_statement_hash = Keccak256::new();
    legacy_statement_hash.update(b"iroha.zk-ams.v1.mkhe.decryption-statement-binding");
    legacy_statement_hash.update(&roster.profile_digest());
    legacy_statement_hash.update(&roster.roster_digest());
    legacy_statement_hash.update(&roster.epoch().to_be_bytes());
    legacy_statement_hash.update(&valid.transcript_digest());
    legacy_statement_hash.update(&valid.ciphertext_record_index().to_be_bytes());
    legacy_statement_hash.update(&valid.sample_index().to_be_bytes());
    legacy_statement_hash.update(&[valid.level()]);
    legacy_statement_hash.update(&key_context_digest);
    assert_eq!(
        decryption_statement_binding_digest_from_axes_v1(&roster, valid, key_context_digest,),
        legacy_statement_hash.finalize(),
        "compact axes must preserve legacy statement bytes without conflating record and sample"
    );

    let mut record_at_limit = valid;
    record_at_limit.ciphertext_record_index =
        u32::try_from(maximum_samples).expect("release record bound fits u32");
    assert_eq!(
        record_at_limit.validate_for_roster_v1(&roster),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );

    let mut sample_at_limit = valid;
    sample_at_limit.sample_index = maximum_samples;
    assert_eq!(
        sample_at_limit.validate_for_roster_v1(&roster),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
}

#[test]
fn live_ciphertext_key_lineage_rejects_each_independent_hostile_axis() {
    let expected = DecryptionCiphertextKeyLineageV1 {
        key_material_digest: [0x21; 32],
        key_transcript_digest: [0x31; 32],
        collective_key_digest: [0x41; 32],
    };
    expected
        .validate_expected_v1(expected)
        .expect("exact admitted lineage");

    let mut wrong_material = expected;
    wrong_material.key_material_digest[0] ^= 1;
    assert_eq!(
        wrong_material.validate_expected_v1(expected),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );

    let mut wrong_transcript = expected;
    wrong_transcript.key_transcript_digest[0] ^= 1;
    assert_eq!(
        wrong_transcript.validate_expected_v1(expected),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );

    let mut wrong_key = expected;
    wrong_key.collective_key_digest[0] ^= 1;
    assert_eq!(
        wrong_key.validate_expected_v1(expected),
        Err(ZkAmsMkheErrorV1::InvalidCiphertext)
    );
}

#[test]
fn persistent_authority_rejects_provider_and_snapshot_mismatch_independently() {
    use super::super::super::persistent_decryption_equality::validate_exact_streaming_provider_snapshot_axes_v1;

    let provider = [0x51; 32];
    let snapshot = [0x61; 32];
    validate_exact_streaming_provider_snapshot_axes_v1(provider, snapshot, provider, snapshot)
        .expect("exact provider snapshot");
    assert_eq!(
        validate_exact_streaming_provider_snapshot_axes_v1(
            [0x52; 32], snapshot, provider, snapshot,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
    assert_eq!(
        validate_exact_streaming_provider_snapshot_axes_v1(
            provider, [0x62; 32], provider, snapshot,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
}

#[test]
fn compact_authority_source_surface_is_move_only_ordered_and_fail_closed() {
    let persistent = include_str!("persistent_decryption_equality.rs");
    let cpk = include_str!("cpk_relation.rs");
    let collective = include_str!("collective.rs");
    let incremental = include_str!("collective/incremental_source.rs");
    let streaming = include_str!("decryption_streaming.rs");

    // The ceremony surface stays internal and monotonic: one exact contribution
    // and one owned share are consumed at each step. The first stage retains no
    // aggregate or party polynomial; the one aggregate is allocated only after
    // all eight published shares have been sealed and their owners dropped.
    assert!(
        persistent.contains("pub(super) struct ZkAmsMkheStreamingDecryptionAuthorityBuilderV1")
    );
    assert!(persistent.contains("contribution: VerifiedZkAmsMkheCpkContributionV1"));
    assert!(persistent.contains("share: ZkAmsMkheCollectivePublicKeyShareV1"));
    assert!(persistent.contains("party_state: &mut ZkAmsMkheCollectivePartyStateV1"));
    assert!(!persistent.contains("share: &ZkAmsMkheCollectivePublicKeyShareV1"));
    assert!(persistent.contains("let party_index = self.next_party_index;"));
    assert!(persistent.contains("self.failed = true;"));
    assert!(persistent.contains("if result.is_ok()"));
    assert!(persistent.contains("self.party_b_pointers.contains(&expected_pointer)"));
    assert!(persistent.contains("pub(super) fn finish_staging_v1("));
    assert!(persistent.contains("std::sync::Arc::try_unwrap(common_public_a)"));
    assert!(persistent.contains("let mut aggregate_b = ZeroizingRns::zero_exact_v1(&profile)?;"));
    assert!(persistent.contains("aggregate_staged_party_b_v1("));
    assert!(persistent.contains("binding.fork_for_state_and_verifier_v1()"));
    assert!(persistent.contains("party_state.admit_staged_verified_cpk_binding_v1("));
    assert!(!persistent.contains("streaming_decryption_authority,\n            self.bindings"));
    let absorb = persistent
        .split("fn absorb_verified_party_inner_v1")
        .nth(1)
        .expect("staged party absorption")
        .split("pub(super) fn finish_staging_v1")
        .next()
        .expect("staged party absorption boundary");
    let verifier_push = absorb
        .find("self.bindings.push(verifier_binding)")
        .expect("verifier successor retention");
    let state_admission = absorb
        .find("party_state.admit_staged_verified_cpk_binding_v1(")
        .expect("state successor admission");
    let index_advance = absorb
        .find("self.next_party_index += 1")
        .expect("ordered cursor advance");
    assert!(verifier_push < state_admission && state_admission < index_advance);

    // The consumed CPK capability supplies both its exact pointer and original
    // complete-read receipt. No caller-provided digest/pointer overload exists.
    assert!(cpk.contains("pub(super) struct VerifiedZkAmsMkheCompactDecryptionSourceV1"));
    assert!(cpk.contains("party_b_read_receipt: ZkAmsMkheDirectObjectReadReceiptV1"));
    assert!(!cpk.contains("impl Clone for VerifiedZkAmsMkheCompactDecryptionSourceV1"));
    assert!(!cpk.contains("from_raw_compact_decryption"));

    // Native active-proof admission is sealed before compact validation and
    // binds the raw proof/authentication evidence, closing proof substitution.
    assert!(collective.contains("validate_collective_public_key_share_unsealed_v1("));
    assert!(collective.contains("share.proof.write_evidence_chunks"));
    assert!(
        collective.contains("validate_collective_public_key_share_active_admission_v1(share)?")
    );
    assert!(collective.contains("pub(super) fn admit_staged_verified_cpk_binding_v1("));
    assert!(collective.contains("admission.validate_for_v1("));
    let staged_state = collective
        .split("pub(super) fn admit_staged_verified_cpk_binding_v1")
        .nth(1)
        .expect("staged state admission")
        .split("pub(super) fn persistent_secret_binding_for")
        .next()
        .expect("staged state admission boundary");
    let binding_validation = staged_state
        .find("binding.validate_for(")
        .expect("binding validation");
    let commitment_validation = staged_state
        .find("ensure_state_owned_cpk_commitments_v1(")
        .expect("state-owned commitment validation");
    let state_assignment = staged_state
        .find("self.persistent_secret_binding = Some(binding)")
        .expect("sole state mutation");
    assert!(binding_validation < state_assignment);
    assert!(commitment_validation < state_assignment);

    // The only production constructor consumes the move-only permit and the
    // direct-object manifest; no native compatibility bridge remains.
    assert!(streaming.contains("pub fn from_verified_cpk_authority_v1<P>("));
    assert!(streaming.contains("authority: ZkAmsMkheStreamingDecryptionAuthorityV1"));
    assert!(streaming.contains("ciphertext: &'a ZkAmsMkheStreamingCollectiveCiphertextV1"));
    assert!(streaming.contains("ciphertext_record_index: u32"));
    assert!(!streaming.contains("pub fn from_native_reference_v1("));
    assert!(!streaming.contains("pub fn from_raw"));
    assert!(!persistent.contains("impl Clone for ZkAmsMkheStreamingDecryptionAuthorityV1"));

    // Authority consumption rejects roster/provider/snapshot/key-lineage
    // splices. The sealed direct reader separately authenticates each C0/C1
    // pointer and requires the fresh complete receipt to equal its exact
    // post-publication readback receipt; publication session identities need
    // not equal the earlier key-publication identity.
    for required in [
        "validate_exact_streaming_provider_snapshot_axes_v1(",
        "observed_provider_identity != expected_provider_identity",
        "observed_snapshot_identity != expected_snapshot_identity",
        "self.roster.to_wire_roster()? != *roster",
        "authority.context_authority_digest != streaming.authority_digest",
        "streaming_collective_key_digest_v1",
    ] {
        assert!(
            persistent.contains(required),
            "missing fail-closed branch: {required}"
        );
    }
    let consume = persistent
        .split("pub(super) fn consume_streaming_authority_v1(")
        .nth(1)
        .expect("streaming authority consumption")
        .split("/// Mint the exact move-only party-use set")
        .next()
        .expect("streaming authority consumption boundary");
    assert!(consume.contains("validate_exact_streaming_provider_snapshot_axes_v1("));
    assert!(consume.contains("self.roster.to_wire_roster()? != *roster"));
    assert!(consume.contains("ciphertext.validate_for_roster_v1(roster)?"));

    let direct_reader = incremental
        .split("fn read_component_limb_into_v1<P>(")
        .nth(1)
        .expect("direct ciphertext limb reader")
        .split("/// Reread and authenticate one exact constant-component limb")
        .next()
        .expect("direct ciphertext limb reader boundary");
    assert!(direct_reader.contains("StreamingCollectiveLimbReaderV1::begin("));
    assert!(direct_reader.contains("if receipt != *publication.post_publish_read_receipt()"));
}

#[test]
fn release_proof_view_is_zero_copy_and_rejects_malformed_encodings() {
    let mut bytes = valid_release_proof_bytes();
    let view = ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes)
        .expect("canonical bounded proof view");
    assert_eq!(view.bytes.as_ptr(), bytes.as_ptr());
    assert_eq!(view.bytes.len(), bytes.len());
    assert_eq!(view.challenge_seed(), [0x5a; 32]);
    assert!(core::mem::needs_drop::<SignedWideV1>());

    assert!(
        ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes[..bytes.len() - 1]).is_err()
    );

    bytes[0] ^= 0x80;
    assert!(ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes).is_err());
    bytes[0] ^= 0x80;

    bytes[11..43].fill(0);
    assert!(ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes).is_err());
    bytes[11..43].fill(0x5a);

    bytes[43..47].copy_from_slice(&0_u32.to_be_bytes());
    assert!(ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes).is_err());
    let degree = u32::try_from(release_profile_v1().ring_degree)
        .expect("release degree")
        .to_be_bytes();
    bytes[43..47].copy_from_slice(&degree);

    let canonical = ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes)
        .expect("restored canonical proof");
    let secret_offset = canonical.secret_offset;
    let smudge_offset = canonical.smudge_offset;
    let wide_response_bytes = canonical.wide_response_bytes;

    bytes[secret_offset..secret_offset + 8].copy_from_slice(&i64::MAX.to_be_bytes());
    assert!(ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes).is_err());
    bytes[secret_offset..secret_offset + 8].fill(0);

    bytes[smudge_offset] = 0x80;
    assert!(
        ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes).is_err(),
        "negative zero must not be canonical"
    );
    bytes[smudge_offset] = 0;

    bytes[smudge_offset..smudge_offset + wide_response_bytes].fill(0x7f);
    assert!(
        ZkAmsMkheDecryptionProofViewV1::decode_release_exact(&bytes).is_err(),
        "wide response above the canonical relation bound was accepted"
    );
}

#[test]
fn signed_manifest_preflight_rejects_order_splice_and_replay() {
    let (first, first_binding) = signed_manifest(b"streaming-manifest-first", 0, 0x41);
    let (second, second_binding) = signed_manifest(b"streaming-manifest-second", 1, 0x71);
    let first_bytes = first.encode().expect("first canonical manifest");
    let second_bytes = second.encode().expect("second canonical manifest");
    let decoded = decode_streaming_manifest_exact(&first_bytes).expect("authenticated manifest");
    validate_streaming_manifest_slot_v1(&decoded, 0, first.party(), &first_binding)
        .expect("exact slot binding");

    assert_eq!(
        validate_streaming_manifest_slot_v1(&decoded, 1, second.party(), &second_binding),
        Err(DecryptionAbortReasonV1::ReorderedOrDuplicateShare)
    );

    let mut replay_binding = first_binding.clone();
    replay_binding.ciphertext_digest = [0x99; 32];
    assert_eq!(
        validate_streaming_manifest_slot_v1(&decoded, 0, first.party(), &replay_binding),
        Err(DecryptionAbortReasonV1::BindingMismatch)
    );

    let first_pointer_hash = first.polynomial().payload_blake3();
    let second_pointer_hash = second.polynomial().payload_blake3();
    let pointer_hash_offset = first_bytes
        .windows(first_pointer_hash.len())
        .position(|window| window == first_pointer_hash)
        .expect("polynomial pointer hash is present");
    let mut spliced = first_bytes.clone();
    spliced[pointer_hash_offset..pointer_hash_offset + second_pointer_hash.len()]
        .copy_from_slice(&second_pointer_hash);
    assert!(decode_streaming_manifest_exact(&spliced).is_err());

    let mut signature_mutation = first_bytes;
    *signature_mutation.last_mut().expect("signature byte") ^= 0x01;
    assert_eq!(
        decode_streaming_manifest_exact(&signature_mutation),
        Err(ZkAmsMkheErrorV1::InvalidAuthentication)
    );

    assert!(decode_streaming_manifest_exact(&second_bytes).is_ok());
}

#[test]
fn direct_object_reads_reject_short_read_snapshot_drift_mutation_and_kind_replay() {
    let bytes = test_payload(2 * ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 + 19);
    let mut stable = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        bytes.clone(),
    );
    let pointer = stable.pointer;
    let (observed, receipt) = read_complete_object_v1(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        pointer,
        &mut stable,
    )
    .expect("complete stable object");
    assert_eq!(observed.as_slice(), bytes);
    assert_eq!(receipt.canonical_bytes(), pointer.payload_bytes());
    assert_eq!(receipt.payload_blake3(), pointer.payload_blake3());

    let mut short = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        bytes.clone(),
    );
    short.short_read_at = Some(1);
    let short_pointer = short.pointer;
    assert!(matches!(
        read_complete_object_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
            short_pointer,
            &mut short,
        ),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    ));

    let mut drift = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        bytes.clone(),
    );
    drift.drift_snapshot_at = Some(3);
    let drift_pointer = drift.pointer;
    assert!(matches!(
        read_complete_object_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
            drift_pointer,
            &mut drift,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    ));

    let mut mutation = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        bytes.clone(),
    );
    mutation.mutate_payload_at = Some(2);
    let mutation_pointer = mutation.pointer;
    assert!(matches!(
        read_complete_object_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
            mutation_pointer,
            &mut mutation,
        ),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    ));

    let mut wrong_kind = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
        bytes,
    );
    let wrong_pointer = wrong_kind.pointer;
    assert!(
        read_complete_object_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
            wrong_pointer,
            &mut wrong_kind,
        )
        .is_err()
    );
}

#[test]
fn snapshot_accumulator_rejects_cross_revision_replay() {
    let bytes = test_payload(97);
    let mut first = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        bytes.clone(),
    );
    let first_pointer = first.pointer;
    let (_, first_receipt) = read_complete_object_v1(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        first_pointer,
        &mut first,
    )
    .expect("first receipt");
    let mut second = TestProvider::new(ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof, bytes);
    second.snapshot_identity = TEST_DRIFTED_SNAPSHOT_ID;
    let second_pointer = second.pointer;
    let (_, second_receipt) = read_complete_object_v1(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        second_pointer,
        &mut second,
    )
    .expect("second receipt");

    let mut accumulator = StreamingSnapshotAccumulatorV1::new();
    accumulator.observe(&first_receipt).expect("first snapshot");
    assert_eq!(
        accumulator.observe(&second_receipt),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );
}

#[test]
fn sparse_negacyclic_subtraction_has_correct_wrap_signs() {
    let modulus = TEST_MODULI[0];
    let mut source = vec![0_u64; 8];
    source[7] = 5;
    let mut challenge = vec![0_i8; 8];
    challenge[1] = 1;
    let mut accumulator = vec![0_u64; 8];
    subtract_sparse_negacyclic_product_in_place(&mut accumulator, &challenge, &source, modulus)
        .expect("positive wrapped challenge product");
    assert_eq!(accumulator[0], 5, "subtracting a wrapped term must add");

    source.fill(0);
    source[0] = 7;
    challenge.fill(0);
    challenge[1] = -1;
    accumulator.fill(0);
    subtract_sparse_negacyclic_product_in_place(&mut accumulator, &challenge, &source, modulus)
        .expect("negative challenge product");
    assert_eq!(accumulator[1], 7, "subtracting a negative term must add");

    source = (0..8).map(|index| (17 * index + 3) as u64).collect();
    challenge = vec![1, 0, -1, 0, 0, 1, 0, -1];
    accumulator = (0..8).map(|index| (31 * index + 9) as u64).collect();
    let challenge_mod = challenge
        .iter()
        .map(|value| signed_mod(i64::from(*value), modulus))
        .collect::<Vec<_>>();
    let product = negacyclic_multiply(&challenge_mod, &source, modulus, TEST_ROOTS[0])
        .expect("reference NTT product");
    let expected = accumulator
        .iter()
        .zip(product)
        .map(|(left, right)| mod_sub(*left, right, modulus))
        .collect::<Vec<_>>();
    subtract_sparse_negacyclic_product_in_place(&mut accumulator, &challenge, &source, modulus)
        .expect("sparse subtraction");
    assert_eq!(accumulator, expected);
}

#[test]
fn in_place_aggregate_matches_native_addition() {
    let profile = test_profile();
    let left = (0..profile.ring_degree * profile.moduli.len())
        .map(|index| (13 * index + 5) as u64 % profile.moduli[index / profile.ring_degree])
        .collect::<Vec<_>>();
    let right = (0..profile.ring_degree * profile.moduli.len())
        .map(|index| (19 * index + 7) as u64 % profile.moduli[index / profile.ring_degree])
        .collect::<Vec<_>>();
    let original = RnsPolynomial::from_flat(&profile, left).expect("left polynomial");
    let rhs = RnsPolynomial::from_flat(&profile, right).expect("right polynomial");
    let expected = original.add(&rhs, &profile).expect("native addition");
    let mut actual = original;
    for limb in 0..profile.moduli.len() {
        add_share_limb_in_place(&mut actual, &profile, limb, rhs.limb(&profile, limb))
            .expect("in-place limb addition");
    }
    assert_eq!(actual, expected);
}

#[test]
fn staged_sparse_coefficient_folds_match_native_vectors_exactly() {
    let challenge = vec![1, 0, -1, 0, 0, 1, 0, -1];
    let terms = sparse_challenge_terms_v1(&challenge).expect("canonical sparse term index");
    let small = vec![-3, 5, 0, 7, -11, 13, 2, -1];
    let native_small = sparse_negacyclic_mul_small(&challenge, &small)
        .expect("native sparse small multiplication");
    let staged_small = (0..small.len())
        .map(|index| sparse_fold_small_coefficient_v1(&terms, &small, index).expect("coefficient"))
        .collect::<Vec<_>>();
    assert_eq!(staged_small, native_small);

    let wide = small
        .iter()
        .copied()
        .map(SignedWideV1::from_i64)
        .collect::<Vec<_>>();
    let native_wide =
        sparse_negacyclic_mul_wide(&challenge, &wide).expect("native sparse wide multiplication");
    let staged_wide = (0..wide.len())
        .map(|index| {
            sparse_fold_wide_coefficient_v1(&terms, &wide, index).expect("wide coefficient")
        })
        .collect::<Vec<_>>();
    assert_eq!(staged_wide, native_wide);
}

#[test]
fn staged_mask_sampling_matches_native_three_pass_random_order() {
    let degree = 8;
    let wide_bound = super::super::WideMagnitudeV1::max_for_bits(13).expect("wide bound");
    let mut staged_random = KatRandom::new(b"staged-three-pass-mask-order");
    let (staged_secret, staged_error, staged_wide) =
        sample_staged_proof_masks_v1(degree, 17, 29, &wide_bound, &mut staged_random)
            .expect("staged mask sample");

    let mut native_random = KatRandom::new(b"staged-three-pass-mask-order");
    let mut native_secret =
        super::super::ZeroizingI64VectorV1::with_capacity(degree).expect("secret mask");
    for _ in 0..degree {
        native_secret.push(sample_signed_small(17, &mut native_random).expect("secret draw"));
    }
    let mut native_error =
        super::super::ZeroizingI64VectorV1::with_capacity(degree).expect("error mask");
    for _ in 0..degree {
        native_error.push(sample_signed_small(29, &mut native_random).expect("error draw"));
    }
    let mut native_wide =
        super::super::ZeroizingSignedWideVectorV1::with_capacity(degree).expect("wide mask");
    for _ in 0..degree {
        native_wide
            .push(sample_signed_wide(&wide_bound, &mut native_random).expect("wide mask draw"));
    }

    assert_eq!(staged_secret.as_slice(), native_secret.as_slice());
    assert_eq!(staged_error.as_slice(), native_error.as_slice());
    assert_eq!(staged_wide.as_slice(), native_wide.as_slice());
    assert_eq!(staged_random.counter, native_random.counter);
}

#[test]
fn staged_complete_zadp_encoding_matches_native_section_order_and_bytes() {
    let degree = 8;
    let challenge_seed = [0xa7; 32];
    let challenge = derive_sparse_challenge(degree, challenge_seed).expect("challenge");
    let terms = sparse_challenge_terms_v1(&challenge).expect("challenge terms");
    let witness_secret = [-1, 0, 1, -1, 1, 0, -1, 1];
    let witness_error = [-2, 1, 0, 2, -1, 1, 2, 0];
    let secret_mask = [91, 92, 93, 94, 95, 96, 97, 98];
    let error_mask = [-71, -72, -73, -74, -75, -76, -77, -78];
    let witness_smudge = [-9, 2, 0, 7, -3, 4, 1, -8]
        .into_iter()
        .map(SignedWideV1::from_i64)
        .collect::<Vec<_>>();
    let smudge_mask = [301, 302, 303, 304, 305, 306, 307, 308]
        .into_iter()
        .map(SignedWideV1::from_i64)
        .collect::<Vec<_>>();
    let secret_response = (0..degree)
        .map(|index| staged_small_response_v1(secret_mask[index], &terms, &witness_secret, index))
        .collect::<Result<Vec<_>, _>>()
        .expect("secret responses");
    let error_response = (0..degree)
        .map(|index| staged_small_response_v1(error_mask[index], &terms, &witness_error, index))
        .collect::<Result<Vec<_>, _>>()
        .expect("error responses");
    let smudge_response = (0..degree)
        .map(|index| staged_wide_response_v1(&smudge_mask[index], &terms, &witness_smudge, index))
        .collect::<Result<Vec<_>, _>>()
        .expect("smudge responses");
    let wide_response_bytes = 17_u16;
    let native = super::super::ZkAmsMkheDecryptionProofV1 {
        wide_response_bytes,
        challenge_seed,
        secret_response: secret_response.clone(),
        public_key_error_response: error_response.clone(),
        smudge_response: smudge_response.clone(),
    }
    .encode()
    .expect("native proof bytes");

    let mut staged = Vec::with_capacity(native.len());
    staged.extend_from_slice(
        &staged_proof_header_v1(degree, usize::from(wide_response_bytes), challenge_seed)
            .expect("staged header"),
    );
    for response in &secret_response {
        staged.extend_from_slice(&response.to_be_bytes());
    }
    for response in &error_response {
        staged.extend_from_slice(&response.to_be_bytes());
    }
    for response in &smudge_response {
        let mut encoded = [0_u8; 17];
        encode_signed_wide_fixed_into_v1(response, &mut encoded).expect("wide response");
        staged.extend_from_slice(&encoded);
    }
    assert_eq!(staged, native);
}

#[test]
fn staged_transcript_rns_frames_match_native_order_and_bytes() {
    let profile = test_profile();
    let (_, binding) = signed_manifest(b"staged-transcript-parity", 0, 0x31);
    let polynomials = (0..7)
        .map(|polynomial| {
            let coefficients = (0..profile.moduli.len())
                .flat_map(|limb| {
                    let modulus = profile.moduli[limb];
                    (0..profile.ring_degree).map(move |coefficient| {
                        (u64::try_from(polynomial * 101 + limb * 17 + coefficient * 29 + 1)
                            .expect("small test index"))
                            % modulus
                    })
                })
                .collect::<Vec<_>>();
            RnsPolynomial::from_flat(&profile, coefficients).expect("test polynomial")
        })
        .collect::<Vec<_>>();

    let mut native =
        initialize_decryption_challenge_transcript(&profile, 13, &binding).expect("native prefix");
    for polynomial in &polynomials {
        super::super::update_rns_hash(&mut native, &profile, polynomial).expect("native RNS frame");
    }

    let mut staged =
        initialize_decryption_challenge_transcript(&profile, 13, &binding).expect("staged prefix");
    for polynomial in &polynomials {
        update_streamed_rns_header(&mut staged, &profile).expect("staged RNS header");
        for limb in 0..profile.moduli.len() {
            update_residue_limb(&mut staged, polynomial.limb(&profile, limb));
        }
    }
    assert_eq!(staged.finalize(), native.finalize());
}

#[test]
fn streamed_ciphertext_digest_frames_match_native_component_major_bytes() {
    let profile = test_profile();
    let polynomial = |domain: u64| {
        let coefficients = profile
            .moduli
            .iter()
            .enumerate()
            .flat_map(|(limb, modulus)| {
                (0..profile.ring_degree).map(move |coefficient| {
                    (domain
                        + u64::try_from(limb * 101 + coefficient * 17)
                            .expect("tiny coefficient index"))
                        % modulus
                })
            })
            .collect::<Vec<_>>();
        RnsPolynomial::from_flat(&profile, coefficients).expect("canonical tiny polynomial")
    };
    let constant = polynomial(7);
    let linear = polynomial(19);
    let profile_digest = profile.digest().expect("tiny profile digest");
    let roster_digest = [0x21; 32];
    let epoch = 9_u64;
    let transcript_digest = [0x31; 32];
    let sample_index = 11_u64;
    let level = 0_u8;

    let mut native = Keccak256::new();
    native.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
    native.update(&profile_digest);
    native.update(&roster_digest);
    native.update(&epoch.to_be_bytes());
    native.update(&transcript_digest);
    native.update(&sample_index.to_be_bytes());
    native.update(&[level]);
    super::super::update_rns_hash(&mut native, &profile, &constant).expect("native constant frame");
    super::super::update_rns_hash(&mut native, &profile, &linear).expect("native linear frame");

    let mut streamed = Keccak256::new();
    streamed.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
    streamed.update(&profile_digest);
    streamed.update(&roster_digest);
    streamed.update(&epoch.to_be_bytes());
    streamed.update(&transcript_digest);
    streamed.update(&sample_index.to_be_bytes());
    streamed.update(&[level]);
    for component in [&constant, &linear] {
        update_streamed_rns_header(&mut streamed, &profile).expect("streamed component header");
        for limb in 0..profile.moduli.len() {
            update_residue_limb(&mut streamed, component.limb(&profile, limb));
        }
    }
    assert_eq!(streamed.finalize(), native.finalize());

    let mut wrong_order = Keccak256::new();
    wrong_order.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
    wrong_order.update(&profile_digest);
    wrong_order.update(&roster_digest);
    wrong_order.update(&epoch.to_be_bytes());
    wrong_order.update(&transcript_digest);
    wrong_order.update(&sample_index.to_be_bytes());
    wrong_order.update(&[level]);
    for component in [&linear, &constant] {
        update_streamed_rns_header(&mut wrong_order, &profile).expect("streamed component header");
        for limb in 0..profile.moduli.len() {
            update_residue_limb(&mut wrong_order, component.limb(&profile, limb));
        }
    }
    assert_ne!(
        wrong_order.finalize(),
        streamed_ciphertext_digest_reference_v1(
            &profile,
            profile_digest,
            roster_digest,
            epoch,
            transcript_digest,
            sample_index,
            level,
            &constant,
            &linear,
        )
    );
}

#[allow(clippy::too_many_arguments)]
fn streamed_ciphertext_digest_reference_v1(
    profile: &BgvProfile,
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    sample_index: u64,
    level: u8,
    constant: &RnsPolynomial,
    linear: &RnsPolynomial,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(COLLECTIVE_CIPHERTEXT_DOMAIN_V1);
    hash.update(&profile_digest);
    hash.update(&roster_digest);
    hash.update(&epoch.to_be_bytes());
    hash.update(&transcript_digest);
    hash.update(&sample_index.to_be_bytes());
    hash.update(&[level]);
    super::super::update_rns_hash(&mut hash, profile, constant).expect("constant frame");
    super::super::update_rns_hash(&mut hash, profile, linear).expect("linear frame");
    hash.finalize()
}

#[test]
fn staged_random_budget_accepts_boundary_and_rejects_one_over_or_source_error() {
    struct RecordingRandom {
        forwarded: u64,
        fail: bool,
    }

    impl MaskedRelaxedRandomSourceV1 for RecordingRandom {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            self.forwarded += u64::try_from(destination.len()).expect("test length");
            if self.fail {
                return Err(MaskedRelaxedRandomErrorV1::Unavailable);
            }
            destination.fill(0x5a);
            Ok(())
        }
    }

    let mut source = RecordingRandom {
        forwarded: 0,
        fail: false,
    };
    let mut bounded = StagedProverBudgetedRandomSourceV1::new(&mut source, 3).expect("budget");
    bounded.fill_bytes(&mut [0_u8; 2]).expect("within budget");
    bounded.fill_bytes(&mut [0_u8; 1]).expect("exact boundary");
    assert_eq!(bounded.remaining_bytes, 0);
    assert_eq!(
        bounded.fill_bytes(&mut [0_u8; 1]),
        Err(MaskedRelaxedRandomErrorV1::Unavailable)
    );
    drop(bounded);
    assert_eq!(
        source.forwarded, 3,
        "one-over request must not be forwarded"
    );

    let mut source = RecordingRandom {
        forwarded: 0,
        fail: true,
    };
    let mut bounded = StagedProverBudgetedRandomSourceV1::new(&mut source, 4).expect("budget");
    assert_eq!(
        bounded.fill_bytes(&mut [0_u8; 2]),
        Err(MaskedRelaxedRandomErrorV1::Unavailable)
    );
    assert_eq!(
        bounded.remaining_bytes, 2,
        "failed source draw stays charged"
    );
}

#[test]
fn staged_outer_attempt_121_is_fail_closed_without_weakening_inner_sampling() {
    assert_eq!(STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1, 120);
    assert_eq!(MAX_RANDOM_REJECTION_ATTEMPTS_V1, 128);
    assert!((0..STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1).contains(&119));
    assert!(!(0..STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1).contains(&120));
}

#[test]
fn staged_zeroizing_ntt_and_wide_encoding_match_native_bytes() {
    let modulus = TEST_MODULI[0];
    let left = (0..8)
        .map(|index| (17 * index + 3) as u64 % modulus)
        .collect::<Vec<_>>();
    let right = (0..8)
        .map(|index| (29 * index + 11) as u64 % modulus)
        .collect::<Vec<_>>();
    let native = negacyclic_multiply(&left, &right, modulus, TEST_ROOTS[0])
        .expect("native negacyclic product");
    let staged = negacyclic_multiply_staged_v1(&left, &right, modulus, TEST_ROOTS[0])
        .expect("zeroizing staged product");
    assert_eq!(staged.as_slice(), native);

    for value in [-91_i64, -1, 0, 1, 91] {
        let value = SignedWideV1::from_i64(value);
        let native = value.encode_fixed(17).expect("native fixed encoding");
        let mut staged = [0_u8; 17];
        encode_signed_wide_fixed_into_v1(&value, &mut staged).expect("staged fixed encoding");
        assert_eq!(staged.as_slice(), native);
    }
}

#[test]
fn verifier_commitment_limb_owner_zeroizes_on_success_error_and_unwind() {
    let modulus = TEST_MODULI[0];
    let left = (0..8)
        .map(|index| (17 * index + 3) as u64 % modulus)
        .collect::<Vec<_>>();
    let right = (0..8)
        .map(|index| (29 * index + 11) as u64 % modulus)
        .collect::<Vec<_>>();

    reset_decryption_transient_zeroized_drop_count_v1();
    drop(
        negacyclic_multiply_staged_v1(&left, &right, modulus, TEST_ROOTS[0])
            .expect("zeroizing verifier commitment limb"),
    );
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 2);

    reset_decryption_transient_zeroized_drop_count_v1();
    assert!(matches!(
        negacyclic_multiply_staged_v1(&left, &right, modulus, 0),
        Err(ZkAmsMkheErrorV1::InvalidProfile)
    ));
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 2);

    reset_decryption_transient_zeroized_drop_count_v1();
    let unwind = std::panic::catch_unwind(|| {
        let commitment = negacyclic_multiply_staged_v1(&left, &right, modulus, TEST_ROOTS[0])
            .expect("zeroizing verifier commitment limb");
        assert_eq!(commitment.as_slice().len(), left.len());
        panic!("exercise verifier commitment limb unwind erasure");
    });
    assert!(unwind.is_err());
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 2);
}

#[test]
fn staged_transients_zeroize_on_success_error_and_unwind() {
    reset_decryption_transient_zeroized_drop_count_v1();
    {
        let mut values = ZeroizingStagedU64VectorV1::with_capacity(4).expect("allocation");
        values.push(7);
        values.push(9);
    }
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);

    reset_decryption_transient_zeroized_drop_count_v1();
    let failed = (|| -> Result<(), ZkAmsMkheErrorV1> {
        let mut values = ZeroizingStagedU64VectorV1::with_capacity(4)?;
        values.push(0xdead_beef);
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    })();
    assert_eq!(failed, Err(ZkAmsMkheErrorV1::RandomUnavailable));
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);

    reset_decryption_transient_zeroized_drop_count_v1();
    let unwind = std::panic::catch_unwind(|| {
        let mut bytes = ZeroizingStagedBytesV1::<32>::zeroed();
        bytes.as_mut_slice().fill(0xa5);
        panic!("exercise staged unwind erasure");
    });
    assert!(unwind.is_err());
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);
}

#[test]
fn complete_proof_byte_owner_zeroizes_exact_capacity_on_success_error_and_unwind() {
    reset_decryption_transient_zeroized_drop_count_v1();
    {
        let mut bytes =
            ZeroizingStagedByteVectorV1::new_zeroed_exact(64).expect("exact proof-byte owner");
        assert_eq!(bytes.0.len(), 64);
        assert_eq!(bytes.0.capacity(), 64);
        bytes.as_mut_slice().fill(0xa5);
    }
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);

    reset_decryption_transient_zeroized_drop_count_v1();
    let failed = (|| -> Result<(), ZkAmsMkheErrorV1> {
        let mut bytes = ZeroizingStagedByteVectorV1::new_zeroed_exact(64)?;
        bytes.as_mut_slice().fill(0x5a);
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    })();
    assert_eq!(failed, Err(ZkAmsMkheErrorV1::InvalidWireEncoding));
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);

    reset_decryption_transient_zeroized_drop_count_v1();
    let unwind = std::panic::catch_unwind(|| {
        let mut bytes =
            ZeroizingStagedByteVectorV1::new_zeroed_exact(64).expect("exact proof-byte owner");
        bytes.as_mut_slice().fill(0x3c);
        panic!("exercise complete proof-byte unwind erasure");
    });
    assert!(unwind.is_err());
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);
}

#[test]
fn decoded_secret_limb_owner_zeroizes_exact_capacity_on_success_error_and_unwind() {
    let profile = release_profile_v1();
    let encoded = vec![0_u8; profile.ring_degree * size_of::<i64>()];
    let proof = ZkAmsMkheDecryptionProofViewV1 {
        bytes: &encoded,
        challenge_seed: [0; 32],
        secret_offset: 0,
        error_offset: encoded.len(),
        smudge_offset: encoded.len(),
        wide_response_bytes: 1,
    };

    reset_decryption_transient_zeroized_drop_count_v1();
    {
        let limb = proof
            .secret_limb(profile.moduli[0])
            .expect("complete secret-response limb");
        assert_eq!(limb.0.len(), profile.ring_degree);
        assert_eq!(limb.0.capacity(), profile.ring_degree);
    }
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);

    reset_decryption_transient_zeroized_drop_count_v1();
    let truncated = [0_u8; size_of::<i64>()];
    let truncated_proof = ZkAmsMkheDecryptionProofViewV1 {
        bytes: &truncated,
        challenge_seed: [0; 32],
        secret_offset: 0,
        error_offset: truncated.len(),
        smudge_offset: truncated.len(),
        wide_response_bytes: 1,
    };
    assert!(matches!(
        truncated_proof.secret_limb(profile.moduli[0]),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    ));
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);

    reset_decryption_transient_zeroized_drop_count_v1();
    let unwind = std::panic::catch_unwind(|| {
        let limb = proof
            .secret_limb(profile.moduli[0])
            .expect("complete secret-response limb");
        assert_eq!(limb.as_slice().len(), profile.ring_degree);
        panic!("exercise decoded secret-response limb unwind erasure");
    });
    assert!(unwind.is_err());
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);
}

fn test_streamed_rns_payload_v1(profile: &BgvProfile) -> Vec<u8> {
    let coefficient_count = profile.ring_degree * profile.moduli.len();
    let mut payload = Vec::with_capacity(size_of::<u32>() + coefficient_count * size_of::<u64>());
    payload.extend_from_slice(
        &u32::try_from(coefficient_count)
            .expect("tiny coefficient count")
            .to_be_bytes(),
    );
    for (limb, modulus) in profile.moduli.iter().copied().enumerate() {
        for index in 0..profile.ring_degree {
            let residue = (u64::try_from(31 * limb + index).expect("tiny residue") + 1) % modulus;
            payload.extend_from_slice(&residue.to_be_bytes());
        }
    }
    payload
}

#[test]
fn streamed_rns_limb_owner_and_scratch_zeroize_on_success_error_and_unwind() {
    let profile = test_profile();
    let payload = test_streamed_rns_payload_v1(&profile);

    reset_decryption_transient_zeroized_drop_count_v1();
    let mut provider = TestProvider::new(ZkAmsMkheDirectObjectKindV1::CpkPartyB, payload.clone());
    let pointer = provider.pointer;
    let mut reader = StreamingRnsObjectReaderV1::begin(
        ZkAmsMkheDirectObjectKindV1::CpkPartyB,
        pointer,
        &profile,
        &mut provider,
    )
    .expect("streamed RNS reader");
    let first = reader.read_limb(&profile, 0).expect("first streamed limb");
    assert_eq!(first.0.len(), profile.ring_degree);
    assert_eq!(first.0.capacity(), profile.ring_degree);
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);
    drop(first);
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 2);
    drop(reader.read_limb(&profile, 1).expect("second streamed limb"));
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 4);
    reader.finish(&profile).expect("complete streamed RNS read");

    reset_decryption_transient_zeroized_drop_count_v1();
    let mut provider = TestProvider::new(ZkAmsMkheDirectObjectKindV1::CpkPartyB, payload.clone());
    provider.short_read_at = Some(2);
    let pointer = provider.pointer;
    let mut reader = StreamingRnsObjectReaderV1::begin(
        ZkAmsMkheDirectObjectKindV1::CpkPartyB,
        pointer,
        &profile,
        &mut provider,
    )
    .expect("streamed RNS reader");
    assert!(matches!(
        reader.read_limb(&profile, 0),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    ));
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 2);

    reset_decryption_transient_zeroized_drop_count_v1();
    let unwind = std::panic::catch_unwind(|| {
        let mut provider =
            TestProvider::new(ZkAmsMkheDirectObjectKindV1::CpkPartyB, payload.clone());
        let pointer = provider.pointer;
        let mut reader = StreamingRnsObjectReaderV1::begin(
            ZkAmsMkheDirectObjectKindV1::CpkPartyB,
            pointer,
            &profile,
            &mut provider,
        )
        .expect("streamed RNS reader");
        let limb = reader.read_limb(&profile, 0).expect("streamed RNS limb");
        assert_eq!(limb.as_slice().len(), profile.ring_degree);
        panic!("exercise streamed RNS limb unwind erasure");
    });
    assert!(unwind.is_err());
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 2);
}

#[test]
fn decrypted_aggregate_owner_zeroizes_on_success_error_and_unwind() {
    let profile = test_profile();

    reset_decryption_transient_zeroized_drop_count_v1();
    {
        let mut aggregate =
            ZeroizingAggregateRnsV1::zero_exact_v1(&profile).expect("zeroizing aggregate");
        assert_eq!(
            aggregate.coefficients_mut().len(),
            profile.ring_degree * profile.moduli.len()
        );
        aggregate.coefficients_mut()[0] = 17;
    }
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);

    reset_decryption_transient_zeroized_drop_count_v1();
    let failed = (|| -> Result<(), ZkAmsMkheErrorV1> {
        let mut aggregate = ZeroizingAggregateRnsV1::zero_exact_v1(&profile)?;
        aggregate.coefficients_mut()[0] = 19;
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    })();
    assert_eq!(failed, Err(ZkAmsMkheErrorV1::InvalidPolynomial));
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);

    reset_decryption_transient_zeroized_drop_count_v1();
    let unwind = std::panic::catch_unwind(|| {
        let mut aggregate =
            ZeroizingAggregateRnsV1::zero_exact_v1(&profile).expect("zeroizing aggregate");
        aggregate.coefficients_mut()[0] = 23;
        panic!("exercise decrypted aggregate unwind erasure");
    });
    assert!(unwind.is_err());
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 1);
}

#[test]
fn verifier_decode_and_streamed_limb_sources_use_only_zeroizing_owners() {
    let streaming = include_str!("decryption_streaming.rs");
    let secret_limb = streaming
        .split("fn secret_limb(")
        .nth(1)
        .expect("secret-response limb decoder")
        .split("fn error_mod")
        .next()
        .expect("secret-response decoder boundary");
    assert!(secret_limb.contains("Result<ZeroizingStagedU64VectorV1"));
    assert!(secret_limb.contains("ZeroizingStagedU64VectorV1::new_zeroed"));
    assert!(!secret_limb.contains("Vec<u64>"));

    let read_limb = streaming
        .split("fn read_limb(")
        .nth(1)
        .expect("streamed RNS limb reader")
        .split("fn finish(")
        .next()
        .expect("streamed RNS reader boundary");
    assert!(read_limb.contains("Result<ZeroizingStagedU64VectorV1"));
    assert!(read_limb.contains("ZeroizingStagedBytesV1::<"));
    assert!(!read_limb.contains("Vec<u64>"));
    assert!(!read_limb.contains("let mut buffer = [0_u8;"));

    let public_key_commitment = streaming
        .split("fn reconstruct_public_key_commitment_v1")
        .nth(1)
        .expect("public-key commitment reconstruction")
        .split("fn reconstruct_share_commitment_and_aggregate_v1")
        .next()
        .expect("public-key commitment boundary");
    let share_commitment = streaming
        .split("fn reconstruct_share_commitment_and_aggregate_v1")
        .nth(1)
        .expect("share commitment reconstruction")
        .split("/// Verify one native share relation")
        .next()
        .expect("share commitment boundary");
    for commitment in [public_key_commitment, share_commitment] {
        assert_eq!(
            commitment.matches("negacyclic_multiply_staged_v1(").count(),
            1
        );
        assert!(commitment.contains("commitment.as_mut_slice()"));
        assert!(commitment.contains("commitment.as_slice()"));
        assert!(!commitment.contains("let mut commitment = negacyclic_multiply("));
    }

    let combine = streaming
        .split("pub fn verify_combine_decode_zk_ams_mkhe_decryption_streaming_v1")
        .nth(1)
        .expect("streaming combine verifier")
        .split("#[cfg(test)]")
        .next()
        .expect("streaming combine boundary");
    assert!(combine.contains("ZeroizingAggregateRnsV1::zero_exact_v1(&profile)"));
    assert!(combine.contains("decode_centered_plaintext(&profile, aggregate.as_rns()"));
    assert!(!combine.contains("let mut aggregate_coefficients = Vec::new()"));
    assert!(!combine.contains("RnsPolynomial::from_flat"));
}

#[test]
fn staged_mask_owners_zeroize_on_success_error_and_unwind() {
    let wide_bound = super::super::WideMagnitudeV1::max_for_bits(13).expect("wide bound");

    reset_decryption_transient_zeroized_drop_count_v1();
    let mut random = KatRandom::new(b"staged-mask-zeroize-success");
    let masks = sample_staged_proof_masks_v1(8, 17, 29, &wide_bound, &mut random)
        .expect("successful masks");
    let before_owner_drop = decryption_transient_zeroized_drop_count_v1();
    drop(masks);
    assert_eq!(
        decryption_transient_zeroized_drop_count_v1(),
        before_owner_drop + 3,
        "both i64 owners and the signed-wide owner must clear"
    );

    struct FailAfterFirstFill {
        fills: usize,
        panic_instead: bool,
    }

    impl MaskedRelaxedRandomSourceV1 for FailAfterFirstFill {
        fn fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), MaskedRelaxedRandomErrorV1> {
            self.fills += 1;
            if self.fills > 1 {
                if self.panic_instead {
                    panic!("exercise partial staged-mask sampling unwind");
                }
                return Err(MaskedRelaxedRandomErrorV1::Unavailable);
            }
            destination.fill(0);
            Ok(())
        }
    }

    reset_decryption_transient_zeroized_drop_count_v1();
    let mut failing = FailAfterFirstFill {
        fills: 0,
        panic_instead: false,
    };
    assert!(matches!(
        sample_staged_proof_masks_v1(8, 17, 29, &wide_bound, &mut failing),
        Err(ZkAmsMkheErrorV1::RandomUnavailable)
    ));
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 3);

    reset_decryption_transient_zeroized_drop_count_v1();
    let unwind = std::panic::catch_unwind(|| {
        let mut panicking = FailAfterFirstFill {
            fills: 0,
            panic_instead: true,
        };
        let _ = sample_staged_proof_masks_v1(8, 17, 29, &wide_bound, &mut panicking);
    });
    assert!(unwind.is_err());
    assert_eq!(decryption_transient_zeroized_drop_count_v1(), 3);
}

#[test]
fn staged_pointer_adapter_rejects_kind_and_component_confusion() {
    let evidence = derive_decryption_resource_evidence(&release_profile_v1())
        .expect("release decryption shape");
    let cpk = ZkAmsMkheDirectObjectPointerV1::new(
        ZkAmsMkheDirectObjectKindV1::CpkPartyB,
        evidence.split_polynomial_object_bytes,
        [0x31; 32],
    )
    .expect("same-size CPK pointer");
    assert!(
        direct_to_decryption_transport_pointer_v1(
            cpk,
            ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
            ZkAmsMkheDecryptionTransportComponentKindV1::SharePolynomial,
        )
        .is_err()
    );

    let share = ZkAmsMkheDirectObjectPointerV1::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
        evidence.split_polynomial_object_bytes,
        [0x41; 32],
    )
    .expect("share pointer");
    assert!(
        direct_to_decryption_transport_pointer_v1(
            share,
            ZkAmsMkheDirectObjectKindV1::DecryptionSharePolynomial,
            ZkAmsMkheDecryptionTransportComponentKindV1::ProofEnvelope,
        )
        .is_err()
    );
}

#[test]
fn staged_header_is_byte_identical_to_native_zadp_header() {
    let native = valid_release_proof_bytes();
    let header = staged_proof_header_v1(
        release_profile_v1().ring_degree,
        usize::from(
            derive_decryption_resource_evidence(&release_profile_v1())
                .expect("release evidence")
                .wide_response_coefficient_bytes,
        ),
        [0x5a; 32],
    )
    .expect("staged header");
    assert_eq!(
        header.as_slice(),
        &native[..DECRYPTION_PROOF_HEADER_BYTES_V1]
    );
}

#[test]
fn staged_prover_source_is_capability_owned_semantic_and_fail_closed() {
    let streaming = include_str!("decryption_streaming.rs");
    let persistent = include_str!("persistent_decryption_equality.rs");
    let common_a = include_str!("cpk_relation_common_a.rs");
    let start = streaming
        .find("pub fn prove_zk_ams_mkhe_decryption_share_staged_v1")
        .expect("public staged prover");
    let end = streaming[start..]
        .find("/// Zero-copy canonical view")
        .map(|offset| start + offset)
        .expect("staged prover end");
    let staged = &streaming[start..end];

    assert!(staged.contains("persistent_use: ZkAmsMkhePersistentDecryptionPartyUseV1"));
    assert!(
        staged.contains("let common_a_context = statement.prepare_common_a_context(&profile)?;")
    );
    assert!(staged.contains("statement.consume_party_use_v1("));
    assert!(staged.contains("publish_staged_share_polynomial_v1("));
    assert!(staged.contains("publish_staged_decryption_proof_v1("));
    assert!(staged.contains("verify_published_staged_relation_v1("));
    assert!(staged.contains("decode_streaming_manifest_exact(&manifest_bytes)?"));
    assert!(staged.contains("validate_streaming_manifest_slot_v1("));
    assert!(!staged.contains("prove_decryption_relation("));
    assert!(!staged.contains("create_decryption_share("));
    assert!(!staged.contains("split_zk_ams_mkhe_decryption_share_v1("));
    assert!(!streaming.contains("impl Clone for ZkAmsMkheStagedDecryptionShareV1"));
    assert!(!streaming.contains("from_raw_staged"));
    assert!(!streaming.contains("derive_active_collective_public_a_limb_budgeted_v1"));
    assert_eq!(
        streaming
            .matches("derive_prepared_common_a_limb_v1(")
            .count(),
        5,
        "one narrow adapter plus all four staged common-a derivation sites"
    );
    assert!(streaming.contains(".published_object_identity()"));
    assert!(streaming.contains("const STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1: usize = 120;"));
    assert!(
        streaming.contains("let transcript_prefix = build_staged_decryption_transcript_prefix_v1(")
    );
    assert!(streaming.contains("let mut transcript = prefix.fork_v1();"));
    assert!(
        streaming
            .contains("let mut bytes = ZeroizingStagedByteVectorV1::new_zeroed_exact(length)?;")
    );
    assert!(
        streaming.contains(
            "ZkAmsMkheDecryptionProofViewV1::decode_release_exact(proof_bytes.as_slice())"
        )
    );

    let statement = streaming
        .split("pub struct ZkAmsMkheStreamingDecryptionStatementV1<'a>")
        .nth(1)
        .expect("direct streaming statement")
        .split("impl fmt::Debug for ZkAmsMkheStreamingDecryptionStatementV1")
        .next()
        .expect("direct streaming statement boundary");
    assert!(statement.contains("&'a ZkAmsMkheStreamingCollectiveCiphertextV1"));
    assert!(statement.contains("ciphertext_axes: DecryptionCiphertextAxesV1"));
    assert!(statement.contains("ciphertext_snapshot: ZkAmsMkheDecryptionStreamingSnapshotV1"));
    assert!(!statement.contains("ZkAmsMkheCollectiveCiphertextWireV1"));
    assert!(!statement.contains("ZkAmsMkheCollectiveCiphertextV1"));
    assert!(!statement.contains("RnsPolynomial"));

    let live_admission = streaming
        .split("fn validate_streaming_ciphertext_live_v1<P>")
        .nth(1)
        .expect("live ciphertext admission")
        .split("/// Compact, context-minted release statement")
        .next()
        .expect("live ciphertext admission boundary");
    for required in [
        "key_material_digest: binding.key_material_digest()",
        "key_transcript_digest: binding.key_transcript_digest()",
        "collective_key_digest: binding.key_digest()",
        ".validate_expected_v1(DecryptionCiphertextKeyLineageV1",
        "hash_streaming_ciphertext_components_v1(",
        "digest.finalize() != binding.ciphertext_digest()",
        "ciphertext_record_index",
        "u64::from(ciphertext_record_index) >= maximum_samples",
        "binding.sample_index() >= maximum_samples",
    ] {
        assert!(
            live_admission.contains(required),
            "missing live admission check: {required}"
        );
    }
    assert!(
        live_admission
            .find("u64::from(ciphertext_record_index) >= maximum_samples")
            .expect("record bound preflight")
            < live_admission
                .find("hash_streaming_ciphertext_components_v1(")
                .expect("live component hash")
    );

    let preflight = staged
        .find("zk_ams_mkhe_decryption_streaming_residency_evidence_v1()?")
        .expect("streaming evidence preflight");
    let prepare_common_a = staged
        .find("statement.prepare_common_a_context(&profile)?")
        .expect("prepared common-a");
    let consume = staged
        .find("statement.consume_party_use_v1(")
        .expect("consume");
    let cached_prefix = staged
        .find("let transcript_prefix = build_staged_decryption_transcript_prefix_v1(")
        .expect("cached transcript prefix");
    let attempt_loop = staged
        .find("for _ in 0..STAGED_PROVER_MAXIMUM_FS_ATTEMPTS_V1")
        .expect("bounded attempt loop");
    let first_random = staged
        .find("validate_wide_relation_random_health(&mut bounded_random)?")
        .expect("random health");
    let first_publish = staged
        .find("publish_staged_share_polynomial_v1(")
        .expect("publication");
    let semantic_replay = staged
        .find("verify_published_staged_relation_v1(")
        .expect("semantic replay");
    let authentication = staged
        .find("party_secret.authenticate_artifact(")
        .expect("manifest authentication");
    assert!(preflight < prepare_common_a && prepare_common_a < consume);
    assert!(consume < first_random && consume < first_publish);
    assert!(first_publish < cached_prefix && cached_prefix < attempt_loop);
    assert!(semantic_replay < authentication);

    let attempts = &staged[attempt_loop..semantic_replay];
    assert!(attempts.contains("&transcript_prefix"));
    assert!(!attempts.contains("build_staged_decryption_transcript_prefix_v1("));

    assert!(persistent.contains("bind_streaming_statement_party_uses_v1("));
    assert!(persistent.contains("consume_streaming_party_use_v1("));
    assert!(persistent.contains("validate_streaming_party_state_axes_v1("));
    assert!(persistent.contains("party_use != expected"));
    assert!(streaming.contains("super::ZeroizingI64VectorV1::with_capacity"));
    assert!(streaming.contains("ZeroizingStagedU64VectorV1::with_capacity"));
    assert!(streaming.contains("drop(secret_mask);"));
    assert!(streaming.contains("drop(smudge);"));
    assert!(
        common_a
            .contains("pub(in super::super) struct ZkAmsMkhePreparedCollectivePublicAContextV1")
    );
    assert!(common_a.contains("pub(in super::super) fn derive_limb_budgeted_v1("));
    assert!(common_a.contains("validate_profile_digest_axis_v1("));
    assert!(!common_a.contains("derive(Clone"));
}

#[test]
#[ignore = "release-size all-38-limb staged/native NTT equivalence; isolated resource job only"]
fn staged_ntt_matches_native_on_every_release_limb_and_boundary_pattern() {
    let profile = release_profile_v1();
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let left = (0..profile.ring_degree)
            .map(|index| match index % 5 {
                0 => 0,
                1 => 1,
                2 => modulus - 1,
                3 => modulus / 2,
                _ => {
                    u64::try_from(index)
                        .expect("release index")
                        .wrapping_mul(0x9e37_79b9)
                        .wrapping_add(u64::try_from(limb).expect("release limb") + 17)
                        % modulus
                }
            })
            .collect::<Vec<_>>();
        let right = (0..profile.ring_degree)
            .map(|index| match index % 7 {
                0 => modulus - 1,
                1 => 0,
                2 => 1,
                _ => {
                    u64::try_from(index)
                        .expect("release index")
                        .wrapping_mul(0x85eb_ca6b)
                        .wrapping_add(u64::try_from(limb).expect("release limb") + 31)
                        % modulus
                }
            })
            .collect::<Vec<_>>();
        let native = negacyclic_multiply(&left, &right, modulus, profile.negacyclic_roots[limb])
            .expect("native release NTT");
        let staged =
            negacyclic_multiply_staged_v1(&left, &right, modulus, profile.negacyclic_roots[limb])
                .expect("staged release NTT");
        assert_eq!(staged.as_slice(), native.as_slice(), "release limb {limb}");
    }
}

#[test]
#[ignore = "release-size 38-limb arithmetic equivalence; run only in an isolated resource job"]
fn sparse_subtraction_matches_ntt_on_all_release_limbs() {
    let profile = release_profile_v1();
    let challenge = derive_sparse_challenge(profile.ring_degree, [0xa5; 32])
        .expect("deterministic release challenge");
    for limb in 0..profile.moduli.len() {
        let modulus = profile.moduli[limb];
        let source = (0..profile.ring_degree)
            .map(|index| {
                (u64::try_from(index)
                    .expect("index")
                    .wrapping_mul(0x9e37_79b9)
                    .wrapping_add(u64::try_from(limb).expect("limb") + 17))
                    % modulus
            })
            .collect::<Vec<_>>();
        let mut actual = (0..profile.ring_degree)
            .map(|index| {
                (u64::try_from(index)
                    .expect("index")
                    .wrapping_mul(0x85eb_ca6b)
                    .wrapping_add(31))
                    % modulus
            })
            .collect::<Vec<_>>();
        let challenge_mod = challenge
            .iter()
            .map(|value| signed_mod(i64::from(*value), modulus))
            .collect::<Vec<_>>();
        let product = negacyclic_multiply(
            &challenge_mod,
            &source,
            modulus,
            profile.negacyclic_roots[limb],
        )
        .expect("release NTT reference");
        let expected = actual
            .iter()
            .zip(product)
            .map(|(left, right)| mod_sub(*left, right, modulus))
            .collect::<Vec<_>>();
        subtract_sparse_negacyclic_product_in_place(&mut actual, &challenge, &source, modulus)
            .expect("release sparse multiplication");
        assert_eq!(actual, expected, "release limb {limb}");
    }
}
