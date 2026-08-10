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
    assert_eq!(evidence.ciphertext_input_bytes, 79_691_776);
    assert_eq!(evidence.aggregate_bytes, 39_845_888);
    assert_eq!(evidence.proof_view_backing_bytes, 33_030_199);
    assert_eq!(evidence.manifest_preflight_bytes, 3_984);
    assert_eq!(evidence.direct_read_buffer_bytes, 8_192);
    assert_eq!(evidence.sparse_challenge_bytes, 131_072);
    assert_eq!(evidence.manifest_preflight_peak_bytes, 79_695_760);
    assert_eq!(evidence.proof_load_peak_bytes, 152_580_039);
    assert_eq!(evidence.public_input_hash_peak_bytes, 153_759_687);
    assert_eq!(evidence.public_key_commitment_peak_bytes, 156_905_415);
    assert_eq!(evidence.share_commitment_peak_bytes, 155_856_839);
    assert_eq!(evidence.crt_decode_peak_bytes, 123_735_952);
    assert_eq!(evidence.enumerated_verifier_peak_bytes, 156_905_415);
    assert_eq!(evidence.governed_workspace_ceiling_bytes, 167_772_160);
    assert_eq!(evidence.maximum_full_rns_polynomials, 3);
    assert_eq!(evidence.maximum_rns_limb_buffers, 4);
    assert_eq!(evidence.party_b_passes, 2);
    assert_eq!(evidence.decryption_share_passes, 2);
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
        119_546_012
    );
    assert_eq!(
        evidence.staged_prover_public_input_hash_peak_bytes,
        154_149_178
    );
    assert_eq!(evidence.staged_prover_commitment_peak_bytes, 157_286_714);
    assert_eq!(evidence.staged_prover_proof_write_peak_bytes, 153_231_836);
    assert_eq!(
        evidence.staged_prover_self_verification_peak_bytes,
        119_153_329
    );
    assert_eq!(evidence.staged_prover_enumerated_peak_bytes, 157_286_714);
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
        1_205_338_112
    );
    assert_eq!(
        evidence.staged_prover_common_a_xof_byte_budget,
        12_000_000_000
    );
    assert_eq!(
        evidence.staged_prover_first_candidate_common_a_xof_bytes,
        9_642_704_896
    );
    assert_eq!(evidence.staged_prover_common_a_prepare_validation_passes, 1);
    assert_eq!(evidence.staged_prover_common_a_limb_derivations, 9_196);
    assert_eq!(evidence.staged_prover_common_a_frame_work_units, 1_452_968);
    assert_eq!(
        evidence.staged_prover_immutable_object_scan_work_units,
        9_901_180_029
    );
    assert_eq!(
        evidence.staged_prover_transcript_hash_work_units,
        33_749_511_664
    );
    assert_eq!(evidence.staged_prover_response_work_units, 13_278_880_301);
    assert_eq!(
        evidence.staged_prover_semantic_replay_work_units,
        1_539_047_424
    );
    assert_eq!(evidence.staged_prover_total_work_units, 97_255_811_806);
    assert!(evidence.staged_prover_work_ceiling_met);
    assert_eq!(evidence.staged_prover_party_b_read_passes, 122);
    assert_eq!(evidence.staged_prover_share_immutable_read_passes, 124);
    assert_eq!(evidence.staged_prover_proof_immutable_read_passes, 3);
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
fn compact_authority_source_surface_is_move_only_ordered_and_fail_closed() {
    let persistent = include_str!("persistent_decryption_equality.rs");
    let cpk = include_str!("cpk_relation.rs");
    let collective = include_str!("collective.rs");
    let streaming = include_str!("decryption_streaming.rs");

    // The ceremony surface stays internal and monotonic: one exact contribution
    // and one borrowed share enter each step, while the builder owns only one
    // aggregate and poisons itself before every fallible/backend transition.
    assert!(
        persistent.contains("pub(super) struct ZkAmsMkheStreamingDecryptionAuthorityBuilderV1")
    );
    assert!(persistent.contains("contribution: VerifiedZkAmsMkheCpkContributionV1"));
    assert!(persistent.contains("share: &ZkAmsMkheCollectivePublicKeyShareV1"));
    assert!(persistent.contains("let party_index = self.next_party_index;"));
    assert!(persistent.contains("self.failed = true;"));
    assert!(persistent.contains("if result.is_ok()"));
    assert!(persistent.contains("self.party_b_pointers.contains(&expected_pointer)"));
    assert!(persistent.contains("drop(self.aggregate_b);"));

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

    // The only public bounded constructor consumes the move-only permit. The
    // retained compatibility bridge accepts a fully validated native statement,
    // never raw hashes or pointers.
    assert!(streaming.contains("pub fn from_verified_cpk_authority_v1("));
    assert!(streaming.contains("authority: ZkAmsMkheStreamingDecryptionAuthorityV1"));
    assert!(streaming.contains("pub fn from_native_reference_v1("));
    assert!(!streaming.contains("pub fn from_raw"));
    assert!(!persistent.contains("impl Clone for ZkAmsMkheStreamingDecryptionAuthorityV1"));

    // Explicit branches reject pointer/content/provider/snapshot/publication
    // splices and rebind the permit to the exact roster and ciphertext.
    for required in [
        "verification_snapshot.pointer() != expected_pointer",
        "publication_receipt.pointer() != expected_pointer",
        "expected_pointer.payload_blake3() != payload_blake3",
        "verification_read_receipt.payload_blake3() != expected_pointer.payload_blake3()",
        "verification_snapshot.provider_identity() != publication_snapshot.provider_identity()",
        "verification_snapshot.snapshot_identity() != publication_snapshot.snapshot_identity()",
        "expected_publication_identity",
        "self.roster.to_wire_roster()? != *roster",
        "decryption_wire_ciphertext_digest_v1(&profile, roster, ciphertext)?",
        "authority.context_authority_digest != streaming.authority_digest",
    ] {
        assert!(
            persistent.contains(required),
            "missing fail-closed branch: {required}"
        );
    }
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
    assert_eq!(observed, bytes);
    assert_eq!(receipt.canonical_bytes(), pointer.payload_bytes());
    assert_eq!(receipt.payload_blake3(), pointer.payload_blake3());

    let mut short = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        bytes.clone(),
    );
    short.short_read_at = Some(1);
    let short_pointer = short.pointer;
    assert_eq!(
        read_complete_object_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
            short_pointer,
            &mut short,
        ),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    );

    let mut drift = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        bytes.clone(),
    );
    drift.drift_snapshot_at = Some(3);
    let drift_pointer = drift.pointer;
    assert_eq!(
        read_complete_object_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
            drift_pointer,
            &mut drift,
        ),
        Err(ZkAmsMkheErrorV1::InvalidKeyMaterial)
    );

    let mut mutation = TestProvider::new(
        ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
        bytes.clone(),
    );
    mutation.mutate_payload_at = Some(2);
    let mutation_pointer = mutation.pointer;
    assert_eq!(
        read_complete_object_v1(
            ZkAmsMkheDirectObjectKindV1::DecryptionRelationProof,
            mutation_pointer,
            &mut mutation,
        ),
        Err(ZkAmsMkheErrorV1::InvalidWireEncoding)
    );

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
    let mut native_wide = super::super::ZeroizingSignedWideVectorV1::with_capacity(degree)
        .expect("wide mask");
    for _ in 0..degree {
        native_wide.push(
            sample_signed_wide(&wide_bound, &mut native_random).expect("wide mask draw"),
        );
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
        .map(|index| {
            staged_small_response_v1(secret_mask[index], &terms, &witness_secret, index)
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("secret responses");
    let error_response = (0..degree)
        .map(|index| staged_small_response_v1(error_mask[index], &terms, &witness_error, index))
        .collect::<Result<Vec<_>, _>>()
        .expect("error responses");
    let smudge_response = (0..degree)
        .map(|index| {
            staged_wide_response_v1(&smudge_mask[index], &terms, &witness_smudge, index)
        })
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
        super::super::update_rns_hash(&mut native, &profile, polynomial)
            .expect("native RNS frame");
    }

    let mut staged = initialize_decryption_challenge_transcript(&profile, 13, &binding)
        .expect("staged prefix");
    for polynomial in &polynomials {
        update_streamed_rns_header(&mut staged, &profile).expect("staged RNS header");
        for limb in 0..profile.moduli.len() {
            update_residue_limb(&mut staged, polynomial.limb(&profile, limb));
        }
    }
    assert_eq!(staged.finalize(), native.finalize());
}

#[test]
fn staged_random_budget_accepts_boundary_and_rejects_one_over_or_source_error() {
    struct RecordingRandom {
        forwarded: u64,
        fail: bool,
    }

    impl MaskedRelaxedRandomSourceV1 for RecordingRandom {
        fn fill_bytes(
            &mut self,
            destination: &mut [u8],
        ) -> Result<(), MaskedRelaxedRandomErrorV1> {
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
    assert_eq!(source.forwarded, 3, "one-over request must not be forwarded");

    let mut source = RecordingRandom {
        forwarded: 0,
        fail: true,
    };
    let mut bounded = StagedProverBudgetedRandomSourceV1::new(&mut source, 4).expect("budget");
    assert_eq!(
        bounded.fill_bytes(&mut [0_u8; 2]),
        Err(MaskedRelaxedRandomErrorV1::Unavailable)
    );
    assert_eq!(bounded.remaining_bytes, 2, "failed source draw stays charged");
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
        fn fill_bytes(
            &mut self,
            destination: &mut [u8],
        ) -> Result<(), MaskedRelaxedRandomErrorV1> {
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
    assert!(staged.contains("let common_a_context = statement.prepare_common_a_context(&profile)?;"));
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

    let preflight = staged
        .find("zk_ams_mkhe_decryption_streaming_residency_evidence_v1()?")
        .expect("streaming evidence preflight");
    let prepare_common_a = staged
        .find("statement.prepare_common_a_context(&profile)?")
        .expect("prepared common-a");
    let consume = staged
        .find("statement.consume_party_use_v1(")
        .expect("consume");
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
    assert!(semantic_replay < authentication);

    assert!(persistent.contains("bind_streaming_statement_party_uses_v1("));
    assert!(persistent.contains("consume_streaming_party_use_v1("));
    assert!(persistent.contains("validate_streaming_party_state_axes_v1("));
    assert!(persistent.contains("party_use != expected"));
    assert!(streaming.contains("super::ZeroizingI64VectorV1::with_capacity"));
    assert!(streaming.contains("ZeroizingStagedU64VectorV1::with_capacity"));
    assert!(streaming.contains("drop(secret_mask);"));
    assert!(streaming.contains("drop(smudge);"));
    assert!(common_a.contains(
        "pub(in super::super) struct ZkAmsMkhePreparedCollectivePublicAContextV1"
    ));
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
                _ => u64::try_from(index)
                    .expect("release index")
                    .wrapping_mul(0x9e37_79b9)
                    .wrapping_add(u64::try_from(limb).expect("release limb") + 17)
                    % modulus,
            })
            .collect::<Vec<_>>();
        let right = (0..profile.ring_degree)
            .map(|index| match index % 7 {
                0 => modulus - 1,
                1 => 0,
                2 => 1,
                _ => u64::try_from(index)
                    .expect("release index")
                    .wrapping_mul(0x85eb_ca6b)
                    .wrapping_add(u64::try_from(limb).expect("release limb") + 31)
                    % modulus,
            })
            .collect::<Vec<_>>();
        let native = negacyclic_multiply(
            &left,
            &right,
            modulus,
            profile.negacyclic_roots[limb],
        )
        .expect("native release NTT");
        let staged = negacyclic_multiply_staged_v1(
            &left,
            &right,
            modulus,
            profile.negacyclic_roots[limb],
        )
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
