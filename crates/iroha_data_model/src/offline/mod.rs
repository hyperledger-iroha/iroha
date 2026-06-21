//! Offline note models.
//!
//! Offline is the first production offline note surface. The legacy
//! allowance, witness-lineage, plaintext receipt, and aggregate proof models are
//! intentionally absent from this module.

use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_data_model_derive::model;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};
use sha2::{Digest as _, Sha256};

pub use self::model::*;
use crate::{
    ChainId,
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    proof::{ProofAttachment, ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord},
    zk::BackendTag,
};

/// Prefix embedded into offline instruction rejection messages.
///
/// Mobile SDKs parse the label after this prefix up to the first `:` to recover
/// stable machine-readable error codes.
pub const OFFLINE_REJECTION_REASON_PREFIX: &str = "offline_reason::";
/// Asset-definition metadata key that enables Offline escrow tracking.
pub const OFFLINE_ASSET_ENABLED_METADATA_KEY: &str = "offline.enabled";
/// Domain-separation tag for deterministic offline escrow derivation.
pub const OFFLINE_ESCROW_SEED_LABEL: &str = "iroha.offline.escrow";
/// Canonical Offline key-certificate format marker for the first release.
pub const OFFLINE_NOTE_KEY_CERTIFICATE_VERSION: u16 = 1;
/// Domain-separation tag for wallet-derived Offline Note note commitments.
pub const OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN: &str = "iroha:offline-note:note-commitment";
/// Domain-separation tag for wallet-derived Offline Note input nullifiers.
pub const OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN: &str = "iroha:offline-note:input-nullifier";
/// Domain-separation tag for wallet-derived Offline Note payment token identifiers.
pub const OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN: &str = "iroha:offline-note:payment-token-id";
/// Domain-separation tag for compact Kagemusha folded-proof public inputs.
pub const KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN: &str = "iroha:kagemusha:v1:folded-public-inputs";
/// Domain-separation tag for reserved Kagemusha recursive aggregation evidence.
pub const KAGEMUSHA_RECURSIVE_AGGREGATION_EVIDENCE_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-aggregation-evidence";
/// Domain-separation tag for recursive aggregation proof public inputs.
pub const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-aggregation-proof-public-inputs";
/// Domain-separation tag for spendable recursive Kagemusha accumulator state.
pub const KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-accumulator";
/// Domain-separation tag for spendable recursive Kagemusha accumulator digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-accumulator-digest";
/// Domain-separation tag for streaming recursive Kagemusha lineage updates.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-lineage";
/// Domain-separation tag for streaming recursive Kagemusha verifier-batch updates.
pub const KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_BATCH_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-verifier-batch";
/// Domain-separation tag for streaming recursive Kagemusha fixed-window table-base updates.
pub const KAGEMUSHA_RECURSIVE_SPEND_FIXED_WINDOW_TABLE_BASE_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-fixed-window-table-base";
/// Domain-separation tag for recursive Kagemusha proof artifact digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_ARTIFACT_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-proof-artifact";
/// Domain-separation tag for streaming recursive Kagemusha proof-chain updates.
pub const KAGEMUSHA_RECURSIVE_SPEND_PROOF_CHAIN_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-proof-chain";
/// Domain-separation tag for reserved recursive Kagemusha transition profiles.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-transition-profile";
/// Domain-separation tag for reserved recursive Kagemusha transition profile digests.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-transition-profile-digest";
/// Domain-separation tag for the non-circular transition-profile binding digest.
///
/// The digest is computed from the canonical transition profile with optional
/// verifier-opening material plus the self-referential resulting accumulator
/// and public-input hashes blanked, so recursive spend accumulators and proof
/// public inputs can bind transition semantics without requiring a hash fixed
/// point or duplicating append-only opening archives.
pub const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest";
/// Canonical verifier-witness profile for reserved Kagemusha recursive aggregation evidence.
pub const KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1: &str =
    "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-85x3";
/// Canonical circuit id for proof-carrying Kagemusha recursive aggregation evidence.
pub const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-aggregation-v1";
/// Canonical circuit id for compact tokens with in-circuit recursive aggregation.
pub const KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1: &str = "kagemusha-recursive-compact-v1";
/// Reserved chain-admission circuit id for lineage-proving recursive spend redemption.
///
/// This is the legacy family selector accepted by ABI helpers. Production
/// verifier records use the profile-specific one-hop or append ids below so
/// both keys can coexist in the verifier registry.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-spend-lineage-v1";
/// Profile-specific Reserved-lineage circuit id for the first offline hop.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-spend-lineage-onehop-v1";
/// Profile-specific Reserved-lineage circuit id for append proofs after hop one.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1: &str =
    "kagemusha-recursive-spend-lineage-append-v1";
/// Maximum hop count admitted for witnessless Reserved-lineage redemption.
///
/// The bound matches the compact-token hop cap so recursive spend-again-offline
/// payloads stay constant-size while still preserving a hard replay/latency cap.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1: u32 = 64;
/// Whether the Reserved-lineage circuit proves accumulator transitions in-circuit.
///
/// This remains separate from the max-hop knob so future cap changes cannot
/// accidentally bypass the append verifier-slice transition constraints.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1: bool = true;
/// Number of previous recursive proof opening envelopes required for one append.
pub const KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1: usize = 1;
/// Domain tag mixed into previous recursive proof opening transcripts.
pub const KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPE_DOMAIN_TAG_V1: &str =
    "iroha:kagemusha:previous-recursive-proof-open-envelope:v1";
/// Domain tag for the previous recursive proof opening archive digest bound into append profiles.
pub const KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_ARCHIVE_DIGEST_DOMAIN_V1: &str =
    "iroha:kagemusha:v1:previous-recursive-proof-open-envelopes-archive-digest";
/// Domain tag for the two-opening Reserved-lineage append preflight contract.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1: &str =
    "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1";
/// Domain tag for the compact Reserved-lineage append boundary digest.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1: &str =
    "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1";
/// Domain tag for chain/asset binding inside compact Reserved-lineage append boundaries.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1: &str =
    "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1";
/// Domain tag for final-root/current-note binding inside compact Reserved-lineage append boundaries.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1: &str =
    "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1";
const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND: &str = "halo2/ipa";
/// Minimum Pallas IPA opening length accepted by reserved Kagemusha recursive evidence.
pub const KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MIN_LEN: u32 = 2;
/// Maximum Pallas IPA opening length accepted by reserved Kagemusha recursive evidence.
pub const KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MAX_LEN: u32 = 128;
/// Supported Pallas IPA opening lengths for recursive compact verifier-slice packages.
pub const KAGEMUSHA_RECURSIVE_COMPACT_SUPPORTED_OPENING_LENS_V1: [u32; 7] =
    [2, 4, 8, 16, 32, 64, 128];
/// Maximum transcript label length accepted by Kagemusha Pallas opening archives.
pub const KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES: usize = 128;
/// Current Kagemusha aggregation mode: every private hop proof is verified before folding.
pub const KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1: u16 = 1;
/// Kagemusha aggregation mode for compact tokens whose private-hop verifier is proven in-circuit.
pub const KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1: u16 = 2;
/// SDK-facing Kagemusha spend mode for recursive compact tokens.
///
/// This mode is intentionally not selected by production defaults until the
/// public compact-token proof uses the composed private-hop verifier-slice
/// circuit instead of the standalone semantic aggregation circuit.
pub const KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_COMPACT_V1: &str = "recursive_compact_v1";
/// SDK-facing Kagemusha spend mode for recursive spend-again-offline cash.
pub const KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1: &str = "recursive_spend_v1";
/// SDK-facing Kagemusha spend mode for legacy checked pre-fold compact tokens.
pub const KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1: &str = "checked_prefold_v1";
/// Canonical verifier-record namespace for Kagemusha proof admission.
pub const KAGEMUSHA_VERIFIER_NAMESPACE: &str = "offline_kagemusha";
/// Return `true` when this release accepts the Kagemusha aggregation mode.
#[must_use]
pub const fn is_supported_kagemusha_aggregation_mode(mode: u16) -> bool {
    mode == KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
}

/// Return the default SDK Kagemusha spend mode for the available native surface.
///
/// Recursive spend bundles are the default product path when ABI 6 recursive
/// spend init/append/verify/redeem is available. Checked pre-fold remains the
/// compatibility fallback for runtimes that only link the older record-backed
/// compact-token surface.
#[must_use]
pub const fn preferred_kagemusha_offline_spend_mode(
    recursive_spend_available: bool,
) -> &'static str {
    preferred_kagemusha_offline_spend_mode_for_capabilities(false, recursive_spend_available)
}

/// Return the default SDK Kagemusha spend mode for advertised native capabilities.
///
/// ABI-6 recursive spend remains the production default when available. The
/// `recursive_compact_available` argument is accepted for source compatibility,
/// but recursive compact mode is not auto-selected until compact-token proofs
/// compose the private-hop verifier-slice relation in-circuit.
#[must_use]
pub const fn preferred_kagemusha_offline_spend_mode_for_capabilities(
    _recursive_compact_available: bool,
    recursive_spend_available: bool,
) -> &'static str {
    if recursive_spend_available {
        KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
    } else {
        KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
    }
}

/// Return `true` when a recursive spend bundle can attempt witnessless online redemption.
///
/// Only Reserved-lineage proofs inside the configured hop cap redeem witnesslessly.
#[must_use]
#[allow(clippy::manual_range_contains)]
pub fn can_redeem_kagemusha_recursive_spend_witnessless(
    proof_circuit_id: &str,
    hop_count: u32,
) -> bool {
    let is_reserved_lineage_circuit = proof_circuit_id
        == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
        || proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
        || proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1;
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
        && is_kagemusha_recursive_spend_lineage_proof_circuit_id(proof_circuit_id)
        && is_reserved_lineage_circuit
        && hop_count >= 1
        && hop_count <= KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
}

/// Return `true` when a circuit id is any Reserved-lineage recursive spend profile.
#[must_use]
pub fn is_kagemusha_recursive_spend_lineage_proof_circuit_id(proof_circuit_id: &str) -> bool {
    matches!(
        proof_circuit_id,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            | KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
            | KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
}

/// Return `true` when a circuit id selects a Reserved-lineage append output profile.
#[must_use]
pub fn is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
    output_proof_circuit_id: &str,
) -> bool {
    matches!(
        output_proof_circuit_id,
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            | KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
}

/// Return `true` when redeem construction must carry a record-backed lineage witness.
#[must_use]
pub fn requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
    proof_circuit_id: &str,
    hop_count: u32,
) -> bool {
    !can_redeem_kagemusha_recursive_spend_witnessless(proof_circuit_id, hop_count)
}

/// Return `true` when this release can append another witnessless Reserved-lineage hop.
///
/// Reserved-lineage append output is available for previous hops below the cap.
#[must_use]
#[allow(clippy::manual_range_contains)]
pub fn can_append_kagemusha_recursive_spend_lineage_witnessless(previous_hop_count: u32) -> bool {
    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1
        && previous_hop_count >= 1
        && previous_hop_count < KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
}

/// Return `true` when append proving must carry previous recursive proof openings.
#[must_use]
pub fn requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
    output_proof_circuit_id: &str,
    previous_hop_count: u32,
) -> bool {
    is_kagemusha_recursive_spend_lineage_append_output_circuit_id(
        normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(output_proof_circuit_id),
    ) && previous_hop_count >= 1
}

/// Normalize an append output proof circuit id for legacy ABI-6 compatibility.
#[must_use]
pub fn normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
    output_proof_circuit_id: &str,
) -> &str {
    if output_proof_circuit_id.is_empty() {
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    } else if output_proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 {
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    } else {
        output_proof_circuit_id
    }
}

/// Return `true` when an append output proof circuit id is supported.
#[must_use]
pub fn is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
    output_proof_circuit_id: &str,
) -> bool {
    matches!(
        normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(output_proof_circuit_id),
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            | KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    )
}

/// Return `true` when a previous recursive proof circuit id can be appended.
#[must_use]
pub fn is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
    previous_proof_circuit_id: &str,
) -> bool {
    matches!(
        previous_proof_circuit_id,
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    ) || is_kagemusha_recursive_spend_lineage_proof_circuit_id(previous_proof_circuit_id)
}

/// Return `true` when append requests must include the previous lineage verifier record.
#[must_use]
pub fn requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
    previous_proof_circuit_id: &str,
) -> bool {
    is_kagemusha_recursive_spend_lineage_proof_circuit_id(previous_proof_circuit_id)
}

/// Return `true` when an append proof circuit transition is structurally allowed.
///
/// This helper separates the long-lived transition rule from the current proving
/// capability. Reserved-lineage append output is structurally valid only after a
/// previous Reserved-lineage proof.
#[must_use]
pub fn is_supported_kagemusha_recursive_spend_append_proof_transition(
    previous_proof_circuit_id: &str,
    output_proof_circuit_id: &str,
) -> bool {
    matches!(
        (
            previous_proof_circuit_id,
            normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
                output_proof_circuit_id
            )
        ),
        (
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        )
    ) || (is_kagemusha_recursive_spend_lineage_proof_circuit_id(previous_proof_circuit_id)
        && matches!(
            normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
                output_proof_circuit_id
            ),
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
                | KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        ))
}

/// Return the preferred append output proof circuit for the current release.
///
/// First-hop init uses Reserved-lineage. Real appends keep using Reserved-lineage
/// while the previous hop is below the witnessless lineage cap.
#[must_use]
pub fn preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
    previous_hop_count: u32,
) -> &'static str {
    if can_append_kagemusha_recursive_spend_lineage_witnessless(previous_hop_count) {
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    } else {
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
    }
}

/// Return `true` when this release can actually prove the selected append output.
///
/// Reserved-lineage append is bounded by the witnessless lineage circuit and hard
/// hop cap. Semantic append remains available for legacy checked pre-fold
/// compatibility.
#[must_use]
pub fn can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
    output_proof_circuit_id: &str,
    previous_hop_count: u32,
) -> bool {
    if previous_hop_count == 0 {
        return false;
    }
    match normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
    ) {
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 => {
            usize::try_from(previous_hop_count.saturating_add(1))
                .is_ok_and(|output_hop_count| output_hop_count <= KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS)
        }
        KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 => {
            can_append_kagemusha_recursive_spend_lineage_witnessless(previous_hop_count)
        }
        _ => false,
    }
}

/// Return `true` when an append request may select this output circuit.
///
/// This combines the current proving capability with the previous-proof
/// circuit transition rule. Semantic previous proofs must continue with
/// semantic append output; Reserved-lineage append output is valid only after a
/// previous Reserved-lineage proof and while the previous hop is inside the
/// witnessless append cap.
#[must_use]
pub fn can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
    previous_proof_circuit_id: &str,
    output_proof_circuit_id: &str,
    previous_hop_count: u32,
) -> bool {
    if !can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
        previous_hop_count,
    ) {
        return false;
    }
    is_supported_kagemusha_recursive_spend_append_proof_transition(
        previous_proof_circuit_id,
        output_proof_circuit_id,
    )
}

/// Return the stable rejection reason for an unsupported Kagemusha aggregation mode.
#[must_use]
pub const fn unsupported_kagemusha_aggregation_mode_reason(mode: u16) -> &'static str {
    match mode {
        KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1 => {
            "recursive compact mode requires ABI-7 recursive compact-token admission; the legacy checked pre-fold path does not accept mode 2"
        }
        _ => "unsupported or unknown Kagemusha aggregation mode",
    }
}

/// Return `true` when `backend` is accepted for Kagemusha proof transcript material.
#[must_use]
pub fn is_supported_kagemusha_proof_backend(backend: &str) -> bool {
    if is_trusted_setup_kagemusha_backend(backend) || is_developer_only_kagemusha_backend(backend) {
        return false;
    }
    backend == "halo2/ipa" || crate::zk::is_stark_fri_v1_backend_label(backend)
}

fn is_trusted_setup_kagemusha_backend(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    let backend = backend.as_str();
    has_trusted_setup_kagemusha_backend_segment(backend)
        || has_trusted_setup_kagemusha_backend_compact_label(backend)
        || backend == "groth16"
        || backend.starts_with("groth16/")
        || backend == "kzg"
        || backend.starts_with("kzg/")
        || backend == "bn254"
        || backend == "bn256"
        || backend == "bls12_381"
        || backend == "bls12-381"
        || backend == "halo2/bn254"
        || backend.starts_with("halo2/bn254/")
        || backend.contains("/bn254")
        || backend.contains(":bn254")
        || backend.contains("/bn256")
        || backend.contains(":bn256")
        || backend.contains("/bls12")
        || backend.contains(":bls12")
        || backend == "halo2/kzg"
        || backend.starts_with("halo2/kzg/")
        || backend.contains("/kzg")
        || backend.contains(":kzg")
}

fn has_trusted_setup_kagemusha_backend_segment(backend: &str) -> bool {
    const TRUSTED_SETUP_SEGMENTS: &[&str] = &[
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "powersoftau",
    ];
    backend
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|segment| TRUSTED_SETUP_SEGMENTS.contains(&segment))
}

fn has_trusted_setup_kagemusha_backend_compact_label(backend: &str) -> bool {
    let compact = backend
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    [
        "groth16",
        "kzg",
        "bn254",
        "bn256",
        "bls12381",
        "bls12",
        "srs",
        "crs",
        "ptau",
        "ceremony",
        "trustedsetup",
        "structuredreferencestring",
        "universalsrs",
        "powersoftau",
    ]
    .iter()
    .any(|token| compact.contains(token))
}

fn is_developer_only_kagemusha_backend(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    if backend.contains("debug") || backend.contains("mock") {
        return true;
    }
    let compact = backend
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    compact.contains("debug") || compact.contains("mock")
}

fn kagemusha_backend_tag(backend: &str) -> Option<BackendTag> {
    if backend == "halo2/ipa" {
        Some(BackendTag::Halo2IpaPasta)
    } else if is_supported_kagemusha_proof_backend(backend) {
        Some(BackendTag::Stark)
    } else {
        None
    }
}

fn kagemusha_record_curve_for_backend(backend: BackendTag) -> Option<&'static str> {
    match backend {
        BackendTag::Halo2IpaPasta => Some("pallas"),
        BackendTag::Stark => Some("goldilocks"),
        _ => None,
    }
}

/// Domain-separation tag for the Poseidon2 Kagemusha aggregation transcript.
pub const KAGEMUSHA_POSEIDON_AGGREGATION_TRANSCRIPT_DOMAIN: &str =
    "iroha:kagemusha:v1:poseidon-aggregation-transcript";
/// Domain-separation tag for Kagemusha per-hop proof public-input statements.
pub const KAGEMUSHA_PROOF_PUBLIC_INPUTS_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:proof-public-inputs";
/// Domain-separation tag for Kagemusha per-hop verifier-key Poseidon2 digests.
pub const KAGEMUSHA_VERIFIER_KEY_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:verifier-key";
/// Maximum number of private Kagemusha hops folded into one compact token.
pub const KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS: usize = 64;
/// Maximum expected Norito size for chain-visible Kagemusha folded public inputs.
///
/// The compact public transcript must remain independent of hop count; proof
/// bytes are budgeted separately by the verifier-key record.
pub const KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES: usize = 1024;
/// Maximum Norito archive bytes accepted for previous recursive proof opening material.
pub const KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES: usize = 8 * 1024 * 1024;
/// Maximum input nullifiers per Kagemusha fold step.
pub const KAGEMUSHA_FOLD_STEP_MAX_INPUTS: usize = 2;
/// Maximum output commitments per Kagemusha fold step.
pub const KAGEMUSHA_FOLD_STEP_MAX_OUTPUTS: usize = 2;
/// Error returned when Offline Note canonical derivation inputs are invalid.
#[derive(Debug)]
pub enum OfflineNoteDerivationError {
    /// Random secret material must be exactly 32 bytes.
    InvalidRandomBytesLength {
        /// Name of the invalid field.
        field: &'static str,
        /// Expected byte count.
        expected: usize,
        /// Actual byte count.
        actual: usize,
    },
    /// Canonical Norito encoding failed.
    Encode(norito::Error),
}

impl core::fmt::Display for OfflineNoteDerivationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidRandomBytesLength {
                field,
                expected,
                actual,
            } => write!(
                f,
                "Offline Note {field} must be exactly {expected} bytes (found {actual})"
            ),
            Self::Encode(err) => write!(f, "failed to encode Offline Note preimage: {err}"),
        }
    }
}

impl std::error::Error for OfflineNoteDerivationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidRandomBytesLength { .. } => None,
            Self::Encode(err) => Some(err),
        }
    }
}

impl From<norito::Error> for OfflineNoteDerivationError {
    fn from(err: norito::Error) -> Self {
        Self::Encode(err)
    }
}

/// Error returned when compact Kagemusha folded-proof public inputs are invalid.
#[derive(Debug)]
pub enum KagemushaFoldError {
    /// Folded public inputs use an unsupported domain separator.
    InvalidPublicInputDomain {
        /// Expected domain separator.
        expected: &'static str,
        /// Domain separator carried by the token.
        actual: String,
    },
    /// Folded public inputs use an unsupported aggregation mode.
    UnsupportedAggregationMode {
        /// Expected aggregation mode.
        expected: u16,
        /// Aggregation mode carried by the token.
        actual: u16,
        /// Stable reason explaining why the mode is rejected.
        reason: &'static str,
    },
    /// At least one private hop is required.
    Empty,
    /// The private hop count exceeds [`KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS`].
    TooManyHops {
        /// Maximum accepted hop count.
        max: usize,
        /// Actual hop count.
        actual: usize,
    },
    /// A hop does not match the supported 1-to-2 transfer shape.
    InvalidStepShape {
        /// Zero-based hop index.
        hop_index: usize,
        /// Input nullifier count.
        input_count: usize,
        /// Output commitment count.
        output_count: usize,
    },
    /// An input nullifier is repeated within or across folded hops.
    DuplicateInputNullifier {
        /// Zero-based hop index where the duplicate was detected.
        hop_index: usize,
    },
    /// An output commitment is repeated within or across folded hops.
    DuplicateOutputCommitment {
        /// Zero-based hop index where the duplicate was detected.
        hop_index: usize,
    },
    /// An input nullifier and output commitment share the same 32-byte value.
    InputOutputOverlap {
        /// Zero-based hop index where the overlap was detected.
        hop_index: usize,
    },
    /// A folded-hop input nullifier is all-zero.
    ZeroInputNullifier {
        /// Zero-based hop index where the zero entry was detected.
        hop_index: usize,
    },
    /// A folded-hop output commitment is all-zero.
    ZeroOutputCommitment {
        /// Zero-based hop index where the zero entry was detected.
        hop_index: usize,
    },
    /// A folded-hop proof public-input digest is all-zero.
    ZeroProofPublicInputsDigest {
        /// Zero-based hop index where the zero digest was detected.
        hop_index: usize,
    },
    /// A folded-hop verifier-key commitment is all-zero.
    ZeroVerifierKeyCommitment {
        /// Zero-based hop index where the zero commitment was detected.
        hop_index: usize,
    },
    /// A folded-hop verifier-key Poseidon2 digest is all-zero.
    ZeroVerifierKeyPoseidonDigest {
        /// Zero-based hop index where the zero digest was detected.
        hop_index: usize,
    },
    /// A folded transcript Merkle root is all-zero.
    ZeroFoldedRoot {
        /// Name of the all-zero root field.
        field: &'static str,
    },
    /// A folded hop root transition does not change the Merkle root.
    UnchangedFoldedRootTransition {
        /// Zero-based hop index where the unchanged transition was detected.
        hop_index: usize,
    },
    /// Folded public inputs carry the same initial and final root.
    UnchangedFoldedPublicRoots,
    /// A direct aggregation transcript statement has a non-canonical hop count.
    HopCountMismatch {
        /// Expected hop count from the statement step list.
        expected: usize,
        /// Hop count carried by the statement.
        actual: u32,
    },
    /// A direct aggregation transcript statement has a non-canonical hop index.
    HopIndexMismatch {
        /// Expected zero-based hop index.
        expected: usize,
        /// Hop index carried by the statement step.
        actual: u32,
    },
    /// A direct aggregation transcript statement carries the wrong initial root.
    InitialRootMismatch {
        /// Expected initial root from the first hop.
        expected: [u8; Hash::LENGTH],
        /// Initial root carried by the statement.
        actual: [u8; Hash::LENGTH],
    },
    /// A direct aggregation transcript statement carries the wrong final root.
    FinalRootMismatch {
        /// Expected final root from the last hop.
        expected: [u8; Hash::LENGTH],
        /// Final root carried by the statement.
        actual: [u8; Hash::LENGTH],
    },
    /// A direct aggregation transcript statement has non-canonical input order.
    NonCanonicalInputNullifierOrder {
        /// Zero-based hop index where ordering failed.
        hop_index: usize,
    },
    /// A direct aggregation transcript statement has non-canonical output order.
    NonCanonicalOutputCommitmentOrder {
        /// Zero-based hop index where ordering failed.
        hop_index: usize,
    },
    /// Adjacent folded hops do not connect through the same Merkle root.
    RootDiscontinuity {
        /// Zero-based hop index where the discontinuity was detected.
        hop_index: usize,
        /// Root expected from the previous hop.
        expected: [u8; Hash::LENGTH],
        /// Root supplied by the current hop.
        actual: [u8; Hash::LENGTH],
    },
    /// A compact token proof is not bound to its canonical folded public inputs.
    PublicInputHashMismatch {
        /// Hash computed from the folded public inputs.
        expected: Hash,
        /// Hash declared by the folded proof.
        actual: Hash,
    },
    /// Folded public inputs are not the canonical projection of the aggregation transcript.
    FoldedPublicInputTranscriptMismatch {
        /// Name of the mismatched folded public-input field.
        field: &'static str,
    },
    /// Recursive aggregation evidence does not declare the reserved recursive mode.
    RecursiveAggregationEvidenceModeMismatch {
        /// Expected reserved recursive aggregation mode.
        expected: u16,
        /// Aggregation mode carried by the evidence statement.
        actual: u16,
    },
    /// Recursive aggregation evidence witness count does not match the folded hop count.
    RecursiveAggregationWitnessCountMismatch {
        /// Hop count carried by the aggregation statement.
        expected: u32,
        /// Witness count carried by the evidence.
        actual: u32,
    },
    /// Recursive aggregation evidence declares an unsupported verifier-witness profile.
    UnsupportedRecursiveVerifierWitnessProfile {
        /// Expected verifier-witness profile.
        expected: &'static str,
        /// Verifier-witness profile carried by the evidence.
        actual: String,
    },
    /// Recursive aggregation evidence declares an unsupported verifier opening length.
    UnsupportedRecursiveVerifierOpeningLength {
        /// Minimum supported opening length.
        min: u32,
        /// Maximum supported opening length.
        max: u32,
        /// Opening length carried by the evidence.
        actual: u32,
    },
    /// Recursive aggregation evidence declares a non-power-of-two verifier opening length.
    NonPowerOfTwoRecursiveVerifierOpeningLength {
        /// Opening length carried by the evidence.
        actual: u32,
    },
    /// Recursive aggregation evidence carries an all-zero verifier parameter fingerprint.
    ZeroRecursiveVerifierParamsFingerprint,
    /// Recursive aggregation evidence carries an all-zero fixed-window table schedule digest.
    ZeroRecursiveFixedWindowTableScheduleDigest,
    /// Recursive aggregation evidence carries an all-zero fixed-window shared-table manifest digest.
    ZeroRecursiveFixedWindowSharedTableManifestDigest,
    /// Recursive aggregation evidence carries an all-zero fixed-window table-base digest.
    ZeroRecursiveFixedWindowTableBaseDigest,
    /// Recursive aggregation evidence carries an all-zero verifier-witness batch digest.
    ZeroRecursiveVerifierWitnessBatchDigest,
    /// Recursive aggregation proof public inputs use an unsupported domain separator.
    InvalidRecursiveAggregationProofPublicInputDomain {
        /// Expected domain separator.
        expected: &'static str,
        /// Domain separator carried by the recursive proof public inputs.
        actual: String,
    },
    /// Recursive aggregation proof public inputs do not match their evidence.
    RecursiveAggregationProofPublicInputMismatch {
        /// Name of the mismatched public-input field.
        field: &'static str,
    },
    /// Recursive aggregation proof is not bound to its canonical public inputs.
    RecursiveAggregationProofPublicInputHashMismatch {
        /// Hash computed from the recursive proof public inputs.
        expected: Hash,
        /// Hash declared by the recursive proof.
        actual: Hash,
    },
    /// Recursive aggregation proof backend does not match its verifier-key id.
    RecursiveAggregationProofBackendMismatch {
        /// Proof backend label.
        proof_backend: String,
        /// Verifier-key backend label.
        verifier_key_backend: String,
    },
    /// Recursive aggregation proof verifier-key id does not use the canonical circuit id.
    RecursiveAggregationProofCircuitIdMismatch {
        /// Expected circuit id.
        expected: &'static str,
        /// Actual circuit id.
        actual: String,
    },
    /// Recursive aggregation proof carries invalid production proof metadata.
    InvalidRecursiveAggregationProof {
        /// Name of the invalid recursive aggregation proof field.
        field: &'static str,
    },
    /// Recursive spend accumulator uses an unsupported domain separator.
    InvalidRecursiveSpendAccumulatorDomain {
        /// Expected domain separator.
        expected: &'static str,
        /// Actual domain separator.
        actual: String,
    },
    /// Recursive spend accumulator field is not bound to its proof public inputs.
    RecursiveSpendPublicInputMismatch {
        /// Name of the mismatched field.
        field: &'static str,
    },
    /// Recursive spend accumulator has invalid top-up anchor nullifiers.
    InvalidRecursiveSpendTopupAnchor {
        /// Name of the invalid anchor field.
        field: &'static str,
    },
    /// Recursive spend accumulator has an invalid current spendable note.
    InvalidRecursiveSpendNote {
        /// Name of the invalid note field.
        field: &'static str,
    },
    /// Recursive spend redeem request has an invalid recursive spend proof attachment.
    InvalidRecursiveSpendProof {
        /// Name of the invalid recursive spend proof field.
        field: &'static str,
    },
    /// Recursive spend redeem request has an invalid final redeem proof attachment.
    InvalidRecursiveSpendRedeemProof {
        /// Name of the invalid redeem-proof field.
        field: &'static str,
    },
    /// Recursive spend append changed chain id.
    RecursiveSpendChainMismatch,
    /// Recursive spend append changed asset id.
    RecursiveSpendAssetMismatch,
    /// Recursive spend append does not continue from the previous final root.
    RecursiveSpendRootMismatch,
    /// Recursive spend append did not consume the previous spendable note nullifier.
    RecursiveSpendMissingPreviousNullifier,
    /// Recursive spend append introduced an input other than the previous spendable note.
    RecursiveSpendUnexpectedAppendInput,
    /// Recursive spend state does not bind the declared current note commitment.
    RecursiveSpendMissingCurrentNoteCommitment,
    /// Recursive spend verifier context changed across an append.
    RecursiveSpendVerifierContextMismatch {
        /// Name of the mismatched verifier context field.
        field: &'static str,
    },
    /// A folded public-input digest column group is all-zero.
    ZeroFoldedPublicInputDigest {
        /// Name of the all-zero folded public-input digest field.
        field: &'static str,
    },
    /// Folded public inputs exceed the compact-token public transcript size budget.
    EncodedSizeExceeded {
        /// Maximum accepted encoded size in bytes.
        max: usize,
        /// Actual encoded size in bytes.
        actual: usize,
    },
    /// A Kagemusha proof public-input statement carries a zero verifier-key hash.
    ZeroProofStatementVerifierKeyHash,
    /// A Kagemusha proof public-input statement carries an empty circuit id.
    EmptyProofStatementCircuitId,
    /// A Kagemusha proof public-input statement carries an empty public-input schema.
    EmptyProofStatementPublicInputsSchema,
    /// A Kagemusha proof public-input statement carries no public instance columns.
    EmptyProofStatementInstanceColumns,
    /// A Kagemusha proof public-input statement carries an empty public instance column.
    EmptyProofStatementInstanceColumn {
        /// Zero-based public instance column index.
        column_index: usize,
    },
    /// A Kagemusha proof public-input statement carries non-canonical auxiliary bytes.
    NonCanonicalProofStatementAuxiliaryBytes {
        /// Actual auxiliary byte count.
        actual: usize,
    },
    /// A Kagemusha verifier-key digest was requested for empty key bytes.
    EmptyVerifierKeyBytes {
        /// Backend label associated with the empty key bytes.
        backend: String,
    },
    /// A Kagemusha folded hop carries an empty verifier-key id name.
    EmptyVerifierKeyIdName {
        /// Zero-based hop index where the empty verifier-key id was found.
        hop_index: usize,
    },
    /// A Kagemusha proof statement or verifier-key digest used an unsupported proof backend.
    UnsupportedProofBackend {
        /// Unsupported backend label.
        backend: String,
    },
    /// A Kagemusha proof public-input statement backend tag does not match its proof backend.
    ProofStatementBackendTagMismatch {
        /// Proof backend label.
        proof_backend: String,
        /// Backend tag carried by the statement.
        envelope_backend: BackendTag,
    },
    /// Canonical Norito encoding failed.
    Encode(norito::Error),
}

impl core::fmt::Display for KagemushaFoldError {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidPublicInputDomain { expected, actual } => write!(
                f,
                "Kagemusha folded token domain must be {expected:?} (found {actual:?})"
            ),
            Self::UnsupportedAggregationMode {
                expected,
                actual,
                reason,
            } => write!(
                f,
                "Kagemusha folded token aggregation mode must be {expected} (found {actual}: {reason})"
            ),
            Self::Empty => write!(f, "Kagemusha folded token requires at least one hop"),
            Self::TooManyHops { max, actual } => write!(
                f,
                "Kagemusha folded token supports at most {max} hops (found {actual})"
            ),
            Self::InvalidStepShape {
                hop_index,
                input_count,
                output_count,
            } => write!(
                f,
                "Kagemusha fold hop {hop_index} requires 1 to {KAGEMUSHA_FOLD_STEP_MAX_INPUTS} inputs and 1 to {KAGEMUSHA_FOLD_STEP_MAX_OUTPUTS} outputs (found {input_count} inputs and {output_count} outputs)"
            ),
            Self::DuplicateInputNullifier { hop_index } => {
                write!(
                    f,
                    "Kagemusha fold hop {hop_index} repeats an input nullifier"
                )
            }
            Self::DuplicateOutputCommitment { hop_index } => {
                write!(
                    f,
                    "Kagemusha fold hop {hop_index} repeats an output commitment"
                )
            }
            Self::InputOutputOverlap { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} reuses an input nullifier as an output commitment"
            ),
            Self::ZeroInputNullifier { hop_index } => {
                write!(
                    f,
                    "Kagemusha fold hop {hop_index} has a zero input nullifier"
                )
            }
            Self::ZeroOutputCommitment { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} has a zero output commitment"
            ),
            Self::ZeroProofPublicInputsDigest { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} has a zero proof public-input digest"
            ),
            Self::ZeroVerifierKeyCommitment { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} has a zero verifier-key commitment"
            ),
            Self::ZeroVerifierKeyPoseidonDigest { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} has a zero verifier-key Poseidon2 digest"
            ),
            Self::ZeroFoldedRoot { field } => {
                write!(f, "Kagemusha folded root field {field:?} must be non-zero")
            }
            Self::UnchangedFoldedRootTransition { hop_index } => {
                write!(
                    f,
                    "Kagemusha fold hop {hop_index} must change the Merkle root"
                )
            }
            Self::UnchangedFoldedPublicRoots => write!(
                f,
                "Kagemusha folded public inputs require distinct initial and final roots"
            ),
            Self::HopCountMismatch { expected, actual } => write!(
                f,
                "Kagemusha aggregation transcript hop_count must be {expected} (found {actual})"
            ),
            Self::HopIndexMismatch { expected, actual } => write!(
                f,
                "Kagemusha aggregation transcript hop index must be {expected} (found {actual})"
            ),
            Self::InitialRootMismatch { .. } => write!(
                f,
                "Kagemusha aggregation transcript initial root does not match the first hop"
            ),
            Self::FinalRootMismatch { .. } => write!(
                f,
                "Kagemusha aggregation transcript final root does not match the last hop"
            ),
            Self::NonCanonicalInputNullifierOrder { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} input nullifiers must be sorted canonically"
            ),
            Self::NonCanonicalOutputCommitmentOrder { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} output commitments must be sorted canonically"
            ),
            Self::RootDiscontinuity { hop_index, .. } => write!(
                f,
                "Kagemusha fold hop {hop_index} does not continue from the previous root"
            ),
            Self::PublicInputHashMismatch { .. } => write!(
                f,
                "Kagemusha folded proof public-input hash does not match the compact token"
            ),
            Self::FoldedPublicInputTranscriptMismatch { field } => write!(
                f,
                "Kagemusha folded public input field {field:?} does not match the aggregation transcript"
            ),
            Self::RecursiveAggregationEvidenceModeMismatch { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation evidence mode must be {expected} (found {actual})"
            ),
            Self::RecursiveAggregationWitnessCountMismatch { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation evidence witness count must be {expected} (found {actual})"
            ),
            Self::UnsupportedRecursiveVerifierWitnessProfile { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation verifier-witness profile must be {expected:?} (found {actual:?})"
            ),
            Self::UnsupportedRecursiveVerifierOpeningLength { min, max, actual } => write!(
                f,
                "Kagemusha recursive aggregation verifier opening length must be {min}..={max} (found {actual})"
            ),
            Self::NonPowerOfTwoRecursiveVerifierOpeningLength { actual } => write!(
                f,
                "Kagemusha recursive aggregation verifier opening length must be a power of two (found {actual})"
            ),
            Self::ZeroRecursiveVerifierParamsFingerprint => write!(
                f,
                "Kagemusha recursive aggregation verifier parameter fingerprint must be non-zero"
            ),
            Self::ZeroRecursiveFixedWindowTableScheduleDigest => write!(
                f,
                "Kagemusha recursive aggregation fixed-window table schedule digest must be non-zero"
            ),
            Self::ZeroRecursiveFixedWindowSharedTableManifestDigest => write!(
                f,
                "Kagemusha recursive aggregation fixed-window shared-table manifest digest must be non-zero"
            ),
            Self::ZeroRecursiveFixedWindowTableBaseDigest => write!(
                f,
                "Kagemusha recursive aggregation fixed-window table base digest must be non-zero"
            ),
            Self::ZeroRecursiveVerifierWitnessBatchDigest => write!(
                f,
                "Kagemusha recursive aggregation verifier-witness batch digest must be non-zero"
            ),
            Self::InvalidRecursiveAggregationProofPublicInputDomain { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation proof public-input domain must be {expected:?} (found {actual:?})"
            ),
            Self::RecursiveAggregationProofPublicInputMismatch { field } => write!(
                f,
                "Kagemusha recursive aggregation proof public input {field:?} does not match the evidence"
            ),
            Self::RecursiveAggregationProofPublicInputHashMismatch { .. } => write!(
                f,
                "Kagemusha recursive aggregation proof public-input hash does not match its public inputs"
            ),
            Self::RecursiveAggregationProofBackendMismatch {
                proof_backend,
                verifier_key_backend,
            } => write!(
                f,
                "Kagemusha recursive aggregation proof backend {proof_backend:?} must match verifier-key backend {verifier_key_backend:?}"
            ),
            Self::RecursiveAggregationProofCircuitIdMismatch { expected, actual } => write!(
                f,
                "Kagemusha recursive aggregation proof circuit id must be {expected:?} (found {actual:?})"
            ),
            Self::InvalidRecursiveAggregationProof { field } => write!(
                f,
                "Kagemusha recursive aggregation proof field {field:?} is invalid"
            ),
            Self::InvalidRecursiveSpendAccumulatorDomain { expected, actual } => write!(
                f,
                "Kagemusha recursive spend accumulator domain must be {expected:?} (found {actual:?})"
            ),
            Self::RecursiveSpendPublicInputMismatch { field } => write!(
                f,
                "Kagemusha recursive spend accumulator field {field:?} is not bound to the recursive proof public inputs"
            ),
            Self::InvalidRecursiveSpendTopupAnchor { field } => {
                write!(
                    f,
                    "Kagemusha recursive spend top-up anchor field {field:?} is invalid"
                )
            }
            Self::InvalidRecursiveSpendNote { field } => {
                write!(
                    f,
                    "Kagemusha recursive spend current note field {field:?} is invalid"
                )
            }
            Self::InvalidRecursiveSpendProof { field } => write!(
                f,
                "Kagemusha recursive spend proof field {field:?} is invalid"
            ),
            Self::InvalidRecursiveSpendRedeemProof { field } => write!(
                f,
                "Kagemusha recursive spend redeem proof field {field:?} is invalid"
            ),
            Self::RecursiveSpendChainMismatch => {
                write!(f, "Kagemusha recursive spend append changed chain id")
            }
            Self::RecursiveSpendAssetMismatch => {
                write!(f, "Kagemusha recursive spend append changed asset id")
            }
            Self::RecursiveSpendRootMismatch => write!(
                f,
                "Kagemusha recursive spend append does not continue from the previous final root"
            ),
            Self::RecursiveSpendMissingPreviousNullifier => write!(
                f,
                "Kagemusha recursive spend append does not consume the previous spendable note nullifier"
            ),
            Self::RecursiveSpendUnexpectedAppendInput => write!(
                f,
                "Kagemusha recursive spend append may only consume the previous spendable note nullifier"
            ),
            Self::RecursiveSpendMissingCurrentNoteCommitment => write!(
                f,
                "Kagemusha recursive spend state does not create the declared current note commitment"
            ),
            Self::RecursiveSpendVerifierContextMismatch { field } => write!(
                f,
                "Kagemusha recursive spend verifier context field {field:?} changed across append"
            ),
            Self::ZeroFoldedPublicInputDigest { field } => write!(
                f,
                "Kagemusha folded public input digest field {field:?} must be non-zero"
            ),
            Self::EncodedSizeExceeded { max, actual } => write!(
                f,
                "Kagemusha folded public inputs must encode to at most {max} bytes (found {actual})"
            ),
            Self::ZeroProofStatementVerifierKeyHash => write!(
                f,
                "Kagemusha proof public-input statement verifier-key hash must be non-zero"
            ),
            Self::EmptyProofStatementCircuitId => {
                write!(
                    f,
                    "Kagemusha proof public-input statement circuit id must be non-empty"
                )
            }
            Self::EmptyProofStatementPublicInputsSchema => write!(
                f,
                "Kagemusha proof public-input statement schema must be non-empty"
            ),
            Self::EmptyProofStatementInstanceColumns => write!(
                f,
                "Kagemusha proof public-input statement instance columns must be non-empty"
            ),
            Self::EmptyProofStatementInstanceColumn { column_index } => write!(
                f,
                "Kagemusha proof public-input statement instance column {column_index} must be non-empty"
            ),
            Self::NonCanonicalProofStatementAuxiliaryBytes { actual } => write!(
                f,
                "Kagemusha proof public-input statement auxiliary bytes must be empty (found {actual})"
            ),
            Self::EmptyVerifierKeyBytes { backend } => write!(
                f,
                "Kagemusha verifier-key bytes for backend {backend:?} must be non-empty"
            ),
            Self::EmptyVerifierKeyIdName { hop_index } => write!(
                f,
                "Kagemusha fold hop {hop_index} verifier-key id name must be non-empty"
            ),
            Self::UnsupportedProofBackend { backend } => {
                write!(f, "Kagemusha proof backend {backend:?} is not supported")
            }
            Self::ProofStatementBackendTagMismatch {
                proof_backend,
                envelope_backend,
            } => write!(
                f,
                "Kagemusha proof public-input statement backend tag {envelope_backend:?} does not match proof backend {proof_backend:?}"
            ),
            Self::Encode(err) => {
                write!(f, "failed to encode Kagemusha folded public inputs: {err}")
            }
        }
    }
}

impl std::error::Error for KagemushaFoldError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(err) => Some(err),
            _ => None,
        }
    }
}

impl From<norito::Error> for KagemushaFoldError {
    fn from(err: norito::Error) -> Self {
        Self::Encode(err)
    }
}

/// Derive the deterministic Offline escrow account for an asset definition.
#[must_use]
pub fn offline_escrow_account_id(
    chain_id: &ChainId,
    definition_id: &AssetDefinitionId,
) -> AccountId {
    let seed_material = format!(
        "{OFFLINE_ESCROW_SEED_LABEL}|{}|{definition_id}",
        chain_id.as_str()
    );
    let seed: [u8; Hash::LENGTH] = Hash::new(seed_material).into();
    let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
        .expect("fixed Offline escrow Ed25519 account seed must derive");
    AccountId::new(keypair.public_key().clone())
}

#[model]
mod model {
    use super::*;

    /// Compact CA-issued certificate for an Offline one-use note key.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteKeyCertificate {
        /// Certificate format marker.
        pub version: u16,
        /// Platform class, for example `ios-appattest` or `android-keymint`.
        pub platform: String,
        /// Issuer-scoped one-use key identifier.
        pub key_id: String,
        /// Device identifier bound by the offline CA.
        pub device_id: String,
        /// Account authorized to control the note key.
        pub account_id: AccountId,
        /// Ed25519 public key bytes for local note/proof signatures.
        pub public_key: Vec<u8>,
        /// Hardware assertion scheme bound to this note key.
        pub assertion_scheme: String,
        /// Hardware assertion key algorithm, for example `ecdsa-p256-sha256`.
        pub assertion_key_algorithm: String,
        /// Hardware assertion public key bytes, for example SEC1 P-256.
        pub assertion_public_key: Vec<u8>,
        /// Hardware one-use limit when the platform exposes it.
        pub assertion_usage_count_limit: Option<u32>,
        /// True when the issuer verified hardware one-use semantics.
        pub one_use: bool,
        /// Offline CA signature over the compact certificate payload.
        pub issuer_signature: Signature,
    }

    /// Canonical payload signed by Offline key-certificate issuers.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteKeyCertificatePayload {
        /// Domain separator for the signed payload.
        pub domain: String,
        /// Certificate format marker.
        pub version: u16,
        /// Platform class, for example `ios-appattest` or `android-keymint`.
        pub platform: String,
        /// Issuer-scoped one-use key identifier.
        pub key_id: String,
        /// Device identifier bound by the offline CA.
        pub device_id: String,
        /// Account authorized to control the note key.
        pub account_id: AccountId,
        /// Ed25519 public key bytes for local note/proof signatures.
        pub public_key: Vec<u8>,
        /// Hardware assertion scheme bound to this note key.
        pub assertion_scheme: String,
        /// Hardware assertion key algorithm, for example `ecdsa-p256-sha256`.
        pub assertion_key_algorithm: String,
        /// Hardware assertion public key bytes, for example SEC1 P-256.
        pub assertion_public_key: Vec<u8>,
        /// Hardware one-use limit when the platform exposes it.
        pub assertion_usage_count_limit: Option<u32>,
        /// True when the issuer verified hardware one-use semantics.
        pub one_use: bool,
    }

    /// Verifier-key-backed recursive proof carried by Offline note tokens.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRecursiveProof {
        /// Stable verifier key identifier selected by the operator and stored in WSV.
        pub verifier_key_id: VerifyingKeyId,
        /// Public input commitment hash.
        pub public_inputs_hash: Hash,
        /// Compact recursive proof payload encoded as an `OpenVerifyEnvelope`.
        pub proof: ProofBox,
    }

    /// Issuer-side note issuance record for online load/consolidation.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssue {
        /// Deterministic note commitment.
        pub note_commitment: Hash,
        /// Owner key certificate for this note.
        pub key_certificate: OfflineNoteKeyCertificate,
        /// Asset held by the note.
        pub asset: AssetId,
        /// Note amount.
        pub amount: Numeric,
    }

    /// Ledger-recognized note claim bound to one compact Offline note certificate.
    ///
    /// Issuer loads create this claim directly; P2P bearer outputs create the same claim only
    /// when their audit lineage is submitted, either before redemption or earlier in the same
    /// transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssuedClaim {
        /// Domain separator for the issued-note claim.
        pub domain: String,
        /// Deterministic note commitment recorded at issuance.
        pub note_commitment: Hash,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Asset held by the issued note.
        pub asset: AssetId,
        /// Note amount reserved into offline escrow.
        pub amount: Numeric,
    }

    /// Redeemable note output observed during Offline audit.
    ///
    /// The output is final for offline bearers when received locally. The ledger recognizes it
    /// after the corresponding audit is committed.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditOutputClaim {
        /// Deterministic note commitment created by the audited transfer.
        pub note_commitment: Hash,
        /// Owner key certificate for this output note.
        pub key_certificate: OfflineNoteKeyCertificate,
        /// Asset held by this output note.
        pub asset: AssetId,
        /// Output amount reserved in offline escrow.
        pub amount: Numeric,
    }

    /// Redemption payload submitted online when defunding a bearer note.
    ///
    /// The source claim must already be ledger-recognized. For unanchored P2P bearer outputs,
    /// submit their ordered audit lineage before this redeem instruction in the same transaction.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRedeem {
        /// Ledger-recognized note commitment consumed by this redemption.
        pub source_note_commitment: Hash,
        /// Nullifiers consumed by the redeeming token.
        pub input_nullifiers: Vec<Hash>,
        /// Compact certificate for the one-use note key that signed the proof.
        pub sender_key_certificate: OfflineNoteKeyCertificate,
        /// Recipient account credited online.
        pub recipient: AccountId,
        /// Asset being redeemed.
        pub asset: AssetId,
        /// Redeemed amount.
        pub amount: Numeric,
        /// Compact recursive proof for the final note state.
        pub recursive_proof: OfflineNoteRecursiveProof,
    }

    /// Public inputs bound by an Offline redemption proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteRedeemPublicInputs {
        /// Domain separator for the redemption public inputs.
        pub domain: String,
        /// Ledger-recognized note commitment consumed by this redemption.
        pub source_note_commitment: Hash,
        /// Nullifiers consumed by the redeeming token.
        pub input_nullifiers: Vec<Hash>,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Recipient account credited online.
        pub recipient: AccountId,
        /// Asset being redeemed.
        pub asset: AssetId,
        /// Redeemed amount.
        pub amount: Numeric,
    }

    /// Audit bundle for Offline P2P bearer lineage.
    ///
    /// It is not required for offline transfer finality, but it anchors P2P output claims so the
    /// ledger can later redeem them from offline escrow. Ledger execution checks each output
    /// certificate signature against the output account before recording that new lineage.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditBundle {
        /// Payment token identifier.
        pub token_id: Hash,
        /// Compact certificate for the one-use note key that signed the proof.
        pub sender_key_certificate: OfflineNoteKeyCertificate,
        /// Input nullifiers observed in the token.
        pub input_nullifiers: Vec<Hash>,
        /// Issued input claims consumed by the token.
        pub input_claims: Vec<OfflineNoteIssuedClaim>,
        /// Output note commitments created by the token.
        pub output_commitments: Vec<Hash>,
        /// Redeemable output claims created by the token.
        pub output_claims: Vec<OfflineNoteAuditOutputClaim>,
        /// Optional recursive proof for audit/replay checks.
        pub recursive_proof: OfflineNoteRecursiveProof,
    }

    /// Public inputs bound by an Offline optional audit proof.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteAuditPublicInputs {
        /// Domain separator for the audit public inputs.
        pub domain: String,
        /// Payment token identifier.
        pub token_id: Hash,
        /// Certificate payload hash identifying the one-use note key.
        pub key_certificate_payload_hash: Hash,
        /// Input nullifiers observed in the token.
        pub input_nullifiers: Vec<Hash>,
        /// Issued input claims consumed by the token.
        pub input_claims: Vec<OfflineNoteIssuedClaim>,
        /// Output note commitments created by the token.
        pub output_commitments: Vec<Hash>,
        /// Redeemable output claims created by the token.
        pub output_claims: Vec<OfflineNoteIssuedClaim>,
    }

    /// Origin of a wallet-derived Offline Note note commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteIssuerLoadOrigin {
        /// Wallet operation id sent to Torii.
        pub operation_id: String,
        /// Issuer lineage id updated by Torii.
        pub lineage_id: String,
        /// Local lineage revision after issuing the note.
        pub local_revision: u64,
    }

    /// Origin data for an offline peer-to-peer payment token output.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteP2pOutputOrigin {
        /// Recipient payment request id.
        pub payment_request_id: String,
        /// Output index inside the payment token.
        pub output_index: u32,
    }

    /// Canonical preimage used to derive an Offline Note note commitment.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteCommitmentPreimage {
        /// Domain separator for note commitments.
        pub domain: String,
        /// Chain id that scopes this note.
        pub chain_id: ChainId,
        /// Hash of the owner key certificate payload.
        pub owner_key_certificate_payload_hash: Hash,
        /// Asset held by the note.
        pub asset: AssetId,
        /// Note amount.
        pub amount: Numeric,
        /// Wallet-generated 32-byte note secret.
        pub note_secret: Vec<u8>,
        /// Origin metadata that separates issuer loads from P2P outputs.
        pub origin: OfflineNoteCommitmentOrigin,
    }

    /// Canonical preimage used to derive an Offline Note input nullifier.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNoteInputNullifierPreimage {
        /// Domain separator for input nullifiers.
        pub domain: String,
        /// Chain id that scopes this nullifier.
        pub chain_id: ChainId,
        /// Commitment of the note being spent.
        pub source_note_commitment: Hash,
        /// Hash of the owner key certificate payload.
        pub owner_key_certificate_payload_hash: Hash,
        /// Wallet-generated 32-byte note secret.
        pub note_secret: Vec<u8>,
    }

    /// Canonical preimage used to derive an Offline Note payment token id.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct OfflineNotePaymentTokenIdPreimage {
        /// Domain separator for payment token ids.
        pub domain: String,
        /// Chain id that scopes this payment token.
        pub chain_id: ChainId,
        /// Wallet-local payment request id that binds this token to one receive request.
        pub payment_request_id: String,
        /// Wallet-local token creation time in Unix milliseconds.
        pub created_at_ms: u64,
        /// Wallet-generated 32-byte payment token nonce.
        pub token_nonce: Vec<u8>,
        /// Hash of the sender key certificate payload.
        pub sender_key_certificate_payload_hash: Hash,
        /// Input nullifiers consumed by the token.
        pub input_nullifiers: Vec<Hash>,
        /// Output commitments created by the token.
        pub output_commitments: Vec<Hash>,
    }

    /// Canonical public-input statement verified for one Kagemusha private hop proof.
    ///
    /// Wallets and future recursive aggregators hash this statement with
    /// [`kagemusha_proof_public_inputs_statement_digest`] before inserting it into a folded-hop
    /// transcript. The statement is canonical only when `vk_hash` is non-zero and
    /// `envelope_aux` is empty; the private proof payload itself is committed separately by
    /// `proof_hash`.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaProofPublicInputsStatement {
        /// Proof backend label carried by the verified `ProofBox`.
        pub proof_backend: String,
        /// Backend tag carried by the transparent `OpenVerifyEnvelope`.
        pub envelope_backend: BackendTag,
        /// Circuit identifier carried by the transparent `OpenVerifyEnvelope`.
        pub circuit_id: String,
        /// Verifier-key hash carried by the transparent `OpenVerifyEnvelope`.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub vk_hash: [u8; 32],
        /// Public-input schema or descriptor bytes carried by the transparent envelope.
        pub public_inputs_schema: Vec<u8>,
        /// Auxiliary bytes carried by the transparent envelope; Kagemusha statements require this
        /// to be empty.
        pub envelope_aux: Vec<u8>,
        /// Backend-native public input columns that were verified.
        pub instance_columns: Vec<Vec<[u8; 32]>>,
    }

    /// One hop statement inside the Poseidon2 Kagemusha aggregation transcript.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPoseidonAggregationStepStatement {
        /// Zero-based hop index inside the folded transcript.
        pub hop_index: u32,
        /// Recent shielded Merkle root before this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_before: [u8; 32],
        /// Canonicalized input nullifiers consumed by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub input_nullifiers: Vec<[u8; 32]>,
        /// Canonicalized output note commitments created by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub output_commitments: Vec<[u8; 32]>,
        /// Shielded Merkle root after this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_after: [u8; 32],
        /// Domain-separated hash of the transparent per-hop proof payload.
        pub proof_hash: Hash,
        /// Poseidon2 digest of the per-hop proof public input statement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub proof_public_inputs_digest: [u8; 32],
        /// Verifier key identifier used to verify the per-hop proof.
        pub verifier_key_id: VerifyingKeyId,
        /// Host-side commitment of the verifier-key bytes used for this hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_commitment: [u8; 32],
        /// Poseidon2 digest of the verifier-key bytes used for this hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_poseidon_digest: [u8; 32],
    }

    /// Canonical Poseidon2 aggregation transcript statement for Kagemusha folding.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaPoseidonAggregationTranscriptStatement {
        /// Aggregation mode declared by the folded public inputs.
        pub aggregation_mode: u16,
        /// Chain id that scopes the folded token.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Root before the first folded hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Root after the final folded hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// Number of private hops folded into the compact proof.
        pub hop_count: u32,
        /// Ordered canonical hop statements.
        pub steps: Vec<KagemushaPoseidonAggregationStepStatement>,
    }

    /// Reserved-mode evidence binding a native verifier-witness batch to an aggregation transcript.
    ///
    /// This is not chain-accepted compact-token state in this release. It is the
    /// canonical wallet/prover-side statement that mode `2` recursive
    /// aggregation work can use to bind host preflight of native Pallas IPA
    /// verifier witnesses to the same ordered hop transcript that mode `1`
    /// exposes through checked pre-fold public inputs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveAggregationEvidence {
        /// Canonical ordered aggregation transcript statement using reserved recursive mode `2`.
        pub aggregation_statement: KagemushaPoseidonAggregationTranscriptStatement,
        /// Number of native verifier witnesses validated into the batch.
        pub verifier_witness_count: u32,
        /// Canonical no-trusted-setup verifier-witness profile used by the native batch preflight.
        pub verifier_witness_profile: String,
        /// Pallas IPA opening vector length used by the native verifier-witness batch.
        pub verifier_opening_len: u32,
        /// Transparent parameter fingerprint used by the native verifier-witness batch.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_params_fingerprint: [u8; 32],
        /// Poseidon2 digest of the deterministic shared fixed-window table schedule.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Poseidon2 digest of the compressed shared fixed-window table row manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Poseidon2 digest of the ordered fixed-window table bases validated by native preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_base_digest: [u8; 32],
        /// Domain-separated digest emitted by the native verifier-witness batch preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_witness_batch_digest: [u8; 32],
    }

    /// Public inputs that a recursive aggregation proof must expose.
    ///
    /// The values are derived from [`KagemushaRecursiveAggregationEvidence`] and
    /// keep a future mode-2 recursive verifier proof bound to the exact
    /// no-trusted-setup verifier-witness batch, opening width, and ordered
    /// aggregation transcript it claims to compress.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveAggregationProofPublicInputs {
        /// Domain separator for recursive aggregation proof public inputs.
        pub domain: String,
        /// Poseidon2 digest of the canonical recursive aggregation evidence.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub evidence_digest: [u8; 32],
        /// Hash of the reserved folded public-input projection claimed by the recursive proof.
        ///
        /// Future `kagemusha-recursive-compact-v1` admission compares this
        /// value with the chain-visible compact token public-input hash, so a
        /// detached recursive proof cannot be replayed against a different
        /// folded compact-token transcript.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub folded_public_inputs_hash: [u8; 32],
        /// Poseidon2 digest of the ordered folded-hop aggregation transcript.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub aggregation_transcript_digest: [u8; 32],
        /// Transparent parameter fingerprint used by the native verifier-witness batch.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_params_fingerprint: [u8; 32],
        /// Poseidon2 digest of the deterministic shared fixed-window table schedule.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Poseidon2 digest of the compressed shared fixed-window table row manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Poseidon2 digest of the ordered fixed-window table bases validated by native preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_base_digest: [u8; 32],
        /// Domain-separated digest emitted by the native verifier-witness batch preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_witness_batch_digest: [u8; 32],
        /// Streaming recursive spend proof-chain digest.
        ///
        /// Plain recursive aggregation proofs set this to zero. Recursive
        /// spend proofs set it from `KagemushaRecursiveSpendAccumulatorV1` so
        /// append verifiers can bind the exact previous recursive proof
        /// artifact without carrying prior hop bundles.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recursive_proof_chain_digest: [u8; 32],
        /// Non-circular digest of the canonical Reserved-lineage transition profile.
        ///
        /// Plain recursive aggregation proofs set this to zero. Recursive
        /// spend proofs set it from `KagemushaRecursiveSpendAccumulatorV1` so
        /// append/redeem verifiers can bind the host transition contract that
        /// the Reserved-lineage circuits reproduce in-circuit.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub transition_profile_binding_digest: [u8; 32],
        /// Digest of the Reserved-lineage append opening preflight contract.
        ///
        /// Plain recursive aggregation proofs, initial recursive spend proofs,
        /// and semantic recursive append proofs set this to zero. A
        /// Reserved-lineage append proof sets it to the validated
        /// `KagemushaRecursiveSpendLineageAppendOpeningPreflightV1` digest so
        /// public proof inputs bind the exact previous-proof opening contract
        /// without carrying hop-count-dependent archives in the spend bundle.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub append_opening_preflight_digest: [u8; 32],
        /// Digest of the compact Reserved-lineage append boundary.
        ///
        /// Plain recursive aggregation proofs, initial recursive spend proofs,
        /// and semantic recursive append proofs set this to zero. A production
        /// Reserved-lineage append proof sets it to the validated
        /// `KagemushaRecursiveSpendLineageAppendBoundaryV1` digest so public
        /// proof inputs bind the compact boundary that the circuit must
        /// reproduce without carrying hop-count-dependent archives.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub append_boundary_digest: [u8; 32],
        /// Scalar-projection digest emitted by the composed recursive verifier slice.
        ///
        /// Plain recursive aggregation proofs and current spend proofs set this
        /// to zero. Reserved-lineage verifier-slice circuits bind these limbs to
        /// the in-circuit scalar projection of their verifier witnesses, giving
        /// the complete recursive circuit a stable public channel for that
        /// challenge-binding output.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recursive_verifier_scalar_projection_digest: [u8; 32],
        /// Pallas IPA opening vector length used by the recursive verifier proof.
        pub verifier_opening_len: u32,
        /// Number of native verifier witnesses compressed by the recursive proof.
        pub verifier_witness_count: u32,
        /// Number of folded private hops represented by the aggregation transcript.
        pub hop_count: u32,
    }

    /// Transparent proof claiming one recursive aggregation evidence statement.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveAggregationProof {
        /// Stable verifier key identifier for the recursive aggregation proof circuit.
        pub verifier_key_id: VerifyingKeyId,
        /// Public inputs exposed by the recursive aggregation proof.
        pub public_inputs: KagemushaRecursiveAggregationProofPublicInputs,
        /// Public input commitment hash.
        pub public_inputs_hash: Hash,
        /// Transparent proof payload encoded as an `OpenVerifyEnvelope`.
        pub proof: ProofBox,
    }

    /// Recursive aggregation evidence paired with the proof that claims it.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveAggregationProofBundle {
        /// Canonical host-side evidence statement.
        pub evidence: KagemushaRecursiveAggregationEvidence,
        /// Transparent no-trusted-setup recursive proof bound to `evidence`.
        pub recursive_proof: KagemushaRecursiveAggregationProof,
    }

    /// Spendable note descriptor carried by recursive Kagemusha offline cash.
    ///
    /// This is the holder-facing constant-size descriptor needed to receive,
    /// store, re-spend, and later redeem the current cash state. It intentionally
    /// does not expose prior hop proofs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaSpendableNoteDescriptorV1 {
        /// Current spendable note commitment created by the latest offline hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub note_commitment: [u8; 32],
        /// Nullifier that must be consumed by the next offline hop or final redeem.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub spend_nullifier: [u8; 32],
        /// Public amount represented by this note.
        pub amount: Numeric,
    }

    /// Constant-size recursive Kagemusha spend accumulator.
    ///
    /// The accumulator is the D2D payload state for recursive Kagemusha cash. It
    /// keeps only streaming commitments to prior hops, the public verifier
    /// context, the current spendable note descriptor, and chain/asset/root
    /// bindings. Prior hop proofs and verifier witnesses are not carried.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendAccumulatorV1 {
        /// Domain separator for recursive spend accumulator state.
        pub domain: String,
        /// Chain id that scopes the spendable state.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Root before the first offline hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Root after the latest offline hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// First-hop input nullifiers that anchor this recursive cash to its online-to-offline top-up lineage.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub topup_anchor_nullifiers: Vec<[u8; 32]>,
        /// Number of offline hops accumulated.
        pub hop_count: u32,
        /// Streaming digest of ordered hop semantics.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub lineage_digest: [u8; 32],
        /// Streaming digest used as the recursive proof aggregation transcript public input.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub aggregation_transcript_digest: [u8; 32],
        /// Streaming digest of all consumed nullifiers.
        pub nullifier_digest: Hash,
        /// Streaming digest of all output commitments.
        pub output_commitment_digest: Hash,
        /// Streaming host hash of folded hop statements.
        pub fold_digest: Hash,
        /// Streaming digest of recursive proof artifacts consumed by append proofs.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub recursive_proof_chain_digest: [u8; 32],
        /// Non-circular digest of the transition profile that produced this accumulator.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub transition_profile_binding_digest: [u8; 32],
        /// Digest of the Reserved-lineage append opening preflight contract for this hop.
        ///
        /// This is zero for init and semantic append outputs. Reserved-lineage
        /// append outputs set it to the two-opening preflight contract digest
        /// that proves the previous recursive proof opening and the current
        /// checked-hop opening share the same verifier corridor.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub append_opening_preflight_digest: [u8; 32],
        /// Digest of the compact Reserved-lineage append boundary for this hop.
        ///
        /// This is zero for initial states, semantic append outputs, and
        /// digest-only compatibility append outputs. Full Reserved-lineage
        /// append outputs set it to the validated
        /// `KagemushaRecursiveSpendLineageAppendBoundaryV1` digest. The
        /// accumulator digest is intentionally computed with this field blanked
        /// so the boundary digest can be placed back into proof public inputs
        /// without creating a self-reference.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub append_boundary_digest: [u8; 32],
        /// Transparent parameter fingerprint used by the recursive verifier batch.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_params_fingerprint: [u8; 32],
        /// Poseidon2 digest of the deterministic fixed-window table schedule.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Poseidon2 digest of the shared fixed-window table manifest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Poseidon2 digest of the fixed-window table bases.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_base_digest: [u8; 32],
        /// Streaming digest of the verifier-witness batch.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_witness_batch_digest: [u8; 32],
        /// Pallas IPA opening vector length used by the recursive proof corridor.
        pub verifier_opening_len: u32,
        /// Current spendable note descriptor.
        pub current_note: KagemushaSpendableNoteDescriptorV1,
    }

    /// Canonical Reserved-lineage accumulator transition profile.
    ///
    /// This object is the host-side contract that the production
    /// Reserved-lineage circuit must reproduce in-circuit for each append. It
    /// binds the previous accumulator and recursive proof artifact, the current
    /// one-hop transfer statement, the native verifier-witness/table-base
    /// preflight digests, and every resulting accumulator/public-input digest.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendTransitionProfileV1 {
        /// Domain separator for transition profiles.
        pub domain: String,
        /// Chain id that scopes the transition.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Digest of the previous accumulator, absent for the initial hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_accumulator_digest: Option<[u8; 32]>,
        /// Previous accumulator initial root, absent for the initial hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_initial_root: Option<[u8; 32]>,
        /// Previous accumulator final root, absent for the initial hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_final_root: Option<[u8; 32]>,
        /// Previous spendable note descriptor, absent for the initial hop.
        pub previous_current_note: Option<KagemushaSpendableNoteDescriptorV1>,
        /// Previous streaming lineage digest, absent for the initial hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_lineage_digest: Option<[u8; 32]>,
        /// Previous recursive proof-chain digest, absent for the initial hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_recursive_proof_chain_digest: Option<[u8; 32]>,
        /// Digest of the previous recursive proof artifact, absent for the initial hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_recursive_proof_artifact_digest: Option<[u8; 32]>,
        /// Public-input hash derived from the previous accumulator.
        pub previous_accumulator_public_inputs_hash: Option<Hash>,
        /// Public-input hash exposed by the previous recursive proof.
        pub previous_recursive_proof_public_inputs_hash: Option<Hash>,
        /// Digest of the previous recursive proof opening archive consumed by append proving.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_recursive_proof_open_envelopes_archive_digest: Option<[u8; 32]>,
        /// Digest of previous-proof plus current-hop opening preflight material.
        ///
        /// Production Reserved-lineage append circuits must prove this same
        /// two-opening preflight in-circuit. Legacy evidence-only transition
        /// profiles leave it empty, and it is valid only when
        /// `previous_recursive_proof_open_envelopes_archive_digest` is present.
        #[norito(default)]
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub append_opening_preflight_digest: Option<[u8; 32]>,
        /// Full two-opening append preflight contract, when native hosts computed it.
        ///
        /// This is defaulted so archives emitted before the contract was promoted
        /// still decode. When present it must hash to
        /// `append_opening_preflight_digest` and match the previous-recursive-proof
        /// plus current-hop fields in this transition profile.
        #[norito(default)]
        pub append_opening_preflight:
            Option<KagemushaRecursiveSpendLineageAppendOpeningPreflightV1>,
        /// Previous streaming verifier-witness batch digest, absent for the initial hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_verifier_witness_batch_digest: Option<[u8; 32]>,
        /// Previous streaming fixed-window table-base digest, absent for the initial hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub previous_fixed_window_table_base_digest: Option<[u8; 32]>,
        /// Zero-based hop index for the transition.
        pub hop_index: u32,
        /// Resulting accumulated hop count.
        pub hop_count: u32,
        /// Current hop statement with sorted public nullifier/commitment sets.
        pub current_hop_statement: KagemushaPoseidonAggregationStepStatement,
        /// Current spendable note created by the hop.
        pub current_note: KagemushaSpendableNoteDescriptorV1,
        /// Per-hop native verifier-witness batch digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub current_hop_verifier_witness_batch_digest: [u8; 32],
        /// Per-hop fixed-window table-base digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub current_hop_fixed_window_table_base_digest: [u8; 32],
        /// Transparent parameter fingerprint used by this verifier corridor.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_params_fingerprint: [u8; 32],
        /// Shared fixed-window table schedule digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Shared fixed-window table manifest digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Pallas IPA opening vector length.
        pub verifier_opening_len: u32,
        /// Resulting accumulator initial root.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_initial_root: [u8; 32],
        /// Resulting accumulator final root.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_final_root: [u8; 32],
        /// Resulting streaming lineage digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_lineage_digest: [u8; 32],
        /// Resulting streaming verifier-witness batch digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_verifier_witness_batch_digest: [u8; 32],
        /// Resulting streaming fixed-window table-base digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_fixed_window_table_base_digest: [u8; 32],
        /// Resulting recursive proof-chain digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_recursive_proof_chain_digest: [u8; 32],
        /// Resulting Reserved-lineage append opening preflight digest.
        ///
        /// Zero means the transition did not bind a Reserved-lineage append
        /// opening preflight contract.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_append_opening_preflight_digest: [u8; 32],
        /// Resulting streaming nullifier digest.
        pub resulting_nullifier_digest: Hash,
        /// Resulting streaming output-commitment digest.
        pub resulting_output_commitment_digest: Hash,
        /// Resulting folded-hop transcript digest.
        pub resulting_fold_digest: Hash,
        /// Resulting accumulator digest exposed through recursive proof public inputs.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_accumulator_digest: [u8; 32],
        /// Resulting recursive proof public-input hash with append-boundary limbs blanked.
        ///
        /// The compact append boundary uses this non-circular hash while
        /// deriving `append_boundary_digest`; the final recursive proof public
        /// inputs then place that digest back into the append-boundary limbs.
        pub resulting_public_inputs_hash: Hash,
    }

    /// Portable Pallas IPA verifier preflight summary for recursive Kagemusha.
    ///
    /// Native hosts derive this after checking an opening witness. Keeping it in
    /// the data model gives SDKs and bridge tests a stable, Norito-encoded view of
    /// the public verifier-batch contract without reimplementing IPA proof logic.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveVerifierPreflightV1 {
        /// Number of native verifier witnesses summarized by this preflight.
        pub proof_count: u32,
        /// Canonical recursive verifier-witness profile.
        pub verifier_witness_profile: String,
        /// Pallas IPA opening vector length.
        pub opening_len: u32,
        /// Transparent Pallas IPA parameter fingerprint.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub params_fingerprint: [u8; 32],
        /// Deterministic fixed-window table schedule digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Shared fixed-window table manifest digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Ordered fixed-window table-base digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_base_digest: [u8; 32],
        /// Domain-separated digest binding validated verifier witnesses in order.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub aggregate_digest: [u8; 32],
    }

    /// Canonical Reserved-lineage append opening-preflight contract.
    ///
    /// A witnessless append circuit must bind the previous recursive proof opening
    /// and the current checked-hop opening. This model is the exact public contract
    /// that native hosts digest before placing `append_opening_preflight_digest`
    /// into the transition profile.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendLineageAppendOpeningPreflightV1 {
        /// Domain separator for append opening preflight contracts.
        pub domain: String,
        /// Native preflight for the previous recursive proof opening.
        pub previous_recursive_proof_preflight: KagemushaRecursiveVerifierPreflightV1,
        /// Native preflight for the current checked-hop proof opening.
        pub current_hop_preflight: KagemushaRecursiveVerifierPreflightV1,
        /// Digest of the previous recursive spend accumulator.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub previous_accumulator_digest: [u8; 32],
        /// Digest of the previous recursive proof artifact.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub previous_recursive_proof_artifact_digest: [u8; 32],
        /// Digest of the previous recursive proof opening archive bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub previous_recursive_proof_open_envelopes_archive_digest: [u8; 32],
        /// Checked-hop proof hash consumed by this append.
        pub current_hop_proof_hash: Hash,
        /// Domain-separated digest of all fields above.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub append_opening_preflight_digest: [u8; 32],
    }

    /// Compact append boundary derived from a full Reserved-lineage transition profile.
    ///
    /// This is the SDK/circuit handoff object for witnessless append work. It
    /// keeps the full transport-sized transition profile out of proving keys
    /// while preserving one canonical digest over the previous proof, append
    /// opening preflight, current hop, and resulting recursive public inputs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendLineageAppendBoundaryV1 {
        /// Domain separator for compact append boundaries.
        pub domain: String,
        /// Digest of the full transition profile, including the opening-preflight contract.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub transition_profile_digest: [u8; 32],
        /// Non-circular accumulator transition binding digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub transition_profile_binding_digest: [u8; 32],
        /// Explicit chain/asset binding digest for the append transition.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub chain_asset_binding_digest: [u8; 32],
        /// Explicit final-root/current-note binding digest for the append transition.
        #[norito(default)]
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_note_binding_digest: [u8; 32],
        /// Digest of the previous recursive spend accumulator.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub previous_accumulator_digest: [u8; 32],
        /// Digest of the previous recursive proof artifact.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub previous_recursive_proof_artifact_digest: [u8; 32],
        /// Digest of previous recursive proof opening archive bytes.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub previous_recursive_proof_open_envelopes_archive_digest: [u8; 32],
        /// Digest of the append opening preflight contract.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub append_opening_preflight_digest: [u8; 32],
        /// Aggregate digest from the previous recursive proof opening preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub previous_recursive_proof_opening_aggregate_digest: [u8; 32],
        /// Aggregate digest from the current checked-hop opening preflight.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub current_hop_opening_aggregate_digest: [u8; 32],
        /// Checked-hop proof hash consumed by this append.
        pub current_hop_proof_hash: Hash,
        /// Resulting accumulator digest exposed through recursive public inputs.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub resulting_accumulator_digest: [u8; 32],
        /// Resulting recursive proof public-input hash with append-boundary limbs blanked.
        ///
        /// This avoids a fixed point while still binding the exact
        /// boundary-free public inputs from which the final append-boundary
        /// public input is derived.
        pub resulting_public_inputs_hash: Hash,
        /// Resulting accumulated hop count.
        pub hop_count: u32,
        /// Pallas IPA opening vector length.
        pub verifier_opening_len: u32,
        /// Transparent Pallas IPA parameter fingerprint.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_params_fingerprint: [u8; 32],
        /// Shared fixed-window table schedule digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_table_schedule_digest: [u8; 32],
        /// Shared fixed-window table manifest digest.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub fixed_window_shared_table_manifest_digest: [u8; 32],
        /// Domain-separated digest of all fields above.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub append_boundary_digest: [u8; 32],
    }

    /// Production recursive Kagemusha spend bundle carried between offline holders.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendBundleV1 {
        /// Constant-size recursive spend accumulator.
        pub accumulator: KagemushaRecursiveSpendAccumulatorV1,
        /// Transparent no-trusted-setup proof bound to the accumulator.
        pub recursive_proof: KagemushaRecursiveAggregationProof,
    }

    /// Portable Reserved-lineage verifier/proving key artifact package.
    ///
    /// Release tooling can generate this Norito archive once per supported
    /// Pallas opening length and circuit profile, then mobile SDKs can embed
    /// the archive and attach its fields to init or append requests without
    /// synthesizing recursive verifier-slice keys at payment time.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendLineageKeyArtifactsV1 {
        /// Profile-specific Reserved-lineage circuit id for these artifacts.
        pub proof_circuit_id: String,
        /// Pallas IPA opening vector length supported by the packaged key pair.
        pub verifier_opening_len: u32,
        /// Packaged Reserved-lineage verifier key.
        pub lineage_verifier_key: VerifyingKeyBox,
        /// Packaged Reserved-lineage proving key archive.
        pub lineage_proving_key_archive: Vec<u8>,
    }

    /// Per-width prover artifacts for ABI-7 recursive compact tokens.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveCompactKeyArtifactEntryV1 {
        /// Pallas IPA opening vector length supported by this entry.
        pub verifier_opening_len: u32,
        /// Verifier key for one-hop compact recursive proofs.
        pub one_hop_verifier_key: VerifyingKeyBox,
        /// Proving key archive for one-hop compact recursive proofs.
        pub one_hop_proving_key_archive: Vec<u8>,
        /// Verifier key for append compact recursive proofs.
        pub append_verifier_key: VerifyingKeyBox,
        /// Proving key archive for append compact recursive proofs.
        pub append_proving_key_archive: Vec<u8>,
    }

    /// Portable prover key package for ABI-7 recursive compact tokens.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveCompactKeyArtifactsV1 {
        /// One or more supported recursive compact opening-length entries.
        pub entries: Vec<KagemushaRecursiveCompactKeyArtifactEntryV1>,
    }

    /// Per-width verifier keys for ABI-7 recursive compact tokens.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveCompactVerifierKeyEntryV1 {
        /// Pallas IPA opening vector length supported by this entry.
        pub verifier_opening_len: u32,
        /// Verifier key for one-hop compact recursive proofs.
        pub one_hop_verifier_key: VerifyingKeyBox,
        /// Verifier key for append compact recursive proofs.
        pub append_verifier_key: VerifyingKeyBox,
    }

    /// Portable verifier-key package for ABI-7 recursive compact tokens.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveCompactVerifierKeysV1 {
        /// One or more supported recursive compact opening-length entries.
        pub entries: Vec<KagemushaRecursiveCompactVerifierKeyEntryV1>,
    }

    /// Bridge request for the first recursive Kagemusha spendable state.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendInitRequestV1 {
        /// One-hop record-backed checked Kagemusha bundle.
        pub record_bundle: KagemushaVerifiedFoldRecordBundle,
        /// Norito archive of `Vec<iroha_zkp_halo2::OpenVerifyEnvelope>`.
        pub pallas_open_envelopes_archive: Vec<u8>,
        /// Spendable note created by the first hop.
        pub current_note: KagemushaSpendableNoteDescriptorV1,
        /// Optional packaged Reserved-lineage verifier key.
        ///
        /// Production SDKs should supply this key instead of asking the native
        /// bridge to synthesize the large recursive verifier-slice key at
        /// runtime. The field defaults to `None` so older ABI-6 request
        /// archives remain decodable.
        #[norito(default)]
        pub lineage_verifier_key: Option<VerifyingKeyBox>,
        /// Optional packaged Reserved-lineage proving key archive.
        ///
        /// The archive is circuit-family and verifier-key-commitment bound by
        /// the core prover before use. It defaults to `None` for legacy archive
        /// compatibility.
        #[norito(default)]
        pub lineage_proving_key_archive: Option<Vec<u8>>,
        /// Optional chain height used for verifier-record activation windows.
        ///
        /// When set, native bridge entrypoints enforce all record-backed proof
        /// checks at this exact height. When omitted, legacy callers remain
        /// decodable but fail closed for verifier records that declare
        /// activation or withdrawal windows.
        #[norito(default)]
        pub block_height: Option<u64>,
    }

    /// Bridge request for appending one offline hop to recursive Kagemusha cash.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendAppendRequestV1 {
        /// Previous spendable recursive state.
        pub previous_bundle: KagemushaRecursiveSpendBundleV1,
        /// One-hop record-backed checked Kagemusha bundle for the new hop.
        pub record_bundle: KagemushaVerifiedFoldRecordBundle,
        /// Norito archive of `Vec<iroha_zkp_halo2::OpenVerifyEnvelope>`.
        pub pallas_open_envelopes_archive: Vec<u8>,
        /// Spendable note created by the appended hop.
        pub current_note: KagemushaSpendableNoteDescriptorV1,
        /// Requested output recursive proof circuit id.
        ///
        /// Missing or empty values preserve legacy ABI-6 behavior and select the
        /// semantic `kagemusha-recursive-aggregation-v1` append output. Set this
        /// to `kagemusha-recursive-spend-lineage-v1` to attempt the
        /// Reserved-lineage output circuit; that selection requires
        /// `previous_recursive_proof_open_envelopes_archive`.
        #[norito(default)]
        pub output_proof_circuit_id: String,
        /// Optional verifier record for a previous reserved-lineage recursive proof.
        ///
        /// Semantic v1 previous proofs use the canonical recursive aggregation
        /// verifier and must leave this empty. Reserved-lineage previous proofs
        /// must provide the active lineage verifier record so append proving can
        /// verify the previous proof before folding the next hop. This field is
        /// defaulted so legacy ABI-6 semantic append archives decode as `None`.
        #[norito(default)]
        pub previous_lineage_verifier_record: Option<VerifyingKeyRecord>,
        /// Optional Norito archive of `Vec<iroha_zkp_halo2::OpenVerifyEnvelope>`
        /// for the previous recursive proof.
        ///
        /// This is reserved witness material for the production witnessless
        /// Reserved-lineage append circuit. Legacy semantic append callers leave
        /// it empty; when present it is decoded and shape-checked at the data-model
        /// boundary so SDKs cannot forward malformed previous-proof opening
        /// archives into the native prover.
        #[norito(default)]
        pub previous_recursive_proof_open_envelopes_archive: Vec<u8>,
        /// Optional packaged verifier key for a Reserved-lineage append output.
        ///
        /// This is used only when `output_proof_circuit_id` selects the
        /// Reserved-lineage append circuit. Semantic append requests leave it
        /// empty. The field is defaulted so legacy ABI-6 archives decode.
        #[norito(default)]
        pub lineage_verifier_key: Option<VerifyingKeyBox>,
        /// Optional packaged proving key archive for a Reserved-lineage append output.
        ///
        /// The archive is validated against `lineage_verifier_key` before proof
        /// generation. Semantic append requests leave it empty.
        #[norito(default)]
        pub lineage_proving_key_archive: Option<Vec<u8>>,
        /// Optional chain height used for current-hop and previous recursive
        /// verifier-record activation windows.
        #[norito(default)]
        pub block_height: Option<u64>,
    }

    /// Full record-backed lineage witness for online recursive spend redemption.
    ///
    /// This is chain-submitted audit material for the current production
    /// admission path. It is intentionally not part of the constant-size D2D
    /// recursive spend bundle; wallets only attach it when converting recursive
    /// offline cash back into public assets.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendLineageWitnessV1 {
        /// Ordered private hop proofs plus verifier records for the full lineage.
        pub record_bundle: KagemushaVerifiedFoldRecordBundle,
        /// Norito archive of `Vec<iroha_zkp_halo2::OpenVerifyEnvelope>`, one envelope per hop.
        pub pallas_open_envelopes_archive: Vec<u8>,
        /// Spendable note descriptor created by each hop, in lineage order.
        ///
        /// Each descriptor must point at an output commitment from its hop. Its
        /// spend nullifier must be unique across descriptors, disjoint from all
        /// lineage output commitments, and for the final descriptor disjoint
        /// from all lineage input nullifiers because that nullifier is consumed
        /// only at online redeem.
        pub current_notes: Vec<KagemushaSpendableNoteDescriptorV1>,
        /// Recursive proofs produced after each previous hop, in lineage order.
        ///
        /// For an `n`-hop bundle this contains `n - 1` proofs: proof `0` is
        /// bound to the accumulator after hop `0`, proof `1` to the accumulator
        /// after hop `1`, and so on. The final proof is carried by
        /// [`KagemushaRecursiveSpendBundleV1::recursive_proof`].
        pub previous_recursive_proofs: Vec<KagemushaRecursiveAggregationProof>,
    }

    /// Bridge request for verifying a recursive Kagemusha spend bundle.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyRequestV1 {
        /// Bundle to verify.
        pub bundle: KagemushaRecursiveSpendBundleV1,
        /// Active verifier record for reserved-lineage recursive spend proofs.
        ///
        /// Semantic v1 bundles use the canonical recursive aggregation verifier
        /// and must leave this empty. Reserved-lineage bundles must provide the
        /// active lineage verifier record so offline receivers can verify the
        /// constant-size D2D proof before accepting or re-spending the note.
        /// This field is defaulted so legacy ABI-6 semantic verify archives
        /// decode as `None`.
        #[norito(default)]
        pub lineage_verifier_record: Option<VerifyingKeyRecord>,
        /// Optional chain height used for lineage verifier-record activation windows.
        #[norito(default)]
        pub block_height: Option<u64>,
    }

    /// Bridge verification result for recursive Kagemusha spend bundles.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendVerifyResultV1 {
        /// True when all public bindings and backend proof verification passed.
        pub valid: bool,
        /// Hop count carried by the verified bundle.
        pub hop_count: u32,
        /// Norito encoded bundle length, used by SDK/CI payload-size checks.
        pub encoded_bytes: u32,
        /// Stable failure reason for diagnostics; empty on success.
        pub reason: String,
        /// True when the verified bundle is directly admissible for online redemption.
        ///
        /// Offline receivers should use [`Self::valid`] for accept/re-spend
        /// decisions. This field is stricter: current semantic recursive spend
        /// bundles can be offline-valid while still requiring record-backed
        /// lineage witness material at redeem time.
        pub chain_admissible: bool,
        /// Stable chain-admission diagnostic; empty when [`Self::chain_admissible`] is true.
        pub chain_admission_reason: String,
        /// True when this verified bundle can redeem without a record-backed lineage witness.
        #[norito(default)]
        pub witnessless_redeem_supported: bool,
        /// True when online redeem construction must attach a record-backed lineage witness.
        #[norito(default)]
        pub lineage_witness_required_for_redeem: bool,
    }

    /// Bridge request for preparing an online recursive Kagemusha redemption.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaRecursiveSpendRedeemRequestV1 {
        /// Final holder's recursive spend bundle.
        pub bundle: KagemushaRecursiveSpendBundleV1,
        /// Recipient public account to credit online.
        pub recipient: AccountId,
        /// Public amount to mint on redemption.
        pub public_amount: u128,
        /// Final unshield/redeem proof bound to the current note descriptor.
        pub redeem_proof: ProofAttachment,
        /// Optional record-backed lineage witness required for production minting.
        pub lineage_witness: Option<KagemushaRecursiveSpendLineageWitnessV1>,
        /// Optional private change note commitment for partial redemption.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::option")
        )]
        pub change_output: Option<[u8; 32]>,
        /// Active verifier record for Reserved-lineage recursive spend proofs.
        ///
        /// Semantic v1 final bundles use the canonical recursive aggregation
        /// verifier and normally leave this empty. A semantic final bundle may
        /// provide this only when its record-backed lineage witness contains
        /// prior Reserved-lineage recursive proofs that native hosts must verify
        /// before serializing an online redeem instruction. Reserved-lineage
        /// final bundles must provide the active lineage verifier record when
        /// their record-backed lineage witness contains Reserved-lineage proofs
        /// that native hosts must verify before serializing an online redeem
        /// instruction.
        /// This field is defaulted so legacy ABI-6 semantic redeem archives
        /// decode as `None`.
        #[norito(default)]
        pub lineage_verifier_record: Option<VerifyingKeyRecord>,
        /// Optional chain height used for wallet-side bridge verification of
        /// lineage witnesses and reserved-lineage final proofs.
        #[norito(default)]
        pub block_height: Option<u64>,
    }

    /// One private hop folded into a compact Kagemusha payment token.
    ///
    /// These steps are prover/wallet witness material. The chain-visible compact token exposes
    /// only [`KagemushaFoldedPublicInputs`] plus the transparent folded proof. The folded
    /// transcript binds the proof payload, the public input statement that was verified for that
    /// payload, and the verifier-key identity/commitment that was used to verify that payload.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaFoldStep {
        /// Recent shielded Merkle root before this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_before: [u8; 32],
        /// Input nullifiers consumed by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub input_nullifiers: Vec<[u8; 32]>,
        /// Output note commitments created by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub output_commitments: Vec<[u8; 32]>,
        /// Shielded Merkle root after this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_after: [u8; 32],
        /// Domain-separated hash of the transparent per-hop proof payload.
        pub proof_hash: Hash,
        /// Poseidon2 digest of the per-hop proof public input statement.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub proof_public_inputs_digest: [u8; 32],
        /// Verifier key identifier used to verify the per-hop proof.
        pub verifier_key_id: VerifyingKeyId,
        /// Commitment of the verifier-key bytes used to verify the per-hop proof.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_commitment: [u8; 32],
        /// Poseidon2 digest of the verifier-key bytes used to verify the per-hop proof.
        ///
        /// This is redundant with [`Self::verifier_key_commitment`] for host checks, but gives
        /// future recursive verifier circuits a hash-friendly public verifier-key binding without
        /// relying on the host hash function inside the circuit.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub verifier_key_poseidon_digest: [u8; 32],
    }

    /// One private hop proof plus the verifier key needed for checked compact-token proving.
    ///
    /// This is wallet/prover input material, not a chain-visible token field. Bridge callers use
    /// it to make the prover verify each hop proof before deriving [`KagemushaFoldedPublicInputs`].
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaVerifiedFoldStep {
        /// Recent shielded Merkle root before this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_before: [u8; 32],
        /// Input nullifiers consumed by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub input_nullifiers: Vec<[u8; 32]>,
        /// Output note commitments created by this private hop.
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_bytes::vec")
        )]
        pub output_commitments: Vec<[u8; 32]>,
        /// Shielded Merkle root after this private hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub root_after: [u8; 32],
        /// Transparent proof attachment that must verify before this hop can be folded.
        pub attachment: ProofAttachment,
        /// Verifier key used to verify [`Self::attachment`].
        pub verifier_key: VerifyingKeyBox,
    }

    /// Checked Kagemusha compact-token proving input.
    ///
    /// Provers and mobile bridges should prefer this bundle over prebuilt folded public inputs.
    /// It lets the prover verify every private hop proof, bind the actual verifier-key bytes, and
    /// only then emit the compact folded token.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaVerifiedFoldBundle {
        /// Chain id that scopes the folded token.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Ordered private hop proofs to verify and fold.
        pub steps: Vec<KagemushaVerifiedFoldStep>,
    }

    /// Verifier registry record supplied for one checked Kagemusha private hop.
    ///
    /// This is the serializable bridge/wallet form of the WSV lookup result:
    /// `id` is the registry key referenced by a hop proof attachment, and
    /// `record` is the governance-managed verifier metadata for that key.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaVerifiedFoldVerifierRecord {
        /// Verifier-key id referenced by a hop proof attachment.
        pub id: VerifyingKeyId,
        /// Governance-managed verifier metadata for [`Self::id`].
        pub record: VerifyingKeyRecord,
    }

    /// Checked Kagemusha compact-token proving input with verifier records.
    ///
    /// Mobile bridges and WSV-backed prover services should prefer this bundle
    /// when they can fetch verifier records. It lets the prover enforce active
    /// verifier metadata before deriving folded public inputs.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaVerifiedFoldRecordBundle {
        /// Private hop proofs to verify and fold.
        pub bundle: KagemushaVerifiedFoldBundle,
        /// Verifier records referenced by the private hop proofs.
        pub verifier_records: Vec<KagemushaVerifiedFoldVerifierRecord>,
    }

    /// Chain-verifiable public inputs for one compact, folded Kagemusha token.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaFoldedPublicInputs {
        /// Domain separator for folded public inputs.
        pub domain: String,
        /// Aggregation mode proved by the folded circuit.
        ///
        /// `1` is the current checked pre-fold mode, where wallet/prover code verifies each
        /// private hop before building the compact folded transcript. Future recursive modes must
        /// use a new supported value and verifier circuit.
        pub aggregation_mode: u16,
        /// Chain id that scopes the folded token.
        pub chain_id: ChainId,
        /// Shielded asset definition id.
        pub asset: AssetDefinitionId,
        /// Root before the first folded hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub initial_root: [u8; 32],
        /// Root after the final folded hop.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub final_root: [u8; 32],
        /// Number of private hops folded into the compact proof.
        pub hop_count: u32,
        /// Canonical digest of all folded input nullifiers.
        pub nullifier_digest: Hash,
        /// Canonical digest of all folded output commitments.
        pub output_commitment_digest: Hash,
        /// Canonical digest of the ordered folded-hop transcript.
        pub fold_digest: Hash,
        /// Poseidon2 digest of the ordered folded-hop aggregation transcript.
        ///
        /// This is kept separate from [`Self::fold_digest`], which preserves the ordinary Iroha
        /// hash commitment used by existing host code. Recursive Kagemusha circuits should use
        /// this field as their hash-friendly public accumulator.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub aggregation_transcript_digest: [u8; 32],
    }

    /// Verifier-key-backed transparent proof for a compact folded Kagemusha token.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaFoldedProof {
        /// Stable verifier key identifier selected by the operator and stored in WSV.
        pub verifier_key_id: VerifyingKeyId,
        /// Public input commitment hash.
        pub public_inputs_hash: Hash,
        /// Compact folded proof payload encoded as a transparent `OpenVerifyEnvelope`.
        pub proof: ProofBox,
    }

    /// Compact multi-hop Kagemusha payment token.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct KagemushaCompactPaymentToken {
        /// Chain-visible folded public inputs.
        pub public_inputs: KagemushaFoldedPublicInputs,
        /// Transparent folded proof bound to `public_inputs`.
        pub folded_proof: KagemushaFoldedProof,
    }
}

/// Origin of a wallet-derived Offline Note note commitment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum OfflineNoteCommitmentOrigin {
    /// Note created by an issuer load operation.
    IssuerLoad(OfflineNoteIssuerLoadOrigin),
    /// Note created as an output of an offline peer-to-peer payment token.
    P2pOutput(OfflineNoteP2pOutputOrigin),
}

const OFFLINE_NOTE_KEY_CERTIFICATE_PAYLOAD_DOMAIN: &str =
    "iroha:offline-note:key-certificate-payload";
const OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN: &str = "iroha:offline-note:issued-claim";
const OFFLINE_NOTE_REDEEM_PUBLIC_INPUTS_DOMAIN: &str = "iroha:offline-note:redeem-public-inputs";
const OFFLINE_NOTE_AUDIT_PUBLIC_INPUTS_DOMAIN: &str = "iroha:offline-note:audit-public-inputs";
const KAGEMUSHA_FOLD_STEP_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:fold-step";
const KAGEMUSHA_FOLD_NULLIFIER_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:nullifiers";
const KAGEMUSHA_FOLD_OUTPUT_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:outputs";
const KAGEMUSHA_FOLD_TRANSCRIPT_DIGEST_DOMAIN: &str = "iroha:kagemusha:v1:fold-transcript";
const KAGEMUSHA_RECURSIVE_SPEND_NULLIFIER_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-nullifiers";
const KAGEMUSHA_RECURSIVE_SPEND_OUTPUT_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-outputs";
const KAGEMUSHA_RECURSIVE_SPEND_FOLD_DIGEST_DOMAIN: &str =
    "iroha:kagemusha:v1:recursive-spend-fold-transcript";
/// Canonical public-input schema descriptor for Offline recursive note proofs.
pub const OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"offline_note_recursive","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","proof_mode","input_count","output_count","input_amount_sum","output_amount_sum","input_nullifier_sum_limb0","output_commitment_sum_limb0","key_certificate_payload_hash_limb0","source_or_token_limb0","input_claim_hash_sum_limb0","output_claim_hash_sum_limb0","reserved_zero"]}"#;
/// Canonical public-input schema descriptor for Kagemusha folded proofs.
pub const KAGEMUSHA_FOLDED_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"kagemusha_folded_v1","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","aggregation_mode","hop_count","initial_root_limb0","initial_root_limb1","initial_root_limb2","initial_root_limb3","final_root_limb0","final_root_limb1","final_root_limb2","final_root_limb3","nullifier_digest_limb0","nullifier_digest_limb1","nullifier_digest_limb2","nullifier_digest_limb3","output_commitment_digest_limb0","output_commitment_digest_limb1","output_commitment_digest_limb2","output_commitment_digest_limb3","fold_digest_limb0","fold_digest_limb1","fold_digest_limb2","fold_digest_limb3","aggregation_transcript_digest_limb0","aggregation_transcript_digest_limb1","aggregation_transcript_digest_limb2","aggregation_transcript_digest_limb3"]}"#;
/// Canonical public-input schema descriptor for Kagemusha recursive aggregation proofs.
pub const KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA: &[u8] = br#"{"schema":"kagemusha_recursive_aggregation_proof_v1","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","evidence_digest_limb0","evidence_digest_limb1","evidence_digest_limb2","evidence_digest_limb3","folded_public_inputs_hash_limb0","folded_public_inputs_hash_limb1","folded_public_inputs_hash_limb2","folded_public_inputs_hash_limb3","aggregation_transcript_digest_limb0","aggregation_transcript_digest_limb1","aggregation_transcript_digest_limb2","aggregation_transcript_digest_limb3","verifier_params_fingerprint_limb0","verifier_params_fingerprint_limb1","verifier_params_fingerprint_limb2","verifier_params_fingerprint_limb3","fixed_window_table_schedule_digest_limb0","fixed_window_table_schedule_digest_limb1","fixed_window_table_schedule_digest_limb2","fixed_window_table_schedule_digest_limb3","fixed_window_shared_table_manifest_digest_limb0","fixed_window_shared_table_manifest_digest_limb1","fixed_window_shared_table_manifest_digest_limb2","fixed_window_shared_table_manifest_digest_limb3","fixed_window_table_base_digest_limb0","fixed_window_table_base_digest_limb1","fixed_window_table_base_digest_limb2","fixed_window_table_base_digest_limb3","verifier_witness_batch_digest_limb0","verifier_witness_batch_digest_limb1","verifier_witness_batch_digest_limb2","verifier_witness_batch_digest_limb3","recursive_proof_chain_digest_limb0","recursive_proof_chain_digest_limb1","recursive_proof_chain_digest_limb2","recursive_proof_chain_digest_limb3","transition_profile_binding_digest_limb0","transition_profile_binding_digest_limb1","transition_profile_binding_digest_limb2","transition_profile_binding_digest_limb3","append_opening_preflight_digest_limb0","append_opening_preflight_digest_limb1","append_opening_preflight_digest_limb2","append_opening_preflight_digest_limb3","append_boundary_digest_limb0","append_boundary_digest_limb1","append_boundary_digest_limb2","append_boundary_digest_limb3","recursive_verifier_scalar_projection_digest_limb0","recursive_verifier_scalar_projection_digest_limb1","recursive_verifier_scalar_projection_digest_limb2","recursive_verifier_scalar_projection_digest_limb3","verifier_opening_len","verifier_witness_count","hop_count"]}"#;

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaFoldStepDigestPreimage {
    domain: String,
    hop_index: u32,
    root_before: [u8; Hash::LENGTH],
    input_nullifiers: Vec<[u8; Hash::LENGTH]>,
    output_commitments: Vec<[u8; Hash::LENGTH]>,
    root_after: [u8; Hash::LENGTH],
    proof_hash: Hash,
    proof_public_inputs_digest: [u8; Hash::LENGTH],
    verifier_key_id: VerifyingKeyId,
    verifier_key_commitment: [u8; Hash::LENGTH],
    verifier_key_poseidon_digest: [u8; Hash::LENGTH],
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaProofPublicInputsDigestPreimage {
    domain: String,
    statement: KagemushaProofPublicInputsStatement,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaVerifierKeyDigestPreimage {
    domain: String,
    backend: String,
    verifier_key_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaFoldListDigestPreimage {
    domain: String,
    values: Vec<[u8; Hash::LENGTH]>,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaFoldTranscriptDigestPreimage {
    domain: String,
    chain_id: ChainId,
    asset: AssetDefinitionId,
    step_digests: Vec<Hash>,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaPoseidonAggregationTranscriptPreimage {
    domain: String,
    statement: KagemushaPoseidonAggregationTranscriptStatement,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveAggregationEvidencePreimage {
    domain: String,
    evidence: KagemushaRecursiveAggregationEvidence,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendAccumulatorDigestPreimage {
    domain: String,
    accumulator: KagemushaRecursiveSpendAccumulatorV1,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendLineageDigestPreimage {
    domain: String,
    previous_lineage_digest: Option<[u8; Hash::LENGTH]>,
    chain_id: ChainId,
    asset: AssetDefinitionId,
    hop_index: u32,
    step: KagemushaPoseidonAggregationStepStatement,
    current_note: KagemushaSpendableNoteDescriptorV1,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendVerifierBatchDigestPreimage {
    domain: String,
    previous_verifier_witness_batch_digest: Option<[u8; Hash::LENGTH]>,
    hop_index: u32,
    hop_verifier_witness_batch_digest: [u8; Hash::LENGTH],
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendFixedWindowTableBaseDigestPreimage {
    domain: String,
    previous_fixed_window_table_base_digest: Option<[u8; Hash::LENGTH]>,
    hop_index: u32,
    hop_fixed_window_table_base_digest: [u8; Hash::LENGTH],
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendProofArtifactDigestPreimage {
    domain: String,
    recursive_proof: KagemushaRecursiveAggregationProof,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendProofChainDigestPreimage {
    domain: String,
    previous_recursive_proof_chain_digest: Option<[u8; Hash::LENGTH]>,
    previous_recursive_proof_artifact_digest: Option<[u8; Hash::LENGTH]>,
    previous_recursive_proof_public_inputs_hash: Option<Hash>,
    hop_index: u32,
    current_hop_proof_hash: Hash,
    current_hop_proof_public_inputs_digest: [u8; Hash::LENGTH],
    current_hop_verifier_witness_batch_digest: [u8; Hash::LENGTH],
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendTransitionProfileDigestPreimage {
    domain: String,
    profile: KagemushaRecursiveSpendTransitionProfileV1,
}

const KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_PENDING: [u8; Hash::LENGTH] =
    [0xA5; Hash::LENGTH];

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursivePreviousProofOpenEnvelopesArchiveDigestPreimage {
    domain: String,
    archive: Vec<u8>,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendLineageAppendOpeningPreflightDigestPreimage {
    domain: String,
    previous_recursive_proof_preflight: KagemushaRecursiveVerifierPreflightV1,
    current_hop_preflight: KagemushaRecursiveVerifierPreflightV1,
    previous_accumulator_digest: [u8; Hash::LENGTH],
    previous_recursive_proof_artifact_digest: [u8; Hash::LENGTH],
    previous_recursive_proof_open_envelopes_archive_digest: [u8; Hash::LENGTH],
    current_hop_proof_hash: Hash,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendLineageAppendBoundaryDigestPreimage {
    domain: String,
    boundary: KagemushaRecursiveSpendLineageAppendBoundaryV1,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendLineageAppendBoundaryChainAssetBindingDigestPreimage {
    domain: String,
    chain_id: ChainId,
    asset: AssetDefinitionId,
}

#[derive(Debug, Clone, Decode, Encode)]
struct KagemushaRecursiveSpendLineageAppendBoundaryFinalNoteBindingDigestPreimage {
    domain: String,
    final_root: [u8; Hash::LENGTH],
    current_note: KagemushaSpendableNoteDescriptorV1,
}

/// Return the registry schema hash required for Offline recursive note verifiers.
#[must_use]
pub fn offline_note_recursive_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    Hash::new(OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA).into()
}

/// Return the registry schema hash required for Offline V2 recursive note verifiers.
#[must_use]
pub fn offline_note_v2_recursive_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    offline_note_recursive_public_inputs_schema_hash()
}

/// Return the registry schema hash required for Kagemusha folded proof verifiers.
#[must_use]
pub fn kagemusha_folded_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    Hash::new(KAGEMUSHA_FOLDED_PUBLIC_INPUTS_SCHEMA).into()
}

/// Return the registry schema hash required for Kagemusha recursive aggregation proof verifiers.
#[must_use]
pub fn kagemusha_recursive_aggregation_proof_public_inputs_schema_hash() -> [u8; Hash::LENGTH] {
    Hash::new(KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA).into()
}

impl From<&OfflineNoteKeyCertificate> for OfflineNoteKeyCertificatePayload {
    fn from(certificate: &OfflineNoteKeyCertificate) -> Self {
        Self {
            domain: OFFLINE_NOTE_KEY_CERTIFICATE_PAYLOAD_DOMAIN.to_owned(),
            version: certificate.version,
            platform: certificate.platform.clone(),
            key_id: certificate.key_id.clone(),
            device_id: certificate.device_id.clone(),
            account_id: certificate.account_id.clone(),
            public_key: certificate.public_key.clone(),
            assertion_scheme: certificate.assertion_scheme.clone(),
            assertion_key_algorithm: certificate.assertion_key_algorithm.clone(),
            assertion_public_key: certificate.assertion_public_key.clone(),
            assertion_usage_count_limit: certificate.assertion_usage_count_limit,
            one_use: certificate.one_use,
        }
    }
}

impl OfflineNoteKeyCertificate {
    /// Canonical payload bytes signed by the Offline certificate issuer.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn signing_bytes(&self) -> Result<Vec<u8>, norito::Error> {
        let payload = OfflineNoteKeyCertificatePayload::from(self);
        to_bytes(&payload)
    }

    /// Deterministic hash of the canonical certificate payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the payload cannot be serialized with Norito.
    pub fn payload_hash(&self) -> Result<Hash, norito::Error> {
        self.signing_bytes().map(Hash::new)
    }
}

impl OfflineNoteIssuedClaim {
    /// Build the claim recorded when an Offline note is issued.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_issue(issue: &OfflineNoteIssue) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: issue.note_commitment,
            key_certificate_payload_hash: issue.key_certificate.payload_hash()?,
            asset: issue.asset.clone(),
            amount: issue.amount.clone(),
        })
    }

    /// Build the claim expected when an Offline note is redeemed.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_redemption(redemption: &OfflineNoteRedeem) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: redemption.source_note_commitment,
            key_certificate_payload_hash: redemption.sender_key_certificate.payload_hash()?,
            asset: redemption.asset.clone(),
            amount: redemption.amount.clone(),
        })
    }

    /// Build the claim recorded when an Offline audited output is accepted.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_audit_output(output: &OfflineNoteAuditOutputClaim) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_ISSUED_CLAIM_DOMAIN.to_owned(),
            note_commitment: output.note_commitment,
            key_certificate_payload_hash: output.key_certificate.payload_hash()?,
            asset: output.asset.clone(),
            amount: output.amount.clone(),
        })
    }

    /// Deterministic hash of the issued-note claim.
    ///
    /// # Errors
    ///
    /// Returns an error when the claim cannot be serialized with Norito.
    pub fn claim_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteRedeemPublicInputs {
    /// Build the public inputs committed by an Offline redemption proof.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_redemption(redemption: &OfflineNoteRedeem) -> Result<Self, norito::Error> {
        Ok(Self {
            domain: OFFLINE_NOTE_REDEEM_PUBLIC_INPUTS_DOMAIN.to_owned(),
            source_note_commitment: redemption.source_note_commitment,
            input_nullifiers: redemption.input_nullifiers.clone(),
            key_certificate_payload_hash: redemption.sender_key_certificate.payload_hash()?,
            recipient: redemption.recipient.clone(),
            asset: redemption.asset.clone(),
            amount: redemption.amount.clone(),
        })
    }

    /// Deterministic hash of the redemption public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public inputs cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteAuditPublicInputs {
    /// Build the public inputs committed by an Offline optional audit proof.
    ///
    /// # Errors
    ///
    /// Returns an error when the certificate payload cannot be serialized.
    pub fn from_audit(audit: &OfflineNoteAuditBundle) -> Result<Self, norito::Error> {
        let output_claims = audit
            .output_claims
            .iter()
            .map(OfflineNoteIssuedClaim::from_audit_output)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            domain: OFFLINE_NOTE_AUDIT_PUBLIC_INPUTS_DOMAIN.to_owned(),
            token_id: audit.token_id,
            key_certificate_payload_hash: audit.sender_key_certificate.payload_hash()?,
            input_nullifiers: audit.input_nullifiers.clone(),
            input_claims: audit.input_claims.clone(),
            output_commitments: audit.output_commitments.clone(),
            output_claims,
        })
    }

    /// Deterministic hash of the audit public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public inputs cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl OfflineNoteAuditBundle {
    /// Deterministic hash that the optional audit proof must expose as public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        OfflineNoteAuditPublicInputs::from_audit(self)?.public_inputs_hash()
    }
}

impl OfflineNoteRedeem {
    /// Deterministic hash that the recursive proof must expose as public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        OfflineNoteRedeemPublicInputs::from_redemption(self)?.public_inputs_hash()
    }
}

fn kagemusha_hash_preimage<T: Encode>(value: &T) -> Result<Hash, KagemushaFoldError> {
    Ok(Hash::new(to_bytes(value)?))
}

fn kagemusha_poseidon_preimage<T: Encode>(
    value: &T,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    let bytes = to_bytes(value)?;
    Ok(iroha_zkp_halo2::poseidon::hash_bytes(&bytes))
}

/// Return the canonical Poseidon2 digest for a Kagemusha proof public-input statement.
///
/// The digest is domain-separated from the folded-hop transcript and commits to the transparent
/// envelope metadata plus the exact backend-native public instance columns verified for one hop.
/// Kagemusha fold statements must carry a non-zero verifier-key hash and empty auxiliary bytes.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the statement is non-canonical or cannot be encoded with
/// Norito.
pub fn kagemusha_proof_public_inputs_statement_digest(
    statement: &KagemushaProofPublicInputsStatement,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    let Some(expected_tag) = kagemusha_backend_tag(&statement.proof_backend) else {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: statement.proof_backend.clone(),
        });
    };
    if statement.envelope_backend != expected_tag {
        return Err(KagemushaFoldError::ProofStatementBackendTagMismatch {
            proof_backend: statement.proof_backend.clone(),
            envelope_backend: statement.envelope_backend,
        });
    }
    if statement.vk_hash == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroProofStatementVerifierKeyHash);
    }
    if statement.circuit_id.trim().is_empty() {
        return Err(KagemushaFoldError::EmptyProofStatementCircuitId);
    }
    if statement.public_inputs_schema.is_empty() {
        return Err(KagemushaFoldError::EmptyProofStatementPublicInputsSchema);
    }
    if statement.instance_columns.is_empty() {
        return Err(KagemushaFoldError::EmptyProofStatementInstanceColumns);
    }
    if let Some(column_index) = statement
        .instance_columns
        .iter()
        .position(std::vec::Vec::is_empty)
    {
        return Err(KagemushaFoldError::EmptyProofStatementInstanceColumn { column_index });
    }
    if !statement.envelope_aux.is_empty() {
        return Err(
            KagemushaFoldError::NonCanonicalProofStatementAuxiliaryBytes {
                actual: statement.envelope_aux.len(),
            },
        );
    }
    kagemusha_poseidon_preimage(&KagemushaProofPublicInputsDigestPreimage {
        domain: KAGEMUSHA_PROOF_PUBLIC_INPUTS_DIGEST_DOMAIN.to_owned(),
        statement: statement.clone(),
    })
}

/// Return the canonical Poseidon2 digest for a Kagemusha verifier key.
///
/// The digest is domain-separated from folded-hop and proof-statement digests
/// and commits to the backend label plus the exact verifier-key bytes used for
/// one hop proof verification.
///
/// # Errors
///
/// Returns [`KagemushaFoldError::Encode`] when the digest preimage cannot be
/// encoded with Norito.
pub fn kagemusha_verifier_key_poseidon_digest(
    backend: impl Into<String>,
    verifier_key_bytes: &[u8],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    let backend = backend.into();
    if !is_supported_kagemusha_proof_backend(&backend) {
        return Err(KagemushaFoldError::UnsupportedProofBackend { backend });
    }
    if verifier_key_bytes.is_empty() {
        return Err(KagemushaFoldError::EmptyVerifierKeyBytes { backend });
    }
    kagemusha_poseidon_preimage(&KagemushaVerifierKeyDigestPreimage {
        domain: KAGEMUSHA_VERIFIER_KEY_DIGEST_DOMAIN.to_owned(),
        backend,
        verifier_key_bytes: verifier_key_bytes.to_vec(),
    })
}

fn kagemusha_verifying_key_commitment(vk: &VerifyingKeyBox) -> [u8; Hash::LENGTH] {
    let backend_len = u64::try_from(vk.backend.len()).expect("backend length fits u64");
    let bytes_len = u64::try_from(vk.bytes.len()).expect("verifying-key length fits u64");
    let mut hash = Sha256::new();
    hash.update(b"iroha:zk:v1:vk");
    hash.update(backend_len.to_be_bytes());
    hash.update(vk.backend.as_bytes());
    hash.update(bytes_len.to_be_bytes());
    hash.update(vk.bytes.as_slice());
    hash.finalize().into()
}

/// Return the canonical Poseidon2 digest for a Kagemusha aggregation transcript statement.
///
/// This is the hash-friendly public accumulator that recursive verifier circuits
/// recompute from their private per-hop witness. It is
/// domain-separated from proof-statement, verifier-key, and host-side folded-hop
/// hashes. The digest accepts both checked pre-fold and reserved recursive
/// aggregation modes so recursive evidence can bind the same transcript shape.
/// The legacy checked pre-fold validator still rejects mode `2` through
/// [`KagemushaFoldedPublicInputs::validate_supported_context`]; ABI-7 recursive
/// compact admission uses the narrower recursive-compact context and projection
/// validators.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the statement is non-canonical or cannot
/// be encoded with Norito.
pub fn kagemusha_poseidon_aggregation_transcript_digest(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    kagemusha_poseidon_aggregation_transcript_shape_digest(statement)
}

fn kagemusha_poseidon_aggregation_transcript_shape_digest(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_hashable_aggregation_transcript_statement(statement)?;
    kagemusha_poseidon_preimage(&KagemushaPoseidonAggregationTranscriptPreimage {
        domain: KAGEMUSHA_POSEIDON_AGGREGATION_TRANSCRIPT_DOMAIN.to_owned(),
        statement: statement.clone(),
    })
}

fn kagemusha_list_digest(
    domain: &str,
    values: Vec<[u8; Hash::LENGTH]>,
) -> Result<Hash, KagemushaFoldError> {
    kagemusha_hash_preimage(&KagemushaFoldListDigestPreimage {
        domain: domain.to_owned(),
        values,
    })
}

fn validate_kagemusha_step_shape_and_sets(
    hop_index: usize,
    input_nullifiers: &[[u8; Hash::LENGTH]],
    output_commitments: &[[u8; Hash::LENGTH]],
) -> Result<(), KagemushaFoldError> {
    if input_nullifiers.is_empty()
        || input_nullifiers.len() > KAGEMUSHA_FOLD_STEP_MAX_INPUTS
        || output_commitments.is_empty()
        || output_commitments.len() > KAGEMUSHA_FOLD_STEP_MAX_OUTPUTS
    {
        return Err(KagemushaFoldError::InvalidStepShape {
            hop_index,
            input_count: input_nullifiers.len(),
            output_count: output_commitments.len(),
        });
    }
    if input_nullifiers.contains(&[0u8; Hash::LENGTH]) {
        return Err(KagemushaFoldError::ZeroInputNullifier { hop_index });
    }
    if output_commitments.contains(&[0u8; Hash::LENGTH]) {
        return Err(KagemushaFoldError::ZeroOutputCommitment { hop_index });
    }
    Ok(())
}

fn validate_kagemusha_canonical_set_order(
    hop_index: usize,
    input_nullifiers: &[[u8; Hash::LENGTH]],
    output_commitments: &[[u8; Hash::LENGTH]],
) -> Result<(), KagemushaFoldError> {
    if input_nullifiers.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(KagemushaFoldError::NonCanonicalInputNullifierOrder { hop_index });
    }
    if output_commitments.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(KagemushaFoldError::NonCanonicalOutputCommitmentOrder { hop_index });
    }
    Ok(())
}

fn validate_kagemusha_step_digest_bindings(
    hop_index: usize,
    proof_public_inputs_digest: [u8; Hash::LENGTH],
    verifier_key_commitment: [u8; Hash::LENGTH],
    verifier_key_poseidon_digest: [u8; Hash::LENGTH],
) -> Result<(), KagemushaFoldError> {
    if proof_public_inputs_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroProofPublicInputsDigest { hop_index });
    }
    if verifier_key_commitment == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index });
    }
    if verifier_key_poseidon_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroVerifierKeyPoseidonDigest { hop_index });
    }
    Ok(())
}

fn validate_kagemusha_fold_root(
    field: &'static str,
    root: [u8; Hash::LENGTH],
) -> Result<(), KagemushaFoldError> {
    if root == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroFoldedRoot { field });
    }
    Ok(())
}

fn validate_kagemusha_root_transition(
    hop_index: usize,
    root_before: [u8; Hash::LENGTH],
    root_after: [u8; Hash::LENGTH],
) -> Result<(), KagemushaFoldError> {
    if root_before == root_after {
        return Err(KagemushaFoldError::UnchangedFoldedRootTransition { hop_index });
    }
    Ok(())
}

fn validate_kagemusha_verifier_key_id(
    hop_index: usize,
    verifier_key_id: &VerifyingKeyId,
) -> Result<(), KagemushaFoldError> {
    if verifier_key_id.name.trim().is_empty() {
        return Err(KagemushaFoldError::EmptyVerifierKeyIdName { hop_index });
    }
    if !is_supported_kagemusha_proof_backend(&verifier_key_id.backend) {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: verifier_key_id.backend.clone(),
        });
    }
    Ok(())
}

fn validate_kagemusha_unique_input_output_sets(
    hop_index: usize,
    input_nullifiers: &[[u8; Hash::LENGTH]],
    output_commitments: &[[u8; Hash::LENGTH]],
) -> Result<(), KagemushaFoldError> {
    let mut inputs = std::collections::BTreeSet::new();
    for input in input_nullifiers {
        if !inputs.insert(*input) {
            return Err(KagemushaFoldError::DuplicateInputNullifier { hop_index });
        }
    }

    let mut outputs = std::collections::BTreeSet::new();
    for output in output_commitments {
        if inputs.contains(output) {
            return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
        }
        if !outputs.insert(*output) {
            return Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index });
        }
    }
    Ok(())
}

fn validate_kagemusha_hashable_aggregation_transcript_statement(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<(), KagemushaFoldError> {
    match statement.aggregation_mode {
        KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        | KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1 => {}
        actual => {
            return Err(KagemushaFoldError::UnsupportedAggregationMode {
                expected: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
                actual,
                reason: unsupported_kagemusha_aggregation_mode_reason(actual),
            });
        }
    }
    validate_kagemusha_aggregation_transcript_statement_shape(statement)
}

fn validate_kagemusha_aggregation_transcript_statement_shape(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<(), KagemushaFoldError> {
    if statement.steps.is_empty() {
        return Err(KagemushaFoldError::Empty);
    }
    if statement.steps.len() > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: statement.steps.len(),
        });
    }
    if usize::try_from(statement.hop_count).ok() != Some(statement.steps.len()) {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: statement.steps.len(),
            actual: statement.hop_count,
        });
    }

    let first = statement.steps.first().expect("validated non-empty steps");
    validate_kagemusha_fold_root("initial_root", statement.initial_root)?;
    validate_kagemusha_fold_root("final_root", statement.final_root)?;
    if statement.initial_root == statement.final_root {
        return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
    }
    if statement.initial_root != first.root_before {
        return Err(KagemushaFoldError::InitialRootMismatch {
            expected: first.root_before,
            actual: statement.initial_root,
        });
    }

    let mut expected_root = statement.initial_root;
    let mut all_inputs = std::collections::BTreeSet::new();
    let mut all_outputs = std::collections::BTreeSet::new();
    for (hop_index, step) in statement.steps.iter().enumerate() {
        if step.hop_index != u32::try_from(hop_index).expect("hop count is bounded to u32") {
            return Err(KagemushaFoldError::HopIndexMismatch {
                expected: hop_index,
                actual: step.hop_index,
            });
        }
        validate_kagemusha_fold_root("root_before", step.root_before)?;
        validate_kagemusha_fold_root("root_after", step.root_after)?;
        validate_kagemusha_root_transition(hop_index, step.root_before, step.root_after)?;
        validate_kagemusha_verifier_key_id(hop_index, &step.verifier_key_id)?;
        validate_kagemusha_step_shape_and_sets(
            hop_index,
            &step.input_nullifiers,
            &step.output_commitments,
        )?;
        validate_kagemusha_canonical_set_order(
            hop_index,
            &step.input_nullifiers,
            &step.output_commitments,
        )?;
        validate_kagemusha_step_digest_bindings(
            hop_index,
            step.proof_public_inputs_digest,
            step.verifier_key_commitment,
            step.verifier_key_poseidon_digest,
        )?;
        if step.root_before != expected_root {
            return Err(KagemushaFoldError::RootDiscontinuity {
                hop_index,
                expected: expected_root,
                actual: step.root_before,
            });
        }
        for nullifier in &step.input_nullifiers {
            if all_outputs.contains(nullifier) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
            if !all_inputs.insert(*nullifier) {
                return Err(KagemushaFoldError::DuplicateInputNullifier { hop_index });
            }
        }
        for commitment in &step.output_commitments {
            if all_inputs.contains(commitment) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
            if !all_outputs.insert(*commitment) {
                return Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index });
            }
        }
        expected_root = step.root_after;
    }

    if statement.final_root != expected_root {
        return Err(KagemushaFoldError::FinalRootMismatch {
            expected: expected_root,
            actual: statement.final_root,
        });
    }
    Ok(())
}

/// Validate reserved-mode recursive aggregation evidence.
///
/// This checks only the canonical host-side evidence shape. It does not make
/// aggregation mode `2` supported for compact-token admission in this release.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the evidence does not declare reserved
/// recursive mode `2`, its hop transcript is non-canonical, its witness count
/// does not match the hop count, its verifier-witness profile or opening length
/// is unsupported, or its verifier parameter, schedule, shared-table manifest,
/// table-base, or batch digest fields are all-zero.
pub fn validate_kagemusha_recursive_aggregation_evidence(
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<(), KagemushaFoldError> {
    if evidence.aggregation_statement.aggregation_mode
        != KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
    {
        return Err(
            KagemushaFoldError::RecursiveAggregationEvidenceModeMismatch {
                expected: KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1,
                actual: evidence.aggregation_statement.aggregation_mode,
            },
        );
    }
    validate_kagemusha_aggregation_transcript_statement_shape(&evidence.aggregation_statement)?;
    if evidence.verifier_witness_count != evidence.aggregation_statement.hop_count {
        return Err(
            KagemushaFoldError::RecursiveAggregationWitnessCountMismatch {
                expected: evidence.aggregation_statement.hop_count,
                actual: evidence.verifier_witness_count,
            },
        );
    }
    if evidence.verifier_witness_profile != KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1 {
        return Err(
            KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile {
                expected: KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1,
                actual: evidence.verifier_witness_profile.clone(),
            },
        );
    }
    validate_kagemusha_recursive_verifier_opening_len(evidence.verifier_opening_len)?;
    if evidence.verifier_params_fingerprint == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveVerifierParamsFingerprint);
    }
    if evidence.fixed_window_table_schedule_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableScheduleDigest);
    }
    if evidence.fixed_window_shared_table_manifest_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveFixedWindowSharedTableManifestDigest);
    }
    if evidence.fixed_window_table_base_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableBaseDigest);
    }
    if evidence.verifier_witness_batch_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest);
    }
    Ok(())
}

/// Validate the Pallas IPA opening length accepted by reserved recursive evidence.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when `opening_len` is outside the bounded
/// power-of-two corridor used by the first recursive verifier profile.
pub fn validate_kagemusha_recursive_verifier_opening_len(
    opening_len: u32,
) -> Result<(), KagemushaFoldError> {
    if !(KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MIN_LEN
        ..=KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MAX_LEN)
        .contains(&opening_len)
    {
        return Err(
            KagemushaFoldError::UnsupportedRecursiveVerifierOpeningLength {
                min: KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MIN_LEN,
                max: KAGEMUSHA_RECURSIVE_PALLAS_IPA_BATCH_MAX_LEN,
                actual: opening_len,
            },
        );
    }
    if !opening_len.is_power_of_two() {
        return Err(
            KagemushaFoldError::NonPowerOfTwoRecursiveVerifierOpeningLength {
                actual: opening_len,
            },
        );
    }
    Ok(())
}

/// Return the canonical Poseidon2 digest for reserved-mode recursive aggregation evidence.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the evidence is non-canonical or cannot
/// be encoded with Norito.
pub fn kagemusha_recursive_aggregation_evidence_digest(
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_recursive_aggregation_evidence(evidence)?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveAggregationEvidencePreimage {
        domain: KAGEMUSHA_RECURSIVE_AGGREGATION_EVIDENCE_DOMAIN.to_owned(),
        evidence: evidence.clone(),
    })
}

/// Derive recursive aggregation proof public inputs from canonical evidence.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the evidence is non-canonical or when the
/// derived public-input payload is not valid for the recursive proof corridor.
pub fn kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<KagemushaRecursiveAggregationProofPublicInputs, KagemushaFoldError> {
    validate_kagemusha_recursive_aggregation_evidence(evidence)?;
    let folded_public_inputs =
        kagemusha_folded_public_inputs_from_aggregation_statement(&evidence.aggregation_statement)?;
    let public_inputs = KagemushaRecursiveAggregationProofPublicInputs {
        domain: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN.to_owned(),
        evidence_digest: kagemusha_recursive_aggregation_evidence_digest(evidence)?,
        folded_public_inputs_hash: hash_bytes_from_hash(folded_public_inputs.public_inputs_hash()?),
        aggregation_transcript_digest: kagemusha_poseidon_aggregation_transcript_shape_digest(
            &evidence.aggregation_statement,
        )?,
        verifier_params_fingerprint: evidence.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: evidence.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: evidence
            .fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest: evidence.fixed_window_table_base_digest,
        verifier_witness_batch_digest: evidence.verifier_witness_batch_digest,
        recursive_proof_chain_digest: [0u8; Hash::LENGTH],
        transition_profile_binding_digest: [0u8; Hash::LENGTH],
        append_opening_preflight_digest: [0u8; Hash::LENGTH],
        append_boundary_digest: [0u8; Hash::LENGTH],
        recursive_verifier_scalar_projection_digest: [0u8; Hash::LENGTH],
        verifier_opening_len: evidence.verifier_opening_len,
        verifier_witness_count: evidence.verifier_witness_count,
        hop_count: evidence.aggregation_statement.hop_count,
    };
    public_inputs.validate_context()?;
    Ok(public_inputs)
}

fn validate_kagemusha_recursive_aggregation_proof_public_input_nonzero_digest(
    field: &'static str,
    digest: &[u8; Hash::LENGTH],
) -> Result<(), KagemushaFoldError> {
    if *digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch { field });
    }
    Ok(())
}

fn validate_kagemusha_recursive_aggregation_proof_public_input_digests(
    public_inputs: &KagemushaRecursiveAggregationProofPublicInputs,
) -> Result<(), KagemushaFoldError> {
    for (field, digest) in [
        ("evidence_digest", &public_inputs.evidence_digest),
        (
            "folded_public_inputs_hash",
            &public_inputs.folded_public_inputs_hash,
        ),
        (
            "aggregation_transcript_digest",
            &public_inputs.aggregation_transcript_digest,
        ),
        (
            "verifier_params_fingerprint",
            &public_inputs.verifier_params_fingerprint,
        ),
        (
            "fixed_window_table_schedule_digest",
            &public_inputs.fixed_window_table_schedule_digest,
        ),
        (
            "fixed_window_shared_table_manifest_digest",
            &public_inputs.fixed_window_shared_table_manifest_digest,
        ),
        (
            "fixed_window_table_base_digest",
            &public_inputs.fixed_window_table_base_digest,
        ),
        (
            "verifier_witness_batch_digest",
            &public_inputs.verifier_witness_batch_digest,
        ),
    ] {
        validate_kagemusha_recursive_aggregation_proof_public_input_nonzero_digest(field, digest)?;
    }
    Ok(())
}

fn validate_kagemusha_recursive_aggregation_proof_public_input_counts(
    public_inputs: &KagemushaRecursiveAggregationProofPublicInputs,
) -> Result<(), KagemushaFoldError> {
    validate_kagemusha_recursive_verifier_opening_len(public_inputs.verifier_opening_len)?;
    if public_inputs.hop_count == 0 {
        return Err(KagemushaFoldError::Empty);
    }
    let hop_count =
        usize::try_from(public_inputs.hop_count).map_err(|_| KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: usize::MAX,
        })?;
    if hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: hop_count,
        });
    }
    if public_inputs.verifier_witness_count != public_inputs.hop_count {
        return Err(
            KagemushaFoldError::RecursiveAggregationWitnessCountMismatch {
                expected: public_inputs.hop_count,
                actual: public_inputs.verifier_witness_count,
            },
        );
    }
    if public_inputs.append_opening_preflight_digest != [0u8; Hash::LENGTH]
        && public_inputs.hop_count <= 1
    {
        return Err(
            KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                field: "append_opening_preflight_digest",
            },
        );
    }
    if public_inputs.append_boundary_digest != [0u8; Hash::LENGTH]
        && (public_inputs.append_opening_preflight_digest == [0u8; Hash::LENGTH]
            || public_inputs.hop_count <= 1)
    {
        return Err(
            KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                field: "append_boundary_digest",
            },
        );
    }
    Ok(())
}

impl KagemushaRecursiveAggregationProofPublicInputs {
    /// Validate the recursive aggregation proof public-input context.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the domain, digest fields, opening
    /// length, or counts are outside the production recursive proof corridor.
    pub fn validate_context(&self) -> Result<(), KagemushaFoldError> {
        if self.domain != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN {
            return Err(
                KagemushaFoldError::InvalidRecursiveAggregationProofPublicInputDomain {
                    expected: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN,
                    actual: self.domain.clone(),
                },
            );
        }
        validate_kagemusha_recursive_aggregation_proof_public_input_digests(self)?;
        validate_kagemusha_recursive_aggregation_proof_public_input_counts(self)?;
        if self.append_opening_preflight_digest != [0u8; Hash::LENGTH] && self.hop_count <= 1 {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "append_opening_preflight_digest",
                },
            );
        }
        Ok(())
    }

    /// Deterministic hash that the recursive aggregation proof must expose.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }
}

impl KagemushaRecursiveAggregationProof {
    /// Validate that the proof envelope metadata is bound to its public inputs.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the proof uses a non-production
    /// backend, is not the canonical Halo2 IPA recursive proof backend, carries
    /// an empty proof payload, has mismatched backend/circuit metadata, or has
    /// an incorrect public-input hash.
    pub fn validate_public_input_binding(&self) -> Result<(), KagemushaFoldError> {
        self.public_inputs.validate_context()?;
        if !is_supported_kagemusha_proof_backend(&self.proof.backend) {
            return Err(KagemushaFoldError::UnsupportedProofBackend {
                backend: self.proof.backend.clone(),
            });
        }
        if !is_supported_kagemusha_proof_backend(&self.verifier_key_id.backend) {
            return Err(KagemushaFoldError::UnsupportedProofBackend {
                backend: self.verifier_key_id.backend.clone(),
            });
        }
        if self.proof.backend != self.verifier_key_id.backend {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofBackendMismatch {
                    proof_backend: self.proof.backend.clone(),
                    verifier_key_backend: self.verifier_key_id.backend.clone(),
                },
            );
        }
        if self.proof.backend != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND {
            return Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.backend",
            });
        }
        if self.proof.bytes.is_empty() {
            return Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.bytes",
            });
        }
        if self.verifier_key_id.name != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofCircuitIdMismatch {
                    expected: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                    actual: self.verifier_key_id.name.clone(),
                },
            );
        }
        for (field, digest) in [
            (
                "recursive_proof_chain_digest",
                self.public_inputs.recursive_proof_chain_digest,
            ),
            (
                "transition_profile_binding_digest",
                self.public_inputs.transition_profile_binding_digest,
            ),
            (
                "append_boundary_digest",
                self.public_inputs.append_boundary_digest,
            ),
            (
                "append_opening_preflight_digest",
                self.public_inputs.append_opening_preflight_digest,
            ),
            (
                "recursive_verifier_scalar_projection_digest",
                self.public_inputs
                    .recursive_verifier_scalar_projection_digest,
            ),
        ] {
            if digest != [0u8; Hash::LENGTH] {
                return Err(
                    KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch { field },
                );
            }
        }
        let expected = self.public_inputs.public_inputs_hash()?;
        if self.public_inputs_hash != expected {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch {
                    expected,
                    actual: self.public_inputs_hash,
                },
            );
        }
        Ok(())
    }
}

impl KagemushaRecursiveAggregationProofBundle {
    /// Validate that recursive proof public inputs are derived from this evidence.
    ///
    /// This still does not make aggregation mode `2` accepted by the legacy
    /// compact-token admission path. It provides the canonical proof-carrying
    /// surface that the ABI-7 recursive compact verifier checks before backend
    /// proof verification.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when evidence, proof metadata, or any
    /// redundant public-input field is not canonical.
    pub fn validate_evidence_binding(&self) -> Result<(), KagemushaFoldError> {
        let expected =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&self.evidence)?;
        self.recursive_proof.validate_public_input_binding()?;
        validate_kagemusha_recursive_aggregation_proof_public_input_parity(
            &expected,
            &self.recursive_proof.public_inputs,
        )
    }
}

fn validate_kagemusha_recursive_aggregation_proof_public_input_parity(
    expected: &KagemushaRecursiveAggregationProofPublicInputs,
    actual: &KagemushaRecursiveAggregationProofPublicInputs,
) -> Result<(), KagemushaFoldError> {
    macro_rules! ensure_field {
        ($field:ident) => {
            if actual.$field != expected.$field {
                return Err(
                    KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                        field: stringify!($field),
                    },
                );
            }
        };
    }
    ensure_field!(domain);
    ensure_field!(evidence_digest);
    ensure_field!(folded_public_inputs_hash);
    ensure_field!(aggregation_transcript_digest);
    ensure_field!(verifier_params_fingerprint);
    ensure_field!(fixed_window_table_schedule_digest);
    ensure_field!(fixed_window_shared_table_manifest_digest);
    ensure_field!(fixed_window_table_base_digest);
    ensure_field!(verifier_witness_batch_digest);
    ensure_field!(recursive_proof_chain_digest);
    ensure_field!(transition_profile_binding_digest);
    ensure_field!(append_opening_preflight_digest);
    ensure_field!(append_boundary_digest);
    ensure_field!(recursive_verifier_scalar_projection_digest);
    ensure_field!(verifier_opening_len);
    ensure_field!(verifier_witness_count);
    ensure_field!(hop_count);
    Ok(())
}

fn validate_kagemusha_recursive_spend_note(
    note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<(), KagemushaFoldError> {
    if note.note_commitment == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "note_commitment",
        });
    }
    if note.spend_nullifier == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "spend_nullifier",
        });
    }
    if note.note_commitment == note.spend_nullifier {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "spend_nullifier",
        });
    }
    if note.amount.is_zero()
        || note.amount.scale() != 0
        || note.amount.try_mantissa_u128().is_none()
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" });
    }
    Ok(())
}

fn validate_kagemusha_recursive_spend_topup_anchor_nullifiers(
    nullifiers: &[[u8; Hash::LENGTH]],
) -> Result<(), KagemushaFoldError> {
    if nullifiers.is_empty() || nullifiers.len() > KAGEMUSHA_FOLD_STEP_MAX_INPUTS {
        return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
            field: "topup_anchor_nullifiers",
        });
    }
    if nullifiers.contains(&[0u8; Hash::LENGTH]) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
            field: "topup_anchor_nullifiers",
        });
    }
    if nullifiers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
            field: "topup_anchor_nullifiers",
        });
    }
    Ok(())
}

fn hash_bytes_from_hash(hash: Hash) -> [u8; Hash::LENGTH] {
    hash.into()
}

fn kagemusha_recursive_spend_step_statement(
    hop_index: u32,
    step: &KagemushaFoldStep,
) -> Result<KagemushaPoseidonAggregationStepStatement, KagemushaFoldError> {
    let hop_index_usize = usize::try_from(hop_index).unwrap_or(usize::MAX);
    validate_kagemusha_verifier_key_id(hop_index_usize, &step.verifier_key_id)?;
    validate_kagemusha_step_shape_and_sets(
        hop_index_usize,
        &step.input_nullifiers,
        &step.output_commitments,
    )?;
    validate_kagemusha_step_digest_bindings(
        hop_index_usize,
        step.proof_public_inputs_digest,
        step.verifier_key_commitment,
        step.verifier_key_poseidon_digest,
    )?;
    validate_kagemusha_fold_root("root_before", step.root_before)?;
    validate_kagemusha_fold_root("root_after", step.root_after)?;
    validate_kagemusha_root_transition(hop_index_usize, step.root_before, step.root_after)?;

    let mut input_nullifiers = step.input_nullifiers.clone();
    input_nullifiers.sort_unstable();
    let mut output_commitments = step.output_commitments.clone();
    output_commitments.sort_unstable();
    validate_kagemusha_canonical_set_order(
        hop_index_usize,
        &input_nullifiers,
        &output_commitments,
    )?;
    validate_kagemusha_unique_input_output_sets(
        hop_index_usize,
        &input_nullifiers,
        &output_commitments,
    )?;

    Ok(KagemushaPoseidonAggregationStepStatement {
        hop_index,
        root_before: step.root_before,
        input_nullifiers,
        output_commitments,
        root_after: step.root_after,
        proof_hash: step.proof_hash,
        proof_public_inputs_digest: step.proof_public_inputs_digest,
        verifier_key_id: step.verifier_key_id.clone(),
        verifier_key_commitment: step.verifier_key_commitment,
        verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
    })
}

fn kagemusha_recursive_spend_step_from_statement(
    step_statement: &KagemushaPoseidonAggregationStepStatement,
) -> KagemushaFoldStep {
    KagemushaFoldStep {
        root_before: step_statement.root_before,
        input_nullifiers: step_statement.input_nullifiers.clone(),
        output_commitments: step_statement.output_commitments.clone(),
        root_after: step_statement.root_after,
        proof_hash: step_statement.proof_hash,
        proof_public_inputs_digest: step_statement.proof_public_inputs_digest,
        verifier_key_id: step_statement.verifier_key_id.clone(),
        verifier_key_commitment: step_statement.verifier_key_commitment,
        verifier_key_poseidon_digest: step_statement.verifier_key_poseidon_digest,
    }
}

fn kagemusha_recursive_spend_lineage_digest(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    hop_index: u32,
    step: &KagemushaFoldStep,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    let step = kagemusha_recursive_spend_step_statement(hop_index, step)?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendLineageDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_DIGEST_DOMAIN.to_owned(),
        previous_lineage_digest: previous.map(|accumulator| accumulator.lineage_digest),
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        hop_index,
        step,
        current_note: current_note.clone(),
    })
}

fn kagemusha_recursive_spend_verifier_batch_digest(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    hop_index: u32,
    hop_verifier_witness_batch_digest: [u8; Hash::LENGTH],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    if hop_verifier_witness_batch_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest);
    }
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendVerifierBatchDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_VERIFIER_BATCH_DIGEST_DOMAIN.to_owned(),
        previous_verifier_witness_batch_digest: previous
            .map(|accumulator| accumulator.verifier_witness_batch_digest),
        hop_index,
        hop_verifier_witness_batch_digest,
    })
}

fn kagemusha_recursive_spend_fixed_window_table_base_digest(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    hop_index: u32,
    hop_fixed_window_table_base_digest: [u8; Hash::LENGTH],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    if hop_fixed_window_table_base_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableBaseDigest);
    }
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendFixedWindowTableBaseDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_FIXED_WINDOW_TABLE_BASE_DIGEST_DOMAIN.to_owned(),
        previous_fixed_window_table_base_digest: previous
            .map(|accumulator| accumulator.fixed_window_table_base_digest),
        hop_index,
        hop_fixed_window_table_base_digest,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KagemushaRecursiveSpendProofCircuit {
    SemanticAggregation,
    Lineage,
}

fn kagemusha_recursive_spend_proof_circuit(
    verifier_key_id: &VerifyingKeyId,
) -> Result<KagemushaRecursiveSpendProofCircuit, KagemushaFoldError> {
    match verifier_key_id.name.as_str() {
        KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 => {
            Ok(KagemushaRecursiveSpendProofCircuit::SemanticAggregation)
        }
        circuit_id if is_kagemusha_recursive_spend_lineage_proof_circuit_id(circuit_id) => {
            Ok(KagemushaRecursiveSpendProofCircuit::Lineage)
        }
        _ => Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "verifier_key_id.name",
        }),
    }
}

fn validate_kagemusha_recursive_spend_proof_public_input_binding(
    recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<KagemushaRecursiveSpendProofCircuit, KagemushaFoldError> {
    recursive_proof.public_inputs.validate_context()?;
    if !is_supported_kagemusha_proof_backend(&recursive_proof.proof.backend) {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: recursive_proof.proof.backend.clone(),
        });
    }
    if !is_supported_kagemusha_proof_backend(&recursive_proof.verifier_key_id.backend) {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: recursive_proof.verifier_key_id.backend.clone(),
        });
    }
    if recursive_proof.proof.backend != recursive_proof.verifier_key_id.backend {
        return Err(
            KagemushaFoldError::RecursiveAggregationProofBackendMismatch {
                proof_backend: recursive_proof.proof.backend.clone(),
                verifier_key_backend: recursive_proof.verifier_key_id.backend.clone(),
            },
        );
    }
    if recursive_proof.proof.backend != KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "proof.backend",
        });
    }
    if recursive_proof.proof.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "proof.bytes",
        });
    }
    let circuit = kagemusha_recursive_spend_proof_circuit(&recursive_proof.verifier_key_id)?;
    let expected_hash = recursive_proof.public_inputs.public_inputs_hash()?;
    if recursive_proof.public_inputs_hash != expected_hash {
        return Err(
            KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch {
                expected: expected_hash,
                actual: recursive_proof.public_inputs_hash,
            },
        );
    }
    let public_inputs = &recursive_proof.public_inputs;
    for (field, digest) in [
        (
            "recursive_proof_chain_digest",
            public_inputs.recursive_proof_chain_digest,
        ),
        (
            "transition_profile_binding_digest",
            public_inputs.transition_profile_binding_digest,
        ),
    ] {
        if digest == [0u8; Hash::LENGTH] {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
        }
    }
    match circuit {
        KagemushaRecursiveSpendProofCircuit::SemanticAggregation => {
            for (field, digest) in [
                (
                    "append_boundary_digest",
                    public_inputs.append_boundary_digest,
                ),
                (
                    "append_opening_preflight_digest",
                    public_inputs.append_opening_preflight_digest,
                ),
                (
                    "recursive_verifier_scalar_projection_digest",
                    public_inputs.recursive_verifier_scalar_projection_digest,
                ),
            ] {
                if digest != [0u8; Hash::LENGTH] {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
                }
            }
        }
        KagemushaRecursiveSpendProofCircuit::Lineage => {
            if public_inputs.recursive_verifier_scalar_projection_digest == [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "recursive_verifier_scalar_projection_digest",
                });
            }
            if public_inputs.append_opening_preflight_digest != [0u8; Hash::LENGTH]
                && public_inputs.append_boundary_digest == [0u8; Hash::LENGTH]
            {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "append_boundary_digest",
                });
            }
        }
    }
    Ok(circuit)
}

fn expected_kagemusha_recursive_spend_public_inputs_for_proof(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
    recursive_proof: &KagemushaRecursiveAggregationProof,
    circuit: KagemushaRecursiveSpendProofCircuit,
) -> Result<KagemushaRecursiveAggregationProofPublicInputs, KagemushaFoldError> {
    let mut expected = accumulator.recursive_public_inputs()?;
    match circuit {
        KagemushaRecursiveSpendProofCircuit::SemanticAggregation => {
            if accumulator.append_boundary_digest != [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "append_boundary_digest",
                });
            }
            if recursive_proof.public_inputs.append_boundary_digest != [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "append_boundary_digest",
                });
            }
            if accumulator.append_opening_preflight_digest != [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "append_opening_preflight_digest",
                });
            }
            if recursive_proof
                .public_inputs
                .append_opening_preflight_digest
                != [0u8; Hash::LENGTH]
            {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "append_opening_preflight_digest",
                });
            }
        }
        KagemushaRecursiveSpendProofCircuit::Lineage => {
            let scalar_projection = recursive_proof
                .public_inputs
                .recursive_verifier_scalar_projection_digest;
            if scalar_projection == [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "recursive_verifier_scalar_projection_digest",
                });
            }
            expected.recursive_verifier_scalar_projection_digest = scalar_projection;
            let append_boundary_digest = recursive_proof.public_inputs.append_boundary_digest;
            if expected.append_opening_preflight_digest == [0u8; Hash::LENGTH] {
                if append_boundary_digest != [0u8; Hash::LENGTH] {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: "append_boundary_digest",
                    });
                }
            } else {
                if expected.append_boundary_digest == [0u8; Hash::LENGTH] {
                    return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "append_boundary_digest",
                    });
                }
                if append_boundary_digest != expected.append_boundary_digest {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: "append_boundary_digest",
                    });
                }
            }
        }
    }
    Ok(expected)
}

fn ensure_recursive_spend_previous_proof_matches(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<(), KagemushaFoldError> {
    let circuit =
        validate_kagemusha_recursive_spend_proof_public_input_binding(previous_recursive_proof)?;
    let expected = expected_kagemusha_recursive_spend_public_inputs_for_proof(
        previous,
        previous_recursive_proof,
        circuit,
    )?;
    macro_rules! ensure_field {
        ($field:ident) => {
            if previous_recursive_proof.public_inputs.$field != expected.$field {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: concat!("previous_recursive_proof.", stringify!($field)),
                });
            }
        };
    }
    ensure_field!(domain);
    ensure_field!(evidence_digest);
    ensure_field!(folded_public_inputs_hash);
    ensure_field!(aggregation_transcript_digest);
    ensure_field!(verifier_params_fingerprint);
    ensure_field!(fixed_window_table_schedule_digest);
    ensure_field!(fixed_window_shared_table_manifest_digest);
    ensure_field!(fixed_window_table_base_digest);
    ensure_field!(verifier_witness_batch_digest);
    ensure_field!(recursive_proof_chain_digest);
    ensure_field!(transition_profile_binding_digest);
    ensure_field!(append_opening_preflight_digest);
    ensure_field!(append_boundary_digest);
    ensure_field!(recursive_verifier_scalar_projection_digest);
    ensure_field!(verifier_opening_len);
    ensure_field!(verifier_witness_count);
    ensure_field!(hop_count);
    if previous_recursive_proof.public_inputs_hash != expected.public_inputs_hash()? {
        return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
            field: "previous_recursive_proof.public_inputs_hash",
        });
    }
    Ok(())
}

/// Return the canonical Poseidon2 digest of a recursive spend proof artifact.
///
/// This is the stable digest appended into
/// [`KagemushaRecursiveSpendAccumulatorV1::recursive_proof_chain_digest`] when a
/// recursive spend state is re-spent offline. Reserved-lineage append circuits
/// use the same digest to bind the previous recursive proof without carrying
/// older hop records in the D2D payload.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the recursive proof does not satisfy the
/// semantic or Reserved-lineage public-input binding rules.
pub fn kagemusha_recursive_spend_proof_artifact_digest(
    recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_recursive_spend_proof_public_input_binding(recursive_proof)?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendProofArtifactDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_PROOF_ARTIFACT_DIGEST_DOMAIN.to_owned(),
        recursive_proof: recursive_proof.clone(),
    })
}

fn kagemusha_recursive_spend_proof_chain_digest(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    previous_recursive_proof: Option<&KagemushaRecursiveAggregationProof>,
    hop_index: u32,
    step: &KagemushaFoldStep,
    hop_verifier_witness_batch_digest: [u8; Hash::LENGTH],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    if hop_verifier_witness_batch_digest == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest);
    }
    let previous_recursive_proof_artifact_digest = match (previous, previous_recursive_proof) {
        (Some(previous), Some(previous_recursive_proof)) => {
            ensure_recursive_spend_previous_proof_matches(previous, previous_recursive_proof)?;
            Some(kagemusha_recursive_spend_proof_artifact_digest(
                previous_recursive_proof,
            )?)
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "previous_recursive_proof",
            });
        }
        (None, None) => None,
    };
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendProofChainDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_PROOF_CHAIN_DIGEST_DOMAIN.to_owned(),
        previous_recursive_proof_chain_digest: previous
            .map(|accumulator| accumulator.recursive_proof_chain_digest),
        previous_recursive_proof_artifact_digest,
        previous_recursive_proof_public_inputs_hash: previous_recursive_proof
            .map(|recursive_proof| recursive_proof.public_inputs_hash),
        hop_index,
        current_hop_proof_hash: step.proof_hash,
        current_hop_proof_public_inputs_digest: step.proof_public_inputs_digest,
        current_hop_verifier_witness_batch_digest: hop_verifier_witness_batch_digest,
    })
}

/// Return the canonical Poseidon2 digest of a recursive Kagemusha spend accumulator.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the accumulator is malformed or cannot be
/// encoded with Norito.
pub fn kagemusha_recursive_spend_accumulator_digest(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    accumulator.validate_context()?;
    let mut accumulator = accumulator.clone();
    accumulator.append_boundary_digest = [0u8; Hash::LENGTH];
    accumulator.validate_context()?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendAccumulatorDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DIGEST_DOMAIN.to_owned(),
        accumulator,
    })
}

/// Derive recursive proof public inputs from a spend accumulator.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the accumulator or derived public-input
/// layout is invalid.
pub fn kagemusha_recursive_spend_public_inputs_from_accumulator(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
) -> Result<KagemushaRecursiveAggregationProofPublicInputs, KagemushaFoldError> {
    accumulator.validate_context()?;
    let folded_public_inputs =
        kagemusha_recursive_spend_folded_public_inputs_from_accumulator(accumulator)?;
    let public_inputs = KagemushaRecursiveAggregationProofPublicInputs {
        domain: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN.to_owned(),
        evidence_digest: kagemusha_recursive_spend_accumulator_digest(accumulator)?,
        folded_public_inputs_hash: hash_bytes_from_hash(folded_public_inputs.public_inputs_hash()?),
        aggregation_transcript_digest: accumulator.aggregation_transcript_digest,
        verifier_params_fingerprint: accumulator.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: accumulator.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: accumulator
            .fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest: accumulator.fixed_window_table_base_digest,
        verifier_witness_batch_digest: accumulator.verifier_witness_batch_digest,
        recursive_proof_chain_digest: accumulator.recursive_proof_chain_digest,
        transition_profile_binding_digest: accumulator.transition_profile_binding_digest,
        append_opening_preflight_digest: accumulator.append_opening_preflight_digest,
        append_boundary_digest: accumulator.append_boundary_digest,
        recursive_verifier_scalar_projection_digest: [0u8; Hash::LENGTH],
        verifier_opening_len: accumulator.verifier_opening_len,
        verifier_witness_count: accumulator.hop_count,
        hop_count: accumulator.hop_count,
    };
    public_inputs.validate_context()?;
    Ok(public_inputs)
}

/// Derive chain-visible recursive compact-token public inputs from a spend accumulator.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the accumulator is malformed or the
/// resulting folded public-input projection is not valid for recursive compact
/// admission.
pub fn kagemusha_recursive_spend_folded_public_inputs_from_accumulator(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
) -> Result<KagemushaFoldedPublicInputs, KagemushaFoldError> {
    accumulator.validate_context()?;
    let folded_public_inputs = KagemushaFoldedPublicInputs {
        domain: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned(),
        aggregation_mode: KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1,
        chain_id: accumulator.chain_id.clone(),
        asset: accumulator.asset.clone(),
        initial_root: accumulator.initial_root,
        final_root: accumulator.final_root,
        hop_count: accumulator.hop_count,
        nullifier_digest: accumulator.nullifier_digest,
        output_commitment_digest: accumulator.output_commitment_digest,
        fold_digest: accumulator.fold_digest,
        aggregation_transcript_digest: accumulator.aggregation_transcript_digest,
    };
    folded_public_inputs.validate_recursive_compact_context()?;
    Ok(folded_public_inputs)
}

/// Build a recursive compact payment token from a validated recursive spend bundle.
///
/// This preserves the spend bundle's recursive proof bytes and verifier-key id
/// while projecting the accumulator into chain-visible recursive compact public
/// inputs.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the bundle public inputs are not derived
/// from the accumulator, or when the folded compact projection is malformed.
pub fn kagemusha_recursive_spend_compact_payment_token_from_bundle(
    bundle: &KagemushaRecursiveSpendBundleV1,
) -> Result<KagemushaCompactPaymentToken, KagemushaFoldError> {
    bundle.validate_public_input_binding()?;
    let public_inputs =
        kagemusha_recursive_spend_folded_public_inputs_from_accumulator(&bundle.accumulator)?;
    KagemushaCompactPaymentToken::from_recursive_compact_projection(
        public_inputs,
        bundle.recursive_proof.clone(),
    )
}

/// Return the recursive spend public-input hash with the append boundary field blanked.
///
/// Compact append-boundary derivation uses this non-circular hash as
/// `resulting_public_inputs_hash`: the boundary digest itself is later placed
/// into `append_boundary_digest` in the final proof public inputs.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the accumulator or resulting public-input
/// layout is malformed.
pub fn kagemusha_recursive_spend_append_boundary_free_public_inputs_hash(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
) -> Result<Hash, KagemushaFoldError> {
    accumulator.validate_context()?;
    let mut accumulator = accumulator.clone();
    accumulator.append_boundary_digest = [0u8; Hash::LENGTH];
    let public_inputs = kagemusha_recursive_spend_public_inputs_from_accumulator(&accumulator)?;
    public_inputs
        .public_inputs_hash()
        .map_err(KagemushaFoldError::from)
}

fn ensure_recursive_spend_verifier_context_matches(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<(), KagemushaFoldError> {
    macro_rules! ensure_field {
        ($field:ident) => {
            if previous.$field != evidence.$field {
                return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                    field: stringify!($field),
                });
            }
        };
    }
    ensure_field!(verifier_opening_len);
    ensure_field!(verifier_params_fingerprint);
    ensure_field!(fixed_window_table_schedule_digest);
    ensure_field!(fixed_window_shared_table_manifest_digest);
    Ok(())
}

const KAGEMUSHA_RECURSIVE_SPEND_BINDING_ONLY_PREVIOUS_OPENINGS_ARCHIVE_DIGEST: [u8; Hash::LENGTH] =
    [0x4b; Hash::LENGTH];

#[allow(clippy::too_many_lines)]
fn kagemusha_recursive_spend_accumulator_from_parts(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    previous_recursive_proof: Option<&KagemushaRecursiveAggregationProof>,
    append_opening_preflight_digest: Option<[u8; Hash::LENGTH]>,
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendAccumulatorV1, KagemushaFoldError> {
    validate_kagemusha_recursive_aggregation_evidence(evidence)?;
    validate_kagemusha_recursive_spend_note(current_note)?;
    if evidence.aggregation_statement.steps.len() != 1 {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: 1,
            actual: evidence.aggregation_statement.hop_count,
        });
    }
    let step_statement = evidence
        .aggregation_statement
        .steps
        .first()
        .expect("validated one-hop evidence");
    let step = kagemusha_recursive_spend_step_from_statement(step_statement);
    if !step
        .output_commitments
        .iter()
        .any(|commitment| commitment == &current_note.note_commitment)
    {
        return Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment);
    }
    if step
        .input_nullifiers
        .iter()
        .any(|nullifier| nullifier == &current_note.spend_nullifier)
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "spend_nullifier",
        });
    }
    if step
        .output_commitments
        .iter()
        .any(|commitment| commitment == &current_note.spend_nullifier)
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "spend_nullifier",
        });
    }

    let chain_id = &evidence.aggregation_statement.chain_id;
    let asset = &evidence.aggregation_statement.asset;
    let hop_index = previous.map_or(0, |accumulator| accumulator.hop_count);
    if let Some(previous) = previous {
        previous.validate_context()?;
        ensure_recursive_spend_verifier_context_matches(previous, evidence)?;
        if previous.chain_id != *chain_id {
            return Err(KagemushaFoldError::RecursiveSpendChainMismatch);
        }
        if previous.asset != *asset {
            return Err(KagemushaFoldError::RecursiveSpendAssetMismatch);
        }
        if current_note.amount != previous.current_note.amount {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" });
        }
        if previous.final_root != step.root_before {
            return Err(KagemushaFoldError::RecursiveSpendRootMismatch);
        }
        if !step
            .input_nullifiers
            .iter()
            .any(|nullifier| nullifier == &previous.current_note.spend_nullifier)
        {
            return Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier);
        }
        if step.input_nullifiers.len() != 1 {
            return Err(KagemushaFoldError::RecursiveSpendUnexpectedAppendInput);
        }
        if current_note.spend_nullifier == previous.current_note.note_commitment {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier",
            });
        }
        if step
            .output_commitments
            .iter()
            .any(|commitment| commitment == &previous.current_note.note_commitment)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "output_commitments",
            });
        }
        if step.output_commitments.iter().any(|commitment| {
            previous
                .topup_anchor_nullifiers
                .iter()
                .any(|anchor| anchor == commitment)
        }) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "output_commitments",
            });
        }
    } else if step_statement.hop_index != 0 {
        return Err(KagemushaFoldError::HopIndexMismatch {
            expected: 0,
            actual: step_statement.hop_index,
        });
    }

    let lineage_digest = kagemusha_recursive_spend_lineage_digest(
        previous,
        chain_id,
        asset,
        hop_index,
        &step,
        current_note,
    )?;
    let verifier_witness_batch_digest = kagemusha_recursive_spend_verifier_batch_digest(
        previous,
        hop_index,
        evidence.verifier_witness_batch_digest,
    )?;
    let fixed_window_table_base_digest = kagemusha_recursive_spend_fixed_window_table_base_digest(
        previous,
        hop_index,
        evidence.fixed_window_table_base_digest,
    )?;
    let recursive_proof_chain_digest = kagemusha_recursive_spend_proof_chain_digest(
        previous,
        previous_recursive_proof,
        hop_index,
        &step,
        evidence.verifier_witness_batch_digest,
    )?;
    let mut current_input_nullifiers = step.input_nullifiers.clone();
    current_input_nullifiers.sort_unstable();
    validate_kagemusha_recursive_spend_topup_anchor_nullifiers(&current_input_nullifiers)?;
    let topup_anchor_nullifiers = previous.map_or_else(
        || current_input_nullifiers.clone(),
        |accumulator| accumulator.topup_anchor_nullifiers.clone(),
    );
    let nullifier_values = previous
        .map(|accumulator| vec![hash_bytes_from_hash(accumulator.nullifier_digest)])
        .unwrap_or_default()
        .into_iter()
        .chain(step.input_nullifiers.iter().copied())
        .collect::<Vec<_>>();
    let output_values = previous
        .map(|accumulator| vec![hash_bytes_from_hash(accumulator.output_commitment_digest)])
        .unwrap_or_default()
        .into_iter()
        .chain(step.output_commitments.iter().copied())
        .collect::<Vec<_>>();
    let nullifier_digest = kagemusha_hash_preimage(&KagemushaFoldListDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_NULLIFIER_DIGEST_DOMAIN.to_owned(),
        values: nullifier_values,
    })?;
    let output_commitment_digest = kagemusha_hash_preimage(&KagemushaFoldListDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_OUTPUT_DIGEST_DOMAIN.to_owned(),
        values: output_values,
    })?;
    let step_digest = kagemusha_hash_preimage(&KagemushaFoldStepDigestPreimage {
        domain: KAGEMUSHA_FOLD_STEP_DIGEST_DOMAIN.to_owned(),
        hop_index,
        root_before: step.root_before,
        input_nullifiers: step.input_nullifiers.clone(),
        output_commitments: step.output_commitments.clone(),
        root_after: step.root_after,
        proof_hash: step.proof_hash,
        proof_public_inputs_digest: step.proof_public_inputs_digest,
        verifier_key_id: step.verifier_key_id.clone(),
        verifier_key_commitment: step.verifier_key_commitment,
        verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
    })?;
    let mut step_digests = Vec::with_capacity(2);
    if let Some(previous) = previous {
        step_digests.push(previous.fold_digest);
    }
    step_digests.push(step_digest);
    let fold_digest = kagemusha_hash_preimage(&KagemushaFoldTranscriptDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_FOLD_DIGEST_DOMAIN.to_owned(),
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        step_digests,
    })?;
    let hop_count = previous.map_or(1, |accumulator| accumulator.hop_count.saturating_add(1));
    let hop_count_usize = usize::try_from(hop_count).unwrap_or(usize::MAX);
    if hop_count_usize > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: hop_count_usize,
        });
    }
    let append_opening_preflight_digest = match append_opening_preflight_digest {
        Some(digest) if digest != [0u8; Hash::LENGTH] => {
            if previous.is_none() || previous_recursive_proof.is_none() {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "append_opening_preflight_digest",
                });
            }
            digest
        }
        Some(_) => {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest",
            });
        }
        None => [0u8; Hash::LENGTH],
    };

    let mut accumulator = KagemushaRecursiveSpendAccumulatorV1 {
        domain: KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN.to_owned(),
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        initial_root: previous.map_or(step.root_before, |accumulator| accumulator.initial_root),
        final_root: step.root_after,
        topup_anchor_nullifiers,
        hop_count,
        lineage_digest,
        aggregation_transcript_digest: lineage_digest,
        nullifier_digest,
        output_commitment_digest,
        fold_digest,
        recursive_proof_chain_digest,
        transition_profile_binding_digest:
            KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_PENDING,
        append_opening_preflight_digest,
        append_boundary_digest: [0u8; Hash::LENGTH],
        verifier_params_fingerprint: evidence.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: evidence.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: evidence
            .fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest,
        verifier_witness_batch_digest,
        verifier_opening_len: evidence.verifier_opening_len,
        current_note: current_note.clone(),
    };
    let transition_profile = if append_opening_preflight_digest == [0u8; Hash::LENGTH] {
        kagemusha_recursive_spend_transition_profile_from_accumulator_and_digest_parts(
            &accumulator,
            previous,
            previous_recursive_proof,
            None,
            None,
            None,
            evidence,
        )?
    } else {
        // The accumulator only needs the non-circular binding digest. The full
        // public profile builders still validate and bind the real archive
        // digest; this placeholder is blanked before hashing the binding digest.
        kagemusha_recursive_spend_transition_profile_from_accumulator_and_digest_parts(
            &accumulator,
            previous,
            previous_recursive_proof,
            Some(KAGEMUSHA_RECURSIVE_SPEND_BINDING_ONLY_PREVIOUS_OPENINGS_ARCHIVE_DIGEST),
            Some(append_opening_preflight_digest),
            None,
            evidence,
        )?
    };
    accumulator.transition_profile_binding_digest =
        kagemusha_recursive_spend_transition_profile_binding_digest(&transition_profile)?;
    accumulator.validate_context()?;
    Ok(accumulator)
}

/// Build the first recursive Kagemusha spend accumulator from one verified hop.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the one-hop evidence is malformed or the
/// current note does not match the hop output.
pub fn kagemusha_recursive_spend_accumulator_from_initial_evidence(
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendAccumulatorV1, KagemushaFoldError> {
    kagemusha_recursive_spend_accumulator_from_parts(None, None, None, evidence, current_note)
}

/// Append one verified hop to an existing recursive Kagemusha spend accumulator.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous accumulator is malformed,
/// the previous recursive proof is not bound to that accumulator, the new
/// one-hop evidence does not continue the lineage, or verifier context changes
/// across hops.
pub fn kagemusha_recursive_spend_accumulator_append_evidence(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendAccumulatorV1, KagemushaFoldError> {
    kagemusha_recursive_spend_accumulator_from_parts(
        Some(previous),
        Some(previous_recursive_proof),
        None,
        evidence,
        current_note,
    )
}

/// Append one verified hop and bind a Reserved-lineage append opening preflight digest.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous accumulator/proof binding,
/// the append opening preflight digest, the one-hop evidence, or the resulting
/// spendable note is malformed.
pub fn kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_digest(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    append_opening_preflight_digest: [u8; Hash::LENGTH],
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendAccumulatorV1, KagemushaFoldError> {
    kagemusha_recursive_spend_accumulator_from_parts(
        Some(previous),
        Some(previous_recursive_proof),
        Some(append_opening_preflight_digest),
        evidence,
        current_note,
    )
}

/// Append one verified hop and bind the full Reserved-lineage append boundary.
///
/// This is the production Reserved-lineage accumulator builder. It validates
/// the previous recursive proof opening archive and full two-opening preflight
/// contract, derives the compact append boundary, and stores that boundary
/// digest in the resulting accumulator so proof public inputs have a canonical
/// source of truth.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when any previous-state, opening-preflight,
/// compact-boundary, verifier-corridor, hop evidence, or resulting note binding
/// is malformed.
pub fn kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    previous_recursive_proof_open_envelopes_archive: &[u8],
    append_opening_preflight: KagemushaRecursiveSpendLineageAppendOpeningPreflightV1,
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendAccumulatorV1, KagemushaFoldError> {
    append_opening_preflight.validate_context()?;
    let mut accumulator =
        kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_digest(
            previous,
            previous_recursive_proof,
            append_opening_preflight.append_opening_preflight_digest,
            evidence,
            current_note,
        )?;
    let transition_profile =
        kagemusha_recursive_spend_transition_profile_from_accumulator_and_parts(
            &accumulator,
            Some(previous),
            Some(previous_recursive_proof),
            Some(previous_recursive_proof_open_envelopes_archive),
            Some(append_opening_preflight.append_opening_preflight_digest),
            Some(append_opening_preflight),
            evidence,
        )?;
    let append_boundary =
        kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
            &transition_profile,
        )?;
    accumulator.append_boundary_digest = append_boundary.append_boundary_digest;
    accumulator.validate_context()?;
    Ok(accumulator)
}

fn kagemusha_recursive_spend_transition_profile_from_parts(
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    previous_recursive_proof: Option<&KagemushaRecursiveAggregationProof>,
    previous_recursive_proof_open_envelopes_archive: Option<&[u8]>,
    append_opening_preflight_digest: Option<[u8; 32]>,
    append_opening_preflight: Option<KagemushaRecursiveSpendLineageAppendOpeningPreflightV1>,
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendTransitionProfileV1, KagemushaFoldError> {
    let accumulator = kagemusha_recursive_spend_accumulator_from_parts(
        previous,
        previous_recursive_proof,
        append_opening_preflight_digest,
        evidence,
        current_note,
    )?;
    kagemusha_recursive_spend_transition_profile_from_accumulator_and_parts(
        &accumulator,
        previous,
        previous_recursive_proof,
        previous_recursive_proof_open_envelopes_archive,
        append_opening_preflight_digest,
        append_opening_preflight,
        evidence,
    )
}

fn kagemusha_recursive_spend_transition_profile_from_accumulator_and_parts(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    previous_recursive_proof: Option<&KagemushaRecursiveAggregationProof>,
    previous_recursive_proof_open_envelopes_archive: Option<&[u8]>,
    append_opening_preflight_digest: Option<[u8; 32]>,
    append_opening_preflight: Option<KagemushaRecursiveSpendLineageAppendOpeningPreflightV1>,
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<KagemushaRecursiveSpendTransitionProfileV1, KagemushaFoldError> {
    let previous_recursive_proof_open_envelopes_archive_digest =
        match previous_recursive_proof_open_envelopes_archive {
            Some(archive) if !archive.is_empty() => {
                let previous = previous.ok_or(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "previous_recursive_proof_open_envelopes_archive",
                })?;
                let previous_recursive_proof = previous_recursive_proof.ok_or(
                    KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "previous_recursive_proof_open_envelopes_archive",
                    },
                )?;
                let previous_bundle = KagemushaRecursiveSpendBundleV1 {
                    accumulator: previous.clone(),
                    recursive_proof: previous_recursive_proof.clone(),
                };
                validate_kagemusha_recursive_previous_proof_open_envelopes_archive(
                    &previous_bundle,
                    archive,
                    false,
                )?;
                Some(kagemusha_recursive_previous_proof_open_envelopes_archive_digest(archive)?)
            }
            _ => None,
        };
    kagemusha_recursive_spend_transition_profile_from_accumulator_and_digest_parts(
        accumulator,
        previous,
        previous_recursive_proof,
        previous_recursive_proof_open_envelopes_archive_digest,
        append_opening_preflight_digest,
        append_opening_preflight,
        evidence,
    )
}

#[allow(clippy::too_many_lines)]
fn kagemusha_recursive_spend_transition_profile_from_accumulator_and_digest_parts(
    accumulator: &KagemushaRecursiveSpendAccumulatorV1,
    previous: Option<&KagemushaRecursiveSpendAccumulatorV1>,
    previous_recursive_proof: Option<&KagemushaRecursiveAggregationProof>,
    previous_recursive_proof_open_envelopes_archive_digest: Option<[u8; Hash::LENGTH]>,
    append_opening_preflight_digest: Option<[u8; 32]>,
    append_opening_preflight: Option<KagemushaRecursiveSpendLineageAppendOpeningPreflightV1>,
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<KagemushaRecursiveSpendTransitionProfileV1, KagemushaFoldError> {
    accumulator.validate_context()?;
    let evidence_step_statement = evidence
        .aggregation_statement
        .steps
        .first()
        .expect("validated one-hop evidence")
        .clone();
    let hop_index = accumulator.hop_count - 1;
    let step_statement = kagemusha_recursive_spend_step_statement(
        hop_index,
        &kagemusha_recursive_spend_step_from_statement(&evidence_step_statement),
    )?;
    let (previous_recursive_proof_artifact_digest, previous_recursive_proof_public_inputs_hash) =
        if let Some(proof) = previous_recursive_proof {
            (
                Some(kagemusha_recursive_spend_proof_artifact_digest(proof)?),
                Some(proof.public_inputs_hash),
            )
        } else {
            (None, None)
        };
    let previous_accumulator_public_inputs_hash = match (previous, previous_recursive_proof) {
        (Some(previous), Some(proof)) => {
            let circuit = validate_kagemusha_recursive_spend_proof_public_input_binding(proof)?;
            Some(
                expected_kagemusha_recursive_spend_public_inputs_for_proof(
                    previous, proof, circuit,
                )?
                .public_inputs_hash()?,
            )
        }
        (Some(previous), None) => Some(previous.recursive_public_inputs()?.public_inputs_hash()?),
        (None, _) => None,
    };
    if append_opening_preflight_digest.is_some()
        && previous_recursive_proof_open_envelopes_archive_digest.is_none()
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "append_opening_preflight_digest",
        });
    }
    if append_opening_preflight_digest.is_some_and(|digest| digest == [0u8; Hash::LENGTH]) {
        return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
            field: "append_opening_preflight_digest",
        });
    }
    if let Some(preflight) = &append_opening_preflight {
        preflight.validate_context()?;
        if append_opening_preflight_digest != Some(preflight.append_opening_preflight_digest) {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest",
            });
        }
    }
    let expected_append_opening_preflight_digest =
        append_opening_preflight_digest.unwrap_or([0u8; Hash::LENGTH]);
    if accumulator.append_opening_preflight_digest != expected_append_opening_preflight_digest {
        return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
            field: "append_opening_preflight_digest",
        });
    }
    let profile = KagemushaRecursiveSpendTransitionProfileV1 {
        domain: KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN.to_owned(),
        chain_id: accumulator.chain_id.clone(),
        asset: accumulator.asset.clone(),
        previous_accumulator_digest: previous
            .map(kagemusha_recursive_spend_accumulator_digest)
            .transpose()?,
        previous_initial_root: previous.map(|previous| previous.initial_root),
        previous_final_root: previous.map(|previous| previous.final_root),
        previous_current_note: previous.map(|previous| previous.current_note.clone()),
        previous_lineage_digest: previous.map(|previous| previous.lineage_digest),
        previous_recursive_proof_chain_digest: previous
            .map(|previous| previous.recursive_proof_chain_digest),
        previous_recursive_proof_artifact_digest,
        previous_accumulator_public_inputs_hash,
        previous_recursive_proof_public_inputs_hash,
        previous_recursive_proof_open_envelopes_archive_digest,
        append_opening_preflight_digest,
        append_opening_preflight,
        previous_verifier_witness_batch_digest: previous
            .map(|previous| previous.verifier_witness_batch_digest),
        previous_fixed_window_table_base_digest: previous
            .map(|previous| previous.fixed_window_table_base_digest),
        hop_index,
        hop_count: accumulator.hop_count,
        current_hop_statement: step_statement,
        current_note: accumulator.current_note.clone(),
        current_hop_verifier_witness_batch_digest: evidence.verifier_witness_batch_digest,
        current_hop_fixed_window_table_base_digest: evidence.fixed_window_table_base_digest,
        verifier_params_fingerprint: accumulator.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: accumulator.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: accumulator
            .fixed_window_shared_table_manifest_digest,
        verifier_opening_len: accumulator.verifier_opening_len,
        resulting_initial_root: accumulator.initial_root,
        resulting_final_root: accumulator.final_root,
        resulting_lineage_digest: accumulator.lineage_digest,
        resulting_verifier_witness_batch_digest: accumulator.verifier_witness_batch_digest,
        resulting_fixed_window_table_base_digest: accumulator.fixed_window_table_base_digest,
        resulting_recursive_proof_chain_digest: accumulator.recursive_proof_chain_digest,
        resulting_append_opening_preflight_digest: accumulator.append_opening_preflight_digest,
        resulting_nullifier_digest: accumulator.nullifier_digest,
        resulting_output_commitment_digest: accumulator.output_commitment_digest,
        resulting_fold_digest: accumulator.fold_digest,
        resulting_accumulator_digest: kagemusha_recursive_spend_accumulator_digest(accumulator)?,
        resulting_public_inputs_hash:
            kagemusha_recursive_spend_append_boundary_free_public_inputs_hash(accumulator)?,
    };
    profile.validate_context()?;
    Ok(profile)
}

/// Build the canonical Reserved-lineage transition profile for the first hop.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the initial one-hop evidence or spendable
/// note is malformed.
pub fn kagemusha_recursive_spend_transition_profile_from_initial_evidence(
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendTransitionProfileV1, KagemushaFoldError> {
    kagemusha_recursive_spend_transition_profile_from_parts(
        None,
        None,
        None,
        None,
        None,
        evidence,
        current_note,
    )
}

/// Build the canonical Reserved-lineage transition profile for one append.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous accumulator/proof binding,
/// current one-hop evidence, or resulting spendable note is malformed.
pub fn kagemusha_recursive_spend_transition_profile_append_evidence(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendTransitionProfileV1, KagemushaFoldError> {
    kagemusha_recursive_spend_transition_profile_from_parts(
        Some(previous),
        Some(previous_recursive_proof),
        None,
        None,
        None,
        evidence,
        current_note,
    )
}

/// Build the canonical Reserved-lineage transition profile for one append with
/// the previous recursive proof opening archive bound into the profile digest.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous accumulator/proof binding,
/// previous proof opening archive digest, current one-hop evidence, or resulting
/// spendable note is malformed.
pub fn kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    previous_recursive_proof_open_envelopes_archive: &[u8],
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendTransitionProfileV1, KagemushaFoldError> {
    kagemusha_recursive_spend_transition_profile_from_parts(
        Some(previous),
        Some(previous_recursive_proof),
        Some(previous_recursive_proof_open_envelopes_archive),
        None,
        None,
        evidence,
        current_note,
    )
}

/// Build the canonical Reserved-lineage append transition profile with both
/// previous-proof opening archive bytes and the append opening preflight digest.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous accumulator/proof binding,
/// previous proof opening archive digest, append opening preflight digest,
/// current one-hop evidence, or resulting spendable note is malformed.
pub fn kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    previous_recursive_proof_open_envelopes_archive: &[u8],
    append_opening_preflight_digest: [u8; 32],
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendTransitionProfileV1, KagemushaFoldError> {
    kagemusha_recursive_spend_transition_profile_from_parts(
        Some(previous),
        Some(previous_recursive_proof),
        Some(previous_recursive_proof_open_envelopes_archive),
        Some(append_opening_preflight_digest),
        None,
        evidence,
        current_note,
    )
}

/// Build the canonical Reserved-lineage append transition profile with the full
/// append opening preflight contract.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous accumulator/proof binding,
/// previous proof opening archive digest, append opening preflight contract,
/// current one-hop evidence, or resulting spendable note is malformed.
pub fn kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
    previous: &KagemushaRecursiveSpendAccumulatorV1,
    previous_recursive_proof: &KagemushaRecursiveAggregationProof,
    previous_recursive_proof_open_envelopes_archive: &[u8],
    append_opening_preflight: KagemushaRecursiveSpendLineageAppendOpeningPreflightV1,
    evidence: &KagemushaRecursiveAggregationEvidence,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<KagemushaRecursiveSpendTransitionProfileV1, KagemushaFoldError> {
    append_opening_preflight.validate_context()?;
    kagemusha_recursive_spend_transition_profile_from_parts(
        Some(previous),
        Some(previous_recursive_proof),
        Some(previous_recursive_proof_open_envelopes_archive),
        Some(append_opening_preflight.append_opening_preflight_digest),
        Some(append_opening_preflight),
        evidence,
        current_note,
    )
}

/// Return the canonical Poseidon2 digest of a Reserved-lineage transition profile.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the profile is malformed or cannot be
/// encoded with Norito.
pub fn kagemusha_recursive_spend_transition_profile_digest(
    profile: &KagemushaRecursiveSpendTransitionProfileV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    profile.validate_context()?;
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendTransitionProfileDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN.to_owned(),
        profile: profile.clone(),
    })
}

/// Return the non-circular public binding digest of a Reserved-lineage transition profile.
///
/// The full transition profile can carry optional verifier-opening material,
/// the resulting accumulator digest, and the resulting public-input hash.
/// Recursive spend public inputs also need to bind transition semantics, so
/// hashing those result hashes back into the public inputs would create a
/// self-reference, and requiring optional opening archives would make the
/// accumulator depend on append-prover transport choices. This digest validates
/// the full profile, then hashes a canonical copy with those optional/self
/// fields blanked. All lineage transition inputs and resulting accumulator
/// state remain bound.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the profile is malformed or cannot be
/// encoded with Norito.
pub fn kagemusha_recursive_spend_transition_profile_binding_digest(
    profile: &KagemushaRecursiveSpendTransitionProfileV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    profile.validate_context()?;
    let mut binding_profile = profile.clone();
    binding_profile.previous_recursive_proof_open_envelopes_archive_digest = None;
    binding_profile.append_opening_preflight_digest = None;
    binding_profile.append_opening_preflight = None;
    binding_profile.resulting_accumulator_digest = [0u8; Hash::LENGTH];
    binding_profile.resulting_public_inputs_hash = Hash::prehashed([0u8; Hash::LENGTH]);
    kagemusha_poseidon_preimage(&KagemushaRecursiveSpendTransitionProfileDigestPreimage {
        domain: KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN.to_owned(),
        profile: binding_profile,
    })
}

/// Return the canonical digest of previous recursive proof opening archive bytes.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the archive is empty or cannot be encoded
/// into the domain-separated Poseidon2 preimage.
pub fn kagemusha_recursive_previous_proof_open_envelopes_archive_digest(
    archive: &[u8],
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    if archive.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_recursive_proof_open_envelopes_archive",
        });
    }
    kagemusha_poseidon_preimage(
        &KagemushaRecursivePreviousProofOpenEnvelopesArchiveDigestPreimage {
            domain: KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_ARCHIVE_DIGEST_DOMAIN_V1
                .to_owned(),
            archive: archive.to_vec(),
        },
    )
}

impl KagemushaRecursiveVerifierPreflightV1 {
    /// Validate a portable recursive verifier preflight summary.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the witness count, profile, opening
    /// length, or any digest field is outside the recursive Kagemusha corridor.
    pub fn validate_context(&self) -> Result<(), KagemushaFoldError> {
        if self.proof_count == 0 {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_verifier_preflight.proof_count",
            });
        }
        if self.verifier_witness_profile != KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1 {
            return Err(
                KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile {
                    expected: KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1,
                    actual: self.verifier_witness_profile.clone(),
                },
            );
        }
        validate_kagemusha_recursive_verifier_opening_len(self.opening_len)?;
        for (field, digest) in [
            (
                "recursive_verifier_preflight.params_fingerprint",
                self.params_fingerprint,
            ),
            (
                "recursive_verifier_preflight.fixed_window_table_schedule_digest",
                self.fixed_window_table_schedule_digest,
            ),
            (
                "recursive_verifier_preflight.fixed_window_shared_table_manifest_digest",
                self.fixed_window_shared_table_manifest_digest,
            ),
            (
                "recursive_verifier_preflight.fixed_window_table_base_digest",
                self.fixed_window_table_base_digest,
            ),
            (
                "recursive_verifier_preflight.aggregate_digest",
                self.aggregate_digest,
            ),
        ] {
            if digest == [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
            }
        }
        Ok(())
    }
}

fn validate_kagemusha_recursive_spend_lineage_append_opening_preflight_preimage(
    preflight: &KagemushaRecursiveSpendLineageAppendOpeningPreflightV1,
) -> Result<(), KagemushaFoldError> {
    if preflight.domain != KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1 {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "append_opening_preflight.domain",
        });
    }
    preflight
        .previous_recursive_proof_preflight
        .validate_context()?;
    preflight.current_hop_preflight.validate_context()?;
    if preflight.previous_recursive_proof_preflight.proof_count != 1 {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "append_opening_preflight.previous_recursive_proof_preflight.proof_count",
        });
    }
    if preflight.current_hop_preflight.proof_count != 1 {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "append_opening_preflight.current_hop_preflight.proof_count",
        });
    }
    if preflight.previous_recursive_proof_preflight.opening_len
        != preflight.current_hop_preflight.opening_len
    {
        return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
            field: "append_opening_preflight.shared_opening_len",
        });
    }
    if preflight
        .previous_recursive_proof_preflight
        .params_fingerprint
        != preflight.current_hop_preflight.params_fingerprint
    {
        return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
            field: "append_opening_preflight.shared_params_fingerprint",
        });
    }
    if preflight
        .previous_recursive_proof_preflight
        .fixed_window_table_schedule_digest
        != preflight
            .current_hop_preflight
            .fixed_window_table_schedule_digest
    {
        return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
            field: "append_opening_preflight.shared_fixed_window_table_schedule_digest",
        });
    }
    if preflight
        .previous_recursive_proof_preflight
        .fixed_window_shared_table_manifest_digest
        != preflight
            .current_hop_preflight
            .fixed_window_shared_table_manifest_digest
    {
        return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
            field: "append_opening_preflight.shared_fixed_window_table_manifest_digest",
        });
    }
    for (field, digest) in [
        (
            "append_opening_preflight.previous_accumulator_digest",
            preflight.previous_accumulator_digest,
        ),
        (
            "append_opening_preflight.previous_recursive_proof_artifact_digest",
            preflight.previous_recursive_proof_artifact_digest,
        ),
        (
            "append_opening_preflight.previous_recursive_proof_open_envelopes_archive_digest",
            preflight.previous_recursive_proof_open_envelopes_archive_digest,
        ),
    ] {
        if digest == [0u8; Hash::LENGTH] {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
        }
    }
    if hash_bytes_from_hash(preflight.current_hop_proof_hash) == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
            field: "append_opening_preflight.current_hop_proof_hash",
        });
    }
    Ok(())
}

fn kagemusha_recursive_spend_lineage_append_opening_preflight_digest_unchecked(
    preflight: &KagemushaRecursiveSpendLineageAppendOpeningPreflightV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_recursive_spend_lineage_append_opening_preflight_preimage(preflight)?;
    kagemusha_poseidon_preimage(
        &KagemushaRecursiveSpendLineageAppendOpeningPreflightDigestPreimage {
            domain: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1
                .to_owned(),
            previous_recursive_proof_preflight: preflight
                .previous_recursive_proof_preflight
                .clone(),
            current_hop_preflight: preflight.current_hop_preflight.clone(),
            previous_accumulator_digest: preflight.previous_accumulator_digest,
            previous_recursive_proof_artifact_digest: preflight
                .previous_recursive_proof_artifact_digest,
            previous_recursive_proof_open_envelopes_archive_digest: preflight
                .previous_recursive_proof_open_envelopes_archive_digest,
            current_hop_proof_hash: preflight.current_hop_proof_hash,
        },
    )
}

/// Return the canonical Poseidon2 digest for a Reserved-lineage append opening preflight.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the preflight contract is malformed or
/// cannot be encoded into the digest preimage.
pub fn kagemusha_recursive_spend_lineage_append_opening_preflight_digest(
    preflight: &KagemushaRecursiveSpendLineageAppendOpeningPreflightV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    preflight.validate_context()?;
    Ok(preflight.append_opening_preflight_digest)
}

impl KagemushaRecursiveSpendLineageAppendOpeningPreflightV1 {
    /// Build a validated Reserved-lineage append opening preflight contract.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when either verifier preflight is malformed
    /// or any append binding digest is zero.
    pub fn new(
        previous_recursive_proof_preflight: KagemushaRecursiveVerifierPreflightV1,
        current_hop_preflight: KagemushaRecursiveVerifierPreflightV1,
        previous_accumulator_digest: [u8; Hash::LENGTH],
        previous_recursive_proof_artifact_digest: [u8; Hash::LENGTH],
        previous_recursive_proof_open_envelopes_archive_digest: [u8; Hash::LENGTH],
        current_hop_proof_hash: Hash,
    ) -> Result<Self, KagemushaFoldError> {
        let mut preflight = Self {
            domain: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1
                .to_owned(),
            previous_recursive_proof_preflight,
            current_hop_preflight,
            previous_accumulator_digest,
            previous_recursive_proof_artifact_digest,
            previous_recursive_proof_open_envelopes_archive_digest,
            current_hop_proof_hash,
            append_opening_preflight_digest: [0u8; Hash::LENGTH],
        };
        preflight.append_opening_preflight_digest =
            kagemusha_recursive_spend_lineage_append_opening_preflight_digest_unchecked(
                &preflight,
            )?;
        preflight.validate_context()?;
        Ok(preflight)
    }

    /// Validate the complete preflight contract, including its digest binding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the contract digest is zero, stale, or
    /// not bound to the verifier-preflight and append fields.
    pub fn validate_context(&self) -> Result<(), KagemushaFoldError> {
        validate_kagemusha_recursive_spend_lineage_append_opening_preflight_preimage(self)?;
        if self.append_opening_preflight_digest == [0u8; Hash::LENGTH] {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight.append_opening_preflight_digest",
            });
        }
        let expected =
            kagemusha_recursive_spend_lineage_append_opening_preflight_digest_unchecked(self)?;
        if self.append_opening_preflight_digest != expected {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight.append_opening_preflight_digest",
            });
        }
        Ok(())
    }

    /// Return the contract digest after validating the contract.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the contract is malformed.
    pub fn digest(&self) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
        kagemusha_recursive_spend_lineage_append_opening_preflight_digest(self)
    }
}

fn validate_kagemusha_recursive_spend_lineage_append_boundary_preimage(
    boundary: &KagemushaRecursiveSpendLineageAppendBoundaryV1,
) -> Result<(), KagemushaFoldError> {
    if boundary.domain != KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1 {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "append_boundary.domain",
        });
    }
    if boundary.hop_count < 2 {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "append_boundary.hop_count",
        });
    }
    validate_kagemusha_recursive_verifier_opening_len(boundary.verifier_opening_len)?;
    for (field, digest) in [
        (
            "append_boundary.transition_profile_digest",
            boundary.transition_profile_digest,
        ),
        (
            "append_boundary.transition_profile_binding_digest",
            boundary.transition_profile_binding_digest,
        ),
        (
            "append_boundary.chain_asset_binding_digest",
            boundary.chain_asset_binding_digest,
        ),
        (
            "append_boundary.final_note_binding_digest",
            boundary.final_note_binding_digest,
        ),
        (
            "append_boundary.previous_accumulator_digest",
            boundary.previous_accumulator_digest,
        ),
        (
            "append_boundary.previous_recursive_proof_artifact_digest",
            boundary.previous_recursive_proof_artifact_digest,
        ),
        (
            "append_boundary.previous_recursive_proof_open_envelopes_archive_digest",
            boundary.previous_recursive_proof_open_envelopes_archive_digest,
        ),
        (
            "append_boundary.append_opening_preflight_digest",
            boundary.append_opening_preflight_digest,
        ),
        (
            "append_boundary.previous_recursive_proof_opening_aggregate_digest",
            boundary.previous_recursive_proof_opening_aggregate_digest,
        ),
        (
            "append_boundary.current_hop_opening_aggregate_digest",
            boundary.current_hop_opening_aggregate_digest,
        ),
        (
            "append_boundary.resulting_accumulator_digest",
            boundary.resulting_accumulator_digest,
        ),
        (
            "append_boundary.verifier_params_fingerprint",
            boundary.verifier_params_fingerprint,
        ),
        (
            "append_boundary.fixed_window_table_schedule_digest",
            boundary.fixed_window_table_schedule_digest,
        ),
        (
            "append_boundary.fixed_window_shared_table_manifest_digest",
            boundary.fixed_window_shared_table_manifest_digest,
        ),
    ] {
        if digest == [0u8; Hash::LENGTH] {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
        }
    }
    if hash_bytes_from_hash(boundary.current_hop_proof_hash) == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
            field: "append_boundary.current_hop_proof_hash",
        });
    }
    if hash_bytes_from_hash(boundary.resulting_public_inputs_hash) == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
            field: "append_boundary.resulting_public_inputs_hash",
        });
    }
    Ok(())
}

fn kagemusha_recursive_spend_lineage_append_boundary_digest_unchecked(
    boundary: &KagemushaRecursiveSpendLineageAppendBoundaryV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_recursive_spend_lineage_append_boundary_preimage(boundary)?;
    let mut boundary = boundary.clone();
    boundary.append_boundary_digest = [0u8; Hash::LENGTH];
    kagemusha_poseidon_preimage(
        &KagemushaRecursiveSpendLineageAppendBoundaryDigestPreimage {
            domain: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1.to_owned(),
            boundary,
        },
    )
}

/// Return the canonical Poseidon2 digest for a compact Reserved-lineage append boundary.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the boundary is malformed or carries a
/// stale digest.
pub fn kagemusha_recursive_spend_lineage_append_boundary_digest(
    boundary: &KagemushaRecursiveSpendLineageAppendBoundaryV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    boundary.validate_context()?;
    Ok(boundary.append_boundary_digest)
}

/// Return the canonical chain/asset binding digest used by compact Reserved-lineage append boundaries.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the digest preimage cannot be encoded.
pub fn kagemusha_recursive_spend_lineage_append_boundary_chain_asset_binding_digest(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    kagemusha_poseidon_preimage(
        &KagemushaRecursiveSpendLineageAppendBoundaryChainAssetBindingDigestPreimage {
            domain: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1
                .to_owned(),
            chain_id: chain_id.clone(),
            asset: asset.clone(),
        },
    )
}

/// Return the canonical final-root/current-note binding digest used by compact Reserved-lineage append boundaries.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the final root or spendable-note
/// descriptor is malformed, or when the digest preimage cannot be encoded.
pub fn kagemusha_recursive_spend_lineage_append_boundary_final_note_binding_digest(
    final_root: [u8; Hash::LENGTH],
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    validate_kagemusha_fold_root("append_boundary.final_root", final_root)?;
    validate_kagemusha_recursive_spend_note(current_note)?;
    kagemusha_poseidon_preimage(
        &KagemushaRecursiveSpendLineageAppendBoundaryFinalNoteBindingDigestPreimage {
            domain: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1
                .to_owned(),
            final_root,
            current_note: current_note.clone(),
        },
    )
}

/// Derive the compact Reserved-lineage append boundary from a full transition profile.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the profile is not an append profile, does
/// not carry the full append opening preflight contract, or fails local
/// transition validation.
pub fn kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
    profile: &KagemushaRecursiveSpendTransitionProfileV1,
) -> Result<KagemushaRecursiveSpendLineageAppendBoundaryV1, KagemushaFoldError> {
    profile.validate_context()?;
    let append_opening_preflight = profile.append_opening_preflight.as_ref().ok_or(
        KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "append_opening_preflight",
        },
    )?;
    let append_opening_preflight_digest = profile.append_opening_preflight_digest.ok_or(
        KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "append_opening_preflight_digest",
        },
    )?;
    if append_opening_preflight_digest != append_opening_preflight.append_opening_preflight_digest {
        return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
            field: "append_opening_preflight_digest",
        });
    }
    let previous_accumulator_digest = profile.previous_accumulator_digest.ok_or(
        KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_accumulator_digest",
        },
    )?;
    let previous_recursive_proof_artifact_digest = profile
        .previous_recursive_proof_artifact_digest
        .ok_or(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_recursive_proof_artifact_digest",
        })?;
    let previous_recursive_proof_open_envelopes_archive_digest = profile
        .previous_recursive_proof_open_envelopes_archive_digest
        .ok_or(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_recursive_proof_open_envelopes_archive_digest",
        })?;
    let mut boundary = KagemushaRecursiveSpendLineageAppendBoundaryV1 {
        domain: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1.to_owned(),
        transition_profile_digest: profile.digest()?,
        transition_profile_binding_digest: profile.binding_digest()?,
        chain_asset_binding_digest:
            kagemusha_recursive_spend_lineage_append_boundary_chain_asset_binding_digest(
                &profile.chain_id,
                &profile.asset,
            )?,
        final_note_binding_digest:
            kagemusha_recursive_spend_lineage_append_boundary_final_note_binding_digest(
                profile.resulting_final_root,
                &profile.current_note,
            )?,
        previous_accumulator_digest,
        previous_recursive_proof_artifact_digest,
        previous_recursive_proof_open_envelopes_archive_digest,
        append_opening_preflight_digest,
        previous_recursive_proof_opening_aggregate_digest: append_opening_preflight
            .previous_recursive_proof_preflight
            .aggregate_digest,
        current_hop_opening_aggregate_digest: append_opening_preflight
            .current_hop_preflight
            .aggregate_digest,
        current_hop_proof_hash: append_opening_preflight.current_hop_proof_hash,
        resulting_accumulator_digest: profile.resulting_accumulator_digest,
        resulting_public_inputs_hash: profile.resulting_public_inputs_hash,
        hop_count: profile.hop_count,
        verifier_opening_len: profile.verifier_opening_len,
        verifier_params_fingerprint: profile.verifier_params_fingerprint,
        fixed_window_table_schedule_digest: profile.fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest: profile
            .fixed_window_shared_table_manifest_digest,
        append_boundary_digest: [0u8; Hash::LENGTH],
    };
    boundary.append_boundary_digest =
        kagemusha_recursive_spend_lineage_append_boundary_digest_unchecked(&boundary)?;
    boundary.validate_context()?;
    Ok(boundary)
}

impl KagemushaRecursiveSpendLineageAppendBoundaryV1 {
    /// Validate the compact append boundary, including the digest over all fields.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the boundary is stale, malformed, or
    /// not a real append boundary.
    pub fn validate_context(&self) -> Result<(), KagemushaFoldError> {
        validate_kagemusha_recursive_spend_lineage_append_boundary_preimage(self)?;
        if self.append_boundary_digest == [0u8; Hash::LENGTH] {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.append_boundary_digest",
            });
        }
        let expected = kagemusha_recursive_spend_lineage_append_boundary_digest_unchecked(self)?;
        if self.append_boundary_digest != expected {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.append_boundary_digest",
            });
        }
        Ok(())
    }

    /// Return the boundary digest after validation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the boundary is malformed.
    pub fn digest(&self) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
        kagemusha_recursive_spend_lineage_append_boundary_digest(self)
    }

    /// Validate this compact boundary against its full Reserved-lineage transition profile.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the boundary is malformed, the profile
    /// is not a full append profile, or any compact boundary field does not
    /// match the canonical boundary derived from the profile.
    pub fn validate_against_transition_profile(
        &self,
        profile: &KagemushaRecursiveSpendTransitionProfileV1,
    ) -> Result<(), KagemushaFoldError> {
        self.validate_context()?;
        let expected =
            kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(profile)?;
        macro_rules! ensure_field {
            ($field:ident) => {
                if self.$field != expected.$field {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: concat!("append_boundary.", stringify!($field)),
                    });
                }
            };
        }
        ensure_field!(domain);
        ensure_field!(transition_profile_digest);
        ensure_field!(transition_profile_binding_digest);
        ensure_field!(chain_asset_binding_digest);
        ensure_field!(final_note_binding_digest);
        ensure_field!(previous_accumulator_digest);
        ensure_field!(previous_recursive_proof_artifact_digest);
        ensure_field!(previous_recursive_proof_open_envelopes_archive_digest);
        ensure_field!(append_opening_preflight_digest);
        ensure_field!(previous_recursive_proof_opening_aggregate_digest);
        ensure_field!(current_hop_opening_aggregate_digest);
        ensure_field!(current_hop_proof_hash);
        ensure_field!(resulting_accumulator_digest);
        ensure_field!(resulting_public_inputs_hash);
        ensure_field!(hop_count);
        ensure_field!(verifier_opening_len);
        ensure_field!(verifier_params_fingerprint);
        ensure_field!(fixed_window_table_schedule_digest);
        ensure_field!(fixed_window_shared_table_manifest_digest);
        ensure_field!(append_boundary_digest);
        Ok(())
    }

    /// Return the Norito-encoded size of this compact boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaRecursiveSpendTransitionProfileV1 {
    /// Validate transition profile shape and local append invariants.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when previous-state fields are present for
    /// an initial hop, absent for an append, or any transition digest/root/note
    /// field is outside the Reserved-lineage circuit contract.
    #[allow(clippy::too_many_lines)]
    pub fn validate_context(&self) -> Result<(), KagemushaFoldError> {
        if self.domain != KAGEMUSHA_RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "transition_profile.domain",
            });
        }
        if self.hop_count == 0 {
            return Err(KagemushaFoldError::Empty);
        }
        let expected_hop_count =
            self.hop_index
                .checked_add(1)
                .ok_or(KagemushaFoldError::TooManyHops {
                    max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                    actual: usize::MAX,
                })?;
        if self.hop_count != expected_hop_count {
            return Err(KagemushaFoldError::HopCountMismatch {
                expected: usize::try_from(expected_hop_count).unwrap_or(usize::MAX),
                actual: self.hop_count,
            });
        }
        let hop_count = usize::try_from(self.hop_count).unwrap_or(usize::MAX);
        if hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
            return Err(KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: hop_count,
            });
        }

        let has_previous = self.hop_index > 0;
        macro_rules! ensure_previous_presence {
            ($field:ident) => {
                if self.$field.is_some() != has_previous {
                    return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: stringify!($field),
                    });
                }
            };
        }
        ensure_previous_presence!(previous_accumulator_digest);
        ensure_previous_presence!(previous_initial_root);
        ensure_previous_presence!(previous_final_root);
        ensure_previous_presence!(previous_current_note);
        ensure_previous_presence!(previous_lineage_digest);
        ensure_previous_presence!(previous_recursive_proof_chain_digest);
        ensure_previous_presence!(previous_recursive_proof_artifact_digest);
        ensure_previous_presence!(previous_accumulator_public_inputs_hash);
        ensure_previous_presence!(previous_recursive_proof_public_inputs_hash);
        ensure_previous_presence!(previous_verifier_witness_batch_digest);
        ensure_previous_presence!(previous_fixed_window_table_base_digest);
        if !has_previous
            && self
                .previous_recursive_proof_open_envelopes_archive_digest
                .is_some()
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive_digest",
            });
        }
        if !has_previous && self.append_opening_preflight_digest.is_some() {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_opening_preflight_digest",
            });
        }
        if !has_previous && self.append_opening_preflight.is_some() {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_opening_preflight",
            });
        }
        if has_previous {
            for (field, digest) in [
                (
                    "previous_accumulator_digest",
                    self.previous_accumulator_digest
                        .expect("checked previous accumulator digest presence"),
                ),
                (
                    "previous_initial_root",
                    self.previous_initial_root
                        .expect("checked previous initial root presence"),
                ),
                (
                    "previous_final_root",
                    self.previous_final_root
                        .expect("checked previous final root presence"),
                ),
                (
                    "previous_lineage_digest",
                    self.previous_lineage_digest
                        .expect("checked previous lineage digest presence"),
                ),
                (
                    "previous_recursive_proof_chain_digest",
                    self.previous_recursive_proof_chain_digest
                        .expect("checked previous proof-chain digest presence"),
                ),
                (
                    "previous_recursive_proof_artifact_digest",
                    self.previous_recursive_proof_artifact_digest
                        .expect("checked previous proof artifact digest presence"),
                ),
                (
                    "previous_verifier_witness_batch_digest",
                    self.previous_verifier_witness_batch_digest
                        .expect("checked previous verifier batch digest presence"),
                ),
                (
                    "previous_fixed_window_table_base_digest",
                    self.previous_fixed_window_table_base_digest
                        .expect("checked previous table-base digest presence"),
                ),
            ] {
                if digest == [0u8; Hash::LENGTH] {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
                }
            }
            if hash_bytes_from_hash(
                self.previous_accumulator_public_inputs_hash
                    .expect("checked previous accumulator public-input hash presence"),
            ) == [0u8; Hash::LENGTH]
            {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "previous_accumulator_public_inputs_hash",
                });
            }
            if hash_bytes_from_hash(
                self.previous_recursive_proof_public_inputs_hash
                    .expect("checked previous proof public-input hash presence"),
            ) == [0u8; Hash::LENGTH]
            {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "previous_recursive_proof_public_inputs_hash",
                });
            }
            if self.previous_accumulator_public_inputs_hash
                != self.previous_recursive_proof_public_inputs_hash
            {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "previous_accumulator_public_inputs_hash",
                });
            }
            if self
                .previous_recursive_proof_open_envelopes_archive_digest
                .is_some_and(|digest| digest == [0u8; Hash::LENGTH])
            {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "previous_recursive_proof_open_envelopes_archive_digest",
                });
            }
            if self.append_opening_preflight_digest.is_some()
                && self
                    .previous_recursive_proof_open_envelopes_archive_digest
                    .is_none()
            {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "append_opening_preflight_digest",
                });
            }
            if self
                .append_opening_preflight_digest
                .is_some_and(|digest| digest == [0u8; Hash::LENGTH])
            {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "append_opening_preflight_digest",
                });
            }
            if let Some(preflight) = &self.append_opening_preflight {
                preflight.validate_context()?;
                let append_digest = self.append_opening_preflight_digest.ok_or(
                    KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "append_opening_preflight_digest",
                    },
                )?;
                if append_digest != preflight.append_opening_preflight_digest {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: "append_opening_preflight_digest",
                    });
                }
                if self.previous_accumulator_digest != Some(preflight.previous_accumulator_digest) {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: "append_opening_preflight.previous_accumulator_digest",
                    });
                }
                if self.previous_recursive_proof_artifact_digest
                    != Some(preflight.previous_recursive_proof_artifact_digest)
                {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: "append_opening_preflight.previous_recursive_proof_artifact_digest",
                    });
                }
                if self.previous_recursive_proof_open_envelopes_archive_digest
                    != Some(preflight.previous_recursive_proof_open_envelopes_archive_digest)
                {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: "append_opening_preflight.previous_recursive_proof_open_envelopes_archive_digest",
                    });
                }
                if self.current_hop_statement.proof_hash != preflight.current_hop_proof_hash {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: "append_opening_preflight.current_hop_proof_hash",
                    });
                }
                if preflight.previous_recursive_proof_preflight.opening_len
                    != self.verifier_opening_len
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.previous_recursive_proof_opening_len",
                    });
                }
                if preflight
                    .previous_recursive_proof_preflight
                    .params_fingerprint
                    != self.verifier_params_fingerprint
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.previous_recursive_proof_params_fingerprint",
                    });
                }
                if preflight
                    .previous_recursive_proof_preflight
                    .fixed_window_table_schedule_digest
                    != self.fixed_window_table_schedule_digest
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.previous_recursive_proof_fixed_window_table_schedule_digest",
                    });
                }
                if preflight
                    .previous_recursive_proof_preflight
                    .fixed_window_shared_table_manifest_digest
                    != self.fixed_window_shared_table_manifest_digest
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.previous_recursive_proof_fixed_window_shared_table_manifest_digest",
                    });
                }
                if preflight.current_hop_preflight.opening_len != self.verifier_opening_len {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.opening_len",
                    });
                }
                if preflight.current_hop_preflight.params_fingerprint
                    != self.verifier_params_fingerprint
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.params_fingerprint",
                    });
                }
                if preflight
                    .current_hop_preflight
                    .fixed_window_table_schedule_digest
                    != self.fixed_window_table_schedule_digest
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.fixed_window_table_schedule_digest",
                    });
                }
                if preflight
                    .current_hop_preflight
                    .fixed_window_shared_table_manifest_digest
                    != self.fixed_window_shared_table_manifest_digest
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.fixed_window_shared_table_manifest_digest",
                    });
                }
                if preflight
                    .current_hop_preflight
                    .fixed_window_table_base_digest
                    != self.current_hop_fixed_window_table_base_digest
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.current_hop_fixed_window_table_base_digest",
                    });
                }
                if preflight.current_hop_preflight.aggregate_digest
                    != self.current_hop_verifier_witness_batch_digest
                {
                    return Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                        field: "append_opening_preflight.current_hop_verifier_witness_batch_digest",
                    });
                }
            }
        }

        macro_rules! ensure_non_zero_bytes {
            ($field:ident) => {
                if self.$field == [0u8; Hash::LENGTH] {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: stringify!($field),
                    });
                }
            };
        }
        ensure_non_zero_bytes!(current_hop_verifier_witness_batch_digest);
        ensure_non_zero_bytes!(current_hop_fixed_window_table_base_digest);
        ensure_non_zero_bytes!(verifier_params_fingerprint);
        ensure_non_zero_bytes!(fixed_window_table_schedule_digest);
        ensure_non_zero_bytes!(fixed_window_shared_table_manifest_digest);
        ensure_non_zero_bytes!(resulting_initial_root);
        ensure_non_zero_bytes!(resulting_final_root);
        ensure_non_zero_bytes!(resulting_lineage_digest);
        ensure_non_zero_bytes!(resulting_verifier_witness_batch_digest);
        ensure_non_zero_bytes!(resulting_fixed_window_table_base_digest);
        ensure_non_zero_bytes!(resulting_recursive_proof_chain_digest);
        ensure_non_zero_bytes!(resulting_accumulator_digest);
        if self.resulting_append_opening_preflight_digest == [0u8; Hash::LENGTH] {
            if self.append_opening_preflight_digest.is_some() {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                    field: "resulting_append_opening_preflight_digest",
                });
            }
        } else if self.append_opening_preflight_digest
            != Some(self.resulting_append_opening_preflight_digest)
        {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "resulting_append_opening_preflight_digest",
            });
        }
        validate_kagemusha_recursive_verifier_opening_len(self.verifier_opening_len)?;
        validate_kagemusha_fold_root("resulting_initial_root", self.resulting_initial_root)?;
        validate_kagemusha_fold_root("resulting_final_root", self.resulting_final_root)?;
        if self.resulting_initial_root == self.resulting_final_root {
            return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
        }
        validate_kagemusha_recursive_spend_note(&self.current_note)?;

        for (field, digest) in [
            (
                "resulting_nullifier_digest",
                self.resulting_nullifier_digest,
            ),
            (
                "resulting_output_commitment_digest",
                self.resulting_output_commitment_digest,
            ),
            ("resulting_fold_digest", self.resulting_fold_digest),
            (
                "resulting_public_inputs_hash",
                self.resulting_public_inputs_hash,
            ),
        ] {
            if hash_bytes_from_hash(digest) == [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
            }
        }

        let canonical_step = kagemusha_recursive_spend_step_statement(
            self.hop_index,
            &kagemusha_recursive_spend_step_from_statement(&self.current_hop_statement),
        )?;
        if canonical_step != self.current_hop_statement {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "current_hop_statement",
            });
        }
        if !self
            .current_hop_statement
            .output_commitments
            .contains(&self.current_note.note_commitment)
        {
            return Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment);
        }
        if self
            .current_hop_statement
            .input_nullifiers
            .contains(&self.current_note.spend_nullifier)
            || self
                .current_hop_statement
                .output_commitments
                .contains(&self.current_note.spend_nullifier)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier",
            });
        }

        if let Some(previous_final_root) = self.previous_final_root {
            validate_kagemusha_fold_root("previous_final_root", previous_final_root)?;
            if previous_final_root != self.current_hop_statement.root_before {
                return Err(KagemushaFoldError::RecursiveSpendRootMismatch);
            }
        } else if self.resulting_initial_root != self.current_hop_statement.root_before {
            return Err(KagemushaFoldError::InitialRootMismatch {
                expected: self.current_hop_statement.root_before,
                actual: self.resulting_initial_root,
            });
        }
        if self.resulting_final_root != self.current_hop_statement.root_after {
            return Err(KagemushaFoldError::FinalRootMismatch {
                expected: self.current_hop_statement.root_after,
                actual: self.resulting_final_root,
            });
        }

        if let Some(previous_initial_root) = self.previous_initial_root {
            validate_kagemusha_fold_root("previous_initial_root", previous_initial_root)?;
            if previous_initial_root != self.resulting_initial_root {
                return Err(KagemushaFoldError::InitialRootMismatch {
                    expected: previous_initial_root,
                    actual: self.resulting_initial_root,
                });
            }
        }

        if let Some(previous_note) = &self.previous_current_note {
            validate_kagemusha_recursive_spend_note(previous_note)?;
            if !self
                .current_hop_statement
                .input_nullifiers
                .contains(&previous_note.spend_nullifier)
            {
                return Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier);
            }
            if self.current_hop_statement.input_nullifiers.len() != 1 {
                return Err(KagemushaFoldError::RecursiveSpendUnexpectedAppendInput);
            }
            if self.current_note.amount != previous_note.amount {
                return Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" });
            }
            if self.current_note.spend_nullifier == previous_note.note_commitment {
                return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                    field: "spend_nullifier",
                });
            }
            if self
                .current_hop_statement
                .output_commitments
                .contains(&previous_note.note_commitment)
            {
                return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                    field: "output_commitments",
                });
            }
        }
        Ok(())
    }

    /// Return the canonical transition profile digest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the profile is malformed.
    pub fn digest(&self) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
        kagemusha_recursive_spend_transition_profile_digest(self)
    }

    /// Return the non-circular transition binding digest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the profile is malformed.
    pub fn binding_digest(&self) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
        kagemusha_recursive_spend_transition_profile_binding_digest(self)
    }

    /// Return the Norito-encoded size of this transition profile.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaRecursiveSpendAccumulatorV1 {
    /// Validate accumulator shape and public verifier corridor.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when any accumulator field is outside the
    /// recursive spend corridor.
    pub fn validate_context(&self) -> Result<(), KagemushaFoldError> {
        if self.domain != KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN {
            return Err(KagemushaFoldError::InvalidRecursiveSpendAccumulatorDomain {
                expected: KAGEMUSHA_RECURSIVE_SPEND_ACCUMULATOR_DOMAIN,
                actual: self.domain.clone(),
            });
        }
        validate_kagemusha_fold_root("initial_root", self.initial_root)?;
        validate_kagemusha_fold_root("final_root", self.final_root)?;
        if self.initial_root == self.final_root {
            return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
        }
        validate_kagemusha_recursive_spend_topup_anchor_nullifiers(&self.topup_anchor_nullifiers)?;
        if self
            .topup_anchor_nullifiers
            .contains(&self.current_note.spend_nullifier)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers",
            });
        }
        if self
            .topup_anchor_nullifiers
            .contains(&self.current_note.note_commitment)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers",
            });
        }
        if self.hop_count == 0 {
            return Err(KagemushaFoldError::Empty);
        }
        let hop_count = usize::try_from(self.hop_count).unwrap_or(usize::MAX);
        if hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
            return Err(KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: hop_count,
            });
        }
        macro_rules! ensure_non_zero_bytes {
            ($field:ident) => {
                if self.$field == [0u8; Hash::LENGTH] {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: stringify!($field),
                    });
                }
            };
        }
        ensure_non_zero_bytes!(lineage_digest);
        ensure_non_zero_bytes!(aggregation_transcript_digest);
        ensure_non_zero_bytes!(verifier_params_fingerprint);
        ensure_non_zero_bytes!(fixed_window_table_schedule_digest);
        ensure_non_zero_bytes!(fixed_window_shared_table_manifest_digest);
        ensure_non_zero_bytes!(fixed_window_table_base_digest);
        ensure_non_zero_bytes!(verifier_witness_batch_digest);
        ensure_non_zero_bytes!(recursive_proof_chain_digest);
        ensure_non_zero_bytes!(transition_profile_binding_digest);
        if self.append_opening_preflight_digest != [0u8; Hash::LENGTH] && self.hop_count <= 1 {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest",
            });
        }
        if self.append_boundary_digest != [0u8; Hash::LENGTH]
            && (self.append_opening_preflight_digest == [0u8; Hash::LENGTH] || self.hop_count <= 1)
        {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary_digest",
            });
        }
        if self.aggregation_transcript_digest != self.lineage_digest {
            return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "aggregation_transcript_digest",
            });
        }
        for (field, digest) in [
            ("nullifier_digest", self.nullifier_digest),
            ("output_commitment_digest", self.output_commitment_digest),
            ("fold_digest", self.fold_digest),
        ] {
            if hash_bytes_from_hash(digest) == [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field });
            }
        }
        validate_kagemusha_recursive_verifier_opening_len(self.verifier_opening_len)?;
        validate_kagemusha_recursive_spend_note(&self.current_note)?;
        Ok(())
    }

    /// Return the chain-visible nullifiers that must be consumed on final redemption.
    ///
    /// This includes the first-hop top-up anchor nullifiers plus the current
    /// spendable note nullifier. Consuming both closes hidden-branch replays:
    /// two recursive branches from the same online-to-offline top-up collide on
    /// the top-up anchor even when their final note nullifiers differ.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the accumulator is malformed.
    pub fn redeem_nullifiers(&self) -> Result<Vec<[u8; Hash::LENGTH]>, KagemushaFoldError> {
        self.validate_context()?;
        let mut nullifiers = self.topup_anchor_nullifiers.clone();
        nullifiers.push(self.current_note.spend_nullifier);
        Ok(nullifiers)
    }

    /// Return the recursive proof public inputs bound to this accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the accumulator cannot be converted
    /// to canonical recursive proof public inputs.
    pub fn recursive_public_inputs(
        &self,
    ) -> Result<KagemushaRecursiveAggregationProofPublicInputs, KagemushaFoldError> {
        kagemusha_recursive_spend_public_inputs_from_accumulator(self)
    }

    /// Return the Norito-encoded size of this accumulator.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaRecursiveSpendBundleV1 {
    /// Validate that the recursive proof public inputs are derived from the accumulator.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the accumulator, proof envelope, or
    /// public-input parity is invalid.
    pub fn validate_public_input_binding(&self) -> Result<(), KagemushaFoldError> {
        let circuit =
            validate_kagemusha_recursive_spend_proof_public_input_binding(&self.recursive_proof)?;
        let expected = expected_kagemusha_recursive_spend_public_inputs_for_proof(
            &self.accumulator,
            &self.recursive_proof,
            circuit,
        )?;
        macro_rules! ensure_field {
            ($field:ident) => {
                if self.recursive_proof.public_inputs.$field != expected.$field {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: stringify!($field),
                    });
                }
            };
        }
        ensure_field!(domain);
        ensure_field!(evidence_digest);
        ensure_field!(folded_public_inputs_hash);
        ensure_field!(aggregation_transcript_digest);
        ensure_field!(verifier_params_fingerprint);
        ensure_field!(fixed_window_table_schedule_digest);
        ensure_field!(fixed_window_shared_table_manifest_digest);
        ensure_field!(fixed_window_table_base_digest);
        ensure_field!(verifier_witness_batch_digest);
        ensure_field!(recursive_proof_chain_digest);
        ensure_field!(transition_profile_binding_digest);
        ensure_field!(append_opening_preflight_digest);
        ensure_field!(append_boundary_digest);
        ensure_field!(recursive_verifier_scalar_projection_digest);
        ensure_field!(verifier_opening_len);
        ensure_field!(hop_count);
        ensure_field!(verifier_witness_count);
        Ok(())
    }

    /// Return the Norito-encoded size of this spendable D2D payload.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

fn validate_kagemusha_recursive_spend_lineage_key_artifact_pair(
    lineage_verifier_key: Option<&VerifyingKeyBox>,
    lineage_proving_key_archive: Option<&[u8]>,
) -> Result<(), KagemushaFoldError> {
    match (lineage_verifier_key, lineage_proving_key_archive) {
        (Some(vk), Some(proving_key_archive)) => {
            if vk.backend.is_empty() || vk.bytes.is_empty() {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "lineage_verifier_key",
                });
            }
            if proving_key_archive.is_empty() {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "lineage_proving_key_archive",
                });
            }
            Ok(())
        }
        (Some(_), None) => Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_proving_key_archive",
        }),
        (None, Some(_)) => Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_verifier_key",
        }),
        (None, None) => Ok(()),
    }
}

const KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct KagemushaLineageProvingKeyArchiveV1 {
    version: u16,
    circuit_family: String,
    vk_commitment: [u8; Hash::LENGTH],
    proving_key: Vec<u8>,
}

/// Encode a Reserved-lineage proving-key archive bound to a verifier key.
///
/// The archive format is intentionally private to this module's validator; use
/// this helper when producing key-artifact packages outside the data model so
/// producers and validators share the exact Norito type identity and binding.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the verifier key envelope is malformed,
/// targets another circuit family, the proving-key payload is empty, or Norito
/// encoding fails.
pub fn kagemusha_lineage_proving_key_archive(
    circuit_family: &str,
    lineage_verifier_key: &VerifyingKeyBox,
    proving_key: Vec<u8>,
) -> Result<Vec<u8>, KagemushaFoldError> {
    if proving_key.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_proving_key_archive",
        });
    }
    let vk_circuit_id = kagemusha_lineage_vk_envelope_circuit_id(lineage_verifier_key)?;
    if vk_circuit_id != circuit_family {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_verifier_key",
        });
    }
    to_bytes(&KagemushaLineageProvingKeyArchiveV1 {
        version: KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1,
        circuit_family: circuit_family.to_owned(),
        vk_commitment: kagemusha_verifying_key_commitment(lineage_verifier_key),
        proving_key,
    })
    .map_err(|_| KagemushaFoldError::InvalidRecursiveSpendProof {
        field: "lineage_proving_key_archive",
    })
}

fn kagemusha_lineage_vk_envelope_circuit_id(
    vk: &VerifyingKeyBox,
) -> Result<String, KagemushaFoldError> {
    const MAGIC: &[u8; 4] = b"ZK1\0";

    if vk.bytes.len() < MAGIC.len() || &vk.bytes[..MAGIC.len()] != MAGIC {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_verifier_key",
        });
    }

    let mut offset = MAGIC.len();
    let mut circuit_id = None;
    let mut saw_ipa_k = false;
    let mut saw_h2vk = false;
    while offset < vk.bytes.len() {
        let Some(tag_end) = offset.checked_add(4) else {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key",
            });
        };
        let Some(len_end) = tag_end.checked_add(4) else {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key",
            });
        };
        if len_end > vk.bytes.len() {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key",
            });
        }
        let tag = &vk.bytes[offset..tag_end];
        let len = u32::from_le_bytes(
            vk.bytes[tag_end..len_end]
                .try_into()
                .expect("TLV length slice is four bytes"),
        );
        let len =
            usize::try_from(len).map_err(|_| KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key",
            })?;
        let Some(payload_end) = len_end.checked_add(len) else {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key",
            });
        };
        if payload_end > vk.bytes.len() {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key",
            });
        }
        let payload = &vk.bytes[len_end..payload_end];
        match tag {
            b"CID1" => {
                if circuit_id.is_some() {
                    return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "lineage_verifier_key",
                    });
                }
                let value = std::str::from_utf8(payload).map(str::trim).map_err(|_| {
                    KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "lineage_verifier_key",
                    }
                })?;
                if value.is_empty() {
                    return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "lineage_verifier_key",
                    });
                }
                circuit_id = Some(value.to_owned());
            }
            b"IPAK" => {
                if saw_ipa_k || payload.len() != 4 {
                    return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "lineage_verifier_key",
                    });
                }
                saw_ipa_k = true;
            }
            b"H2VK" => {
                if saw_h2vk || payload.is_empty() {
                    return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "lineage_verifier_key",
                    });
                }
                saw_h2vk = true;
            }
            _ => {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "lineage_verifier_key",
                });
            }
        }
        offset = payload_end;
    }

    if !saw_ipa_k || !saw_h2vk {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_verifier_key",
        });
    }
    circuit_id.ok_or(KagemushaFoldError::InvalidRecursiveSpendProof {
        field: "lineage_verifier_key",
    })
}

fn validate_kagemusha_recursive_spend_lineage_key_artifact_package_binding(
    proof_circuit_id: &str,
    lineage_verifier_key: &VerifyingKeyBox,
    lineage_proving_key_archive: &[u8],
) -> Result<(), KagemushaFoldError> {
    let vk_circuit_id = kagemusha_lineage_vk_envelope_circuit_id(lineage_verifier_key)?;
    if vk_circuit_id != proof_circuit_id {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_verifier_key",
        });
    }

    let archive: KagemushaLineageProvingKeyArchiveV1 =
        norito::decode_from_bytes(lineage_proving_key_archive).map_err(|_| {
            KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive",
            }
        })?;
    if archive.version != KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1
        || archive.circuit_family != proof_circuit_id
        || archive.vk_commitment != kagemusha_verifying_key_commitment(lineage_verifier_key)
        || archive.proving_key.is_empty()
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_proving_key_archive",
        });
    }
    Ok(())
}

fn is_supported_kagemusha_recursive_spend_lineage_verifier_opening_len(
    verifier_opening_len: u32,
) -> bool {
    KAGEMUSHA_RECURSIVE_COMPACT_SUPPORTED_OPENING_LENS_V1.contains(&verifier_opening_len)
}

fn validate_kagemusha_recursive_compact_verifier_key(
    verifier_key: &VerifyingKeyBox,
) -> Result<(), KagemushaFoldError> {
    if verifier_key.backend.as_str() != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "recursive_compact_verifier_key",
        });
    }
    if verifier_key.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "recursive_compact_verifier_key",
        });
    }
    let circuit_id = kagemusha_lineage_vk_envelope_circuit_id(verifier_key)?;
    if circuit_id != KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1 {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "recursive_compact_verifier_key",
        });
    }
    Ok(())
}

fn validate_kagemusha_recursive_compact_package_widths<I>(
    entries: I,
) -> Result<(), KagemushaFoldError>
where
    I: IntoIterator<Item = u32>,
{
    let mut widths = entries.into_iter().collect::<Vec<_>>();
    if widths.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "recursive_compact_key_artifacts.entries",
        });
    }
    widths.sort_unstable();
    let has_duplicate = widths.windows(2).any(|pair| pair[0] == pair[1]);
    if has_duplicate
        || widths
            .iter()
            .any(|width| !KAGEMUSHA_RECURSIVE_COMPACT_SUPPORTED_OPENING_LENS_V1.contains(width))
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "recursive_compact_key_artifacts.entries",
        });
    }
    Ok(())
}

impl KagemushaRecursiveCompactKeyArtifactEntryV1 {
    /// Build and validate one recursive compact prover key-package entry.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the opening length is unsupported or any
    /// verifier/proving key pair is not bound to the compact circuit id.
    pub fn new(
        verifier_opening_len: u32,
        one_hop_verifier_key: VerifyingKeyBox,
        one_hop_proving_key_archive: Vec<u8>,
        append_verifier_key: VerifyingKeyBox,
        append_proving_key_archive: Vec<u8>,
    ) -> Result<Self, KagemushaFoldError> {
        let entry = Self {
            verifier_opening_len,
            one_hop_verifier_key,
            one_hop_proving_key_archive,
            append_verifier_key,
            append_proving_key_archive,
        };
        entry.validate_public_binding()?;
        Ok(entry)
    }

    /// Validate this entry before proving.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when any field violates the compact key
    /// artifact contract.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        if !is_supported_kagemusha_recursive_spend_lineage_verifier_opening_len(
            self.verifier_opening_len,
        ) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_opening_len",
            });
        }
        validate_kagemusha_recursive_compact_verifier_key(&self.one_hop_verifier_key)?;
        validate_kagemusha_recursive_compact_verifier_key(&self.append_verifier_key)?;
        validate_kagemusha_recursive_spend_lineage_key_artifact_package_binding(
            KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
            &self.one_hop_verifier_key,
            &self.one_hop_proving_key_archive,
        )?;
        validate_kagemusha_recursive_spend_lineage_key_artifact_package_binding(
            KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
            &self.append_verifier_key,
            &self.append_proving_key_archive,
        )
    }
}

impl KagemushaRecursiveCompactKeyArtifactsV1 {
    /// Build and validate a recursive compact prover package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the package is empty, contains
    /// duplicate widths, or contains an invalid entry.
    pub fn new(
        entries: Vec<KagemushaRecursiveCompactKeyArtifactEntryV1>,
    ) -> Result<Self, KagemushaFoldError> {
        let package = Self { entries };
        package.validate_public_binding()?;
        Ok(package)
    }

    /// Validate this recursive compact prover package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when any entry is malformed, duplicated, or
    /// unsupported.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        validate_kagemusha_recursive_compact_package_widths(
            self.entries.iter().map(|entry| entry.verifier_opening_len),
        )?;
        for entry in &self.entries {
            entry.validate_public_binding()?;
        }
        Ok(())
    }

    /// Return the prover entry for `verifier_opening_len`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the package is malformed or the width is
    /// not present.
    pub fn entry_for_opening_len(
        &self,
        verifier_opening_len: u32,
    ) -> Result<&KagemushaRecursiveCompactKeyArtifactEntryV1, KagemushaFoldError> {
        self.validate_public_binding()?;
        self.entries
            .iter()
            .find(|entry| entry.verifier_opening_len == verifier_opening_len)
            .ok_or(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_opening_len",
            })
    }

    /// Return a verifier-only package derived from this prover package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the prover package is malformed.
    pub fn verifier_keys(
        &self,
    ) -> Result<KagemushaRecursiveCompactVerifierKeysV1, KagemushaFoldError> {
        self.validate_public_binding()?;
        KagemushaRecursiveCompactVerifierKeysV1::new(
            self.entries
                .iter()
                .map(|entry| KagemushaRecursiveCompactVerifierKeyEntryV1 {
                    verifier_opening_len: entry.verifier_opening_len,
                    one_hop_verifier_key: entry.one_hop_verifier_key.clone(),
                    append_verifier_key: entry.append_verifier_key.clone(),
                })
                .collect(),
        )
    }

    /// Return the Norito-encoded size of this prover package.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaRecursiveCompactVerifierKeyEntryV1 {
    /// Build and validate one recursive compact verifier package entry.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the opening length is unsupported or any
    /// verifier key is not bound to the compact circuit id.
    pub fn new(
        verifier_opening_len: u32,
        one_hop_verifier_key: VerifyingKeyBox,
        append_verifier_key: VerifyingKeyBox,
    ) -> Result<Self, KagemushaFoldError> {
        let entry = Self {
            verifier_opening_len,
            one_hop_verifier_key,
            append_verifier_key,
        };
        entry.validate_public_binding()?;
        Ok(entry)
    }

    /// Validate this verifier package entry.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when any field violates the verifier-key
    /// package contract.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        if !is_supported_kagemusha_recursive_spend_lineage_verifier_opening_len(
            self.verifier_opening_len,
        ) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_opening_len",
            });
        }
        validate_kagemusha_recursive_compact_verifier_key(&self.one_hop_verifier_key)?;
        validate_kagemusha_recursive_compact_verifier_key(&self.append_verifier_key)
    }
}

impl KagemushaRecursiveCompactVerifierKeysV1 {
    /// Build and validate a recursive compact verifier package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the package is empty, contains
    /// duplicate widths, or contains an invalid entry.
    pub fn new(
        entries: Vec<KagemushaRecursiveCompactVerifierKeyEntryV1>,
    ) -> Result<Self, KagemushaFoldError> {
        let package = Self { entries };
        package.validate_public_binding()?;
        Ok(package)
    }

    /// Validate this recursive compact verifier package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when any entry is malformed, duplicated, or
    /// unsupported.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        validate_kagemusha_recursive_compact_package_widths(
            self.entries.iter().map(|entry| entry.verifier_opening_len),
        )?;
        for entry in &self.entries {
            entry.validate_public_binding()?;
        }
        Ok(())
    }

    /// Return the verifier entry for `verifier_opening_len`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the package is malformed or the width is
    /// not present.
    pub fn entry_for_opening_len(
        &self,
        verifier_opening_len: u32,
    ) -> Result<&KagemushaRecursiveCompactVerifierKeyEntryV1, KagemushaFoldError> {
        self.validate_public_binding()?;
        self.entries
            .iter()
            .find(|entry| entry.verifier_opening_len == verifier_opening_len)
            .ok_or(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_opening_len",
            })
    }

    /// Return the Norito-encoded size of this verifier package.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaRecursiveSpendLineageKeyArtifactsV1 {
    /// Build and validate a portable Reserved-lineage key artifact package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the circuit id is not a
    /// profile-specific Reserved-lineage id, the opening length is unsupported,
    /// or either packaged key artifact is malformed.
    pub fn new(
        proof_circuit_id: impl Into<String>,
        verifier_opening_len: u32,
        lineage_verifier_key: VerifyingKeyBox,
        lineage_proving_key_archive: Vec<u8>,
    ) -> Result<Self, KagemushaFoldError> {
        let artifacts = Self {
            proof_circuit_id: proof_circuit_id.into(),
            verifier_opening_len,
            lineage_verifier_key,
            lineage_proving_key_archive,
        };
        artifacts.validate_public_binding()?;
        Ok(artifacts)
    }

    /// Build artifacts for first-hop Reserved-lineage init proving.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when validation fails.
    pub fn new_for_init(
        verifier_opening_len: u32,
        lineage_verifier_key: VerifyingKeyBox,
        lineage_proving_key_archive: Vec<u8>,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            verifier_opening_len,
            lineage_verifier_key,
            lineage_proving_key_archive,
        )
    }

    /// Build artifacts for Reserved-lineage append proving.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when validation fails.
    pub fn new_for_append(
        verifier_opening_len: u32,
        lineage_verifier_key: VerifyingKeyBox,
        lineage_proving_key_archive: Vec<u8>,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            verifier_opening_len,
            lineage_verifier_key,
            lineage_proving_key_archive,
        )
    }

    /// Return `true` when this artifact package targets first-hop init.
    #[must_use]
    pub fn is_init_artifact(&self) -> bool {
        self.proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
    }

    /// Return `true` when this artifact package targets Reserved-lineage append.
    #[must_use]
    pub fn is_append_artifact(&self) -> bool {
        self.proof_circuit_id == KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
    }

    /// Validate the portable artifact package before attaching it to a request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when any field is outside the
    /// Reserved-lineage artifact contract.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        if !matches!(
            self.proof_circuit_id.as_str(),
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
                | KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        ) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof_circuit_id",
            });
        }
        if !is_supported_kagemusha_recursive_spend_lineage_verifier_opening_len(
            self.verifier_opening_len,
        ) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_opening_len",
            });
        }
        if self.lineage_verifier_key.backend.as_str()
            != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key",
            });
        }
        validate_kagemusha_recursive_spend_lineage_key_artifact_pair(
            Some(&self.lineage_verifier_key),
            Some(&self.lineage_proving_key_archive),
        )?;
        validate_kagemusha_recursive_spend_lineage_key_artifact_package_binding(
            &self.proof_circuit_id,
            &self.lineage_verifier_key,
            &self.lineage_proving_key_archive,
        )
    }

    /// Return the verified key artifacts as request fields.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] if the artifact package is malformed.
    pub fn into_key_artifacts(self) -> Result<(VerifyingKeyBox, Vec<u8>), KagemushaFoldError> {
        self.validate_public_binding()?;
        Ok((self.lineage_verifier_key, self.lineage_proving_key_archive))
    }

    /// Return the Norito-encoded size of this artifact package.
    ///
    /// # Errors
    ///
    /// Returns an error when Norito encoding fails.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaRecursiveSpendInitRequestV1 {
    /// Build and validate the first-hop recursive spend init request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the checked hop fragment, Pallas
    /// envelope archive, or spendable note binding is malformed.
    pub fn new(
        record_bundle: KagemushaVerifiedFoldRecordBundle,
        pallas_open_envelopes_archive: Vec<u8>,
        current_note: KagemushaSpendableNoteDescriptorV1,
    ) -> Result<Self, KagemushaFoldError> {
        let request = Self {
            record_bundle,
            pallas_open_envelopes_archive,
            current_note,
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        };
        request.validate_public_binding()?;
        Ok(request)
    }

    /// Build and validate an init request with packaged Reserved-lineage proof artifacts.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when either key artifact is missing,
    /// empty, or the resulting init request no longer satisfies its public
    /// binding.
    pub fn new_with_lineage_key_artifacts(
        record_bundle: KagemushaVerifiedFoldRecordBundle,
        pallas_open_envelopes_archive: Vec<u8>,
        current_note: KagemushaSpendableNoteDescriptorV1,
        lineage_verifier_key: VerifyingKeyBox,
        lineage_proving_key_archive: Vec<u8>,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new(record_bundle, pallas_open_envelopes_archive, current_note)?
            .with_lineage_key_artifacts(lineage_verifier_key, lineage_proving_key_archive)
    }

    /// Build and validate an init request from a portable key artifact package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the artifact package is malformed,
    /// targets the append circuit, or the resulting init request no longer
    /// satisfies its public binding.
    pub fn new_with_lineage_key_artifact_package(
        record_bundle: KagemushaVerifiedFoldRecordBundle,
        pallas_open_envelopes_archive: Vec<u8>,
        current_note: KagemushaSpendableNoteDescriptorV1,
        artifacts: KagemushaRecursiveSpendLineageKeyArtifactsV1,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new(record_bundle, pallas_open_envelopes_archive, current_note)?
            .with_lineage_key_artifact_package(artifacts)
    }

    /// Attach packaged Reserved-lineage proving artifacts to an init request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when either key artifact is empty or the
    /// resulting init request no longer satisfies its public binding.
    pub fn with_lineage_key_artifacts(
        mut self,
        lineage_verifier_key: VerifyingKeyBox,
        lineage_proving_key_archive: Vec<u8>,
    ) -> Result<Self, KagemushaFoldError> {
        self.lineage_verifier_key = Some(lineage_verifier_key);
        self.lineage_proving_key_archive = Some(lineage_proving_key_archive);
        self.validate_public_binding()?;
        Ok(self)
    }

    /// Attach a portable Reserved-lineage key artifact package to an init request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the package is malformed, targets
    /// the append circuit, or the resulting init request no longer satisfies
    /// its public binding.
    pub fn with_lineage_key_artifact_package(
        self,
        artifacts: KagemushaRecursiveSpendLineageKeyArtifactsV1,
    ) -> Result<Self, KagemushaFoldError> {
        if !artifacts.is_init_artifact() {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof_circuit_id",
            });
        }
        let (lineage_verifier_key, lineage_proving_key_archive) = artifacts.into_key_artifacts()?;
        self.with_lineage_key_artifacts(lineage_verifier_key, lineage_proving_key_archive)
    }

    /// Attach a verifier-record activation height to an init request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the resulting init request no longer
    /// satisfies its public binding.
    pub fn with_block_height(mut self, block_height: u64) -> Result<Self, KagemushaFoldError> {
        self.block_height = Some(block_height);
        self.validate_public_binding()?;
        Ok(self)
    }

    /// Validate the one-hop init request before proving the first recursive spend bundle.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the checked hop fragment, Pallas
    /// envelope archive, verifier-record metadata, or spendable note binding is
    /// malformed.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        validate_kagemusha_recursive_lineage_record_fragment(
            &self.record_bundle,
            &self.pallas_open_envelopes_archive,
            1,
        )?;
        validate_kagemusha_recursive_spend_lineage_key_artifact_pair(
            self.lineage_verifier_key.as_ref(),
            self.lineage_proving_key_archive.as_deref(),
        )?;
        let step = self
            .record_bundle
            .bundle
            .steps
            .first()
            .ok_or(KagemushaFoldError::Empty)?;
        validate_kagemusha_recursive_spend_request_note_for_step(step, &self.current_note)
    }
}

impl KagemushaRecursiveSpendAppendRequestV1 {
    /// Build and validate a semantic recursive spend append request.
    ///
    /// Semantic append requests leave the previous lineage verifier record and
    /// previous-proof opening archive empty for ABI compatibility.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the previous bundle, checked hop
    /// fragment, Pallas envelope archive, or next spendable note binding is
    /// malformed.
    pub fn new(
        previous_bundle: KagemushaRecursiveSpendBundleV1,
        record_bundle: KagemushaVerifiedFoldRecordBundle,
        pallas_open_envelopes_archive: Vec<u8>,
        current_note: KagemushaSpendableNoteDescriptorV1,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new_with_previous_proof_witness(
            previous_bundle,
            None,
            Vec::new(),
            record_bundle,
            pallas_open_envelopes_archive,
            current_note,
        )
    }

    /// Build and validate an append request with previous-proof verifier material.
    ///
    /// Reserved-lineage previous proofs must provide the active lineage verifier
    /// record. The previous-proof opening archive is optional for legacy semantic
    /// append, but when supplied it must decode as exactly one
    /// `iroha_zkp_halo2::OpenVerifyEnvelope`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when any append public binding, verifier
    /// record selection, or previous-proof opening archive is malformed.
    pub fn new_with_previous_proof_witness(
        previous_bundle: KagemushaRecursiveSpendBundleV1,
        previous_lineage_verifier_record: Option<VerifyingKeyRecord>,
        previous_recursive_proof_open_envelopes_archive: Vec<u8>,
        record_bundle: KagemushaVerifiedFoldRecordBundle,
        pallas_open_envelopes_archive: Vec<u8>,
        current_note: KagemushaSpendableNoteDescriptorV1,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new_with_previous_proof_witness_and_output_circuit(
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            previous_bundle,
            previous_lineage_verifier_record,
            previous_recursive_proof_open_envelopes_archive,
            record_bundle,
            pallas_open_envelopes_archive,
            current_note,
        )
    }

    /// Build and validate an append request with explicit output proof circuit selection.
    ///
    /// Missing or empty `output_proof_circuit_id` values are accepted only after
    /// archive decoding for legacy compatibility. New callers should pass either
    /// [`KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1`] or
    /// [`KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1`].
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the output circuit id is unsupported
    /// or any append public binding is malformed.
    pub fn new_with_previous_proof_witness_and_output_circuit(
        output_proof_circuit_id: impl Into<String>,
        previous_bundle: KagemushaRecursiveSpendBundleV1,
        previous_lineage_verifier_record: Option<VerifyingKeyRecord>,
        previous_recursive_proof_open_envelopes_archive: Vec<u8>,
        record_bundle: KagemushaVerifiedFoldRecordBundle,
        pallas_open_envelopes_archive: Vec<u8>,
        current_note: KagemushaSpendableNoteDescriptorV1,
    ) -> Result<Self, KagemushaFoldError> {
        let request = Self {
            previous_bundle,
            record_bundle,
            pallas_open_envelopes_archive,
            current_note,
            output_proof_circuit_id: output_proof_circuit_id.into(),
            previous_lineage_verifier_record,
            previous_recursive_proof_open_envelopes_archive,
            lineage_verifier_key: None,
            lineage_proving_key_archive: None,
            block_height: None,
        };
        request.validate_public_binding()?;
        Ok(request)
    }

    /// Build and validate a Reserved-lineage append request with packaged key artifacts.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when either key artifact is missing,
    /// empty, or any append public binding is malformed.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_previous_lineage_proof_witness_and_key_artifacts(
        previous_bundle: KagemushaRecursiveSpendBundleV1,
        previous_lineage_verifier_record: Option<VerifyingKeyRecord>,
        previous_recursive_proof_open_envelopes_archive: Vec<u8>,
        record_bundle: KagemushaVerifiedFoldRecordBundle,
        pallas_open_envelopes_archive: Vec<u8>,
        current_note: KagemushaSpendableNoteDescriptorV1,
        lineage_verifier_key: VerifyingKeyBox,
        lineage_proving_key_archive: Vec<u8>,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new_with_previous_proof_witness_and_output_circuit(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            previous_bundle,
            previous_lineage_verifier_record,
            previous_recursive_proof_open_envelopes_archive,
            record_bundle,
            pallas_open_envelopes_archive,
            current_note,
        )?
        .with_lineage_key_artifacts(lineage_verifier_key, lineage_proving_key_archive)
    }

    /// Build and validate a Reserved-lineage append request from a portable key artifact package.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the package is malformed, targets
    /// init, or any append public binding is malformed.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_previous_lineage_proof_witness_and_key_artifact_package(
        previous_bundle: KagemushaRecursiveSpendBundleV1,
        previous_lineage_verifier_record: Option<VerifyingKeyRecord>,
        previous_recursive_proof_open_envelopes_archive: Vec<u8>,
        record_bundle: KagemushaVerifiedFoldRecordBundle,
        pallas_open_envelopes_archive: Vec<u8>,
        current_note: KagemushaSpendableNoteDescriptorV1,
        artifacts: KagemushaRecursiveSpendLineageKeyArtifactsV1,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new_with_previous_proof_witness_and_output_circuit(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            previous_bundle,
            previous_lineage_verifier_record,
            previous_recursive_proof_open_envelopes_archive,
            record_bundle,
            pallas_open_envelopes_archive,
            current_note,
        )?
        .with_lineage_key_artifact_package(artifacts)
    }

    /// Attach packaged Reserved-lineage proving artifacts to an append request.
    ///
    /// The request must select the Reserved-lineage append output circuit. The
    /// semantic append output rejects these artifacts so legacy callers cannot
    /// accidentally smuggle unused key material through the ABI.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when either key artifact is empty, when the
    /// append output circuit does not consume Reserved-lineage key artifacts, or
    /// when the resulting append request no longer satisfies its public binding.
    pub fn with_lineage_key_artifacts(
        mut self,
        lineage_verifier_key: VerifyingKeyBox,
        lineage_proving_key_archive: Vec<u8>,
    ) -> Result<Self, KagemushaFoldError> {
        self.lineage_verifier_key = Some(lineage_verifier_key);
        self.lineage_proving_key_archive = Some(lineage_proving_key_archive);
        self.validate_public_binding()?;
        Ok(self)
    }

    /// Attach a portable Reserved-lineage key artifact package to an append request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the package is malformed, targets
    /// init, the request does not select Reserved-lineage output, or the
    /// resulting append request no longer satisfies its public binding.
    pub fn with_lineage_key_artifact_package(
        self,
        artifacts: KagemushaRecursiveSpendLineageKeyArtifactsV1,
    ) -> Result<Self, KagemushaFoldError> {
        if !artifacts.is_append_artifact() {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof_circuit_id",
            });
        }
        let (lineage_verifier_key, lineage_proving_key_archive) = artifacts.into_key_artifacts()?;
        self.with_lineage_key_artifacts(lineage_verifier_key, lineage_proving_key_archive)
    }

    /// Attach a verifier-record activation height to an append request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the resulting append request no
    /// longer satisfies its public binding.
    pub fn with_block_height(mut self, block_height: u64) -> Result<Self, KagemushaFoldError> {
        self.block_height = Some(block_height);
        self.validate_public_binding()?;
        Ok(self)
    }

    /// Return the selected append output proof circuit id.
    #[must_use]
    pub fn output_proof_circuit_id(&self) -> &str {
        normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
            &self.output_proof_circuit_id,
        )
    }

    /// Validate the one-hop append request before proving the next recursive spend bundle.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the previous bundle, previous
    /// lineage verifier-record selection, checked hop fragment, Pallas envelope
    /// archive, or next spendable note binding is malformed.
    #[allow(clippy::too_many_lines)]
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        let output_proof_circuit_id = self.output_proof_circuit_id();
        if !is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
            output_proof_circuit_id,
        ) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "output_proof_circuit_id",
            });
        }
        validate_kagemusha_recursive_spend_bundle_production_proof_attachment(
            &self.previous_bundle,
        )?;
        self.previous_bundle.validate_public_input_binding()?;
        validate_kagemusha_recursive_spend_previous_lineage_record_selection(
            &self.previous_bundle,
            self.previous_lineage_verifier_record.as_ref(),
            KagemushaRecursiveSpendLineageRecordFieldNames::PREVIOUS_LINEAGE_VERIFIER_RECORD,
        )?;
        validate_kagemusha_recursive_previous_proof_open_envelopes_archive(
            &self.previous_bundle,
            &self.previous_recursive_proof_open_envelopes_archive,
            requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                output_proof_circuit_id,
                self.previous_bundle.accumulator.hop_count,
            ),
        )?;
        if is_kagemusha_recursive_spend_lineage_append_output_circuit_id(output_proof_circuit_id) {
            validate_kagemusha_recursive_spend_lineage_key_artifact_pair(
                self.lineage_verifier_key.as_ref(),
                self.lineage_proving_key_archive.as_deref(),
            )?;
        } else if self.lineage_verifier_key.is_some() || self.lineage_proving_key_archive.is_some()
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_key_artifacts",
            });
        }
        validate_kagemusha_recursive_spend_append_output_selection(
            &self.previous_bundle,
            output_proof_circuit_id,
        )?;
        validate_kagemusha_recursive_lineage_record_fragment(
            &self.record_bundle,
            &self.pallas_open_envelopes_archive,
            1,
        )?;

        if self.record_bundle.bundle.chain_id != self.previous_bundle.accumulator.chain_id {
            return Err(KagemushaFoldError::RecursiveSpendChainMismatch);
        }
        if self.record_bundle.bundle.asset != self.previous_bundle.accumulator.asset {
            return Err(KagemushaFoldError::RecursiveSpendAssetMismatch);
        }

        let step = self
            .record_bundle
            .bundle
            .steps
            .first()
            .ok_or(KagemushaFoldError::Empty)?;
        if step.root_before != self.previous_bundle.accumulator.final_root {
            return Err(KagemushaFoldError::RecursiveSpendRootMismatch);
        }
        if self.current_note.amount != self.previous_bundle.accumulator.current_note.amount {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" });
        }
        if step.input_nullifiers.len() != 1 {
            return Err(KagemushaFoldError::RecursiveSpendUnexpectedAppendInput);
        }
        if !step.input_nullifiers.iter().any(|nullifier| {
            nullifier
                == &self
                    .previous_bundle
                    .accumulator
                    .current_note
                    .spend_nullifier
        }) {
            return Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier);
        }
        if self.current_note.spend_nullifier
            == self
                .previous_bundle
                .accumulator
                .current_note
                .note_commitment
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "current_note.spend_nullifier",
            });
        }
        if step.output_commitments.iter().any(|commitment| {
            commitment
                == &self
                    .previous_bundle
                    .accumulator
                    .current_note
                    .note_commitment
        }) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "record_bundle.bundle.steps.output_commitments",
            });
        }
        if step.output_commitments.iter().any(|commitment| {
            self.previous_bundle
                .accumulator
                .topup_anchor_nullifiers
                .iter()
                .any(|anchor| anchor == commitment)
        }) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "record_bundle.bundle.steps.output_commitments",
            });
        }

        validate_kagemusha_recursive_spend_request_note_for_step(step, &self.current_note)
    }
}

impl KagemushaRecursiveSpendVerifyRequestV1 {
    /// Build and validate a semantic recursive spend verify request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the bundle binding is malformed or the
    /// bundle requires a lineage verifier record.
    pub fn new(bundle: KagemushaRecursiveSpendBundleV1) -> Result<Self, KagemushaFoldError> {
        Self::new_with_lineage_verifier_record(bundle, None)
    }

    /// Build and validate a recursive spend verify request with optional lineage record.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the bundle binding is malformed, a
    /// semantic bundle carries a lineage verifier record, or a Reserved-lineage
    /// bundle omits or mismatches its lineage verifier record.
    pub fn new_with_lineage_verifier_record(
        bundle: KagemushaRecursiveSpendBundleV1,
        lineage_verifier_record: Option<VerifyingKeyRecord>,
    ) -> Result<Self, KagemushaFoldError> {
        let request = Self {
            bundle,
            lineage_verifier_record,
            block_height: None,
        };
        request.validate_public_binding()?;
        Ok(request)
    }

    /// Validate verifier-record selection for a recursive spend verify request.
    ///
    /// This is a deterministic metadata guard. It does not replace backend
    /// proof verification in `iroha_core`.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the bundle binding is malformed, a
    /// semantic bundle carries a lineage verifier record, or a reserved-lineage
    /// bundle omits or mismatches its lineage verifier record metadata.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        validate_kagemusha_recursive_spend_bundle_production_proof_attachment(&self.bundle)?;
        self.bundle.validate_public_input_binding()?;
        validate_kagemusha_recursive_spend_previous_lineage_record_selection(
            &self.bundle,
            self.lineage_verifier_record.as_ref(),
            KagemushaRecursiveSpendLineageRecordFieldNames::LINEAGE_VERIFIER_RECORD,
        )
    }
}

/// Build the record-backed lineage witness that corresponds to the first recursive spend bundle.
///
/// This witness is not part of the constant-size D2D bundle. Wallets keep it as
/// redeem-side audit material for semantic recursive spend bundles and as a
/// compatibility fallback outside the witnessless Reserved-lineage cap.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the request is not a single-hop lineage
/// fragment, when its Pallas envelope archive is malformed, or when the result
/// does not bind to `bundle`.
pub fn kagemusha_recursive_spend_lineage_witness_from_init_result(
    request: &KagemushaRecursiveSpendInitRequestV1,
    bundle: &KagemushaRecursiveSpendBundleV1,
) -> Result<KagemushaRecursiveSpendLineageWitnessV1, KagemushaFoldError> {
    request.validate_public_binding()?;
    let witness = KagemushaRecursiveSpendLineageWitnessV1 {
        record_bundle: request.record_bundle.clone(),
        pallas_open_envelopes_archive: request.pallas_open_envelopes_archive.clone(),
        current_notes: vec![request.current_note.clone()],
        previous_recursive_proofs: Vec::new(),
    };
    validate_kagemusha_recursive_spend_lineage_witness(bundle, &witness)?;
    Ok(witness)
}

/// Append one hop of record-backed redeem witness material alongside a recursive spend append.
///
/// `append_request.previous_bundle` must be the bundle that `previous_witness`
/// already describes, and `appended_bundle` must be the newly proved recursive
/// spend bundle. The returned witness can be stored separately from the D2D
/// bundle and later attached to [`KagemushaRecursiveSpendRedeemRequestV1`].
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous witness is not bound to the
/// previous bundle, when the new hop is not a single-hop lineage fragment, when
/// verifier records conflict, when Pallas envelope archives cannot be merged, or
/// when the appended witness does not bind to `appended_bundle`.
pub fn kagemusha_recursive_spend_lineage_witness_append_result(
    previous_witness: &KagemushaRecursiveSpendLineageWitnessV1,
    append_request: &KagemushaRecursiveSpendAppendRequestV1,
    appended_bundle: &KagemushaRecursiveSpendBundleV1,
) -> Result<KagemushaRecursiveSpendLineageWitnessV1, KagemushaFoldError> {
    append_request.validate_public_binding()?;
    validate_kagemusha_recursive_spend_lineage_witness(
        &append_request.previous_bundle,
        previous_witness,
    )?;
    validate_kagemusha_recursive_lineage_record_fragment(
        &append_request.record_bundle,
        &append_request.pallas_open_envelopes_archive,
        1,
    )?;

    let previous_hops = previous_witness.record_bundle.bundle.steps.len();
    let mut envelopes = decode_kagemusha_recursive_lineage_open_envelopes(
        &previous_witness.pallas_open_envelopes_archive,
        previous_hops,
    )?;
    let mut appended_envelopes = decode_kagemusha_recursive_lineage_open_envelopes(
        &append_request.pallas_open_envelopes_archive,
        1,
    )?;
    envelopes.append(&mut appended_envelopes);
    let pallas_open_envelopes_archive =
        to_bytes(&envelopes).map_err(|_| KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.pallas_open_envelopes_archive",
        })?;

    let mut record_bundle = previous_witness.record_bundle.clone();
    if record_bundle.bundle.chain_id != append_request.record_bundle.bundle.chain_id {
        return Err(KagemushaFoldError::RecursiveSpendChainMismatch);
    }
    if record_bundle.bundle.asset != append_request.record_bundle.bundle.asset {
        return Err(KagemushaFoldError::RecursiveSpendAssetMismatch);
    }
    record_bundle
        .bundle
        .steps
        .extend(append_request.record_bundle.bundle.steps.clone());
    for entry in &append_request.record_bundle.verifier_records {
        match record_bundle
            .verifier_records
            .iter()
            .find(|existing| existing.id == entry.id)
        {
            Some(existing) if existing == entry => {}
            Some(_) => {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "lineage_witness.record_bundle.verifier_records.conflict",
                });
            }
            None => record_bundle.verifier_records.push(entry.clone()),
        }
    }

    let mut current_notes = previous_witness.current_notes.clone();
    current_notes.push(append_request.current_note.clone());
    let mut previous_recursive_proofs = previous_witness.previous_recursive_proofs.clone();
    previous_recursive_proofs.push(append_request.previous_bundle.recursive_proof.clone());
    let witness = KagemushaRecursiveSpendLineageWitnessV1 {
        record_bundle,
        pallas_open_envelopes_archive,
        current_notes,
        previous_recursive_proofs,
    };
    validate_kagemusha_recursive_spend_lineage_witness(appended_bundle, &witness)?;
    Ok(witness)
}

fn validate_kagemusha_recursive_lineage_record_fragment(
    record_bundle: &KagemushaVerifiedFoldRecordBundle,
    pallas_open_envelopes_archive: &[u8],
    expected_hops: usize,
) -> Result<(), KagemushaFoldError> {
    if record_bundle.bundle.steps.len() != expected_hops {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: expected_hops,
            actual: u32::try_from(record_bundle.bundle.steps.len()).unwrap_or(u32::MAX),
        });
    }
    validate_kagemusha_verified_fold_lineage_steps(&record_bundle.bundle)?;
    validate_kagemusha_verified_fold_record_bundle_exact_records(record_bundle)?;
    let envelopes = decode_kagemusha_recursive_lineage_open_envelopes(
        pallas_open_envelopes_archive,
        expected_hops,
    )?;
    validate_kagemusha_recursive_lineage_open_envelope_metadata(record_bundle, &envelopes)?;
    Ok(())
}

fn validate_kagemusha_verified_fold_lineage_steps(
    bundle: &KagemushaVerifiedFoldBundle,
) -> Result<(), KagemushaFoldError> {
    if bundle.steps.is_empty() {
        return Err(KagemushaFoldError::Empty);
    }
    if bundle.steps.len() > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: bundle.steps.len(),
        });
    }

    let mut expected_root = bundle
        .steps
        .first()
        .expect("validated non-empty lineage steps")
        .root_before;
    validate_kagemusha_fold_root("initial_root", expected_root)?;
    let mut seen_inputs = std::collections::BTreeSet::new();
    let mut seen_outputs = std::collections::BTreeSet::new();

    for (hop_index, step) in bundle.steps.iter().enumerate() {
        validate_kagemusha_verifier_key_id(hop_index, &step.attachment.vk_ref)?;
        validate_kagemusha_verified_fold_lineage_step_attachment(hop_index, step)?;
        validate_kagemusha_step_shape_and_sets(
            hop_index,
            &step.input_nullifiers,
            &step.output_commitments,
        )?;
        validate_kagemusha_fold_root("root_before", step.root_before)?;
        validate_kagemusha_fold_root("root_after", step.root_after)?;
        validate_kagemusha_root_transition(hop_index, step.root_before, step.root_after)?;
        if step.root_before != expected_root {
            return Err(KagemushaFoldError::RootDiscontinuity {
                hop_index,
                expected: expected_root,
                actual: step.root_before,
            });
        }

        let mut input_nullifiers = step.input_nullifiers.clone();
        input_nullifiers.sort_unstable();
        for input in input_nullifiers {
            if seen_outputs.contains(&input) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
            if !seen_inputs.insert(input) {
                return Err(KagemushaFoldError::DuplicateInputNullifier { hop_index });
            }
        }

        let mut output_commitments = step.output_commitments.clone();
        output_commitments.sort_unstable();
        for output in output_commitments {
            if seen_inputs.contains(&output) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
            if !seen_outputs.insert(output) {
                return Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index });
            }
        }

        expected_root = step.root_after;
    }

    let initial_root = bundle
        .steps
        .first()
        .expect("validated non-empty lineage steps")
        .root_before;
    if initial_root == expected_root {
        return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
    }
    Ok(())
}

fn validate_kagemusha_verified_fold_lineage_step_attachment(
    hop_index: usize,
    step: &KagemushaVerifiedFoldStep,
) -> Result<(), KagemushaFoldError> {
    if !is_supported_kagemusha_proof_backend(step.attachment.backend.as_str()) {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: step.attachment.backend.clone(),
        });
    }
    if step.attachment.proof.backend != step.attachment.backend {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.backend",
        });
    }
    if step.attachment.vk_ref.backend != step.attachment.backend {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.bundle.steps.attachment.vk_ref.backend",
        });
    }
    if step.verifier_key.backend != step.attachment.backend {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.bundle.steps.verifier_key.backend",
        });
    }
    if step.attachment.proof.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.bytes",
        });
    }
    match step.attachment.vk_commitment {
        Some(commitment) if commitment != [0u8; Hash::LENGTH] => {}
        Some(_) => return Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index }),
        None => {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.bundle.steps.attachment.vk_commitment",
            });
        }
    }
    kagemusha_verifier_key_poseidon_digest(
        step.verifier_key.backend.as_str(),
        &step.verifier_key.bytes,
    )?;
    Ok(())
}

fn validate_kagemusha_verified_fold_record_bundle_exact_records(
    record_bundle: &KagemushaVerifiedFoldRecordBundle,
) -> Result<(), KagemushaFoldError> {
    let required_records = record_bundle
        .bundle
        .steps
        .iter()
        .map(|step| step.attachment.vk_ref.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut supplied_records = std::collections::BTreeSet::new();
    for entry in &record_bundle.verifier_records {
        if !supplied_records.insert(entry.id.clone()) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.duplicate",
            });
        }
        if !required_records.contains(&entry.id) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.unreferenced",
            });
        }
    }
    if supplied_records != required_records {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.missing",
        });
    }
    for (hop_index, step) in record_bundle.bundle.steps.iter().enumerate() {
        let entry = record_bundle
            .verifier_records
            .iter()
            .find(|entry| entry.id == step.attachment.vk_ref)
            .ok_or(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.missing",
            })?;
        validate_kagemusha_verified_fold_lineage_record_entry(hop_index, step, entry)?;
    }
    Ok(())
}

fn validate_kagemusha_verified_fold_lineage_record_entry(
    hop_index: usize,
    step: &KagemushaVerifiedFoldStep,
    entry: &KagemushaVerifiedFoldVerifierRecord,
) -> Result<(), KagemushaFoldError> {
    if !entry.record.is_active() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.status",
        });
    }
    if entry.record.commitment == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index });
    }
    if step.attachment.vk_commitment != Some(entry.record.commitment) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.commitment",
        });
    }
    if entry.record.namespace != KAGEMUSHA_VERIFIER_NAMESPACE {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.namespace",
        });
    }
    let Some(expected_backend) = kagemusha_backend_tag(step.attachment.backend.as_str()) else {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: step.attachment.backend.clone(),
        });
    };
    if entry.record.backend != expected_backend {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.backend",
        });
    }
    if kagemusha_record_curve_for_backend(expected_backend) != Some(entry.record.curve.as_str()) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.curve",
        });
    }
    if entry.record.circuit_id.trim().is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.circuit_id",
        });
    }
    if entry.record.public_inputs_schema_hash == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.public_inputs_schema_hash",
        });
    }
    if entry.record.max_proof_bytes == 0
        || step.attachment.proof.bytes.len() > entry.record.max_proof_bytes as usize
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.max_proof_bytes",
        });
    }
    if u32::try_from(step.verifier_key.bytes.len()).ok() != Some(entry.record.vk_len) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.vk_len",
        });
    }
    match &entry.record.key {
        Some(inline_key) if inline_key == &step.verifier_key => {}
        Some(_) | None => {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.key",
            });
        }
    }
    if entry.record.commitment != kagemusha_verifying_key_commitment(&step.verifier_key) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.verifier_records.key_commitment",
        });
    }
    if step.attachment.backend.as_str() == "halo2/ipa" {
        let proof_envelope =
            kagemusha_verified_fold_lineage_step_open_verify_envelope(hop_index, step)?;
        if proof_envelope.circuit_id != entry.record.circuit_id {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.circuit_id",
            });
        }
        let proof_schema_hash: [u8; Hash::LENGTH] =
            Hash::new(proof_envelope.public_inputs.as_slice()).into();
        if entry.record.public_inputs_schema_hash != proof_schema_hash {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.public_inputs_schema_hash",
            });
        }
    }
    Ok(())
}

fn kagemusha_verified_fold_lineage_step_open_verify_envelope(
    _hop_index: usize,
    step: &KagemushaVerifiedFoldStep,
) -> Result<crate::zk::OpenVerifyEnvelope, KagemushaFoldError> {
    let invalid = || KagemushaFoldError::InvalidRecursiveSpendProof {
        field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.bytes",
    };
    let envelope: crate::zk::OpenVerifyEnvelope =
        norito::decode_from_bytes(&step.attachment.proof.bytes).map_err(|_| invalid())?;
    let Some(expected_backend) = kagemusha_backend_tag(step.attachment.backend.as_str()) else {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: step.attachment.backend.clone(),
        });
    };
    if envelope.backend != expected_backend {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.backend",
        });
    }
    if envelope.circuit_id.trim().is_empty() {
        return Err(invalid());
    }
    if envelope.vk_hash
        != step
            .attachment
            .vk_commitment
            .expect("validated vk commitment")
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.vk_hash",
        });
    }
    if envelope.public_inputs.is_empty()
        || envelope.proof_bytes.is_empty()
        || !envelope.aux.is_empty()
    {
        return Err(invalid());
    }
    Ok(envelope)
}

fn validate_kagemusha_recursive_lineage_open_envelope_metadata(
    record_bundle: &KagemushaVerifiedFoldRecordBundle,
    envelopes: &[iroha_zkp_halo2::OpenVerifyEnvelope],
) -> Result<(), KagemushaFoldError> {
    for (hop_index, (step, envelope)) in record_bundle
        .bundle
        .steps
        .iter()
        .zip(envelopes.iter())
        .enumerate()
    {
        if step.attachment.backend.as_str() != "halo2/ipa" {
            continue;
        }
        let Some(expected_vk_commitment) = step.attachment.vk_commitment else {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.bundle.steps.attachment.vk_commitment",
            });
        };
        if envelope.vk_commitment != Some(expected_vk_commitment) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive.vk_commitment",
            });
        }
        let proof_envelope =
            kagemusha_verified_fold_lineage_step_open_verify_envelope(hop_index, step)?;
        let expected_schema_hash: [u8; Hash::LENGTH] =
            Hash::new(proof_envelope.public_inputs.as_slice()).into();
        if envelope.public_inputs_schema_hash != Some(expected_schema_hash) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive.public_inputs_schema_hash",
            });
        }
    }
    Ok(())
}

fn decode_kagemusha_recursive_lineage_open_envelopes(
    pallas_open_envelopes_archive: &[u8],
    expected_hops: usize,
) -> Result<Vec<iroha_zkp_halo2::OpenVerifyEnvelope>, KagemushaFoldError> {
    if pallas_open_envelopes_archive.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "lineage_witness.pallas_open_envelopes_archive",
        });
    }
    let envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
        norito::decode_from_bytes(pallas_open_envelopes_archive).map_err(|_| {
            KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive",
            }
        })?;
    if envelopes.len() != expected_hops {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: expected_hops,
            actual: u32::try_from(envelopes.len()).unwrap_or(u32::MAX),
        });
    }
    for (envelope_index, envelope) in envelopes.iter().enumerate() {
        validate_kagemusha_recursive_pallas_open_envelope_shape(
            "lineage_witness.pallas_open_envelopes_archive",
            envelope_index,
            envelope,
        )?;
    }
    Ok(envelopes)
}

fn validate_kagemusha_recursive_pallas_open_envelope_shape(
    field: &'static str,
    _envelope_index: usize,
    envelope: &iroha_zkp_halo2::OpenVerifyEnvelope,
) -> Result<(), KagemushaFoldError> {
    let invalid = || KagemushaFoldError::InvalidRecursiveSpendProof { field };
    if envelope.transcript_label.is_empty()
        || envelope.transcript_label.len()
            > KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES
    {
        return Err(invalid());
    }
    validate_kagemusha_recursive_pallas_open_envelope_metadata_field(
        field,
        envelope.vk_commitment,
    )?;
    validate_kagemusha_recursive_pallas_open_envelope_metadata_field(
        field,
        envelope.public_inputs_schema_hash,
    )?;
    validate_kagemusha_recursive_pallas_open_envelope_metadata_field(field, envelope.domain_tag)?;
    if envelope.params.version != 1 || envelope.public.version != 1 || envelope.proof.version != 1 {
        return Err(invalid());
    }
    let pallas_curve_id = iroha_zkp_halo2::ZkCurveId::Pallas.as_u16();
    if envelope.params.curve_id != pallas_curve_id || envelope.public.curve_id != pallas_curve_id {
        return Err(invalid());
    }
    if envelope.params.n != envelope.public.n {
        return Err(invalid());
    }
    validate_kagemusha_recursive_verifier_opening_len(envelope.params.n).map_err(|_| invalid())?;

    let opening_len = usize::try_from(envelope.params.n).map_err(|_| invalid())?;
    if envelope.params.g.len() != opening_len || envelope.params.h.len() != opening_len {
        return Err(invalid());
    }
    let expected_rounds = opening_len.trailing_zeros() as usize;
    if envelope.proof.l.len() != envelope.proof.r.len() || envelope.proof.l.len() != expected_rounds
    {
        return Err(invalid());
    }
    Ok(())
}

fn validate_kagemusha_recursive_pallas_open_envelope_metadata_field(
    field: &'static str,
    value: Option<[u8; 32]>,
) -> Result<[u8; 32], KagemushaFoldError> {
    match value {
        Some(value) if value != [0u8; Hash::LENGTH] => Ok(value),
        Some(_) | None => Err(KagemushaFoldError::InvalidRecursiveSpendProof { field }),
    }
}

fn kagemusha_recursive_poseidon_update_u64(
    hasher: &mut iroha_zkp_halo2::poseidon::PoseidonByteHasher,
    value: u64,
) {
    hasher.update(&value.to_le_bytes());
}

fn kagemusha_recursive_poseidon_update_len(
    hasher: &mut iroha_zkp_halo2::poseidon::PoseidonByteHasher,
    label: &'static str,
    value: usize,
) -> Result<(), KagemushaFoldError> {
    if label.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "poseidon_length_label",
        });
    }
    let label_len =
        u64::try_from(label.len()).map_err(|_| KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "poseidon_length_label",
        })?;
    let value =
        u64::try_from(value).map_err(|_| KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "poseidon_length_value",
        })?;
    kagemusha_recursive_poseidon_update_u64(hasher, label_len);
    hasher.update(label.as_bytes());
    kagemusha_recursive_poseidon_update_u64(hasher, value);
    Ok(())
}

fn kagemusha_recursive_poseidon_update_tagged_bytes(
    hasher: &mut iroha_zkp_halo2::poseidon::PoseidonByteHasher,
    tag: &[u8],
    value: &[u8],
) -> Result<(), KagemushaFoldError> {
    kagemusha_recursive_poseidon_update_len(hasher, "tag", tag.len())?;
    hasher.update(tag);
    kagemusha_recursive_poseidon_update_len(hasher, "value", value.len())?;
    hasher.update(value);
    Ok(())
}

/// Return the canonical domain tag for a previous recursive proof opening envelope.
///
/// Reserved-lineage append proving uses this tag to make the Pallas IPA opening
/// transcript specific to the exact previous recursive proof artifact and
/// accumulator state.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous recursive proof or decoded
/// proof envelope is malformed.
pub fn kagemusha_recursive_previous_proof_open_envelope_domain_tag(
    previous_bundle: &KagemushaRecursiveSpendBundleV1,
    proof_envelope: &crate::zk::OpenVerifyEnvelope,
) -> Result<[u8; Hash::LENGTH], KagemushaFoldError> {
    previous_bundle.validate_public_input_binding()?;
    let proof = &previous_bundle.recursive_proof;
    validate_kagemusha_recursive_spend_proof_public_input_binding(proof)?;
    let mut hasher = iroha_zkp_halo2::poseidon::PoseidonByteHasher::new();
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"domain",
        KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPE_DOMAIN_TAG_V1.as_bytes(),
    )?;
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"chain-id",
        previous_bundle.accumulator.chain_id.as_str().as_bytes(),
    )?;
    let asset = previous_bundle.accumulator.asset.to_string();
    kagemusha_recursive_poseidon_update_tagged_bytes(&mut hasher, b"asset", asset.as_bytes())?;
    kagemusha_recursive_poseidon_update_len(
        &mut hasher,
        "previous hop count",
        usize::try_from(previous_bundle.accumulator.hop_count).unwrap_or(usize::MAX),
    )?;
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"verifier-key-backend",
        proof.verifier_key_id.backend.as_bytes(),
    )?;
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"verifier-key-name",
        proof.verifier_key_id.name.as_bytes(),
    )?;
    let proof_artifact_digest = kagemusha_recursive_spend_proof_artifact_digest(proof)?;
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"recursive-proof-artifact-digest",
        &proof_artifact_digest,
    )?;
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"proof-envelope-vk-hash",
        &proof_envelope.vk_hash,
    )?;
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"public-inputs-hash",
        proof.public_inputs_hash.as_ref(),
    )?;
    let proof_payload_hash = Hash::new(&proof.proof.bytes);
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"proof-payload-hash",
        proof_payload_hash.as_ref(),
    )?;
    kagemusha_recursive_poseidon_update_tagged_bytes(
        &mut hasher,
        b"recursive-proof-chain-digest",
        &previous_bundle.accumulator.recursive_proof_chain_digest,
    )?;
    Ok(hasher.finalize())
}

/// Return the expected metadata for a previous recursive proof opening envelope.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the previous bundle proof is not a
/// canonical recursive proof envelope.
pub fn kagemusha_recursive_previous_proof_open_envelope_metadata(
    previous_bundle: &KagemushaRecursiveSpendBundleV1,
) -> Result<iroha_zkp_halo2::PolyOpenTranscriptMetadata, KagemushaFoldError> {
    let proof = &previous_bundle.recursive_proof;
    let envelope: crate::zk::OpenVerifyEnvelope = norito::decode_from_bytes(&proof.proof.bytes)
        .map_err(|_| KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_bundle.recursive_proof.proof.bytes",
        })?;
    let Some(expected_backend) = kagemusha_backend_tag(proof.proof.backend.as_str()) else {
        return Err(KagemushaFoldError::UnsupportedProofBackend {
            backend: proof.proof.backend.clone(),
        });
    };
    if envelope.backend != expected_backend {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_bundle.recursive_proof.proof.backend",
        });
    }
    if envelope.circuit_id != proof.verifier_key_id.name {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_bundle.recursive_proof.proof.circuit_id",
        });
    }
    if envelope.vk_hash == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_bundle.recursive_proof.proof.vk_hash",
        });
    }
    if envelope.public_inputs.as_slice()
        != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_bundle.recursive_proof.proof.public_inputs",
        });
    }
    Ok(iroha_zkp_halo2::PolyOpenTranscriptMetadata {
        vk_commitment: Some(envelope.vk_hash),
        public_inputs_schema_hash: Some(
            kagemusha_recursive_aggregation_proof_public_inputs_schema_hash(),
        ),
        domain_tag: Some(kagemusha_recursive_previous_proof_open_envelope_domain_tag(
            previous_bundle,
            &envelope,
        )?),
    })
}

fn validate_kagemusha_recursive_previous_proof_open_envelope_metadata(
    envelope: &iroha_zkp_halo2::OpenVerifyEnvelope,
    expected: &iroha_zkp_halo2::PolyOpenTranscriptMetadata,
) -> Result<(), KagemushaFoldError> {
    if envelope.vk_commitment != expected.vk_commitment {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_recursive_proof_open_envelopes_archive.vk_commitment",
        });
    }
    if envelope.public_inputs_schema_hash != expected.public_inputs_schema_hash {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_recursive_proof_open_envelopes_archive.public_inputs_schema_hash",
        });
    }
    if envelope.domain_tag != expected.domain_tag {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_recursive_proof_open_envelopes_archive.domain_tag",
        });
    }
    Ok(())
}

fn validate_kagemusha_recursive_previous_proof_open_envelopes_archive(
    previous_bundle: &KagemushaRecursiveSpendBundleV1,
    previous_recursive_proof_open_envelopes_archive: &[u8],
    required: bool,
) -> Result<(), KagemushaFoldError> {
    if previous_recursive_proof_open_envelopes_archive.is_empty() {
        return if required {
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive",
            })
        } else {
            Ok(())
        };
    }
    if previous_recursive_proof_open_envelopes_archive.len()
        > KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_recursive_proof_open_envelopes_archive",
        });
    }
    let envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> = norito::decode_from_bytes(
        previous_recursive_proof_open_envelopes_archive,
    )
    .map_err(|_| KagemushaFoldError::InvalidRecursiveSpendProof {
        field: "previous_recursive_proof_open_envelopes_archive",
    })?;
    if envelopes.len() != KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1 {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "previous_recursive_proof_open_envelopes_archive",
        });
    }
    let expected_metadata =
        kagemusha_recursive_previous_proof_open_envelope_metadata(previous_bundle)?;
    for (envelope_index, envelope) in envelopes.iter().enumerate() {
        validate_kagemusha_recursive_pallas_open_envelope_shape(
            "previous_recursive_proof_open_envelopes_archive",
            envelope_index,
            envelope,
        )?;
        validate_kagemusha_recursive_previous_proof_open_envelope_metadata(
            envelope,
            &expected_metadata,
        )?;
    }
    Ok(())
}

const KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND: &str = KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND;
const KAGEMUSHA_RECURSIVE_SPEND_REDEEM_PROOF_BACKEND: &str =
    KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND;

fn validate_kagemusha_recursive_spend_bundle_production_proof_attachment(
    bundle: &KagemushaRecursiveSpendBundleV1,
) -> Result<(), KagemushaFoldError> {
    validate_kagemusha_recursive_spend_proof_attachment(&bundle.recursive_proof)
}

fn validate_kagemusha_recursive_spend_proof_attachment(
    recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<(), KagemushaFoldError> {
    if recursive_proof.proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "proof.backend",
        });
    }
    if recursive_proof.verifier_key_id.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "verifier_key_id.backend",
        });
    }
    kagemusha_recursive_spend_proof_circuit(&recursive_proof.verifier_key_id)?;
    if recursive_proof.proof.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "proof.bytes",
        });
    }
    Ok(())
}

fn validate_kagemusha_recursive_spend_request_note_for_step(
    step: &KagemushaVerifiedFoldStep,
    current_note: &KagemushaSpendableNoteDescriptorV1,
) -> Result<(), KagemushaFoldError> {
    validate_kagemusha_recursive_spend_note(current_note)?;
    if !step
        .output_commitments
        .iter()
        .any(|commitment| commitment == &current_note.note_commitment)
    {
        return Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment);
    }
    if step
        .input_nullifiers
        .iter()
        .any(|nullifier| nullifier == &current_note.spend_nullifier)
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "current_note.spend_nullifier",
        });
    }
    if step
        .output_commitments
        .iter()
        .any(|commitment| commitment == &current_note.spend_nullifier)
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "current_note.spend_nullifier",
        });
    }
    Ok(())
}

fn validate_kagemusha_recursive_spend_previous_lineage_record_selection(
    bundle: &KagemushaRecursiveSpendBundleV1,
    previous_lineage_verifier_record: Option<&VerifyingKeyRecord>,
    fields: KagemushaRecursiveSpendLineageRecordFieldNames,
) -> Result<(), KagemushaFoldError> {
    match kagemusha_recursive_spend_proof_circuit(&bundle.recursive_proof.verifier_key_id)? {
        KagemushaRecursiveSpendProofCircuit::SemanticAggregation => {
            if previous_lineage_verifier_record.is_some() {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof { field: fields.root });
            }
            Ok(())
        }
        KagemushaRecursiveSpendProofCircuit::Lineage => {
            let record = previous_lineage_verifier_record
                .ok_or(KagemushaFoldError::InvalidRecursiveSpendProof { field: fields.root })?;
            validate_kagemusha_recursive_spend_lineage_verifier_record_metadata(record, fields)?;
            if record.circuit_id != bundle.recursive_proof.verifier_key_id.name {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: fields.circuit_id,
                });
            }
            Ok(())
        }
    }
}

fn validate_kagemusha_recursive_spend_append_output_selection(
    previous_bundle: &KagemushaRecursiveSpendBundleV1,
    output_proof_circuit_id: &str,
) -> Result<(), KagemushaFoldError> {
    kagemusha_recursive_spend_proof_circuit(&previous_bundle.recursive_proof.verifier_key_id)?;
    if !is_supported_kagemusha_recursive_spend_append_proof_transition(
        previous_bundle
            .recursive_proof
            .verifier_key_id
            .name
            .as_str(),
        output_proof_circuit_id,
    ) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "output_proof_circuit_id",
        });
    }
    if !can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
        output_proof_circuit_id,
        previous_bundle.accumulator.hop_count,
    ) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: "output_proof_circuit_id",
        });
    }
    Ok(())
}

fn kagemusha_recursive_spend_lineage_witness_has_reserved_previous_proof(
    witness: &KagemushaRecursiveSpendLineageWitnessV1,
) -> Result<bool, KagemushaFoldError> {
    witness
        .previous_recursive_proofs
        .iter()
        .try_fold(false, |found, proof| {
            let circuit = kagemusha_recursive_spend_proof_circuit(&proof.verifier_key_id)?;
            Ok(found || matches!(circuit, KagemushaRecursiveSpendProofCircuit::Lineage))
        })
}

fn validate_kagemusha_recursive_spend_redeem_lineage_record_selection(
    bundle: &KagemushaRecursiveSpendBundleV1,
    lineage_witness: Option<&KagemushaRecursiveSpendLineageWitnessV1>,
    lineage_verifier_record: Option<&VerifyingKeyRecord>,
) -> Result<(), KagemushaFoldError> {
    let fields = KagemushaRecursiveSpendLineageRecordFieldNames::LINEAGE_VERIFIER_RECORD;
    let final_circuit =
        kagemusha_recursive_spend_proof_circuit(&bundle.recursive_proof.verifier_key_id)?;
    let witness_has_reserved_previous = lineage_witness
        .map(kagemusha_recursive_spend_lineage_witness_has_reserved_previous_proof)
        .transpose()?
        .unwrap_or(false);
    match final_circuit {
        KagemushaRecursiveSpendProofCircuit::Lineage => {
            if lineage_witness.is_none()
                && !can_redeem_kagemusha_recursive_spend_witnessless(
                    &bundle.recursive_proof.verifier_key_id.name,
                    bundle.accumulator.hop_count,
                )
            {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "lineage_witness",
                });
            }
            let record = lineage_verifier_record
                .ok_or(KagemushaFoldError::InvalidRecursiveSpendProof { field: fields.root })?;
            validate_kagemusha_recursive_spend_lineage_verifier_record_metadata(record, fields)?;
            if record.circuit_id != bundle.recursive_proof.verifier_key_id.name {
                return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: fields.circuit_id,
                });
            }
            Ok(())
        }
        KagemushaRecursiveSpendProofCircuit::SemanticAggregation => {
            if witness_has_reserved_previous {
                let record = lineage_verifier_record
                    .ok_or(KagemushaFoldError::InvalidRecursiveSpendProof { field: fields.root })?;
                validate_kagemusha_recursive_spend_lineage_verifier_record_metadata(
                    record, fields,
                )?;
                if let Some(witness) = lineage_witness {
                    for previous_proof in &witness.previous_recursive_proofs {
                        if is_kagemusha_recursive_spend_lineage_proof_circuit_id(
                            &previous_proof.verifier_key_id.name,
                        ) && record.circuit_id != previous_proof.verifier_key_id.name
                        {
                            return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                                field: fields.circuit_id,
                            });
                        }
                    }
                }
                Ok(())
            } else if lineage_verifier_record.is_some() {
                Err(KagemushaFoldError::InvalidRecursiveSpendProof { field: fields.root })
            } else {
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct KagemushaRecursiveSpendLineageRecordFieldNames {
    root: &'static str,
    status: &'static str,
    namespace: &'static str,
    backend: &'static str,
    curve: &'static str,
    circuit_id: &'static str,
    public_inputs_schema_hash: &'static str,
    commitment: &'static str,
    max_proof_bytes: &'static str,
    key: &'static str,
    key_backend: &'static str,
    key_bytes: &'static str,
    vk_len: &'static str,
}

impl KagemushaRecursiveSpendLineageRecordFieldNames {
    const PREVIOUS_LINEAGE_VERIFIER_RECORD: Self = Self {
        root: "previous_lineage_verifier_record",
        status: "previous_lineage_verifier_record.status",
        namespace: "previous_lineage_verifier_record.namespace",
        backend: "previous_lineage_verifier_record.backend",
        curve: "previous_lineage_verifier_record.curve",
        circuit_id: "previous_lineage_verifier_record.circuit_id",
        public_inputs_schema_hash: "previous_lineage_verifier_record.public_inputs_schema_hash",
        commitment: "previous_lineage_verifier_record.commitment",
        max_proof_bytes: "previous_lineage_verifier_record.max_proof_bytes",
        key: "previous_lineage_verifier_record.key",
        key_backend: "previous_lineage_verifier_record.key.backend",
        key_bytes: "previous_lineage_verifier_record.key.bytes",
        vk_len: "previous_lineage_verifier_record.vk_len",
    };

    const LINEAGE_VERIFIER_RECORD: Self = Self {
        root: "lineage_verifier_record",
        status: "lineage_verifier_record.status",
        namespace: "lineage_verifier_record.namespace",
        backend: "lineage_verifier_record.backend",
        curve: "lineage_verifier_record.curve",
        circuit_id: "lineage_verifier_record.circuit_id",
        public_inputs_schema_hash: "lineage_verifier_record.public_inputs_schema_hash",
        commitment: "lineage_verifier_record.commitment",
        max_proof_bytes: "lineage_verifier_record.max_proof_bytes",
        key: "lineage_verifier_record.key",
        key_backend: "lineage_verifier_record.key.backend",
        key_bytes: "lineage_verifier_record.key.bytes",
        vk_len: "lineage_verifier_record.vk_len",
    };
}

fn validate_kagemusha_recursive_spend_lineage_verifier_record_metadata(
    record: &VerifyingKeyRecord,
    fields: KagemushaRecursiveSpendLineageRecordFieldNames,
) -> Result<(), KagemushaFoldError> {
    if !record.is_active() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.status,
        });
    }
    if record.namespace != KAGEMUSHA_VERIFIER_NAMESPACE {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.namespace,
        });
    }
    if record.backend != BackendTag::Halo2IpaPasta {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.backend,
        });
    }
    if kagemusha_record_curve_for_backend(record.backend) != Some(record.curve.as_str()) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.curve,
        });
    }
    if !is_kagemusha_recursive_spend_lineage_proof_circuit_id(&record.circuit_id) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.circuit_id,
        });
    }
    if record.public_inputs_schema_hash
        != kagemusha_recursive_aggregation_proof_public_inputs_schema_hash()
    {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.public_inputs_schema_hash,
        });
    }
    if record.commitment == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.commitment,
        });
    }
    if record.max_proof_bytes == 0 {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.max_proof_bytes,
        });
    }
    let Some(inline_key) = record.key.as_ref() else {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof { field: fields.key });
    };
    if inline_key.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.key_backend,
        });
    }
    if inline_key.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.key_bytes,
        });
    }
    if u32::try_from(inline_key.bytes.len()).ok() != Some(record.vk_len) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.vk_len,
        });
    }
    if kagemusha_verifying_key_commitment(inline_key) != record.commitment {
        return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
            field: fields.commitment,
        });
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn validate_kagemusha_recursive_spend_lineage_witness(
    bundle: &KagemushaRecursiveSpendBundleV1,
    witness: &KagemushaRecursiveSpendLineageWitnessV1,
) -> Result<(), KagemushaFoldError> {
    kagemusha_recursive_spend_proof_circuit(&bundle.recursive_proof.verifier_key_id)?;
    bundle.validate_public_input_binding()?;
    validate_kagemusha_verified_fold_lineage_steps(&witness.record_bundle.bundle)?;
    let hop_count = witness.record_bundle.bundle.steps.len();
    if hop_count == 0 {
        return Err(KagemushaFoldError::Empty);
    }
    if hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: hop_count,
        });
    }
    if witness.current_notes.len() != hop_count {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: hop_count,
            actual: u32::try_from(witness.current_notes.len()).unwrap_or(u32::MAX),
        });
    }
    if witness.previous_recursive_proofs.len().saturating_add(1) != hop_count {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: hop_count.saturating_sub(1),
            actual: u32::try_from(witness.previous_recursive_proofs.len()).unwrap_or(u32::MAX),
        });
    }
    let pallas_open_envelopes = decode_kagemusha_recursive_lineage_open_envelopes(
        &witness.pallas_open_envelopes_archive,
        hop_count,
    )?;
    validate_kagemusha_recursive_lineage_open_envelope_metadata(
        &witness.record_bundle,
        &pallas_open_envelopes,
    )?;
    if witness.record_bundle.bundle.chain_id != bundle.accumulator.chain_id {
        return Err(KagemushaFoldError::RecursiveSpendChainMismatch);
    }
    if witness.record_bundle.bundle.asset != bundle.accumulator.asset {
        return Err(KagemushaFoldError::RecursiveSpendAssetMismatch);
    }
    let first_step = witness
        .record_bundle
        .bundle
        .steps
        .first()
        .expect("validated non-empty lineage steps");
    if first_step.root_before != bundle.accumulator.initial_root {
        return Err(KagemushaFoldError::InitialRootMismatch {
            expected: bundle.accumulator.initial_root,
            actual: first_step.root_before,
        });
    }
    let final_step = witness
        .record_bundle
        .bundle
        .steps
        .last()
        .expect("validated non-empty lineage steps");
    if final_step.root_after != bundle.accumulator.final_root {
        return Err(KagemushaFoldError::FinalRootMismatch {
            expected: bundle.accumulator.final_root,
            actual: final_step.root_after,
        });
    }
    if usize::try_from(bundle.accumulator.hop_count).ok() != Some(hop_count) {
        return Err(KagemushaFoldError::HopCountMismatch {
            expected: hop_count,
            actual: bundle.accumulator.hop_count,
        });
    }
    validate_kagemusha_verified_fold_record_bundle_exact_records(&witness.record_bundle)?;

    for (proof_index, previous_proof) in witness.previous_recursive_proofs.iter().enumerate() {
        let circuit =
            validate_kagemusha_recursive_spend_proof_public_input_binding(previous_proof)?;
        macro_rules! ensure_previous_context {
            ($field:ident, $name:literal) => {
                if previous_proof.public_inputs.$field
                    != bundle.recursive_proof.public_inputs.$field
                {
                    return Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                        field: concat!("lineage_witness.previous_recursive_proofs.", $name),
                    });
                }
            };
        }
        ensure_previous_context!(verifier_opening_len, "verifier_opening_len");
        ensure_previous_context!(verifier_params_fingerprint, "verifier_params_fingerprint");
        ensure_previous_context!(
            fixed_window_table_schedule_digest,
            "fixed_window_table_schedule_digest"
        );
        ensure_previous_context!(
            fixed_window_shared_table_manifest_digest,
            "fixed_window_shared_table_manifest_digest"
        );
        match circuit {
            KagemushaRecursiveSpendProofCircuit::SemanticAggregation => {
                if previous_proof
                    .public_inputs
                    .recursive_verifier_scalar_projection_digest
                    != [0u8; Hash::LENGTH]
                {
                    return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "lineage_witness.previous_recursive_proofs.recursive_verifier_scalar_projection_digest",
                    });
                }
            }
            KagemushaRecursiveSpendProofCircuit::Lineage => {
                if previous_proof
                    .public_inputs
                    .recursive_verifier_scalar_projection_digest
                    == [0u8; Hash::LENGTH]
                {
                    return Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                        field: "lineage_witness.previous_recursive_proofs.recursive_verifier_scalar_projection_digest",
                    });
                }
            }
        }
        let expected_hop_count = proof_index.saturating_add(1);
        let expected_hop_count_u32 =
            u32::try_from(expected_hop_count).map_err(|_| KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: expected_hop_count,
            })?;
        if previous_proof.public_inputs.hop_count != expected_hop_count_u32 {
            return Err(KagemushaFoldError::HopCountMismatch {
                expected: expected_hop_count,
                actual: previous_proof.public_inputs.hop_count,
            });
        }
    }

    let mut topup_anchor_nullifiers = first_step.input_nullifiers.clone();
    topup_anchor_nullifiers.sort_unstable();
    validate_kagemusha_recursive_spend_topup_anchor_nullifiers(&topup_anchor_nullifiers)?;
    let lineage_input_nullifiers = witness
        .record_bundle
        .bundle
        .steps
        .iter()
        .flat_map(|step| step.input_nullifiers.iter().copied())
        .collect::<std::collections::BTreeSet<_>>();
    let lineage_output_commitments = witness
        .record_bundle
        .bundle
        .steps
        .iter()
        .flat_map(|step| step.output_commitments.iter().copied())
        .collect::<std::collections::BTreeSet<_>>();
    let mut seen_note_spend_nullifiers = std::collections::BTreeSet::new();
    for (hop_index, (step, note)) in witness
        .record_bundle
        .bundle
        .steps
        .iter()
        .zip(witness.current_notes.iter())
        .enumerate()
    {
        validate_kagemusha_recursive_spend_note(note)?;
        if !seen_note_spend_nullifiers.insert(note.spend_nullifier) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier",
            });
        }
        if lineage_output_commitments.contains(&note.spend_nullifier) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier",
            });
        }
        if hop_index + 1 == hop_count && lineage_input_nullifiers.contains(&note.spend_nullifier) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier",
            });
        }
        if !step
            .output_commitments
            .iter()
            .any(|commitment| commitment == &note.note_commitment)
        {
            return Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment);
        }
        if step
            .input_nullifiers
            .iter()
            .any(|nullifier| nullifier == &note.spend_nullifier)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier",
            });
        }
        if step
            .output_commitments
            .iter()
            .any(|commitment| commitment == &note.spend_nullifier)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier",
            });
        }
        if hop_index == 0 {
            continue;
        }
        let previous_note = &witness.current_notes[hop_index - 1];
        if note.amount != previous_note.amount {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" });
        }
        if step.input_nullifiers.len() != 1 {
            return Err(KagemushaFoldError::RecursiveSpendUnexpectedAppendInput);
        }
        if !step
            .input_nullifiers
            .iter()
            .any(|nullifier| nullifier == &previous_note.spend_nullifier)
        {
            return Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier);
        }
        if note.spend_nullifier == previous_note.note_commitment {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier",
            });
        }
        if step
            .output_commitments
            .iter()
            .any(|commitment| commitment == &previous_note.note_commitment)
        {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.record_bundle.bundle.steps.output_commitments",
            });
        }
        if step.output_commitments.iter().any(|commitment| {
            topup_anchor_nullifiers
                .iter()
                .any(|anchor| anchor == commitment)
        }) {
            return Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "lineage_witness.record_bundle.bundle.steps.output_commitments",
            });
        }
    }

    if witness.current_notes.last() != Some(&bundle.accumulator.current_note) {
        return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
            field: "lineage_witness.current_notes.final",
        });
    }
    Ok(())
}

fn validate_kagemusha_recursive_spend_redeem_proof_attachment(
    redeem_proof: &ProofAttachment,
) -> Result<(), KagemushaFoldError> {
    if redeem_proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_REDEEM_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof { field: "backend" });
    }
    if redeem_proof.proof.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_REDEEM_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "proof.backend",
        });
    }
    if redeem_proof.vk_ref.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_REDEEM_PROOF_BACKEND {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "vk_ref.backend",
        });
    }
    if redeem_proof.vk_ref.name.trim().is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "vk_ref.name",
        });
    }
    if redeem_proof.proof.bytes.is_empty() {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "proof.bytes",
        });
    }
    let Some(vk_commitment) = redeem_proof.vk_commitment else {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "vk_commitment",
        });
    };
    if vk_commitment == [0u8; Hash::LENGTH] {
        return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
            field: "vk_commitment",
        });
    }
    if let Some(envelope_hash) = redeem_proof.envelope_hash {
        let expected_hash: [u8; Hash::LENGTH] = Hash::new(&redeem_proof.proof.bytes).into();
        if envelope_hash != expected_hash {
            return Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "envelope_hash",
            });
        }
    }
    Ok(())
}

impl KagemushaRecursiveSpendRedeemRequestV1 {
    /// Build and validate a witnessless recursive spend redeem request.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the recursive bundle, final redeem
    /// proof, public amount, or lineage verifier selection is malformed.
    pub fn new(
        bundle: KagemushaRecursiveSpendBundleV1,
        recipient: AccountId,
        public_amount: u128,
        redeem_proof: ProofAttachment,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new_with_lineage_witness_and_change(
            bundle,
            recipient,
            public_amount,
            redeem_proof,
            None,
            None,
            None,
        )
    }

    /// Build and validate a recursive spend redeem request with lineage material.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the recursive bundle, final redeem
    /// proof, public amount, lineage witness, or lineage verifier record selection
    /// is malformed.
    pub fn new_with_lineage_witness(
        bundle: KagemushaRecursiveSpendBundleV1,
        recipient: AccountId,
        public_amount: u128,
        redeem_proof: ProofAttachment,
        lineage_witness: Option<KagemushaRecursiveSpendLineageWitnessV1>,
        lineage_verifier_record: Option<VerifyingKeyRecord>,
    ) -> Result<Self, KagemushaFoldError> {
        Self::new_with_lineage_witness_and_change(
            bundle,
            recipient,
            public_amount,
            redeem_proof,
            lineage_witness,
            lineage_verifier_record,
            None,
        )
    }

    /// Build and validate a recursive spend redeem request with optional private change.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the recursive bundle, final redeem
    /// proof, public amount, lineage witness, lineage verifier selection, or
    /// change commitment is malformed.
    pub fn new_with_lineage_witness_and_change(
        bundle: KagemushaRecursiveSpendBundleV1,
        recipient: AccountId,
        public_amount: u128,
        redeem_proof: ProofAttachment,
        lineage_witness: Option<KagemushaRecursiveSpendLineageWitnessV1>,
        lineage_verifier_record: Option<VerifyingKeyRecord>,
        change_output: Option<[u8; 32]>,
    ) -> Result<Self, KagemushaFoldError> {
        let request = Self {
            bundle,
            recipient,
            public_amount,
            redeem_proof,
            lineage_witness,
            change_output,
            lineage_verifier_record,
            block_height: None,
        };
        request.validate_public_binding()?;
        Ok(request)
    }

    /// Validate wallet-side public bindings before producing a redeem instruction.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the recursive bundle is malformed,
    /// the requested public amount/change pair does not match the current
    /// spendable note, or the final redeem proof attachment is not in the
    /// transparent production corridor.
    pub fn validate_public_binding(&self) -> Result<(), KagemushaFoldError> {
        validate_kagemusha_recursive_spend_bundle_production_proof_attachment(&self.bundle)?;
        self.bundle.validate_public_input_binding()?;
        validate_kagemusha_recursive_spend_redeem_proof_attachment(&self.redeem_proof)?;
        let current_note = &self.bundle.accumulator.current_note;
        if self.public_amount == 0 || current_note.amount.scale() != 0 {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount",
            });
        }
        let Some(current_amount) = current_note.amount.try_mantissa_u128() else {
            return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount",
            });
        };
        let redeem_nullifiers = self.bundle.accumulator.redeem_nullifiers()?;
        match self.change_output {
            None if self.public_amount != current_amount => {
                return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                    field: "public_amount",
                });
            }
            Some(change_output) if self.public_amount >= current_amount => {
                return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                    field: "public_amount",
                });
            }
            Some(change_output) if change_output == [0u8; Hash::LENGTH] => {
                return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                    field: "change_output",
                });
            }
            Some(change_output)
                if change_output == current_note.note_commitment
                    || redeem_nullifiers.contains(&change_output) =>
            {
                return Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                    field: "change_output",
                });
            }
            Some(_) | None => {}
        }
        if let Some(witness) = &self.lineage_witness {
            validate_kagemusha_recursive_spend_lineage_witness(&self.bundle, witness)?;
        }
        validate_kagemusha_recursive_spend_redeem_lineage_record_selection(
            &self.bundle,
            self.lineage_witness.as_ref(),
            self.lineage_verifier_record.as_ref(),
        )?;
        Ok(())
    }
}

struct KagemushaCanonicalFoldParts {
    nullifier_digest: Hash,
    output_commitment_digest: Hash,
    fold_digest: Hash,
    aggregation_statement: KagemushaPoseidonAggregationTranscriptStatement,
}

#[allow(clippy::too_many_lines)]
fn kagemusha_canonical_fold_parts(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    steps: &[KagemushaFoldStep],
) -> Result<KagemushaCanonicalFoldParts, KagemushaFoldError> {
    if steps.is_empty() {
        return Err(KagemushaFoldError::Empty);
    }
    if steps.len() > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
        return Err(KagemushaFoldError::TooManyHops {
            max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
            actual: steps.len(),
        });
    }

    let initial_root = steps[0].root_before;
    validate_kagemusha_fold_root("initial_root", initial_root)?;
    let mut expected_root = initial_root;
    let mut all_inputs = Vec::new();
    let mut all_outputs = Vec::new();
    let mut step_digests = Vec::with_capacity(steps.len());
    let mut aggregation_steps = Vec::with_capacity(steps.len());
    let mut seen_inputs = std::collections::BTreeSet::new();
    let mut seen_outputs = std::collections::BTreeSet::new();

    for (hop_index, step) in steps.iter().enumerate() {
        validate_kagemusha_verifier_key_id(hop_index, &step.verifier_key_id)?;
        validate_kagemusha_step_shape_and_sets(
            hop_index,
            &step.input_nullifiers,
            &step.output_commitments,
        )?;
        validate_kagemusha_step_digest_bindings(
            hop_index,
            step.proof_public_inputs_digest,
            step.verifier_key_commitment,
            step.verifier_key_poseidon_digest,
        )?;
        validate_kagemusha_fold_root("root_before", step.root_before)?;
        validate_kagemusha_fold_root("root_after", step.root_after)?;
        validate_kagemusha_root_transition(hop_index, step.root_before, step.root_after)?;
        if step.root_before != expected_root {
            return Err(KagemushaFoldError::RootDiscontinuity {
                hop_index,
                expected: expected_root,
                actual: step.root_before,
            });
        }

        let mut input_nullifiers = step.input_nullifiers.clone();
        input_nullifiers.sort_unstable();
        for input in &input_nullifiers {
            if seen_outputs.contains(input) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
            if !seen_inputs.insert(*input) {
                return Err(KagemushaFoldError::DuplicateInputNullifier { hop_index });
            }
        }

        let mut output_commitments = step.output_commitments.clone();
        output_commitments.sort_unstable();
        for output in &output_commitments {
            if seen_inputs.contains(output) {
                return Err(KagemushaFoldError::InputOutputOverlap { hop_index });
            }
            if !seen_outputs.insert(*output) {
                return Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index });
            }
        }

        let step_digest = kagemusha_hash_preimage(&KagemushaFoldStepDigestPreimage {
            domain: KAGEMUSHA_FOLD_STEP_DIGEST_DOMAIN.to_owned(),
            hop_index: u32::try_from(hop_index).expect("hop count is bounded to u32"),
            root_before: step.root_before,
            input_nullifiers: input_nullifiers.clone(),
            output_commitments: output_commitments.clone(),
            root_after: step.root_after,
            proof_hash: step.proof_hash,
            proof_public_inputs_digest: step.proof_public_inputs_digest,
            verifier_key_id: step.verifier_key_id.clone(),
            verifier_key_commitment: step.verifier_key_commitment,
            verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
        })?;
        aggregation_steps.push(KagemushaPoseidonAggregationStepStatement {
            hop_index: u32::try_from(hop_index).expect("hop count is bounded to u32"),
            root_before: step.root_before,
            input_nullifiers: input_nullifiers.clone(),
            output_commitments: output_commitments.clone(),
            root_after: step.root_after,
            proof_hash: step.proof_hash,
            proof_public_inputs_digest: step.proof_public_inputs_digest,
            verifier_key_id: step.verifier_key_id.clone(),
            verifier_key_commitment: step.verifier_key_commitment,
            verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
        });
        step_digests.push(step_digest);
        all_inputs.extend(input_nullifiers);
        all_outputs.extend(output_commitments);
        expected_root = step.root_after;
    }

    let nullifier_digest =
        kagemusha_list_digest(KAGEMUSHA_FOLD_NULLIFIER_DIGEST_DOMAIN, all_inputs)?;
    let output_commitment_digest =
        kagemusha_list_digest(KAGEMUSHA_FOLD_OUTPUT_DIGEST_DOMAIN, all_outputs)?;
    let fold_digest = kagemusha_hash_preimage(&KagemushaFoldTranscriptDigestPreimage {
        domain: KAGEMUSHA_FOLD_TRANSCRIPT_DIGEST_DOMAIN.to_owned(),
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        step_digests,
    })?;
    if initial_root == expected_root {
        return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
    }
    let hop_count = u32::try_from(steps.len()).expect("hop count is bounded to u32");
    Ok(KagemushaCanonicalFoldParts {
        nullifier_digest,
        output_commitment_digest,
        fold_digest,
        aggregation_statement: KagemushaPoseidonAggregationTranscriptStatement {
            aggregation_mode: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
            chain_id: chain_id.clone(),
            asset: asset.clone(),
            initial_root,
            final_root: expected_root,
            hop_count,
            steps: aggregation_steps,
        },
    })
}

/// Build the canonical Poseidon2 aggregation transcript statement for Kagemusha folding.
///
/// The builder performs the same shape checks, per-hop canonicalization, duplicate detection, and
/// root-continuity checks as [`kagemusha_folded_public_inputs`]. Future recursive verifier
/// circuits and SDKs should use this function to derive the exact statement that
/// [`kagemusha_poseidon_aggregation_transcript_digest`] hashes.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the witness is empty, oversized, malformed, duplicate, root
/// discontinuous, or cannot be encoded with Norito.
pub fn kagemusha_poseidon_aggregation_transcript_statement(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    steps: &[KagemushaFoldStep],
) -> Result<KagemushaPoseidonAggregationTranscriptStatement, KagemushaFoldError> {
    Ok(kagemusha_canonical_fold_parts(chain_id, asset, steps)?.aggregation_statement)
}

/// Build reserved-mode recursive aggregation evidence from checked folded-hop material.
///
/// The builder canonicalizes the folded-hop transcript exactly like
/// [`kagemusha_folded_public_inputs`], then changes only the aggregation mode to
/// reserved mode `2` and binds the canonical no-trusted-setup verifier-witness
/// profile plus the caller-supplied native verifier-witness batch preflight
/// fields, including the verifier opening length and fixed-window table
/// schedule, shared-table manifest, and base digests that the batch preflight
/// and recursive table manifest bind.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the folded-hop witness is non-canonical,
/// the verifier parameter fingerprint is all-zero, the fixed-window table
/// schedule, shared-table manifest, or table-base digest is all-zero, or the
/// verifier-witness batch digest is all-zero.
#[allow(clippy::too_many_arguments)]
pub fn kagemusha_recursive_aggregation_evidence_from_steps(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    steps: &[KagemushaFoldStep],
    verifier_opening_len: u32,
    verifier_params_fingerprint: [u8; Hash::LENGTH],
    fixed_window_table_schedule_digest: [u8; Hash::LENGTH],
    fixed_window_shared_table_manifest_digest: [u8; Hash::LENGTH],
    fixed_window_table_base_digest: [u8; Hash::LENGTH],
    verifier_witness_batch_digest: [u8; Hash::LENGTH],
) -> Result<KagemushaRecursiveAggregationEvidence, KagemushaFoldError> {
    let mut aggregation_statement =
        kagemusha_canonical_fold_parts(chain_id, asset, steps)?.aggregation_statement;
    aggregation_statement.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
    let evidence = KagemushaRecursiveAggregationEvidence {
        verifier_witness_count: aggregation_statement.hop_count,
        verifier_witness_profile: KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1.to_owned(),
        verifier_opening_len,
        aggregation_statement,
        verifier_params_fingerprint,
        fixed_window_table_schedule_digest,
        fixed_window_shared_table_manifest_digest,
        fixed_window_table_base_digest,
        verifier_witness_batch_digest,
    };
    validate_kagemusha_recursive_aggregation_evidence(&evidence)?;
    Ok(evidence)
}

#[allow(clippy::struct_field_names)]
struct KagemushaFoldDigestParts {
    nullifier: Hash,
    output_commitment: Hash,
    fold: Hash,
}

fn kagemusha_fold_digest_parts_from_aggregation_statement(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<KagemushaFoldDigestParts, KagemushaFoldError> {
    validate_kagemusha_hashable_aggregation_transcript_statement(statement)?;

    let mut all_inputs = Vec::new();
    let mut all_outputs = Vec::new();
    let mut step_digests = Vec::with_capacity(statement.steps.len());

    for step in &statement.steps {
        let step_digest = kagemusha_hash_preimage(&KagemushaFoldStepDigestPreimage {
            domain: KAGEMUSHA_FOLD_STEP_DIGEST_DOMAIN.to_owned(),
            hop_index: step.hop_index,
            root_before: step.root_before,
            input_nullifiers: step.input_nullifiers.clone(),
            output_commitments: step.output_commitments.clone(),
            root_after: step.root_after,
            proof_hash: step.proof_hash,
            proof_public_inputs_digest: step.proof_public_inputs_digest,
            verifier_key_id: step.verifier_key_id.clone(),
            verifier_key_commitment: step.verifier_key_commitment,
            verifier_key_poseidon_digest: step.verifier_key_poseidon_digest,
        })?;
        step_digests.push(step_digest);
        all_inputs.extend(step.input_nullifiers.iter().copied());
        all_outputs.extend(step.output_commitments.iter().copied());
    }

    Ok(KagemushaFoldDigestParts {
        nullifier: kagemusha_list_digest(KAGEMUSHA_FOLD_NULLIFIER_DIGEST_DOMAIN, all_inputs)?,
        output_commitment: kagemusha_list_digest(KAGEMUSHA_FOLD_OUTPUT_DIGEST_DOMAIN, all_outputs)?,
        fold: kagemusha_hash_preimage(&KagemushaFoldTranscriptDigestPreimage {
            domain: KAGEMUSHA_FOLD_TRANSCRIPT_DIGEST_DOMAIN.to_owned(),
            chain_id: statement.chain_id.clone(),
            asset: statement.asset.clone(),
            step_digests,
        })?,
    })
}

/// Project a canonical aggregation transcript statement into folded public inputs.
///
/// Future recursive verifier circuits should produce the same public projection
/// from their private hop witness before proving `kagemusha-folded-v1`.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the statement is non-canonical or cannot
/// be encoded with Norito.
pub fn kagemusha_folded_public_inputs_from_aggregation_statement(
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<KagemushaFoldedPublicInputs, KagemushaFoldError> {
    let parts = kagemusha_fold_digest_parts_from_aggregation_statement(statement)?;
    let aggregation_transcript_digest =
        kagemusha_poseidon_aggregation_transcript_digest(statement)?;

    Ok(KagemushaFoldedPublicInputs {
        domain: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned(),
        aggregation_mode: statement.aggregation_mode,
        chain_id: statement.chain_id.clone(),
        asset: statement.asset.clone(),
        initial_root: statement.initial_root,
        final_root: statement.final_root,
        hop_count: statement.hop_count,
        nullifier_digest: parts.nullifier,
        output_commitment_digest: parts.output_commitment,
        fold_digest: parts.fold,
        aggregation_transcript_digest,
    })
}

/// Validate that folded public inputs are the canonical projection of an aggregation transcript.
///
/// This is the host-side equivalent of the public projection that recursive
/// Kagemusha verifier circuits must enforce in-circuit.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when either side is non-canonical or when any
/// folded public-input field differs from the aggregation transcript projection.
pub fn kagemusha_validate_folded_public_inputs_against_aggregation_statement(
    public_inputs: &KagemushaFoldedPublicInputs,
    statement: &KagemushaPoseidonAggregationTranscriptStatement,
) -> Result<(), KagemushaFoldError> {
    public_inputs.validate_supported_context()?;
    let expected = kagemusha_folded_public_inputs_from_aggregation_statement(statement)?;

    macro_rules! ensure_field {
        ($field:ident) => {
            if public_inputs.$field != expected.$field {
                return Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch {
                    field: stringify!($field),
                });
            }
        };
    }

    ensure_field!(chain_id);
    ensure_field!(asset);
    ensure_field!(aggregation_mode);
    ensure_field!(initial_root);
    ensure_field!(final_root);
    ensure_field!(hop_count);
    ensure_field!(nullifier_digest);
    ensure_field!(output_commitment_digest);
    ensure_field!(fold_digest);
    ensure_field!(aggregation_transcript_digest);

    Ok(())
}

fn validate_kagemusha_folded_public_inputs_projection(
    public_inputs: &KagemushaFoldedPublicInputs,
    expected: &KagemushaFoldedPublicInputs,
) -> Result<(), KagemushaFoldError> {
    macro_rules! ensure_field {
        ($field:ident) => {
            if public_inputs.$field != expected.$field {
                return Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch {
                    field: stringify!($field),
                });
            }
        };
    }

    ensure_field!(chain_id);
    ensure_field!(asset);
    ensure_field!(aggregation_mode);
    ensure_field!(initial_root);
    ensure_field!(final_root);
    ensure_field!(hop_count);
    ensure_field!(nullifier_digest);
    ensure_field!(output_commitment_digest);
    ensure_field!(fold_digest);
    ensure_field!(aggregation_transcript_digest);

    Ok(())
}

/// Validate a reserved recursive evidence statement against its folded public-input projection.
///
/// This is the host-side projection contract for ABI-7
/// `kagemusha-recursive-compact-v1` admission. It deliberately accepts only the
/// reserved recursive aggregation mode `2` and does not make
/// [`KagemushaFoldedPublicInputs::validate_supported_context`] accept mode `2`
/// compact tokens.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the evidence is not canonical reserved
/// recursive evidence, when the folded public-input domain is not canonical, or
/// when any folded public-input field is not the exact projection of the
/// evidence aggregation transcript.
pub fn kagemusha_validate_recursive_evidence_folded_public_input_projection(
    public_inputs: &KagemushaFoldedPublicInputs,
    evidence: &KagemushaRecursiveAggregationEvidence,
) -> Result<(), KagemushaFoldError> {
    validate_kagemusha_recursive_aggregation_evidence(evidence)?;
    if public_inputs.domain != KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN {
        return Err(KagemushaFoldError::InvalidPublicInputDomain {
            expected: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN,
            actual: public_inputs.domain.clone(),
        });
    }
    if public_inputs.aggregation_mode != KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1 {
        return Err(KagemushaFoldError::UnsupportedAggregationMode {
            expected: KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1,
            actual: public_inputs.aggregation_mode,
            reason: "reserved recursive evidence projection requires Kagemusha aggregation mode 2",
        });
    }
    let expected =
        kagemusha_folded_public_inputs_from_aggregation_statement(&evidence.aggregation_statement)?;
    validate_kagemusha_folded_public_inputs_projection(public_inputs, &expected)
}

/// Validate the chain-visible folded projection claimed by a recursive proof.
///
/// This is the public-input-only relation future
/// `kagemusha-recursive-compact-v1` admission can enforce from a compact token:
/// the folded public inputs must be canonical reserved mode `2`, and the
/// recursive proof public inputs must expose the exact folded public-input hash,
/// aggregation transcript digest, and hop count carried by that token.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the folded input shape is not reserved
/// recursive compact mode, the recursive proof public inputs are malformed, or
/// the two public surfaces do not match.
pub fn kagemusha_validate_recursive_proof_folded_public_input_projection(
    public_inputs: &KagemushaFoldedPublicInputs,
    recursive_proof: &KagemushaRecursiveAggregationProof,
) -> Result<(), KagemushaFoldError> {
    public_inputs.validate_recursive_compact_context()?;
    recursive_proof.validate_public_input_binding()?;
    let expected_hash = public_inputs.public_inputs_hash()?;
    let mut expected_hash_bytes = [0u8; Hash::LENGTH];
    expected_hash_bytes.copy_from_slice(expected_hash.as_ref());
    if recursive_proof.public_inputs.folded_public_inputs_hash != expected_hash_bytes {
        return Err(KagemushaFoldError::PublicInputHashMismatch {
            expected: expected_hash,
            actual: Hash::prehashed(recursive_proof.public_inputs.folded_public_inputs_hash),
        });
    }
    if recursive_proof.public_inputs.aggregation_transcript_digest
        != public_inputs.aggregation_transcript_digest
    {
        return Err(
            KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                field: "aggregation_transcript_digest",
            },
        );
    }
    if recursive_proof.public_inputs.hop_count != public_inputs.hop_count {
        return Err(
            KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch { field: "hop_count" },
        );
    }
    Ok(())
}

/// Build the chain-visible public inputs for a compact folded Kagemusha token.
///
/// The builder canonicalizes nullifier and output order inside each hop because the ledger treats
/// them as sets and appends output commitments deterministically. Adjacent hops must be root
/// continuous, and nullifiers/commitments may not repeat across the folded witness.
///
/// # Errors
///
/// Returns [`KagemushaFoldError`] when the witness is empty, oversized, malformed, duplicate, root
/// discontinuous, or cannot be encoded with Norito.
pub fn kagemusha_folded_public_inputs(
    chain_id: &ChainId,
    asset: &AssetDefinitionId,
    steps: &[KagemushaFoldStep],
) -> Result<KagemushaFoldedPublicInputs, KagemushaFoldError> {
    let parts = kagemusha_canonical_fold_parts(chain_id, asset, steps)?;

    let aggregation_transcript_digest =
        kagemusha_poseidon_aggregation_transcript_digest(&parts.aggregation_statement)?;

    Ok(KagemushaFoldedPublicInputs {
        domain: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned(),
        aggregation_mode: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
        chain_id: chain_id.clone(),
        asset: asset.clone(),
        initial_root: parts.aggregation_statement.initial_root,
        final_root: parts.aggregation_statement.final_root,
        hop_count: parts.aggregation_statement.hop_count,
        nullifier_digest: parts.nullifier_digest,
        output_commitment_digest: parts.output_commitment_digest,
        fold_digest: parts.fold_digest,
        aggregation_transcript_digest,
    })
}

impl KagemushaFoldedPublicInputs {
    /// Validate the domain and aggregation mode supported by the legacy compact path.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError::InvalidPublicInputDomain`] when the domain separator is not
    /// canonical, or [`KagemushaFoldError::UnsupportedAggregationMode`] when the folded token
    /// declares recursive compact mode `2` or an unknown aggregation mode.
    pub fn validate_supported_context(&self) -> Result<(), KagemushaFoldError> {
        if self.domain != KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN {
            return Err(KagemushaFoldError::InvalidPublicInputDomain {
                expected: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN,
                actual: self.domain.clone(),
            });
        }
        if !is_supported_kagemusha_aggregation_mode(self.aggregation_mode) {
            return Err(KagemushaFoldError::UnsupportedAggregationMode {
                expected: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
                actual: self.aggregation_mode,
                reason: unsupported_kagemusha_aggregation_mode_reason(self.aggregation_mode),
            });
        }
        if self.hop_count == 0 {
            return Err(KagemushaFoldError::Empty);
        }
        if usize::try_from(self.hop_count).map_or(true, |hop_count| {
            hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS
        }) {
            return Err(KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: usize::try_from(self.hop_count).unwrap_or(usize::MAX),
            });
        }
        validate_kagemusha_fold_root("initial_root", self.initial_root)?;
        validate_kagemusha_fold_root("final_root", self.final_root)?;
        if self.initial_root == self.final_root {
            return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
        }
        if self.aggregation_transcript_digest == [0u8; Hash::LENGTH] {
            return Err(KagemushaFoldError::ZeroFoldedPublicInputDigest {
                field: "aggregation_transcript_digest",
            });
        }
        let encoded_len = self.norito_encoded_len()?;
        if encoded_len > KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES {
            return Err(KagemushaFoldError::EncodedSizeExceeded {
                max: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES,
                actual: encoded_len,
            });
        }
        Ok(())
    }

    /// Validate the reserved recursive compact-token public-input context.
    ///
    /// This deliberately does not change [`Self::validate_supported_context`]:
    /// checked pre-fold mode `1` remains the only mode accepted by the legacy
    /// compact-token path. ABI-7 recursive compact admission calls this narrower
    /// validator and then verifies a recursive proof whose
    /// `folded_public_inputs_hash` matches [`Self::public_inputs_hash`].
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the public inputs are not the
    /// canonical reserved mode-2 folded public-input shape.
    pub fn validate_recursive_compact_context(&self) -> Result<(), KagemushaFoldError> {
        if self.domain != KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN {
            return Err(KagemushaFoldError::InvalidPublicInputDomain {
                expected: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN,
                actual: self.domain.clone(),
            });
        }
        if self.aggregation_mode != KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1 {
            return Err(KagemushaFoldError::UnsupportedAggregationMode {
                expected: KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1,
                actual: self.aggregation_mode,
                reason: "recursive compact admission requires Kagemusha aggregation mode 2",
            });
        }
        if self.hop_count == 0 {
            return Err(KagemushaFoldError::Empty);
        }
        if usize::try_from(self.hop_count).map_or(true, |hop_count| {
            hop_count > KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS
        }) {
            return Err(KagemushaFoldError::TooManyHops {
                max: KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS,
                actual: usize::try_from(self.hop_count).unwrap_or(usize::MAX),
            });
        }
        validate_kagemusha_fold_root("initial_root", self.initial_root)?;
        validate_kagemusha_fold_root("final_root", self.final_root)?;
        if self.initial_root == self.final_root {
            return Err(KagemushaFoldError::UnchangedFoldedPublicRoots);
        }
        for (field, digest) in [
            (
                "nullifier_digest",
                hash_bytes_from_hash(self.nullifier_digest),
            ),
            (
                "output_commitment_digest",
                hash_bytes_from_hash(self.output_commitment_digest),
            ),
            ("fold_digest", hash_bytes_from_hash(self.fold_digest)),
            (
                "aggregation_transcript_digest",
                self.aggregation_transcript_digest,
            ),
        ] {
            if digest == [0u8; Hash::LENGTH] {
                return Err(KagemushaFoldError::ZeroFoldedPublicInputDigest { field });
            }
        }
        let encoded_len = self.norito_encoded_len()?;
        if encoded_len > KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES {
            return Err(KagemushaFoldError::EncodedSizeExceeded {
                max: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES,
                actual: encoded_len,
            });
        }
        Ok(())
    }

    /// Deterministic hash that the compact folded proof must expose as public inputs.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn public_inputs_hash(&self) -> Result<Hash, norito::Error> {
        to_bytes(self).map(Hash::new)
    }

    /// Return the canonical Norito encoded size for folded public inputs.
    ///
    /// Wallets and QR/NFC transports can use this to enforce payload budgets
    /// before attaching backend-specific proof bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the public-input payload cannot be serialized with Norito.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

impl KagemushaCompactPaymentToken {
    /// Build a recursive compact token from a folded projection and recursive proof.
    ///
    /// This constructor is for recursive mode `2`; it validates the folded
    /// public-input projection against the recursive proof but does not route
    /// through [`Self::validate_public_input_binding`], which is intentionally
    /// reserved for the legacy checked-prefold compact-token path.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError`] when the folded public inputs are not in
    /// recursive compact mode, the recursive proof public inputs are malformed,
    /// or the proof does not expose the folded public-input hash.
    pub fn from_recursive_compact_projection(
        public_inputs: KagemushaFoldedPublicInputs,
        recursive_proof: KagemushaRecursiveAggregationProof,
    ) -> Result<Self, KagemushaFoldError> {
        public_inputs.validate_recursive_compact_context()?;
        recursive_proof.public_inputs.validate_context()?;
        if !is_supported_kagemusha_proof_backend(&recursive_proof.proof.backend) {
            return Err(KagemushaFoldError::UnsupportedProofBackend {
                backend: recursive_proof.proof.backend.clone(),
            });
        }
        if !is_supported_kagemusha_proof_backend(&recursive_proof.verifier_key_id.backend) {
            return Err(KagemushaFoldError::UnsupportedProofBackend {
                backend: recursive_proof.verifier_key_id.backend.clone(),
            });
        }
        if recursive_proof.proof.backend != recursive_proof.verifier_key_id.backend {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofBackendMismatch {
                    proof_backend: recursive_proof.proof.backend.clone(),
                    verifier_key_backend: recursive_proof.verifier_key_id.backend.clone(),
                },
            );
        }
        if recursive_proof.proof.backend != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND {
            return Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.backend",
            });
        }
        if recursive_proof.proof.bytes.is_empty() {
            return Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.bytes",
            });
        }
        if recursive_proof.verifier_key_id.name
            != KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            && recursive_proof.verifier_key_id.name != KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1
            && !is_kagemusha_recursive_spend_lineage_proof_circuit_id(
                &recursive_proof.verifier_key_id.name,
            )
        {
            return Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "verifier_key_id.name",
            });
        }
        let recursive_public_inputs_hash = recursive_proof.public_inputs.public_inputs_hash()?;
        if recursive_proof.public_inputs_hash != recursive_public_inputs_hash {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch {
                    expected: recursive_public_inputs_hash,
                    actual: recursive_proof.public_inputs_hash,
                },
            );
        }
        let expected_hash = public_inputs.public_inputs_hash()?;
        let mut expected_hash_bytes = [0u8; Hash::LENGTH];
        expected_hash_bytes.copy_from_slice(expected_hash.as_ref());
        if recursive_proof.public_inputs.folded_public_inputs_hash != expected_hash_bytes {
            return Err(KagemushaFoldError::PublicInputHashMismatch {
                expected: expected_hash,
                actual: Hash::prehashed(recursive_proof.public_inputs.folded_public_inputs_hash),
            });
        }
        if recursive_proof.public_inputs.aggregation_transcript_digest
            != public_inputs.aggregation_transcript_digest
        {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "aggregation_transcript_digest",
                },
            );
        }
        if recursive_proof.public_inputs.hop_count != public_inputs.hop_count {
            return Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "hop_count",
                },
            );
        }
        let public_inputs_hash = public_inputs.public_inputs_hash()?;
        Ok(Self {
            public_inputs,
            folded_proof: KagemushaFoldedProof {
                verifier_key_id: recursive_proof.verifier_key_id,
                public_inputs_hash,
                proof: recursive_proof.proof,
            },
        })
    }

    /// Validate that the folded proof is bound to the canonical folded public inputs.
    ///
    /// This does not verify the proof cryptographically; it prevents accepting a compact token
    /// whose proof declares public inputs for a different folded transcript before the backend
    /// verifier runs.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaFoldError::PublicInputHashMismatch`] when the proof's declared public
    /// input hash differs from the canonical public-input hash, or [`KagemushaFoldError::Encode`]
    /// when the public inputs cannot be serialized.
    pub fn validate_public_input_binding(&self) -> Result<(), KagemushaFoldError> {
        self.public_inputs.validate_supported_context()?;
        let expected = self.public_inputs.public_inputs_hash()?;
        let actual = self.folded_proof.public_inputs_hash;
        if actual != expected {
            return Err(KagemushaFoldError::PublicInputHashMismatch { expected, actual });
        }
        Ok(())
    }

    /// Return the canonical Norito encoded size for this compact payment token.
    ///
    /// This includes the backend proof payload carried by [`KagemushaFoldedProof`].
    ///
    /// # Errors
    ///
    /// Returns an error when the token cannot be serialized with Norito.
    pub fn norito_encoded_len(&self) -> Result<usize, norito::Error> {
        to_bytes(self).map(|bytes| bytes.len())
    }
}

fn validate_offline_note_random_bytes(
    field: &'static str,
    bytes: &[u8],
) -> Result<(), OfflineNoteDerivationError> {
    if bytes.len() != Hash::LENGTH {
        return Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
            field,
            expected: Hash::LENGTH,
            actual: bytes.len(),
        });
    }
    Ok(())
}

/// Derive the canonical Offline Note note commitment from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `note_secret` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_note_commitment(
    preimage: &OfflineNoteCommitmentPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("note_secret", &preimage.note_secret)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

/// Derive the canonical Offline Note input nullifier from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `note_secret` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_input_nullifier(
    preimage: &OfflineNoteInputNullifierPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("note_secret", &preimage.note_secret)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

/// Derive the canonical Offline Note payment token id from a wallet preimage.
///
/// # Errors
///
/// Returns an error when `token_nonce` is not exactly 32 bytes or the preimage
/// cannot be encoded with Norito.
pub fn derive_offline_note_payment_token_id(
    preimage: &OfflineNotePaymentTokenIdPreimage,
) -> Result<Hash, OfflineNoteDerivationError> {
    validate_offline_note_random_bytes("token_nonce", &preimage.token_nonce)?;
    let bytes = to_bytes(preimage)?;
    Ok(Hash::new(bytes))
}

#[cfg(test)]
mod offline_note_tests {
    #![allow(
        clippy::assertions_on_constants,
        clippy::items_after_statements,
        clippy::option_if_let_else,
        clippy::too_many_lines,
        clippy::type_complexity
    )]

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use iroha_crypto::{Algorithm, KeyPair, PublicKey};
    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::{asset::AssetDefinitionId, confidential::ConfidentialStatus, domain::DomainId};

    fn sample_signature(seed: u8) -> Signature {
        let mut payload = [0u8; 64];
        for (idx, byte) in payload.iter_mut().enumerate() {
            let offset = u8::try_from(idx).expect("index fits into u8");
            *byte = seed.wrapping_add(offset);
        }
        Signature::from_bytes(&payload)
    }

    fn sample_public_key(seed: u8) -> PublicKey {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair");
        key_pair.public_key().clone()
    }

    fn sample_account(seed: u8, domain: &str) -> AccountId {
        let key = sample_public_key(seed);
        let _domain_id = DomainId::try_new(domain, "universal").expect("domain id");
        AccountId::new(key)
    }

    fn fixed_hash(label: &[u8]) -> [u8; Hash::LENGTH] {
        Hash::new(label).into()
    }

    fn shared_recursive_spend_archive_entry_json(
        name: &str,
        operation: &str,
        norito_type: &str,
        bytes: &[u8],
    ) -> String {
        let sha256_hex = hex::encode(Sha256::digest(bytes));
        format!(
            concat!(
                "    {{\n",
                "      \"name\": \"{name}\",\n",
                "      \"operation\": \"{operation}\",\n",
                "      \"norito_type\": \"{norito_type}\",\n",
                "      \"byte_len\": {byte_len},\n",
                "      \"sha256_hex\": \"{sha256_hex}\",\n",
                "      \"bytes_base64\": \"{bytes_base64}\"\n",
                "    }}"
            ),
            name = name,
            operation = operation,
            norito_type = norito_type,
            byte_len = bytes.len(),
            sha256_hex = sha256_hex,
            bytes_base64 = BASE64_STANDARD.encode(bytes),
        )
    }

    fn shared_recursive_spend_request_archive_fields_json() -> &'static str {
        concat!(
            "  \"request_archive_fields\": [\n",
            "    {\n",
            "      \"norito_type\": \"KagemushaRecursiveSpendInitRequestV1\",\n",
            "      \"fields\": [\n",
            "        {\"name\": \"record_bundle\", \"type\": \"KagemushaVerifiedFoldRecordBundle\", \"norito_default\": false},\n",
            "        {\"name\": \"pallas_open_envelopes_archive\", \"type\": \"Vec<u8>\", \"norito_default\": false},\n",
            "        {\"name\": \"current_note\", \"type\": \"KagemushaSpendableNoteDescriptorV1\", \"norito_default\": false},\n",
            "        {\"name\": \"lineage_verifier_key\", \"type\": \"Option<VerifyingKeyBox>\", \"norito_default\": true},\n",
            "        {\"name\": \"lineage_proving_key_archive\", \"type\": \"Option<Vec<u8>>\", \"norito_default\": true},\n",
            "        {\"name\": \"block_height\", \"type\": \"Option<u64>\", \"norito_default\": true, \"semantics\": \"verifier_record_activation_height\"}\n",
            "      ]\n",
            "    },\n",
            "    {\n",
            "      \"norito_type\": \"KagemushaRecursiveSpendAppendRequestV1\",\n",
            "      \"fields\": [\n",
            "        {\"name\": \"previous_bundle\", \"type\": \"KagemushaRecursiveSpendBundleV1\", \"norito_default\": false},\n",
            "        {\"name\": \"record_bundle\", \"type\": \"KagemushaVerifiedFoldRecordBundle\", \"norito_default\": false},\n",
            "        {\"name\": \"pallas_open_envelopes_archive\", \"type\": \"Vec<u8>\", \"norito_default\": false},\n",
            "        {\"name\": \"current_note\", \"type\": \"KagemushaSpendableNoteDescriptorV1\", \"norito_default\": false},\n",
            "        {\"name\": \"output_proof_circuit_id\", \"type\": \"String\", \"norito_default\": true},\n",
            "        {\"name\": \"previous_lineage_verifier_record\", \"type\": \"Option<VerifyingKeyRecord>\", \"norito_default\": true},\n",
            "        {\"name\": \"previous_recursive_proof_open_envelopes_archive\", \"type\": \"Vec<u8>\", \"norito_default\": true},\n",
            "        {\"name\": \"lineage_verifier_key\", \"type\": \"Option<VerifyingKeyBox>\", \"norito_default\": true},\n",
            "        {\"name\": \"lineage_proving_key_archive\", \"type\": \"Option<Vec<u8>>\", \"norito_default\": true},\n",
            "        {\"name\": \"block_height\", \"type\": \"Option<u64>\", \"norito_default\": true, \"semantics\": \"verifier_record_activation_height\"}\n",
            "      ]\n",
            "    },\n",
            "    {\n",
            "      \"norito_type\": \"KagemushaRecursiveSpendVerifyRequestV1\",\n",
            "      \"fields\": [\n",
            "        {\"name\": \"bundle\", \"type\": \"KagemushaRecursiveSpendBundleV1\", \"norito_default\": false},\n",
            "        {\"name\": \"lineage_verifier_record\", \"type\": \"Option<VerifyingKeyRecord>\", \"norito_default\": true},\n",
            "        {\"name\": \"block_height\", \"type\": \"Option<u64>\", \"norito_default\": true, \"semantics\": \"verifier_record_activation_height\"}\n",
            "      ]\n",
            "    },\n",
            "    {\n",
            "      \"norito_type\": \"KagemushaRecursiveSpendRedeemRequestV1\",\n",
            "      \"fields\": [\n",
            "        {\"name\": \"bundle\", \"type\": \"KagemushaRecursiveSpendBundleV1\", \"norito_default\": false},\n",
            "        {\"name\": \"recipient\", \"type\": \"AccountId\", \"norito_default\": false},\n",
            "        {\"name\": \"public_amount\", \"type\": \"u128\", \"norito_default\": false},\n",
            "        {\"name\": \"redeem_proof\", \"type\": \"ProofAttachment\", \"norito_default\": false},\n",
            "        {\"name\": \"lineage_witness\", \"type\": \"Option<KagemushaRecursiveSpendLineageWitnessV1>\", \"norito_default\": false},\n",
            "        {\"name\": \"change_output\", \"type\": \"Option<[u8; 32]>\", \"norito_default\": false, \"semantics\": \"private_change_commitment_for_partial_redeem\"},\n",
            "        {\"name\": \"lineage_verifier_record\", \"type\": \"Option<VerifyingKeyRecord>\", \"norito_default\": true},\n",
            "        {\"name\": \"block_height\", \"type\": \"Option<u64>\", \"norito_default\": true, \"semantics\": \"verifier_record_activation_height\"}\n",
            "      ]\n",
            "    }\n",
            "  ]"
        )
    }

    fn shared_recursive_spend_archive_fixture_json(
        archives: &[(&str, &str, &str, &[u8])],
    ) -> String {
        let request_archive_fields = shared_recursive_spend_request_archive_fields_json();
        let entries = archives
            .iter()
            .map(|(name, operation, norito_type, bytes)| {
                shared_recursive_spend_archive_entry_json(name, operation, norito_type, bytes)
            })
            .collect::<Vec<_>>()
            .join(",\n");
        format!(
            concat!(
                "{{\n",
                "  \"schema\": \"iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1\",\n",
                "  \"fixture_kind\": \"norito_archives\",\n",
                "  \"native_bridge_abi_version\": 6,\n",
                "{request_archive_fields},\n",
                "  \"archives\": [\n",
                "{entries}\n",
                "  ]\n",
                "}}\n"
            ),
            request_archive_fields = request_archive_fields,
            entries = entries,
        )
    }

    fn assert_shared_recursive_spend_abi6_archive_fixture_matches(
        archives: &[(&str, &str, &str, &[u8])],
    ) {
        let generated = shared_recursive_spend_archive_fixture_json(archives);
        if std::env::var_os("KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI6_ARCHIVES").is_some() {
            println!("{generated}");
            return;
        }
        let committed =
            include_str!("../../../../fixtures/kagemusha_recursive_spend_abi6/archives.json");
        assert_eq!(
            committed, generated,
            "shared recursive spend ABI-6 Norito archive fixtures drifted; rerun with \
             KAGEMUSHA_RECURSIVE_SPEND_PRINT_ABI6_ARCHIVES=1 to regenerate"
        );
    }

    fn kagemusha_recursive_verifier_preflight_for_evidence(
        evidence: &KagemushaRecursiveAggregationEvidence,
        aggregate_digest: [u8; Hash::LENGTH],
    ) -> KagemushaRecursiveVerifierPreflightV1 {
        KagemushaRecursiveVerifierPreflightV1 {
            proof_count: 1,
            verifier_witness_profile: KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1.to_owned(),
            opening_len: evidence.verifier_opening_len,
            params_fingerprint: evidence.verifier_params_fingerprint,
            fixed_window_table_schedule_digest: evidence.fixed_window_table_schedule_digest,
            fixed_window_shared_table_manifest_digest: evidence
                .fixed_window_shared_table_manifest_digest,
            fixed_window_table_base_digest: evidence.fixed_window_table_base_digest,
            aggregate_digest,
        }
    }

    fn append_zk1_tlv(bytes: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
        bytes.extend_from_slice(&tag);
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("test TLV payload length fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(payload);
    }

    fn kagemusha_lineage_key_artifact_vk(circuit_id: &str, payload_seed: u8) -> VerifyingKeyBox {
        let mut bytes = b"ZK1\0".to_vec();
        append_zk1_tlv(&mut bytes, *b"IPAK", &8u32.to_le_bytes());
        append_zk1_tlv(&mut bytes, *b"CID1", circuit_id.as_bytes());
        append_zk1_tlv(&mut bytes, *b"H2VK", &[payload_seed; 32]);
        VerifyingKeyBox::new("halo2/ipa".into(), bytes)
    }

    fn kagemusha_lineage_key_artifact_pk_archive_with_commitment(
        circuit_id: &str,
        vk_commitment: [u8; Hash::LENGTH],
        payload: Vec<u8>,
    ) -> Vec<u8> {
        to_bytes(&KagemushaLineageProvingKeyArchiveV1 {
            version: KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1,
            circuit_family: circuit_id.to_owned(),
            vk_commitment,
            proving_key: payload,
        })
        .expect("encode test lineage proving-key archive")
    }

    fn kagemusha_lineage_key_artifact_pk_archive(
        circuit_id: &str,
        vk: &VerifyingKeyBox,
        payload_seed: u8,
    ) -> Vec<u8> {
        kagemusha_lineage_key_artifact_pk_archive_with_commitment(
            circuit_id,
            kagemusha_verifying_key_commitment(vk),
            vec![payload_seed; 64],
        )
    }

    #[test]
    fn kagemusha_lineage_key_artifact_packages_reject_profile_splices() {
        let init_verifier_key = kagemusha_lineage_key_artifact_vk(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            0xE7,
        );
        let init_proving_key_archive = kagemusha_lineage_key_artifact_pk_archive(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            &init_verifier_key,
            0xE8,
        );
        KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
            2,
            init_verifier_key.clone(),
            init_proving_key_archive.clone(),
        )
        .expect("canonical init lineage artifact package validates");

        let append_verifier_key = kagemusha_lineage_key_artifact_vk(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            0xA7,
        );
        let append_proving_key_archive = kagemusha_lineage_key_artifact_pk_archive(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            &append_verifier_key,
            0xA8,
        );
        KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_append(
            2,
            append_verifier_key.clone(),
            append_proving_key_archive.clone(),
        )
        .expect("canonical append lineage artifact package validates");

        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                append_verifier_key.clone(),
                init_proving_key_archive.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_append(
                2,
                init_verifier_key.clone(),
                append_proving_key_archive.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                init_verifier_key.clone(),
                append_proving_key_archive,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));

        let malformed_vk = VerifyingKeyBox::new("halo2/ipa".into(), b"ZK1\0".to_vec());
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                malformed_vk,
                init_proving_key_archive.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));

        let mut duplicate_cid_vk = init_verifier_key.clone();
        append_zk1_tlv(
            &mut duplicate_cid_vk.bytes,
            *b"CID1",
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.as_bytes(),
        );
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                duplicate_cid_vk,
                init_proving_key_archive.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));

        let wrong_commitment_pk = kagemusha_lineage_key_artifact_pk_archive_with_commitment(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            [0x5A; Hash::LENGTH],
            vec![0xE8; 64],
        );
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                init_verifier_key.clone(),
                wrong_commitment_pk,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));

        let bad_version_pk = to_bytes(&KagemushaLineageProvingKeyArchiveV1 {
            version: KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1 + 1,
            circuit_family: KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
                .to_owned(),
            vk_commitment: kagemusha_verifying_key_commitment(&init_verifier_key),
            proving_key: vec![0xE8; 64],
        })
        .expect("encode bad-version archive");
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                init_verifier_key.clone(),
                bad_version_pk,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));

        let empty_payload_pk = kagemusha_lineage_key_artifact_pk_archive_with_commitment(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            kagemusha_verifying_key_commitment(&init_verifier_key),
            Vec::new(),
        );
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                init_verifier_key,
                empty_payload_pk,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));
    }

    #[test]
    fn kagemusha_recursive_compact_key_packages_accept_supported_subsets() {
        let one_hop_verifier_key =
            kagemusha_lineage_key_artifact_vk(KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1, 0xB1);
        let append_verifier_key =
            kagemusha_lineage_key_artifact_vk(KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1, 0xB2);
        let entry = KagemushaRecursiveCompactKeyArtifactEntryV1::new(
            4,
            one_hop_verifier_key.clone(),
            kagemusha_lineage_key_artifact_pk_archive(
                KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
                &one_hop_verifier_key,
                0xB3,
            ),
            append_verifier_key.clone(),
            kagemusha_lineage_key_artifact_pk_archive(
                KAGEMUSHA_RECURSIVE_COMPACT_CIRCUIT_ID_V1,
                &append_verifier_key,
                0xB4,
            ),
        )
        .expect("single-width recursive compact package entry");

        let package = KagemushaRecursiveCompactKeyArtifactsV1::new(vec![entry.clone()])
            .expect("single-width recursive compact key package");
        assert_eq!(
            package
                .entry_for_opening_len(4)
                .expect("LEN=4 compact package entry")
                .verifier_opening_len,
            4
        );
        assert!(matches!(
            package.entry_for_opening_len(8),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_opening_len"
            })
        ));
        let verifier_keys = package
            .verifier_keys()
            .expect("single-width recursive compact verifier keys");
        assert_eq!(
            verifier_keys
                .entry_for_opening_len(4)
                .expect("LEN=4 verifier key entry")
                .verifier_opening_len,
            4
        );

        assert!(matches!(
            KagemushaRecursiveCompactKeyArtifactsV1::new(Vec::new()),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_compact_key_artifacts.entries"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveCompactKeyArtifactsV1::new(vec![entry.clone(), entry.clone()]),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_compact_key_artifacts.entries"
            })
        ));

        let unsupported_entry = KagemushaRecursiveCompactKeyArtifactEntryV1 {
            verifier_opening_len: 3,
            one_hop_verifier_key,
            one_hop_proving_key_archive: entry.one_hop_proving_key_archive,
            append_verifier_key,
            append_proving_key_archive: entry.append_proving_key_archive,
        };
        assert!(matches!(
            KagemushaRecursiveCompactKeyArtifactsV1::new(vec![unsupported_entry]),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_compact_key_artifacts.entries"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveCompactVerifierKeysV1::new(Vec::new()),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_compact_key_artifacts.entries"
            })
        ));
    }

    fn kagemusha_asset(name: &str) -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            name.parse().expect("asset name"),
        )
    }

    #[test]
    fn offline_note_v2_recursive_schema_hash_alias_matches_canonical_hash() {
        assert_eq!(
            offline_note_v2_recursive_public_inputs_schema_hash(),
            offline_note_recursive_public_inputs_schema_hash()
        );
    }

    fn kagemusha_step(
        root_before: [u8; Hash::LENGTH],
        root_after: [u8; Hash::LENGTH],
        input_seed: u8,
        output_seed: u8,
        proof_label: &'static [u8],
    ) -> KagemushaFoldStep {
        let mut proof_inputs_label = proof_label.to_vec();
        proof_inputs_label.extend_from_slice(b":public-inputs");
        KagemushaFoldStep {
            root_before,
            input_nullifiers: vec![
                [input_seed.wrapping_add(1); Hash::LENGTH],
                [input_seed; Hash::LENGTH],
            ],
            output_commitments: vec![
                [output_seed.wrapping_add(1); Hash::LENGTH],
                [output_seed; Hash::LENGTH],
            ],
            root_after,
            proof_hash: Hash::new(proof_label),
            proof_public_inputs_digest: fixed_hash(&proof_inputs_label),
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-fixture"),
            verifier_key_commitment: fixed_hash(proof_label),
            verifier_key_poseidon_digest: kagemusha_verifier_key_poseidon_digest(
                "halo2/ipa",
                proof_label,
            )
            .expect("verifier-key digest"),
        }
    }

    fn sample_kagemusha_recursive_aggregation_evidence() -> KagemushaRecursiveAggregationEvidence {
        let chain_id: ChainId = "kagemusha-recursive-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive");
        let root0 = fixed_hash(b"kagemusha-recursive-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-proof-hop-1"),
        ];
        kagemusha_recursive_aggregation_evidence_from_steps(
            &chain_id,
            &asset,
            &steps,
            4,
            fixed_hash(b"recursive-pallas-params"),
            fixed_hash(b"recursive-fixed-window-schedule"),
            fixed_hash(b"recursive-fixed-window-shared-manifest"),
            fixed_hash(b"recursive-fixed-window-bases"),
            fixed_hash(b"recursive-pallas-witness-batch"),
        )
        .expect("recursive aggregation evidence")
    }

    fn kagemusha_recursive_spend_note(
        note_label: &[u8],
        nullifier_label: &[u8],
        amount: u128,
    ) -> KagemushaSpendableNoteDescriptorV1 {
        KagemushaSpendableNoteDescriptorV1 {
            note_commitment: fixed_hash(note_label),
            spend_nullifier: fixed_hash(nullifier_label),
            amount: Numeric::new(amount, 0),
        }
    }

    fn kagemusha_recursive_spend_one_hop_evidence(
        chain_id: &ChainId,
        asset: &AssetDefinitionId,
        step: KagemushaFoldStep,
        witness_label: &[u8],
    ) -> KagemushaRecursiveAggregationEvidence {
        kagemusha_recursive_aggregation_evidence_from_steps(
            chain_id,
            asset,
            &[step],
            4,
            fixed_hash(b"recursive-spend-pallas-params"),
            fixed_hash(b"recursive-spend-fixed-window-schedule"),
            fixed_hash(b"recursive-spend-fixed-window-shared-manifest"),
            fixed_hash(b"recursive-spend-fixed-window-bases"),
            fixed_hash(witness_label),
        )
        .expect("one-hop recursive spend evidence")
    }

    fn kagemusha_recursive_spend_proof(
        accumulator: &KagemushaRecursiveSpendAccumulatorV1,
    ) -> KagemushaRecursiveAggregationProof {
        let public_inputs = accumulator
            .recursive_public_inputs()
            .expect("recursive spend public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive spend public-input hash");
        KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 256]),
        }
    }

    fn kagemusha_recursive_spend_lineage_proof(
        accumulator: &KagemushaRecursiveSpendAccumulatorV1,
        scalar_projection_label: &[u8],
    ) -> KagemushaRecursiveAggregationProof {
        let mut proof = kagemusha_recursive_spend_proof(accumulator);
        proof.verifier_key_id.name = KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.into();
        proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest = fixed_hash(scalar_projection_label);
        proof.public_inputs_hash = proof
            .public_inputs
            .public_inputs_hash()
            .expect("recursive spend lineage public-input hash");
        proof
    }

    fn kagemusha_recursive_spend_bundle(
        accumulator: KagemushaRecursiveSpendAccumulatorV1,
    ) -> KagemushaRecursiveSpendBundleV1 {
        let recursive_proof = kagemusha_recursive_spend_proof(&accumulator);
        KagemushaRecursiveSpendBundleV1 {
            accumulator,
            recursive_proof,
        }
    }

    fn kagemusha_recursive_spend_record_bundle_for_step(
        chain_id: ChainId,
        asset: AssetDefinitionId,
        step: &KagemushaFoldStep,
        vk_name: &'static str,
        proof_label: &'static [u8],
    ) -> KagemushaVerifiedFoldRecordBundle {
        let vk_id = VerifyingKeyId::new("halo2/ipa", vk_name);
        let verifier_key = VerifyingKeyBox::new("halo2/ipa".into(), vec![0x42; 48]);
        let vk_commitment = kagemusha_verifying_key_commitment(&verifier_key);
        let proof_schema = b"kagemusha-recursive-lineage-hop-public-inputs-v1".to_vec();
        let proof_envelope = crate::zk::OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:kagemusha-hop-fixture".to_owned(),
            vk_hash: vk_commitment,
            public_inputs: proof_schema.clone(),
            proof_bytes: proof_label.to_vec(),
            aux: Vec::new(),
        };
        let proof = ProofBox::new(
            "halo2/ipa".into(),
            to_bytes(&proof_envelope).expect("encode hop OpenVerifyEnvelope"),
        );
        let mut attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id.clone());
        attachment.vk_commitment = Some(vk_commitment);
        let lineage_step = KagemushaVerifiedFoldStep {
            root_before: step.root_before,
            input_nullifiers: step.input_nullifiers.clone(),
            output_commitments: step.output_commitments.clone(),
            root_after: step.root_after,
            attachment,
            verifier_key: verifier_key.clone(),
        };
        let mut record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:kagemusha-hop-fixture",
            BackendTag::Halo2IpaPasta,
            "pallas",
            Hash::new(proof_schema.as_slice()).into(),
            vk_commitment,
        );
        record.status = ConfidentialStatus::Active;
        record.namespace = KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.max_proof_bytes = 4096;
        record.key = Some(verifier_key);
        KagemushaVerifiedFoldRecordBundle {
            bundle: KagemushaVerifiedFoldBundle {
                chain_id,
                asset,
                steps: vec![lineage_step],
            },
            verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id: vk_id, record }],
        }
    }

    fn kagemusha_recursive_spend_lineage_pallas_open_envelope_archive(
        record_bundle: &KagemushaVerifiedFoldRecordBundle,
        label: u8,
    ) -> Vec<u8> {
        let mut envelopes = Vec::with_capacity(record_bundle.bundle.steps.len());
        for (hop_index, step) in record_bundle.bundle.steps.iter().enumerate() {
            let mut envelope = kagemusha_recursive_spend_pallas_open_envelope(
                label.wrapping_add(u8::try_from(hop_index).unwrap_or(u8::MAX)),
            );
            let proof_envelope: crate::zk::OpenVerifyEnvelope =
                norito::decode_from_bytes(&step.attachment.proof.bytes)
                    .expect("decode hop OpenVerifyEnvelope");
            envelope.vk_commitment = step.attachment.vk_commitment;
            envelope.public_inputs_schema_hash =
                Some(Hash::new(proof_envelope.public_inputs.as_slice()).into());
            envelopes.push(envelope);
        }
        to_bytes(&envelopes).expect("encode metadata-bound lineage Pallas envelope archive")
    }

    fn kagemusha_recursive_spend_pallas_open_envelope(
        label: u8,
    ) -> iroha_zkp_halo2::OpenVerifyEnvelope {
        iroha_zkp_halo2::OpenVerifyEnvelope {
            params: iroha_zkp_halo2::IpaParams {
                version: 1,
                curve_id: 1,
                n: 2,
                g: vec![[label; Hash::LENGTH], [label.wrapping_add(1); Hash::LENGTH]],
                h: vec![
                    [label.wrapping_add(2); Hash::LENGTH],
                    [label.wrapping_add(3); Hash::LENGTH],
                ],
                u: [label.wrapping_add(4); Hash::LENGTH],
            },
            public: iroha_zkp_halo2::PolyOpenPublic {
                version: 1,
                curve_id: 1,
                n: 2,
                z: [label.wrapping_add(5); Hash::LENGTH],
                t: [label.wrapping_add(6); Hash::LENGTH],
                p_g: [label.wrapping_add(7); Hash::LENGTH],
            },
            proof: iroha_zkp_halo2::IpaProofData {
                version: 1,
                l: vec![[label.wrapping_add(8); Hash::LENGTH]],
                r: vec![[label.wrapping_add(9); Hash::LENGTH]],
                a_final: [label.wrapping_add(10); Hash::LENGTH],
                b_final: [label.wrapping_add(11); Hash::LENGTH],
            },
            transcript_label: format!("kagemusha-recursive-lineage-{label}"),
            vk_commitment: Some([label.wrapping_add(12); Hash::LENGTH]),
            public_inputs_schema_hash: Some([label.wrapping_add(13); Hash::LENGTH]),
            domain_tag: Some([label.wrapping_add(14); Hash::LENGTH]),
        }
    }

    fn kagemusha_recursive_spend_pallas_open_envelope_archive(label: u8) -> Vec<u8> {
        let envelope = kagemusha_recursive_spend_pallas_open_envelope(label);
        to_bytes(&vec![envelope]).expect("encode Pallas envelope archive")
    }

    fn attach_recursive_spend_open_verify_envelope(
        bundle: &mut KagemushaRecursiveSpendBundleV1,
        vk_hash_label: &[u8],
    ) {
        let envelope = crate::zk::OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: bundle.recursive_proof.verifier_key_id.name.clone(),
            vk_hash: fixed_hash(vk_hash_label),
            public_inputs: KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_SCHEMA.to_vec(),
            proof_bytes: vec![0xA5; 64],
            aux: Vec::new(),
        };
        bundle.recursive_proof.proof.bytes =
            to_bytes(&envelope).expect("encode recursive spend OpenVerifyEnvelope");
    }

    fn kagemusha_recursive_spend_previous_proof_open_envelope_archive(
        previous_bundle: &KagemushaRecursiveSpendBundleV1,
        label: u8,
    ) -> Vec<u8> {
        let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> = norito::decode_from_bytes(
            &kagemusha_recursive_spend_pallas_open_envelope_archive(label),
        )
        .expect("decode previous proof Pallas envelope archive");
        let expected = kagemusha_recursive_previous_proof_open_envelope_metadata(previous_bundle)
            .expect("previous proof opening metadata");
        let envelope = envelopes
            .first_mut()
            .expect("fixture previous proof archive has one envelope");
        envelope.vk_commitment = expected.vk_commitment;
        envelope.public_inputs_schema_hash = expected.public_inputs_schema_hash;
        envelope.domain_tag = expected.domain_tag;
        to_bytes(&envelopes).expect("encode metadata-bound previous proof envelope archive")
    }

    fn kagemusha_recursive_spend_active_lineage_verifier_record() -> VerifyingKeyRecord {
        let verifier_key = VerifyingKeyBox::new("halo2/ipa".into(), vec![0x69; 96]);
        let mut record = VerifyingKeyRecord::new_with_owner(
            3,
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            Some("kagemusha-recursive-spend-lineage-test".to_owned()),
            KAGEMUSHA_VERIFIER_NAMESPACE,
            BackendTag::Halo2IpaPasta,
            "pallas",
            kagemusha_recursive_aggregation_proof_public_inputs_schema_hash(),
            kagemusha_verifying_key_commitment(&verifier_key),
        );
        record.status = ConfidentialStatus::Active;
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.max_proof_bytes = 4096;
        record.key = Some(verifier_key);
        record
    }

    fn assert_transition_profile_mutation_changes_or_rejects(
        mut profile: KagemushaRecursiveSpendTransitionProfileV1,
        original_digest: [u8; Hash::LENGTH],
        mutate: impl FnOnce(&mut KagemushaRecursiveSpendTransitionProfileV1),
    ) {
        mutate(&mut profile);
        if let Ok(mutated_digest) = profile.digest() {
            assert_ne!(mutated_digest, original_digest);
        }
    }

    #[test]
    fn kagemusha_record_curve_maps_only_supported_backends() {
        assert_eq!(
            kagemusha_record_curve_for_backend(BackendTag::Halo2IpaPasta),
            Some("pallas")
        );
        assert_eq!(
            kagemusha_record_curve_for_backend(BackendTag::Stark),
            Some("goldilocks")
        );
        assert_eq!(
            kagemusha_record_curve_for_backend(BackendTag::Halo2Bn254),
            None
        );
        assert_eq!(
            kagemusha_record_curve_for_backend(BackendTag::Groth16),
            None
        );
        assert_eq!(
            kagemusha_record_curve_for_backend(BackendTag::Unsupported),
            None
        );
    }

    #[test]
    fn offline_escrow_account_derivation_binds_chain_and_asset_definition() {
        let chain_id: ChainId = "offline-escrow-testnet".parse().expect("chain id");
        let other_chain_id: ChainId = "offline-escrow-mainnet".parse().expect("chain id");
        let domain_id = DomainId::try_new("offline", "universal").expect("domain id");
        let definition_id = AssetDefinitionId::new(
            domain_id.clone(),
            "usd".parse().expect("asset definition name"),
        );
        let other_definition_id =
            AssetDefinitionId::new(domain_id, "eur".parse().expect("asset definition name"));

        let escrow = offline_escrow_account_id(&chain_id, &definition_id);

        assert_eq!(
            escrow
                .signatory()
                .try_algorithm()
                .expect("escrow account public key algorithm"),
            Algorithm::Ed25519
        );
        assert_eq!(escrow, offline_escrow_account_id(&chain_id, &definition_id));
        assert_ne!(
            escrow,
            offline_escrow_account_id(&other_chain_id, &definition_id)
        );
        assert_ne!(
            escrow,
            offline_escrow_account_id(&chain_id, &other_definition_id)
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn offline_note_claims_and_public_inputs_bind_payload_fields() {
        let account_id = sample_account(0xD4, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id.clone());
        let note_public_key = sample_public_key(0xA8);
        let (algorithm, note_key) = note_public_key
            .try_to_bytes()
            .expect("fixture note public key must be well-formed");
        assert_eq!(algorithm, Algorithm::Ed25519);
        let certificate = OfflineNoteKeyCertificate {
            version: OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
            platform: "ios-appattest".to_owned(),
            key_id: "one-use-key".to_owned(),
            device_id: "device-1".to_owned(),
            account_id: account_id.clone(),
            public_key: note_key.to_vec(),
            assertion_scheme: "apple-appattest-counter".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: sample_signature(0xAB),
        };
        let proof = OfflineNoteRecursiveProof {
            verifier_key_id: VerifyingKeyId::new("halo2/ipa", "offline-note-recursive"),
            public_inputs_hash: Hash::new(b"offline-public-inputs"),
            proof: ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
        };
        let note_commitment = Hash::new(b"offline-note-issued-note");
        let issue = OfflineNoteIssue {
            note_commitment,
            key_certificate: certificate.clone(),
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
        };
        let mut redemption = OfflineNoteRedeem {
            source_note_commitment: note_commitment,
            input_nullifiers: vec![Hash::new(b"offline-note-nullifier")],
            sender_key_certificate: certificate.clone(),
            recipient: account_id,
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
            recursive_proof: proof.clone(),
        };

        let issue_claim = OfflineNoteIssuedClaim::from_issue(&issue)
            .expect("issue claim")
            .claim_hash()
            .expect("issue claim hash");
        let redeem_claim = OfflineNoteIssuedClaim::from_redemption(&redemption)
            .expect("redemption claim")
            .claim_hash()
            .expect("redemption claim hash");
        assert_eq!(issue_claim, redeem_claim);
        let redemption_inputs = redemption
            .public_inputs_hash()
            .expect("redemption public inputs hash");
        redemption.source_note_commitment = Hash::new(b"offline-note-other-note");
        assert_ne!(
            redemption_inputs,
            redemption
                .public_inputs_hash()
                .expect("changed redemption public inputs hash")
        );
        assert_ne!(
            issue_claim,
            OfflineNoteIssuedClaim::from_redemption(&redemption)
                .expect("changed redemption claim")
                .claim_hash()
                .expect("changed redemption claim hash")
        );

        let mut audit = OfflineNoteAuditBundle {
            token_id: Hash::new(b"offline-note-audit-token"),
            sender_key_certificate: certificate.clone(),
            input_nullifiers: vec![Hash::new(b"offline-note-audit-nullifier")],
            input_claims: vec![
                OfflineNoteIssuedClaim::from_issue(&issue).expect("audit input claim"),
            ],
            output_commitments: vec![Hash::new(b"offline-note-output-note")],
            output_claims: vec![OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-output-note"),
                key_certificate: certificate,
                asset,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: proof,
        };
        let audit_inputs = audit
            .public_inputs_hash()
            .expect("audit public inputs hash");
        audit.output_commitments = vec![Hash::new(b"offline-note-other-output")];
        assert_ne!(
            audit_inputs,
            audit
                .public_inputs_hash()
                .expect("changed audit public inputs hash")
        );
        audit.output_commitments = vec![Hash::new(b"offline-note-output-note")];
        audit.input_claims[0].amount = Numeric::new(9, 0);
        assert_ne!(
            audit_inputs,
            audit
                .public_inputs_hash()
                .expect("changed audit input claim public inputs hash")
        );
    }

    #[test]
    fn offline_note_wallet_derivations_bind_preimages() {
        let chain_id: ChainId = "offline-note-derivation-chain".parse().expect("chain id");
        let account_id = sample_account(0xD5, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id);
        let owner_key_certificate_payload_hash = Hash::new(b"offline-note-owner-cert");
        let note_secret = vec![0xA5; Hash::LENGTH];
        let commitment_preimage = OfflineNoteCommitmentPreimage {
            domain: OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            owner_key_certificate_payload_hash,
            asset: asset.clone(),
            amount: Numeric::new(42, 0),
            note_secret: note_secret.clone(),
            origin: OfflineNoteCommitmentOrigin::IssuerLoad(OfflineNoteIssuerLoadOrigin {
                operation_id: "operation-1".to_owned(),
                lineage_id: "lineage-1".to_owned(),
                local_revision: 7,
            }),
        };
        let commitment =
            derive_offline_note_note_commitment(&commitment_preimage).expect("commitment");

        assert_eq!(
            commitment,
            derive_offline_note_note_commitment(&commitment_preimage).expect("repeat commitment")
        );
        let mut changed_commitment_preimage = commitment_preimage.clone();
        changed_commitment_preimage.origin =
            OfflineNoteCommitmentOrigin::P2pOutput(OfflineNoteP2pOutputOrigin {
                payment_request_id: "payment-request-1".to_owned(),
                output_index: 0,
            });
        assert_ne!(
            commitment,
            derive_offline_note_note_commitment(&changed_commitment_preimage)
                .expect("changed origin commitment")
        );

        let nullifier_preimage = OfflineNoteInputNullifierPreimage {
            domain: OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            source_note_commitment: commitment,
            owner_key_certificate_payload_hash,
            note_secret: note_secret.clone(),
        };
        let nullifier =
            derive_offline_note_input_nullifier(&nullifier_preimage).expect("nullifier");
        assert_eq!(
            nullifier,
            derive_offline_note_input_nullifier(&nullifier_preimage).expect("repeat nullifier")
        );
        let mut changed_nullifier_preimage = nullifier_preimage.clone();
        changed_nullifier_preimage.note_secret[0] ^= 0x01;
        assert_ne!(
            nullifier,
            derive_offline_note_input_nullifier(&changed_nullifier_preimage)
                .expect("changed secret nullifier")
        );

        let token_preimage = OfflineNotePaymentTokenIdPreimage {
            domain: OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN.to_owned(),
            chain_id,
            payment_request_id: "payment-request-fixture".to_owned(),
            created_at_ms: 1_700_000_001_000,
            token_nonce: vec![0xC6; Hash::LENGTH],
            sender_key_certificate_payload_hash: owner_key_certificate_payload_hash,
            input_nullifiers: vec![nullifier],
            output_commitments: vec![commitment],
        };
        let token_id =
            derive_offline_note_payment_token_id(&token_preimage).expect("payment token id");
        assert_eq!(
            token_id,
            derive_offline_note_payment_token_id(&token_preimage).expect("repeat payment token id")
        );
        let mut changed_token_preimage = token_preimage.clone();
        changed_token_preimage.token_nonce[0] ^= 0x01;
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_token_preimage)
                .expect("changed nonce payment token id")
        );
        let mut changed_request_token_preimage = token_preimage.clone();
        changed_request_token_preimage.payment_request_id = "payment-request-other".to_owned();
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_request_token_preimage)
                .expect("changed request payment token id")
        );
        let mut changed_created_at_token_preimage = token_preimage.clone();
        changed_created_at_token_preimage.created_at_ms += 1;
        assert_ne!(
            token_id,
            derive_offline_note_payment_token_id(&changed_created_at_token_preimage)
                .expect("changed created_at payment token id")
        );
    }

    #[test]
    fn offline_note_wallet_derivations_reject_short_random_material() {
        let chain_id: ChainId = "offline-note-derivation-chain".parse().expect("chain id");
        let account_id = sample_account(0xD6, "offline");
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "usd".parse().expect("asset name"),
        );
        let asset = AssetId::new(definition, account_id);
        let owner_key_certificate_payload_hash = Hash::new(b"offline-note-owner-cert");
        let commitment_preimage = OfflineNoteCommitmentPreimage {
            domain: OFFLINE_NOTE_NOTE_COMMITMENT_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            owner_key_certificate_payload_hash,
            asset,
            amount: Numeric::new(42, 0),
            note_secret: vec![0xA5; Hash::LENGTH - 1],
            origin: OfflineNoteCommitmentOrigin::IssuerLoad(OfflineNoteIssuerLoadOrigin {
                operation_id: "operation-1".to_owned(),
                lineage_id: "lineage-1".to_owned(),
                local_revision: 7,
            }),
        };
        assert!(matches!(
            derive_offline_note_note_commitment(&commitment_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "note_secret",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));

        let nullifier_preimage = OfflineNoteInputNullifierPreimage {
            domain: OFFLINE_NOTE_INPUT_NULLIFIER_DOMAIN.to_owned(),
            chain_id: chain_id.clone(),
            source_note_commitment: Hash::new(b"source-note"),
            owner_key_certificate_payload_hash,
            note_secret: vec![0xB6; Hash::LENGTH - 1],
        };
        assert!(matches!(
            derive_offline_note_input_nullifier(&nullifier_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "note_secret",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));

        let token_preimage = OfflineNotePaymentTokenIdPreimage {
            domain: OFFLINE_NOTE_PAYMENT_TOKEN_ID_DOMAIN.to_owned(),
            chain_id,
            payment_request_id: "payment-request-fixture".to_owned(),
            created_at_ms: 1_700_000_001_000,
            token_nonce: vec![0xC7; Hash::LENGTH - 1],
            sender_key_certificate_payload_hash: owner_key_certificate_payload_hash,
            input_nullifiers: vec![Hash::new(b"nullifier")],
            output_commitments: vec![Hash::new(b"commitment")],
        };
        assert!(matches!(
            derive_offline_note_payment_token_id(&token_preimage),
            Err(OfflineNoteDerivationError::InvalidRandomBytesLength {
                field: "token_nonce",
                expected: Hash::LENGTH,
                actual
            }) if actual == Hash::LENGTH - 1
        ));
    }

    #[test]
    fn kagemusha_proof_public_inputs_statement_digest_binds_all_statement_fields() {
        let statement = KagemushaProofPublicInputsStatement {
            proof_backend: "halo2/ipa".to_owned(),
            envelope_backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:kagemusha-hop-fixture".to_owned(),
            vk_hash: fixed_hash(b"kagemusha-hop-vk"),
            public_inputs_schema: b"kagemusha-hop-public-schema-v1".to_vec(),
            envelope_aux: Vec::new(),
            instance_columns: vec![vec![[0x11; Hash::LENGTH]], vec![[0x22; Hash::LENGTH]]],
        };
        let digest =
            kagemusha_proof_public_inputs_statement_digest(&statement).expect("statement digest");
        assert_ne!(digest, [0u8; Hash::LENGTH]);
        assert_eq!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&statement)
                .expect("repeat statement digest")
        );

        let mut changed_backend = statement.clone();
        changed_backend.proof_backend = "stark/fri".to_owned();
        changed_backend.envelope_backend = BackendTag::Stark;
        assert_ne!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&changed_backend)
                .expect("changed backend digest")
        );

        let mut changed_vk = statement.clone();
        changed_vk.vk_hash = fixed_hash(b"kagemusha-hop-other-vk");
        assert_ne!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&changed_vk)
                .expect("changed verifier-key digest")
        );

        let mut changed_schema = statement.clone();
        changed_schema.public_inputs_schema.push(0xA5);
        assert_ne!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&changed_schema)
                .expect("changed schema digest")
        );

        let mut changed_instance = statement;
        changed_instance.instance_columns[1][0][0] ^= 0x01;
        assert_ne!(
            digest,
            kagemusha_proof_public_inputs_statement_digest(&changed_instance)
                .expect("changed instance digest")
        );
    }

    #[test]
    fn kagemusha_proof_public_inputs_statement_digest_rejects_noncanonical_metadata() {
        let mut statement = KagemushaProofPublicInputsStatement {
            proof_backend: "halo2/ipa".to_owned(),
            envelope_backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:kagemusha-hop-fixture".to_owned(),
            vk_hash: fixed_hash(b"kagemusha-hop-vk"),
            public_inputs_schema: b"kagemusha-hop-public-schema-v1".to_vec(),
            envelope_aux: b"kagemusha-hop-aux".to_vec(),
            instance_columns: vec![vec![[0x11; Hash::LENGTH]]],
        };
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::NonCanonicalProofStatementAuxiliaryBytes { actual })
                if actual == b"kagemusha-hop-aux".len()
        ));

        statement.envelope_aux.clear();
        statement.vk_hash = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::ZeroProofStatementVerifierKeyHash)
        ));

        statement.vk_hash = fixed_hash(b"kagemusha-hop-vk");
        statement.proof_backend = "halo2/ipa".to_owned();
        statement.envelope_backend = BackendTag::Stark;
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::ProofStatementBackendTagMismatch {
                proof_backend,
                envelope_backend: BackendTag::Stark
            }) if proof_backend == "halo2/ipa"
        ));

        statement.proof_backend = "halo2/kzg".to_owned();
        statement.envelope_backend = BackendTag::Halo2IpaPasta;
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));

        statement.proof_backend = "halo2/ipa".to_owned();
        statement.envelope_backend = BackendTag::Halo2IpaPasta;
        statement.circuit_id.clear();
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::EmptyProofStatementCircuitId)
        ));

        statement.circuit_id = "halo2/ipa:kagemusha-hop-fixture".to_owned();
        statement.public_inputs_schema.clear();
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::EmptyProofStatementPublicInputsSchema)
        ));

        statement.public_inputs_schema = b"kagemusha-hop-public-schema-v1".to_vec();
        statement.instance_columns.clear();
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::EmptyProofStatementInstanceColumns)
        ));

        statement.instance_columns = vec![Vec::new()];
        assert!(matches!(
            kagemusha_proof_public_inputs_statement_digest(&statement),
            Err(KagemushaFoldError::EmptyProofStatementInstanceColumn { column_index: 0 })
        ));
    }

    #[test]
    fn kagemusha_verifier_key_poseidon_digest_binds_backend_and_bytes() {
        let digest = kagemusha_verifier_key_poseidon_digest("halo2/ipa", b"kagemusha-hop-vk")
            .expect("verifier-key digest");
        assert_ne!(digest, [0u8; Hash::LENGTH]);
        assert_eq!(
            digest,
            kagemusha_verifier_key_poseidon_digest("halo2/ipa", b"kagemusha-hop-vk")
                .expect("repeat verifier-key digest")
        );
        assert_ne!(
            digest,
            kagemusha_verifier_key_poseidon_digest("stark/fri", b"kagemusha-hop-vk")
                .expect("backend-mutated verifier-key digest")
        );
        assert_ne!(
            digest,
            kagemusha_verifier_key_poseidon_digest("halo2/ipa", b"kagemusha-other-vk")
                .expect("bytes-mutated verifier-key digest")
        );
        assert!(
            kagemusha_verifier_key_poseidon_digest(
                "stark/fri/sha256_goldilocks.v1",
                b"kagemusha-hop-vk"
            )
            .is_ok()
        );
        assert!(
            kagemusha_verifier_key_poseidon_digest(
                "stark/fri/sha256-goldilocks",
                b"kagemusha-hop-vk"
            )
            .is_ok()
        );
        assert!(
            kagemusha_verifier_key_poseidon_digest(
                "stark/fri/poseidon2-goldilocks",
                b"kagemusha-hop-vk"
            )
            .is_ok()
        );
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("halo2/kzg", b"kagemusha-hop-vk"),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));
        for backend in [
            "kzg",
            "KZG",
            " kzg ",
            "kzg/ceremony-v1",
            "KZG/ceremony-v1",
            "bn254",
            "BN254",
            "\tBN254\n",
            "bn256",
            "bls12_381",
            "halo2/ipa:kzg",
            "halo2/ipa:KZG",
            "halo2/ipa: KZG",
            "stark/fri:kzg",
            "stark/fri:KZG",
            "stark/fri: KZG",
            "stark/fri/kzg",
            "stark/fri/KZG",
            "stark/fri/ KZG",
            "stark/fri/prod;kzg",
            "stark/fri/prod,kzg",
            "stark/fri/prod+kzg",
            "stark/fri/prod.kzg",
            "stark/fri/prod-k-z-g",
            "stark/fri/prod(kzg)",
            "stark/fri/bn254",
            "stark/fri/prod;bn254",
            "stark/fri/prod-bn-254",
            "stark/fri/prod+bn256",
            "stark/fri/prod-bn-256",
            "stark/fri/bls12_381",
            "stark/fri/prod-bls12-381",
            "stark/fri/prod.bls12_381",
            "stark/fri/prod-b.l.s.12.381",
            "srs",
            "SRS",
            "crs",
            "ptau",
            "powersoftau",
            "powers-of-tau",
            "trusted-setup",
            "structured-reference-string",
            "universal-srs",
            "halo2/ipa:universal-srs",
            "stark/fri/prod-srs",
            "stark/fri/prod-s-r-s",
            "stark/fri/prod.crs",
            "stark/fri/prod-ptau",
            "stark/fri/prod-powers-of-tau",
            "stark/fri/prod-ceremony",
            "stark/fri/structured-reference-string",
            "halo2/ipa;groth16",
            "halo2/ipa:groth-16",
        ] {
            assert!(matches!(
                kagemusha_verifier_key_poseidon_digest(backend, b"kagemusha-hop-vk"),
                Err(KagemushaFoldError::UnsupportedProofBackend { backend: rejected })
                    if rejected == backend
            ));
        }
        for backend in [
            "stark/fri/d-e-b-u-g",
            "stark/fri/m-o-c-k",
            "halo2/ipa:d-e-b-u-g",
            "halo2/ipa:m-o-c-k",
        ] {
            assert!(matches!(
                kagemusha_verifier_key_poseidon_digest(backend, b"kagemusha-hop-vk"),
                Err(KagemushaFoldError::UnsupportedProofBackend { backend: rejected })
                    if rejected == backend
            ));
        }
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("stark/fri/", b"kagemusha-hop-vk"),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/"
        ));
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("stark/fri/ ", b"kagemusha-hop-vk"),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/ "
        ));
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("stark/fri/\t\n", b"kagemusha-hop-vk"),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/\t\n"
        ));
        for backend in [
            "stark/fri/latest",
            "stark/fri/random-profile",
            "stark/fri/sha512-goldilocks",
            "stark/fri/sha256_goldilocks.v2",
            "stark/fri/audit-proof-v1",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            "stark/fri/ sha256-goldilocks",
            "stark/fri/sha256-goldilocks ",
            "stark/fri/sha256 goldilocks",
            "halo2/unknown-native-v1",
            "halo2/ipa:tiny-add-public",
            "halo2/pasta/tiny-add",
            "halo2/pasta/ivm-execution-v2",
            "halo2/pasta/unknown-native-v1",
            "stark/fri/prod;foo",
            "stark/fri/prod,foo",
            "stark/fri/prod+foo",
            "stark/fri/prod/foo",
            "stark/fri/prod(foo)",
            "stark/fri/Δ",
        ] {
            assert!(matches!(
                kagemusha_verifier_key_poseidon_digest(backend, b"kagemusha-hop-vk"),
                Err(KagemushaFoldError::UnsupportedProofBackend { backend: rejected })
                    if rejected == backend
            ));
        }
        for backend in [
            "debug-proof",
            "Debug-Proof",
            "stark/fri/debug",
            "stark/fri/Debug",
            "stark/fri/debug-proof",
            "mock-proof",
            "Mock-Proof",
            "stark/fri/mock",
            "stark/fri/Mock",
            "stark/fri/mock-proof",
            "halo2/ipa:Mock-Proof",
        ] {
            assert!(matches!(
                kagemusha_verifier_key_poseidon_digest(backend, b"kagemusha-hop-vk"),
                Err(KagemushaFoldError::UnsupportedProofBackend { backend: rejected })
                    if rejected == backend
            ));
        }
        assert!(matches!(
            kagemusha_verifier_key_poseidon_digest("halo2/ipa", &[]),
            Err(KagemushaFoldError::EmptyVerifierKeyBytes { backend })
                if backend == "halo2/ipa"
        ));
    }

    #[test]
    fn kagemusha_aggregation_mode_helpers_keep_recursive_mode_out_of_legacy_path() {
        assert!(is_supported_kagemusha_aggregation_mode(
            KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        ));
        assert!(!is_supported_kagemusha_aggregation_mode(
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));
        assert!(
            unsupported_kagemusha_aggregation_mode_reason(
                KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
            )
            .contains("requires ABI-7 recursive compact-token admission")
        );
        assert!(
            unsupported_kagemusha_aggregation_mode_reason(
                KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
            )
            .contains("legacy checked pre-fold path does not accept mode 2")
        );
        assert!(
            !unsupported_kagemusha_aggregation_mode_reason(
                KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
            )
            .contains("no recursive verifier")
        );
        assert!(
            unsupported_kagemusha_aggregation_mode_reason(0xFFFF)
                .contains("unsupported or unknown")
        );
        assert_eq!(
            preferred_kagemusha_offline_spend_mode_for_capabilities(true, true),
            KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
        );
        assert_eq!(
            preferred_kagemusha_offline_spend_mode_for_capabilities(true, false),
            KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
        );
        assert_eq!(
            preferred_kagemusha_offline_spend_mode_for_capabilities(false, true),
            KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
        );
        assert_eq!(
            preferred_kagemusha_offline_spend_mode_for_capabilities(false, false),
            KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
        );
        assert_eq!(
            preferred_kagemusha_offline_spend_mode(true),
            KAGEMUSHA_OFFLINE_SPEND_MODE_RECURSIVE_V1
        );
        assert_eq!(
            preferred_kagemusha_offline_spend_mode(false),
            KAGEMUSHA_OFFLINE_SPEND_MODE_CHECKED_PREFOLD_V1
        );
        assert_eq!(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1, 64,
            "witnessless Reserved-lineage redemption keeps the hard compact-token hop cap"
        );
        assert!(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1,
            "raising the Reserved-lineage hop cap requires accumulator-transition constraints"
        );
        assert_eq!(
            KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1, 1,
            "each Reserved-lineage append binds exactly one previous recursive proof opening envelope"
        );
        assert!(can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1
        ));
        assert!(can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            1
        ));
        assert!(can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            2
        ));
        assert!(can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
        ));
        assert!(!can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            0
        ));
        assert!(!can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 + 1
        ));
        assert!(!can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            u32::MAX
        ));
        assert!(!can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            1
        ));
        assert!(!can_redeem_kagemusha_recursive_spend_witnessless(
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            u32::MAX
        ));
        assert!(!can_redeem_kagemusha_recursive_spend_witnessless(
            "unknown-kagemusha-recursive-spend-circuit",
            1
        ));
        assert!(!can_redeem_kagemusha_recursive_spend_witnessless(
            "unknown-kagemusha-recursive-spend-circuit",
            u32::MAX
        ));
        assert!(
            !requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1
            )
        );
        assert!(
            requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1
            )
        );
        assert!(
            !requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                2
            )
        );
        assert!(
            requires_kagemusha_recursive_spend_lineage_witness_for_redeem(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                u32::MAX
            )
        );
        assert!(!can_append_kagemusha_recursive_spend_lineage_witnessless(0));
        assert!(
            can_append_kagemusha_recursive_spend_lineage_witnessless(1),
            "witnessless Reserved-lineage append is available after one-hop init"
        );
        assert!(can_append_kagemusha_recursive_spend_lineage_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 - 1
        ));
        assert!(!can_append_kagemusha_recursive_spend_lineage_witnessless(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
        ));
        assert!(!can_append_kagemusha_recursive_spend_lineage_witnessless(
            u32::MAX
        ));
        assert!(
            requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1
            )
        );
        assert_eq!(
            normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(""),
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        );
        assert_eq!(
            normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            ),
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        );
        assert_eq!(
            normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            ),
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        );
        assert_eq!(
            normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            ),
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        );
        assert_eq!(
            normalize_kagemusha_recursive_spend_append_output_proof_circuit_id(
                "unknown-kagemusha-recursive-spend-circuit"
            ),
            "unknown-kagemusha-recursive-spend-circuit"
        );
        assert!(is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(""));
        assert!(
            is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            !is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            !is_supported_kagemusha_recursive_spend_append_output_proof_circuit_id(
                "unknown-kagemusha-recursive-spend-circuit"
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            !is_supported_kagemusha_recursive_spend_previous_proof_circuit_id(
                "unknown-kagemusha-recursive-spend-circuit"
            )
        );
        assert!(
            !requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            !requires_kagemusha_recursive_spend_previous_lineage_verifier_record_for_append(
                "unknown-kagemusha-recursive-spend-circuit"
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_append_proof_transition(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_append_proof_transition(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                ""
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_append_proof_transition(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            is_supported_kagemusha_recursive_spend_append_proof_transition(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            ),
            "Reserved-lineage to Reserved-lineage is the enabled structural append transition"
        );
        assert!(
            !is_supported_kagemusha_recursive_spend_append_proof_transition(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
            ),
            "semantic previous proofs cannot be upgraded into Reserved-lineage output"
        );
        assert!(
            !is_supported_kagemusha_recursive_spend_append_proof_transition(
                "unknown-kagemusha-recursive-spend-circuit",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            )
        );
        assert!(
            !is_supported_kagemusha_recursive_spend_append_proof_transition(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                "unknown-kagemusha-recursive-spend-circuit"
            )
        );
        assert_eq!(
            preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(1),
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            "real appends prefer Reserved-lineage output inside the witnessless cap"
        );
        assert_eq!(
            preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
                u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS - 1).expect("hop cap fits u32")
            ),
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        );
        assert_eq!(
            preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
            ),
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            "preferred append selector falls back at the witnessless hop cap"
        );
        assert_eq!(
            preferred_kagemusha_recursive_spend_append_output_proof_circuit_id(0),
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        );
        assert!(
            can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1
            )
        );
        assert!(can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id("", 1));
        assert!(
            can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS - 1).expect("hop cap fits u32")
            )
        );
        assert!(
            !can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                0
            )
        );
        assert!(
            !can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS).expect("hop cap fits u32")
            )
        );
        assert!(
            can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1
            ),
            "one-hop previous Reserved-lineage can prove the two-hop append output"
        );
        assert!(
            can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                1
            ),
            "append-specific Reserved-lineage id can prove the two-hop append output"
        );
        assert!(
            can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 - 1
            )
        );
        assert!(
            !can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
            ),
            "append output beyond the witnessless cap must reject"
        );
        assert!(
            !can_prove_kagemusha_recursive_spend_append_output_proof_circuit_id(
                "unknown-kagemusha-recursive-spend-circuit",
                1
            )
        );
        assert!(
            can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1
            )
        );
        assert!(
            can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1
            )
        );
        assert!(
            !can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                "unknown-kagemusha-recursive-spend-circuit",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1
            )
        );
        assert!(
            !can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1
            ),
            "semantic previous proofs cannot select Reserved-lineage output"
        );
        assert!(
            can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                1
            ),
            "Reserved-lineage previous proofs can select Reserved-lineage output inside the cap"
        );
        assert!(
            can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                1
            ),
            "one-hop Reserved-lineage proofs can select the append-specific Reserved-lineage output"
        );
        assert!(
            can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                2
            ),
            "append Reserved-lineage proofs can keep selecting append-specific Reserved-lineage output"
        );
        assert!(
            !can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                "unknown-kagemusha-recursive-spend-circuit",
                1
            )
        );
        assert!(
            !can_select_kagemusha_recursive_spend_append_output_proof_circuit_id(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                0
            )
        );
        assert!(
            requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                64
            )
        );
        assert!(
            !requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                0
            )
        );
        assert!(
            !requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                1
            )
        );
        assert!(
            !requires_kagemusha_recursive_spend_previous_proof_open_envelopes_for_append("", 1)
        );
    }

    #[test]
    fn kagemusha_poseidon_aggregation_transcript_digest_binds_statement_fields() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let statement = KagemushaPoseidonAggregationTranscriptStatement {
            aggregation_mode: KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1,
            chain_id,
            asset,
            initial_root: root0,
            final_root: root1,
            hop_count: 1,
            steps: vec![KagemushaPoseidonAggregationStepStatement {
                hop_index: 0,
                root_before: root0,
                input_nullifiers: vec![[0x11; Hash::LENGTH]],
                output_commitments: vec![[0x22; Hash::LENGTH]],
                root_after: root1,
                proof_hash: Hash::new(b"kagemusha-hop-proof"),
                proof_public_inputs_digest: fixed_hash(b"kagemusha-hop-public-inputs"),
                verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-fixture"),
                verifier_key_commitment: fixed_hash(b"kagemusha-hop-vk"),
                verifier_key_poseidon_digest: kagemusha_verifier_key_poseidon_digest(
                    "halo2/ipa",
                    b"kagemusha-hop-vk",
                )
                .expect("verifier-key digest"),
            }],
        };
        let digest = kagemusha_poseidon_aggregation_transcript_digest(&statement)
            .expect("aggregation transcript digest");
        assert_ne!(digest, [0u8; Hash::LENGTH]);
        assert_eq!(
            digest,
            kagemusha_poseidon_aggregation_transcript_digest(&statement)
                .expect("repeat aggregation transcript digest")
        );

        let mut changed_mode = statement.clone();
        changed_mode.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
        assert_ne!(
            digest,
            kagemusha_poseidon_aggregation_transcript_digest(&changed_mode)
                .expect("reserved recursive mode still has a separated transcript digest")
        );
        let mut reserved_public_inputs =
            kagemusha_folded_public_inputs_from_aggregation_statement(&changed_mode)
                .expect("reserved recursive transcript projection");
        reserved_public_inputs.aggregation_mode =
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
        assert!(matches!(
            reserved_public_inputs.validate_supported_context(),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));
        let mut unknown_mode = changed_mode.clone();
        unknown_mode.aggregation_mode = 0xFFFF;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&unknown_mode),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual: 0xFFFF, .. })
        ));

        let mut changed_hop = statement;
        changed_hop.steps[0].verifier_key_poseidon_digest[0] ^= 0x01;
        assert_ne!(
            digest,
            kagemusha_poseidon_aggregation_transcript_digest(&changed_hop)
                .expect("changed hop statement digest")
        );
    }

    #[test]
    fn kagemusha_recursive_aggregation_evidence_binds_batch_preflight_digest() {
        let chain_id: ChainId = "kagemusha-recursive-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive");
        let root0 = fixed_hash(b"kagemusha-recursive-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-proof-hop-1"),
        ];
        let opening_len = 4;
        let params_fingerprint = fixed_hash(b"recursive-pallas-params");
        let schedule_digest = fixed_hash(b"recursive-fixed-window-schedule");
        let manifest_digest = fixed_hash(b"recursive-fixed-window-manifest");
        let base_digest = fixed_hash(b"recursive-fixed-window-bases");
        let batch_digest = fixed_hash(b"recursive-pallas-witness-batch");
        let evidence = kagemusha_recursive_aggregation_evidence_from_steps(
            &chain_id,
            &asset,
            &steps,
            opening_len,
            params_fingerprint,
            schedule_digest,
            manifest_digest,
            base_digest,
            batch_digest,
        )
        .expect("recursive aggregation evidence");
        assert_eq!(
            evidence.aggregation_statement.aggregation_mode,
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        );
        assert_eq!(evidence.verifier_witness_count, 2);
        assert_eq!(
            evidence.verifier_witness_profile,
            KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1
        );
        assert_eq!(evidence.verifier_opening_len, opening_len);
        assert_eq!(evidence.verifier_params_fingerprint, params_fingerprint);
        assert_eq!(evidence.fixed_window_table_schedule_digest, schedule_digest);
        assert_eq!(
            evidence.fixed_window_shared_table_manifest_digest,
            manifest_digest
        );
        assert_eq!(evidence.fixed_window_table_base_digest, base_digest);
        assert_eq!(evidence.verifier_witness_batch_digest, batch_digest);
        let transcript_digest =
            kagemusha_poseidon_aggregation_transcript_digest(&evidence.aggregation_statement)
                .expect("reserved recursive aggregation transcript digest");
        assert_ne!(transcript_digest, [0u8; Hash::LENGTH]);

        let digest = kagemusha_recursive_aggregation_evidence_digest(&evidence)
            .expect("recursive aggregation evidence digest");
        assert_ne!(digest, [0u8; Hash::LENGTH]);
        assert_eq!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&evidence)
                .expect("repeat recursive aggregation evidence digest")
        );
        let bytes = to_bytes(&evidence).expect("encode recursive aggregation evidence");
        let decoded: KagemushaRecursiveAggregationEvidence =
            norito::decode_from_bytes(&bytes).expect("decode recursive aggregation evidence");
        assert_eq!(decoded, evidence);
        assert_eq!(
            decoded.verifier_witness_profile,
            KAGEMUSHA_RECURSIVE_VERIFIER_WITNESS_PROFILE_V1
        );
        assert_eq!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&decoded)
                .expect("decoded recursive aggregation evidence digest")
        );

        let mut changed_batch = evidence.clone();
        changed_batch.verifier_witness_batch_digest =
            fixed_hash(b"recursive-pallas-witness-batch-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_batch)
                .expect("changed batch digest evidence")
        );

        let mut changed_opening_len = evidence.clone();
        changed_opening_len.verifier_opening_len = 8;
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_opening_len)
                .expect("changed opening length evidence")
        );

        let mut changed_schedule = evidence.clone();
        changed_schedule.fixed_window_table_schedule_digest =
            fixed_hash(b"recursive-fixed-window-schedule-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_schedule)
                .expect("changed schedule digest evidence")
        );

        let mut changed_manifest = evidence.clone();
        changed_manifest.fixed_window_shared_table_manifest_digest =
            fixed_hash(b"recursive-fixed-window-manifest-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_manifest)
                .expect("changed shared-table manifest digest evidence")
        );

        let mut changed_base = evidence.clone();
        changed_base.fixed_window_table_base_digest =
            fixed_hash(b"recursive-fixed-window-bases-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_base)
                .expect("changed table-base digest evidence")
        );

        let mut changed_profile = evidence.clone();
        changed_profile.verifier_witness_profile =
            "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-unsafe".to_owned();
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_digest(&changed_profile),
            Err(
                KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile { actual, .. }
            ) if actual == "pallas-ipa-transparent-v1/vesta-recursive-fixed-window-unsafe"
        ));

        let mut changed_params = evidence;
        changed_params.verifier_params_fingerprint = fixed_hash(b"recursive-pallas-params-other");
        assert_ne!(
            digest,
            kagemusha_recursive_aggregation_evidence_digest(&changed_params)
                .expect("changed params evidence")
        );
    }

    #[test]
    fn kagemusha_recursive_evidence_validates_reserved_folded_projection_without_opening_admission()
    {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let public_inputs = kagemusha_folded_public_inputs_from_aggregation_statement(
            &evidence.aggregation_statement,
        )
        .expect("reserved recursive folded projection");

        assert_eq!(public_inputs.domain, KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN);
        assert_eq!(
            public_inputs.aggregation_mode,
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        );
        kagemusha_validate_recursive_evidence_folded_public_input_projection(
            &public_inputs,
            &evidence,
        )
        .expect("reserved recursive evidence must bind its folded projection");
        public_inputs
            .validate_recursive_compact_context()
            .expect("reserved recursive folded projection has a valid compact context");
        let recursive_proof_public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive proof public inputs");
        let recursive_proof_public_inputs_hash = recursive_proof_public_inputs
            .public_inputs_hash()
            .expect("recursive proof public-input hash");
        let recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND,
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs: recursive_proof_public_inputs.clone(),
            public_inputs_hash: recursive_proof_public_inputs_hash,
            proof: ProofBox::new(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_BACKEND.to_owned(),
                vec![0xA5],
            ),
        };
        assert_eq!(
            recursive_proof_public_inputs.folded_public_inputs_hash,
            hash_bytes_from_hash(
                public_inputs
                    .public_inputs_hash()
                    .expect("reserved recursive folded public-input hash")
            )
        );
        kagemusha_validate_recursive_proof_folded_public_input_projection(
            &public_inputs,
            &recursive_proof,
        )
        .expect("recursive proof public inputs must bind the folded public projection");
        let token = KagemushaCompactPaymentToken::from_recursive_compact_projection(
            public_inputs.clone(),
            recursive_proof.clone(),
        )
        .expect("recursive compact token projection");
        assert_eq!(token.public_inputs, public_inputs);
        assert_eq!(
            token.folded_proof.verifier_key_id,
            recursive_proof.verifier_key_id
        );
        assert_eq!(
            token.folded_proof.public_inputs_hash,
            public_inputs
                .public_inputs_hash()
                .expect("recursive compact token public-input hash")
        );
        assert_eq!(token.folded_proof.proof, recursive_proof.proof);
        assert!(matches!(
            public_inputs.validate_supported_context(),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));

        let mut checked_mode = public_inputs.clone();
        checked_mode.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1;
        assert!(matches!(
            kagemusha_validate_recursive_evidence_folded_public_input_projection(
                &checked_mode,
                &evidence,
            ),
            Err(KagemushaFoldError::UnsupportedAggregationMode { expected, actual, .. })
                if expected == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
                    && actual == KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        ));
        assert!(matches!(
            checked_mode.validate_recursive_compact_context(),
            Err(KagemushaFoldError::UnsupportedAggregationMode { expected, actual, .. })
                if expected == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
                    && actual == KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        ));

        let mut forged_proof_inputs = recursive_proof_public_inputs.clone();
        forged_proof_inputs.folded_public_inputs_hash =
            fixed_hash(b"recursive-compact-forged-public-input-hash");
        let mut forged_recursive_proof = recursive_proof.clone();
        forged_recursive_proof.public_inputs = forged_proof_inputs.clone();
        forged_recursive_proof.public_inputs_hash = forged_proof_inputs
            .public_inputs_hash()
            .expect("forged recursive proof public-input hash");
        assert!(matches!(
            kagemusha_validate_recursive_proof_folded_public_input_projection(
                &public_inputs,
                &forged_recursive_proof,
            ),
            Err(KagemushaFoldError::PublicInputHashMismatch { .. })
        ));
        assert!(matches!(
            KagemushaCompactPaymentToken::from_recursive_compact_projection(
                public_inputs.clone(),
                forged_recursive_proof.clone(),
            ),
            Err(KagemushaFoldError::PublicInputHashMismatch { .. })
        ));
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_proof_public_input_parity(
                &recursive_proof_public_inputs,
                &forged_proof_inputs,
            ),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "folded_public_inputs_hash"
                }
            )
        ));

        let mut zero_proof_inputs = recursive_proof_public_inputs.clone();
        zero_proof_inputs.folded_public_inputs_hash = [0u8; Hash::LENGTH];
        assert!(matches!(
            zero_proof_inputs.validate_context(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "folded_public_inputs_hash"
                }
            )
        ));

        let mut forged_domain = public_inputs.clone();
        forged_domain.domain = "iroha:kagemusha:recursive-compact-forged-domain".to_owned();
        assert!(matches!(
            kagemusha_validate_recursive_evidence_folded_public_input_projection(
                &forged_domain,
                &evidence,
            ),
            Err(KagemushaFoldError::InvalidPublicInputDomain { .. })
        ));

        let mut forged_chain = public_inputs.clone();
        forged_chain.chain_id = "kagemusha-recursive-forged-chain"
            .parse()
            .expect("chain id");
        assert!(matches!(
            kagemusha_validate_recursive_evidence_folded_public_input_projection(
                &forged_chain,
                &evidence,
            ),
            Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch { field: "chain_id" })
        ));

        let mut forged_digest = public_inputs.clone();
        forged_digest.aggregation_transcript_digest =
            fixed_hash(b"recursive-compact-forged-aggregation-digest");
        assert!(matches!(
            kagemusha_validate_recursive_evidence_folded_public_input_projection(
                &forged_digest,
                &evidence,
            ),
            Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch {
                field: "aggregation_transcript_digest"
            })
        ));

        let mut forged_evidence = evidence;
        forged_evidence.verifier_witness_count += 1;
        assert!(matches!(
            kagemusha_validate_recursive_evidence_folded_public_input_projection(
                &public_inputs,
                &forged_evidence,
            ),
            Err(KagemushaFoldError::RecursiveAggregationWitnessCountMismatch { .. })
        ));
    }

    #[test]
    fn kagemusha_recursive_public_inputs_reject_zero_required_digests() {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive proof public inputs");
        let zero_digest_cases: [(
            &'static str,
            fn(&mut KagemushaRecursiveAggregationProofPublicInputs),
        ); 8] = [
            (
                "evidence_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.evidence_digest = [0u8; Hash::LENGTH];
                },
            ),
            (
                "folded_public_inputs_hash",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.folded_public_inputs_hash = [0u8; Hash::LENGTH];
                },
            ),
            (
                "aggregation_transcript_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.aggregation_transcript_digest = [0u8; Hash::LENGTH];
                },
            ),
            (
                "verifier_params_fingerprint",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.verifier_params_fingerprint = [0u8; Hash::LENGTH];
                },
            ),
            (
                "fixed_window_table_schedule_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.fixed_window_table_schedule_digest = [0u8; Hash::LENGTH];
                },
            ),
            (
                "fixed_window_shared_table_manifest_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.fixed_window_shared_table_manifest_digest = [0u8; Hash::LENGTH];
                },
            ),
            (
                "fixed_window_table_base_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.fixed_window_table_base_digest = [0u8; Hash::LENGTH];
                },
            ),
            (
                "verifier_witness_batch_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.verifier_witness_batch_digest = [0u8; Hash::LENGTH];
                },
            ),
        ];

        for (expected_field, zero_field) in zero_digest_cases {
            let mut changed_public_inputs = public_inputs.clone();
            zero_field(&mut changed_public_inputs);
            let err = changed_public_inputs
                .validate_context()
                .expect_err("zero required digest must be rejected");
            assert!(
                matches!(
                    err,
                    KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch { field }
                    if field == expected_field
                ),
                "unexpected zero-digest error for {expected_field}: {err:?}"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_aggregation_evidence_rejects_noncanonical_fields() {
        let chain_id: ChainId = "kagemusha-recursive-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive");
        let root0 = fixed_hash(b"kagemusha-recursive-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-bad-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-bad-hop-1"),
        ];
        let evidence = kagemusha_recursive_aggregation_evidence_from_steps(
            &chain_id,
            &asset,
            &steps,
            4,
            fixed_hash(b"recursive-bad-params"),
            fixed_hash(b"recursive-bad-schedule"),
            fixed_hash(b"recursive-bad-manifest"),
            fixed_hash(b"recursive-bad-bases"),
            fixed_hash(b"recursive-bad-batch"),
        )
        .expect("recursive aggregation evidence");

        let mut checked_mode = evidence.clone();
        checked_mode.aggregation_statement.aggregation_mode =
            KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&checked_mode),
            Err(KagemushaFoldError::RecursiveAggregationEvidenceModeMismatch { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        ));

        let mut bad_count = evidence.clone();
        bad_count.verifier_witness_count = 1;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&bad_count),
            Err(
                KagemushaFoldError::RecursiveAggregationWitnessCountMismatch {
                    expected: 2,
                    actual: 1
                }
            )
        ));

        let mut empty_statement = evidence.clone();
        empty_statement.aggregation_statement.steps.clear();
        empty_statement.aggregation_statement.hop_count = 0;
        empty_statement.verifier_witness_count = 0;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&empty_statement),
            Err(KagemushaFoldError::Empty)
        ));

        let mut too_many_hops = evidence.clone();
        too_many_hops.aggregation_statement.steps =
            vec![
                too_many_hops.aggregation_statement.steps[0].clone();
                KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
            ];
        too_many_hops.aggregation_statement.hop_count =
            u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1).expect("hop count fits");
        too_many_hops.verifier_witness_count = too_many_hops.aggregation_statement.hop_count;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&too_many_hops),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));

        let mut bad_profile = evidence.clone();
        bad_profile.verifier_witness_profile = "pallas-ipa-transparent-v1/mock".to_owned();
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&bad_profile),
            Err(
                KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile { actual, .. }
            ) if actual == "pallas-ipa-transparent-v1/mock"
        ));

        let mut unsupported_opening_len = evidence.clone();
        unsupported_opening_len.verifier_opening_len = 1;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&unsupported_opening_len),
            Err(KagemushaFoldError::UnsupportedRecursiveVerifierOpeningLength { actual: 1, .. })
        ));

        let mut non_power_opening_len = evidence.clone();
        non_power_opening_len.verifier_opening_len = 3;
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&non_power_opening_len),
            Err(KagemushaFoldError::NonPowerOfTwoRecursiveVerifierOpeningLength { actual: 3 })
        ));

        let bad_profile_bytes =
            to_bytes(&bad_profile).expect("encode unsupported-profile recursive evidence");
        let bad_profile_decoded: KagemushaRecursiveAggregationEvidence =
            norito::decode_from_bytes(&bad_profile_bytes)
                .expect("decode unsupported-profile recursive evidence");
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&bad_profile_decoded),
            Err(
                KagemushaFoldError::UnsupportedRecursiveVerifierWitnessProfile { actual, .. }
            ) if actual == "pallas-ipa-transparent-v1/mock"
        ));

        let mut zero_params = evidence.clone();
        zero_params.verifier_params_fingerprint = [0u8; Hash::LENGTH];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&zero_params),
            Err(KagemushaFoldError::ZeroRecursiveVerifierParamsFingerprint)
        ));

        let mut zero_schedule = evidence.clone();
        zero_schedule.fixed_window_table_schedule_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&zero_schedule),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableScheduleDigest)
        ));

        let mut zero_bases = evidence.clone();
        zero_bases.fixed_window_table_base_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&zero_bases),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableBaseDigest)
        ));

        let mut zero_batch = evidence.clone();
        zero_batch.verifier_witness_batch_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&zero_batch),
            Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest)
        ));

        let mut discontinuous = evidence.clone();
        discontinuous.aggregation_statement.steps[1].root_before =
            fixed_hash(b"kagemusha-recursive-wrong-root");
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&discontinuous),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));

        let mut duplicate_input = evidence.clone();
        duplicate_input.aggregation_statement.steps[1].input_nullifiers[0] =
            duplicate_input.aggregation_statement.steps[0].input_nullifiers[0];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&duplicate_input),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 1 })
        ));

        let mut duplicate_output = evidence.clone();
        duplicate_output.aggregation_statement.steps[1].output_commitments[0] =
            duplicate_output.aggregation_statement.steps[0].output_commitments[0];
        assert!(matches!(
            validate_kagemusha_recursive_aggregation_evidence(&duplicate_output),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));

        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                1,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::UnsupportedRecursiveVerifierOpeningLength { actual: 1, .. })
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                3,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::NonPowerOfTwoRecursiveVerifierOpeningLength { actual: 3 })
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                [0u8; Hash::LENGTH],
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::ZeroRecursiveVerifierParamsFingerprint)
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                fixed_hash(b"recursive-bad-params"),
                [0u8; Hash::LENGTH],
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableScheduleDigest)
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                [0u8; Hash::LENGTH],
                fixed_hash(b"recursive-bad-bases"),
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowSharedTableManifestDigest)
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                [0u8; Hash::LENGTH],
                fixed_hash(b"recursive-bad-batch"),
            ),
            Err(KagemushaFoldError::ZeroRecursiveFixedWindowTableBaseDigest)
        ));
        assert!(matches!(
            kagemusha_recursive_aggregation_evidence_from_steps(
                &chain_id,
                &asset,
                &steps,
                4,
                fixed_hash(b"recursive-bad-params"),
                fixed_hash(b"recursive-bad-schedule"),
                fixed_hash(b"recursive-bad-manifest"),
                fixed_hash(b"recursive-bad-bases"),
                [0u8; Hash::LENGTH],
            ),
            Err(KagemushaFoldError::ZeroRecursiveVerifierWitnessBatchDigest)
        ));

        let bytes = to_bytes(&evidence).expect("encode canonical recursive evidence");
        for len in 0..bytes.len().min(8) {
            assert!(
                norito::decode_from_bytes::<KagemushaRecursiveAggregationEvidence>(&bytes[..len])
                    .is_err(),
                "truncated recursive evidence archive at length {len} must reject"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_aggregation_proof_bundle_binds_evidence_and_roundtrips() {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive proof public inputs");
        assert_eq!(
            public_inputs.domain,
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS_DOMAIN
        );
        assert_eq!(
            public_inputs.evidence_digest,
            kagemusha_recursive_aggregation_evidence_digest(&evidence)
                .expect("recursive evidence digest")
        );
        assert_ne!(public_inputs.evidence_digest, [0u8; Hash::LENGTH]);
        assert_ne!(
            public_inputs.aggregation_transcript_digest,
            [0u8; Hash::LENGTH]
        );
        assert_eq!(
            public_inputs.verifier_params_fingerprint,
            evidence.verifier_params_fingerprint
        );
        assert_eq!(
            public_inputs.fixed_window_table_schedule_digest,
            evidence.fixed_window_table_schedule_digest
        );
        assert_eq!(
            public_inputs.fixed_window_shared_table_manifest_digest,
            evidence.fixed_window_shared_table_manifest_digest
        );
        assert_eq!(
            public_inputs.fixed_window_table_base_digest,
            evidence.fixed_window_table_base_digest
        );
        assert_eq!(
            public_inputs.verifier_witness_batch_digest,
            evidence.verifier_witness_batch_digest
        );
        assert_eq!(
            public_inputs.recursive_proof_chain_digest,
            [0u8; Hash::LENGTH],
            "plain recursive aggregation proofs do not carry spend proof-chain state"
        );
        assert_eq!(
            public_inputs.transition_profile_binding_digest,
            [0u8; Hash::LENGTH],
            "plain recursive aggregation proofs do not carry spend transition binding state"
        );
        assert_eq!(
            public_inputs.append_opening_preflight_digest,
            [0u8; Hash::LENGTH],
            "plain recursive aggregation proofs do not carry spend append opening preflight state"
        );
        assert_eq!(
            public_inputs.append_boundary_digest,
            [0u8; Hash::LENGTH],
            "plain recursive aggregation proofs do not carry spend append boundary state"
        );
        assert_eq!(
            public_inputs.recursive_verifier_scalar_projection_digest,
            [0u8; Hash::LENGTH],
            "plain recursive aggregation proofs do not carry verifier-slice scalar projection state"
        );
        assert_eq!(
            public_inputs.verifier_opening_len,
            evidence.verifier_opening_len
        );
        assert_eq!(
            public_inputs.verifier_witness_count,
            evidence.verifier_witness_count
        );
        assert_eq!(
            public_inputs.hop_count,
            evidence.aggregation_statement.hop_count
        );
        assert_ne!(
            kagemusha_recursive_aggregation_proof_public_inputs_schema_hash(),
            [0u8; Hash::LENGTH]
        );

        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive proof public-input hash");
        let recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
        };
        recursive_proof
            .validate_public_input_binding()
            .expect("recursive proof public inputs bind to proof metadata");
        assert!(matches!(
            kagemusha_recursive_spend_proof_artifact_digest(&recursive_proof),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_proof_chain_digest"
            })
        ));
        let bundle = KagemushaRecursiveAggregationProofBundle {
            evidence,
            recursive_proof,
        };
        bundle
            .validate_evidence_binding()
            .expect("recursive proof bundle binds evidence");

        let bytes = to_bytes(&bundle).expect("encode recursive proof bundle");
        let decoded: KagemushaRecursiveAggregationProofBundle =
            norito::decode_from_bytes(&bytes).expect("decode recursive proof bundle");
        assert_eq!(decoded, bundle);
        decoded
            .validate_evidence_binding()
            .expect("decoded recursive proof bundle remains canonical");
    }

    #[test]
    fn kagemusha_recursive_aggregation_proof_bundle_rejects_public_input_substitution() {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive proof public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive proof public-input hash");
        let recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
        };
        let bundle = KagemushaRecursiveAggregationProofBundle {
            evidence,
            recursive_proof,
        };

        let mut changed_batch = bundle.clone();
        changed_batch
            .recursive_proof
            .public_inputs
            .verifier_witness_batch_digest = fixed_hash(b"substituted-recursive-batch-digest");
        changed_batch.recursive_proof.public_inputs_hash = changed_batch
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed public-input hash");
        assert!(matches!(
            changed_batch.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "verifier_witness_batch_digest"
                }
            )
        ));

        let mut changed_params = bundle.clone();
        changed_params
            .recursive_proof
            .public_inputs
            .verifier_params_fingerprint = fixed_hash(b"substituted-recursive-params");
        changed_params.recursive_proof.public_inputs_hash = changed_params
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed public-input hash");
        assert!(matches!(
            changed_params.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "verifier_params_fingerprint"
                }
            )
        ));

        let mut changed_manifest = bundle.clone();
        changed_manifest
            .recursive_proof
            .public_inputs
            .fixed_window_shared_table_manifest_digest =
            fixed_hash(b"substituted-recursive-shared-manifest");
        changed_manifest.recursive_proof.public_inputs_hash = changed_manifest
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed public-input hash");
        assert!(matches!(
            changed_manifest.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "fixed_window_shared_table_manifest_digest"
                }
            )
        ));

        let mut changed_proof_chain = bundle.clone();
        changed_proof_chain
            .recursive_proof
            .public_inputs
            .recursive_proof_chain_digest = fixed_hash(b"substituted-recursive-proof-chain");
        changed_proof_chain.recursive_proof.public_inputs_hash = changed_proof_chain
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed proof-chain public-input hash");
        assert!(matches!(
            changed_proof_chain.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "recursive_proof_chain_digest"
                }
            )
        ));

        let mut changed_transition_binding = bundle.clone();
        changed_transition_binding
            .recursive_proof
            .public_inputs
            .transition_profile_binding_digest =
            fixed_hash(b"substituted-recursive-transition-binding");
        changed_transition_binding
            .recursive_proof
            .public_inputs_hash = changed_transition_binding
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed transition binding public-input hash");
        assert!(matches!(
            changed_transition_binding.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "transition_profile_binding_digest"
                }
            )
        ));

        let mut changed_append_opening_preflight = bundle.clone();
        changed_append_opening_preflight
            .recursive_proof
            .public_inputs
            .append_opening_preflight_digest =
            fixed_hash(b"substituted-recursive-append-opening-preflight");
        changed_append_opening_preflight
            .recursive_proof
            .public_inputs_hash = changed_append_opening_preflight
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed append opening preflight public-input hash");
        assert!(matches!(
            changed_append_opening_preflight.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "append_opening_preflight_digest"
                }
            )
        ));

        let mut changed_append_boundary = bundle.clone();
        changed_append_boundary
            .recursive_proof
            .public_inputs
            .append_boundary_digest = fixed_hash(b"substituted-recursive-append-boundary");
        changed_append_boundary.recursive_proof.public_inputs_hash = changed_append_boundary
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed append boundary public-input hash");
        assert!(matches!(
            changed_append_boundary.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "append_boundary_digest"
                }
            )
        ));

        let mut changed_scalar_projection = bundle.clone();
        changed_scalar_projection
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_hash(b"substituted-recursive-scalar-projection");
        changed_scalar_projection.recursive_proof.public_inputs_hash = changed_scalar_projection
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed scalar projection public-input hash");
        assert!(matches!(
            changed_scalar_projection.validate_evidence_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "recursive_verifier_scalar_projection_digest"
                }
            )
        ));

        let mut changed_hash = bundle.clone();
        changed_hash.recursive_proof.public_inputs_hash =
            Hash::new(b"wrong-recursive-proof-public-input-hash");
        assert!(matches!(
            changed_hash.validate_evidence_binding(),
            Err(KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch { .. })
        ));

        let mut changed_domain = bundle.clone();
        changed_domain.recursive_proof.public_inputs.domain =
            "iroha:kagemusha:v1:recursive-proof-alias".to_owned();
        changed_domain.recursive_proof.public_inputs_hash = changed_domain
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("changed domain public-input hash");
        assert!(matches!(
            changed_domain.validate_evidence_binding(),
            Err(KagemushaFoldError::InvalidRecursiveAggregationProofPublicInputDomain { .. })
        ));
    }

    #[test]
    fn kagemusha_recursive_public_inputs_reject_one_hop_append_opening_preflight() {
        let chain_id: ChainId = "kagemusha-recursive-one-hop-append-opening"
            .parse()
            .expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-one-hop-append-opening");
        let step = kagemusha_step(
            fixed_hash(b"kagemusha-recursive-one-hop-append-opening-root-0"),
            fixed_hash(b"kagemusha-recursive-one-hop-append-opening-root-1"),
            0x25,
            0x45,
            b"recursive-one-hop-append-opening-proof",
        );
        let evidence = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step,
            b"recursive-one-hop-append-opening-witness",
        );
        let mut public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive one-hop public inputs");
        assert_eq!(public_inputs.hop_count, 1);
        public_inputs.append_opening_preflight_digest =
            fixed_hash(b"forged-one-hop-append-opening-preflight");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("forged one-hop append opening public-input hash");
        assert!(matches!(
            public_inputs.validate_context(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "append_opening_preflight_digest"
                }
            )
        ));

        let recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
        };
        assert!(matches!(
            recursive_proof.validate_public_input_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "append_opening_preflight_digest"
                }
            )
        ));
    }

    #[test]
    fn kagemusha_recursive_aggregation_proof_rejects_spend_state_on_generic_circuit() {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let forged_spend_state_cases: [(
            &'static str,
            fn(&mut KagemushaRecursiveAggregationProofPublicInputs),
        ); 5] = [
            (
                "recursive_proof_chain_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.recursive_proof_chain_digest =
                        fixed_hash(b"forged-generic-recursive-proof-chain");
                },
            ),
            (
                "transition_profile_binding_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.transition_profile_binding_digest =
                        fixed_hash(b"forged-generic-transition-profile-binding");
                },
            ),
            (
                "append_opening_preflight_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.append_opening_preflight_digest =
                        fixed_hash(b"forged-generic-append-opening-preflight");
                },
            ),
            (
                "append_boundary_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.append_opening_preflight_digest =
                        fixed_hash(b"forged-generic-boundary-opening-preflight");
                    public_inputs.append_boundary_digest =
                        fixed_hash(b"forged-generic-append-boundary");
                },
            ),
            (
                "recursive_verifier_scalar_projection_digest",
                |public_inputs: &mut KagemushaRecursiveAggregationProofPublicInputs| {
                    public_inputs.recursive_verifier_scalar_projection_digest =
                        fixed_hash(b"forged-generic-recursive-scalar-projection");
                },
            ),
        ];
        for (expected_field, mutate) in forged_spend_state_cases {
            let mut public_inputs =
                kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                    .expect("recursive proof public inputs");
            public_inputs
                .validate_context()
                .expect("plain recursive proof public inputs validate before mutation");
            mutate(&mut public_inputs);
            public_inputs
                .validate_context()
                .expect("spend-only field ownership remains a proof-circuit policy check");
            let public_inputs_hash = public_inputs
                .public_inputs_hash()
                .expect("forged spend-state public-input hash");
            let recursive_proof = KagemushaRecursiveAggregationProof {
                verifier_key_id: VerifyingKeyId::new(
                    "halo2/ipa",
                    KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                ),
                public_inputs,
                public_inputs_hash,
                proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
            };
            let err = recursive_proof
                .validate_public_input_binding()
                .expect_err("generic recursive proof must reject spend-only public inputs");
            assert!(
                matches!(
                    err,
                    KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch { field }
                    if field == expected_field
                ),
                "unexpected generic proof spend-state error for {expected_field}: {err:?}"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_aggregation_proof_bundle_rejects_backend_and_circuit_substitution() {
        let evidence = sample_kagemusha_recursive_aggregation_evidence();
        let public_inputs =
            kagemusha_recursive_aggregation_proof_public_inputs_from_evidence(&evidence)
                .expect("recursive proof public inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("recursive proof public-input hash");
        let recursive_proof = KagemushaRecursiveAggregationProof {
            verifier_key_id: VerifyingKeyId::new(
                "halo2/ipa",
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
            ),
            public_inputs,
            public_inputs_hash,
            proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
        };
        let bundle = KagemushaRecursiveAggregationProofBundle {
            evidence,
            recursive_proof,
        };

        let mut backend_mismatch = bundle.clone();
        backend_mismatch.recursive_proof.verifier_key_id = VerifyingKeyId::new(
            "stark/fri",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        );
        assert!(matches!(
            backend_mismatch.validate_evidence_binding(),
            Err(KagemushaFoldError::RecursiveAggregationProofBackendMismatch { .. })
        ));

        let mut trusted_setup_backend = bundle.clone();
        trusted_setup_backend.recursive_proof.proof =
            ProofBox::new("halo2/kzg".into(), vec![0xA5; 64]);
        assert!(matches!(
            trusted_setup_backend.validate_evidence_binding(),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));

        let mut stark_wrong_family = bundle.clone();
        stark_wrong_family.recursive_proof.proof =
            ProofBox::new("stark/fri/sha256-goldilocks".into(), vec![0xA5; 64]);
        stark_wrong_family.recursive_proof.verifier_key_id = VerifyingKeyId::new(
            "stark/fri/sha256-goldilocks",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        );
        assert!(matches!(
            stark_wrong_family.validate_evidence_binding(),
            Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.backend"
            })
        ));

        let mut empty_proof_payload = bundle.clone();
        empty_proof_payload.recursive_proof.proof = ProofBox::new("halo2/ipa".into(), Vec::new());
        assert!(matches!(
            empty_proof_payload.validate_evidence_binding(),
            Err(KagemushaFoldError::InvalidRecursiveAggregationProof {
                field: "proof.bytes"
            })
        ));

        let mut wrong_circuit = bundle;
        wrong_circuit.recursive_proof.verifier_key_id =
            VerifyingKeyId::new("halo2/ipa", "kagemusha-recursive-aggregation-alias");
        assert!(matches!(
            wrong_circuit.validate_evidence_binding(),
            Err(KagemushaFoldError::RecursiveAggregationProofCircuitIdMismatch {
                actual,
                ..
            }) if actual == "kagemusha-recursive-aggregation-alias"
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_bundle_roundtrips_and_appends_without_prior_hops() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-spend-root-2");

        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-spend-hop-0");
        let mut expected_topup_anchors = step0.input_nullifiers.clone();
        expected_topup_anchors.sort_unstable();
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-witness-hop-0",
        );
        let accumulator0 =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let transition_profile0 =
            kagemusha_recursive_spend_transition_profile_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend transition profile");
        assert_eq!(accumulator0.hop_count, 1);
        assert_eq!(accumulator0.initial_root, root0);
        assert_eq!(accumulator0.final_root, root1);
        assert_eq!(accumulator0.topup_anchor_nullifiers, expected_topup_anchors);
        assert_eq!(transition_profile0.hop_index, 0);
        assert_eq!(transition_profile0.hop_count, 1);
        assert!(transition_profile0.previous_accumulator_digest.is_none());
        assert!(
            transition_profile0
                .previous_accumulator_public_inputs_hash
                .is_none()
        );
        assert_eq!(transition_profile0.current_hop_statement.hop_index, 0);
        assert_eq!(transition_profile0.current_note, note0);
        assert_eq!(
            transition_profile0.resulting_accumulator_digest,
            kagemusha_recursive_spend_accumulator_digest(&accumulator0)
                .expect("initial accumulator digest")
        );
        assert_eq!(
            transition_profile0.resulting_public_inputs_hash,
            accumulator0
                .recursive_public_inputs()
                .expect("initial public inputs")
                .public_inputs_hash()
                .expect("initial public-input hash")
        );
        let transition_profile0_digest = transition_profile0
            .digest()
            .expect("initial transition profile digest");
        let folded_nullifier_digest = kagemusha_list_digest(
            KAGEMUSHA_FOLD_NULLIFIER_DIGEST_DOMAIN,
            evidence0.aggregation_statement.steps[0]
                .input_nullifiers
                .clone(),
        )
        .expect("folded nullifier digest");
        let folded_output_digest = kagemusha_list_digest(
            KAGEMUSHA_FOLD_OUTPUT_DIGEST_DOMAIN,
            evidence0.aggregation_statement.steps[0]
                .output_commitments
                .clone(),
        )
        .expect("folded output digest");
        let mut checked_statement = evidence0.aggregation_statement.clone();
        checked_statement.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1;
        let folded_parts =
            kagemusha_fold_digest_parts_from_aggregation_statement(&checked_statement)
                .expect("checked folded digest parts");
        assert_ne!(
            accumulator0.nullifier_digest, folded_nullifier_digest,
            "recursive spend nullifier stream must not reuse the folded-list digest domain"
        );
        assert_ne!(
            accumulator0.output_commitment_digest, folded_output_digest,
            "recursive spend output stream must not reuse the folded-list digest domain"
        );
        assert_ne!(
            accumulator0.fold_digest, folded_parts.fold,
            "recursive spend fold stream must not reuse the checked folded-token transcript domain"
        );
        assert_ne!(
            accumulator0.recursive_proof_chain_digest,
            [0u8; Hash::LENGTH],
            "recursive spend proof-chain stream must be initialized at the first hop"
        );
        assert_ne!(
            accumulator0.transition_profile_binding_digest,
            [0u8; Hash::LENGTH],
            "recursive spend transition-profile binding must be initialized at the first hop"
        );
        assert_eq!(
            accumulator0.transition_profile_binding_digest,
            transition_profile0
                .binding_digest()
                .expect("initial transition profile binding digest")
        );
        let previous_proof0 = kagemusha_recursive_spend_proof(&accumulator0);
        let lineage_previous_proof0 = kagemusha_recursive_spend_lineage_proof(
            &accumulator0,
            b"recursive-spend-lineage-previous-proof-0",
        );
        let previous_proof0_artifact =
            kagemusha_recursive_spend_proof_artifact_digest(&previous_proof0)
                .expect("semantic previous proof artifact digest");
        let lineage_previous_proof0_artifact =
            kagemusha_recursive_spend_proof_artifact_digest(&lineage_previous_proof0)
                .expect("Reserved-lineage previous proof artifact digest");
        assert_ne!(
            previous_proof0_artifact, lineage_previous_proof0_artifact,
            "previous proof circuit id and scalar-projection material must be proof-chain visible"
        );

        let mut step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-spend-hop-1");
        step1.input_nullifiers = vec![note0.spend_nullifier];
        let note1 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step1.output_commitments[1],
            spend_nullifier: fixed_hash(b"recursive-spend-note-1-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence1 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1,
            b"recursive-spend-witness-hop-1",
        );
        let accumulator1 = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &previous_proof0,
            &evidence1,
            &note1,
        )
        .expect("append recursive spend accumulator");
        let transition_profile1 = kagemusha_recursive_spend_transition_profile_append_evidence(
            &accumulator0,
            &previous_proof0,
            &evidence1,
            &note1,
        )
        .expect("append recursive spend transition profile");
        assert_eq!(accumulator1.hop_count, 2);
        assert_eq!(accumulator1.initial_root, root0);
        assert_eq!(accumulator1.final_root, root2);
        assert_eq!(accumulator1.current_note, note1);
        assert_eq!(accumulator1.topup_anchor_nullifiers, expected_topup_anchors);
        assert_eq!(
            accumulator1.transition_profile_binding_digest,
            transition_profile1
                .binding_digest()
                .expect("append transition profile binding digest")
        );
        assert_eq!(transition_profile1.hop_index, 1);
        assert_eq!(transition_profile1.hop_count, 2);
        assert_eq!(transition_profile1.current_hop_statement.hop_index, 1);
        let mut duplicate_initial_input_profile = transition_profile0.clone();
        duplicate_initial_input_profile
            .current_hop_statement
            .input_nullifiers[1] = duplicate_initial_input_profile
            .current_hop_statement
            .input_nullifiers[0];
        assert!(matches!(
            duplicate_initial_input_profile.validate_context(),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 0 })
        ));
        let mut overlapping_initial_output_profile = transition_profile0.clone();
        overlapping_initial_output_profile
            .current_hop_statement
            .output_commitments[0] = overlapping_initial_output_profile
            .current_hop_statement
            .input_nullifiers[0];
        assert!(matches!(
            overlapping_initial_output_profile.validate_context(),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 0 })
        ));
        let mut duplicate_append_output_profile = transition_profile1.clone();
        duplicate_append_output_profile
            .current_hop_statement
            .output_commitments[1] = duplicate_append_output_profile
            .current_hop_statement
            .output_commitments[0];
        assert!(matches!(
            duplicate_append_output_profile.validate_context(),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));
        assert_eq!(
            transition_profile1.previous_current_note,
            Some(note0.clone())
        );
        assert_eq!(
            transition_profile1.previous_accumulator_digest,
            Some(
                kagemusha_recursive_spend_accumulator_digest(&accumulator0)
                    .expect("previous accumulator digest")
            )
        );
        assert_eq!(
            transition_profile1.previous_recursive_proof_artifact_digest,
            Some(previous_proof0_artifact)
        );
        assert_eq!(
            transition_profile1.previous_accumulator_public_inputs_hash,
            Some(
                accumulator0
                    .recursive_public_inputs()
                    .expect("previous accumulator public inputs")
                    .public_inputs_hash()
                    .expect("previous accumulator public-input hash")
            )
        );
        assert_eq!(
            transition_profile1.previous_recursive_proof_public_inputs_hash,
            Some(previous_proof0.public_inputs_hash)
        );
        assert_eq!(
            transition_profile1.previous_recursive_proof_open_envelopes_archive_digest, None,
            "legacy evidence-only append profiles omit host opening-archive bytes"
        );
        assert_eq!(
            transition_profile1.append_opening_preflight_digest, None,
            "legacy evidence-only append profiles omit append opening preflight bytes"
        );
        let mut previous_bundle0 = KagemushaRecursiveSpendBundleV1 {
            accumulator: accumulator0.clone(),
            recursive_proof: previous_proof0.clone(),
        };
        attach_recursive_spend_open_verify_envelope(
            &mut previous_bundle0,
            b"recursive-spend-transition-previous-openings-vk",
        );
        let compact_token0 =
            kagemusha_recursive_spend_compact_payment_token_from_bundle(&previous_bundle0)
                .expect("initial recursive spend bundle projects to a compact token");
        let compact_public_inputs0 =
            kagemusha_recursive_spend_folded_public_inputs_from_accumulator(&accumulator0)
                .expect("initial recursive spend compact public inputs");
        assert_eq!(compact_token0.public_inputs, compact_public_inputs0);
        assert_eq!(
            compact_token0.folded_proof.verifier_key_id,
            previous_bundle0.recursive_proof.verifier_key_id
        );
        assert_eq!(
            compact_token0.folded_proof.public_inputs_hash,
            compact_public_inputs0
                .public_inputs_hash()
                .expect("initial recursive spend compact public-input hash")
        );
        assert_eq!(
            compact_token0.folded_proof.proof,
            previous_bundle0.recursive_proof.proof
        );
        let transition_profile1_with_attached_previous_proof =
            kagemusha_recursive_spend_transition_profile_append_evidence(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &evidence1,
                &note1,
            )
            .expect("append transition profile with attached previous proof");
        let previous_openings_archive =
            kagemusha_recursive_spend_previous_proof_open_envelope_archive(&previous_bundle0, 0x92);
        let transition_profile1_with_previous_openings =
            kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                &evidence1,
                &note1,
            )
            .expect("append transition profile with previous proof openings");
        let previous_openings_archive_digest =
            kagemusha_recursive_previous_proof_open_envelopes_archive_digest(
                &previous_openings_archive,
            )
            .expect("previous proof opening archive digest");
        assert_eq!(
            transition_profile1_with_previous_openings
                .previous_recursive_proof_open_envelopes_archive_digest,
            Some(previous_openings_archive_digest)
        );
        assert_ne!(
            transition_profile1_with_previous_openings
                .digest()
                .expect("append transition profile with openings digest"),
            transition_profile1
                .digest()
                .expect("legacy append transition profile digest"),
            "binding previous proof opening bytes must change the transition profile digest"
        );
        assert_eq!(
            transition_profile1_with_previous_openings
                .binding_digest()
                .expect("append transition profile with openings binding digest"),
            transition_profile1_with_attached_previous_proof
                .binding_digest()
                .expect("attached previous proof transition profile binding digest"),
            "accumulator transition binding ignores append transport opening archives"
        );
        assert_ne!(
            transition_profile1_with_attached_previous_proof
                .binding_digest()
                .expect("attached previous proof transition profile binding digest"),
            transition_profile1
                .binding_digest()
                .expect("legacy append transition profile binding digest"),
            "previous proof artifact changes remain visible to the transition binding"
        );
        let append_opening_preflight_digest =
            fixed_hash(b"recursive-spend-transition-append-opening-preflight");
        let transition_profile1_with_append_opening_preflight =
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                append_opening_preflight_digest,
                &evidence1,
                &note1,
            )
            .expect("append transition profile with append opening preflight");
        assert_eq!(
            transition_profile1_with_append_opening_preflight
                .previous_recursive_proof_open_envelopes_archive_digest,
            Some(previous_openings_archive_digest)
        );
        assert_eq!(
            transition_profile1_with_append_opening_preflight.append_opening_preflight_digest,
            Some(append_opening_preflight_digest)
        );
        assert_eq!(
            transition_profile1_with_append_opening_preflight
                .resulting_append_opening_preflight_digest,
            append_opening_preflight_digest
        );
        assert_ne!(
            transition_profile1_with_append_opening_preflight
                .digest()
                .expect("append transition profile with append opening preflight digest"),
            transition_profile1_with_previous_openings
                .digest()
                .expect("append transition profile with openings digest"),
            "binding append opening preflight bytes must change the transition profile digest"
        );
        let accumulator1_with_append_opening_preflight =
            kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_digest(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                append_opening_preflight_digest,
                &evidence1,
                &note1,
            )
            .expect("append accumulator with append opening preflight");
        assert_eq!(
            accumulator1_with_append_opening_preflight.append_opening_preflight_digest,
            append_opening_preflight_digest
        );
        assert_eq!(
            accumulator1_with_append_opening_preflight.append_boundary_digest,
            [0u8; Hash::LENGTH],
            "digest-only compatibility appends do not invent a compact append boundary"
        );
        assert_ne!(
            transition_profile1_with_append_opening_preflight
                .binding_digest()
                .expect("append transition profile with preflight binding digest"),
            transition_profile1
                .binding_digest()
                .expect("legacy append transition profile binding digest"),
            "accumulator transition binding must expose append opening preflight digest"
        );
        let digest_only_semantic_bundle =
            kagemusha_recursive_spend_bundle(accumulator1_with_append_opening_preflight.clone());
        assert!(matches!(
            digest_only_semantic_bundle.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_compact_payment_token_from_bundle(
                &digest_only_semantic_bundle
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_proof_artifact_digest(
                &digest_only_semantic_bundle.recursive_proof
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest"
            })
        ));
        let digest_only_lineage_bundle = KagemushaRecursiveSpendBundleV1 {
            accumulator: accumulator1_with_append_opening_preflight.clone(),
            recursive_proof: kagemusha_recursive_spend_lineage_proof(
                &accumulator1_with_append_opening_preflight,
                b"recursive-spend-lineage-digest-only-append-opening",
            ),
        };
        assert!(matches!(
            digest_only_lineage_bundle.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_boundary_digest"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_proof_artifact_digest(
                &digest_only_lineage_bundle.recursive_proof
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_boundary_digest"
            })
        ));
        let attached_previous_proof0_artifact =
            kagemusha_recursive_spend_proof_artifact_digest(&previous_bundle0.recursive_proof)
                .expect("attached previous proof artifact digest");
        let append_opening_preflight_contract =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                kagemusha_recursive_verifier_preflight_for_evidence(
                    &evidence1,
                    fixed_hash(b"recursive-spend-transition-previous-proof-opening-preflight"),
                ),
                kagemusha_recursive_verifier_preflight_for_evidence(
                    &evidence1,
                    evidence1.verifier_witness_batch_digest,
                ),
                kagemusha_recursive_spend_accumulator_digest(&accumulator0)
                    .expect("previous accumulator digest for opening preflight"),
                attached_previous_proof0_artifact,
                previous_openings_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect("append opening preflight contract");
        let transition_profile1_with_append_opening_contract =
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                append_opening_preflight_contract.clone(),
                &evidence1,
                &note1,
            )
            .expect("append transition profile with append opening preflight contract");
        assert_eq!(
            transition_profile1_with_append_opening_contract.append_opening_preflight_digest,
            Some(append_opening_preflight_contract.append_opening_preflight_digest)
        );
        assert_eq!(
            transition_profile1_with_append_opening_contract
                .resulting_append_opening_preflight_digest,
            append_opening_preflight_contract.append_opening_preflight_digest
        );
        assert_eq!(
            transition_profile1_with_append_opening_contract
                .append_opening_preflight
                .as_ref(),
            Some(&append_opening_preflight_contract)
        );
        assert_ne!(
            transition_profile1_with_append_opening_contract
                .digest()
                .expect("append transition profile with opening preflight contract digest"),
            transition_profile1_with_append_opening_preflight
                .digest()
                .expect("append transition profile with digest-only preflight digest"),
            "binding the full append opening preflight contract must change the transition profile digest"
        );
        assert_ne!(
            transition_profile1_with_append_opening_contract
                .binding_digest()
                .expect("append transition profile with contract binding digest"),
            transition_profile1_with_append_opening_preflight
                .binding_digest()
                .expect("append transition profile with preflight binding digest"),
            "full append-opening contracts bind the resulting append boundary into the transition"
        );
        let append_boundary =
            kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
                &transition_profile1_with_append_opening_contract,
            )
            .expect("append boundary from full transition profile");
        assert_eq!(
            append_boundary.transition_profile_digest,
            transition_profile1_with_append_opening_contract
                .digest()
                .expect("transition profile digest for append boundary")
        );
        assert_eq!(
            append_boundary.transition_profile_binding_digest,
            transition_profile1_with_append_opening_contract
                .binding_digest()
                .expect("transition profile binding digest for append boundary")
        );
        assert_eq!(
            append_boundary.chain_asset_binding_digest,
            kagemusha_recursive_spend_lineage_append_boundary_chain_asset_binding_digest(
                &chain_id, &asset
            )
            .expect("chain/asset binding digest for append boundary")
        );
        let other_chain_id: ChainId = "kagemusha-recursive-spend-other-chain"
            .parse()
            .expect("other chain id");
        assert_ne!(
            append_boundary.chain_asset_binding_digest,
            kagemusha_recursive_spend_lineage_append_boundary_chain_asset_binding_digest(
                &other_chain_id,
                &asset
            )
            .expect("other chain binding digest"),
            "append boundary chain/asset binding must bind chain id"
        );
        let other_asset = kagemusha_asset("kgm-recursive-spend-other");
        assert_ne!(
            append_boundary.chain_asset_binding_digest,
            kagemusha_recursive_spend_lineage_append_boundary_chain_asset_binding_digest(
                &chain_id,
                &other_asset
            )
            .expect("other asset binding digest"),
            "append boundary chain/asset binding must bind asset id"
        );
        assert_eq!(
            append_boundary.final_note_binding_digest,
            kagemusha_recursive_spend_lineage_append_boundary_final_note_binding_digest(
                transition_profile1_with_append_opening_contract.resulting_final_root,
                &transition_profile1_with_append_opening_contract.current_note,
            )
            .expect("final-note binding digest for append boundary")
        );
        assert_ne!(
            append_boundary.final_note_binding_digest,
            kagemusha_recursive_spend_lineage_append_boundary_final_note_binding_digest(
                fixed_hash(b"kagemusha-recursive-spend-other-final-root"),
                &transition_profile1_with_append_opening_contract.current_note,
            )
            .expect("other final-root binding digest"),
            "append boundary final-note binding must bind final root"
        );
        let mut other_current_note = transition_profile1_with_append_opening_contract
            .current_note
            .clone();
        other_current_note.note_commitment =
            fixed_hash(b"kagemusha-recursive-spend-other-current-note");
        assert_ne!(
            append_boundary.final_note_binding_digest,
            kagemusha_recursive_spend_lineage_append_boundary_final_note_binding_digest(
                transition_profile1_with_append_opening_contract.resulting_final_root,
                &other_current_note,
            )
            .expect("other current-note binding digest"),
            "append boundary final-note binding must bind the current spendable note"
        );
        assert_eq!(
            append_boundary.previous_accumulator_digest,
            kagemusha_recursive_spend_accumulator_digest(&accumulator0)
                .expect("previous accumulator digest for append boundary")
        );
        assert_eq!(
            append_boundary.previous_recursive_proof_artifact_digest,
            attached_previous_proof0_artifact
        );
        assert_eq!(
            append_boundary.previous_recursive_proof_open_envelopes_archive_digest,
            previous_openings_archive_digest
        );
        assert_eq!(
            append_boundary.append_opening_preflight_digest,
            append_opening_preflight_contract.append_opening_preflight_digest
        );
        assert_eq!(
            append_boundary.previous_recursive_proof_opening_aggregate_digest,
            append_opening_preflight_contract
                .previous_recursive_proof_preflight
                .aggregate_digest
        );
        assert_eq!(
            append_boundary.current_hop_opening_aggregate_digest,
            append_opening_preflight_contract
                .current_hop_preflight
                .aggregate_digest
        );
        assert_eq!(
            append_boundary.current_hop_proof_hash,
            append_opening_preflight_contract.current_hop_proof_hash
        );
        assert_eq!(
            append_boundary.hop_count,
            transition_profile1_with_append_opening_contract.hop_count
        );
        assert_ne!(append_boundary.append_boundary_digest, [0u8; Hash::LENGTH]);
        let accumulator1_with_append_opening_contract_digest =
            kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_digest(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                append_opening_preflight_contract.append_opening_preflight_digest,
                &evidence1,
                &note1,
            )
            .expect("append accumulator with contract preflight digest only");
        let accumulator1_with_append_boundary =
            kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                append_opening_preflight_contract.clone(),
                &evidence1,
                &note1,
            )
            .expect("append accumulator with full append boundary");
        assert_eq!(
            accumulator1_with_append_boundary.append_opening_preflight_digest,
            append_opening_preflight_contract.append_opening_preflight_digest
        );
        assert_eq!(
            accumulator1_with_append_boundary.append_boundary_digest,
            append_boundary.append_boundary_digest,
            "full Reserved-lineage append accumulators carry the canonical compact boundary digest"
        );
        assert_eq!(
            kagemusha_recursive_spend_accumulator_digest(&accumulator1_with_append_boundary)
                .expect("boundary accumulator digest"),
            kagemusha_recursive_spend_accumulator_digest(
                &accumulator1_with_append_opening_contract_digest
            )
            .expect("digest-only accumulator digest"),
            "append-boundary digest must not feed back into the accumulator digest"
        );
        assert_eq!(
            append_boundary.resulting_public_inputs_hash,
            kagemusha_recursive_spend_append_boundary_free_public_inputs_hash(
                &accumulator1_with_append_boundary,
            )
            .expect("boundary-free resulting public-input hash"),
            "append boundary binds the non-circular resulting public-input hash"
        );
        let final_public_inputs = accumulator1_with_append_boundary
            .recursive_public_inputs()
            .expect("final recursive spend public inputs");
        assert_eq!(
            final_public_inputs.append_boundary_digest,
            append_boundary.append_boundary_digest
        );
        assert_ne!(
            final_public_inputs
                .public_inputs_hash()
                .expect("final public-input hash"),
            append_boundary.resulting_public_inputs_hash,
            "the final proof public-input hash includes the append-boundary digest"
        );
        let lineage_append_boundary_bundle = KagemushaRecursiveSpendBundleV1 {
            accumulator: accumulator1_with_append_boundary.clone(),
            recursive_proof: kagemusha_recursive_spend_lineage_proof(
                &accumulator1_with_append_boundary,
                b"recursive-spend-lineage-append-boundary-scalar",
            ),
        };
        lineage_append_boundary_bundle
            .validate_public_input_binding()
            .expect("lineage append proof binds canonical append-boundary public input");
        let compact_lineage_append_token =
            kagemusha_recursive_spend_compact_payment_token_from_bundle(
                &lineage_append_boundary_bundle,
            )
            .expect("lineage append recursive spend bundle projects to a compact token");
        let compact_append_public_inputs =
            kagemusha_recursive_spend_folded_public_inputs_from_accumulator(
                &accumulator1_with_append_boundary,
            )
            .expect("append recursive spend compact public inputs");
        assert_eq!(
            compact_lineage_append_token.public_inputs,
            compact_append_public_inputs
        );
        assert_eq!(compact_lineage_append_token.public_inputs.hop_count, 2);
        assert_eq!(
            compact_lineage_append_token.public_inputs.initial_root,
            root0
        );
        assert_eq!(compact_lineage_append_token.public_inputs.final_root, root2);
        assert_eq!(
            compact_lineage_append_token.folded_proof.verifier_key_id,
            lineage_append_boundary_bundle
                .recursive_proof
                .verifier_key_id
        );
        assert_eq!(
            compact_lineage_append_token.folded_proof.public_inputs_hash,
            compact_append_public_inputs
                .public_inputs_hash()
                .expect("append recursive spend compact public-input hash")
        );
        assert_eq!(
            compact_lineage_append_token.folded_proof.proof,
            lineage_append_boundary_bundle.recursive_proof.proof
        );
        let mut forged_accumulator_boundary = lineage_append_boundary_bundle.clone();
        forged_accumulator_boundary
            .accumulator
            .append_boundary_digest[0] ^= 0x01;
        assert!(matches!(
            forged_accumulator_boundary.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary_digest"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_compact_payment_token_from_bundle(
                &forged_accumulator_boundary
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary_digest"
            })
        ));
        let semantic_boundary_bundle =
            kagemusha_recursive_spend_bundle(accumulator1_with_append_boundary.clone());
        assert!(matches!(
            semantic_boundary_bundle.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary_digest"
            })
        ));
        assert_eq!(
            append_boundary
                .digest()
                .expect("append boundary digest validates"),
            append_boundary.append_boundary_digest
        );
        let append_boundary_bytes = to_bytes(&append_boundary).expect("encode append boundary");
        let decoded_append_boundary: KagemushaRecursiveSpendLineageAppendBoundaryV1 =
            norito::decode_from_bytes(&append_boundary_bytes).expect("decode append boundary");
        assert_eq!(decoded_append_boundary, append_boundary);
        decoded_append_boundary
            .validate_context()
            .expect("decoded append boundary binding");
        let opening_contract_bytes = to_bytes(&append_opening_preflight_contract)
            .expect("encode append opening preflight contract");
        let decoded_opening_contract: KagemushaRecursiveSpendLineageAppendOpeningPreflightV1 =
            norito::decode_from_bytes(&opening_contract_bytes)
                .expect("decode append opening preflight contract");
        assert_eq!(decoded_opening_contract, append_opening_preflight_contract);
        decoded_opening_contract
            .validate_context()
            .expect("decoded append opening preflight contract binding");
        for (forged_current_preflight, expected_field) in [
            {
                let mut preflight = append_opening_preflight_contract
                    .current_hop_preflight
                    .clone();
                preflight.opening_len = 8;
                (preflight, "append_opening_preflight.shared_opening_len")
            },
            {
                let mut preflight = append_opening_preflight_contract
                    .current_hop_preflight
                    .clone();
                preflight.params_fingerprint = fixed_hash(b"recursive-spend-forged-shared-params");
                (
                    preflight,
                    "append_opening_preflight.shared_params_fingerprint",
                )
            },
            {
                let mut preflight = append_opening_preflight_contract
                    .current_hop_preflight
                    .clone();
                preflight.fixed_window_table_schedule_digest =
                    fixed_hash(b"recursive-spend-forged-shared-schedule");
                (
                    preflight,
                    "append_opening_preflight.shared_fixed_window_table_schedule_digest",
                )
            },
            {
                let mut preflight = append_opening_preflight_contract
                    .current_hop_preflight
                    .clone();
                preflight.fixed_window_shared_table_manifest_digest =
                    fixed_hash(b"recursive-spend-forged-shared-manifest");
                (
                    preflight,
                    "append_opening_preflight.shared_fixed_window_table_manifest_digest",
                )
            },
        ] {
            let err = KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                append_opening_preflight_contract
                    .previous_recursive_proof_preflight
                    .clone(),
                forged_current_preflight,
                append_opening_preflight_contract.previous_accumulator_digest,
                append_opening_preflight_contract.previous_recursive_proof_artifact_digest,
                append_opening_preflight_contract
                    .previous_recursive_proof_open_envelopes_archive_digest,
                append_opening_preflight_contract.current_hop_proof_hash,
            )
            .expect_err("append opening preflight contract must reject shared-context splice");
            assert!(
                matches!(
                    err,
                    KagemushaFoldError::RecursiveSpendVerifierContextMismatch { field }
                    if field == expected_field
                ),
                "unexpected shared verifier context error for {expected_field}: {err:?}"
            );
        }
        let replayed_previous_openings_archive =
            kagemusha_recursive_spend_previous_proof_open_envelope_archive(&previous_bundle0, 0x93);
        assert_ne!(
            previous_openings_archive_digest,
            kagemusha_recursive_previous_proof_open_envelopes_archive_digest(
                &replayed_previous_openings_archive,
            )
            .expect("replayed previous proof opening archive digest"),
            "opening transcript bytes remain bound even when metadata is identical"
        );
        assert_eq!(
            transition_profile1.resulting_accumulator_digest,
            kagemusha_recursive_spend_accumulator_digest(&accumulator1)
                .expect("append accumulator digest")
        );
        assert_eq!(
            transition_profile1.resulting_public_inputs_hash,
            accumulator1
                .recursive_public_inputs()
                .expect("append public inputs")
                .public_inputs_hash()
                .expect("append public-input hash")
        );
        assert_ne!(
            transition_profile1
                .digest()
                .expect("append transition profile digest"),
            transition_profile0_digest
        );
        assert_ne!(accumulator1.lineage_digest, accumulator0.lineage_digest);
        assert_ne!(accumulator1.nullifier_digest, accumulator0.nullifier_digest);
        assert_ne!(
            accumulator1.output_commitment_digest,
            accumulator0.output_commitment_digest
        );
        assert_ne!(
            accumulator1.verifier_witness_batch_digest,
            accumulator0.verifier_witness_batch_digest
        );
        assert_ne!(
            accumulator1.recursive_proof_chain_digest,
            accumulator0.recursive_proof_chain_digest
        );
        let accumulator1_from_lineage = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &lineage_previous_proof0,
            &evidence1,
            &note1,
        )
        .expect("append recursive spend accumulator from lineage proof");
        assert_eq!(accumulator1_from_lineage.hop_count, 2);
        assert_eq!(accumulator1_from_lineage.current_note, note1);
        assert_eq!(
            accumulator1_from_lineage.topup_anchor_nullifiers,
            expected_topup_anchors
        );
        assert_ne!(
            accumulator1_from_lineage.recursive_proof_chain_digest,
            accumulator1.recursive_proof_chain_digest,
            "lineage proof artifact must be distinguished from semantic v1 proof artifact"
        );
        let transition_profile1_from_lineage =
            kagemusha_recursive_spend_transition_profile_append_evidence(
                &accumulator0,
                &lineage_previous_proof0,
                &evidence1,
                &note1,
            )
            .expect("append transition profile from lineage proof");
        assert_eq!(
            transition_profile1_from_lineage.previous_accumulator_public_inputs_hash,
            Some(lineage_previous_proof0.public_inputs_hash),
            "Reserved-lineage previous proofs use proof-compatible public-input normalization"
        );
        assert_ne!(
            transition_profile1_from_lineage.previous_accumulator_public_inputs_hash,
            Some(
                accumulator0
                    .recursive_public_inputs()
                    .expect("raw previous accumulator public inputs")
                    .public_inputs_hash()
                    .expect("raw previous accumulator public-input hash")
            ),
            "Reserved-lineage scalar projection must not be normalized away"
        );
        let mut lineage_scalar_projection_splice = lineage_previous_proof0.clone();
        lineage_scalar_projection_splice
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_hash(b"recursive-spend-lineage-previous-proof-0-scalar-splice");
        lineage_scalar_projection_splice.public_inputs_hash = lineage_scalar_projection_splice
            .public_inputs
            .public_inputs_hash()
            .expect("lineage scalar splice public-input hash");
        let lineage_scalar_projection_splice_artifact =
            kagemusha_recursive_spend_proof_artifact_digest(&lineage_scalar_projection_splice)
                .expect("scalar-spliced Reserved-lineage proof artifact digest");
        assert_ne!(
            lineage_scalar_projection_splice_artifact, lineage_previous_proof0_artifact,
            "scalar-projection substitutions must change the exported proof artifact digest"
        );
        let accumulator1_from_lineage_scalar_splice =
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &lineage_scalar_projection_splice,
                &evidence1,
                &note1,
            )
            .expect("append recursive spend accumulator from scalar-spliced lineage proof");
        assert_ne!(
            accumulator1_from_lineage_scalar_splice.recursive_proof_chain_digest,
            accumulator1_from_lineage.recursive_proof_chain_digest,
            "lineage scalar-projection substitutions must be bound into the proof-chain digest"
        );
        let mut zero_lineage_scalar_projection = lineage_previous_proof0.clone();
        zero_lineage_scalar_projection
            .public_inputs
            .recursive_verifier_scalar_projection_digest = [0u8; Hash::LENGTH];
        zero_lineage_scalar_projection.public_inputs_hash = zero_lineage_scalar_projection
            .public_inputs
            .public_inputs_hash()
            .expect("zero lineage scalar public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_proof_artifact_digest(&zero_lineage_scalar_projection),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_verifier_scalar_projection_digest"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &zero_lineage_scalar_projection,
                &evidence1,
                &note1,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_verifier_scalar_projection_digest"
            })
        ));

        let bundle = kagemusha_recursive_spend_bundle(accumulator1);
        bundle
            .validate_public_input_binding()
            .expect("recursive spend bundle binding");
        let bytes = to_bytes(&bundle).expect("encode recursive spend bundle");
        let decoded: KagemushaRecursiveSpendBundleV1 =
            norito::decode_from_bytes(&bytes).expect("decode recursive spend bundle");
        assert_eq!(decoded, bundle);
        decoded
            .validate_public_input_binding()
            .expect("decoded recursive spend bundle binding");
        let transition_bytes = to_bytes(&transition_profile1).expect("encode transition profile");
        let decoded_transition: KagemushaRecursiveSpendTransitionProfileV1 =
            norito::decode_from_bytes(&transition_bytes).expect("decode transition profile");
        assert_eq!(decoded_transition, transition_profile1);
        decoded_transition
            .validate_context()
            .expect("decoded transition profile binding");
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn kagemusha_recursive_spend_transition_profile_binds_adversarial_mutations() {
        let chain_id: ChainId = "kagemusha-recursive-spend-transition-chain"
            .parse()
            .expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend-transition");
        let root0 = fixed_hash(b"kagemusha-recursive-transition-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-transition-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-transition-root-2");

        let step0 = kagemusha_step(root0, root1, 0x21, 0x41, b"recursive-transition-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-transition-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0,
            b"recursive-transition-witness-hop-0",
        );
        let accumulator0 =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let previous_proof0 = kagemusha_recursive_spend_proof(&accumulator0);
        let initial_profile =
            kagemusha_recursive_spend_transition_profile_from_initial_evidence(&evidence0, &note0)
                .expect("initial transition profile");
        assert_transition_profile_mutation_changes_or_rejects(
            initial_profile.clone(),
            initial_profile
                .digest()
                .expect("initial transition profile digest"),
            |profile| {
                profile.previous_accumulator_digest =
                    Some(fixed_hash(b"forged-previous-accumulator-on-initial-hop"));
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            initial_profile.clone(),
            initial_profile
                .digest()
                .expect("initial transition profile digest"),
            |profile| {
                profile.previous_accumulator_public_inputs_hash =
                    Some(Hash::new(b"forged-previous-accumulator-pi-on-initial"));
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            initial_profile.clone(),
            initial_profile
                .digest()
                .expect("initial transition profile digest"),
            |profile| {
                profile.previous_recursive_proof_open_envelopes_archive_digest =
                    Some(fixed_hash(b"forged-previous-openings-on-initial"));
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            initial_profile.clone(),
            initial_profile
                .digest()
                .expect("initial transition profile digest"),
            |profile| {
                profile.append_opening_preflight_digest =
                    Some(fixed_hash(b"forged-append-opening-preflight-on-initial"));
            },
        );

        let mut step1 = kagemusha_step(root1, root2, 0x61, 0x81, b"recursive-transition-hop-1");
        step1.input_nullifiers = vec![note0.spend_nullifier];
        let note1 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step1.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-transition-note-1-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence1 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1,
            b"recursive-transition-witness-hop-1",
        );
        let profile = kagemusha_recursive_spend_transition_profile_append_evidence(
            &accumulator0,
            &previous_proof0,
            &evidence1,
            &note1,
        )
        .expect("append transition profile");
        let original_digest = profile.digest().expect("append transition profile digest");
        let mut previous_bundle0 = KagemushaRecursiveSpendBundleV1 {
            accumulator: accumulator0.clone(),
            recursive_proof: previous_proof0.clone(),
        };
        attach_recursive_spend_open_verify_envelope(
            &mut previous_bundle0,
            b"recursive-transition-previous-openings-vk",
        );
        let valid_previous_proof_envelope: crate::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(&previous_bundle0.recursive_proof.proof.bytes)
                .expect("decode valid previous recursive proof envelope");
        let mut mismatched_previous_opening_bundle = previous_bundle0.clone();
        mismatched_previous_opening_bundle
            .recursive_proof
            .public_inputs
            .recursive_proof_chain_digest =
            fixed_hash(b"recursive-transition-previous-opening-mismatched-proof-chain");
        mismatched_previous_opening_bundle
            .recursive_proof
            .public_inputs_hash = mismatched_previous_opening_bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("mismatched previous opening proof public-input hash");
        assert!(matches!(
            kagemusha_recursive_previous_proof_open_envelope_domain_tag(
                &mismatched_previous_opening_bundle,
                &valid_previous_proof_envelope,
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_proof_chain_digest"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_previous_proof_open_envelope_metadata(
                &mismatched_previous_opening_bundle
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_proof_chain_digest"
            })
        ));
        let previous_openings_archive =
            kagemusha_recursive_spend_previous_proof_open_envelope_archive(&previous_bundle0, 0x94);
        let profile_with_previous_openings =
            kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                &evidence1,
                &note1,
            )
            .expect("append transition profile with previous proof openings");
        assert!(matches!(
            kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                b"not a norito previous-proof opening archive",
                &evidence1,
                &note1,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            }),
        ));
        assert!(matches!(
            kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &kagemusha_recursive_spend_pallas_open_envelope_archive(0x95),
                &evidence1,
                &note1,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive.vk_commitment"
            }),
        ));
        let mut previous_bundle_with_bad_circuit_id = previous_bundle0.clone();
        let mut previous_proof_envelope: crate::zk::OpenVerifyEnvelope = norito::decode_from_bytes(
            &previous_bundle_with_bad_circuit_id
                .recursive_proof
                .proof
                .bytes,
        )
        .expect("decode previous recursive proof envelope");
        previous_proof_envelope.circuit_id =
            "forged-previous-recursive-proof-circuit-id".to_owned();
        previous_bundle_with_bad_circuit_id
            .recursive_proof
            .proof
            .bytes = to_bytes(&previous_proof_envelope)
            .expect("encode previous recursive proof envelope with forged circuit id");
        assert!(matches!(
            kagemusha_recursive_previous_proof_open_envelope_metadata(
                &previous_bundle_with_bad_circuit_id
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_bundle.recursive_proof.proof.circuit_id"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_transition_profile_append_evidence_with_previous_proof_openings(
                &accumulator0,
                &previous_bundle_with_bad_circuit_id.recursive_proof,
                &previous_openings_archive,
                &evidence1,
                &note1,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_bundle.recursive_proof.proof.circuit_id"
            })
        ));
        let original_digest_with_previous_openings = profile_with_previous_openings
            .digest()
            .expect("append transition profile with previous openings digest");
        assert_transition_profile_mutation_changes_or_rejects(
            profile_with_previous_openings.clone(),
            original_digest_with_previous_openings,
            |profile| {
                profile
                    .previous_recursive_proof_open_envelopes_archive_digest
                    .as_mut()
                    .expect("previous proof openings archive digest")[0] ^= 0x01;
            },
        );
        let mut zero_previous_openings_digest = profile_with_previous_openings.clone();
        zero_previous_openings_digest.previous_recursive_proof_open_envelopes_archive_digest =
            Some([0u8; Hash::LENGTH]);
        assert!(matches!(
            zero_previous_openings_digest.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "previous_recursive_proof_open_envelopes_archive_digest"
            })
        ));
        assert_transition_profile_mutation_changes_or_rejects(
            zero_previous_openings_digest,
            original_digest_with_previous_openings,
            |_| {},
        );
        let append_opening_preflight_digest =
            fixed_hash(b"recursive-transition-append-opening-preflight");
        let profile_with_append_opening_preflight =
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                append_opening_preflight_digest,
                &evidence1,
                &note1,
            )
            .expect("append transition profile with append opening preflight");
        assert_eq!(
            profile_with_append_opening_preflight.append_opening_preflight_digest,
            Some(append_opening_preflight_digest)
        );
        let append_preflight_digest = profile_with_append_opening_preflight
            .digest()
            .expect("append transition profile with append opening preflight digest");
        assert_ne!(
            append_preflight_digest, original_digest_with_previous_openings,
            "append opening preflight must be transition-profile visible"
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile_with_append_opening_preflight.clone(),
            append_preflight_digest,
            |profile| {
                profile
                    .append_opening_preflight_digest
                    .as_mut()
                    .expect("append opening preflight digest")[0] ^= 0x01;
            },
        );
        let mut zero_append_opening_preflight = profile_with_append_opening_preflight.clone();
        zero_append_opening_preflight.append_opening_preflight_digest = Some([0u8; Hash::LENGTH]);
        assert!(matches!(
            zero_append_opening_preflight.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest"
            })
        ));
        let mut detached_append_opening_preflight = profile.clone();
        detached_append_opening_preflight.append_opening_preflight_digest =
            Some(append_opening_preflight_digest);
        assert!(matches!(
            detached_append_opening_preflight.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_opening_preflight_digest"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                [0u8; Hash::LENGTH],
                &evidence1,
                &note1,
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest"
            })
        ));
        let previous_openings_archive_digest = profile_with_previous_openings
            .previous_recursive_proof_open_envelopes_archive_digest
            .expect("previous openings digest");
        let previous_accumulator_digest = profile_with_previous_openings
            .previous_accumulator_digest
            .expect("previous accumulator digest");
        let previous_proof_artifact_digest =
            kagemusha_recursive_spend_proof_artifact_digest(&previous_bundle0.recursive_proof)
                .expect("attached previous proof artifact digest");
        let previous_opening_preflight = kagemusha_recursive_verifier_preflight_for_evidence(
            &evidence1,
            fixed_hash(b"recursive-transition-previous-proof-opening-preflight"),
        );
        let current_opening_preflight = kagemusha_recursive_verifier_preflight_for_evidence(
            &evidence1,
            evidence1.verifier_witness_batch_digest,
        );
        let append_opening_preflight_contract =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                previous_opening_preflight.clone(),
                current_opening_preflight.clone(),
                previous_accumulator_digest,
                previous_proof_artifact_digest,
                previous_openings_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect("append opening preflight contract");
        let profile_with_append_opening_contract =
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                append_opening_preflight_contract.clone(),
                &evidence1,
                &note1,
            )
            .expect("append transition profile with full opening preflight contract");
        assert_eq!(
            profile_with_append_opening_contract
                .append_opening_preflight
                .as_ref(),
            Some(&append_opening_preflight_contract)
        );
        let append_contract_profile_digest = profile_with_append_opening_contract
            .digest()
            .expect("append transition profile with full opening preflight contract digest");
        let append_boundary =
            kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
                &profile_with_append_opening_contract,
            )
            .expect("append boundary from full opening preflight contract");
        let append_boundary_digest = append_boundary
            .digest()
            .expect("append boundary digest validates");
        assert_ne!(
            append_boundary_digest, append_contract_profile_digest,
            "append boundary uses a separate digest domain from full transition profiles"
        );
        append_boundary
            .validate_against_transition_profile(&profile_with_append_opening_contract)
            .expect("append boundary matches its source transition profile");

        fn refresh_append_boundary_digest(
            boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1,
        ) {
            boundary.append_boundary_digest =
                kagemusha_recursive_spend_lineage_append_boundary_digest_unchecked(boundary)
                    .expect("refreshed append boundary digest");
        }

        fn assert_self_consistent_forged_boundary_rejected(
            source: &KagemushaRecursiveSpendLineageAppendBoundaryV1,
            profile: &KagemushaRecursiveSpendTransitionProfileV1,
            expected_field: &'static str,
            mutate: impl FnOnce(&mut KagemushaRecursiveSpendLineageAppendBoundaryV1),
        ) {
            let mut boundary = source.clone();
            mutate(&mut boundary);
            refresh_append_boundary_digest(&mut boundary);
            let err = boundary
                .validate_against_transition_profile(profile)
                .expect_err("self-consistent forged append boundary must not match profile");
            assert!(
                matches!(
                    err,
                    KagemushaFoldError::RecursiveSpendPublicInputMismatch { field }
                    if field == expected_field
                ),
                "unexpected append-boundary mismatch for {expected_field}: {err:?}"
            );
        }

        let forged_boundary_cases: [(
            &'static str,
            fn(&mut KagemushaRecursiveSpendLineageAppendBoundaryV1),
        ); 12] = [
            (
                "append_boundary.transition_profile_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.transition_profile_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-profile-digest");
                },
            ),
            (
                "append_boundary.transition_profile_binding_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.transition_profile_binding_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-profile-binding");
                },
            ),
            (
                "append_boundary.chain_asset_binding_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.chain_asset_binding_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-chain-asset");
                },
            ),
            (
                "append_boundary.final_note_binding_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.final_note_binding_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-final-note");
                },
            ),
            (
                "append_boundary.previous_recursive_proof_artifact_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.previous_recursive_proof_artifact_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-previous-artifact");
                },
            ),
            (
                "append_boundary.previous_recursive_proof_open_envelopes_archive_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.previous_recursive_proof_open_envelopes_archive_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-previous-openings");
                },
            ),
            (
                "append_boundary.previous_recursive_proof_opening_aggregate_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.previous_recursive_proof_opening_aggregate_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-previous-aggregate");
                },
            ),
            (
                "append_boundary.current_hop_proof_hash",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.current_hop_proof_hash =
                        Hash::new(b"self-consistent-forged-append-boundary-current-proof");
                },
            ),
            (
                "append_boundary.resulting_accumulator_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.resulting_accumulator_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-result-accumulator");
                },
            ),
            (
                "append_boundary.verifier_opening_len",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.verifier_opening_len = 8;
                },
            ),
            (
                "append_boundary.fixed_window_table_schedule_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.fixed_window_table_schedule_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-schedule");
                },
            ),
            (
                "append_boundary.fixed_window_shared_table_manifest_digest",
                |boundary: &mut KagemushaRecursiveSpendLineageAppendBoundaryV1| {
                    boundary.fixed_window_shared_table_manifest_digest =
                        fixed_hash(b"self-consistent-forged-append-boundary-shared-table");
                },
            ),
        ];
        for (expected_field, mutate) in forged_boundary_cases {
            assert_self_consistent_forged_boundary_rejected(
                &append_boundary,
                &profile_with_append_opening_contract,
                expected_field,
                mutate,
            );
        }

        let mut self_consistent_forged_previous = append_boundary.clone();
        self_consistent_forged_previous.previous_accumulator_digest =
            fixed_hash(b"self-consistent-forged-append-boundary-previous");
        refresh_append_boundary_digest(&mut self_consistent_forged_previous);
        assert!(matches!(
            self_consistent_forged_previous
                .validate_against_transition_profile(&profile_with_append_opening_contract),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.previous_accumulator_digest"
            })
        ));

        let mut self_consistent_forged_opening = append_boundary.clone();
        self_consistent_forged_opening.append_opening_preflight_digest =
            fixed_hash(b"self-consistent-forged-append-boundary-opening");
        refresh_append_boundary_digest(&mut self_consistent_forged_opening);
        assert!(matches!(
            self_consistent_forged_opening
                .validate_against_transition_profile(&profile_with_append_opening_contract),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.append_opening_preflight_digest"
            })
        ));

        let mut self_consistent_forged_current_opening = append_boundary.clone();
        self_consistent_forged_current_opening.current_hop_opening_aggregate_digest =
            fixed_hash(b"self-consistent-forged-append-boundary-current-open");
        refresh_append_boundary_digest(&mut self_consistent_forged_current_opening);
        assert!(matches!(
            self_consistent_forged_current_opening
                .validate_against_transition_profile(&profile_with_append_opening_contract),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.current_hop_opening_aggregate_digest"
            })
        ));

        let mut self_consistent_forged_public_inputs = append_boundary.clone();
        self_consistent_forged_public_inputs.resulting_public_inputs_hash =
            Hash::new(b"self-consistent-forged-append-boundary-public-inputs");
        refresh_append_boundary_digest(&mut self_consistent_forged_public_inputs);
        assert!(matches!(
            self_consistent_forged_public_inputs
                .validate_against_transition_profile(&profile_with_append_opening_contract),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.resulting_public_inputs_hash"
            })
        ));

        let mut self_consistent_forged_verifier_context = append_boundary.clone();
        self_consistent_forged_verifier_context.verifier_params_fingerprint =
            fixed_hash(b"self-consistent-forged-append-boundary-params");
        refresh_append_boundary_digest(&mut self_consistent_forged_verifier_context);
        assert!(matches!(
            self_consistent_forged_verifier_context
                .validate_against_transition_profile(&profile_with_append_opening_contract),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.verifier_params_fingerprint"
            })
        ));

        let mut self_consistent_forged_hop_count = append_boundary.clone();
        self_consistent_forged_hop_count.hop_count += 1;
        refresh_append_boundary_digest(&mut self_consistent_forged_hop_count);
        assert!(matches!(
            self_consistent_forged_hop_count
                .validate_against_transition_profile(&profile_with_append_opening_contract),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.hop_count"
            })
        ));

        let mut stale_append_boundary = append_boundary.clone();
        stale_append_boundary.current_hop_opening_aggregate_digest[0] ^= 0x01;
        assert!(matches!(
            stale_append_boundary.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.append_boundary_digest"
            })
        ));
        let mut zero_append_boundary = append_boundary.clone();
        zero_append_boundary.previous_recursive_proof_opening_aggregate_digest =
            [0u8; Hash::LENGTH];
        assert!(matches!(
            zero_append_boundary.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.previous_recursive_proof_opening_aggregate_digest"
            })
        ));
        let mut zero_chain_asset_boundary = append_boundary.clone();
        zero_chain_asset_boundary.chain_asset_binding_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            zero_chain_asset_boundary.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.chain_asset_binding_digest"
            })
        ));
        let mut zero_final_note_boundary = append_boundary.clone();
        zero_final_note_boundary.final_note_binding_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            zero_final_note_boundary.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.final_note_binding_digest"
            })
        ));
        let mut stale_chain_asset_boundary = append_boundary.clone();
        stale_chain_asset_boundary.chain_asset_binding_digest[0] ^= 0x01;
        assert!(matches!(
            stale_chain_asset_boundary.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.append_boundary_digest"
            })
        ));
        let mut stale_final_note_boundary = append_boundary.clone();
        stale_final_note_boundary.final_note_binding_digest[0] ^= 0x01;
        assert!(matches!(
            stale_final_note_boundary.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary.append_boundary_digest"
            })
        ));
        let mut one_hop_append_boundary = append_boundary.clone();
        one_hop_append_boundary.hop_count = 1;
        one_hop_append_boundary.append_boundary_digest =
            kagemusha_recursive_spend_lineage_append_boundary_digest_unchecked(
                &one_hop_append_boundary,
            )
            .unwrap_or([0u8; Hash::LENGTH]);
        assert!(matches!(
            one_hop_append_boundary.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_boundary.hop_count"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
                &profile_with_append_opening_preflight
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_opening_preflight"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(&profile),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_opening_preflight"
            })
        ));
        assert_transition_profile_mutation_changes_or_rejects(
            profile_with_append_opening_contract.clone(),
            append_contract_profile_digest,
            |profile| {
                profile
                    .append_opening_preflight
                    .as_mut()
                    .expect("append opening preflight contract")
                    .current_hop_preflight
                    .aggregate_digest[0] ^= 0x01;
            },
        );
        let mut stale_contract = append_opening_preflight_contract.clone();
        stale_contract.current_hop_proof_hash = Hash::new(b"forged-current-hop-proof-hash");
        assert!(matches!(
            stale_contract.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight.append_opening_preflight_digest"
            })
        ));
        let forged_current_hash_contract =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                previous_opening_preflight.clone(),
                current_opening_preflight.clone(),
                previous_accumulator_digest,
                previous_proof_artifact_digest,
                previous_openings_archive_digest,
                Hash::new(b"valid-but-wrong-current-hop-proof-hash"),
            )
            .expect("forged current-hop hash contract with refreshed digest");
        assert!(matches!(
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                forged_current_hash_contract,
                &evidence1,
                &note1,
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight.current_hop_proof_hash"
            })
        ));
        let forged_previous_openings_contract =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                previous_opening_preflight.clone(),
                current_opening_preflight.clone(),
                previous_accumulator_digest,
                previous_proof_artifact_digest,
                fixed_hash(b"valid-but-wrong-previous-opening-archive-digest"),
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect("forged previous opening digest contract with refreshed digest");
        assert!(matches!(
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                forged_previous_openings_contract,
                &evidence1,
                &note1,
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field:
                    "append_opening_preflight.previous_recursive_proof_open_envelopes_archive_digest"
            })
        ));
        let mut forged_previous_opening_len = previous_opening_preflight.clone();
        forged_previous_opening_len.opening_len = 8;
        let forged_previous_opening_len_err =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                forged_previous_opening_len,
                current_opening_preflight.clone(),
                previous_accumulator_digest,
                previous_proof_artifact_digest,
                previous_openings_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect_err("forged previous opening length contract must fail shared-context checks");
        assert!(matches!(
            forged_previous_opening_len_err,
            KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                field: "append_opening_preflight.shared_opening_len"
            }
        ));
        let mut forged_previous_params = previous_opening_preflight.clone();
        forged_previous_params.params_fingerprint =
            fixed_hash(b"valid-but-wrong-previous-proof-params");
        let forged_previous_params_err =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                forged_previous_params,
                current_opening_preflight.clone(),
                previous_accumulator_digest,
                previous_proof_artifact_digest,
                previous_openings_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect_err("forged previous params contract must fail shared-context checks");
        assert!(matches!(
            forged_previous_params_err,
            KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                field: "append_opening_preflight.shared_params_fingerprint"
            }
        ));
        let mut forged_previous_schedule = previous_opening_preflight.clone();
        forged_previous_schedule.fixed_window_table_schedule_digest =
            fixed_hash(b"valid-but-wrong-previous-proof-schedule");
        let forged_previous_schedule_err =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                forged_previous_schedule,
                current_opening_preflight.clone(),
                previous_accumulator_digest,
                previous_proof_artifact_digest,
                previous_openings_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect_err("forged previous schedule contract must fail shared-context checks");
        assert!(matches!(
            forged_previous_schedule_err,
            KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                field: "append_opening_preflight.shared_fixed_window_table_schedule_digest"
            }
        ));
        let mut forged_previous_manifest = previous_opening_preflight.clone();
        forged_previous_manifest.fixed_window_shared_table_manifest_digest =
            fixed_hash(b"valid-but-wrong-previous-proof-manifest");
        let forged_previous_manifest_err =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                forged_previous_manifest,
                current_opening_preflight.clone(),
                previous_accumulator_digest,
                previous_proof_artifact_digest,
                previous_openings_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect_err("forged previous manifest contract must fail shared-context checks");
        assert!(matches!(
            forged_previous_manifest_err,
            KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                field: "append_opening_preflight.shared_fixed_window_table_manifest_digest"
            }
        ));
        let mut forged_current_preflight = current_opening_preflight.clone();
        forged_current_preflight.aggregate_digest =
            fixed_hash(b"valid-but-wrong-current-hop-preflight-aggregate");
        let forged_current_preflight_contract =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                previous_opening_preflight.clone(),
                forged_current_preflight,
                previous_accumulator_digest,
                previous_proof_artifact_digest,
                previous_openings_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect("forged current-hop preflight contract with refreshed digest");
        assert!(matches!(
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
                &accumulator0,
                &previous_bundle0.recursive_proof,
                &previous_openings_archive,
                forged_current_preflight_contract,
                &evidence1,
                &note1,
            ),
            Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                field: "append_opening_preflight.current_hop_verifier_witness_batch_digest"
            })
        ));
        let mut mismatched_params_preflight = current_opening_preflight.clone();
        mismatched_params_preflight.params_fingerprint =
            fixed_hash(b"mismatched-current-hop-preflight-params");
        let mismatched_params_err = KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
            previous_opening_preflight,
            mismatched_params_preflight,
            previous_accumulator_digest,
            previous_proof_artifact_digest,
            previous_openings_archive_digest,
            evidence1.aggregation_statement.steps[0].proof_hash,
        )
        .expect_err("mismatched current-hop params contract must fail shared-context checks");
        assert!(matches!(
            mismatched_params_err,
            KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                field: "append_opening_preflight.shared_params_fingerprint"
            }
        ));
        let mut detached_append_contract = profile_with_previous_openings.clone();
        detached_append_contract.append_opening_preflight = Some(append_opening_preflight_contract);
        assert!(matches!(
            detached_append_contract.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_opening_preflight_digest"
            })
        ));

        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.domain.push_str(":forged"),
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.hop_count += 1,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.previous_accumulator_digest = None,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile
                    .previous_accumulator_digest
                    .as_mut()
                    .expect("previous accumulator digest")[0] ^= 0x01;
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile
                    .previous_lineage_digest
                    .as_mut()
                    .expect("previous lineage digest")[0] ^= 0x01;
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile
                    .previous_recursive_proof_chain_digest
                    .as_mut()
                    .expect("previous proof-chain digest")[0] ^= 0x01;
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile
                    .previous_recursive_proof_artifact_digest
                    .as_mut()
                    .expect("previous proof artifact digest")[0] ^= 0x01;
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.previous_accumulator_public_inputs_hash = None,
        );
        let mut forged_previous_accumulator_pi = profile.clone();
        forged_previous_accumulator_pi.previous_accumulator_public_inputs_hash =
            Some(Hash::new(b"forged-previous-accumulator-public-inputs"));
        assert!(matches!(
            forged_previous_accumulator_pi.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "previous_accumulator_public_inputs_hash"
            })
        ));
        assert_transition_profile_mutation_changes_or_rejects(
            forged_previous_accumulator_pi,
            original_digest,
            |_| {},
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile.previous_recursive_proof_public_inputs_hash =
                    Some(Hash::new(b"forged-previous-public-inputs"));
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile
                    .previous_current_note
                    .as_mut()
                    .expect("previous note")
                    .spend_nullifier = fixed_hash(b"forged-previous-nullifier");
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.current_hop_statement.hop_index = 0,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.current_hop_statement.proof_hash = Hash::new(b"forged-hop-proof"),
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile.current_hop_statement.input_nullifiers[0] =
                    fixed_hash(b"forged-hop-input-nullifier");
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile.current_hop_statement.output_commitments[0] =
                    fixed_hash(b"forged-hop-output-commitment");
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.current_note.note_commitment = fixed_hash(b"forged-current-note"),
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.current_hop_verifier_witness_batch_digest = [0u8; Hash::LENGTH],
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.current_hop_fixed_window_table_base_digest = [0u8; Hash::LENGTH],
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.verifier_params_fingerprint = [0u8; Hash::LENGTH],
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.fixed_window_table_schedule_digest = [0u8; Hash::LENGTH],
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.fixed_window_shared_table_manifest_digest = [0u8; Hash::LENGTH],
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.verifier_opening_len = 3,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_initial_root = fixed_hash(b"forged-initial-root"),
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_final_root = fixed_hash(b"forged-final-root"),
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_lineage_digest[0] ^= 0x01,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_verifier_witness_batch_digest[0] ^= 0x01,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_fixed_window_table_base_digest[0] ^= 0x01,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_recursive_proof_chain_digest[0] ^= 0x01,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_nullifier_digest = Hash::new(b"forged-nullifier-digest"),
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| {
                profile.resulting_output_commitment_digest =
                    Hash::new(b"forged-output-commitment-digest");
            },
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_fold_digest = Hash::new(b"forged-fold-digest"),
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile.clone(),
            original_digest,
            |profile| profile.resulting_accumulator_digest[0] ^= 0x01,
        );
        assert_transition_profile_mutation_changes_or_rejects(
            profile,
            original_digest,
            |profile| profile.resulting_public_inputs_hash = Hash::new(b"forged-public-inputs"),
        );
    }

    #[test]
    fn kagemusha_recursive_spend_lineage_witness_helpers_append_record_backed_material() {
        let chain_id: ChainId = "kagemusha-recursive-spend-lineage-chain"
            .parse()
            .expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend-lineage");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-lineage-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-lineage-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-spend-lineage-root-2");

        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-lineage-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-lineage-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-lineage-witness-hop-0",
        );
        let accumulator0 =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let bundle0 = kagemusha_recursive_spend_bundle(accumulator0);
        let init_record_bundle = kagemusha_recursive_spend_record_bundle_for_step(
            chain_id.clone(),
            asset.clone(),
            &step0,
            "kagemusha-recursive-lineage-hop-0",
            b"recursive-lineage-proof-hop-0",
        );
        let init_request = KagemushaRecursiveSpendInitRequestV1::new(
            init_record_bundle.clone(),
            kagemusha_recursive_spend_lineage_pallas_open_envelope_archive(
                &init_record_bundle,
                0x41,
            ),
            note0.clone(),
        )
        .expect("init request validates before proving");
        let mut missing_init_current_note = init_request.clone();
        missing_init_current_note.current_note.note_commitment =
            fixed_hash(b"recursive-lineage-missing-init-note");
        assert!(matches!(
            missing_init_current_note.validate_public_binding(),
            Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment)
        ));
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_from_init_result(
                &missing_init_current_note,
                &bundle0
            ),
            Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment)
        ));
        let mut init_note_reuses_input = init_request.clone();
        init_note_reuses_input.current_note.spend_nullifier =
            init_note_reuses_input.record_bundle.bundle.steps[0].input_nullifiers[0];
        assert!(matches!(
            init_note_reuses_input.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "current_note.spend_nullifier"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendInitRequestV1::new(
                init_request.record_bundle.clone(),
                vec![0x01, 0x02],
                note0.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive"
            })
        ));
        let witness0 =
            kagemusha_recursive_spend_lineage_witness_from_init_result(&init_request, &bundle0)
                .expect("initial lineage witness");
        assert_eq!(witness0.current_notes, vec![note0.clone()]);
        assert!(witness0.previous_recursive_proofs.is_empty());

        let mut reserved_lineage_init_bundle = bundle0.clone();
        reserved_lineage_init_bundle.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &reserved_lineage_init_bundle.accumulator,
            b"recursive-lineage-init-reserved-scalar",
        );
        let reserved_lineage_witness0 = kagemusha_recursive_spend_lineage_witness_from_init_result(
            &init_request,
            &reserved_lineage_init_bundle,
        )
        .expect("initial lineage witness accepts reserved-lineage final bundle");
        assert_eq!(reserved_lineage_witness0.current_notes, vec![note0.clone()]);
        assert!(
            reserved_lineage_witness0
                .previous_recursive_proofs
                .is_empty()
        );

        let mut stale_init_verifier_count = bundle0.clone();
        stale_init_verifier_count
            .recursive_proof
            .public_inputs
            .verifier_witness_count = 2;
        stale_init_verifier_count.recursive_proof.public_inputs_hash = stale_init_verifier_count
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("stale init verifier-count public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_from_init_result(
                &init_request,
                &stale_init_verifier_count
            ),
            Err(
                KagemushaFoldError::RecursiveAggregationWitnessCountMismatch {
                    expected: 1,
                    actual: 2,
                }
            )
        ));
        let mut stale_init_hop_count = bundle0.clone();
        stale_init_hop_count.recursive_proof.public_inputs.hop_count = 2;
        stale_init_hop_count
            .recursive_proof
            .public_inputs
            .verifier_witness_count = 2;
        stale_init_hop_count.recursive_proof.public_inputs_hash = stale_init_hop_count
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("stale init hop-count public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_from_init_result(
                &init_request,
                &stale_init_hop_count
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch { field: "hop_count" })
        ));

        let mut step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-lineage-hop-1");
        step1.input_nullifiers = vec![note0.spend_nullifier];
        let note1 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step1.output_commitments[1],
            spend_nullifier: fixed_hash(b"recursive-lineage-note-1-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence1 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1.clone(),
            b"recursive-lineage-witness-hop-1",
        );
        let accumulator1 = kagemusha_recursive_spend_accumulator_append_evidence(
            &bundle0.accumulator,
            &bundle0.recursive_proof,
            &evidence1,
            &note1,
        )
        .expect("append recursive spend accumulator");
        let bundle1 = kagemusha_recursive_spend_bundle(accumulator1);
        let mut previous_append_boundary_bundle = bundle0.clone();
        attach_recursive_spend_open_verify_envelope(
            &mut previous_append_boundary_bundle,
            b"recursive-lineage-append-boundary-previous-openings-vk",
        );
        let previous_append_boundary_openings_archive =
            kagemusha_recursive_spend_previous_proof_open_envelope_archive(
                &previous_append_boundary_bundle,
                0x5a,
            );
        let previous_append_boundary_openings_archive_digest =
            kagemusha_recursive_previous_proof_open_envelopes_archive_digest(
                &previous_append_boundary_openings_archive,
            )
            .expect("previous append-boundary openings archive digest");
        let append_boundary_preflight_contract =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                kagemusha_recursive_verifier_preflight_for_evidence(
                    &evidence1,
                    fixed_hash(b"recursive-lineage-append-boundary-previous-opening"),
                ),
                kagemusha_recursive_verifier_preflight_for_evidence(
                    &evidence1,
                    evidence1.verifier_witness_batch_digest,
                ),
                kagemusha_recursive_spend_accumulator_digest(
                    &previous_append_boundary_bundle.accumulator,
                )
                .expect("previous append-boundary accumulator digest"),
                kagemusha_recursive_spend_proof_artifact_digest(
                    &previous_append_boundary_bundle.recursive_proof,
                )
                .expect("previous append-boundary proof artifact digest"),
                previous_append_boundary_openings_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect("append-boundary opening preflight contract");
        let accumulator1_with_append_boundary =
            kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract(
                &previous_append_boundary_bundle.accumulator,
                &previous_append_boundary_bundle.recursive_proof,
                &previous_append_boundary_openings_archive,
                append_boundary_preflight_contract,
                &evidence1,
                &note1,
            )
            .expect("append accumulator with canonical append boundary");
        assert_ne!(
            accumulator1_with_append_boundary.append_boundary_digest,
            [0u8; Hash::LENGTH]
        );
        let append_boundary_accumulator = accumulator1_with_append_boundary;
        let lineage_append_boundary_proof = kagemusha_recursive_spend_lineage_proof(
            &append_boundary_accumulator,
            b"recursive-lineage-append-boundary-scalar",
        );
        let lineage_append_boundary_bundle = KagemushaRecursiveSpendBundleV1 {
            accumulator: append_boundary_accumulator,
            recursive_proof: lineage_append_boundary_proof,
        };
        assert_eq!(
            lineage_append_boundary_bundle
                .recursive_proof
                .public_inputs
                .append_boundary_digest,
            lineage_append_boundary_bundle
                .accumulator
                .append_boundary_digest
        );
        lineage_append_boundary_bundle
            .validate_public_input_binding()
            .expect("lineage append proof binds canonical append-boundary public input");
        let mut missing_append_boundary = lineage_append_boundary_bundle.clone();
        missing_append_boundary
            .recursive_proof
            .public_inputs
            .append_boundary_digest = [0u8; Hash::LENGTH];
        missing_append_boundary.recursive_proof.public_inputs_hash = missing_append_boundary
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("missing append boundary public-input hash");
        assert!(matches!(
            missing_append_boundary.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "append_boundary_digest"
            })
        ));
        let mut forged_accumulator_append_boundary = lineage_append_boundary_bundle.clone();
        forged_accumulator_append_boundary
            .accumulator
            .append_boundary_digest[0] ^= 0x01;
        assert!(matches!(
            forged_accumulator_append_boundary.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary_digest"
            })
        ));
        let append_record_bundle = kagemusha_recursive_spend_record_bundle_for_step(
            chain_id,
            asset,
            &step1,
            "kagemusha-recursive-lineage-hop-1",
            b"recursive-lineage-proof-hop-1",
        );
        let append_request = KagemushaRecursiveSpendAppendRequestV1::new(
            bundle0.clone(),
            append_record_bundle.clone(),
            kagemusha_recursive_spend_lineage_pallas_open_envelope_archive(
                &append_record_bundle,
                0x51,
            ),
            note1.clone(),
        )
        .expect("append request validates before proving");
        assert_eq!(
            append_request.output_proof_circuit_id(),
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        );
        assert_eq!(
            append_request.output_proof_circuit_id,
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        );
        let mut append_with_unsupported_output_circuit = append_request.clone();
        append_with_unsupported_output_circuit.output_proof_circuit_id =
            "kagemusha-recursive-spend-unsupported-output-v1".to_owned();
        assert!(matches!(
            append_with_unsupported_output_circuit.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "output_proof_circuit_id"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness_and_output_circuit(
                "kagemusha-recursive-spend-unsupported-output-v1",
                append_request.previous_bundle.clone(),
                None,
                Vec::new(),
                append_request.record_bundle.clone(),
                append_request.pallas_open_envelopes_archive.clone(),
                append_request.current_note.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "output_proof_circuit_id"
            })
        ));
        let mut append_request_with_enveloped_previous = append_request.clone();
        attach_recursive_spend_open_verify_envelope(
            &mut append_request_with_enveloped_previous.previous_bundle,
            b"recursive-lineage-append-request-semantic-vk",
        );
        let previous_proof_open_envelopes_archive =
            kagemusha_recursive_spend_previous_proof_open_envelope_archive(
                &append_request_with_enveloped_previous.previous_bundle,
                0x71,
            );
        let mut reserved_output_append_missing_previous_proof_open_envelopes =
            append_request_with_enveloped_previous.clone();
        reserved_output_append_missing_previous_proof_open_envelopes.output_proof_circuit_id =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.to_owned();
        assert!(matches!(
            reserved_output_append_missing_previous_proof_open_envelopes.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            })
        ));
        let mut reserved_output_append_with_previous_proof_open_envelopes =
            reserved_output_append_missing_previous_proof_open_envelopes.clone();
        reserved_output_append_with_previous_proof_open_envelopes
            .previous_recursive_proof_open_envelopes_archive =
            previous_proof_open_envelopes_archive.clone();
        assert!(matches!(
            reserved_output_append_with_previous_proof_open_envelopes.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "output_proof_circuit_id"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness_and_output_circuit(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                append_request_with_enveloped_previous.previous_bundle.clone(),
                None,
                previous_proof_open_envelopes_archive.clone(),
                append_request_with_enveloped_previous.record_bundle.clone(),
                append_request_with_enveloped_previous.pallas_open_envelopes_archive.clone(),
                append_request_with_enveloped_previous.current_note.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "output_proof_circuit_id"
            })
        ));
        let mut capped_lineage_previous = lineage_append_boundary_bundle.clone();
        capped_lineage_previous.accumulator.hop_count =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1;
        capped_lineage_previous.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &capped_lineage_previous.accumulator,
            b"recursive-lineage-capped-previous-scalar",
        );
        attach_recursive_spend_open_verify_envelope(
            &mut capped_lineage_previous,
            b"recursive-lineage-capped-previous-vk",
        );
        let capped_lineage_previous_openings =
            kagemusha_recursive_spend_previous_proof_open_envelope_archive(
                &capped_lineage_previous,
                0x72,
            );
        assert!(
            matches!(
                KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness_and_output_circuit(
                    KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                    capped_lineage_previous,
                    Some(kagemusha_recursive_spend_active_lineage_verifier_record()),
                    capped_lineage_previous_openings,
                    append_request.record_bundle.clone(),
                    append_request.pallas_open_envelopes_archive.clone(),
                    append_request.current_note.clone(),
                ),
                Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                    field: "output_proof_circuit_id"
                })
            ),
            "Reserved-lineage append request at the witnessless hop cap must reject before proving"
        );
        let previous_proof_open_envelope: iroha_zkp_halo2::OpenVerifyEnvelope =
            norito::decode_from_bytes::<Vec<iroha_zkp_halo2::OpenVerifyEnvelope>>(
                &previous_proof_open_envelopes_archive,
            )
            .expect("decode one previous proof open-envelope archive")
            .into_iter()
            .next()
            .expect("one previous proof open-envelope");
        for (case, expected_field) in [
            (
                "vk_commitment",
                "previous_recursive_proof_open_envelopes_archive.vk_commitment",
            ),
            (
                "public_inputs_schema_hash",
                "previous_recursive_proof_open_envelopes_archive.public_inputs_schema_hash",
            ),
            (
                "domain_tag",
                "previous_recursive_proof_open_envelopes_archive.domain_tag",
            ),
        ] {
            let mut mismatched_previous_proof_open_envelopes =
                vec![previous_proof_open_envelope.clone()];
            match case {
                "vk_commitment" => {
                    mismatched_previous_proof_open_envelopes[0].vk_commitment =
                        Some(fixed_hash(b"forged-previous-proof-opening-vk"));
                }
                "public_inputs_schema_hash" => {
                    mismatched_previous_proof_open_envelopes[0].public_inputs_schema_hash =
                        Some(fixed_hash(b"forged-previous-proof-opening-schema"));
                }
                "domain_tag" => {
                    mismatched_previous_proof_open_envelopes[0].domain_tag =
                        Some(fixed_hash(b"forged-previous-proof-opening-domain-tag"));
                }
                _ => unreachable!("covered previous proof opening metadata case"),
            }
            let mut reserved_output_append_with_metadata_mismatch =
                reserved_output_append_missing_previous_proof_open_envelopes.clone();
            reserved_output_append_with_metadata_mismatch
                .previous_recursive_proof_open_envelopes_archive =
                to_bytes(&mismatched_previous_proof_open_envelopes)
                    .expect("encode metadata-mismatch previous proof envelope archive");
            let err = reserved_output_append_with_metadata_mismatch
                .validate_public_binding()
                .expect_err("previous proof opening metadata mismatch must reject");
            assert!(
                matches!(
                    err,
                    KagemushaFoldError::InvalidRecursiveSpendProof { field }
                        if field == expected_field
                ),
                "{case} mismatch returned unexpected error: {err:?}"
            );
        }
        let mut reserved_output_append_with_stale_previous_proof_payload =
            reserved_output_append_missing_previous_proof_open_envelopes.clone();
        reserved_output_append_with_stale_previous_proof_payload
            .previous_recursive_proof_open_envelopes_archive =
            previous_proof_open_envelopes_archive.clone();
        let mut stale_previous_proof_envelope: crate::zk::OpenVerifyEnvelope =
            norito::decode_from_bytes(
                &reserved_output_append_with_stale_previous_proof_payload
                    .previous_bundle
                    .recursive_proof
                    .proof
                    .bytes,
            )
            .expect("decode previous recursive proof envelope");
        stale_previous_proof_envelope.proof_bytes.push(0x42);
        reserved_output_append_with_stale_previous_proof_payload
            .previous_bundle
            .recursive_proof
            .proof
            .bytes = to_bytes(&stale_previous_proof_envelope)
            .expect("encode stale previous recursive proof envelope");
        assert!(matches!(
            reserved_output_append_with_stale_previous_proof_payload.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive.domain_tag"
            })
        ));
        let over_count_previous_proof_open_envelopes_archive = to_bytes(&vec![
            previous_proof_open_envelope.clone(),
            previous_proof_open_envelope,
        ])
        .expect("encode over-count previous proof open-envelope archive");
        let mut reserved_output_append_with_over_count_previous_proof_open_envelopes =
            reserved_output_append_missing_previous_proof_open_envelopes.clone();
        reserved_output_append_with_over_count_previous_proof_open_envelopes
            .previous_recursive_proof_open_envelopes_archive =
            over_count_previous_proof_open_envelopes_archive.clone();
        assert!(matches!(
            reserved_output_append_with_over_count_previous_proof_open_envelopes
                .validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness_and_output_circuit(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                append_request_with_enveloped_previous.previous_bundle.clone(),
                None,
                over_count_previous_proof_open_envelopes_archive,
                append_request_with_enveloped_previous.record_bundle.clone(),
                append_request_with_enveloped_previous.pallas_open_envelopes_archive.clone(),
                append_request_with_enveloped_previous.current_note.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            })
        ));
        let mut semantic_append_with_lineage_record = append_request.clone();
        semantic_append_with_lineage_record.previous_lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        assert!(matches!(
            semantic_append_with_lineage_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_lineage_verifier_record"
            })
        ));
        let mut reserved_append_missing_lineage_record = append_request.clone();
        reserved_append_missing_lineage_record
            .previous_bundle
            .recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &reserved_append_missing_lineage_record
                .previous_bundle
                .accumulator,
            b"recursive-lineage-append-request-reserved-scalar",
        );
        attach_recursive_spend_open_verify_envelope(
            &mut reserved_append_missing_lineage_record.previous_bundle,
            b"recursive-lineage-append-request-reserved-vk",
        );
        assert!(matches!(
            reserved_append_missing_lineage_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_lineage_verifier_record"
            })
        ));
        let mut reserved_append_with_lineage_record =
            reserved_append_missing_lineage_record.clone();
        reserved_append_with_lineage_record.previous_lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        reserved_append_with_lineage_record
            .validate_public_binding()
            .expect("reserved previous append request accepts active lineage verifier record");
        let mut reserved_append_with_previous_proof_open_envelopes =
            reserved_append_with_lineage_record.clone();
        let reserved_previous_proof_open_envelopes_archive =
            kagemusha_recursive_spend_previous_proof_open_envelope_archive(
                &reserved_append_with_lineage_record.previous_bundle,
                0x72,
            );
        reserved_append_with_previous_proof_open_envelopes
            .previous_recursive_proof_open_envelopes_archive =
            reserved_previous_proof_open_envelopes_archive.clone();
        reserved_append_with_previous_proof_open_envelopes
            .validate_public_binding()
            .expect("reserved previous append request accepts previous proof open envelopes");
        let mut reserved_previous_reserved_output_append =
            reserved_append_with_previous_proof_open_envelopes.clone();
        reserved_previous_reserved_output_append.output_proof_circuit_id =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1.to_owned();
        reserved_previous_reserved_output_append
            .validate_public_binding()
            .expect("reserved previous append request accepts structurally valid reserved output");
        let reserved_output_append_from_builder =
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness_and_output_circuit(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
                reserved_append_with_lineage_record.previous_bundle.clone(),
                reserved_append_with_lineage_record
                    .previous_lineage_verifier_record
                    .clone(),
                reserved_previous_proof_open_envelopes_archive.clone(),
                reserved_append_with_lineage_record.record_bundle.clone(),
                reserved_append_with_lineage_record
                    .pallas_open_envelopes_archive
                    .clone(),
                reserved_append_with_lineage_record.current_note.clone(),
            )
            .expect("reserved previous builder accepts structurally valid reserved output");
        assert_eq!(
            reserved_output_append_from_builder.output_proof_circuit_id(),
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
        );
        let reserved_append_from_builder =
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness(
                reserved_append_with_lineage_record.previous_bundle.clone(),
                reserved_append_with_lineage_record
                    .previous_lineage_verifier_record
                    .clone(),
                reserved_previous_proof_open_envelopes_archive.clone(),
                reserved_append_with_lineage_record.record_bundle.clone(),
                reserved_append_with_lineage_record
                    .pallas_open_envelopes_archive
                    .clone(),
                reserved_append_with_lineage_record.current_note.clone(),
            )
            .expect("reserved append builder accepts previous proof open envelopes");
        assert_eq!(
            reserved_append_from_builder.previous_recursive_proof_open_envelopes_archive,
            reserved_previous_proof_open_envelopes_archive
        );
        {
            let expect_malformed_previous_proof_opening_shape =
                |case: &str, mutate: fn(&mut iroha_zkp_halo2::OpenVerifyEnvelope)| {
                    let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                        norito::decode_from_bytes(&reserved_previous_proof_open_envelopes_archive)
                            .expect("decode previous-proof opening archive");
                    mutate(
                        envelopes
                            .first_mut()
                            .expect("fixture has one previous-proof opening"),
                    );
                    let mut malformed = reserved_append_with_lineage_record.clone();
                    malformed.previous_recursive_proof_open_envelopes_archive =
                        to_bytes(&envelopes)
                            .expect("encode malformed previous-proof opening archive");
                    let err = malformed
                        .validate_public_binding()
                        .expect_err("malformed previous-proof opening shape must reject");
                    assert!(
                        matches!(
                            err,
                            KagemushaFoldError::InvalidRecursiveSpendProof { field }
                                if field == "previous_recursive_proof_open_envelopes_archive"
                        ),
                        "{case} returned unexpected error: {err:?}"
                    );
                };
            expect_malformed_previous_proof_opening_shape(
                "empty previous-proof opening transcript label",
                |envelope| envelope.transcript_label.clear(),
            );
            expect_malformed_previous_proof_opening_shape(
                "non-Pallas previous-proof opening curve id",
                |envelope| {
                    envelope.params.curve_id = iroha_zkp_halo2::ZkCurveId::Pasta.as_u16();
                    envelope.public.curve_id = iroha_zkp_halo2::ZkCurveId::Pasta.as_u16();
                },
            );
            expect_malformed_previous_proof_opening_shape(
                "previous-proof opening generator count mismatch",
                |envelope| {
                    envelope.params.g.pop();
                },
            );
            expect_malformed_previous_proof_opening_shape(
                "previous-proof opening IPA round count mismatch",
                |envelope| {
                    envelope.proof.r.push([0xAA; Hash::LENGTH]);
                },
            );
        }
        let mut reserved_append_with_bad_previous_proof_open_envelopes =
            reserved_append_with_lineage_record.clone();
        reserved_append_with_bad_previous_proof_open_envelopes
            .previous_recursive_proof_open_envelopes_archive = vec![0x01, 0x02];
        assert!(matches!(
            reserved_append_with_bad_previous_proof_open_envelopes.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness(
                reserved_append_with_lineage_record.previous_bundle.clone(),
                reserved_append_with_lineage_record
                    .previous_lineage_verifier_record
                    .clone(),
                vec![0x01, 0x02],
                reserved_append_with_lineage_record.record_bundle.clone(),
                reserved_append_with_lineage_record
                    .pallas_open_envelopes_archive
                    .clone(),
                reserved_append_with_lineage_record.current_note.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            })
        ));
        let oversized_previous_proof_open_envelopes =
            vec![0x42; KAGEMUSHA_RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES + 1];
        let mut reserved_append_with_oversized_previous_proof_open_envelopes =
            reserved_append_with_lineage_record.clone();
        reserved_append_with_oversized_previous_proof_open_envelopes
            .previous_recursive_proof_open_envelopes_archive =
            oversized_previous_proof_open_envelopes.clone();
        assert!(matches!(
            reserved_append_with_oversized_previous_proof_open_envelopes.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness(
                reserved_append_with_lineage_record.previous_bundle.clone(),
                reserved_append_with_lineage_record
                    .previous_lineage_verifier_record
                    .clone(),
                oversized_previous_proof_open_envelopes,
                reserved_append_with_lineage_record.record_bundle.clone(),
                reserved_append_with_lineage_record
                    .pallas_open_envelopes_archive
                    .clone(),
                reserved_append_with_lineage_record.current_note.clone(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            })
        ));
        let mut reserved_append_with_empty_previous_proof_open_envelopes_vec =
            reserved_append_with_lineage_record.clone();
        reserved_append_with_empty_previous_proof_open_envelopes_vec
            .previous_recursive_proof_open_envelopes_archive =
            to_bytes::<Vec<iroha_zkp_halo2::OpenVerifyEnvelope>>(&Vec::new())
                .expect("encode empty previous proof envelope archive");
        assert!(matches!(
            reserved_append_with_empty_previous_proof_open_envelopes_vec.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_recursive_proof_open_envelopes_archive"
            })
        ));
        let witness_from_reserved_previous =
            kagemusha_recursive_spend_lineage_witness_append_result(
                &reserved_lineage_witness0,
                &reserved_append_with_previous_proof_open_envelopes,
                &bundle1,
            )
            .expect("append witness accepts reserved-lineage previous recursive proof");
        assert_eq!(
            witness_from_reserved_previous.current_notes,
            vec![note0.clone(), note1.clone()]
        );
        assert_eq!(
            witness_from_reserved_previous.previous_recursive_proofs[0]
                .verifier_key_id
                .name,
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1
        );
        assert_ne!(
            witness_from_reserved_previous.previous_recursive_proofs[0]
                .public_inputs
                .fixed_window_table_base_digest,
            bundle1
                .recursive_proof
                .public_inputs
                .fixed_window_table_base_digest,
            "lineage previous-proof table bases are proof-witness-specific, not shared context"
        );
        let mut reserved_append_with_semantic_record = reserved_append_with_lineage_record.clone();
        reserved_append_with_semantic_record
            .previous_lineage_verifier_record
            .as_mut()
            .expect("lineage record")
            .circuit_id = KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1.to_owned();
        assert!(matches!(
            reserved_append_with_semantic_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_lineage_verifier_record.circuit_id"
            })
        ));
        let mut append_amount_drift = append_request.clone();
        append_amount_drift.current_note.amount = Numeric::new(43, 0);
        assert!(matches!(
            append_amount_drift.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &append_amount_drift,
                &bundle1
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));
        let mut append_missing_previous_nullifier = append_request.clone();
        append_missing_previous_nullifier.record_bundle.bundle.steps[0].input_nullifiers[0] =
            fixed_hash(b"recursive-lineage-append-missing-previous-nullifier");
        assert!(matches!(
            append_missing_previous_nullifier.validate_public_binding(),
            Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier)
        ));
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &append_missing_previous_nullifier,
                &bundle1
            ),
            Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier)
        ));
        let mut append_root_splice = append_request.clone();
        append_root_splice.record_bundle.bundle.steps[0].root_before =
            fixed_hash(b"recursive-lineage-append-root-splice");
        assert!(matches!(
            append_root_splice.validate_public_binding(),
            Err(KagemushaFoldError::RecursiveSpendRootMismatch)
        ));
        let mut append_anchor_output_reuse = append_request.clone();
        append_anchor_output_reuse.record_bundle.bundle.steps[0].output_commitments[0] =
            bundle0.accumulator.topup_anchor_nullifiers[0];
        assert!(matches!(
            append_anchor_output_reuse.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "record_bundle.bundle.steps.output_commitments"
            })
        ));
        let witness1 = kagemusha_recursive_spend_lineage_witness_append_result(
            &witness0,
            &append_request,
            &bundle1,
        )
        .expect("appended lineage witness");
        assert_eq!(witness1.current_notes, vec![note0.clone(), note1.clone()]);
        assert_eq!(
            witness1.previous_recursive_proofs,
            vec![bundle0.recursive_proof.clone()]
        );
        let envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&witness1.pallas_open_envelopes_archive)
                .expect("decode merged Pallas envelope archive");
        assert_eq!(envelopes.len(), 2);

        let mut reserved_lineage_previous_append = append_request.clone();
        reserved_lineage_previous_append
            .previous_bundle
            .recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &reserved_lineage_previous_append.previous_bundle.accumulator,
            b"recursive-lineage-previous-reserved-scalar",
        );
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &reserved_lineage_previous_append,
                &bundle1
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "previous_lineage_verifier_record"
            })
        ));

        let mut reserved_lineage_appended_bundle = bundle1.clone();
        reserved_lineage_appended_bundle.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &reserved_lineage_appended_bundle.accumulator,
            b"recursive-lineage-appended-reserved-scalar",
        );
        let reserved_lineage_witness1 = kagemusha_recursive_spend_lineage_witness_append_result(
            &witness0,
            &append_request,
            &reserved_lineage_appended_bundle,
        )
        .expect("append witness accepts reserved-lineage final bundle");
        assert_eq!(
            reserved_lineage_witness1.current_notes,
            vec![note0.clone(), note1.clone()]
        );

        let mut redeem_proof = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
            VerifyingKeyId::new("halo2/ipa", "kagemusha-unshield-fixture"),
        );
        redeem_proof.vk_commitment = Some(fixed_hash(b"recursive-lineage-unshield-vk"));
        let valid_redeem_request =
            KagemushaRecursiveSpendRedeemRequestV1::new_with_lineage_witness(
                bundle1.clone(),
                sample_account(0xB3, "offline"),
                42,
                redeem_proof.clone(),
                Some(witness1.clone()),
                None,
            )
            .expect("redeem request accepts assembled lineage witness");
        KagemushaRecursiveSpendRedeemRequestV1::new(
            bundle1.clone(),
            sample_account(0xB4, "offline"),
            42,
            redeem_proof.clone(),
        )
        .expect("witnessless semantic redeem request is structurally valid but chain-gated later");
        let mut one_hop_witnessless_lineage_redeem = valid_redeem_request.clone();
        one_hop_witnessless_lineage_redeem.bundle = reserved_lineage_init_bundle.clone();
        one_hop_witnessless_lineage_redeem.lineage_witness = None;
        one_hop_witnessless_lineage_redeem.lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        one_hop_witnessless_lineage_redeem
            .validate_public_binding()
            .expect("one-hop Reserved-lineage redeem is witnessless inside the cap");
        let mut one_hop_witnessless_lineage_missing_record =
            one_hop_witnessless_lineage_redeem.clone();
        one_hop_witnessless_lineage_missing_record.lineage_verifier_record = None;
        assert!(matches!(
            one_hop_witnessless_lineage_missing_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record"
            })
        ));
        let mut one_hop_witnessless_lineage_wrong_record =
            one_hop_witnessless_lineage_redeem.clone();
        one_hop_witnessless_lineage_wrong_record.lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        one_hop_witnessless_lineage_wrong_record
            .lineage_verifier_record
            .as_mut()
            .expect("lineage verifier record")
            .circuit_id = KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.to_owned();
        assert!(matches!(
            one_hop_witnessless_lineage_wrong_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record.circuit_id"
            })
        ));
        let mut two_hop_witnessless_lineage_redeem = valid_redeem_request.clone();
        two_hop_witnessless_lineage_redeem.bundle = reserved_lineage_appended_bundle.clone();
        two_hop_witnessless_lineage_redeem.lineage_witness = None;
        two_hop_witnessless_lineage_redeem.lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        two_hop_witnessless_lineage_redeem
            .validate_public_binding()
            .expect("multi-hop Reserved-lineage redeem is witnessless inside the cap");
        let mut two_hop_record_backed_lineage_redeem = valid_redeem_request.clone();
        two_hop_record_backed_lineage_redeem.bundle = reserved_lineage_appended_bundle.clone();
        two_hop_record_backed_lineage_redeem.lineage_witness =
            Some(reserved_lineage_witness1.clone());
        two_hop_record_backed_lineage_redeem.lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        two_hop_record_backed_lineage_redeem
            .validate_public_binding()
            .expect("multi-hop Reserved-lineage redeem is structurally admissible with record-backed lineage witness");

        let semantic_verify_request = KagemushaRecursiveSpendVerifyRequestV1::new(bundle1.clone())
            .expect("semantic verify request validates without lineage record");
        assert!(semantic_verify_request.lineage_verifier_record.is_none());
        assert!(matches!(
            KagemushaRecursiveSpendVerifyRequestV1::new_with_lineage_verifier_record(
                bundle1.clone(),
                Some(kagemusha_recursive_spend_active_lineage_verifier_record()),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record"
            })
        ));
        let mut lineage_verify_bundle = bundle1.clone();
        lineage_verify_bundle.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &lineage_verify_bundle.accumulator,
            b"recursive-lineage-verify-request-scalar",
        );
        assert!(matches!(
            KagemushaRecursiveSpendVerifyRequestV1::new(lineage_verify_bundle.clone()),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record"
            })
        ));
        let lineage_verify_request =
            KagemushaRecursiveSpendVerifyRequestV1::new_with_lineage_verifier_record(
                lineage_verify_bundle.clone(),
                Some(kagemusha_recursive_spend_active_lineage_verifier_record()),
            )
            .expect("reserved-lineage verify request validates with active lineage record");
        assert!(lineage_verify_request.lineage_verifier_record.is_some());
        let mut forged_commitment_lineage_verify_record =
            kagemusha_recursive_spend_active_lineage_verifier_record();
        forged_commitment_lineage_verify_record.commitment =
            fixed_hash(b"recursive-lineage-forged-verifier-commitment");
        assert!(matches!(
            KagemushaRecursiveSpendVerifyRequestV1::new_with_lineage_verifier_record(
                lineage_verify_bundle.clone(),
                Some(forged_commitment_lineage_verify_record),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record.commitment"
            })
        ));
        let mut inactive_lineage_verify_record =
            kagemusha_recursive_spend_active_lineage_verifier_record();
        inactive_lineage_verify_record.status = ConfidentialStatus::Proposed;
        assert!(matches!(
            KagemushaRecursiveSpendVerifyRequestV1::new_with_lineage_verifier_record(
                lineage_verify_bundle,
                Some(inactive_lineage_verify_record),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record.status"
            })
        ));

        let mut root_discontinuous = valid_redeem_request.clone();
        root_discontinuous
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[1]
            .root_before = fixed_hash(b"recursive-lineage-forged-root-before");
        assert!(matches!(
            root_discontinuous.validate_public_binding(),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));

        let mut duplicate_output_commitment = valid_redeem_request.clone();
        let first_output = duplicate_output_commitment
            .lineage_witness
            .as_ref()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[0]
            .output_commitments[0];
        duplicate_output_commitment
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[1]
            .output_commitments[0] = first_output;
        assert!(matches!(
            duplicate_output_commitment.validate_public_binding(),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));

        let mut topup_anchor_output_reuse = valid_redeem_request.clone();
        let topup_anchor = topup_anchor_output_reuse
            .lineage_witness
            .as_ref()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[0]
            .input_nullifiers[0];
        topup_anchor_output_reuse
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[1]
            .output_commitments[0] = topup_anchor;
        assert!(matches!(
            topup_anchor_output_reuse.validate_public_binding(),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 1 })
        ));

        let mut intermediate_note_nullifier_reuses_output = valid_redeem_request.clone();
        let sibling_output = intermediate_note_nullifier_reuses_output
            .lineage_witness
            .as_ref()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[0]
            .output_commitments[1];
        intermediate_note_nullifier_reuses_output
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .current_notes[0]
            .spend_nullifier = sibling_output;
        assert!(matches!(
            intermediate_note_nullifier_reuses_output.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier"
            })
        ));

        let mut final_note_nullifier_reuses_initial_input = valid_redeem_request.clone();
        let initial_input = final_note_nullifier_reuses_initial_input
            .lineage_witness
            .as_ref()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[0]
            .input_nullifiers[0];
        final_note_nullifier_reuses_initial_input
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .current_notes[1]
            .spend_nullifier = initial_input;
        assert!(matches!(
            final_note_nullifier_reuses_initial_input.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier"
            })
        ));

        let mut final_note_nullifier_reuses_prior_output = valid_redeem_request.clone();
        let prior_sibling_output = final_note_nullifier_reuses_prior_output
            .lineage_witness
            .as_ref()
            .expect("lineage witness")
            .record_bundle
            .bundle
            .steps[0]
            .output_commitments[1];
        final_note_nullifier_reuses_prior_output
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .current_notes[1]
            .spend_nullifier = prior_sibling_output;
        assert!(matches!(
            final_note_nullifier_reuses_prior_output.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.spend_nullifier"
            })
        ));

        let mut reserved_previous_proof = valid_redeem_request.clone();
        let lineage_previous_proof = kagemusha_recursive_spend_lineage_proof(
            &bundle0.accumulator,
            b"recursive-lineage-previous-proof-scalar",
        );
        reserved_previous_proof
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .previous_recursive_proofs[0] = lineage_previous_proof;
        assert!(matches!(
            reserved_previous_proof.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record"
            })
        ));
        reserved_previous_proof.lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        reserved_previous_proof
            .validate_public_binding()
            .expect("lineage witness accepts reserved-lineage previous proof with verifier record");
        let mut reserved_previous_proof_wrong_record = reserved_previous_proof.clone();
        reserved_previous_proof_wrong_record
            .lineage_verifier_record
            .as_mut()
            .expect("lineage verifier record")
            .circuit_id = KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.to_owned();
        assert!(matches!(
            reserved_previous_proof_wrong_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record.circuit_id"
            })
        ));

        let previous_context_splice_cases: [(
            &str,
            &'static str,
            fn(&mut KagemushaRecursiveAggregationProof),
        ); 4] = [
            (
                "verifier_opening_len",
                "lineage_witness.previous_recursive_proofs.verifier_opening_len",
                |proof: &mut KagemushaRecursiveAggregationProof| {
                    proof.public_inputs.verifier_opening_len = 8;
                },
            ),
            (
                "verifier_params_fingerprint",
                "lineage_witness.previous_recursive_proofs.verifier_params_fingerprint",
                |proof: &mut KagemushaRecursiveAggregationProof| {
                    proof.public_inputs.verifier_params_fingerprint =
                        fixed_hash(b"recursive-lineage-previous-proof-forged-params");
                },
            ),
            (
                "fixed_window_table_schedule_digest",
                "lineage_witness.previous_recursive_proofs.fixed_window_table_schedule_digest",
                |proof: &mut KagemushaRecursiveAggregationProof| {
                    proof.public_inputs.fixed_window_table_schedule_digest =
                        fixed_hash(b"recursive-lineage-previous-proof-forged-schedule");
                },
            ),
            (
                "fixed_window_shared_table_manifest_digest",
                "lineage_witness.previous_recursive_proofs.fixed_window_shared_table_manifest_digest",
                |proof: &mut KagemushaRecursiveAggregationProof| {
                    proof
                        .public_inputs
                        .fixed_window_shared_table_manifest_digest =
                        fixed_hash(b"recursive-lineage-previous-proof-forged-manifest");
                },
            ),
        ];
        for (case, expected_field, mutate) in previous_context_splice_cases {
            let mut spliced_previous_context = valid_redeem_request.clone();
            let previous_proof = &mut spliced_previous_context
                .lineage_witness
                .as_mut()
                .expect("lineage witness")
                .previous_recursive_proofs[0];
            mutate(previous_proof);
            previous_proof.public_inputs_hash = previous_proof
                .public_inputs
                .public_inputs_hash()
                .expect("context-spliced previous proof public-input hash");
            let err = spliced_previous_context
                .validate_public_binding()
                .expect_err("previous proof verifier-context splice must reject");
            assert!(
                matches!(
                    err,
                    KagemushaFoldError::RecursiveSpendPublicInputMismatch { field }
                        if field == expected_field
                ),
                "{case} returned unexpected error: {err:?}"
            );
        }

        let mut scalar_spliced_previous_proof = valid_redeem_request.clone();
        let previous_proof = &mut scalar_spliced_previous_proof
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .previous_recursive_proofs[0];
        previous_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_hash(b"recursive-lineage-semantic-previous-proof-scalar-splice");
        previous_proof.public_inputs_hash = previous_proof
            .public_inputs
            .public_inputs_hash()
            .expect("scalar-spliced previous proof public-input hash");
        assert!(matches!(
            scalar_spliced_previous_proof.validate_public_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_verifier_scalar_projection_digest"
            })
        ));

        let mut out_of_order_previous_proof = valid_redeem_request;
        let previous_proof = &mut out_of_order_previous_proof
            .lineage_witness
            .as_mut()
            .expect("lineage witness")
            .previous_recursive_proofs[0];
        previous_proof.public_inputs.hop_count = 2;
        previous_proof.public_inputs.verifier_witness_count = 2;
        previous_proof.public_inputs_hash = previous_proof
            .public_inputs
            .public_inputs_hash()
            .expect("out-of-order previous proof public-input hash");
        assert!(matches!(
            out_of_order_previous_proof.validate_public_binding(),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 1,
                actual: 2
            })
        ));

        let mut bad_init = init_request;
        bad_init.pallas_open_envelopes_archive = vec![0x01, 0x02];
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_from_init_result(&bad_init, &bundle0),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive"
            })
        ));

        let mut conflicting_append = append_request.clone();
        let previous_entry = &witness0.record_bundle.verifier_records[0];
        conflicting_append.record_bundle.bundle.steps[0]
            .attachment
            .vk_ref = previous_entry.id.clone();
        conflicting_append.record_bundle.verifier_records[0].id = previous_entry.id.clone();
        conflicting_append.record_bundle.verifier_records[0]
            .record
            .max_proof_bytes += 1;
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &conflicting_append,
                &bundle0
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.conflict"
            })
        ));

        let mut chain_spliced_append = append_request.clone();
        chain_spliced_append.record_bundle.bundle.chain_id =
            "kagemusha-recursive-spend-lineage-forged-chain"
                .parse()
                .expect("forged chain id");
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &chain_spliced_append,
                &bundle1
            ),
            Err(KagemushaFoldError::RecursiveSpendChainMismatch)
        ));

        let mut asset_spliced_append = append_request.clone();
        asset_spliced_append.record_bundle.bundle.asset =
            kagemusha_asset("kgm-recursive-spend-lineage-forged-asset");
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &asset_spliced_append,
                &bundle1
            ),
            Err(KagemushaFoldError::RecursiveSpendAssetMismatch)
        ));

        let mut mismatched_previous_bundle = append_request.clone();
        mismatched_previous_bundle
            .previous_bundle
            .accumulator
            .current_note
            .spend_nullifier = fixed_hash(b"recursive-lineage-forged-previous-nullifier");
        mismatched_previous_bundle
            .previous_bundle
            .recursive_proof
            .public_inputs = mismatched_previous_bundle
            .previous_bundle
            .accumulator
            .recursive_public_inputs()
            .expect("forged previous bundle public inputs");
        mismatched_previous_bundle
            .previous_bundle
            .recursive_proof
            .public_inputs_hash = mismatched_previous_bundle
            .previous_bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged previous bundle public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &mismatched_previous_bundle,
                &bundle1
            ),
            Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier)
        ));

        let mut stale_appended_bundle = bundle1.clone();
        stale_appended_bundle
            .accumulator
            .current_note
            .spend_nullifier = fixed_hash(b"recursive-lineage-stale-appended-nullifier");
        stale_appended_bundle.recursive_proof.public_inputs = stale_appended_bundle
            .accumulator
            .recursive_public_inputs()
            .expect("stale appended bundle public inputs");
        stale_appended_bundle.recursive_proof.public_inputs_hash = stale_appended_bundle
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("stale appended bundle public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_lineage_witness_append_result(
                &witness0,
                &append_request,
                &stale_appended_bundle
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.final"
            })
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_rejects_malformed_notes_and_lineage() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let other_chain_id: ChainId = "kagemusha-recursive-spend-other-chain"
            .parse()
            .expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let other_asset = kagemusha_asset("kgm-recursive-spend-other");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-spend-root-2");

        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-spend-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-witness-hop-0",
        );
        let accumulator0 =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let previous_proof0 = kagemusha_recursive_spend_proof(&accumulator0);
        let mut missing_topup_anchor = accumulator0.clone();
        missing_topup_anchor.topup_anchor_nullifiers.clear();
        assert!(matches!(
            missing_topup_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut zero_topup_anchor = accumulator0.clone();
        zero_topup_anchor.topup_anchor_nullifiers[0] = [0u8; Hash::LENGTH];
        assert!(matches!(
            zero_topup_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut duplicate_topup_anchor = accumulator0.clone();
        duplicate_topup_anchor.topup_anchor_nullifiers[1] =
            duplicate_topup_anchor.topup_anchor_nullifiers[0];
        assert!(matches!(
            duplicate_topup_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut unsorted_topup_anchor = accumulator0.clone();
        unsorted_topup_anchor.topup_anchor_nullifiers.swap(0, 1);
        assert!(matches!(
            unsorted_topup_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut reused_final_nullifier_anchor = accumulator0.clone();
        reused_final_nullifier_anchor.topup_anchor_nullifiers[0] = note0.spend_nullifier;
        reused_final_nullifier_anchor
            .topup_anchor_nullifiers
            .sort_unstable();
        assert!(matches!(
            reused_final_nullifier_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut reused_final_commitment_anchor = accumulator0.clone();
        reused_final_commitment_anchor.topup_anchor_nullifiers[0] = note0.note_commitment;
        reused_final_commitment_anchor
            .topup_anchor_nullifiers
            .sort_unstable();
        assert!(matches!(
            reused_final_commitment_anchor.validate_context(),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "topup_anchor_nullifiers"
            })
        ));

        let mut detached_aggregation_transcript = accumulator0.clone();
        detached_aggregation_transcript.aggregation_transcript_digest[0] ^= 0x01;
        assert!(matches!(
            detached_aggregation_transcript.validate_context(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "aggregation_transcript_digest"
            })
        ));

        let mut zero_commitment_note = note0.clone();
        zero_commitment_note.note_commitment = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &zero_commitment_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "note_commitment"
            })
        ));

        let mut zero_nullifier_note = note0.clone();
        zero_nullifier_note.spend_nullifier = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &zero_nullifier_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));

        let mut duplicate_note_fields = note0.clone();
        duplicate_note_fields.spend_nullifier = duplicate_note_fields.note_commitment;
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &duplicate_note_fields
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));

        let mut sibling_output_nullifier_note = note0.clone();
        sibling_output_nullifier_note.spend_nullifier = step0.output_commitments[1];
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &sibling_output_nullifier_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));

        let mut zero_amount_note = note0.clone();
        zero_amount_note.amount = Numeric::new(0, 0);
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &zero_amount_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));

        let mut fractional_amount_note = note0.clone();
        fractional_amount_note.amount = Numeric::new(425, 1);
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &fractional_amount_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));

        let mut negative_amount_note = note0.clone();
        negative_amount_note.amount = Numeric::new(-42, 0);
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &negative_amount_note
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));

        let missing_output_note = kagemusha_recursive_spend_note(
            b"recursive-spend-forged-output",
            b"recursive-spend-forged-nullifier",
            42,
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_from_initial_evidence(
                &evidence0,
                &missing_output_note
            ),
            Err(KagemushaFoldError::RecursiveSpendMissingCurrentNoteCommitment)
        ));

        let mut step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-spend-hop-1");
        let note1 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step1.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-1-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let missing_previous_evidence = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1.clone(),
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &missing_previous_evidence,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendMissingPreviousNullifier)
        ));

        let mut merged_external_input_step = step1.clone();
        merged_external_input_step.input_nullifiers[0] = note0.spend_nullifier;
        let merged_external_input = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            merged_external_input_step,
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &merged_external_input,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendUnexpectedAppendInput)
        ));

        step1.input_nullifiers = vec![note0.spend_nullifier];
        let append_evidence = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1.clone(),
            b"recursive-spend-witness-hop-1",
        );
        let valid_append_accumulator = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &previous_proof0,
            &append_evidence,
            &note1,
        )
        .expect("valid append evidence");
        let mut previous_public_input_splice = previous_proof0.clone();
        previous_public_input_splice.public_inputs.evidence_digest[0] ^= 0x01;
        previous_public_input_splice.public_inputs_hash = previous_public_input_splice
            .public_inputs
            .public_inputs_hash()
            .expect("spliced previous proof public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_public_input_splice,
                &append_evidence,
                &note1,
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "previous_recursive_proof.evidence_digest"
            })
        ));
        let mut previous_folded_hash_splice = previous_proof0.clone();
        previous_folded_hash_splice
            .public_inputs
            .folded_public_inputs_hash =
            fixed_hash(b"recursive-spend-previous-proof-folded-hash-splice");
        previous_folded_hash_splice.public_inputs_hash = previous_folded_hash_splice
            .public_inputs
            .public_inputs_hash()
            .expect("spliced previous proof folded public-input hash");
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_folded_hash_splice,
                &append_evidence,
                &note1,
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "previous_recursive_proof.folded_public_inputs_hash"
            })
        ));
        let mut stale_previous_public_input_hash = previous_proof0.clone();
        stale_previous_public_input_hash.public_inputs_hash =
            Hash::new(b"recursive-spend-stale-previous-proof-public-input-hash");
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &stale_previous_public_input_hash,
                &append_evidence,
                &note1,
            ),
            Err(KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch { .. })
        ));
        let mut previous_proof_byte_splice = previous_proof0.clone();
        previous_proof_byte_splice.proof.bytes[0] ^= 0x01;
        assert_ne!(
            kagemusha_recursive_spend_proof_artifact_digest(&previous_proof_byte_splice)
                .expect("proof-byte-spliced artifact digest"),
            kagemusha_recursive_spend_proof_artifact_digest(&previous_proof0)
                .expect("original previous proof artifact digest"),
            "proof bytes must be part of the exported recursive proof artifact digest"
        );
        let byte_splice_accumulator = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &previous_proof_byte_splice,
            &append_evidence,
            &note1,
        )
        .expect("proof-byte splice is bound into accumulator state");
        assert_ne!(
            byte_splice_accumulator.recursive_proof_chain_digest,
            valid_append_accumulator.recursive_proof_chain_digest
        );
        let mut table_base_rotation = append_evidence.clone();
        table_base_rotation.fixed_window_table_base_digest =
            fixed_hash(b"recursive-spend-rotated-table-base");
        let rotated_table_base_accumulator = kagemusha_recursive_spend_accumulator_append_evidence(
            &accumulator0,
            &previous_proof0,
            &table_base_rotation,
            &note1,
        )
        .expect("per-hop fixed-window table-base digest must stream across append");
        assert_ne!(
            rotated_table_base_accumulator.fixed_window_table_base_digest,
            accumulator0.fixed_window_table_base_digest
        );
        assert_ne!(
            rotated_table_base_accumulator.fixed_window_table_base_digest,
            table_base_rotation.fixed_window_table_base_digest
        );
        let mut amount_drift_note = note1.clone();
        amount_drift_note.amount = Numeric::new(43, 0);
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &append_evidence,
                &amount_drift_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote { field: "amount" })
        ));
        let mut reused_input_nullifier_note = note1.clone();
        reused_input_nullifier_note.spend_nullifier = note0.spend_nullifier;
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &kagemusha_recursive_spend_one_hop_evidence(
                    &chain_id,
                    &asset,
                    step1.clone(),
                    b"recursive-spend-witness-hop-1-reused-nullifier",
                ),
                &reused_input_nullifier_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));
        let mut reused_previous_commitment_nullifier_note = note1.clone();
        reused_previous_commitment_nullifier_note.spend_nullifier = note0.note_commitment;
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &append_evidence,
                &reused_previous_commitment_nullifier_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "spend_nullifier"
            })
        ));

        let mut reused_previous_commitment_output_step = step1.clone();
        reused_previous_commitment_output_step.output_commitments[0] = note0.note_commitment;
        let reused_previous_commitment_output_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: reused_previous_commitment_output_step.output_commitments[1],
            spend_nullifier: fixed_hash(b"recursive-spend-note-1-output-commitment-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let reused_previous_commitment_output = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            reused_previous_commitment_output_step,
            b"recursive-spend-witness-hop-1-output-reuses-previous-commitment",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &reused_previous_commitment_output,
                &reused_previous_commitment_output_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "output_commitments"
            })
        ));

        let mut reused_topup_anchor_output_step = step1.clone();
        reused_topup_anchor_output_step.output_commitments[0] =
            accumulator0.topup_anchor_nullifiers[0];
        let reused_topup_anchor_output_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: reused_topup_anchor_output_step.output_commitments[1],
            spend_nullifier: fixed_hash(b"recursive-spend-note-1-topup-anchor-output-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let reused_topup_anchor_output = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            reused_topup_anchor_output_step,
            b"recursive-spend-witness-hop-1-output-reuses-topup-anchor",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &reused_topup_anchor_output,
                &reused_topup_anchor_output_note,
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendTopupAnchor {
                field: "output_commitments"
            })
        ));

        let chain_mismatch = kagemusha_recursive_spend_one_hop_evidence(
            &other_chain_id,
            &asset,
            step1.clone(),
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &chain_mismatch,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendChainMismatch)
        ));

        let asset_mismatch = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &other_asset,
            step1.clone(),
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &asset_mismatch,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendAssetMismatch)
        ));

        let mut root_mismatch_step = step1.clone();
        root_mismatch_step.root_before = fixed_hash(b"recursive-spend-forged-root-before");
        let root_mismatch = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            root_mismatch_step,
            b"recursive-spend-witness-hop-1",
        );
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &root_mismatch,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendRootMismatch)
        ));

        let mut verifier_context_mismatch = append_evidence;
        verifier_context_mismatch.fixed_window_table_schedule_digest =
            fixed_hash(b"recursive-spend-forged-schedule");
        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &accumulator0,
                &previous_proof0,
                &verifier_context_mismatch,
                &note1
            ),
            Err(KagemushaFoldError::RecursiveSpendVerifierContextMismatch {
                field: "fixed_window_table_schedule_digest"
            })
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_bundle_rejects_public_input_tampering() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-root-1");
        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-spend-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-witness-hop-0",
        );
        let accumulator =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let bundle = kagemusha_recursive_spend_bundle(accumulator);
        bundle
            .validate_public_input_binding()
            .expect("valid recursive spend bundle");
        let folded_public_inputs = KagemushaFoldedPublicInputs {
            domain: KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned(),
            aggregation_mode: KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1,
            chain_id: bundle.accumulator.chain_id.clone(),
            asset: bundle.accumulator.asset.clone(),
            initial_root: bundle.accumulator.initial_root,
            final_root: bundle.accumulator.final_root,
            hop_count: bundle.accumulator.hop_count,
            nullifier_digest: bundle.accumulator.nullifier_digest,
            output_commitment_digest: bundle.accumulator.output_commitment_digest,
            fold_digest: bundle.accumulator.fold_digest,
            aggregation_transcript_digest: bundle.accumulator.aggregation_transcript_digest,
        };
        assert_eq!(
            bundle
                .recursive_proof
                .public_inputs
                .folded_public_inputs_hash,
            hash_bytes_from_hash(
                folded_public_inputs
                    .public_inputs_hash()
                    .expect("recursive spend folded public-input hash")
            )
        );
        assert_eq!(
            bundle
                .recursive_proof
                .public_inputs
                .recursive_proof_chain_digest,
            bundle.accumulator.recursive_proof_chain_digest
        );
        assert_eq!(
            bundle
                .recursive_proof
                .public_inputs
                .append_opening_preflight_digest,
            [0u8; Hash::LENGTH]
        );
        assert_eq!(
            bundle.recursive_proof.public_inputs.append_boundary_digest,
            [0u8; Hash::LENGTH]
        );
        assert_eq!(
            bundle
                .recursive_proof
                .public_inputs
                .recursive_verifier_scalar_projection_digest,
            [0u8; Hash::LENGTH]
        );

        let mut wrong_domain = bundle.clone();
        wrong_domain.accumulator.domain = "iroha:kagemusha:v1:recursive-spend-forged".into();
        assert!(matches!(
            wrong_domain.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendAccumulatorDomain { .. })
        ));

        let mut forged_evidence_digest = bundle.clone();
        forged_evidence_digest
            .recursive_proof
            .public_inputs
            .evidence_digest = fixed_hash(b"recursive-spend-forged-evidence-digest");
        forged_evidence_digest.recursive_proof.public_inputs_hash = forged_evidence_digest
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged recursive spend public-input hash");
        assert!(matches!(
            forged_evidence_digest.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "evidence_digest"
            })
        ));

        let mut forged_folded_public_inputs_hash = bundle.clone();
        forged_folded_public_inputs_hash
            .recursive_proof
            .public_inputs
            .folded_public_inputs_hash =
            fixed_hash(b"recursive-spend-forged-folded-public-inputs-hash");
        forged_folded_public_inputs_hash
            .recursive_proof
            .public_inputs_hash = forged_folded_public_inputs_hash
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged recursive spend folded hash public-input hash");
        assert!(matches!(
            forged_folded_public_inputs_hash.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "folded_public_inputs_hash"
            })
        ));

        let mut forged_hash = bundle.clone();
        forged_hash.recursive_proof.public_inputs_hash =
            Hash::new(b"recursive-spend-forged-public-input-hash");
        assert!(matches!(
            forged_hash.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveAggregationProofPublicInputHashMismatch { .. })
        ));

        let mut forged_topup_anchor = bundle.clone();
        forged_topup_anchor.accumulator.topup_anchor_nullifiers[0][0] ^= 0x01;
        forged_topup_anchor
            .accumulator
            .topup_anchor_nullifiers
            .sort_unstable();
        assert!(matches!(
            forged_topup_anchor.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "evidence_digest"
            })
        ));

        let mut forged_proof_chain_public_input = bundle.clone();
        forged_proof_chain_public_input
            .recursive_proof
            .public_inputs
            .recursive_proof_chain_digest =
            fixed_hash(b"recursive-spend-forged-proof-chain-public-input");
        forged_proof_chain_public_input
            .recursive_proof
            .public_inputs_hash = forged_proof_chain_public_input
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged proof-chain public-input hash");
        assert!(matches!(
            forged_proof_chain_public_input.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_proof_chain_digest"
            })
        ));

        let mut forged_transition_binding_public_input = bundle.clone();
        forged_transition_binding_public_input
            .recursive_proof
            .public_inputs
            .transition_profile_binding_digest =
            fixed_hash(b"recursive-spend-forged-transition-binding-public-input");
        forged_transition_binding_public_input
            .recursive_proof
            .public_inputs_hash = forged_transition_binding_public_input
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged transition binding public-input hash");
        assert!(matches!(
            forged_transition_binding_public_input.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "transition_profile_binding_digest"
            })
        ));

        let mut forged_append_opening_public_input = bundle.clone();
        forged_append_opening_public_input
            .recursive_proof
            .public_inputs
            .append_opening_preflight_digest =
            fixed_hash(b"recursive-spend-forged-append-opening-public-input");
        forged_append_opening_public_input
            .recursive_proof
            .public_inputs_hash = forged_append_opening_public_input
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged append opening public-input hash");
        assert!(matches!(
            forged_append_opening_public_input.validate_public_input_binding(),
            Err(
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "append_opening_preflight_digest"
                }
            )
        ));

        let mut forged_append_boundary_public_input = bundle.clone();
        forged_append_boundary_public_input
            .recursive_proof
            .public_inputs
            .append_boundary_digest =
            fixed_hash(b"recursive-spend-forged-append-boundary-public-input");
        forged_append_boundary_public_input
            .recursive_proof
            .public_inputs_hash = forged_append_boundary_public_input
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged append boundary public-input hash");
        let err = forged_append_boundary_public_input
            .validate_public_input_binding()
            .expect_err("forged append-boundary public input must reject");
        assert!(
            matches!(
                err,
                KagemushaFoldError::RecursiveAggregationProofPublicInputMismatch {
                    field: "append_boundary_digest"
                }
            ),
            "forged append-boundary public input returned unexpected error: {err:?}"
        );

        let mut forged_initial_append_opening_accumulator = bundle.clone();
        forged_initial_append_opening_accumulator
            .accumulator
            .append_opening_preflight_digest =
            fixed_hash(b"recursive-spend-forged-initial-append-opening");
        assert!(matches!(
            forged_initial_append_opening_accumulator.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_opening_preflight_digest"
            })
        ));

        let mut forged_initial_append_boundary_accumulator = bundle.clone();
        forged_initial_append_boundary_accumulator
            .accumulator
            .append_boundary_digest = fixed_hash(b"recursive-spend-forged-initial-append-boundary");
        assert!(matches!(
            forged_initial_append_boundary_accumulator.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "append_boundary_digest"
            })
        ));

        let mut forged_scalar_projection_public_input = bundle.clone();
        forged_scalar_projection_public_input
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest =
            fixed_hash(b"recursive-spend-forged-scalar-projection-public-input");
        forged_scalar_projection_public_input
            .recursive_proof
            .public_inputs_hash = forged_scalar_projection_public_input
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged scalar projection public-input hash");
        assert!(matches!(
            forged_scalar_projection_public_input.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_verifier_scalar_projection_digest"
            })
        ));
        assert!(matches!(
            kagemusha_recursive_spend_proof_artifact_digest(
                &forged_scalar_projection_public_input.recursive_proof
            ),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_verifier_scalar_projection_digest"
            })
        ));

        let mut lineage_bundle = bundle.clone();
        lineage_bundle.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &lineage_bundle.accumulator,
            b"recursive-spend-lineage-scalar-projection",
        );
        lineage_bundle
            .validate_public_input_binding()
            .expect("reserved lineage recursive spend proof binding");

        let mut zero_lineage_scalar = lineage_bundle.clone();
        zero_lineage_scalar
            .recursive_proof
            .public_inputs
            .recursive_verifier_scalar_projection_digest = [0u8; Hash::LENGTH];
        zero_lineage_scalar.recursive_proof.public_inputs_hash = zero_lineage_scalar
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("zero lineage scalar public-input hash");
        assert!(matches!(
            zero_lineage_scalar.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "recursive_verifier_scalar_projection_digest"
            })
        ));

        let mut lineage_forged_evidence = lineage_bundle.clone();
        lineage_forged_evidence
            .recursive_proof
            .public_inputs
            .evidence_digest = fixed_hash(b"recursive-spend-lineage-forged-evidence");
        lineage_forged_evidence.recursive_proof.public_inputs_hash = lineage_forged_evidence
            .recursive_proof
            .public_inputs
            .public_inputs_hash()
            .expect("forged lineage evidence public-input hash");
        assert!(matches!(
            lineage_forged_evidence.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "evidence_digest"
            })
        ));

        let mut unknown_lineage_circuit = lineage_bundle;
        unknown_lineage_circuit.recursive_proof.verifier_key_id.name =
            "kagemusha-recursive-spend-lineage-dev".to_owned();
        assert!(matches!(
            unknown_lineage_circuit.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_key_id.name"
            })
        ));

        let mut forged_proof_chain = bundle.clone();
        forged_proof_chain.accumulator.recursive_proof_chain_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            forged_proof_chain.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "recursive_proof_chain_digest"
            })
        ));

        let mut forged_transition_binding = bundle.clone();
        forged_transition_binding
            .accumulator
            .transition_profile_binding_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            forged_transition_binding.validate_public_input_binding(),
            Err(KagemushaFoldError::RecursiveSpendPublicInputMismatch {
                field: "transition_profile_binding_digest"
            })
        ));

        let mut forged_proof_backend = bundle;
        forged_proof_backend.recursive_proof.proof =
            ProofBox::new("halo2/kzg".into(), vec![0xA5; 256]);
        assert!(matches!(
            forged_proof_backend.validate_public_input_binding(),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_redeem_request_binds_public_amount() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-root-1");
        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"recursive-spend-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-note-0-nullifier"),
            amount: Numeric::new(42, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-witness-hop-0",
        );
        let accumulator =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("initial recursive spend accumulator");
        let bundle = kagemusha_recursive_spend_bundle(accumulator);
        let mut redeem_proof = ProofAttachment::new_ref(
            "halo2/ipa".into(),
            ProofBox::new("halo2/ipa".into(), vec![0xA5; 64]),
            VerifyingKeyId::new("halo2/ipa", "kagemusha-unshield-fixture"),
        );
        redeem_proof.vk_commitment = Some(fixed_hash(b"recursive-spend-unshield-vk"));
        let recipient = sample_account(0xB2, "offline");
        let valid = KagemushaRecursiveSpendRedeemRequestV1 {
            bundle: bundle.clone(),
            recipient: recipient.clone(),
            public_amount: 42,
            redeem_proof: redeem_proof.clone(),
            lineage_witness: None,
            change_output: None,
            lineage_verifier_record: None,
            block_height: None,
        };
        valid
            .validate_public_binding()
            .expect("redeem request amount binding");
        let change_output = fixed_hash(b"recursive-spend-change-output");
        let mut valid_partial = valid.clone();
        valid_partial.public_amount = 7;
        valid_partial.change_output = Some(change_output);
        valid_partial
            .validate_public_binding()
            .expect("partial redeem request accepts private change output");

        let mut partial_without_change = valid.clone();
        partial_without_change.public_amount = 7;
        assert!(matches!(
            partial_without_change.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount"
            })
        ));

        let mut full_with_change = valid.clone();
        full_with_change.change_output = Some(change_output);
        assert!(matches!(
            full_with_change.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount"
            })
        ));

        let mut zero_change = valid_partial.clone();
        zero_change.change_output = Some([0u8; Hash::LENGTH]);
        assert!(matches!(
            zero_change.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "change_output"
            })
        ));

        let mut zero_amount_with_change = valid_partial.clone();
        zero_amount_with_change.public_amount = 0;
        assert!(matches!(
            zero_amount_with_change.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount"
            })
        ));

        let mut current_note_commitment_as_change = valid_partial.clone();
        current_note_commitment_as_change.change_output = Some(note0.note_commitment);
        assert!(matches!(
            current_note_commitment_as_change.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "change_output"
            })
        ));

        let mut current_note_nullifier_as_change = valid_partial.clone();
        current_note_nullifier_as_change.change_output = Some(note0.spend_nullifier);
        assert!(matches!(
            current_note_nullifier_as_change.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "change_output"
            })
        ));

        let mut topup_anchor_nullifier_as_change = valid_partial.clone();
        topup_anchor_nullifier_as_change.change_output =
            Some(bundle.accumulator.topup_anchor_nullifiers[0]);
        assert!(matches!(
            topup_anchor_nullifier_as_change.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "change_output"
            })
        ));

        let lineage_record_bundle = kagemusha_recursive_spend_record_bundle_for_step(
            chain_id.clone(),
            asset.clone(),
            &step0,
            "kagemusha-recursive-hop-fixture",
            b"recursive-spend-hop-proof",
        );
        let lineage_witness = KagemushaRecursiveSpendLineageWitnessV1 {
            record_bundle: lineage_record_bundle.clone(),
            pallas_open_envelopes_archive:
                kagemusha_recursive_spend_lineage_pallas_open_envelope_archive(
                    &lineage_record_bundle,
                    0x61,
                ),
            current_notes: vec![note0.clone()],
            previous_recursive_proofs: Vec::new(),
        };
        let mut valid_with_lineage = valid.clone();
        valid_with_lineage.lineage_witness = Some(lineage_witness.clone());
        valid_with_lineage
            .validate_public_binding()
            .expect("redeem request accepts well-shaped lineage witness");

        let mut reserved_lineage_with_record_witness = valid_with_lineage.clone();
        reserved_lineage_with_record_witness.bundle.recursive_proof =
            kagemusha_recursive_spend_lineage_proof(
                &reserved_lineage_with_record_witness.bundle.accumulator,
                b"recursive-spend-redeem-reserved-lineage-scalar",
            );
        assert!(matches!(
            reserved_lineage_with_record_witness.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_record"
            })
        ));
        reserved_lineage_with_record_witness.lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        reserved_lineage_with_record_witness
            .validate_public_binding()
            .expect("reserved-lineage redeem witness validates with active lineage record");

        let mut final_root_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.bundle.steps[0].root_after =
            fixed_hash(b"recursive-spend-forged-lineage-final-root");
        final_root_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            final_root_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::FinalRootMismatch { .. })
        ));

        let mut duplicate_lineage_output = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.bundle.steps[0].output_commitments[1] =
            bad_witness.record_bundle.bundle.steps[0].output_commitments[0];
        duplicate_lineage_output.lineage_witness = Some(bad_witness);
        assert!(matches!(
            duplicate_lineage_output.validate_public_binding(),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 0 })
        ));

        let mut proof_backend_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.bundle.steps[0].attachment.proof =
            ProofBox::new("stark/fri/production".into(), vec![0xA5; 64]);
        proof_backend_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            proof_backend_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.backend"
            })
        ));

        let mut missing_vk_commitment = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.bundle.steps[0]
            .attachment
            .vk_commitment = None;
        missing_vk_commitment.lineage_witness = Some(bad_witness);
        assert!(matches!(
            missing_vk_commitment.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.bundle.steps.attachment.vk_commitment"
            })
        ));

        let mut zero_vk_commitment = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.bundle.steps[0]
            .attachment
            .vk_commitment = Some([0u8; Hash::LENGTH]);
        zero_vk_commitment.lineage_witness = Some(bad_witness);
        assert!(matches!(
            zero_vk_commitment.validate_public_binding(),
            Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index: 0 })
        ));

        let mut empty_verifier_key = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.bundle.steps[0]
            .verifier_key
            .bytes
            .clear();
        empty_verifier_key.lineage_witness = Some(bad_witness);
        assert!(matches!(
            empty_verifier_key.validate_public_binding(),
            Err(KagemushaFoldError::EmptyVerifierKeyBytes { backend })
                if backend == "halo2/ipa"
        ));

        let mut inactive_record = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0].record.status = ConfidentialStatus::Withdrawn;
        inactive_record.lineage_witness = Some(bad_witness);
        assert!(matches!(
            inactive_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.status"
            })
        ));

        let mut record_commitment_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0]
            .record
            .commitment[0] ^= 0x01;
        record_commitment_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            record_commitment_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.commitment"
            })
        ));

        let mut record_namespace_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0]
            .record
            .namespace = "core".to_owned();
        record_namespace_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            record_namespace_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.namespace"
            })
        ));

        let mut record_backend_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0].record.backend = BackendTag::Stark;
        record_backend_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            record_backend_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.backend"
            })
        ));

        let mut record_curve_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0].record.curve = "pasta".to_owned();
        record_curve_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            record_curve_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.curve"
            })
        ));

        let mut empty_record_circuit_id = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0]
            .record
            .circuit_id = "   ".to_owned();
        empty_record_circuit_id.lineage_witness = Some(bad_witness);
        assert!(matches!(
            empty_record_circuit_id.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.circuit_id"
            })
        ));

        let mut zero_record_schema_hash = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0]
            .record
            .public_inputs_schema_hash = [0u8; Hash::LENGTH];
        zero_record_schema_hash.lineage_witness = Some(bad_witness);
        assert!(matches!(
            zero_record_schema_hash.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.public_inputs_schema_hash"
            })
        ));

        let mut missing_inline_record_key = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0].record.key = None;
        missing_inline_record_key.lineage_witness = Some(bad_witness);
        assert!(matches!(
            missing_inline_record_key.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.key"
            })
        ));

        let mut verifier_key_commitment_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.bundle.steps[0]
            .verifier_key
            .bytes
            .push(0x99);
        bad_witness.record_bundle.verifier_records[0].record.key = Some(
            bad_witness.record_bundle.bundle.steps[0]
                .verifier_key
                .clone(),
        );
        bad_witness.record_bundle.verifier_records[0].record.vk_len = u32::try_from(
            bad_witness.record_bundle.bundle.steps[0]
                .verifier_key
                .bytes
                .len(),
        )
        .expect("mutated verifier key length fits");
        verifier_key_commitment_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            verifier_key_commitment_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.key_commitment"
            })
        ));

        let mut hop_proof_vk_hash_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let mut hop_proof_envelope: crate::zk::OpenVerifyEnvelope = norito::decode_from_bytes(
            &bad_witness.record_bundle.bundle.steps[0]
                .attachment
                .proof
                .bytes,
        )
        .expect("decode hop proof envelope");
        hop_proof_envelope.vk_hash = fixed_hash(b"forged-hop-proof-vk-hash");
        bad_witness.record_bundle.bundle.steps[0]
            .attachment
            .proof
            .bytes = to_bytes(&hop_proof_envelope).expect("encode forged hop proof envelope");
        hop_proof_vk_hash_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            hop_proof_vk_hash_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.vk_hash"
            })
        ));

        let mut hop_proof_circuit_id_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let mut hop_proof_envelope: crate::zk::OpenVerifyEnvelope = norito::decode_from_bytes(
            &bad_witness.record_bundle.bundle.steps[0]
                .attachment
                .proof
                .bytes,
        )
        .expect("decode hop proof envelope");
        hop_proof_envelope.circuit_id = "forged-hop-proof-circuit-id".to_owned();
        bad_witness.record_bundle.bundle.steps[0]
            .attachment
            .proof
            .bytes = to_bytes(&hop_proof_envelope).expect("encode forged hop proof envelope");
        hop_proof_circuit_id_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            hop_proof_circuit_id_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.bundle.steps.attachment.proof.circuit_id"
            })
        ));

        let mut stale_hop_proof_schema = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let mut hop_proof_envelope: crate::zk::OpenVerifyEnvelope = norito::decode_from_bytes(
            &bad_witness.record_bundle.bundle.steps[0]
                .attachment
                .proof
                .bytes,
        )
        .expect("decode hop proof envelope");
        hop_proof_envelope
            .public_inputs
            .extend_from_slice(b":stale-schema");
        bad_witness.record_bundle.bundle.steps[0]
            .attachment
            .proof
            .bytes = to_bytes(&hop_proof_envelope).expect("encode stale hop proof envelope");
        stale_hop_proof_schema.lineage_witness = Some(bad_witness);
        assert!(matches!(
            stale_hop_proof_schema.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive.public_inputs_schema_hash"
            })
        ));

        let mut proof_size_cap = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records[0]
            .record
            .max_proof_bytes = 1;
        proof_size_cap.lineage_witness = Some(bad_witness);
        assert!(matches!(
            proof_size_cap.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.max_proof_bytes"
            })
        ));

        let mut malformed_archive = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.pallas_open_envelopes_archive = vec![0xE1, 0xE2];
        malformed_archive.lineage_witness = Some(bad_witness);
        assert!(matches!(
            malformed_archive.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.pallas_open_envelopes_archive"
            })
        ));

        {
            let expect_malformed_lineage_pallas_opening_shape =
                |case: &str, mutate: fn(&mut iroha_zkp_halo2::OpenVerifyEnvelope)| {
                    let mut malformed = valid.clone();
                    let mut bad_witness = lineage_witness.clone();
                    let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                        norito::decode_from_bytes(&bad_witness.pallas_open_envelopes_archive)
                            .expect("decode one-hop Pallas envelope archive");
                    mutate(
                        envelopes
                            .first_mut()
                            .expect("fixture has one lineage Pallas opening"),
                    );
                    bad_witness.pallas_open_envelopes_archive =
                        to_bytes(&envelopes).expect("encode malformed Pallas envelope archive");
                    malformed.lineage_witness = Some(bad_witness);
                    let err = malformed
                        .validate_public_binding()
                        .expect_err("malformed lineage Pallas opening shape must reject");
                    assert!(
                        matches!(
                            err,
                            KagemushaFoldError::InvalidRecursiveSpendProof { field }
                                if field == "lineage_witness.pallas_open_envelopes_archive"
                        ),
                        "{case} returned unexpected error: {err:?}"
                    );
                };
            expect_malformed_lineage_pallas_opening_shape(
                "empty lineage Pallas opening transcript label",
                |envelope| envelope.transcript_label.clear(),
            );
            expect_malformed_lineage_pallas_opening_shape(
                "zero lineage Pallas opening verifier-key metadata",
                |envelope| envelope.vk_commitment = Some([0u8; Hash::LENGTH]),
            );
            expect_malformed_lineage_pallas_opening_shape(
                "lineage Pallas opening parameter/public length mismatch",
                |envelope| envelope.public.n = envelope.public.n.saturating_mul(2),
            );
            expect_malformed_lineage_pallas_opening_shape(
                "lineage Pallas opening generator count mismatch",
                |envelope| {
                    envelope.params.h.pop();
                },
            );
            expect_malformed_lineage_pallas_opening_shape(
                "lineage Pallas opening IPA round count mismatch",
                |envelope| {
                    envelope.proof.l.clear();
                },
            );
        }

        {
            let expect_lineage_pallas_opening_metadata_mismatch =
                |case: &str,
                 expected_field: &'static str,
                 mutate: fn(&mut iroha_zkp_halo2::OpenVerifyEnvelope)| {
                    let mut malformed = valid.clone();
                    let mut bad_witness = lineage_witness.clone();
                    let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
                        norito::decode_from_bytes(&bad_witness.pallas_open_envelopes_archive)
                            .expect("decode one-hop Pallas envelope archive");
                    mutate(
                        envelopes
                            .first_mut()
                            .expect("fixture has one lineage Pallas opening"),
                    );
                    bad_witness.pallas_open_envelopes_archive = to_bytes(&envelopes)
                        .expect("encode metadata-mismatch Pallas envelope archive");
                    malformed.lineage_witness = Some(bad_witness);
                    let err = malformed
                        .validate_public_binding()
                        .expect_err("metadata-mismatched lineage Pallas opening must reject");
                    assert!(
                        matches!(
                            err,
                            KagemushaFoldError::InvalidRecursiveSpendProof { field }
                                if field == expected_field
                        ),
                        "{case} returned unexpected error: {err:?}"
                    );
                };
            expect_lineage_pallas_opening_metadata_mismatch(
                "lineage Pallas opening verifier-key metadata substitution",
                "lineage_witness.pallas_open_envelopes_archive.vk_commitment",
                |envelope| envelope.vk_commitment = Some(fixed_hash(b"forged-lineage-open-vk")),
            );
            expect_lineage_pallas_opening_metadata_mismatch(
                "lineage Pallas opening public-input schema metadata substitution",
                "lineage_witness.pallas_open_envelopes_archive.public_inputs_schema_hash",
                |envelope| {
                    envelope.public_inputs_schema_hash =
                        Some(fixed_hash(b"forged-lineage-open-schema"));
                },
            );
        }

        let mut envelope_count_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let mut envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&bad_witness.pallas_open_envelopes_archive)
                .expect("decode one-hop Pallas envelope archive");
        let mut extra_envelopes: Vec<iroha_zkp_halo2::OpenVerifyEnvelope> =
            norito::decode_from_bytes(&kagemusha_recursive_spend_pallas_open_envelope_archive(
                0x62,
            ))
            .expect("decode extra Pallas envelope archive");
        envelopes.append(&mut extra_envelopes);
        bad_witness.pallas_open_envelopes_archive =
            to_bytes(&envelopes).expect("encode two-hop Pallas envelope archive");
        envelope_count_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            envelope_count_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 1,
                actual: 2
            })
        ));

        let mut note_count_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.current_notes.clear();
        note_count_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            note_count_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 1,
                actual: 0
            })
        ));

        let mut final_note_mismatch = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.current_notes[0].spend_nullifier =
            fixed_hash(b"recursive-spend-wrong-final-nullifier");
        final_note_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            final_note_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "lineage_witness.current_notes.final"
            })
        ));

        let mut missing_record = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        bad_witness.record_bundle.verifier_records.clear();
        missing_record.lineage_witness = Some(bad_witness);
        assert!(matches!(
            missing_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.missing"
            })
        ));

        let mut duplicate_record = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let duplicate = bad_witness.record_bundle.verifier_records[0].clone();
        bad_witness.record_bundle.verifier_records.push(duplicate);
        duplicate_record.lineage_witness = Some(bad_witness);
        assert!(matches!(
            duplicate_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.duplicate"
            })
        ));

        let mut unreferenced_record = valid.clone();
        let mut bad_witness = lineage_witness.clone();
        let mut extra = bad_witness.record_bundle.verifier_records[0].clone();
        extra.id = VerifyingKeyId::new("halo2/ipa", "unused-kagemusha-hop-fixture");
        bad_witness.record_bundle.verifier_records.push(extra);
        unreferenced_record.lineage_witness = Some(bad_witness);
        assert!(matches!(
            unreferenced_record.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_witness.record_bundle.verifier_records.unreferenced"
            })
        ));

        let mut previous_proof_count_mismatch = valid.clone();
        let mut bad_witness = lineage_witness;
        bad_witness
            .previous_recursive_proofs
            .push(bundle.recursive_proof.clone());
        previous_proof_count_mismatch.lineage_witness = Some(bad_witness);
        assert!(matches!(
            previous_proof_count_mismatch.validate_public_binding(),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 0,
                actual: 1
            })
        ));

        let mut valid_with_envelope_hash = valid.clone();
        valid_with_envelope_hash.redeem_proof.envelope_hash =
            Some(Hash::new(&valid_with_envelope_hash.redeem_proof.proof.bytes).into());
        valid_with_envelope_hash
            .validate_public_binding()
            .expect("redeem request accepts matching envelope hash");

        let mut lineage_valid = valid.clone();
        lineage_valid.bundle.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &lineage_valid.bundle.accumulator,
            b"recursive-spend-redeem-lineage-scalar-projection",
        );
        lineage_valid.lineage_verifier_record =
            Some(kagemusha_recursive_spend_active_lineage_verifier_record());
        lineage_valid
            .validate_public_binding()
            .expect("redeem request accepts reserved lineage proof profile");

        let mut stark_recursive_bundle = valid.clone();
        stark_recursive_bundle.bundle.recursive_proof.proof =
            ProofBox::new("stark/fri/production".into(), vec![0xA5; 64]);
        stark_recursive_bundle
            .bundle
            .recursive_proof
            .verifier_key_id = VerifyingKeyId::new(
            "stark/fri/production",
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
        );
        assert!(matches!(
            stark_recursive_bundle.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof.backend"
            })
        ));

        let mut empty_recursive_proof = valid.clone();
        empty_recursive_proof.bundle.recursive_proof.proof =
            ProofBox::new("halo2/ipa".into(), Vec::new());
        assert!(matches!(
            empty_recursive_proof.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof.bytes"
            })
        ));

        let wrong_amount = KagemushaRecursiveSpendRedeemRequestV1 {
            bundle: bundle.clone(),
            recipient: recipient.clone(),
            public_amount: 41,
            redeem_proof: redeem_proof.clone(),
            lineage_witness: None,
            change_output: None,
            lineage_verifier_record: None,
            block_height: None,
        };
        assert!(matches!(
            wrong_amount.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount"
            })
        ));

        let mut bad_redeem_backend = valid.clone();
        bad_redeem_backend.redeem_proof = ProofAttachment::new_ref(
            "groth16".into(),
            ProofBox::new("groth16".into(), vec![0xA5; 64]),
            VerifyingKeyId::new("groth16", "kagemusha-unshield-fixture"),
        );
        assert!(matches!(
            bad_redeem_backend.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof { field: "backend" })
        ));

        let mut bad_proof_backend = valid.clone();
        bad_proof_backend.redeem_proof.proof = ProofBox::new("halo2/kzg".into(), vec![0xA5; 64]);
        assert!(matches!(
            bad_proof_backend.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "proof.backend"
            })
        ));

        let mut bad_vk_backend = valid.clone();
        bad_vk_backend.redeem_proof.vk_ref =
            VerifyingKeyId::new("halo2/kzg", "kagemusha-unshield-fixture");
        assert!(matches!(
            bad_vk_backend.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "vk_ref.backend"
            })
        ));

        let mut empty_vk_name = valid.clone();
        empty_vk_name.redeem_proof.vk_ref = VerifyingKeyId::new("halo2/ipa", "   ");
        assert!(matches!(
            empty_vk_name.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "vk_ref.name"
            })
        ));

        let mut empty_proof = valid.clone();
        empty_proof.redeem_proof.proof = ProofBox::new("halo2/ipa".into(), Vec::new());
        assert!(matches!(
            empty_proof.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "proof.bytes"
            })
        ));

        let mut missing_vk_commitment = valid.clone();
        missing_vk_commitment.redeem_proof.vk_commitment = None;
        assert!(matches!(
            missing_vk_commitment.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "vk_commitment"
            })
        ));

        let mut zero_vk_commitment = valid.clone();
        zero_vk_commitment.redeem_proof.vk_commitment = Some([0u8; Hash::LENGTH]);
        assert!(matches!(
            zero_vk_commitment.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "vk_commitment"
            })
        ));

        let mut bad_envelope_hash = valid.clone();
        bad_envelope_hash.redeem_proof.envelope_hash =
            Some(fixed_hash(b"recursive-spend-bad-envelope-hash"));
        assert!(matches!(
            bad_envelope_hash.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendRedeemProof {
                field: "envelope_hash"
            })
        ));

        let zero_amount = KagemushaRecursiveSpendRedeemRequestV1 {
            bundle,
            recipient,
            public_amount: 0,
            redeem_proof,
            lineage_witness: None,
            change_output: None,
            lineage_verifier_record: None,
            block_height: None,
        };
        assert!(matches!(
            zero_amount.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendNote {
                field: "public_amount"
            })
        ));
    }

    #[test]
    fn kagemusha_recursive_spend_payload_size_is_hop_count_independent() {
        const FIXED_PROOF_PAYLOAD_BUNDLE_LEN: usize = 1_784;
        const FIXED_PROOF_PAYLOAD_MATERIAL_GROWTH_CEILING: usize = 2_048;

        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let target_hops = [1usize, 2, 3, 5, 8, 13, 21, 34, 55, 64];
        let mut observed = Vec::new();
        let mut observed_transition_profiles = Vec::new();
        let mut previous = None::<KagemushaRecursiveSpendAccumulatorV1>;
        let mut previous_proof = None::<KagemushaRecursiveAggregationProof>;

        for hop_index in 0..KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
            let root_before =
                fixed_hash(format!("recursive-spend-size-root-{hop_index}").as_bytes());
            let root_after =
                fixed_hash(format!("recursive-spend-size-root-{}", hop_index + 1).as_bytes());
            let input_seed = u8::try_from(0x20 + hop_index).expect("input seed fits");
            let proof_label = format!("recursive-spend-size-hop-{hop_index}");
            let mut step = kagemusha_step(
                previous.as_ref().map_or(root_before, |acc| acc.final_root),
                root_after,
                input_seed,
                0x80,
                b"recursive-spend-size-hop",
            );
            step.output_commitments = vec![
                fixed_hash(format!("recursive-spend-size-output-{hop_index}-0").as_bytes()),
                fixed_hash(format!("recursive-spend-size-output-{hop_index}-1").as_bytes()),
            ];
            step.proof_hash = Hash::new(proof_label.as_bytes());
            step.proof_public_inputs_digest =
                fixed_hash(format!("{proof_label}:public-inputs").as_bytes());
            step.verifier_key_commitment = fixed_hash(format!("{proof_label}:vk").as_bytes());
            step.verifier_key_poseidon_digest =
                kagemusha_verifier_key_poseidon_digest("halo2/ipa", proof_label.as_bytes())
                    .expect("size verifier-key digest");
            if let Some(previous) = previous.as_ref() {
                step.input_nullifiers = vec![previous.current_note.spend_nullifier];
            }
            let note = KagemushaSpendableNoteDescriptorV1 {
                note_commitment: step.output_commitments[0],
                spend_nullifier: fixed_hash(
                    format!("recursive-spend-size-nullifier-{hop_index}").as_bytes(),
                ),
                amount: Numeric::new(42, 0),
            };
            let evidence = kagemusha_recursive_spend_one_hop_evidence(
                &chain_id,
                &asset,
                step,
                format!("recursive-spend-size-witness-{hop_index}").as_bytes(),
            );
            let accumulator = match previous.as_ref() {
                Some(previous) => kagemusha_recursive_spend_accumulator_append_evidence(
                    previous,
                    previous_proof.as_ref().expect("previous recursive proof"),
                    &evidence,
                    &note,
                )
                .expect("append size accumulator"),
                None => {
                    kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence, &note)
                        .expect("initial size accumulator")
                }
            };
            let transition_profile = match previous.as_ref() {
                Some(previous) => kagemusha_recursive_spend_transition_profile_append_evidence(
                    previous,
                    previous_proof.as_ref().expect("previous recursive proof"),
                    &evidence,
                    &note,
                )
                .expect("append transition profile"),
                None => kagemusha_recursive_spend_transition_profile_from_initial_evidence(
                    &evidence, &note,
                )
                .expect("initial transition profile"),
            };
            let hop_count = usize::try_from(accumulator.hop_count).expect("hop count fits");
            if target_hops.contains(&hop_count) {
                let bundle = kagemusha_recursive_spend_bundle(accumulator.clone());
                bundle
                    .validate_public_input_binding()
                    .expect("size bundle binding");
                observed.push((
                    hop_count,
                    bundle
                        .norito_encoded_len()
                        .expect("recursive spend bundle encoded length"),
                ));
                observed_transition_profiles.push((
                    hop_count,
                    transition_profile
                        .norito_encoded_len()
                        .expect("recursive spend transition profile encoded length"),
                ));
            }
            previous_proof = Some(kagemusha_recursive_spend_proof(&accumulator));
            previous = Some(accumulator);
        }

        assert_eq!(observed.len(), target_hops.len());
        assert_eq!(observed_transition_profiles.len(), target_hops.len());
        let first_len = observed[0].1;
        assert_eq!(
            first_len, FIXED_PROOF_PAYLOAD_BUNDLE_LEN,
            "recursive Kagemusha fixed-proof fixture archive length changed"
        );
        assert!(
            first_len <= FIXED_PROOF_PAYLOAD_MATERIAL_GROWTH_CEILING,
            "recursive Kagemusha fixed-proof fixture archive exceeded the material-growth ceiling: {first_len} > {FIXED_PROOF_PAYLOAD_MATERIAL_GROWTH_CEILING}"
        );
        for (hop_count, len) in observed {
            assert_eq!(
                len, first_len,
                "recursive Kagemusha D2D payload grew at hop {hop_count}: {len} != {first_len}"
            );
        }
        let first_append_profile_len = observed_transition_profiles
            .iter()
            .find(|(hop_count, _)| *hop_count > 1)
            .map(|(_, len)| *len)
            .expect("at least one append transition profile");
        for (hop_count, len) in observed_transition_profiles
            .into_iter()
            .filter(|(hop_count, _)| *hop_count > 1)
        {
            assert_eq!(
                len, first_append_profile_len,
                "recursive Kagemusha append transition profile grew at hop {hop_count}: {len} != {first_append_profile_len}"
            );
        }
    }

    #[test]
    fn kagemusha_recursive_spend_append_rejects_hop_count_above_cap() {
        let chain_id: ChainId = "kagemusha-recursive-spend-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-spend");
        let mut previous = None::<KagemushaRecursiveSpendAccumulatorV1>;
        let mut previous_proof = None::<KagemushaRecursiveAggregationProof>;

        for hop_index in 0..KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS {
            let root_after =
                fixed_hash(format!("recursive-spend-cap-root-{}", hop_index + 1).as_bytes());
            let mut step = kagemusha_step(
                previous.as_ref().map_or_else(
                    || fixed_hash(b"recursive-spend-cap-root-0"),
                    |accumulator| accumulator.final_root,
                ),
                root_after,
                u8::try_from(0x20 + hop_index).expect("input seed fits"),
                0x80,
                b"recursive-spend-cap-hop",
            );
            step.output_commitments = vec![
                fixed_hash(format!("recursive-spend-cap-output-{hop_index}-0").as_bytes()),
                fixed_hash(format!("recursive-spend-cap-output-{hop_index}-1").as_bytes()),
            ];
            let proof_label = format!("recursive-spend-cap-hop-{hop_index}");
            step.proof_hash = Hash::new(proof_label.as_bytes());
            step.proof_public_inputs_digest =
                fixed_hash(format!("{proof_label}:public-inputs").as_bytes());
            step.verifier_key_commitment = fixed_hash(format!("{proof_label}:vk").as_bytes());
            step.verifier_key_poseidon_digest =
                kagemusha_verifier_key_poseidon_digest("halo2/ipa", proof_label.as_bytes())
                    .expect("cap verifier-key digest");
            if let Some(previous) = previous.as_ref() {
                step.input_nullifiers = vec![previous.current_note.spend_nullifier];
            }
            let note = KagemushaSpendableNoteDescriptorV1 {
                note_commitment: step.output_commitments[0],
                spend_nullifier: fixed_hash(
                    format!("recursive-spend-cap-nullifier-{hop_index}").as_bytes(),
                ),
                amount: Numeric::new(42, 0),
            };
            let evidence = kagemusha_recursive_spend_one_hop_evidence(
                &chain_id,
                &asset,
                step,
                format!("recursive-spend-cap-witness-{hop_index}").as_bytes(),
            );
            let accumulator = previous.as_ref().map_or_else(
                || {
                    kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence, &note)
                        .expect("initial capped accumulator")
                },
                |previous| {
                    kagemusha_recursive_spend_accumulator_append_evidence(
                        previous,
                        previous_proof.as_ref().expect("previous recursive proof"),
                        &evidence,
                        &note,
                    )
                    .expect("append capped accumulator")
                },
            );
            previous_proof = Some(kagemusha_recursive_spend_proof(&accumulator));
            previous = Some(accumulator);
        }

        let previous = previous.expect("64-hop accumulator");
        let previous_proof = previous_proof.expect("64-hop recursive proof");
        assert_eq!(
            previous.hop_count,
            u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS).expect("hop cap fits u32")
        );
        let mut overflow_step = kagemusha_step(
            previous.final_root,
            fixed_hash(b"recursive-spend-cap-root-65"),
            0x70,
            0xC0,
            b"recursive-spend-cap-overflow-hop",
        );
        overflow_step.input_nullifiers = vec![previous.current_note.spend_nullifier];
        let overflow_note = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: overflow_step.output_commitments[0],
            spend_nullifier: fixed_hash(b"recursive-spend-cap-nullifier-64"),
            amount: Numeric::new(42, 0),
        };
        let overflow_evidence = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            overflow_step,
            b"recursive-spend-cap-witness-64",
        );

        assert!(matches!(
            kagemusha_recursive_spend_accumulator_append_evidence(
                &previous,
                &previous_proof,
                &overflow_evidence,
                &overflow_note,
            ),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));
    }

    #[test]
    fn kagemusha_poseidon_aggregation_transcript_digest_rejects_noncanonical_statement() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let root2 = fixed_hash(b"kagemusha-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"proof-hop-1"),
        ];
        let statement =
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &steps)
                .expect("canonical aggregation statement");

        let mut empty = statement.clone();
        empty.steps.clear();
        empty.hop_count = 0;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&empty),
            Err(KagemushaFoldError::Empty)
        ));

        let mut bad_hop_count = statement.clone();
        bad_hop_count.hop_count = 1;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&bad_hop_count),
            Err(KagemushaFoldError::HopCountMismatch {
                expected: 2,
                actual: 1
            })
        ));

        let mut bad_hop_index = statement.clone();
        bad_hop_index.steps[1].hop_index = 7;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&bad_hop_index),
            Err(KagemushaFoldError::HopIndexMismatch {
                expected: 1,
                actual: 7
            })
        ));

        let mut bad_initial_root = statement.clone();
        bad_initial_root.initial_root = fixed_hash(b"kagemusha-forged-initial-root");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&bad_initial_root),
            Err(KagemushaFoldError::InitialRootMismatch { .. })
        ));

        let mut bad_final_root = statement.clone();
        bad_final_root.final_root = fixed_hash(b"kagemusha-forged-final-root");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&bad_final_root),
            Err(KagemushaFoldError::FinalRootMismatch { .. })
        ));

        let mut zero_initial_root = statement.clone();
        zero_initial_root.initial_root = [0u8; Hash::LENGTH];
        zero_initial_root.steps[0].root_before = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_initial_root),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "initial_root"
            })
        ));

        let mut zero_final_root = statement.clone();
        zero_final_root.final_root = [0u8; Hash::LENGTH];
        zero_final_root.steps[1].root_after = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_final_root),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "final_root"
            })
        ));

        let mut zero_intermediate_root = statement.clone();
        zero_intermediate_root.steps[0].root_after = [0u8; Hash::LENGTH];
        zero_intermediate_root.steps[1].root_before = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_intermediate_root),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "root_after"
            })
        ));

        let mut unchanged_public_roots = statement.clone();
        unchanged_public_roots.final_root = unchanged_public_roots.initial_root;
        unchanged_public_roots.steps[1].root_after = unchanged_public_roots.initial_root;
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&unchanged_public_roots),
            Err(KagemushaFoldError::UnchangedFoldedPublicRoots)
        ));

        let mut discontinuous = statement.clone();
        discontinuous.steps[1].root_before = fixed_hash(b"kagemusha-forged-root-before");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&discontinuous),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));

        let mut unsorted_inputs = statement.clone();
        unsorted_inputs.steps[0].input_nullifiers.reverse();
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&unsorted_inputs),
            Err(KagemushaFoldError::NonCanonicalInputNullifierOrder { hop_index: 0 })
        ));

        let mut unsorted_outputs = statement.clone();
        unsorted_outputs.steps[0].output_commitments.reverse();
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&unsorted_outputs),
            Err(KagemushaFoldError::NonCanonicalOutputCommitmentOrder { hop_index: 0 })
        ));

        let mut zero_input = statement.clone();
        zero_input.steps[0].input_nullifiers[0] = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_input),
            Err(KagemushaFoldError::ZeroInputNullifier { hop_index: 0 })
        ));

        let mut zero_output = statement.clone();
        zero_output.steps[0].output_commitments[0] = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_output),
            Err(KagemushaFoldError::ZeroOutputCommitment { hop_index: 0 })
        ));

        let mut zero_proof_inputs = statement.clone();
        zero_proof_inputs.steps[0].proof_public_inputs_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_proof_inputs),
            Err(KagemushaFoldError::ZeroProofPublicInputsDigest { hop_index: 0 })
        ));

        let mut zero_vk_commitment = statement.clone();
        zero_vk_commitment.steps[0].verifier_key_commitment = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_vk_commitment),
            Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index: 0 })
        ));

        let mut zero_vk_poseidon = statement.clone();
        zero_vk_poseidon.steps[0].verifier_key_poseidon_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&zero_vk_poseidon),
            Err(KagemushaFoldError::ZeroVerifierKeyPoseidonDigest { hop_index: 0 })
        ));

        let mut duplicate_input = statement.clone();
        duplicate_input.steps[1].input_nullifiers[0] = duplicate_input.steps[0].input_nullifiers[0];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&duplicate_input),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 1 })
        ));

        let mut duplicate_output = statement.clone();
        duplicate_output.steps[1].output_commitments[0] =
            duplicate_output.steps[0].output_commitments[0];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&duplicate_output),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));

        let mut same_hop_overlap = statement.clone();
        same_hop_overlap.steps[0].output_commitments[0] =
            same_hop_overlap.steps[0].input_nullifiers[0];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&same_hop_overlap),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 0 })
        ));

        let mut cross_hop_overlap = statement.clone();
        cross_hop_overlap.steps[1].input_nullifiers[0] =
            cross_hop_overlap.steps[0].output_commitments[0];
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&cross_hop_overlap),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 1 })
        ));

        let mut empty_vk_id_name = statement.clone();
        empty_vk_id_name.steps[0].verifier_key_id.name = "   ".to_owned();
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&empty_vk_id_name),
            Err(KagemushaFoldError::EmptyVerifierKeyIdName { hop_index: 0 })
        ));

        let mut empty_stark_profile = statement.clone();
        empty_stark_profile.steps[0].verifier_key_id =
            VerifyingKeyId::new("stark/fri/", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&empty_stark_profile),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/"
        ));

        let mut developer_only_stark_profile = statement.clone();
        developer_only_stark_profile.steps[0].verifier_key_id =
            VerifyingKeyId::new("stark/fri/debug", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&developer_only_stark_profile),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/debug"
        ));

        let mut developer_only_hyphen_profile = developer_only_stark_profile.clone();
        developer_only_hyphen_profile.steps[0].verifier_key_id =
            VerifyingKeyId::new("stark/fri/debug-proof", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&developer_only_hyphen_profile),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/debug-proof"
        ));

        let mut trusted_setup_backend = statement;
        trusted_setup_backend.steps[0].verifier_key_id =
            VerifyingKeyId::new("halo2/kzg", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&trusted_setup_backend),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));

        let mut trusted_setup_stark_profile = trusted_setup_backend.clone();
        trusted_setup_stark_profile.steps[0].verifier_key_id =
            VerifyingKeyId::new("stark/fri/kzg", "kagemusha-hop-fixture");
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_digest(&trusted_setup_stark_profile),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "stark/fri/kzg"
        ));
    }

    #[test]
    fn kagemusha_folded_public_inputs_canonicalize_and_bind_transcript() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let root2 = fixed_hash(b"kagemusha-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"proof-hop-1"),
        ];

        let public_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &steps).expect("folded inputs");
        assert_eq!(public_inputs.domain, KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN);
        assert_eq!(
            public_inputs.aggregation_mode,
            KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        );
        assert_eq!(public_inputs.initial_root, root0);
        assert_eq!(public_inputs.final_root, root2);
        assert_eq!(public_inputs.hop_count, 2);
        assert_ne!(
            public_inputs.aggregation_transcript_digest,
            [0u8; Hash::LENGTH]
        );
        let aggregation_statement =
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &steps)
                .expect("aggregation statement");
        assert_eq!(
            aggregation_statement.aggregation_mode,
            KAGEMUSHA_AGGREGATION_MODE_CHECKED_PREFOLD_V1
        );
        assert_eq!(aggregation_statement.initial_root, root0);
        assert_eq!(aggregation_statement.final_root, root2);
        assert_eq!(aggregation_statement.hop_count, 2);
        assert_eq!(
            aggregation_statement.steps[0].input_nullifiers,
            vec![[0x20; Hash::LENGTH], [0x21; Hash::LENGTH]]
        );
        assert_eq!(
            aggregation_statement.steps[0].output_commitments,
            vec![[0x40; Hash::LENGTH], [0x41; Hash::LENGTH]]
        );
        assert_eq!(
            aggregation_statement.steps[1].input_nullifiers,
            vec![[0x60; Hash::LENGTH], [0x61; Hash::LENGTH]]
        );
        assert_eq!(
            public_inputs.aggregation_transcript_digest,
            kagemusha_poseidon_aggregation_transcript_digest(&aggregation_statement)
                .expect("aggregation statement digest")
        );
        assert_eq!(
            public_inputs,
            kagemusha_folded_public_inputs_from_aggregation_statement(&aggregation_statement)
                .expect("aggregation statement projection")
        );
        kagemusha_validate_folded_public_inputs_against_aggregation_statement(
            &public_inputs,
            &aggregation_statement,
        )
        .expect("aggregation statement must bind folded public inputs");

        let mut reordered = steps.clone();
        reordered[0].input_nullifiers.reverse();
        reordered[0].output_commitments.reverse();
        reordered[1].input_nullifiers.reverse();
        reordered[1].output_commitments.reverse();
        let reordered_inputs = kagemusha_folded_public_inputs(&chain_id, &asset, &reordered)
            .expect("canonical reordered folded inputs");
        assert_eq!(public_inputs, reordered_inputs);
        let reordered_statement =
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &reordered)
                .expect("canonical reordered aggregation statement");
        assert_eq!(aggregation_statement, reordered_statement);

        let public_hash = public_inputs
            .public_inputs_hash()
            .expect("folded public inputs hash");
        let aggregation_digest = public_inputs.aggregation_transcript_digest;
        let mut changed_proof = steps.clone();
        changed_proof[1].proof_hash = Hash::new(b"proof-hop-1-forged");
        let changed_proof_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &changed_proof)
                .expect("changed proof folded inputs");
        assert_ne!(
            public_hash,
            changed_proof_inputs
                .public_inputs_hash()
                .expect("changed proof public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_proof_inputs.aggregation_transcript_digest
        );

        let mut changed_proof_statement = steps.clone();
        changed_proof_statement[1].proof_public_inputs_digest =
            fixed_hash(b"kagemusha-hop-public-inputs-forged");
        let changed_proof_statement_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &changed_proof_statement)
                .expect("changed proof public-input folded inputs");
        assert_ne!(
            public_hash,
            changed_proof_statement_inputs
                .public_inputs_hash()
                .expect("changed proof public-input public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_proof_statement_inputs.aggregation_transcript_digest
        );

        let mut changed_vk = steps.clone();
        changed_vk[1].verifier_key_commitment = fixed_hash(b"kagemusha-hop-vk-forged");
        let changed_vk_inputs = kagemusha_folded_public_inputs(&chain_id, &asset, &changed_vk)
            .expect("changed verifier-key folded inputs");
        assert_ne!(
            public_hash,
            changed_vk_inputs
                .public_inputs_hash()
                .expect("changed verifier-key public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_vk_inputs.aggregation_transcript_digest
        );

        let mut changed_vk_poseidon = steps.clone();
        changed_vk_poseidon[1].verifier_key_poseidon_digest =
            fixed_hash(b"kagemusha-hop-vk-poseidon-forged");
        let changed_vk_poseidon_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &changed_vk_poseidon)
                .expect("changed verifier-key Poseidon digest folded inputs");
        assert_ne!(
            public_hash,
            changed_vk_poseidon_inputs
                .public_inputs_hash()
                .expect("changed verifier-key Poseidon digest public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_vk_poseidon_inputs.aggregation_transcript_digest
        );

        let mut changed_vk_ref = steps.clone();
        changed_vk_ref[1].verifier_key_id = VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-other");
        let changed_vk_ref_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &changed_vk_ref)
                .expect("changed verifier-key id folded inputs");
        assert_ne!(
            public_hash,
            changed_vk_ref_inputs
                .public_inputs_hash()
                .expect("changed verifier-key id public hash")
        );
        assert_ne!(
            aggregation_digest,
            changed_vk_ref_inputs.aggregation_transcript_digest
        );

        let other_chain_id: ChainId = "kagemusha-fold-other-chain".parse().expect("chain id");
        let other_chain_inputs = kagemusha_folded_public_inputs(&other_chain_id, &asset, &steps)
            .expect("changed chain folded inputs");
        assert_ne!(
            public_hash,
            other_chain_inputs
                .public_inputs_hash()
                .expect("changed chain public hash")
        );
        assert_ne!(
            aggregation_digest,
            other_chain_inputs.aggregation_transcript_digest
        );

        let other_asset = kagemusha_asset("kgm-other");
        let other_asset_inputs = kagemusha_folded_public_inputs(&chain_id, &other_asset, &steps)
            .expect("changed asset folded inputs");
        assert_ne!(
            public_hash,
            other_asset_inputs
                .public_inputs_hash()
                .expect("changed asset public hash")
        );
        assert_ne!(
            aggregation_digest,
            other_asset_inputs.aggregation_transcript_digest
        );
    }

    #[test]
    fn kagemusha_folded_public_inputs_reject_transcript_projection_mismatches() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let root2 = fixed_hash(b"kagemusha-root-2");
        let steps = vec![
            kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0"),
            kagemusha_step(root1, root2, 0x60, 0x80, b"proof-hop-1"),
        ];
        let public_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &steps).expect("folded inputs");
        let statement =
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &steps)
                .expect("aggregation statement");

        let expect_mismatch = |forged: KagemushaFoldedPublicInputs, field: &'static str| {
            forged
                .public_inputs_hash()
                .expect("forged public inputs remain encodable");
            assert!(matches!(
                kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                    &forged,
                    &statement,
                ),
                Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch {
                    field: actual
                }) if actual == field
            ));
        };

        let mut forged_chain = public_inputs.clone();
        forged_chain.chain_id = "kagemusha-forged-chain".parse().expect("chain id");
        expect_mismatch(forged_chain, "chain_id");

        let mut forged_asset = public_inputs.clone();
        forged_asset.asset = kagemusha_asset("kgm-forged");
        expect_mismatch(forged_asset, "asset");

        let mut forged_initial_root = public_inputs.clone();
        forged_initial_root.initial_root = fixed_hash(b"kagemusha-forged-initial-root");
        expect_mismatch(forged_initial_root, "initial_root");

        let mut forged_final_root = public_inputs.clone();
        forged_final_root.final_root = fixed_hash(b"kagemusha-forged-final-root");
        expect_mismatch(forged_final_root, "final_root");

        let mut forged_hop_count = public_inputs.clone();
        forged_hop_count.hop_count = 1;
        expect_mismatch(forged_hop_count, "hop_count");

        let mut forged_nullifiers = public_inputs.clone();
        forged_nullifiers.nullifier_digest = Hash::new(b"kagemusha-forged-nullifiers");
        expect_mismatch(forged_nullifiers, "nullifier_digest");

        let mut forged_outputs = public_inputs.clone();
        forged_outputs.output_commitment_digest = Hash::new(b"kagemusha-forged-outputs");
        expect_mismatch(forged_outputs, "output_commitment_digest");

        let mut forged_fold = public_inputs.clone();
        forged_fold.fold_digest = Hash::new(b"kagemusha-forged-fold");
        expect_mismatch(forged_fold, "fold_digest");

        let mut forged_aggregation = public_inputs.clone();
        forged_aggregation.aggregation_transcript_digest =
            fixed_hash(b"kagemusha-forged-aggregation");
        expect_mismatch(forged_aggregation, "aggregation_transcript_digest");

        let mut zero_aggregation = public_inputs.clone();
        zero_aggregation.aggregation_transcript_digest = [0u8; Hash::LENGTH];
        assert!(matches!(
            kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                &zero_aggregation,
                &statement,
            ),
            Err(KagemushaFoldError::ZeroFoldedPublicInputDigest {
                field: "aggregation_transcript_digest"
            })
        ));

        let mut forged_domain = public_inputs.clone();
        forged_domain.domain = "iroha:kagemusha:forged-domain".to_owned();
        assert!(matches!(
            kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                &forged_domain,
                &statement,
            ),
            Err(KagemushaFoldError::InvalidPublicInputDomain { .. })
        ));

        let mut forged_mode = public_inputs.clone();
        forged_mode.aggregation_mode = KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
        assert!(matches!(
            kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                &forged_mode,
                &statement,
            ),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));

        let mut forged_statement = statement;
        forged_statement.steps[1].proof_hash = Hash::new(b"kagemusha-forged-hop-proof");
        assert!(matches!(
            kagemusha_validate_folded_public_inputs_against_aggregation_statement(
                &public_inputs,
                &forged_statement,
            ),
            Err(KagemushaFoldError::FoldedPublicInputTranscriptMismatch {
                field: "fold_digest"
            })
        ));
    }

    #[test]
    fn kagemusha_compact_token_binds_folded_proof_public_inputs() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let public_inputs = kagemusha_folded_public_inputs(
            &chain_id,
            &asset,
            &[kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0")],
        )
        .expect("folded inputs");
        let public_inputs_hash = public_inputs
            .public_inputs_hash()
            .expect("folded public inputs hash");
        let token = KagemushaCompactPaymentToken {
            public_inputs: public_inputs.clone(),
            folded_proof: KagemushaFoldedProof {
                verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-folded-v1"),
                public_inputs_hash,
                proof: ProofBox::new("halo2/ipa".into(), vec![0xCA, 0xFE]),
            },
        };
        token
            .validate_public_input_binding()
            .expect("matching compact token binding");

        let mut forged = token.clone();
        forged.folded_proof.public_inputs_hash = Hash::new(b"forged-folded-public-inputs");
        assert!(matches!(
            forged.validate_public_input_binding(),
            Err(KagemushaFoldError::PublicInputHashMismatch { .. })
        ));

        let mut forged_domain = forged.clone();
        forged_domain.public_inputs.domain = "iroha:kagemusha:forged-domain".to_owned();
        forged_domain.folded_proof.public_inputs_hash = forged_domain
            .public_inputs
            .public_inputs_hash()
            .expect("forged-domain hash");
        assert!(matches!(
            forged_domain.validate_public_input_binding(),
            Err(KagemushaFoldError::InvalidPublicInputDomain { .. })
        ));

        let mut forged_zero_hops = forged_domain.clone();
        forged_zero_hops.public_inputs.domain = KAGEMUSHA_FOLDED_PUBLIC_INPUTS_DOMAIN.to_owned();
        forged_zero_hops.public_inputs.hop_count = 0;
        forged_zero_hops.folded_proof.public_inputs_hash = forged_zero_hops
            .public_inputs
            .public_inputs_hash()
            .expect("forged-zero-hop hash");
        assert!(matches!(
            forged_zero_hops.validate_public_input_binding(),
            Err(KagemushaFoldError::Empty)
        ));

        let mut forged_zero_aggregation = forged_zero_hops.clone();
        forged_zero_aggregation.public_inputs.hop_count = 1;
        forged_zero_aggregation
            .public_inputs
            .aggregation_transcript_digest = [0u8; Hash::LENGTH];
        forged_zero_aggregation.folded_proof.public_inputs_hash = forged_zero_aggregation
            .public_inputs
            .public_inputs_hash()
            .expect("forged-zero-aggregation hash");
        assert!(matches!(
            forged_zero_aggregation.validate_public_input_binding(),
            Err(KagemushaFoldError::ZeroFoldedPublicInputDigest {
                field: "aggregation_transcript_digest"
            })
        ));

        let mut forged_zero_initial_root = token.clone();
        forged_zero_initial_root.public_inputs.initial_root = [0u8; Hash::LENGTH];
        forged_zero_initial_root.folded_proof.public_inputs_hash = forged_zero_initial_root
            .public_inputs
            .public_inputs_hash()
            .expect("forged-zero-initial-root hash");
        assert!(matches!(
            forged_zero_initial_root.validate_public_input_binding(),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "initial_root"
            })
        ));

        let mut forged_zero_final_root = token.clone();
        forged_zero_final_root.public_inputs.final_root = [0u8; Hash::LENGTH];
        forged_zero_final_root.folded_proof.public_inputs_hash = forged_zero_final_root
            .public_inputs
            .public_inputs_hash()
            .expect("forged-zero-final-root hash");
        assert!(matches!(
            forged_zero_final_root.validate_public_input_binding(),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "final_root"
            })
        ));

        let mut forged_unchanged_roots = token.clone();
        forged_unchanged_roots.public_inputs.final_root =
            forged_unchanged_roots.public_inputs.initial_root;
        forged_unchanged_roots.folded_proof.public_inputs_hash = forged_unchanged_roots
            .public_inputs
            .public_inputs_hash()
            .expect("forged-unchanged-root hash");
        assert!(matches!(
            forged_unchanged_roots.validate_public_input_binding(),
            Err(KagemushaFoldError::UnchangedFoldedPublicRoots)
        ));

        let mut forged_too_many_hops = forged_zero_hops.clone();
        forged_too_many_hops.public_inputs.hop_count =
            u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1).expect("hop count fits");
        forged_too_many_hops.folded_proof.public_inputs_hash = forged_too_many_hops
            .public_inputs
            .public_inputs_hash()
            .expect("forged-too-many-hop hash");
        assert!(matches!(
            forged_too_many_hops.validate_public_input_binding(),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));

        let mut forged_mode = forged;
        forged_mode.public_inputs.aggregation_mode =
            KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1;
        forged_mode.folded_proof.public_inputs_hash = forged_mode
            .public_inputs
            .public_inputs_hash()
            .expect("forged-mode hash");
        assert!(matches!(
            forged_mode.validate_public_input_binding(),
            Err(KagemushaFoldError::UnsupportedAggregationMode { actual, .. })
                if actual == KAGEMUSHA_AGGREGATION_MODE_RECURSIVE_IN_CIRCUIT_V1
        ));
        let err = forged_mode
            .validate_public_input_binding()
            .expect_err("reserved recursive mode must be rejected");
        assert!(
            err.to_string()
                .contains("requires ABI-7 recursive compact-token admission")
        );
    }

    fn kagemusha_verified_fold_record_bundle_fixture() -> KagemushaVerifiedFoldRecordBundle {
        let chain_id: ChainId = "kagemusha-record-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm-record");
        let vk_id = VerifyingKeyId::new("halo2/ipa", "kagemusha-hop-fixture");
        let verifier_key = VerifyingKeyBox::new("halo2/ipa".into(), vec![0x42; 48]);
        let vk_commitment = kagemusha_verifying_key_commitment(&verifier_key);
        let proof_schema = b"kagemusha-record-hop-public-inputs-v1".to_vec();
        let proof_envelope = crate::zk::OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:tiny-add".to_owned(),
            vk_hash: vk_commitment,
            public_inputs: proof_schema.clone(),
            proof_bytes: vec![0xCA, 0xFE, 0x01],
            aux: Vec::new(),
        };
        let proof = ProofBox::new(
            "halo2/ipa".into(),
            to_bytes(&proof_envelope).expect("encode hop OpenVerifyEnvelope"),
        );
        let mut attachment = ProofAttachment::new_ref("halo2/ipa".into(), proof, vk_id.clone());
        attachment.vk_commitment = Some(vk_commitment);
        let step = KagemushaVerifiedFoldStep {
            root_before: fixed_hash(b"kagemusha-record-root-0"),
            input_nullifiers: vec![fixed_hash(b"kagemusha-record-nullifier")],
            output_commitments: vec![fixed_hash(b"kagemusha-record-output")],
            root_after: fixed_hash(b"kagemusha-record-root-1"),
            attachment,
            verifier_key: verifier_key.clone(),
        };
        let bundle = KagemushaVerifiedFoldBundle {
            chain_id,
            asset,
            steps: vec![step],
        };
        let mut record = VerifyingKeyRecord::new(
            1,
            "halo2/ipa:tiny-add",
            BackendTag::Halo2IpaPasta,
            "pallas",
            Hash::new(proof_schema.as_slice()).into(),
            vk_commitment,
        );
        record.status = ConfidentialStatus::Active;
        record.namespace = KAGEMUSHA_VERIFIER_NAMESPACE.to_owned();
        record.vk_len = u32::try_from(verifier_key.bytes.len()).expect("vk length fits");
        record.max_proof_bytes = 4096;
        record.key = Some(verifier_key);
        KagemushaVerifiedFoldRecordBundle {
            bundle,
            verifier_records: vec![KagemushaVerifiedFoldVerifierRecord { id: vk_id, record }],
        }
    }

    #[test]
    fn kagemusha_verified_fold_record_bundle_roundtrips() {
        let record_bundle = kagemusha_verified_fold_record_bundle_fixture();

        let bytes = to_bytes(&record_bundle).expect("encode record-backed bundle");
        let decoded: KagemushaVerifiedFoldRecordBundle =
            norito::decode_from_bytes(&bytes).expect("decode record-backed bundle");
        assert_eq!(decoded, record_bundle);
    }

    #[test]
    fn kagemusha_recursive_spend_bridge_abi_archives_roundtrip() {
        let chain_id: ChainId = "kagemusha-recursive-spend-abi-chain"
            .parse()
            .expect("chain id");
        let asset = kagemusha_asset("kgm-recursive-abi");
        let root0 = fixed_hash(b"kagemusha-recursive-spend-abi-root-0");
        let root1 = fixed_hash(b"kagemusha-recursive-spend-abi-root-1");
        let root2 = fixed_hash(b"kagemusha-recursive-spend-abi-root-2");

        let step0 = kagemusha_step(root0, root1, 0x32, 0x74, b"recursive-spend-abi-hop-0");
        let note0 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step0.output_commitments[0],
            spend_nullifier: fixed_hash(b"kagemusha-recursive-spend-abi-nullifier"),
            amount: Numeric::new(7, 0),
        };
        let evidence0 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step0.clone(),
            b"recursive-spend-abi-witness-hop",
        );
        let accumulator0 =
            kagemusha_recursive_spend_accumulator_from_initial_evidence(&evidence0, &note0)
                .expect("recursive spend ABI accumulator");
        let bundle0 = kagemusha_recursive_spend_bundle(accumulator0);
        let init_record_bundle = kagemusha_recursive_spend_record_bundle_for_step(
            chain_id.clone(),
            asset.clone(),
            &step0,
            "kagemusha-recursive-spend-abi-hop-0",
            b"recursive-spend-abi-proof-hop-0",
        );
        let init_pallas_open_envelopes_archive =
            kagemusha_recursive_spend_lineage_pallas_open_envelope_archive(
                &init_record_bundle,
                0x41,
            );
        let init_without_key_artifacts = KagemushaRecursiveSpendInitRequestV1::new(
            init_record_bundle.clone(),
            init_pallas_open_envelopes_archive.clone(),
            note0.clone(),
        )
        .expect("ABI init request validates before proving");
        assert_eq!(
            init_without_key_artifacts
                .clone()
                .with_block_height(12)
                .expect("ABI init request accepts block height")
                .block_height,
            Some(12)
        );
        assert!(matches!(
            init_without_key_artifacts
                .clone()
                .with_lineage_key_artifacts(
                    VerifyingKeyBox::new("halo2/ipa".into(), Vec::new()),
                    vec![0xE8; 64],
                ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));
        assert!(matches!(
            init_without_key_artifacts
                .clone()
                .with_lineage_key_artifacts(
                    VerifyingKeyBox::new("halo2/ipa".into(), vec![0xE7; 64]),
                    Vec::new(),
                ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new(
                "kagemusha-recursive-spend-lineage-forged-circuit",
                2,
                VerifyingKeyBox::new("halo2/ipa".into(), vec![0xE7; 64]),
                vec![0xE8; 64],
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof_circuit_id"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                3,
                VerifyingKeyBox::new("halo2/ipa".into(), vec![0xE7; 64]),
                vec![0xE8; 64],
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "verifier_opening_len"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                VerifyingKeyBox::new("halo2/kzg".into(), vec![0xE7; 64]),
                vec![0xE8; 64],
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                VerifyingKeyBox::new("halo2/ipa".into(), Vec::new()),
                vec![0xE8; 64],
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));
        assert!(matches!(
            KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
                2,
                VerifyingKeyBox::new("halo2/ipa".into(), vec![0xE7; 64]),
                Vec::new(),
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));
        let init_lineage_verifier_key = kagemusha_lineage_key_artifact_vk(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            0xE7,
        );
        let init_lineage_proving_key_archive = kagemusha_lineage_key_artifact_pk_archive(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            &init_lineage_verifier_key,
            0xE8,
        );
        let append_lineage_verifier_key = kagemusha_lineage_key_artifact_vk(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            0xA7,
        );
        let append_lineage_proving_key_archive = kagemusha_lineage_key_artifact_pk_archive(
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            &append_lineage_verifier_key,
            0xA8,
        );
        let init_artifacts = KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_init(
            2,
            init_lineage_verifier_key.clone(),
            init_lineage_proving_key_archive.clone(),
        )
        .expect("init Reserved-lineage key artifact package validates");
        assert!(init_artifacts.is_init_artifact());
        assert!(!init_artifacts.is_append_artifact());
        assert!(
            init_artifacts
                .norito_encoded_len()
                .expect("encode init artifact package length")
                > 0
        );
        let init_artifacts_bytes = to_bytes(&init_artifacts).expect("encode init artifact package");
        let decoded_init_artifacts: KagemushaRecursiveSpendLineageKeyArtifactsV1 =
            norito::decode_from_bytes(&init_artifacts_bytes).expect("decode init artifact package");
        assert_eq!(decoded_init_artifacts, init_artifacts);
        let init_from_artifact_package = init_without_key_artifacts
            .clone()
            .with_lineage_key_artifact_package(init_artifacts.clone())
            .expect("ABI init request builder accepts Reserved-lineage key artifact package");
        assert!(matches!(
            init_without_key_artifacts
                .clone()
                .with_lineage_key_artifact_package(
                    KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_append(
                        2,
                        append_lineage_verifier_key.clone(),
                        append_lineage_proving_key_archive.clone(),
                    )
                    .expect("append artifact package validates"),
                ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof_circuit_id"
            })
        ));
        let init = init_without_key_artifacts
            .with_lineage_key_artifacts(
                init_lineage_verifier_key.clone(),
                init_lineage_proving_key_archive.clone(),
            )
            .expect("ABI init request builder accepts Reserved-lineage key material");
        assert_eq!(init_from_artifact_package, init);
        let init_from_production_builder =
            KagemushaRecursiveSpendInitRequestV1::new_with_lineage_key_artifacts(
                init_record_bundle.clone(),
                init_pallas_open_envelopes_archive.clone(),
                note0.clone(),
                init_lineage_verifier_key.clone(),
                init_lineage_proving_key_archive.clone(),
            )
            .expect("ABI init production builder accepts Reserved-lineage key material");
        assert_eq!(init_from_production_builder, init);
        let init_from_artifact_package_builder =
            KagemushaRecursiveSpendInitRequestV1::new_with_lineage_key_artifact_package(
                init_record_bundle.clone(),
                init_pallas_open_envelopes_archive.clone(),
                note0.clone(),
                init_artifacts.clone(),
            )
            .expect("ABI init production builder accepts Reserved-lineage key artifact package");
        assert_eq!(init_from_artifact_package_builder, init);
        let mut init_missing_proving_key = init.clone();
        init_missing_proving_key.lineage_proving_key_archive = None;
        assert!(matches!(
            init_missing_proving_key.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));
        let mut init_missing_verifier_key = init.clone();
        init_missing_verifier_key.lineage_verifier_key = None;
        assert!(matches!(
            init_missing_verifier_key.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));
        let init_bytes = to_bytes(&init).expect("encode recursive spend init request");
        let decoded_init: KagemushaRecursiveSpendInitRequestV1 =
            norito::decode_from_bytes(&init_bytes).expect("decode recursive spend init request");
        assert_eq!(decoded_init, init);

        #[derive(Encode)]
        struct LegacyKagemushaRecursiveSpendInitRequestV1 {
            record_bundle: KagemushaVerifiedFoldRecordBundle,
            pallas_open_envelopes_archive: Vec<u8>,
            current_note: KagemushaSpendableNoteDescriptorV1,
        }

        let legacy_init = LegacyKagemushaRecursiveSpendInitRequestV1 {
            record_bundle: init_record_bundle.clone(),
            pallas_open_envelopes_archive: init_pallas_open_envelopes_archive.clone(),
            current_note: note0.clone(),
        };
        let mut legacy_init_bytes =
            to_bytes(&legacy_init).expect("encode legacy recursive spend init request");
        let init_request_schema =
            <KagemushaRecursiveSpendInitRequestV1 as norito::NoritoSerialize>::schema_hash();
        legacy_init_bytes[6..22].copy_from_slice(&init_request_schema);
        let decoded_legacy_init: KagemushaRecursiveSpendInitRequestV1 =
            norito::decode_from_bytes(&legacy_init_bytes)
                .expect("decode legacy recursive spend init request with defaults");
        assert_eq!(decoded_legacy_init.record_bundle, legacy_init.record_bundle);
        assert_eq!(
            decoded_legacy_init.pallas_open_envelopes_archive,
            legacy_init.pallas_open_envelopes_archive
        );
        assert_eq!(decoded_legacy_init.current_note, legacy_init.current_note);
        assert!(decoded_legacy_init.lineage_verifier_key.is_none());
        assert!(decoded_legacy_init.lineage_proving_key_archive.is_none());
        assert!(decoded_legacy_init.block_height.is_none());

        let transition_profile_init =
            kagemusha_recursive_spend_transition_profile_from_initial_evidence(&evidence0, &note0)
                .expect("initial transition profile");
        let transition_profile_init_bytes =
            to_bytes(&transition_profile_init).expect("encode initial transition profile");
        let decoded_transition_profile_init: KagemushaRecursiveSpendTransitionProfileV1 =
            norito::decode_from_bytes(&transition_profile_init_bytes)
                .expect("decode initial transition profile");
        assert_eq!(decoded_transition_profile_init, transition_profile_init);

        let mut init_lineage_bundle = bundle0.clone();
        init_lineage_bundle.recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &init_lineage_bundle.accumulator,
            b"kagemusha-recursive-spend-abi-lineage-one-hop-scalar",
        );
        init_lineage_bundle.recursive_proof.verifier_key_id.name =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1.into();
        attach_recursive_spend_open_verify_envelope(
            &mut init_lineage_bundle,
            b"kagemusha-recursive-spend-abi-lineage-one-hop-vk",
        );
        init_lineage_bundle
            .validate_public_input_binding()
            .expect("one-hop Reserved-lineage bundle binds public inputs");
        let witness0 =
            kagemusha_recursive_spend_lineage_witness_from_init_result(&init, &init_lineage_bundle)
                .expect("initial lineage witness");
        let witness0_bytes = to_bytes(&witness0).expect("encode initial lineage witness");
        let decoded_witness0: KagemushaRecursiveSpendLineageWitnessV1 =
            norito::decode_from_bytes(&witness0_bytes).expect("decode initial lineage witness");
        assert_eq!(decoded_witness0, witness0);

        let mut previous_lineage_verifier_record =
            kagemusha_recursive_spend_active_lineage_verifier_record();
        previous_lineage_verifier_record.circuit_id = init_lineage_bundle
            .recursive_proof
            .verifier_key_id
            .name
            .clone();
        let previous_recursive_proof_open_envelopes_archive =
            kagemusha_recursive_spend_previous_proof_open_envelope_archive(
                &init_lineage_bundle,
                0x71,
            );

        let mut step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"recursive-spend-abi-hop-1");
        step1.input_nullifiers = vec![note0.spend_nullifier];
        let note1 = KagemushaSpendableNoteDescriptorV1 {
            note_commitment: step1.output_commitments[1],
            spend_nullifier: fixed_hash(b"kagemusha-recursive-spend-abi-nullifier-1"),
            amount: Numeric::new(7, 0),
        };
        let evidence1 = kagemusha_recursive_spend_one_hop_evidence(
            &chain_id,
            &asset,
            step1.clone(),
            b"recursive-spend-abi-witness-hop-1",
        );
        let append_record_bundle = kagemusha_recursive_spend_record_bundle_for_step(
            chain_id.clone(),
            asset.clone(),
            &step1,
            "kagemusha-recursive-spend-abi-hop-1",
            b"recursive-spend-abi-proof-hop-1",
        );
        let append_pallas_open_envelopes_archive =
            kagemusha_recursive_spend_lineage_pallas_open_envelope_archive(
                &append_record_bundle,
                0x51,
            );
        let append_without_key_artifacts =
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness_and_output_circuit(
                KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                init_lineage_bundle.clone(),
                Some(previous_lineage_verifier_record.clone()),
                previous_recursive_proof_open_envelopes_archive.clone(),
                append_record_bundle.clone(),
                append_pallas_open_envelopes_archive.clone(),
                note1.clone(),
            )
            .expect("ABI Reserved-lineage append request validates before proving");
        assert_eq!(
            append_without_key_artifacts
                .clone()
                .with_block_height(13)
                .expect("ABI append request accepts block height")
                .block_height,
            Some(13)
        );
        assert!(matches!(
            append_without_key_artifacts
                .clone()
                .with_lineage_key_artifacts(
                    VerifyingKeyBox::new("halo2/ipa".into(), Vec::new()),
                    vec![0xA8; 64],
                ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));
        assert!(matches!(
            append_without_key_artifacts
                .clone()
                .with_lineage_key_artifacts(
                    VerifyingKeyBox::new("halo2/ipa".into(), vec![0xA7; 64]),
                    Vec::new(),
                ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));
        let append_artifacts = KagemushaRecursiveSpendLineageKeyArtifactsV1::new_for_append(
            2,
            append_lineage_verifier_key.clone(),
            append_lineage_proving_key_archive.clone(),
        )
        .expect("append Reserved-lineage key artifact package validates");
        assert!(append_artifacts.is_append_artifact());
        assert!(!append_artifacts.is_init_artifact());
        assert!(
            append_artifacts
                .norito_encoded_len()
                .expect("encode append artifact package length")
                > 0
        );
        let append_artifacts_bytes =
            to_bytes(&append_artifacts).expect("encode append artifact package");
        let decoded_append_artifacts: KagemushaRecursiveSpendLineageKeyArtifactsV1 =
            norito::decode_from_bytes(&append_artifacts_bytes)
                .expect("decode append artifact package");
        assert_eq!(decoded_append_artifacts, append_artifacts);
        assert!(matches!(
            append_without_key_artifacts
                .clone()
                .with_lineage_key_artifact_package(init_artifacts),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "proof_circuit_id"
            })
        ));
        let append_from_artifact_package = append_without_key_artifacts
            .clone()
            .with_lineage_key_artifact_package(append_artifacts.clone())
            .expect("ABI append request builder accepts Reserved-lineage key artifact package");
        let semantic_append_without_key_artifacts =
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_proof_witness_and_output_circuit(
                KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                bundle0.clone(),
                None,
                Vec::new(),
                append_record_bundle.clone(),
                append_pallas_open_envelopes_archive.clone(),
                note1.clone(),
            )
            .expect("ABI semantic append request validates without Reserved-lineage key material");
        assert!(matches!(
            semantic_append_without_key_artifacts
                .clone()
                .with_lineage_key_artifact_package(append_artifacts.clone()),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_key_artifacts"
            })
        ));
        assert!(matches!(
            semantic_append_without_key_artifacts.with_lineage_key_artifacts(
                VerifyingKeyBox::new("halo2/ipa".into(), vec![0xA7; 64]),
                vec![0xA8; 64],
            ),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_key_artifacts"
            })
        ));
        let append = append_without_key_artifacts
            .with_lineage_key_artifacts(
                append_lineage_verifier_key.clone(),
                append_lineage_proving_key_archive.clone(),
            )
            .expect("ABI append request builder accepts Reserved-lineage key material");
        assert_eq!(append_from_artifact_package, append);
        let append_from_production_builder =
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_lineage_proof_witness_and_key_artifacts(
                init_lineage_bundle.clone(),
                Some(previous_lineage_verifier_record.clone()),
                previous_recursive_proof_open_envelopes_archive.clone(),
                append_record_bundle.clone(),
                append_pallas_open_envelopes_archive.clone(),
                note1.clone(),
                append_lineage_verifier_key.clone(),
                append_lineage_proving_key_archive.clone(),
            )
            .expect("ABI append production builder accepts Reserved-lineage key material");
        assert_eq!(append_from_production_builder, append);
        let append_from_artifact_package_builder =
            KagemushaRecursiveSpendAppendRequestV1::new_with_previous_lineage_proof_witness_and_key_artifact_package(
                init_lineage_bundle.clone(),
                Some(previous_lineage_verifier_record.clone()),
                previous_recursive_proof_open_envelopes_archive.clone(),
                append_record_bundle.clone(),
                append_pallas_open_envelopes_archive.clone(),
                note1.clone(),
                append_artifacts,
            )
            .expect("ABI append production builder accepts Reserved-lineage key artifact package");
        assert_eq!(append_from_artifact_package_builder, append);
        let mut append_missing_proving_key = append.clone();
        append_missing_proving_key.lineage_proving_key_archive = None;
        assert!(matches!(
            append_missing_proving_key.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_proving_key_archive"
            })
        ));
        let mut append_missing_verifier_key = append.clone();
        append_missing_verifier_key.lineage_verifier_key = None;
        assert!(matches!(
            append_missing_verifier_key.validate_public_binding(),
            Err(KagemushaFoldError::InvalidRecursiveSpendProof {
                field: "lineage_verifier_key"
            })
        ));
        let append_bytes = to_bytes(&append).expect("encode recursive spend append request");
        let decoded_append: KagemushaRecursiveSpendAppendRequestV1 =
            norito::decode_from_bytes(&append_bytes)
                .expect("decode recursive spend append request");
        assert_eq!(decoded_append, append);

        let previous_recursive_proof_open_envelopes_archive_digest =
            kagemusha_recursive_previous_proof_open_envelopes_archive_digest(
                &previous_recursive_proof_open_envelopes_archive,
            )
            .expect("previous proof opening archive digest");
        let append_opening_preflight_contract =
            KagemushaRecursiveSpendLineageAppendOpeningPreflightV1::new(
                kagemusha_recursive_verifier_preflight_for_evidence(
                    &evidence1,
                    fixed_hash(b"kagemusha-recursive-spend-abi-previous-opening"),
                ),
                kagemusha_recursive_verifier_preflight_for_evidence(
                    &evidence1,
                    evidence1.verifier_witness_batch_digest,
                ),
                kagemusha_recursive_spend_accumulator_digest(&init_lineage_bundle.accumulator)
                    .expect("previous accumulator digest"),
                kagemusha_recursive_spend_proof_artifact_digest(
                    &init_lineage_bundle.recursive_proof,
                )
                .expect("previous proof artifact digest"),
                previous_recursive_proof_open_envelopes_archive_digest,
                evidence1.aggregation_statement.steps[0].proof_hash,
            )
            .expect("append opening preflight contract");
        let transition_profile_append =
            kagemusha_recursive_spend_transition_profile_append_evidence_with_opening_preflight_contract(
                &init_lineage_bundle.accumulator,
                &init_lineage_bundle.recursive_proof,
                &previous_recursive_proof_open_envelopes_archive,
                append_opening_preflight_contract.clone(),
                &evidence1,
                &note1,
            )
            .expect("append transition profile");
        let transition_profile_append_bytes =
            to_bytes(&transition_profile_append).expect("encode append transition profile");
        let decoded_transition_profile_append: KagemushaRecursiveSpendTransitionProfileV1 =
            norito::decode_from_bytes(&transition_profile_append_bytes)
                .expect("decode append transition profile");
        assert_eq!(decoded_transition_profile_append, transition_profile_append);

        let append_boundary =
            kagemusha_recursive_spend_lineage_append_boundary_from_transition_profile(
                &transition_profile_append,
            )
            .expect("append boundary");
        let append_boundary_bytes = to_bytes(&append_boundary).expect("encode append boundary");
        let decoded_append_boundary: KagemushaRecursiveSpendLineageAppendBoundaryV1 =
            norito::decode_from_bytes(&append_boundary_bytes).expect("decode append boundary");
        assert_eq!(decoded_append_boundary, append_boundary);

        let accumulator1 =
            kagemusha_recursive_spend_accumulator_append_evidence_with_opening_preflight_contract(
                &init_lineage_bundle.accumulator,
                &init_lineage_bundle.recursive_proof,
                &previous_recursive_proof_open_envelopes_archive,
                append_opening_preflight_contract,
                &evidence1,
                &note1,
            )
            .expect("append accumulator");
        let mut append_recursive_proof = kagemusha_recursive_spend_lineage_proof(
            &accumulator1,
            b"kagemusha-recursive-spend-abi-lineage-append-scalar",
        );
        append_recursive_proof.verifier_key_id.name =
            KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1.into();
        let mut appended_bundle = KagemushaRecursiveSpendBundleV1 {
            accumulator: accumulator1,
            recursive_proof: append_recursive_proof,
        };
        attach_recursive_spend_open_verify_envelope(
            &mut appended_bundle,
            b"kagemusha-recursive-spend-abi-lineage-append-vk",
        );
        appended_bundle
            .validate_public_input_binding()
            .expect("append Reserved-lineage bundle binds public inputs");
        let appended_bundle_bytes =
            to_bytes(&appended_bundle).expect("encode appended bundle fixture");
        let witness1 = kagemusha_recursive_spend_lineage_witness_append_result(
            &witness0,
            &append,
            &appended_bundle,
        )
        .expect("append lineage witness");
        let witness1_bytes = to_bytes(&witness1).expect("encode append lineage witness");
        let decoded_witness1: KagemushaRecursiveSpendLineageWitnessV1 =
            norito::decode_from_bytes(&witness1_bytes).expect("decode append lineage witness");
        assert_eq!(decoded_witness1, witness1);

        #[derive(Encode)]
        struct LegacyKagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle: KagemushaRecursiveSpendBundleV1,
            record_bundle: KagemushaVerifiedFoldRecordBundle,
            pallas_open_envelopes_archive: Vec<u8>,
            current_note: KagemushaSpendableNoteDescriptorV1,
        }

        let legacy_append = LegacyKagemushaRecursiveSpendAppendRequestV1 {
            previous_bundle: bundle0.clone(),
            record_bundle: init_record_bundle.clone(),
            pallas_open_envelopes_archive: init_pallas_open_envelopes_archive,
            current_note: note0.clone(),
        };
        let mut legacy_append_bytes =
            to_bytes(&legacy_append).expect("encode legacy recursive spend append request");
        let append_request_schema =
            <KagemushaRecursiveSpendAppendRequestV1 as norito::NoritoSerialize>::schema_hash();
        legacy_append_bytes[6..22].copy_from_slice(&append_request_schema);
        let decoded_legacy_append: KagemushaRecursiveSpendAppendRequestV1 =
            norito::decode_from_bytes(&legacy_append_bytes)
                .expect("decode legacy recursive spend append request with default verifier");
        assert_eq!(
            decoded_legacy_append.previous_bundle,
            legacy_append.previous_bundle
        );
        assert_eq!(
            decoded_legacy_append.record_bundle,
            legacy_append.record_bundle
        );
        assert_eq!(
            decoded_legacy_append.pallas_open_envelopes_archive,
            legacy_append.pallas_open_envelopes_archive
        );
        assert_eq!(
            decoded_legacy_append.current_note,
            legacy_append.current_note
        );
        assert!(
            decoded_legacy_append
                .previous_lineage_verifier_record
                .is_none()
        );
        assert!(decoded_legacy_append.output_proof_circuit_id.is_empty());
        assert_eq!(
            decoded_legacy_append.output_proof_circuit_id(),
            KAGEMUSHA_RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
        );
        assert!(
            decoded_legacy_append
                .previous_recursive_proof_open_envelopes_archive
                .is_empty()
        );
        assert!(decoded_legacy_append.lineage_verifier_key.is_none());
        assert!(decoded_legacy_append.lineage_proving_key_archive.is_none());
        assert!(decoded_legacy_append.block_height.is_none());

        let mut final_lineage_verifier_record =
            kagemusha_recursive_spend_active_lineage_verifier_record();
        final_lineage_verifier_record.circuit_id =
            appended_bundle.recursive_proof.verifier_key_id.name.clone();
        let verify = KagemushaRecursiveSpendVerifyRequestV1 {
            bundle: appended_bundle.clone(),
            lineage_verifier_record: Some(final_lineage_verifier_record.clone()),
            block_height: Some(14),
        };
        let verify_bytes = to_bytes(&verify).expect("encode recursive spend verify request");
        let decoded_verify: KagemushaRecursiveSpendVerifyRequestV1 =
            norito::decode_from_bytes(&verify_bytes)
                .expect("decode recursive spend verify request");
        assert_eq!(decoded_verify, verify);

        #[derive(Encode)]
        struct LegacyKagemushaRecursiveSpendVerifyRequestV1 {
            bundle: KagemushaRecursiveSpendBundleV1,
        }

        let legacy_verify = LegacyKagemushaRecursiveSpendVerifyRequestV1 {
            bundle: bundle0.clone(),
        };
        let mut legacy_verify_bytes =
            to_bytes(&legacy_verify).expect("encode legacy recursive spend verify request");
        let verify_request_schema =
            <KagemushaRecursiveSpendVerifyRequestV1 as norito::NoritoSerialize>::schema_hash();
        legacy_verify_bytes[6..22].copy_from_slice(&verify_request_schema);
        let decoded_legacy_verify: KagemushaRecursiveSpendVerifyRequestV1 =
            norito::decode_from_bytes(&legacy_verify_bytes)
                .expect("decode legacy recursive spend verify request with default verifier");
        assert_eq!(decoded_legacy_verify.bundle, legacy_verify.bundle);
        assert!(decoded_legacy_verify.lineage_verifier_record.is_none());
        assert!(decoded_legacy_verify.block_height.is_none());

        let verify_result = KagemushaRecursiveSpendVerifyResultV1 {
            valid: false,
            hop_count: appended_bundle.accumulator.hop_count,
            encoded_bytes: u32::try_from(
                appended_bundle
                    .norito_encoded_len()
                    .expect("recursive spend bundle encoded length"),
            )
            .expect("encoded length fits u32"),
            reason: "fixture recursive proof is not a production proof".to_owned(),
            chain_admissible: false,
            chain_admission_reason: "offline verification failed".to_owned(),
            witnessless_redeem_supported: false,
            lineage_witness_required_for_redeem: true,
        };
        let verify_result_bytes =
            to_bytes(&verify_result).expect("encode recursive spend verify result");
        let decoded_verify_result: KagemushaRecursiveSpendVerifyResultV1 =
            norito::decode_from_bytes(&verify_result_bytes)
                .expect("decode recursive spend verify result");
        assert!(!decoded_verify_result.witnessless_redeem_supported);
        assert!(decoded_verify_result.lineage_witness_required_for_redeem);
        assert_eq!(decoded_verify_result, verify_result);

        let mut redeem_proof = ProofAttachment::new_ref(
            "halo2/ipa".to_owned(),
            ProofBox::new("halo2/ipa".to_owned(), vec![0xA7; 64]),
            VerifyingKeyId::new("halo2/ipa", "kagemusha-recursive-spend-abi-redeem"),
        );
        redeem_proof.vk_commitment = Some(fixed_hash(b"kagemusha-recursive-spend-abi-redeem-vk"));
        let recipient = sample_account(0xAB, "offline");
        let redeem = KagemushaRecursiveSpendRedeemRequestV1::new_with_lineage_witness(
            appended_bundle.clone(),
            recipient.clone(),
            7,
            redeem_proof.clone(),
            Some(witness1.clone()),
            Some(final_lineage_verifier_record),
        )
        .expect("ABI redeem request validates with lineage witness");
        let redeem_bytes = to_bytes(&redeem).expect("encode recursive spend redeem request");
        let decoded_redeem: KagemushaRecursiveSpendRedeemRequestV1 =
            norito::decode_from_bytes(&redeem_bytes)
                .expect("decode recursive spend redeem request");
        assert_eq!(decoded_redeem, redeem);

        #[derive(Encode)]
        struct LegacyKagemushaRecursiveSpendRedeemRequestV1 {
            bundle: KagemushaRecursiveSpendBundleV1,
            recipient: AccountId,
            public_amount: u128,
            redeem_proof: ProofAttachment,
            lineage_witness: Option<KagemushaRecursiveSpendLineageWitnessV1>,
            lineage_verifier_record: Option<VerifyingKeyRecord>,
            block_height: Option<u64>,
        }

        let legacy_redeem = LegacyKagemushaRecursiveSpendRedeemRequestV1 {
            bundle: redeem.bundle.clone(),
            recipient: redeem.recipient.clone(),
            public_amount: redeem.public_amount,
            redeem_proof: redeem.redeem_proof.clone(),
            lineage_witness: redeem.lineage_witness.clone(),
            lineage_verifier_record: redeem.lineage_verifier_record.clone(),
            block_height: redeem.block_height,
        };
        let mut legacy_redeem_bytes =
            to_bytes(&legacy_redeem).expect("encode legacy recursive spend redeem request");
        let redeem_request_schema =
            <KagemushaRecursiveSpendRedeemRequestV1 as norito::NoritoSerialize>::schema_hash();
        legacy_redeem_bytes[6..22].copy_from_slice(&redeem_request_schema);
        assert!(
            norito::decode_from_bytes::<KagemushaRecursiveSpendRedeemRequestV1>(
                &legacy_redeem_bytes
            )
            .is_err(),
            "recursive spend redeem requests must carry explicit change_output in first-release V1"
        );

        let redeem_instruction =
            crate::isi::offline::RedeemKagemushaRecursive::new_with_lineage_witness(
                appended_bundle,
                recipient,
                7,
                redeem_proof,
                Some(witness1),
            );
        let redeem_instruction_bytes =
            to_bytes(&redeem_instruction).expect("encode recursive spend redeem instruction");
        let decoded_redeem_instruction: crate::isi::offline::RedeemKagemushaRecursive =
            norito::decode_from_bytes(&redeem_instruction_bytes)
                .expect("decode recursive spend redeem instruction");
        assert_eq!(decoded_redeem_instruction, redeem_instruction);

        assert_shared_recursive_spend_abi6_archive_fixture_matches(&[
            (
                "init_request",
                "init",
                "KagemushaRecursiveSpendInitRequestV1",
                &init_bytes,
            ),
            (
                "init_bundle",
                "init",
                "KagemushaRecursiveSpendBundleV1",
                &to_bytes(&init_lineage_bundle).expect("encode init lineage bundle fixture"),
            ),
            (
                "transition_profile_init",
                "transition_profile_init",
                "KagemushaRecursiveSpendTransitionProfileV1",
                &transition_profile_init_bytes,
            ),
            (
                "append_request",
                "append",
                "KagemushaRecursiveSpendAppendRequestV1",
                &append_bytes,
            ),
            (
                "append_bundle",
                "append",
                "KagemushaRecursiveSpendBundleV1",
                &appended_bundle_bytes,
            ),
            (
                "transition_profile_append",
                "transition_profile_append",
                "KagemushaRecursiveSpendTransitionProfileV1",
                &transition_profile_append_bytes,
            ),
            (
                "lineage_append_boundary",
                "lineage_append_boundary",
                "KagemushaRecursiveSpendLineageAppendBoundaryV1",
                &append_boundary_bytes,
            ),
            (
                "lineage_witness_from_init_result",
                "lineage_witness_from_init_result",
                "KagemushaRecursiveSpendLineageWitnessV1",
                &witness0_bytes,
            ),
            (
                "lineage_witness_append_result",
                "lineage_witness_append_result",
                "KagemushaRecursiveSpendLineageWitnessV1",
                &witness1_bytes,
            ),
            (
                "verify_request",
                "verify",
                "KagemushaRecursiveSpendVerifyRequestV1",
                &verify_bytes,
            ),
            (
                "verify_result",
                "verify",
                "KagemushaRecursiveSpendVerifyResultV1",
                &verify_result_bytes,
            ),
            (
                "redeem_request",
                "redeem",
                "KagemushaRecursiveSpendRedeemRequestV1",
                &redeem_bytes,
            ),
            (
                "redeem_instruction",
                "redeem",
                "RedeemKagemushaRecursive",
                &redeem_instruction_bytes,
            ),
        ]);
    }

    #[test]
    fn kagemusha_folded_public_inputs_stay_size_bounded_at_max_hops() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let roots = (0..=KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS)
            .map(|index| fixed_hash(format!("kagemusha-size-root-{index}").as_bytes()))
            .collect::<Vec<_>>();
        let steps = (0..KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS)
            .map(|index| {
                let input_seed = u8::try_from(index * 2 + 1).expect("bounded input seed");
                let output_seed = u8::try_from(128 + index * 2).expect("bounded output seed");
                let proof_label = format!("kagemusha-size-proof-hop-{index}");
                let mut step = kagemusha_step(
                    roots[index],
                    roots[index + 1],
                    input_seed,
                    output_seed,
                    b"kagemusha-size-proof",
                );
                step.proof_hash = Hash::new(proof_label.as_bytes());
                step.proof_public_inputs_digest =
                    fixed_hash(format!("{proof_label}:public-inputs").as_bytes());
                step.verifier_key_commitment = fixed_hash(format!("{proof_label}:vk").as_bytes());
                step.verifier_key_poseidon_digest =
                    kagemusha_verifier_key_poseidon_digest("halo2/ipa", proof_label.as_bytes())
                        .expect("size verifier-key digest");
                step.input_nullifiers = vec![
                    fixed_hash(format!("{proof_label}:input-a").as_bytes()),
                    fixed_hash(format!("{proof_label}:input-b").as_bytes()),
                ];
                step.input_nullifiers.sort_unstable();
                step.output_commitments = vec![
                    fixed_hash(format!("{proof_label}:output-a").as_bytes()),
                    fixed_hash(format!("{proof_label}:output-b").as_bytes()),
                ];
                step.output_commitments.sort_unstable();
                step
            })
            .collect::<Vec<_>>();

        let public_inputs =
            kagemusha_folded_public_inputs(&chain_id, &asset, &steps).expect("folded inputs");
        assert_eq!(
            public_inputs.hop_count,
            u32::try_from(KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS).expect("hop count fits")
        );
        let public_inputs_len = public_inputs
            .norito_encoded_len()
            .expect("folded public inputs encoded length");
        assert!(
            public_inputs_len <= KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES,
            "folded public inputs grew to {public_inputs_len} bytes"
        );
        public_inputs
            .validate_supported_context()
            .expect("max-hop folded public inputs stay inside size budget");

        let mut oversized_public_inputs = public_inputs.clone();
        oversized_public_inputs.chain_id = "kagemusha-size-chain-"
            .repeat(KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES)
            .into();
        assert!(matches!(
            oversized_public_inputs.validate_supported_context(),
            Err(KagemushaFoldError::EncodedSizeExceeded { actual, .. })
                if actual > KAGEMUSHA_FOLDED_PUBLIC_INPUTS_MAX_ENCODED_BYTES
        ));

        let token = KagemushaCompactPaymentToken {
            public_inputs: public_inputs.clone(),
            folded_proof: KagemushaFoldedProof {
                verifier_key_id: VerifyingKeyId::new("halo2/ipa", "kagemusha-folded-v1"),
                public_inputs_hash: public_inputs
                    .public_inputs_hash()
                    .expect("folded public inputs hash"),
                proof: ProofBox::new("halo2/ipa".into(), vec![0xA5; 256]),
            },
        };
        let token_len = token
            .norito_encoded_len()
            .expect("compact token encoded length");
        assert!(
            token_len > public_inputs_len,
            "compact token length should include proof payload"
        );
        token
            .validate_public_input_binding()
            .expect("size-regression token binding");
    }

    #[test]
    fn kagemusha_folded_public_inputs_reject_malformed_witnesses() {
        let chain_id: ChainId = "kagemusha-fold-chain".parse().expect("chain id");
        let asset = kagemusha_asset("kgm");
        let root0 = fixed_hash(b"kagemusha-root-0");
        let root1 = fixed_hash(b"kagemusha-root-1");
        let root2 = fixed_hash(b"kagemusha-root-2");
        let step0 = kagemusha_step(root0, root1, 0x20, 0x40, b"proof-hop-0");
        let step1 = kagemusha_step(root1, root2, 0x60, 0x80, b"proof-hop-1");

        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &[]),
            Err(KagemushaFoldError::Empty)
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &[]),
            Err(KagemushaFoldError::Empty)
        ));

        let too_many = vec![step0.clone(); KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &too_many),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(&chain_id, &asset, &too_many),
            Err(KagemushaFoldError::TooManyHops { actual, .. })
                if actual == KAGEMUSHA_COMPACT_TOKEN_MAX_HOPS + 1
        ));

        let mut trusted_setup_backend = step0.clone();
        trusted_setup_backend.verifier_key_id =
            VerifyingKeyId::new("halo2/kzg", "kagemusha-hop-fixture");
        let trusted_setup_backend_steps = [trusted_setup_backend];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &trusted_setup_backend_steps),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &trusted_setup_backend_steps
            ),
            Err(KagemushaFoldError::UnsupportedProofBackend { backend })
                if backend == "halo2/kzg"
        ));

        let mut zero_input = step0.clone();
        zero_input.input_nullifiers[0] = [0u8; Hash::LENGTH];
        let zero_input_steps = [zero_input];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_input_steps),
            Err(KagemushaFoldError::ZeroInputNullifier { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_input_steps
            ),
            Err(KagemushaFoldError::ZeroInputNullifier { hop_index: 0 })
        ));

        let mut zero_output = step0.clone();
        zero_output.output_commitments[0] = [0u8; Hash::LENGTH];
        let zero_output_steps = [zero_output];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_output_steps),
            Err(KagemushaFoldError::ZeroOutputCommitment { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_output_steps
            ),
            Err(KagemushaFoldError::ZeroOutputCommitment { hop_index: 0 })
        ));

        let mut zero_proof_inputs = step0.clone();
        zero_proof_inputs.proof_public_inputs_digest = [0u8; Hash::LENGTH];
        let zero_proof_inputs_steps = [zero_proof_inputs];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_proof_inputs_steps),
            Err(KagemushaFoldError::ZeroProofPublicInputsDigest { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_proof_inputs_steps
            ),
            Err(KagemushaFoldError::ZeroProofPublicInputsDigest { hop_index: 0 })
        ));

        let mut zero_vk_commitment = step0.clone();
        zero_vk_commitment.verifier_key_commitment = [0u8; Hash::LENGTH];
        let zero_vk_commitment_steps = [zero_vk_commitment];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_vk_commitment_steps),
            Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_vk_commitment_steps
            ),
            Err(KagemushaFoldError::ZeroVerifierKeyCommitment { hop_index: 0 })
        ));

        let mut zero_vk_poseidon = step0.clone();
        zero_vk_poseidon.verifier_key_poseidon_digest = [0u8; Hash::LENGTH];
        let zero_vk_poseidon_steps = [zero_vk_poseidon];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_vk_poseidon_steps),
            Err(KagemushaFoldError::ZeroVerifierKeyPoseidonDigest { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_vk_poseidon_steps
            ),
            Err(KagemushaFoldError::ZeroVerifierKeyPoseidonDigest { hop_index: 0 })
        ));

        let mut zero_initial_root = step0.clone();
        zero_initial_root.root_before = [0u8; Hash::LENGTH];
        let zero_initial_root_steps = [zero_initial_root];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_initial_root_steps),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "initial_root"
            })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_initial_root_steps
            ),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "initial_root"
            })
        ));

        let mut zero_root_after = step0.clone();
        zero_root_after.root_after = [0u8; Hash::LENGTH];
        let zero_root_after_steps = [zero_root_after];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &zero_root_after_steps),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "root_after"
            })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &zero_root_after_steps
            ),
            Err(KagemushaFoldError::ZeroFoldedRoot {
                field: "root_after"
            })
        ));

        let mut unchanged_root = step0.clone();
        unchanged_root.root_after = unchanged_root.root_before;
        let unchanged_root_steps = [unchanged_root];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &unchanged_root_steps),
            Err(KagemushaFoldError::UnchangedFoldedRootTransition { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &unchanged_root_steps
            ),
            Err(KagemushaFoldError::UnchangedFoldedRootTransition { hop_index: 0 })
        ));

        let mut empty_input = step0.clone();
        empty_input.input_nullifiers.clear();
        let empty_input_steps = [empty_input];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &empty_input_steps),
            Err(KagemushaFoldError::InvalidStepShape {
                hop_index: 0,
                input_count: 0,
                ..
            })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &empty_input_steps
            ),
            Err(KagemushaFoldError::InvalidStepShape {
                hop_index: 0,
                input_count: 0,
                ..
            })
        ));

        let mut oversized_output = step0.clone();
        oversized_output
            .output_commitments
            .push([0xAB; Hash::LENGTH]);
        let oversized_output_steps = [oversized_output];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &oversized_output_steps),
            Err(KagemushaFoldError::InvalidStepShape {
                hop_index: 0,
                output_count: 3,
                ..
            })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &oversized_output_steps
            ),
            Err(KagemushaFoldError::InvalidStepShape {
                hop_index: 0,
                output_count: 3,
                ..
            })
        ));

        let mut duplicate_input = step1.clone();
        duplicate_input.input_nullifiers[0] = step0.input_nullifiers[0];
        let duplicate_input_steps = [step0.clone(), duplicate_input];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &duplicate_input_steps),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 1 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &duplicate_input_steps
            ),
            Err(KagemushaFoldError::DuplicateInputNullifier { hop_index: 1 })
        ));

        let mut duplicate_output = step1.clone();
        duplicate_output.output_commitments[0] = duplicate_output.output_commitments[1];
        let duplicate_output_steps = [step0.clone(), duplicate_output];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &duplicate_output_steps),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &duplicate_output_steps
            ),
            Err(KagemushaFoldError::DuplicateOutputCommitment { hop_index: 1 })
        ));

        let mut same_hop_overlap = step0.clone();
        same_hop_overlap.output_commitments[0] = same_hop_overlap.input_nullifiers[0];
        let same_hop_overlap_steps = [same_hop_overlap];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &same_hop_overlap_steps),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 0 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &same_hop_overlap_steps
            ),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 0 })
        ));

        let mut cross_hop_overlap = step1.clone();
        cross_hop_overlap.input_nullifiers[0] = step0.output_commitments[0];
        let cross_hop_overlap_steps = [step0.clone(), cross_hop_overlap];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &cross_hop_overlap_steps),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 1 })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &cross_hop_overlap_steps
            ),
            Err(KagemushaFoldError::InputOutputOverlap { hop_index: 1 })
        ));

        let mut discontinuous = step1;
        discontinuous.root_before = fixed_hash(b"kagemusha-root-forged");
        let discontinuous_steps = [step0, discontinuous];
        assert!(matches!(
            kagemusha_folded_public_inputs(&chain_id, &asset, &discontinuous_steps),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));
        assert!(matches!(
            kagemusha_poseidon_aggregation_transcript_statement(
                &chain_id,
                &asset,
                &discontinuous_steps
            ),
            Err(KagemushaFoldError::RootDiscontinuity { hop_index: 1, .. })
        ));
    }
}
