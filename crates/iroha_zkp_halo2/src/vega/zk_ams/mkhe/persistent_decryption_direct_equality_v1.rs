//! Sealed direct-equality soundness prerequisite for persistent decryption.
//!
//! The existing `persistent_decryption_response_link` is auxiliary and
//! non-authorizing. Its T256 projection can check a useful consistency
//! relation, but it cannot establish that the same wide integer witness is
//! used by every RNS share equation. In particular, neither evaluation at one
//! field point nor cancellation by a sparse challenge is sound in a
//! negacyclic quotient ring with zero divisors.
//!
//! The direct language frozen here would instead require each existing
//! commitment to open as `Cs_j = <s[j*16384..(j+1)*16384], G> + r_j H`,
//! constrain the assembled polynomial `s` to be ternary, and prove all 38
//! equations directly:
//!
//! ```text
//! share_l = c1_l * s + (pT mod q_l) * (z mod q_l) mod q_l,
//! ```
//!
//! where multiplication is the full negacyclic convolution and `z` is one
//! signed integer polynomial, shared across the limbs, with each coefficient
//! bounded by `2^1855 - 1`. There is no `D` inversion or cyclotomic
//! cancellation step. This module defines no prover, verifier, proof codec, or
//! receipt, and it does not close persistent-decryption audit bit 7.

#![allow(
    dead_code,
    reason = "direct-equality production capabilities are deliberately uninhabited"
)]

use core::convert::Infallible;

use super::super::{ZkAmsMkhePartyIdV1, manifest::RELEASE_MODULI_V1};
use crate::vega::{VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256PointV1 as Point};

const DIRECT_EQUALITY_VERSION_V1: u8 = 1;
const DIRECT_EQUALITY_PARTIES_V1: usize = 8;
const DIRECT_EQUALITY_SECRET_COMMITMENTS_V1: usize = 8;
const DIRECT_EQUALITY_CHUNK_COEFFICIENTS_V1: usize = 16_384;
const DIRECT_EQUALITY_RING_DEGREE_V1: usize = 131_072;
const DIRECT_EQUALITY_RNS_LIMBS_V1: usize = 38;
const DIRECT_EQUALITY_SECRET_MIN_V1: i8 = -1;
const DIRECT_EQUALITY_SECRET_MAX_V1: i8 = 1;
const DIRECT_EQUALITY_WIDE_Z_MAGNITUDE_BITS_V1: u16 = 1_855;

const DIRECT_EQUALITY_LANGUAGE_V1: &[u8] = b"R=Z[X]/(X^131072+1);for j=0..7 the existing T256 Cs_j=<s[j*16384..(j+1)*16384],G>+r_j*H under the bound generator basis;the assembled s is one ternary polynomial with s[k] in {-1,0,1};one signed integer z has |z[k]|<=2^1855-1;for every ordered release limb l=0..37:share_l=negacyclic(c1_l,s)+(pT mod q_l)*(z mod q_l) mod q_l;the same s and z are used in all limbs;no D inversion;no cyclotomic cancellation";
const RESPONSE_LINK_STATUS_V1: &[u8] = b"persistent_decryption_response_link is auxiliary and non-authorizing;its signed-small T256 projection does not prove the same approximately-1855-bit z across 38 RNS limbs";

const PARENT_BEFORE_DECLARATION_SHA256_V1: &str =
    "370c605f7d740f1b91310942999ab2690d1c29f21496d53d7090cc0130e3e64d";
const AUXILIARY_RESPONSE_LINK_SHA256_V1: &str =
    "cdc5aabf77ed20abf402b3921d73471fb315bd77b21466eb95880cfa84d97530";
const AUXILIARY_RESPONSE_LINK_TESTS_SHA256_V1: &str =
    "03e59fb4bd35ca3de976e93da598d3a392926ff01ea9b6b3005dad65b69d6efd";

const SHORTCUT_FIELD_MODULUS_V1: i64 = 17;
const SHORTCUT_RING_DEGREE_V1: usize = 4;
const SHORTCUT_D_V1: [i64; SHORTCUT_RING_DEGREE_V1] = [1, 1, 0, 0];
const SHORTCUT_D_PRIME_V1: [i64; SHORTCUT_RING_DEGREE_V1] = [-1, 0, 1, 0];
const SHORTCUT_DIFFERENCE_V1: [i64; SHORTCUT_RING_DEGREE_V1] = [2, 1, -1, 0];
const SHORTCUT_ANNIHILATOR_V1: [i64; SHORTCUT_RING_DEGREE_V1] = [13, 15, 16, 8];
const SHORTCUT_INTEGER_PRODUCT_V1: [i64; SHORTCUT_RING_DEGREE_V1] = [34, 51, 34, 17];
const SHORTCUT_EVALUATION_POINT_V1: i64 = 2;
const SHORTCUT_COMMON_EVALUATION_V1: i64 = 3;
const RESPONSE_LINK_AUTHORIZES_DIRECT_EQUALITY_V1: bool = false;
const D_INVERSION_PERMITTED_V1: bool = false;
const CYCLOTOMIC_CANCELLATION_PERMITTED_V1: bool = false;

const WORKER_HEAP_CAP_BYTES_V1: u64 = 167_772_160;
const DIRECT_PROOF_CAP_BYTES_V1: u64 = 33_554_432;
const EXISTING_MANIFEST_BYTES_V1: u64 = 498;
const DIRECT_OBJECT_POINTER_BYTES_V1: u64 = 42;
const FUTURE_DIRECT_POINTER_ORDINAL_V1: u8 = 3;
const FUTURE_DIRECT_MANIFEST_BYTES_V1: u64 = 540;
const EXISTING_PROVER_PEAK_BYTES_V1: u64 = 77_707_146;
const PROVER_REMAINING_BYTES_V1: u64 = 90_065_014;
const EXISTING_VERIFIER_PEAK_BYTES_V1: u64 = 77_317_655;
const VERIFIER_REMAINING_BYTES_V1: u64 = 90_454_505;
const RELEASE_WORK_CAP_V1: u64 = 100_000_000_000;
const EXISTING_PROVER_WORK_V1: u64 = 69_492_485_649;
const REMAINING_WORK_V1: u64 = 30_507_514_351;
const ONE_PARTY_MAX_TOTAL_BYTES_V1: u64 = 106_431_059;
const SEQUENTIAL_NONCOEXISTENCE_REQUIRED_V1: bool = true;
const SEQUENTIAL_NONCOEXISTENCE_CONTRACT_V1: &[u8] = b"the direct-equality backend stage and the existing staged-decryption prover peak execute sequentially;the 106431059-byte one-party maximum does not authorize co-residency";

struct DirectEqualityCapLedgerV1 {
    direct_proof_cap_bytes: u64,
    future_manifest_bytes: u64,
    future_pointer_ordinal: u8,
    future_pointer_bytes: u64,
    prover_existing_bytes: u64,
    prover_remaining_bytes: u64,
    verifier_existing_bytes: u64,
    verifier_remaining_bytes: u64,
    remaining_work: u64,
    one_party_max_total_bytes: u64,
    sequential_noncoexistence_required: bool,
}

const DIRECT_EQUALITY_CAP_LEDGER_V1: DirectEqualityCapLedgerV1 = DirectEqualityCapLedgerV1 {
    direct_proof_cap_bytes: DIRECT_PROOF_CAP_BYTES_V1,
    future_manifest_bytes: FUTURE_DIRECT_MANIFEST_BYTES_V1,
    future_pointer_ordinal: FUTURE_DIRECT_POINTER_ORDINAL_V1,
    future_pointer_bytes: DIRECT_OBJECT_POINTER_BYTES_V1,
    prover_existing_bytes: EXISTING_PROVER_PEAK_BYTES_V1,
    prover_remaining_bytes: PROVER_REMAINING_BYTES_V1,
    verifier_existing_bytes: EXISTING_VERIFIER_PEAK_BYTES_V1,
    verifier_remaining_bytes: VERIFIER_REMAINING_BYTES_V1,
    remaining_work: REMAINING_WORK_V1,
    one_party_max_total_bytes: ONE_PARTY_MAX_TOTAL_BYTES_V1,
    sequential_noncoexistence_required: SEQUENTIAL_NONCOEXISTENCE_REQUIRED_V1,
};

const FALLBACK_PER_ROUND_ENVELOPE_BYTES_V1: u64 = 33_032_907;
const FALLBACK_PROOF_CAP_MARGIN_BYTES_V1: u64 = 521_525;
const FALLBACK_MANIFEST_FIXED_BYTES_V1: u64 = 456;
const FALLBACK_THREE_ROUNDS_V1: u8 = 3;
const FALLBACK_NINE_ROUNDS_V1: u8 = 9;
const FALLBACK_THREE_ROUND_BYTES_V1: u64 = 99_098_721;
const FALLBACK_NINE_ROUND_BYTES_V1: u64 = 297_296_163;
const FALLBACK_THREE_ROUND_MANIFEST_BYTES_V1: u64 = 582;
const FALLBACK_NINE_ROUND_MANIFEST_BYTES_V1: u64 = 834;
const FALLBACK_PER_ROUND_WORK_V1: u64 = 69_492_485_649;
const FALLBACK_THREE_ROUND_WORK_V1: u64 = 208_477_456_947;
const FALLBACK_NINE_ROUND_WORK_V1: u64 = 625_432_370_841;
const FALLBACK_THREE_ROUND_SHARED_MIN_WORK_V1: u64 = 116_177_911_296;
const FALLBACK_COORDINATES_V1: u64 = 4_980_736;
const FALLBACK_GRINDING_ATTEMPTS_V1: u16 = 120;
const FALLBACK_FINAL_CHOICES_V1: u64 = 262_106;
const FALLBACK_PER_ROUND_BITS_HUNDREDTHS_V1: u32 = 1_800;
const FALLBACK_SOUNDNESS_LOSS_HUNDREDTHS_V1: u32 = 2_916;
const FALLBACK_EIGHT_ROUND_BITS_HUNDREDTHS_V1: u32 = 11_484;
const FALLBACK_NINE_ROUND_BITS_HUNDREDTHS_V1: u32 = 13_284;
const FALLBACK_TARGET_BITS_HUNDREDTHS_V1: u32 = 12_800;
const FALLBACK_EIGHT_ROUNDS_SUFFICIENT_V1: bool = false;
const FALLBACK_NINE_ROUNDS_SUFFICIENT_V1: bool = true;

struct RepetitionFallbackLedgerV1 {
    per_round_envelope_bytes: u64,
    three_round_bytes: u64,
    three_round_manifest_bytes: u64,
    three_round_work: u64,
    nine_round_bytes: u64,
    nine_round_manifest_bytes: u64,
    nine_round_work: u64,
    three_round_shared_min_work: u64,
    coordinates: u64,
    grinding_attempts: u16,
    final_choices: u64,
    eight_round_bits_hundredths: u32,
    nine_round_bits_hundredths: u32,
}

const REPETITION_FALLBACK_LEDGER_V1: RepetitionFallbackLedgerV1 = RepetitionFallbackLedgerV1 {
    per_round_envelope_bytes: FALLBACK_PER_ROUND_ENVELOPE_BYTES_V1,
    three_round_bytes: FALLBACK_THREE_ROUND_BYTES_V1,
    three_round_manifest_bytes: FALLBACK_THREE_ROUND_MANIFEST_BYTES_V1,
    three_round_work: FALLBACK_THREE_ROUND_WORK_V1,
    nine_round_bytes: FALLBACK_NINE_ROUND_BYTES_V1,
    nine_round_manifest_bytes: FALLBACK_NINE_ROUND_MANIFEST_BYTES_V1,
    nine_round_work: FALLBACK_NINE_ROUND_WORK_V1,
    three_round_shared_min_work: FALLBACK_THREE_ROUND_SHARED_MIN_WORK_V1,
    coordinates: FALLBACK_COORDINATES_V1,
    grinding_attempts: FALLBACK_GRINDING_ATTEMPTS_V1,
    final_choices: FALLBACK_FINAL_CHOICES_V1,
    eight_round_bits_hundredths: FALLBACK_EIGHT_ROUND_BITS_HUNDREDTHS_V1,
    nine_round_bits_hundredths: FALLBACK_NINE_ROUND_BITS_HUNDREDTHS_V1,
};

const THEOREM_DIGEST_SLOT_V1: [u8; 32] = [0; 32];
const CIRCUIT_DIGEST_SLOT_V1: [u8; 32] = [0; 32];
const BACKEND_DIGEST_SLOT_V1: [u8; 32] = [0; 32];

const THEOREM_PINNED_V1: bool = false;
const CIRCUIT_PINNED_V1: bool = false;
const BACKEND_IMPLEMENTED_V1: bool = false;
const INTEGRATION_WIRED_V1: bool = false;
const DIRECT_EQUALITY_VERIFIED_V1: bool = false;
const ATOMIC_REPLAY_WIRED_V1: bool = false;
const VERIFIED_RECEIPT_CONSUMED_V1: bool = false;
const PRODUCTION_KAT_QUALIFIED_V1: bool = false;
const PRODUCTION_RSS_QUALIFIED_V1: bool = false;
const ZERO_KNOWLEDGE_ACCEPTED_V1: bool = false;
const PERSISTENT_DECRYPTION_AUDIT_BIT_7_CLOSED_V1: bool = false;
const RELEASE_READY_V1: bool = false;

struct DirectEqualityStatementAxesV1 {
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    binding_set_root: [u8; 32],
    collective_public_key_digest: [u8; 32],
    key_context_digest: [u8; 32],
    cpk_transcript_digest: [u8; 32],
    public_contribution_set_digest: [u8; 32],
    decryption_statement_digest: [u8; 32],
    ciphertext_digest: [u8; 32],
    persistent_use_digest: [u8; 32],
    secret_identity_digest: [u8; 32],
    generator_basis_digest: [u8; 32],
    commitment_set_digest: [u8; 32],
    commitment_context_digest: [u8; 32],
    persistent_equation_contract_digest: [u8; 32],
    legacy_short_solution_assumption_digest: [u8; 32],
    party: ZkAmsMkhePartyIdV1,
    epoch: u64,
    ciphertext_record_index: u32,
    sample_index: u64,
    party_index: u8,
    level: u8,
}

struct DirectEqualityRnsLimbAxisV1 {
    limb_index: u8,
    modulus: u64,
    plaintext_modulus_residue: u64,
    ciphertext_c1_digest: [u8; 32],
    decryption_share_digest: [u8; 32],
}

struct DirectEqualityArtifactDigestSlotsV1 {
    theorem_digest: [u8; 32],
    circuit_digest: [u8; 32],
    backend_digest: [u8; 32],
}

struct DirectEqualityPrivateWitnessShapeV1 {
    secret_coefficients: usize,
    secret_commitment_blindings: usize,
    secret_min: i8,
    secret_max: i8,
    wide_z_coefficients: usize,
    wide_z_magnitude_bits: u16,
    wide_z_is_signed: bool,
    shared_wide_z_limb_count: usize,
}

const DIRECT_EQUALITY_PRIVATE_WITNESS_SHAPE_V1: DirectEqualityPrivateWitnessShapeV1 =
    DirectEqualityPrivateWitnessShapeV1 {
        secret_coefficients: DIRECT_EQUALITY_RING_DEGREE_V1,
        secret_commitment_blindings: DIRECT_EQUALITY_SECRET_COMMITMENTS_V1,
        secret_min: DIRECT_EQUALITY_SECRET_MIN_V1,
        secret_max: DIRECT_EQUALITY_SECRET_MAX_V1,
        wide_z_coefficients: DIRECT_EQUALITY_RING_DEGREE_V1,
        wide_z_magnitude_bits: DIRECT_EQUALITY_WIDE_Z_MAGNITUDE_BITS_V1,
        wide_z_is_signed: true,
        shared_wide_z_limb_count: DIRECT_EQUALITY_RNS_LIMBS_V1,
    };

const UNPINNED_ARTIFACT_DIGEST_SLOTS_V1: DirectEqualityArtifactDigestSlotsV1 =
    DirectEqualityArtifactDigestSlotsV1 {
        theorem_digest: THEOREM_DIGEST_SLOT_V1,
        circuit_digest: CIRCUIT_DIGEST_SLOT_V1,
        backend_digest: BACKEND_DIGEST_SLOT_V1,
    };

enum DirectEqualityWitnessSealV1 {
    Production {
        retained_persistent_openings: Infallible,
        exact_ternary_secret: Infallible,
        one_signed_wide_z_across_all_limbs: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

enum DirectEqualityBackendSealV1 {
    Production {
        theorem_digest_pinned: Infallible,
        circuit_digest_pinned: Infallible,
        backend_digest_pinned: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

enum DirectEqualityIntegrationSealV1 {
    Production {
        state_hook: Infallible,
        atomic_semantic_replay: Infallible,
        receipt_consumer: Infallible,
    },
    #[cfg(test)]
    TestOnly,
}

struct PersistentDecryptionDirectEqualityLanguageV1 {
    axes: DirectEqualityStatementAxesV1,
    persistent_secret_commitments: [Point; DIRECT_EQUALITY_SECRET_COMMITMENTS_V1],
    rns_limbs: [DirectEqualityRnsLimbAxisV1; DIRECT_EQUALITY_RNS_LIMBS_V1],
    artifacts: DirectEqualityArtifactDigestSlotsV1,
    witness_shape: DirectEqualityPrivateWitnessShapeV1,
    witness_seal: DirectEqualityWitnessSealV1,
    backend_seal: DirectEqualityBackendSealV1,
    integration_seal: DirectEqualityIntegrationSealV1,
}

fn t256_residue_v1(modulus: u64) -> u64 {
    VEGA_T256_SCALAR_MODULUS_BE_V1
        .iter()
        .fold(0_u64, |acc, byte| {
            ((u128::from(acc) * 256 + u128::from(*byte)) % u128::from(modulus)) as u64
        })
}

impl PersistentDecryptionDirectEqualityLanguageV1 {
    fn has_exact_frozen_shape_v1(&self) -> bool {
        let axes = &self.axes;
        let digests = [
            axes.profile_digest,
            axes.roster_digest,
            axes.binding_set_root,
            axes.collective_public_key_digest,
            axes.key_context_digest,
            axes.cpk_transcript_digest,
            axes.public_contribution_set_digest,
            axes.decryption_statement_digest,
            axes.ciphertext_digest,
            axes.persistent_use_digest,
            axes.secret_identity_digest,
            axes.generator_basis_digest,
            axes.commitment_set_digest,
            axes.commitment_context_digest,
            axes.persistent_equation_contract_digest,
            axes.legacy_short_solution_assumption_digest,
            self.artifacts.theorem_digest,
            self.artifacts.circuit_digest,
            self.artifacts.backend_digest,
        ];
        if digests.contains(&[0; 32])
            || axes.epoch == 0
            || usize::from(axes.party_index) >= DIRECT_EQUALITY_PARTIES_V1
            || axes.level > 1
            || self.witness_shape.secret_coefficients != DIRECT_EQUALITY_RING_DEGREE_V1
            || self.witness_shape.secret_commitment_blindings
                != DIRECT_EQUALITY_SECRET_COMMITMENTS_V1
            || self.witness_shape.secret_min != DIRECT_EQUALITY_SECRET_MIN_V1
            || self.witness_shape.secret_max != DIRECT_EQUALITY_SECRET_MAX_V1
            || self.witness_shape.wide_z_coefficients != DIRECT_EQUALITY_RING_DEGREE_V1
            || self.witness_shape.wide_z_magnitude_bits != DIRECT_EQUALITY_WIDE_Z_MAGNITUDE_BITS_V1
            || !self.witness_shape.wide_z_is_signed
            || self.witness_shape.shared_wide_z_limb_count != DIRECT_EQUALITY_RNS_LIMBS_V1
            || self
                .persistent_secret_commitments
                .iter()
                .copied()
                .any(Point::is_identity)
        {
            return false;
        }
        self.rns_limbs.iter().enumerate().all(|(index, limb)| {
            usize::from(limb.limb_index) == index
                && limb.modulus == RELEASE_MODULI_V1[index]
                && limb.plaintext_modulus_residue == t256_residue_v1(limb.modulus)
                && limb.ciphertext_c1_digest != [0; 32]
                && limb.decryption_share_digest != [0; 32]
        })
    }
}

const _: () = {
    assert!(DIRECT_EQUALITY_VERSION_V1 == 1);
    assert!(
        DIRECT_EQUALITY_RING_DEGREE_V1
            == DIRECT_EQUALITY_SECRET_COMMITMENTS_V1 * DIRECT_EQUALITY_CHUNK_COEFFICIENTS_V1
    );
    assert!(DIRECT_EQUALITY_RNS_LIMBS_V1 == RELEASE_MODULI_V1.len());
    assert!(DIRECT_EQUALITY_SECRET_MIN_V1 == -1 && DIRECT_EQUALITY_SECRET_MAX_V1 == 1);
    assert!(DIRECT_EQUALITY_WIDE_Z_MAGNITUDE_BITS_V1 == 1_855);
    assert!(FUTURE_DIRECT_MANIFEST_BYTES_V1 == EXISTING_MANIFEST_BYTES_V1 + 42);
    assert!(WORKER_HEAP_CAP_BYTES_V1 == EXISTING_PROVER_PEAK_BYTES_V1 + PROVER_REMAINING_BYTES_V1);
    assert!(
        WORKER_HEAP_CAP_BYTES_V1 == EXISTING_VERIFIER_PEAK_BYTES_V1 + VERIFIER_REMAINING_BYTES_V1
    );
    assert!(RELEASE_WORK_CAP_V1 == EXISTING_PROVER_WORK_V1 + REMAINING_WORK_V1);
    assert!(ONE_PARTY_MAX_TOTAL_BYTES_V1 <= WORKER_HEAP_CAP_BYTES_V1);
    assert!(SEQUENTIAL_NONCOEXISTENCE_REQUIRED_V1);
    assert!(
        FALLBACK_PER_ROUND_ENVELOPE_BYTES_V1 + FALLBACK_PROOF_CAP_MARGIN_BYTES_V1
            == DIRECT_PROOF_CAP_BYTES_V1
    );
    assert!(
        FALLBACK_THREE_ROUND_BYTES_V1
            == FALLBACK_PER_ROUND_ENVELOPE_BYTES_V1 * FALLBACK_THREE_ROUNDS_V1 as u64
    );
    assert!(
        FALLBACK_NINE_ROUND_BYTES_V1
            == FALLBACK_PER_ROUND_ENVELOPE_BYTES_V1 * FALLBACK_NINE_ROUNDS_V1 as u64
    );
    assert!(
        FALLBACK_THREE_ROUND_MANIFEST_BYTES_V1
            == FALLBACK_MANIFEST_FIXED_BYTES_V1
                + DIRECT_OBJECT_POINTER_BYTES_V1 * FALLBACK_THREE_ROUNDS_V1 as u64
    );
    assert!(
        FALLBACK_NINE_ROUND_MANIFEST_BYTES_V1
            == FALLBACK_MANIFEST_FIXED_BYTES_V1
                + DIRECT_OBJECT_POINTER_BYTES_V1 * FALLBACK_NINE_ROUNDS_V1 as u64
    );
    assert!(
        FALLBACK_THREE_ROUND_WORK_V1
            == FALLBACK_PER_ROUND_WORK_V1 * FALLBACK_THREE_ROUNDS_V1 as u64
    );
    assert!(
        FALLBACK_NINE_ROUND_WORK_V1 == FALLBACK_PER_ROUND_WORK_V1 * FALLBACK_NINE_ROUNDS_V1 as u64
    );
    assert!(FALLBACK_COORDINATES_V1 == 38 * 131_072);
    assert!(FALLBACK_FINAL_CHOICES_V1 == 2 * 131_072 - 38);
    assert!(FALLBACK_THREE_ROUND_SHARED_MIN_WORK_V1 > RELEASE_WORK_CAP_V1);
    assert!(
        FALLBACK_EIGHT_ROUND_BITS_HUNDREDTHS_V1
            == 8 * FALLBACK_PER_ROUND_BITS_HUNDREDTHS_V1 - FALLBACK_SOUNDNESS_LOSS_HUNDREDTHS_V1
    );
    assert!(
        FALLBACK_NINE_ROUND_BITS_HUNDREDTHS_V1
            == 9 * FALLBACK_PER_ROUND_BITS_HUNDREDTHS_V1 - FALLBACK_SOUNDNESS_LOSS_HUNDREDTHS_V1
    );
    assert!(FALLBACK_EIGHT_ROUND_BITS_HUNDREDTHS_V1 < FALLBACK_TARGET_BITS_HUNDREDTHS_V1);
    assert!(FALLBACK_NINE_ROUND_BITS_HUNDREDTHS_V1 >= FALLBACK_TARGET_BITS_HUNDREDTHS_V1);
    assert!(!FALLBACK_EIGHT_ROUNDS_SUFFICIENT_V1 && FALLBACK_NINE_ROUNDS_SUFFICIENT_V1);
    assert!(!RESPONSE_LINK_AUTHORIZES_DIRECT_EQUALITY_V1);
    assert!(!D_INVERSION_PERMITTED_V1 && !CYCLOTOMIC_CANCELLATION_PERMITTED_V1);
    assert!(!THEOREM_PINNED_V1 && !CIRCUIT_PINNED_V1 && !BACKEND_IMPLEMENTED_V1);
    assert!(!INTEGRATION_WIRED_V1 && !DIRECT_EQUALITY_VERIFIED_V1);
    assert!(!ATOMIC_REPLAY_WIRED_V1 && !VERIFIED_RECEIPT_CONSUMED_V1);
    assert!(!PRODUCTION_KAT_QUALIFIED_V1 && !PRODUCTION_RSS_QUALIFIED_V1);
    assert!(!ZERO_KNOWLEDGE_ACCEPTED_V1 && !PERSISTENT_DECRYPTION_AUDIT_BIT_7_CLOSED_V1);
    assert!(!RELEASE_READY_V1);
};

// TODO: Replace the `Infallible` seals only after independent theorem,
// circuit, backend, integration, replay, receipt, KAT, RSS, and ZK evidence is
// pinned and jointly re-audited under the release caps above.

#[cfg(test)]
#[path = "persistent_decryption_direct_equality_v1_tests.rs"]
mod tests;
