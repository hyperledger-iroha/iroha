#![allow(clippy::many_single_char_names)]
//! Minimal ZK verification utilities and batch de-duplication scaffolding.
//!
//! This module provides:
//! - Stable proof/verifying-key hash helpers (`hash_proof`, `hash_vk`).
//! - Batch-local de-duplication cache (`DedupCache`) and a light pre-verifier.
//! - Closed production dispatch for transparent Halo2 IPA/STARK proof
//!   families, with tiny-circuit scaffolding confined to unit tests.
//! - A unified ZK envelope (`ZK1 | TLV*`) reader/writer helpers for tests and
//!   clients.
//!
//! Storage/WSV integration is intentionally limited to proof records and
//! verifying-key registry ISIs; consensus-critical state and policies live in
//! `smartcontracts::isi` and related modules.
//
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use std::collections::{BTreeMap, btree_map::Entry};
#[cfg(any(
    feature = "zk-halo2",
    feature = "zk-halo2-ipa",
    feature = "zk-preverify"
))]
use std::sync::Arc;
#[cfg(any(
    feature = "zk-halo2",
    feature = "zk-halo2-ipa",
    feature = "zk-preverify"
))]
use std::sync::Mutex;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use std::sync::MutexGuard;
#[cfg(any(
    feature = "zk-preverify",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa"
))]
use std::sync::OnceLock;
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};
/// Confidential transfer v2 helpers, circuits, and proof builders.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub mod confidential_v2;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
mod halo2_backend;
/// Constant-depth Pasta IPA accumulation and terminal decisions for Kagemusha.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) mod kagemusha_accumulation;
#[cfg(feature = "zk-halo2-ipa")]
pub mod kagemusha_artifact_source_v4;
/// Authenticated KRV4 framing and role-safe ABI-21 artifact carriers.
#[cfg(feature = "zk-halo2-ipa")]
pub mod kagemusha_artifact_v4;
/// Fixed opposite-field Pasta instructions used by both Kagemusha step parities.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) mod kagemusha_cycle_loader;
/// Dense normalized-GLV MSM used by the reciprocal Kagemusha point audit.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) mod kagemusha_dense_msm;
/// Offline-verifiable consensus finality for Kagemusha top-up anchors.
#[cfg(feature = "zk-halo2-ipa")]
pub mod kagemusha_finality;
/// Fixed-shape ABI-21/V4 Eq/Ep recursive verifier and terminal IPA decisions.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) mod kagemusha_recursion_adapter;
/// Phase-zero serialized advice binding for the review-blocked V7 audit join.
#[cfg(all(feature = "zk-halo2-ipa", feature = "kagemusha-generation-memory-lab"))]
pub(crate) mod kagemusha_serialized_audit_v7;
pub(crate) mod kagemusha_sha256_table16_v4;
/// Exact row-bounded SHA-256 used by the composite Kagemusha Step circuit.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) mod kagemusha_sha256_v4;
#[cfg(feature = "zk-halo2-ipa")]
/// Exact field-neutral operation ABI and assigned two-parent Step transition.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) mod kagemusha_step_transition;
/// ABI-21/V4 Kagemusha facade plus unchanged V2 amount, note, and membership primitives.
#[cfg(feature = "zk-halo2-ipa")]
pub mod kagemusha_v2;
/// Clean first-release offline-cash paired-proof terminal boundary.
#[cfg(feature = "zk-halo2-ipa")]
#[allow(
    dead_code,
    reason = "staged offline-cash boundary remains disconnected until exact STATE circuits and activation wiring land"
)]
pub mod offline_cash_v1;
/// Private non-authorizing Offline Cash V2 source contracts.
#[cfg(feature = "zk-halo2-ipa")]
#[allow(
    dead_code,
    reason = "staged V2 contracts remain disconnected from wire, artifact, verifier, backend, readiness, and release authority"
)]
mod offline_cash_v2;
/// Shared fixed-profile accounting for Pasta IPA recursive proofs.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) mod pasta_ipa_recursion;
/// Core-owned authenticated confidential-spool adapter for MKHE RNS-native sources.
pub mod rns_native_source_v1;
/// Canonical verifier-record namespace for Kagemusha offline proofs.
pub const KAGEMUSHA_VERIFIER_NAMESPACE: &str =
    iroha_data_model::offline::KAGEMUSHA_VERIFIER_NAMESPACE;
/// Canonical Halo2 IPA parameter degree for recursive-spend lineage proofs.
pub const KAGEMUSHA_RECURSIVE_SPEND_LINEAGE_IPA_K: u32 = 12;
#[cfg(feature = "zk-preverify")]
use crate::kura::PipelineProofSnapshot;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub(crate) use halo2_backend::{
    PastaParams, assign_advice_compat, params_fingerprint, params_new as pasta_params_new,
    read_proving_key, read_verifying_key,
};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use halo2_proofs::poly::commitment::Params as _;
#[cfg(all(
    test,
    feature = "zk-halo2-ipa-poseidon",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
use halo2_proofs::poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA};
use iroha_data_model::proof::{ProofBox, VerifyingKeyBox, VerifyingKeyId, VerifyingKeyRecord};
#[cfg(feature = "zk-preverify")]
use ivm::halo2::VMExecutionCircuit;
#[cfg(feature = "zk-halo2")]
use kaigi_zk::{
    KAIGI_ROSTER_BACKEND, KAIGI_USAGE_BACKEND, KaigiRosterJoinCircuit, KaigiUsageCommitmentCircuit,
};
#[cfg(feature = "zk-halo2-ipa")]
use norito::codec::{Decode, Encode};
use sha2::{Digest, Sha256};
#[cfg(feature = "zk-preverify")]
use tokio::sync::mpsc;
#[cfg(feature = "zk-halo2-ipa")]
const HALO2_IPA_PROVING_KEY_ARCHIVE_VERSION: u16 = 1;
#[cfg(feature = "zk-halo2-ipa")]
/// Maximum canonical bytes accepted for one Halo2 IPA proving-key archive.
pub const HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES: usize = 64 * 1024 * 1024;
#[cfg(feature = "zk-halo2-ipa")]
const HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_CIRCUIT_FAMILY_BYTES: usize =
    iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES;
#[cfg(feature = "zk-halo2-ipa")]
const HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_NESTING_DEPTH: usize = 16;
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct Halo2IpaProvingKeyArchive {
    version: u16,
    circuit_family: String,
    vk_commitment: [u8; 32],
    proving_key: Vec<u8>,
}
#[cfg(feature = "zk-halo2-ipa")]
/// Encode Halo2 IPA proving-key bytes with circuit-family and verifier-key binding.
///
/// The archive is the portable key-artifact format consumed by the IVM prover.
/// It rejects empty or oversized fields and emits one exact canonical, uncompressed Norito frame.
///
/// # Errors
///
/// Returns an error when the circuit family or proving-key payload is empty or
/// exceeds its bound, when the complete archive exceeds
/// [`HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES`], or when Norito encoding fails.
pub fn encode_halo2_ipa_proving_key_archive(
    circuit_family: &str,
    vk_commitment: [u8; 32],
    proving_key: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if circuit_family.is_empty() {
        return Err("proving key archive circuit family must be non-empty".to_owned());
    }
    if proving_key.is_empty() {
        return Err("proving key archive payload must be non-empty".to_owned());
    }
    if circuit_family.len() > HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_CIRCUIT_FAMILY_BYTES {
        return Err(format!(
            "proving key archive circuit family exceeds the {}-byte limit",
            HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_CIRCUIT_FAMILY_BYTES
        ));
    }
    if proving_key.len() > HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES {
        return Err(format!(
            "proving key archive payload exceeds the {}-byte archive limit",
            HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES
        ));
    }
    let archive = norito::encode_canonical(&Halo2IpaProvingKeyArchive {
        version: HALO2_IPA_PROVING_KEY_ARCHIVE_VERSION,
        circuit_family: circuit_family.to_owned(),
        vk_commitment,
        proving_key,
    })
    .map_err(|err| format!("failed to encode proving key archive: {err}"))?;
    if archive.len() > HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES {
        return Err(format!(
            "canonical proving key archive exceeds the {}-byte limit",
            HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES
        ));
    }
    Ok(archive)
}
#[cfg(feature = "zk-halo2-ipa")]
/// Write Halo2 IPA proving-key bytes with circuit-family and verifier-key binding.
///
/// This writes the same bounded canonical Norito archive as
/// [`encode_halo2_ipa_proving_key_archive`].
///
/// # Errors
///
/// Returns an error when archive validation or canonical encoding fails, or
/// when the writer rejects the encoded bytes.
pub fn write_halo2_ipa_proving_key_archive<W>(
    writer: &mut W,
    circuit_family: &str,
    vk_commitment: [u8; 32],
    proving_key: Vec<u8>,
) -> Result<(), String>
where
    W: std::io::Write,
{
    let archive = encode_halo2_ipa_proving_key_archive(circuit_family, vk_commitment, proving_key)?;
    writer
        .write_all(&archive)
        .map_err(|err| format!("failed to write proving key archive: {err}"))
}
#[cfg(feature = "zk-halo2-ipa")]
fn decode_halo2_ipa_proving_key_archive(
    bytes: &[u8],
    expected_circuit_family: &str,
    expected_vk_commitment: [u8; 32],
) -> Result<Vec<u8>, String> {
    if bytes.len() > HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES {
        return Err(format!(
            "proving key archive exceeds the {}-byte limit",
            HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES
        ));
    }
    let limits = norito::DecodeLimits::new(
        HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES,
        HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES,
        HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES
            .saturating_add(HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_CIRCUIT_FAMILY_BYTES),
        HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES.saturating_mul(4),
        HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_NESTING_DEPTH,
    );
    let archive: Halo2IpaProvingKeyArchive = norito::decode_canonical_with_limits(bytes, limits)
        .map_err(|err| format!("failed to decode proving key archive: {err}"))?;
    if archive.version != HALO2_IPA_PROVING_KEY_ARCHIVE_VERSION {
        return Err(format!(
            "unsupported proving key archive version {}",
            archive.version
        ));
    }
    if archive.circuit_family.len() > HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_CIRCUIT_FAMILY_BYTES {
        return Err(format!(
            "proving key archive circuit family exceeds the {}-byte limit",
            HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_CIRCUIT_FAMILY_BYTES
        ));
    }
    if archive.circuit_family != expected_circuit_family {
        return Err(format!(
            "proving key archive circuit family `{}` does not match `{expected_circuit_family}`",
            archive.circuit_family
        ));
    }
    if archive.vk_commitment != expected_vk_commitment {
        return Err("proving key archive verifier-key commitment mismatch".to_owned());
    }
    if archive.proving_key.is_empty() {
        return Err("proving key archive payload must be non-empty".to_owned());
    }
    if archive.proving_key.len() > HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES {
        return Err(format!(
            "proving key archive payload exceeds the {}-byte archive limit",
            HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_BYTES
        ));
    }
    Ok(archive.proving_key)
}
/// Hard caps for TLV sections to preserve bounded parsing and determinism.
/// These are generous relative to current tests and examples.
const MAX_PROOF_LEN: usize = 8 * 1024 * 1024; // 8 MiB
/// Maximum accepted bytes for one first-release Halo2 IPA verifying-key container.
///
/// The strict key envelope contains only bounded `CID1`, `IPAK`, and `H2VK`
/// sections. Keeping the whole container under the same 8 MiB ceiling as an
/// individual backend payload ensures state hydration rejects oversized keys
/// before any Halo2 decoder or parameter construction is reached.
pub const HALO2_IPA_VERIFYING_KEY_V1_MAX_BYTES: usize =
    iroha_data_model::proof::VERIFYING_KEY_BOX_MAX_PAYLOAD_BYTES_V1;
/// Maximum canonical encoding accepted for a STARK/FRI V1 verifying key.
///
/// The payload contains one bounded circuit identifier and a fixed set of
/// scalar parameters, so 4 KiB leaves ample format headroom without allowing
/// registry input to inherit a caller-sized decode budget.
pub const STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES: usize = 4 * 1024;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Upper bound for parsed public instance columns. This covers current IVM and
/// confidential-transfer proof layouts while keeping malformed envelopes bounded.
const MAX_INST_COLS: usize = 65;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
const MAX_INST_ROWS: usize = 8192;
/// Canonical backend identifier for Halo2 IPA verification.
pub const ZK_BACKEND_HALO2_IPA: &str = "halo2/ipa";
/// Canonical backend family identifier for native STARK/FRI verification.
pub const ZK_BACKEND_STARK_FRI_V1: &str = iroha_data_model::zk::ZK_BACKEND_STARK_FRI_V1;
/// Exact production-admitted STARK/FRI verifier profiles.
const STARK_FRI_V1_PRODUCTION_PROFILES: &[&str] = &[
    "sha256-goldilocks",
    "poseidon2-goldilocks",
    "sha256_goldilocks.v1",
];
/// Canonical circuit identifier suffix for proved IVM execution commitments.
pub const IVM_EXECUTION_V1_CIRCUIT_ID: &str = "ivm-execution-v1";
/// Canonical semantic role reserved for governance ballot proofs.
pub(crate) const GOVERNANCE_BALLOT_CIRCUIT_ID_V1: &str = "vote-ballot";
/// Canonical semantic role reserved for governance tally proofs.
pub(crate) const GOVERNANCE_TALLY_CIRCUIT_ID_V1: &str = "vote-tally";
/// Exact Halo2/Pasta verifier-registry label for IVM execution commitments.
pub const IVM_EXECUTION_V1_HALO2_BACKEND: &str = "halo2/pasta/ivm-execution-v1";
const IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID: &str = "halo2/pasta/ipa/ivm-execution-v1";
/// Canonical Halo2 IPA circuit identifiers admitted by generic OpenVerify v1.
///
/// The list contains only semantic production circuits. Tiny arithmetic,
/// anonymous-transfer, vote-bool, historical IVM overlay-binding, and retired
/// recursive-spend circuits intentionally have no entry.
const HALO2_IPA_PRODUCTION_CIRCUIT_IDS_V1: &[&str] = &[
    IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
    "halo2/pasta/ipa/kaigi-roster-v1",
    "halo2/pasta/ipa/kaigi-usage-v1",
    "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
    "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
    "halo2/pasta/ipa/confidential-unshield-full-merkle16-axiom-poseidon-v3",
    "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
];
/// Halo2 IPA parameter degree used by the canonical IVM execution binding circuit.
pub const IVM_EXECUTION_V1_IPA_K: u32 = 7;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
const KAIGI_IPA_K_V1: u32 = 8;
#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
const HALO2_IPA_MAX_K_V1: u32 = confidential_v2::CONFIDENTIAL_TRANSFER_V2_IPA_K;
/// Maximum encoded proof payload accepted for IVM execution proofs.
pub const IVM_EXECUTION_V1_MAX_PROOF_BYTES: u32 = 8 * 1024 * 1024;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn halo2_ipa_canonical_k_v1(circuit_id: &str) -> Option<u32> {
    match normalize_halo2_ipa_circuit_id(circuit_id)?.as_str() {
        IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID => Some(IVM_EXECUTION_V1_IPA_K),
        "halo2/pasta/ipa/kaigi-roster-v1" | "halo2/pasta/ipa/kaigi-usage-v1" => {
            Some(KAIGI_IPA_K_V1)
        }
        confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID => {
            Some(confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_IPA_K)
        }
        confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID => {
            Some(confidential_v2::CONFIDENTIAL_TRANSFER_V2_IPA_K)
        }
        confidential_v2::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID => {
            Some(confidential_v2::CONFIDENTIAL_UNSHIELD_V2_IPA_K)
        }
        confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID => {
            Some(confidential_v2::CONFIDENTIAL_UNSHIELD_V3_IPA_K)
        }
        _ => None,
    }
}
#[cfg(feature = "zk-halo2-ipa")]
fn is_ivm_execution_v1_circuit_id(circuit_id: &str) -> bool {
    let trimmed = circuit_id.trim();
    trimmed == IVM_EXECUTION_V1_CIRCUIT_ID
        || trimmed
            .strip_prefix("halo2/ipa:")
            .is_some_and(|suffix| suffix == IVM_EXECUTION_V1_CIRCUIT_ID)
        || trimmed
            .strip_prefix("halo2/ipa::")
            .is_some_and(|suffix| suffix == IVM_EXECUTION_V1_CIRCUIT_ID)
        || trimmed
            .strip_prefix("halo2/ipa/")
            .is_some_and(|suffix| suffix == IVM_EXECUTION_V1_CIRCUIT_ID)
        || trimmed
            .strip_prefix("halo2/pasta/")
            .is_some_and(|suffix| suffix == IVM_EXECUTION_V1_CIRCUIT_ID)
        || trimmed
            .strip_prefix("halo2/pasta/ipa/")
            .is_some_and(|suffix| suffix == IVM_EXECUTION_V1_CIRCUIT_ID)
}
/// Canonical public-input schema descriptor for `halo2/ipa:ivm-execution-v1`.
///
/// The execution proof instances still carry concrete values in the proof payload;
/// this descriptor is only used for stable registry binding via
/// `VerifyingKeyRecord.public_inputs_schema_hash`.
pub const IVM_EXECUTION_PUBLIC_INPUTS_SCHEMA_V1: &[u8] = br#"{"schema":"ivm_execution_current","public_inputs":["code_hash_limb0","code_hash_limb1","code_hash_limb2","code_hash_limb3","overlay_hash_limb0","overlay_hash_limb1","overlay_hash_limb2","overlay_hash_limb3","events_commitment_limb0","events_commitment_limb1","events_commitment_limb2","events_commitment_limb3","gas_policy_commitment_limb0","gas_policy_commitment_limb1","gas_policy_commitment_limb2","gas_policy_commitment_limb3"]}"#;
/// Returns the canonical schema descriptor bytes for `ivm-execution-v1`.
#[must_use]
pub fn ivm_execution_public_inputs_schema_descriptor() -> &'static [u8] {
    IVM_EXECUTION_PUBLIC_INPUTS_SCHEMA_V1
}
/// Returns the canonical schema hash for `ivm-execution-v1`.
#[must_use]
pub fn ivm_execution_public_inputs_schema_hash() -> [u8; 32] {
    iroha_crypto::Hash::new(ivm_execution_public_inputs_schema_descriptor()).into()
}
/// Build the canonical inline verifier key for `ivm-execution-v1`.
///
/// The returned key is a real Halo2 IPA verifier key envelope
/// (`IPAK` + `CID1` + `H2VK`) for the current IVM execution binding circuit,
/// suitable for WSV registration.
///
/// # Errors
///
/// Returns an error if Halo2 verifier-key generation fails.
#[cfg(feature = "zk-halo2-ipa")]
pub fn halo2_ipa_ivm_execution_vk_box() -> Result<VerifyingKeyBox, String> {
    static CACHE: std::sync::OnceLock<Result<VerifyingKeyBox, String>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            build_halo2_ipa_ivm_execution_vk_box()
                .map_err(|err| format!("failed to generate ivm-execution-v1 verifying key: {err}"))
        })
        .clone()
}
#[cfg(feature = "zk-halo2-ipa")]
fn build_halo2_ipa_ivm_execution_vk_box() -> Result<VerifyingKeyBox, halo2_backend::Error> {
    let params = pasta_params_new(IVM_EXECUTION_V1_IPA_K);
    let circuit = pasta_tiny::IvmExecutionBindV1::default();
    let vk = halo2_backend::keygen_vk(&params, &circuit)?;
    let mut bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut bytes, IVM_EXECUTION_V1_IPA_K);
    zk1::wrap_append_circuit_id(&mut bytes, IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID);
    zk1::wrap_append_vk_pasta(&mut bytes, &vk);
    Ok(VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), bytes))
}
/// Build a parseable non-IVM key whose envelope is relabelled as the IVM
/// execution circuit, for registry-boundary regression tests.
#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
pub(crate) fn relabelled_halo2_ipa_demo_vk_box_for_test() -> Result<VerifyingKeyBox, String> {
    let params = pasta_params_new(IVM_EXECUTION_V1_IPA_K);
    let vk = halo2_backend::keygen_vk(&params, &pasta_tiny::Add)
        .map_err(|err| format!("failed to generate relabelled demo key: {err}"))?;
    let mut bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut bytes, IVM_EXECUTION_V1_IPA_K);
    zk1::wrap_append_circuit_id(&mut bytes, IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID);
    zk1::wrap_append_vk_pasta(&mut bytes, &vk);
    Ok(VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), bytes))
}
/// Require the exact first-release IVM execution verifier key and parameter source.
///
/// Halo2/Pasta IPA parameters are transparent and deterministic for the fixed
/// domain exponent. The envelope must bind the canonical circuit identifier,
/// carry `IPAK = 7`, repeat that exponent in the processed `H2VK` header, and
/// match the verifier key generated from the compiled circuit.
///
/// # Errors
///
/// Returns an error for a wrong backend, malformed or mismatched metadata, or
/// any verifier key other than the canonical compiled circuit key.
#[cfg(feature = "zk-halo2-ipa")]
pub fn ensure_halo2_ipa_ivm_execution_canonical_vk_box(
    vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    if vk_box.backend.as_str() != ZK_BACKEND_HALO2_IPA {
        return Err(format!(
            "ivm-execution-v1 verifier key backend `{}` is not `{ZK_BACKEND_HALO2_IPA}`",
            vk_box.backend
        ));
    }
    let ipa_k = zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(
        &vk_box.bytes,
        IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
    )
    .map_err(|err| format!("ivm-execution-v1 verifier key {err}"))?;
    if ipa_k != IVM_EXECUTION_V1_IPA_K {
        return Err(format!(
            "ivm-execution-v1 verifier key IPAK `{ipa_k}` is not `{IVM_EXECUTION_V1_IPA_K}`"
        ));
    }
    let h2vk = zk1::h2vk_payload(&vk_box.bytes)
        .map_err(|err| format!("ivm-execution-v1 verifier key {err}"))?;
    let (h2vk_k, _compress_selectors, _fixed_columns) = zk1::halo2_pasta_vk_header(h2vk)
        .map_err(|err| format!("ivm-execution-v1 verifier key {err}"))?;
    if h2vk_k != ipa_k {
        return Err(format!(
            "ivm-execution-v1 verifier key IPAK `{ipa_k}` does not match H2VK domain `{h2vk_k}`"
        ));
    }
    let canonical = halo2_ipa_ivm_execution_vk_box()?;
    if vk_box.bytes != canonical.bytes {
        return Err(
            "ivm-execution-v1 verifier key must match the canonical compiled circuit key"
                .to_owned(),
        );
    }
    Ok(())
}
/// Build a governance/WSV verifier-key record for `ivm-execution-v1`.
///
/// The record is active, embeds the real Halo2 IPA verifier key inline, and
/// binds to the canonical IVM execution public-input schema hash.
///
/// # Errors
///
/// Returns an error if verifier-key generation fails or the key length cannot be encoded.
#[cfg(feature = "zk-halo2-ipa")]
pub fn halo2_ipa_ivm_execution_vk_record(
    namespace: impl Into<String>,
    version: u32,
) -> Result<iroha_data_model::proof::VerifyingKeyRecord, String> {
    use iroha_data_model::{
        confidential::ConfidentialStatus, proof::VerifyingKeyRecord, zk::BackendTag,
    };
    let vk_box = halo2_ipa_ivm_execution_vk_box()?;
    let mut record = VerifyingKeyRecord::new(
        version,
        IVM_EXECUTION_V1_CIRCUIT_ID,
        BackendTag::Halo2IpaPasta,
        "pallas",
        ivm_execution_public_inputs_schema_hash(),
        hash_vk(&vk_box),
    );
    record.vk_len = u32::try_from(vk_box.bytes.len())
        .map_err(|_| "ivm-execution-v1 verifying key length overflowed u32".to_owned())?;
    record.max_proof_bytes = IVM_EXECUTION_V1_MAX_PROOF_BYTES;
    record.gas_schedule_id = Some("halo2_default".to_owned());
    record.key = Some(vk_box);
    record.status = ConfidentialStatus::Active;
    record.namespace = namespace.into();
    Ok(record)
}
fn hash_domain_separated_payload(domain: &[u8], backend: &str, bytes: &[u8]) -> [u8; 32] {
    let backend_len = u64::try_from(backend.len()).expect("backend length must fit into u64");
    let bytes_len = u64::try_from(bytes.len()).expect("payload length must fit into u64");
    let mut h = Sha256::new();
    h.update(domain);
    h.update(backend_len.to_be_bytes());
    h.update(backend.as_bytes());
    h.update(bytes_len.to_be_bytes());
    h.update(bytes);
    h.finalize().into()
}
/// Compute a stable, domain-separated 32-byte hash of the proof payload.
pub fn hash_proof(proof: &ProofBox) -> [u8; 32] {
    hash_domain_separated_payload(b"iroha:zk:v1:proof", &proof.backend, &proof.bytes)
}
/// Compute a stable, domain-separated 32-byte hash of the verifying key payload.
pub fn hash_vk(vk: &VerifyingKeyBox) -> [u8; 32] {
    hash_vk_bytes(&vk.backend, &vk.bytes)
}
#[cfg(all(test, feature = "zk-halo2-ipa"))]
fn relabel_halo2_ipa_open_verify_fixture(
    proof: &ProofBox,
    vk: &VerifyingKeyBox,
    exact_backend: &str,
) -> (ProofBox, VerifyingKeyBox) {
    assert_eq!(proof.backend.as_str(), ZK_BACKEND_HALO2_IPA);
    assert_eq!(vk.backend.as_str(), ZK_BACKEND_HALO2_IPA);
    assert_eq!(
        production_verify_backend_tag(exact_backend),
        Some(iroha_data_model::zk::BackendTag::Halo2IpaPasta)
    );
    assert_ne!(exact_backend, ZK_BACKEND_HALO2_IPA);

    let exact_vk = VerifyingKeyBox::new(exact_backend.to_owned(), vk.bytes.clone());
    let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
        norito::decode_canonical(&proof.bytes).expect("canonical Halo2 OpenVerifyEnvelope");
    envelope.circuit_id = exact_backend.to_owned();
    envelope.vk_hash = hash_vk(&exact_vk);
    let exact_proof = ProofBox::new(
        exact_backend.to_owned(),
        norito::encode_canonical(&envelope).expect("encode exact Halo2 OpenVerifyEnvelope"),
    );
    (exact_proof, exact_vk)
}
pub(crate) fn hash_vk_bytes(backend: &str, bytes: &[u8]) -> [u8; 32] {
    hash_domain_separated_payload(b"iroha:zk:v1:vk", backend, bytes)
}
/// Returns `true` when `backend` denotes an explicitly admitted native
/// STARK/FRI verifier profile.
#[inline]
pub(crate) fn is_stark_fri_v1_backend(backend: &str) -> bool {
    backend == ZK_BACKEND_STARK_FRI_V1
        || backend
            .strip_prefix("stark/fri/")
            .is_some_and(|profile| STARK_FRI_V1_PRODUCTION_PROFILES.contains(&profile))
}
/// Returns `true` for backend labels that require a trusted setup and are not
/// admitted into the native verifier registry.
#[inline]
#[must_use]
pub fn is_trusted_setup_backend_label(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    let backend = backend.as_str();
    has_trusted_setup_backend_segment(backend)
        || has_trusted_setup_backend_compact_label(backend)
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
fn has_trusted_setup_backend_segment(backend: &str) -> bool {
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
fn has_trusted_setup_backend_compact_label(backend: &str) -> bool {
    let compact = backend
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
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
const DEVELOPER_ONLY_EMBEDDED_BACKEND_TOKENS: &[&str] = &[
    "debug", "mock", "fixture", "dev", "todo", "draft", "pending", "replace",
];
const DEVELOPER_ONLY_EXACT_BACKEND_TOKENS: &[&str] = &[
    "test",
    "dummy",
    "fake",
    "stub",
    "sample",
    "placeholder",
    "todo",
    "draft",
];
const DEVELOPER_ONLY_COMPACT_BACKEND_FRAGMENTS: &[&str] = &[
    "notforproduction",
    "notproduction",
    "notproductionready",
    "notready",
    "replacebeforeproduction",
    "replacebeforemainnet",
    "draftonly",
];
const PRODUCTION_CLAIM_BACKEND_FRAGMENTS: &[&str] = &[
    "productionready",
    "productionhardened",
    "productionenabled",
    "productionapproved",
    "productioncertified",
    "productionclaim",
    "claimedproduction",
    "mainnetready",
    "mainnetcomplete",
    "mainnetclaim",
    "claimedmainnet",
    "mainnetcertified",
    "mainnetapproved",
    "mainnetrelease",
    "auditedproduction",
    "externallyaudited",
    "thirdpartyaudited",
    "boiaudited",
    "auditedmainnet",
    "externalaudit",
    "auditpassed",
    "auditapproved",
    "auditsignoff",
    "auditclaim",
    "claimedaudit",
    "securityreviewpassed",
    "securityauditpassed",
    "securityaudited",
    "externalsecurityreview",
    "certifiedproduction",
    "certifiedmainnet",
    "releaseready",
    "releaseapproved",
    "releasecertified",
];
fn compact_ascii_lowercase_label(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}
#[inline]
fn is_developer_only_direct_backend_token(token: &str) -> bool {
    DEVELOPER_ONLY_EMBEDDED_BACKEND_TOKENS
        .iter()
        .any(|reserved| token.contains(reserved))
        || DEVELOPER_ONLY_EXACT_BACKEND_TOKENS.contains(&token)
}
#[inline]
fn is_developer_only_compact_backend_run(run: &str) -> bool {
    DEVELOPER_ONLY_EMBEDDED_BACKEND_TOKENS
        .iter()
        .any(|reserved| run.contains(reserved))
        || DEVELOPER_ONLY_EXACT_BACKEND_TOKENS.contains(&run)
}
/// Returns `true` for developer-only backend labels that must not enter
/// proof admission, preverification, or native verifier dispatch.
#[inline]
#[must_use]
pub fn is_developer_only_backend_label(backend: &str) -> bool {
    let backend = backend.to_ascii_lowercase();
    let compact = compact_ascii_lowercase_label(&backend);
    if DEVELOPER_ONLY_COMPACT_BACKEND_FRAGMENTS
        .iter()
        .any(|fragment| compact.contains(fragment))
    {
        return true;
    }
    let mut letter_run = String::new();
    for token in backend
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        if is_developer_only_direct_backend_token(token) {
            return true;
        }
        if token.len() == 1 {
            letter_run.push_str(token);
        } else {
            if is_developer_only_compact_backend_run(&letter_run) {
                return true;
            }
            letter_run.clear();
        }
    }
    is_developer_only_compact_backend_run(&letter_run)
}
/// Returns `true` for verifier backend labels that claim production, mainnet,
/// or audit approval instead of matching an explicitly admitted verifier id.
#[inline]
#[must_use]
pub fn is_production_claim_backend_label(backend: &str) -> bool {
    let compact = compact_ascii_lowercase_label(backend);
    PRODUCTION_CLAIM_BACKEND_FRAGMENTS
        .iter()
        .any(|fragment| compact.contains(fragment))
}
/// Compatibility spelling for callers that still classify textual readiness
/// claims separately from the production verifier allowlist.
#[inline]
#[must_use]
pub fn is_verifier_readiness_claim_label(backend: &str) -> bool {
    is_production_claim_backend_label(backend)
}
/// Returns `true` when `backend` is accepted for `ivm-execution-v1` proofs.
#[inline]
#[must_use]
pub fn is_ivm_execution_backend(backend: &str) -> bool {
    matches!(
        backend,
        ZK_BACKEND_HALO2_IPA | IVM_EXECUTION_V1_HALO2_BACKEND
    ) || is_stark_fri_v1_backend(backend)
}
/// Return the expected OpenVerify backend tag for labels admitted by native
/// verifier dispatch.
#[must_use]
pub fn verifier_backend_registry_tag_v1(backend: &str) -> Option<iroha_data_model::zk::BackendTag> {
    iroha_data_model::zk::verifier_backend_registry_tag_v1(backend)
}
/// Returns `true` when `backend` names a verifier family that can reach native
/// verifier dispatch.
#[inline]
#[must_use]
pub fn is_verifier_backend_registry_label_v1(backend: &str) -> bool {
    verifier_backend_registry_tag_v1(backend).is_some()
}
fn production_verify_backend_label_is_portable(backend: &str) -> bool {
    if backend.is_empty() || backend.trim() != backend {
        return false;
    }
    let is_lower_ascii_alphanumeric =
        |byte: u8| byte.is_ascii_alphanumeric() && matches!(byte, b'a'..=b'z' | b'0'..=b'9');
    let bytes = backend.as_bytes();
    if !is_lower_ascii_alphanumeric(bytes[0])
        || !is_lower_ascii_alphanumeric(bytes[bytes.len() - 1])
        || !bytes.iter().copied().all(|byte| {
            is_lower_ascii_alphanumeric(byte) || matches!(byte, b'/' | b':' | b'.' | b'_' | b'-')
        })
    {
        return false;
    }
    !["//", "::", "..", "/:", ":/", "/.", "./", ":.", ".:"]
        .iter()
        .any(|separator| backend.contains(separator))
}
/// Return the low-level proof engine for an exact production verifier label.
///
/// Textual readiness claims, trusted-setup families, developer-only labels,
/// non-portable spellings, and labels outside the closed registry all fail
/// closed.
#[must_use]
pub fn production_verify_backend_tag(backend: &str) -> Option<iroha_data_model::zk::BackendTag> {
    if !production_verify_backend_label_is_portable(backend)
        || is_production_claim_backend_label(backend)
        || is_trusted_setup_backend_label(backend)
        || is_developer_only_backend_label(backend)
    {
        return None;
    }
    match verifier_backend_registry_tag_v1(backend) {
        Some(iroha_data_model::zk::BackendTag::Stark) => {
            Some(iroha_data_model::zk::BackendTag::Stark)
        }
        Some(iroha_data_model::zk::BackendTag::Halo2IpaPasta) => {
            Some(iroha_data_model::zk::BackendTag::Halo2IpaPasta)
        }
        None => None,
    }
}
/// Returns `true` only for an exact production verifier label.
#[inline]
#[must_use]
pub fn is_production_verify_backend_label(backend: &str) -> bool {
    production_verify_backend_tag(backend).is_some()
}
pub(crate) fn halo2_open_verify_circuit_id_matches_backend(
    backend: &str,
    circuit_id: &str,
) -> bool {
    if circuit_id.len() > iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES
        || !iroha_data_model::zk::open_verify_circuit_id_is_portable(circuit_id)
        || iroha_data_model::zk::open_verify_circuit_id_uses_reserved_privacy_protocol_label_v1(
            circuit_id,
        )
        || production_verify_backend_tag(backend)
            != Some(iroha_data_model::zk::BackendTag::Halo2IpaPasta)
    {
        return false;
    }
    if backend == ZK_BACKEND_HALO2_IPA {
        return halo2_open_verify_circuit_id_is_production_v1(circuit_id);
    }
    if !halo2_open_verify_circuit_id_is_production_v1(circuit_id) {
        return false;
    }
    normalize_halo2_ipa_circuit_id(backend) == normalize_halo2_ipa_circuit_id(circuit_id)
}
fn halo2_open_verify_circuit_id_is_production_v1(circuit_id: &str) -> bool {
    normalize_halo2_ipa_circuit_id(circuit_id).is_some_and(|normalized| {
        HALO2_IPA_PRODUCTION_CIRCUIT_IDS_V1.contains(&normalized.as_str())
    })
}
/// Return the one canonical outer-envelope schema for a production Halo2 circuit.
///
/// Halo2 authenticates the instance columns inside the backend proof, but it
/// does not see the surrounding [`iroha_data_model::zk::OpenVerifyEnvelope`].
/// Keeping this mapping closed prevents valid proofs from being relabelled with
/// arbitrary non-empty schema bytes.
fn halo2_ipa_public_inputs_schema_v1(circuit_id: &str) -> Option<&'static [u8]> {
    let canonical = normalize_halo2_ipa_circuit_id(circuit_id)?;
    match canonical.as_str() {
        IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID => Some(IVM_EXECUTION_PUBLIC_INPUTS_SCHEMA_V1),
        #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
        confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_CIRCUIT_ID => {
            Some(confidential_v2::KAGEMUSHA_TOPUP_SHIELD_V2_PUBLIC_INPUTS_SCHEMA_V2)
        }
        #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
        confidential_v2::CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID => {
            Some(confidential_v2::CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA_V1)
        }
        #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
        confidential_v2::CONFIDENTIAL_UNSHIELD_V2_CIRCUIT_ID => {
            Some(confidential_v2::CONFIDENTIAL_UNSHIELD_V2_PUBLIC_INPUTS_SCHEMA_V1)
        }
        #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
        confidential_v2::CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID => {
            Some(confidential_v2::CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA_V1)
        }
        #[cfg(feature = "zk-halo2")]
        "halo2/pasta/ipa/kaigi-roster-v1" => Some(kaigi_zk::KAIGI_ROSTER_PUBLIC_INPUTS_SCHEMA_V1),
        #[cfg(feature = "zk-halo2")]
        "halo2/pasta/ipa/kaigi-usage-v1" => Some(kaigi_zk::KAIGI_USAGE_PUBLIC_INPUTS_SCHEMA_V1),
        _ => None,
    }
}

fn halo2_ipa_public_inputs_schema_hash_v1(circuit_id: &str) -> Option<[u8; 32]> {
    halo2_ipa_public_inputs_schema_v1(circuit_id)
        .map(|schema| iroha_crypto::Hash::new(schema).into())
}
/// Backend material prepared by the strict first-release verifying-key validator.
///
/// This contains only bounded, already-validated parameters. Callers retain the
/// canonical key bytes separately for the native verifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PreparedVerifyingKeyMaterialV1 {
    /// Transparent Halo2 IPA material over Pasta.
    Halo2IpaPasta {
        /// Fixed circuit-domain exponent authenticated by both `IPAK` and `H2VK`.
        ipa_k: u32,
    },
    /// Native STARK/FRI material pinned by the canonical registry payload.
    #[cfg_attr(not(feature = "zk-stark"), allow(dead_code))]
    StarkFri {
        /// Canonical circuit identifier embedded in the key payload.
        circuit_id: String,
        /// Evaluation-domain exponent.
        n_log2: u8,
        /// FRI blow-up exponent.
        blowup_log2: u8,
        /// FRI folding arity.
        fold_arity: u8,
        /// Number of verifier queries.
        queries: u16,
        /// Merkle-tree arity.
        merkle_arity: u8,
        /// Hash-function selector.
        hash_fn: u8,
    },
}
impl PreparedVerifyingKeyMaterialV1 {
    /// Return the authenticated Halo2 IPA domain exponent, when applicable.
    #[must_use]
    pub(crate) const fn ipa_k(&self) -> Option<u32> {
        match self {
            Self::Halo2IpaPasta { ipa_k } => Some(*ipa_k),
            Self::StarkFri { .. } => None,
        }
    }
}
/// Validate and prepare exact inline verifier material under backend-specific
/// resource limits.
///
/// This is the single material gate shared by registry mutation, state
/// hydration, and native proof dispatch. It rejects a backend/circuit mismatch,
/// oversized or malformed containers, non-canonical STARK encodings, weak
/// STARK parameters, and Halo2 keys that differ from the deterministically
/// compiled circuit key.
pub(crate) fn validate_and_prepare_verifying_key_material_v1(
    backend: &str,
    circuit_id: &str,
    backend_tag: iroha_data_model::zk::BackendTag,
    vk: &VerifyingKeyBox,
) -> Result<PreparedVerifyingKeyMaterialV1, String> {
    if vk.backend.as_str() != backend {
        return Err("verifying-key payload backend does not match registry backend".to_owned());
    }
    if production_verify_backend_tag(backend) != Some(backend_tag) {
        return Err("verifying-key backend is not an exact production backend".to_owned());
    }
    match backend_tag {
        iroha_data_model::zk::BackendTag::Halo2IpaPasta => {
            #[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
            {
                let _ = (circuit_id, vk);
                Err("verifying-key backend Halo2 IPA is not enabled".to_owned())
            }
            #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
            {
                if vk.bytes.len() > HALO2_IPA_VERIFYING_KEY_V1_MAX_BYTES {
                    return Err(format!(
                        "Halo2 IPA verifying-key container exceeds the {}-byte limit",
                        HALO2_IPA_VERIFYING_KEY_V1_MAX_BYTES
                    ));
                }
                validate_builtin_halo2_ipa_verifying_key_v1(backend, circuit_id, vk)?;
                let canonical_circuit_id = normalize_halo2_ipa_circuit_id(circuit_id)
                    .ok_or_else(|| "invalid Halo2 IPA circuit id".to_owned())?;
                let ipa_k = zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(
                    vk.bytes.as_slice(),
                    &canonical_circuit_id,
                )?;
                Ok(PreparedVerifyingKeyMaterialV1::Halo2IpaPasta { ipa_k })
            }
        }
        iroha_data_model::zk::BackendTag::Stark => {
            #[cfg(not(feature = "zk-stark"))]
            {
                let _ = (circuit_id, vk);
                Err("verifying-key backend Stark is not enabled".to_owned())
            }
            #[cfg(feature = "zk-stark")]
            {
                // The decoder performs the whole-container check and bounded
                // canonical Norito decode before materializing the typed key.
                let payload =
                    validate_stark_fri_verifying_key_v1(backend, circuit_id, vk.bytes.as_slice())?;
                Ok(PreparedVerifyingKeyMaterialV1::StarkFri {
                    circuit_id: payload.circuit_id,
                    n_log2: payload.n_log2,
                    blowup_log2: payload.blowup_log2,
                    fold_arity: payload.fold_arity,
                    queries: payload.queries,
                    merkle_arity: payload.merkle_arity,
                    hash_fn: payload.hash_fn,
                })
            }
        }
    }
}
/// Validate one verifier registry record and prepare any inline key material.
///
/// Commitment, declared length, registry backend, curve, circuit, and inline
/// key bytes are checked together so mutation and state rehydration cannot
/// disagree about the record that proof dispatch later consumes. Records that
/// publish only an off-ledger commitment have no prepared inline material and
/// remain unusable by native proof dispatch until a validated inline key is
/// installed.
pub(crate) fn validate_and_prepare_verifying_key_record_v1(
    id: &VerifyingKeyId,
    record: &VerifyingKeyRecord,
) -> Result<Option<PreparedVerifyingKeyMaterialV1>, String> {
    if !id.is_portable_registry_id() {
        return Err("verifying-key registry id is not bounded and portable".to_owned());
    }
    if record.commitment == [0_u8; 32] {
        return Err("verifying-key commitment must be non-zero".to_owned());
    }
    if record.public_inputs_schema_hash == [0_u8; 32] {
        return Err("verifying-key public-input schema hash must be non-zero".to_owned());
    }
    let backend = id.backend.as_str();
    if production_verify_backend_tag(backend) != Some(record.backend) {
        return Err(
            "verifying-key record backend does not match the production registry backend"
                .to_owned(),
        );
    }
    match record.backend {
        iroha_data_model::zk::BackendTag::Halo2IpaPasta => {
            if record.curve != "pallas" {
                return Err("Halo2 IPA verifying-key curve must be pallas".to_owned());
            }
            if !halo2_open_verify_circuit_id_matches_backend(backend, &record.circuit_id) {
                return Err(
                    "Halo2 IPA verifying-key circuit is not admitted for the registry backend"
                        .to_owned(),
                );
            }
            let expected_schema_hash = halo2_ipa_public_inputs_schema_hash_v1(&record.circuit_id)
                .ok_or_else(|| {
                "Halo2 IPA circuit has no canonical public-input schema".to_owned()
            })?;
            if record.public_inputs_schema_hash != expected_schema_hash {
                return Err(
                    "Halo2 IPA verifying-key public-input schema hash is not canonical".to_owned(),
                );
            }
        }
        iroha_data_model::zk::BackendTag::Stark => {
            if record.curve != "goldilocks" {
                return Err("STARK/FRI verifying-key curve must be goldilocks".to_owned());
            }
            if !stark_open_verify_circuit_id_matches_backend(backend, &record.circuit_id) {
                return Err(
                    "STARK/FRI verifying-key circuit is not admitted for the registry backend"
                        .to_owned(),
                );
            }
        }
    }
    let max_payload_bytes = match record.backend {
        iroha_data_model::zk::BackendTag::Halo2IpaPasta => HALO2_IPA_VERIFYING_KEY_V1_MAX_BYTES,
        iroha_data_model::zk::BackendTag::Stark => STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES,
    };
    if u64::from(record.vk_len) > max_payload_bytes as u64 {
        return Err(format!(
            "declared verifying-key length exceeds the {max_payload_bytes}-byte backend limit"
        ));
    }
    validate_verifying_key_record_metadata_v1(record)?;
    let Some(vk) = record.key.as_ref() else {
        return Ok(None);
    };
    if vk.bytes.len() > max_payload_bytes {
        return Err(format!(
            "inline verifying-key container exceeds the {max_payload_bytes}-byte backend limit"
        ));
    }
    if vk.backend != id.backend {
        return Err("verifying-key payload backend does not match registry id".to_owned());
    }
    let vk_len = u32::try_from(vk.bytes.len())
        .map_err(|_| "inline verifying-key length exceeds u32".to_owned())?;
    if record.vk_len != vk_len {
        return Err("verifying-key vk_len does not match inline bytes".to_owned());
    }
    if hash_vk(vk) != record.commitment {
        return Err("verifying-key commitment does not match inline bytes".to_owned());
    }
    validate_and_prepare_verifying_key_material_v1(backend, &record.circuit_id, record.backend, vk)
        .map(Some)
}

fn validate_verifying_key_record_metadata_v1(record: &VerifyingKeyRecord) -> Result<(), String> {
    if !iroha_data_model::proof::verifying_key_id_field_is_portable(&record.namespace) {
        return Err("verifying-key namespace is not bounded and portable".to_owned());
    }
    if record
        .owner_manifest_id
        .as_ref()
        .is_some_and(|owner| !iroha_data_model::proof::verifying_key_id_field_is_portable(owner))
    {
        return Err("verifying-key owner manifest id is not bounded and portable".to_owned());
    }
    let Some(gas_schedule_id) = record.gas_schedule_id.as_deref() else {
        return Err("verifying-key gas schedule id is required".to_owned());
    };
    if !iroha_data_model::proof::verifying_key_id_field_is_portable(gas_schedule_id) {
        return Err("verifying-key gas schedule id is not bounded and portable".to_owned());
    }
    if record
        .metadata_uri_cid
        .as_deref()
        .is_some_and(|uri| !verifying_key_content_uri_is_portable_v1(uri))
    {
        return Err("verifying-key metadata URI is not bounded and portable".to_owned());
    }
    if record
        .vk_bytes_cid
        .as_deref()
        .is_some_and(|uri| !verifying_key_content_uri_is_portable_v1(uri))
    {
        return Err("verifying-key bytes URI is not bounded and portable".to_owned());
    }
    if matches!(
        (record.activation_height, record.withdraw_height),
        (Some(activation), Some(withdraw)) if withdraw <= activation
    ) {
        return Err(
            "verifying-key withdraw height must be greater than activation height".to_owned(),
        );
    }
    Ok(())
}

fn verifying_key_content_uri_is_portable_v1(uri: &str) -> bool {
    const MAX_URI_BYTES: usize = 512;
    if uri.is_empty()
        || uri.len() > MAX_URI_BYTES
        || uri.trim() != uri
        || uri
            .as_bytes()
            .iter()
            .any(|byte| !byte.is_ascii_graphic() || matches!(*byte, b'\\' | b'?' | b'#' | b'@'))
    {
        return false;
    }
    let body = uri
        .strip_prefix("ipfs://")
        .or_else(|| uri.strip_prefix("cid:"))
        .unwrap_or(uri);
    if body.is_empty()
        || body.starts_with('/')
        || body.ends_with('/')
        || body.contains("..")
        || body.contains("//")
    {
        return false;
    }
    body.split('/').all(|segment| {
        !segment.is_empty()
            && segment
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.'))
    })
}
include!("zk/strict_verifying_key_preparation_tests.rs");
fn hash_to_u64_limbs_le(hash: &iroha_crypto::Hash) -> [u64; 4] {
    let bytes: &[u8; 32] = hash.as_ref();
    bytes_to_u64_limbs_le(bytes)
}
fn bytes_to_u64_limbs_le(bytes: &[u8; 32]) -> [u64; 4] {
    let mut limbs = [0u64; 4];
    for (idx, limb) in limbs.iter_mut().enumerate() {
        let start = idx * 8;
        let end = start + 8;
        *limb = u64::from_le_bytes(bytes[start..end].try_into().expect("8-byte limb"));
    }
    limbs
}
#[cfg(feature = "zk-stark")]
fn limb_as_instance_bytes(limb: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&limb.to_le_bytes());
    out
}
#[cfg(feature = "zk-stark")]
fn ivm_execution_public_inputs_columns(
    code_hash: iroha_crypto::Hash,
    overlay_hash: iroha_crypto::Hash,
    events_commitment: iroha_crypto::Hash,
    gas_policy_commitment: iroha_crypto::Hash,
) -> Vec<Vec<[u8; 32]>> {
    let code_limbs = hash_to_u64_limbs_le(&code_hash);
    let overlay_limbs = hash_to_u64_limbs_le(&overlay_hash);
    let events_limbs = hash_to_u64_limbs_le(&events_commitment);
    let gas_limbs = hash_to_u64_limbs_le(&gas_policy_commitment);
    code_limbs
        .into_iter()
        .chain(overlay_limbs)
        .chain(events_limbs)
        .chain(gas_limbs)
        .map(limb_as_instance_bytes)
        .map(|value| vec![value])
        .collect()
}
#[cfg(feature = "zk-halo2-ipa")]
fn ensure_halo2_ipa_proving_key_compatible(
    proving_key: &halo2_backend::ProvingKey,
    parsed_vk: &halo2_backend::VerifyingKey,
    params: &PastaParams,
    domain_message: &str,
    vk_message: &str,
) -> Result<(), String> {
    if halo2_backend::proving_key_domain_k(proving_key) != params.k() {
        return Err(domain_message.to_owned());
    }
    if halo2_backend::proving_key_vk_to_processed_bytes(proving_key)
        != halo2_backend::verifying_key_to_processed_bytes(parsed_vk)
    {
        return Err(vk_message.to_owned());
    }
    Ok(())
}
#[cfg(feature = "zk-halo2-ipa")]
fn preflight_halo2_ipa_processed_proving_key(
    bytes: &[u8],
    parsed_vk: &halo2_backend::VerifyingKey,
    params: &PastaParams,
) -> Result<(), String> {
    fn read_u32_be(bytes: &[u8], offset: &mut usize, label: &str) -> Result<u32, String> {
        let end = offset
            .checked_add(4)
            .ok_or_else(|| format!("proving key {label} offset overflow"))?;
        let encoded = bytes
            .get(*offset..end)
            .ok_or_else(|| format!("proving key {label} is truncated"))?;
        *offset = end;
        Ok(u32::from_be_bytes(
            encoded.try_into().expect("four-byte proving-key field"),
        ))
    }
    fn skip_polynomial(
        bytes: &[u8],
        offset: &mut usize,
        expected_rows: u32,
        scalar_bytes: usize,
        label: &str,
    ) -> Result<(), String> {
        let encoded_rows = read_u32_be(bytes, offset, label)?;
        if encoded_rows != expected_rows {
            return Err(format!(
                "proving key {label} length `{encoded_rows}` does not match domain `{expected_rows}`"
            ));
        }
        let payload_bytes = usize::try_from(expected_rows)
            .ok()
            .and_then(|rows| rows.checked_mul(scalar_bytes))
            .ok_or_else(|| format!("proving key {label} byte length overflow"))?;
        let end = offset
            .checked_add(payload_bytes)
            .ok_or_else(|| format!("proving key {label} offset overflow"))?;
        if end > bytes.len() {
            return Err(format!("proving key {label} payload is truncated"));
        }
        *offset = end;
        Ok(())
    }
    fn skip_polynomial_vec(
        bytes: &[u8],
        offset: &mut usize,
        expected_count: usize,
        expected_rows: u32,
        scalar_bytes: usize,
        label: &str,
    ) -> Result<(), String> {
        let encoded_count = usize::try_from(read_u32_be(bytes, offset, label)?)
            .map_err(|_| format!("proving key {label} count does not fit usize"))?;
        if encoded_count != expected_count {
            return Err(format!(
                "proving key {label} count `{encoded_count}` does not match circuit `{expected_count}`"
            ));
        }
        for _ in 0..expected_count {
            skip_polynomial(bytes, offset, expected_rows, scalar_bytes, label)?;
        }
        Ok(())
    }
    let canonical_vk = halo2_backend::verifying_key_to_processed_bytes(parsed_vk);
    if !bytes.starts_with(&canonical_vk) {
        return Err("proving key embeds a different verifying key".to_owned());
    }
    let expected_rows = 1_u32
        .checked_shl(params.k())
        .ok_or_else(|| "proving key domain row count overflow".to_owned())?;
    let scalar_bytes = <halo2_backend::Scalar as ff::PrimeField>::Repr::default()
        .as_ref()
        .len();
    let fixed_polynomials = parsed_vk.fixed_commitments().len();
    let permutation_polynomials = parsed_vk.permutation().commitments().len();
    let mut offset = canonical_vk.len();
    for label in [
        "l0 polynomial",
        "l_last polynomial",
        "l_active_row polynomial",
    ] {
        skip_polynomial(bytes, &mut offset, expected_rows, scalar_bytes, label)?;
    }
    for (expected_count, label) in [
        (fixed_polynomials, "fixed-value polynomials"),
        (fixed_polynomials, "fixed coefficient polynomials"),
        (permutation_polynomials, "permutation Lagrange polynomials"),
        (
            permutation_polynomials,
            "permutation coefficient polynomials",
        ),
    ] {
        skip_polynomial_vec(
            bytes,
            &mut offset,
            expected_count,
            expected_rows,
            scalar_bytes,
            label,
        )?;
    }
    if offset != bytes.len() {
        return Err("proving key has trailing bytes".to_owned());
    }
    Ok(())
}
#[cfg(feature = "zk-halo2-ipa")]
fn create_halo2_ipa_proof<C>(
    params: &PastaParams,
    proving_key: &halo2_backend::ProvingKey,
    circuit: C,
    instance_refs: &[&[&[halo2_backend::Scalar]]],
    context: &str,
) -> Result<Vec<u8>, String>
where
    C: halo2_proofs::plonk::Circuit<halo2_backend::Scalar>,
{
    halo2_backend::create_ipa_proof(params, proving_key, &[circuit], instance_refs)
        .map_err(|err| format!("failed to create {context} proof: {err}"))
}
#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
fn verify_halo2_ipa_payload_no_instances(
    params: &PastaParams,
    vk: &halo2_backend::VerifyingKey,
    proof_payload: &[u8],
) -> bool {
    halo2_backend::verify_ipa_proof_no_instances(params, vk, proof_payload).is_ok()
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn verify_halo2_ipa_payload_columns_result(
    params: &PastaParams,
    vk: &halo2_backend::VerifyingKey,
    proof_payload: &[u8],
    col_refs: &[&[halo2_backend::Scalar]],
) -> Result<(), halo2_backend::Error> {
    halo2_backend::verify_ipa_proof_with_columns(params, vk, proof_payload, col_refs)
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn verify_halo2_ipa_payload_columns(
    params: &PastaParams,
    vk: &halo2_backend::VerifyingKey,
    proof_payload: &[u8],
    col_refs: &[&[halo2_backend::Scalar]],
) -> bool {
    verify_halo2_ipa_payload_columns_result(params, vk, proof_payload, col_refs).is_ok()
}
#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
fn verify_halo2_ipa_payload_optional_columns(
    params: &PastaParams,
    vk: &halo2_backend::VerifyingKey,
    proof_payload: &[u8],
    col_refs: &[&[halo2_backend::Scalar]],
) -> bool {
    if col_refs.is_empty() {
        verify_halo2_ipa_payload_no_instances(params, vk, proof_payload)
    } else {
        verify_halo2_ipa_payload_columns(params, vk, proof_payload, col_refs)
    }
}
/// Build a Halo2 IPA `ivm-execution-v1` proof envelope for IVM proved execution.
///
/// The produced proof binds these public commitments:
/// `(code_hash, overlay_hash, events_commitment, gas_policy_commitment)`.
///
/// Note: the current `ivm-execution-v1` circuit is a **binding** circuit. It does **not**
/// prove correct IVM execution semantics by itself, so admission still performs deterministic
/// VM replay to recompute the overlay/commitments and reject mismatches.
///
/// If `proving_key_bytes` is provided, it is used as the proving key after strict
/// compatibility checks against `vk_box`. If omitted, the proving key is derived
/// from `vk_box` and the canonical `IvmExecutionBindV1` circuit.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_halo2_ipa_ivm_execution_envelope(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    code_hash: iroha_crypto::Hash,
    overlay_hash: iroha_crypto::Hash,
    events_commitment: iroha_crypto::Hash,
    gas_policy_commitment: iroha_crypto::Hash,
    proving_key_bytes: Option<&[u8]>,
) -> Result<ProofBox, String> {
    use halo2_backend::Scalar;
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope};
    use std::io::Cursor;
    if !is_ivm_execution_v1_circuit_id(circuit_id) {
        return Err(format!(
            "unsupported IVM execution circuit id `{circuit_id}`"
        ));
    }
    if vk_box.backend.as_str() != ZK_BACKEND_HALO2_IPA {
        return Err("ivm execution proving requires halo2/ipa verifying key backend".to_owned());
    }
    ensure_halo2_ipa_ivm_execution_canonical_vk_box(vk_box)?;
    let params =
        zkparse::params_for_circuit_v1(vk_box.bytes.as_slice(), IVM_EXECUTION_V1_CIRCUIT_ID)
            .ok_or_else(|| {
                "invalid fixed IVM execution parameter metadata in verifying key envelope"
                    .to_owned()
            })?;
    let parsed_vk: halo2_backend::VerifyingKey = zkparse::vk_from_bytes::<
        pasta_tiny::IvmExecutionBindV1,
    >(vk_box.bytes.as_slice(), &params)
    .ok_or_else(|| "missing/invalid H2VK payload for ivm-execution-v1 verifying key".to_owned())?;
    let code_limbs = hash_to_u64_limbs_le(&code_hash);
    let overlay_limbs = hash_to_u64_limbs_le(&overlay_hash);
    let events_limbs = hash_to_u64_limbs_le(&events_commitment);
    let gas_limbs = hash_to_u64_limbs_le(&gas_policy_commitment);
    let values: [Scalar; 16] = [
        Scalar::from(code_limbs[0]),
        Scalar::from(code_limbs[1]),
        Scalar::from(code_limbs[2]),
        Scalar::from(code_limbs[3]),
        Scalar::from(overlay_limbs[0]),
        Scalar::from(overlay_limbs[1]),
        Scalar::from(overlay_limbs[2]),
        Scalar::from(overlay_limbs[3]),
        Scalar::from(events_limbs[0]),
        Scalar::from(events_limbs[1]),
        Scalar::from(events_limbs[2]),
        Scalar::from(events_limbs[3]),
        Scalar::from(gas_limbs[0]),
        Scalar::from(gas_limbs[1]),
        Scalar::from(gas_limbs[2]),
        Scalar::from(gas_limbs[3]),
    ];
    let instance_columns_owned: Vec<Vec<Scalar>> =
        values.iter().map(|value| vec![*value]).collect();
    let instance_columns: Vec<&[Scalar]> =
        instance_columns_owned.iter().map(Vec::as_slice).collect();
    let instance_refs: Vec<&[&[Scalar]]> = vec![instance_columns.as_slice()];
    let vk_commitment = hash_vk(vk_box);
    let proving_key: halo2_backend::ProvingKey = if let Some(bytes) = proving_key_bytes {
        let proving_key_raw = decode_halo2_ipa_proving_key_archive(
            bytes,
            IVM_EXECUTION_V1_CIRCUIT_ID,
            vk_commitment,
        )?;
        preflight_halo2_ipa_processed_proving_key(&proving_key_raw, &parsed_vk, &params)?;
        let mut cursor = Cursor::new(proving_key_raw.as_slice());
        let pk = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            read_proving_key::<pasta_tiny::IvmExecutionBindV1, _>(&mut cursor)
        }))
        .map_err(|_| "failed to decode proving key: Halo2 reader panicked".to_owned())?
        .map_err(|err| format!("failed to decode proving key: {err}"))?;
        let consumed = usize::try_from(cursor.position()).unwrap_or(usize::MAX);
        if consumed != proving_key_raw.len() {
            return Err("failed to decode proving key: trailing bytes".to_owned());
        }
        ensure_halo2_ipa_proving_key_compatible(
            &pk,
            &parsed_vk,
            &params,
            "proving key domain does not match IPAK parameters",
            "proving key verifying key does not match vk_ref bytes",
        )?;
        if halo2_backend::proving_key_to_processed_bytes(&pk) != proving_key_raw {
            return Err("failed to decode proving key: non-canonical encoding".to_owned());
        }
        pk
    } else {
        halo2_backend::keygen_pk(
            &params,
            parsed_vk.clone(),
            &pasta_tiny::IvmExecutionBindV1::default(),
        )
        .map_err(|err| format!("failed to derive proving key: {err}"))?
    };
    let circuit = pasta_tiny::IvmExecutionBindV1 { values };
    let proof_raw = create_halo2_ipa_proof(
        &params,
        &proving_key,
        circuit,
        &instance_refs,
        "ivm-execution-v1",
    )?;
    let mut proof_payload = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_payload, &proof_raw);
    zk1::wrap_append_instances_pasta_fp_cols(instance_columns.as_slice(), &mut proof_payload);
    let public_inputs = ivm_execution_public_inputs_schema_descriptor().to_vec();
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: circuit_id.to_owned(),
        vk_hash: vk_commitment,
        public_inputs,
        proof_bytes: proof_payload,
        aux: Vec::new(),
    };
    let encoded = norito::encode_canonical(&envelope)
        .map_err(|err| format!("failed to encode OpenVerifyEnvelope: {err}"))?;
    Ok(ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), encoded))
}
/// Derive Halo2 IPA proving-key bytes for the canonical `ivm-execution-v1` circuit.
///
/// The returned bytes are a Norito archive containing the Halo2 `ProvingKey`
/// serialization, canonical circuit family, and verifier-key commitment,
/// suitable for persistence in the Torii prover key store (`<backend>__<name>.pk`).
///
/// Note: this operation is expensive (key generation) and should be performed offline.
#[cfg(feature = "zk-halo2-ipa")]
pub fn derive_halo2_ipa_ivm_execution_proving_key_bytes(
    vk_box: &VerifyingKeyBox,
) -> Result<Vec<u8>, String> {
    if vk_box.backend.as_str() != ZK_BACKEND_HALO2_IPA {
        return Err(
            "ivm execution proving key derivation requires halo2/ipa verifying key backend"
                .to_owned(),
        );
    }
    ensure_halo2_ipa_ivm_execution_canonical_vk_box(vk_box)?;
    let params =
        zkparse::params_for_circuit_v1(vk_box.bytes.as_slice(), IVM_EXECUTION_V1_CIRCUIT_ID)
            .ok_or_else(|| {
                "invalid fixed IVM execution parameter metadata in verifying key envelope"
                    .to_owned()
            })?;
    let parsed_vk: halo2_backend::VerifyingKey = zkparse::vk_from_bytes::<
        pasta_tiny::IvmExecutionBindV1,
    >(vk_box.bytes.as_slice(), &params)
    .ok_or_else(|| "missing/invalid H2VK payload for ivm-execution-v1 verifying key".to_owned())?;
    let pk = halo2_backend::keygen_pk(
        &params,
        parsed_vk,
        &pasta_tiny::IvmExecutionBindV1::default(),
    )
    .map_err(|err| format!("failed to derive proving key: {err}"))?;
    encode_halo2_ipa_proving_key_archive(
        IVM_EXECUTION_V1_CIRCUIT_ID,
        hash_vk(vk_box),
        halo2_backend::proving_key_to_processed_bytes(&pk),
    )
}
pub(crate) fn normalize_stark_fri_circuit_id_for_backend(
    backend: &str,
    raw: &str,
) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == backend {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix(backend) {
        if let Some(rest) = rest.strip_prefix(':') {
            return (!rest.is_empty()).then(|| trimmed.to_string());
        }
        if let Some(rest) = rest.strip_prefix('/') {
            return (!rest.is_empty()).then(|| format!("{backend}:{rest}"));
        }
    }
    Some(format!("{backend}:{trimmed}"))
}
fn stark_open_verify_circuit_id_matches_backend(backend: &str, circuit_id: &str) -> bool {
    if circuit_id.len() > iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES
        || !iroha_data_model::zk::open_verify_circuit_id_is_portable(circuit_id)
        || iroha_data_model::zk::open_verify_circuit_id_uses_reserved_privacy_protocol_label_v1(
            circuit_id,
        )
        || !is_stark_fri_v1_backend(backend)
        || normalize_stark_fri_circuit_id_for_backend(backend, circuit_id).is_none()
    {
        return false;
    }
    let trimmed = circuit_id.trim();
    if stark_open_verify_circuit_id_uses_reserved_proof_family(trimmed) {
        return false;
    }
    if backend == ZK_BACKEND_STARK_FRI_V1 {
        return true;
    }
    if trimmed == ZK_BACKEND_STARK_FRI_V1 || trimmed.starts_with("stark/fri:") {
        return false;
    }
    if trimmed.starts_with("stark/fri/") {
        return trimmed
            .strip_prefix(backend)
            .is_some_and(|suffix| suffix.starts_with(':') || suffix.starts_with('/'));
    }
    true
}
#[cfg(feature = "zk-stark")]
#[derive(Clone, Copy)]
enum StarkFriBackendHashPolicyV1 {
    Any,
    Exact(u8),
}
#[cfg(feature = "zk-stark")]
impl StarkFriBackendHashPolicyV1 {
    fn expected(self) -> Option<u8> {
        match self {
            Self::Any => None,
            Self::Exact(hash_fn) => Some(hash_fn),
        }
    }
}
#[cfg(feature = "zk-stark")]
fn stark_fri_backend_hash_policy_v1(backend: &str) -> Option<StarkFriBackendHashPolicyV1> {
    use crate::zk_stark::{STARK_HASH_POSEIDON2_V1, STARK_HASH_SHA256_V1};
    match backend {
        ZK_BACKEND_STARK_FRI_V1 => Some(StarkFriBackendHashPolicyV1::Any),
        "stark/fri/sha256-goldilocks" | "stark/fri/sha256_goldilocks.v1" => {
            Some(StarkFriBackendHashPolicyV1::Exact(STARK_HASH_SHA256_V1))
        }
        "stark/fri/poseidon2-goldilocks" => {
            Some(StarkFriBackendHashPolicyV1::Exact(STARK_HASH_POSEIDON2_V1))
        }
        _ => None,
    }
}
#[cfg(feature = "zk-stark")]
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct StarkVerifyingKeyCacheKeyV1 {
    backend: String,
    circuit_id: String,
    vk_hash: [u8; 32],
}
#[cfg(feature = "zk-stark")]
type StarkVerifyingKeyCacheV1 = std::sync::Mutex<
    std::collections::BTreeMap<
        StarkVerifyingKeyCacheKeyV1,
        crate::zk_stark::StarkFriVerifyingKeyV1,
    >,
>;
#[cfg(feature = "zk-stark")]
static STARK_VERIFYING_KEY_CACHE_V1: std::sync::OnceLock<StarkVerifyingKeyCacheV1> =
    std::sync::OnceLock::new();
/// Decode and validate a canonical STARK/FRI V1 verifier key for one registry binding.
///
/// The returned value is the typed, bounded material that proof verification
/// consumes. Validation binds its circuit and hash function to the exact
/// production backend before a registry record can retain the original bytes.
#[cfg(feature = "zk-stark")]
pub(crate) fn validate_stark_fri_verifying_key_v1(
    backend: &str,
    circuit_id: &str,
    bytes: &[u8],
) -> Result<crate::zk_stark::StarkFriVerifyingKeyV1, String> {
    if !stark_open_verify_circuit_id_matches_backend(backend, circuit_id) {
        return Err("STARK/FRI circuit id does not match the production backend".to_owned());
    }
    if bytes.len() > crate::zk_stark::STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES {
        return Err(format!(
            "STARK/FRI verifier key exceeds the {}-byte limit",
            crate::zk_stark::STARK_FRI_VERIFYING_KEY_V1_MAX_BYTES
        ));
    }
    let hash_policy = stark_fri_backend_hash_policy_v1(backend)
        .ok_or_else(|| "unsupported STARK/FRI backend variant".to_owned())?;
    let expected_circuit_id = normalize_stark_fri_circuit_id_for_backend(backend, circuit_id)
        .ok_or_else(|| "invalid STARK/FRI registry circuit id".to_owned())?;
    let cache_key = StarkVerifyingKeyCacheKeyV1 {
        backend: backend.to_owned(),
        circuit_id: expected_circuit_id.clone(),
        vk_hash: hash_vk_bytes(backend, bytes),
    };
    let cache = STARK_VERIFYING_KEY_CACHE_V1
        .get_or_init(|| std::sync::Mutex::new(std::collections::BTreeMap::new()));
    if let Some(cached) = cache
        .lock()
        .map_err(|_| "STARK/FRI verifier-key cache lock poisoned".to_owned())?
        .get(&cache_key)
        .cloned()
    {
        return Ok(cached);
    }
    let payload = crate::zk_stark::decode_stark_fri_verifying_key_v1(bytes)?;
    crate::zk_stark::validate_stark_fri_canonical_verifying_key_payload(
        &payload,
        &payload.circuit_id,
        "OpenVerify",
    )?;
    let payload_circuit_id =
        normalize_stark_fri_circuit_id_for_backend(backend, &payload.circuit_id)
            .ok_or_else(|| "invalid STARK/FRI verifier-key circuit id".to_owned())?;
    if payload_circuit_id != expected_circuit_id {
        return Err("STARK/FRI verifier-key circuit id does not match registry record".to_owned());
    }
    if hash_policy
        .expected()
        .is_some_and(|expected| payload.hash_fn != expected)
    {
        return Err("STARK/FRI verifier-key hash function does not match backend".to_owned());
    }
    let mut guard = cache
        .lock()
        .map_err(|_| "STARK/FRI verifier-key cache lock poisoned".to_owned())?;
    Ok(guard.entry(cache_key).or_insert(payload).clone())
}
fn stark_open_verify_circuit_id_uses_reserved_proof_family(circuit_id: &str) -> bool {
    let trimmed = circuit_id.trim();
    if stark_open_verify_circuit_id_fragment_uses_reserved_proof_family(trimmed) {
        return true;
    }
    let Some(stark_suffix) = trimmed
        .strip_prefix("stark/fri:")
        .or_else(|| trimmed.strip_prefix("stark/fri/"))
    else {
        return false;
    };
    let circuit_fragment = stark_suffix
        .split_once(':')
        .or_else(|| stark_suffix.split_once('/'))
        .map_or(stark_suffix, |(_, fragment)| fragment);
    stark_open_verify_circuit_id_fragment_uses_reserved_proof_family(circuit_fragment)
}
fn stark_open_verify_circuit_id_fragment_uses_reserved_proof_family(fragment: &str) -> bool {
    let lower = fragment.to_ascii_lowercase();
    lower == "halo2"
        || lower
            .strip_prefix("halo2")
            .is_some_and(|suffix| suffix.starts_with('/') || suffix.starts_with(':'))
        || is_trusted_setup_backend_label(&lower)
}
/// Normalize the retired generic-STARK ZK-ACE relation id.
///
/// This is a tombstone only: callers must use the typed privacy protocol and
/// the dedicated native engine. Keeping this closed rejection prevents an old
/// generic `OpenVerify` wire from being reintroduced under a backend alias.
fn normalized_retired_zk_ace_stark_circuit_id_for_backend(backend: &str) -> Option<String> {
    normalize_stark_fri_circuit_id_for_backend(
        backend,
        iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
    )
}
fn normalized_bfv_full_bootstrap_stark_circuit_id_for_backend(backend: &str) -> Option<String> {
    normalize_stark_fri_circuit_id_for_backend(
        backend,
        iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
    )
}
fn normalized_ivm_execution_stark_circuit_id_for_backend(backend: &str) -> Option<String> {
    normalize_stark_fri_circuit_id_for_backend(backend, IVM_EXECUTION_V1_CIRCUIT_ID)
}
fn normalized_circuit_is_governance_vote_relation_for_backend(
    backend: &str,
    normalized_circuit_id: &str,
) -> bool {
    [
        GOVERNANCE_BALLOT_CIRCUIT_ID_V1,
        GOVERNANCE_TALLY_CIRCUIT_ID_V1,
    ]
    .into_iter()
    .filter_map(|circuit_id| normalize_stark_fri_circuit_id_for_backend(backend, circuit_id))
    .any(|circuit_id| circuit_id == normalized_circuit_id)
}
/// Return whether a normalized circuit id names a typed Soracloud FHE relation.
///
/// These circuit ids must never fall back to the generic binding AIR: that AIR
/// only authenticates public metadata and does not prove any of the private FHE
/// witness relations advertised by the typed Soracloud protocols.
fn normalized_circuit_is_soracloud_fhe_relation_for_backend(
    backend: &str,
    normalized_circuit_id: &str,
) -> bool {
    [
        iroha_data_model::soracloud::SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
        iroha_data_model::soracloud::SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
        iroha_data_model::soracloud::SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
        iroha_data_model::soracloud::SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
    ]
    .into_iter()
    .filter_map(|circuit_id| normalize_stark_fri_circuit_id_for_backend(backend, circuit_id))
    .any(|circuit_id| circuit_id == normalized_circuit_id)
}
#[cfg(feature = "zk-stark")]
pub(crate) fn stark_open_verify_domain_tag_current(
    backend: &str,
    circuit_id: &str,
    vk_hash: [u8; 32],
    env_public_inputs: &[u8],
    public_inputs: &[Vec<[u8; 32]>],
) -> String {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"iroha:zk:stark-fri-open-proof:v1");
    preimage.extend_from_slice(&(backend.len() as u64).to_le_bytes());
    preimage.extend_from_slice(backend.as_bytes());
    preimage.extend_from_slice(&(circuit_id.len() as u64).to_le_bytes());
    preimage.extend_from_slice(circuit_id.as_bytes());
    preimage.extend_from_slice(&vk_hash);
    preimage.extend_from_slice(&(env_public_inputs.len() as u64).to_le_bytes());
    preimage.extend_from_slice(env_public_inputs);
    preimage.extend_from_slice(&(public_inputs.len() as u64).to_le_bytes());
    for column in public_inputs {
        preimage.extend_from_slice(&(column.len() as u64).to_le_bytes());
        for value in column {
            preimage.extend_from_slice(value);
        }
    }
    let digest = Sha256::digest(&preimage);
    hex::encode(digest)
}
#[cfg(feature = "zk-stark")]
const STARK_BINDING_AIR_CONSTANT: u64 = 17;
#[cfg(feature = "zk-stark")]
const STARK_BINDING_AIR_Z_COEFF: u64 = 19;
#[cfg(feature = "zk-stark")]
const STARK_GOLDILOCKS_MODULUS: u128 = (1u128 << 64) - (1u128 << 32) + 1;
#[cfg(feature = "zk-stark")]
pub(crate) const STARK_OPEN_VERIFY_AIR_TRANSCRIPT_LABEL_V1: &str = "IROHA-STARK-AIR-V1";
#[cfg(feature = "zk-stark")]
fn stark_binding_air_preimage(
    backend: &str,
    circuit_id: &str,
    vk_hash: [u8; 32],
    env_public_inputs: &[u8],
    public_inputs: &[Vec<[u8; 32]>],
) -> Vec<u8> {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"iroha:zk:stark-binding-air:v1");
    preimage.extend_from_slice(&(backend.len() as u64).to_le_bytes());
    preimage.extend_from_slice(backend.as_bytes());
    preimage.extend_from_slice(&(circuit_id.len() as u64).to_le_bytes());
    preimage.extend_from_slice(circuit_id.as_bytes());
    preimage.extend_from_slice(&vk_hash);
    preimage.extend_from_slice(&(env_public_inputs.len() as u64).to_le_bytes());
    preimage.extend_from_slice(env_public_inputs);
    preimage.extend_from_slice(&(public_inputs.len() as u64).to_le_bytes());
    let mut cell_count = 0u64;
    for column in public_inputs {
        preimage.extend_from_slice(&(column.len() as u64).to_le_bytes());
        cell_count = cell_count.saturating_add(column.len() as u64);
        for value in column {
            preimage.extend_from_slice(value);
        }
    }
    preimage.extend_from_slice(&cell_count.to_le_bytes());
    preimage
}
#[cfg(feature = "zk-stark")]
fn stark_field_limb_from_digest(bytes: &[u8]) -> u64 {
    let mut word = [0u8; 8];
    word.copy_from_slice(bytes);
    let value = u64::from_le_bytes(word);
    (u128::from(value) % STARK_GOLDILOCKS_MODULUS) as u64
}
#[cfg(feature = "zk-stark")]
fn stark_binding_air_terms(
    backend: &str,
    circuit_id: &str,
    vk_hash: [u8; 32],
    env_public_inputs: &[u8],
    public_inputs: &[Vec<[u8; 32]>],
) -> Vec<crate::zk_stark::StarkCompositionTermV1> {
    let preimage = stark_binding_air_preimage(
        backend,
        circuit_id,
        vk_hash,
        env_public_inputs,
        public_inputs,
    );
    let digest = Sha256::digest(&preimage);
    let mut terms = Vec::with_capacity(6);
    for (idx, chunk) in digest.chunks_exact(8).enumerate() {
        let coeff = (idx as u64) + 3;
        terms.push(crate::zk_stark::StarkCompositionTermV1 {
            wire_index: idx as u32,
            value: stark_field_limb_from_digest(chunk),
            coeff,
        });
    }
    terms.push(crate::zk_stark::StarkCompositionTermV1 {
        wire_index: 4,
        value: (public_inputs.len() as u128 % STARK_GOLDILOCKS_MODULUS) as u64,
        coeff: 11,
    });
    let cell_count = public_inputs
        .iter()
        .map(Vec::len)
        .fold(0usize, usize::saturating_add);
    terms.push(crate::zk_stark::StarkCompositionTermV1 {
        wire_index: 5,
        value: (cell_count as u128 % STARK_GOLDILOCKS_MODULUS) as u64,
        coeff: 13,
    });
    terms
}
#[cfg(feature = "zk-stark")]
pub(crate) fn stark_open_verify_air_public_digest_current(
    backend: &str,
    circuit_id: &str,
    vk_hash: [u8; 32],
    env_public_inputs: &[u8],
    public_inputs: &[Vec<[u8; 32]>],
) -> Result<[u8; 32], String> {
    let terms = stark_binding_air_terms(
        backend,
        circuit_id,
        vk_hash,
        env_public_inputs,
        public_inputs,
    );
    crate::zk_stark::stark_air_public_digest_from_composition(
        STARK_BINDING_AIR_CONSTANT,
        STARK_BINDING_AIR_Z_COEFF,
        &terms,
    )
}
/// Build a STARK/FRI `OpenVerifyEnvelope` from backend-native public inputs.
///
/// The first-release native V1 circuit carries an explicit AIR section whose
/// public statement digest is reconstructed from the outer envelope metadata,
/// verifying-key hash, schema descriptor, and public input columns.
#[cfg(feature = "zk-stark")]
pub fn prove_stark_fri_open_verify_envelope(
    backend: &str,
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    schema_descriptor: &[u8],
    public_inputs: Vec<Vec<[u8; 32]>>,
) -> Result<ProofBox, String> {
    prove_stark_fri_open_verify_envelope_with_policy(
        backend,
        circuit_id,
        vk_box,
        schema_descriptor,
        public_inputs,
        StarkOpenVerifyCircuitPolicy::Generic,
    )
}
#[cfg(feature = "zk-stark")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StarkOpenVerifyCircuitPolicy {
    Generic,
    IvmExecution,
}
#[cfg(feature = "zk-stark")]
fn prove_stark_fri_open_verify_envelope_with_policy(
    backend: &str,
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    schema_descriptor: &[u8],
    public_inputs: Vec<Vec<[u8; 32]>>,
    circuit_policy: StarkOpenVerifyCircuitPolicy,
) -> Result<ProofBox, String> {
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};
    if !is_stark_fri_v1_backend(backend) {
        return Err("backend is not a STARK/FRI V1 backend".to_owned());
    }
    if vk_box.backend != backend {
        return Err("STARK verifying key backend mismatch".to_owned());
    }
    let vk_payload =
        validate_stark_fri_verifying_key_v1(backend, circuit_id, vk_box.bytes.as_slice())
            .map_err(|err| format!("invalid STARK verifying key payload: {err}"))?;
    let env_circuit_id = normalize_stark_fri_circuit_id_for_backend(backend, circuit_id)
        .ok_or_else(|| "invalid STARK circuit_id".to_owned())?;
    if !stark_open_verify_circuit_id_matches_backend(backend, circuit_id) {
        return Err("STARK circuit_id does not match backend family".to_owned());
    }
    let is_ivm_execution_circuit = normalized_ivm_execution_stark_circuit_id_for_backend(backend)
        .as_deref()
        == Some(env_circuit_id.as_str());
    if circuit_policy == StarkOpenVerifyCircuitPolicy::Generic && is_ivm_execution_circuit {
        return Err(
            "generic STARK OpenVerify proof cannot target the IVM execution circuit; use the IVM execution STARK prover"
                .to_owned(),
        );
    }
    if circuit_policy == StarkOpenVerifyCircuitPolicy::IvmExecution && !is_ivm_execution_circuit {
        return Err("IVM execution STARK prover requires ivm-execution-v1 circuit_id".to_owned());
    }
    if circuit_policy == StarkOpenVerifyCircuitPolicy::IvmExecution
        && schema_descriptor != ivm_execution_public_inputs_schema_descriptor()
    {
        return Err("IVM execution STARK prover requires canonical public-input schema".to_owned());
    }
    if circuit_policy == StarkOpenVerifyCircuitPolicy::IvmExecution
        && (public_inputs.len() != 16 || !public_inputs.iter().all(|column| column.len() == 1))
    {
        return Err("IVM execution STARK prover requires single-row public inputs".to_owned());
    }
    if normalized_retired_zk_ace_stark_circuit_id_for_backend(backend).as_deref()
        == Some(env_circuit_id.as_str())
    {
        return Err(
            "generic STARK OpenVerify proof cannot target the retired ZK-ACE relation; use SubmitPrivacyProofV1"
                .to_owned(),
        );
    }
    if normalized_bfv_full_bootstrap_stark_circuit_id_for_backend(backend).as_deref()
        == Some(env_circuit_id.as_str())
    {
        return Err(
            "generic STARK OpenVerify proof cannot target the BFV full-bootstrap circuit; use the BFV full-bootstrap STARK prover"
                .to_owned(),
        );
    }
    if normalized_circuit_is_governance_vote_relation_for_backend(backend, &env_circuit_id) {
        return Err(
            "generic STARK OpenVerify proof cannot target a governance vote role; a dedicated semantic governance circuit is required"
                .to_owned(),
        );
    }
    if circuit_policy == StarkOpenVerifyCircuitPolicy::Generic
        && normalized_circuit_is_soracloud_fhe_relation_for_backend(backend, &env_circuit_id)
    {
        return Err(
            "generic STARK OpenVerify proof cannot target a Soracloud FHE relation; a dedicated typed Soracloud verifier is required"
                .to_owned(),
        );
    }
    let vk_circuit_id = normalize_stark_fri_circuit_id_for_backend(backend, &vk_payload.circuit_id)
        .ok_or_else(|| "invalid STARK verifying key circuit_id".to_owned())?;
    if env_circuit_id != vk_circuit_id {
        return Err("STARK verifying key circuit_id mismatch".to_owned());
    }
    let expected_hash_fn = stark_fri_backend_hash_policy_v1(backend)
        .ok_or_else(|| "unsupported STARK/FRI backend variant".to_owned())?
        .expected()
        .unwrap_or(vk_payload.hash_fn);
    if vk_payload.hash_fn != expected_hash_fn {
        return Err("STARK verifying key hash_fn mismatch".to_owned());
    }
    let vk_hash = hash_vk(vk_box);
    let domain_tag = stark_open_verify_domain_tag_current(
        backend,
        circuit_id,
        vk_hash,
        schema_descriptor,
        &public_inputs,
    );
    let params = crate::zk_stark::StarkFriParamsV1 {
        version: 1,
        n_log2: vk_payload.n_log2,
        blowup_log2: vk_payload.blowup_log2,
        fold_arity: vk_payload.fold_arity,
        queries: vk_payload.queries,
        merkle_arity: vk_payload.merkle_arity,
        hash_fn: vk_payload.hash_fn,
        domain_tag,
    };
    let terms = stark_binding_air_terms(
        backend,
        circuit_id,
        vk_hash,
        schema_descriptor,
        &public_inputs,
    );
    let public_digest = crate::zk_stark::stark_air_public_digest_from_composition(
        STARK_BINDING_AIR_CONSTANT,
        STARK_BINDING_AIR_Z_COEFF,
        &terms,
    )?;
    let envelope_bytes = if circuit_policy == StarkOpenVerifyCircuitPolicy::IvmExecution {
        crate::zk_stark::prove_stark_fri_reserved_air_envelope_bytes(
            params,
            STARK_OPEN_VERIFY_AIR_TRANSCRIPT_LABEL_V1.to_owned(),
            env_circuit_id.clone(),
            public_digest,
        )
    } else {
        crate::zk_stark::prove_stark_fri_air_envelope_bytes(
            params,
            STARK_OPEN_VERIFY_AIR_TRANSCRIPT_LABEL_V1.to_owned(),
            env_circuit_id.clone(),
            public_digest,
        )
    }?;
    let open = StarkFriOpenProofV1 {
        version: 1,
        public_inputs,
        envelope_bytes,
    };
    let env = OpenVerifyEnvelope {
        backend: BackendTag::Stark,
        circuit_id: circuit_id.to_owned(),
        vk_hash,
        public_inputs: schema_descriptor.to_vec(),
        proof_bytes: norito::encode_canonical(&open)
            .map_err(|err| format!("failed to encode STARK wrapper payload: {err}"))?,
        aux: Vec::new(),
    };
    let bytes = norito::encode_canonical(&env)
        .map_err(|err| format!("failed to encode OpenVerifyEnvelope: {err}"))?;
    Ok(ProofBox::new(backend.to_owned(), bytes))
}
/// Build a STARK/FRI `ivm-execution-v1` proof envelope for IVM proved execution.
///
/// This is the STARK analogue to [`prove_halo2_ipa_ivm_execution_envelope`]. It binds
/// `(code_hash, overlay_hash, events_commitment, gas_policy_commitment)` as backend-native
/// public inputs in a `StarkFriOpenProofV1` wrapper.
#[cfg(feature = "zk-stark")]
pub fn prove_stark_fri_ivm_execution_envelope(
    backend: &str,
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    code_hash: iroha_crypto::Hash,
    overlay_hash: iroha_crypto::Hash,
    events_commitment: iroha_crypto::Hash,
    gas_policy_commitment: iroha_crypto::Hash,
) -> Result<ProofBox, String> {
    let expected_circuit_id =
        normalize_stark_fri_circuit_id_for_backend(backend, IVM_EXECUTION_V1_CIRCUIT_ID)
            .ok_or_else(|| "invalid STARK IVM execution circuit_id".to_owned())?;
    let actual_circuit_id = normalize_stark_fri_circuit_id_for_backend(backend, circuit_id)
        .ok_or_else(|| "invalid STARK IVM execution circuit_id".to_owned())?;
    if actual_circuit_id != expected_circuit_id {
        return Err(format!(
            "STARK IVM execution proving requires `{IVM_EXECUTION_V1_CIRCUIT_ID}` circuit_id"
        ));
    }
    let public_inputs = ivm_execution_public_inputs_columns(
        code_hash,
        overlay_hash,
        events_commitment,
        gas_policy_commitment,
    );
    prove_stark_fri_open_verify_envelope_with_policy(
        backend,
        circuit_id,
        vk_box,
        ivm_execution_public_inputs_schema_descriptor(),
        public_inputs,
        StarkOpenVerifyCircuitPolicy::IvmExecution,
    )
}
#[cfg(any(test, feature = "iroha-core-tests"))]
/// Test fixtures and helpers for constructing deterministic `OpenVerifyEnvelope` payloads.
pub mod test_utils {
    #[allow(unused_imports)]
    use super::*;
    use iroha_crypto::Hash as CryptoHash;
    use iroha_data_model::{
        proof::{ProofBox, VerifyingKeyBox},
        zk::{BackendTag, OpenVerifyEnvelope},
    };
    const HALO2_PROOF_BYTES_LEN: usize = 64;
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    use rand_core_06::{CryptoRng, Error as RandError, RngCore};
    /// Deterministic Halo2 fixture envelope used across unit and integration tests.
    #[derive(Clone, Debug)]
    pub struct FixtureEnvelope {
        /// Norito-encoded `OpenVerifyEnvelope` bytes suitable for `ProofBox`.
        pub proof_bytes: Vec<u8>,
        /// Canonical public inputs serialized in the envelope.
        pub public_inputs: Vec<u8>,
        /// Blake2b-32 hash of `public_inputs`, matching verifier registry expectations.
        pub schema_hash: [u8; 32],
        /// Optional verifying-key bytes for the fixture circuit (ZK1-encoded VK payload).
        pub vk_bytes: Option<Vec<u8>>,
    }
    impl FixtureEnvelope {
        /// Create a `ProofBox` tagged with the provided backend identifier.
        #[must_use]
        pub fn proof_box(&self, backend: impl Into<String>) -> ProofBox {
            ProofBox::new(backend.into(), self.proof_bytes.clone())
        }
        /// Create a verifying-key box for the fixture circuit, if available.
        #[must_use]
        pub fn vk_box(&self, backend: impl Into<String>) -> Option<VerifyingKeyBox> {
            self.vk_bytes
                .as_ref()
                .map(|bytes| VerifyingKeyBox::new(backend.into(), bytes.clone()))
        }
        /// Compute the verifying-key hash for this fixture and backend, if available.
        #[must_use]
        pub fn vk_hash(&self, backend: impl Into<String>) -> Option<[u8; 32]> {
            self.vk_box(backend).map(|vk| super::hash_vk(&vk))
        }
    }
    /// Build a deterministic Halo2 IPA envelope fixture for the provided circuit identifier.
    ///
    /// When the circuit identifier resolves to a supported fixture circuit (currently
    /// `tiny-add`, `tiny-add-public`, `tiny-add-2rows`), the returned
    /// [`FixtureEnvelope`] embeds a real Halo2 proof and VK bytes.
    /// Otherwise, it falls back to a deterministic placeholder payload for negative tests.
    /// The public input bytes and their Blake2b hash are returned so tests can reuse the hash when
    /// registering verifying keys to satisfy `public_inputs_schema_hash` requirements.
    #[must_use]
    pub fn halo2_fixture_envelope(
        circuit_id: impl Into<String>,
        vk_hash: [u8; 32],
    ) -> FixtureEnvelope {
        let circuit_id = circuit_id.into();
        let mut vk_bytes = None;
        let (proof_payload, public_inputs) = fixture_circuit_from_id(circuit_id.as_str())
            .map_or_else(
                || {
                    let public_inputs = fixture_public_inputs_bytes();
                    let proof_payload = halo2_proof_payload(&public_inputs);
                    (proof_payload, public_inputs)
                },
                |fixture| {
                    let (proof_payload, public_inputs, vk) = fixture();
                    vk_bytes = Some(vk);
                    (proof_payload, public_inputs)
                },
            );
        let schema_hash: [u8; 32] = CryptoHash::new(&public_inputs).into();
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id,
            vk_hash,
            public_inputs: public_inputs.clone(),
            proof_bytes: proof_payload,
            aux: Vec::new(),
        };
        let proof_bytes = norito::encode_canonical(&envelope)
            .expect("OpenVerifyEnvelope Norito serialization must work");
        FixtureEnvelope {
            proof_bytes,
            public_inputs,
            schema_hash,
            vk_bytes,
        }
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[must_use]
    fn halo2_ivm_binding_envelope(
        circuit_id: &str,
        code_hash: CryptoHash,
        overlay_hash: CryptoHash,
    ) -> FixtureEnvelope {
        use ff::PrimeField as _;
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{ProvingKey, create_proof, keygen_pk, keygen_vk},
            poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
            transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
        };
        #[derive(Clone)]
        struct KeyMaterial {
            k: u32,
            pk: ProvingKey<Curve>,
            vk_bytes: Vec<u8>,
        }
        fn keys() -> &'static KeyMaterial {
            static CACHE: OnceLock<KeyMaterial> = OnceLock::new();
            CACHE.get_or_init(|| {
                let k = 6u32;
                let params = pasta_params_new(k);
                let circuit = super::pasta_tiny::IvmOverlayBind::default();
                let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
                let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");
                let mut vk_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_ipa_k(&mut vk_bytes, k);
                super::zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
                KeyMaterial { k, pk, vk_bytes }
            })
        }
        fn limbs(hash: &CryptoHash) -> [u64; 4] {
            let bytes: &[u8; 32] = hash.as_ref();
            let mut out = [0u64; 4];
            for (i, limb) in out.iter_mut().enumerate() {
                let start = i * 8;
                let end = start + 8;
                *limb = u64::from_le_bytes(bytes[start..end].try_into().expect("8 bytes"));
            }
            out
        }
        let code_limbs = limbs(&code_hash);
        let overlay_limbs = limbs(&overlay_hash);
        let values: [Scalar; 8] = [
            Scalar::from(code_limbs[0]),
            Scalar::from(code_limbs[1]),
            Scalar::from(code_limbs[2]),
            Scalar::from(code_limbs[3]),
            Scalar::from(overlay_limbs[0]),
            Scalar::from(overlay_limbs[1]),
            Scalar::from(overlay_limbs[2]),
            Scalar::from(overlay_limbs[3]),
        ];
        let inst_cols_owned: Vec<Vec<Scalar>> = values.iter().map(|v| vec![*v]).collect();
        let inst_cols: Vec<&[Scalar]> = inst_cols_owned.iter().map(Vec::as_slice).collect();
        let inst_refs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
        let circuit = super::pasta_tiny::IvmOverlayBind { values };
        let material = keys();
        let params = pasta_params_new(material.k);
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let mut rng = fixture_rng(0x5EED_F1C7_1234_5690);
        create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(
            &params,
            &material.pk,
            &[circuit],
            &inst_refs,
            &mut rng,
            &mut transcript,
        )
        .expect("create proof");
        let proof_raw = transcript.finalize();
        let mut proof_bytes = super::zk1::wrap_start();
        super::zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
        super::zk1::wrap_append_instances_pasta_fp_cols(inst_cols.as_slice(), &mut proof_bytes);
        let mut public_inputs = Vec::with_capacity(values.len() * 32);
        for value in values {
            public_inputs.extend_from_slice(value.to_repr().as_ref());
        }
        let schema_hash: [u8; 32] = CryptoHash::new(&public_inputs).into();
        let vk_hash = {
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), material.vk_bytes.clone());
            super::hash_vk(&vk_box)
        };
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: circuit_id.to_owned(),
            vk_hash,
            public_inputs: public_inputs.clone(),
            proof_bytes,
            aux: Vec::new(),
        };
        let proof_bytes = norito::encode_canonical(&envelope)
            .expect("OpenVerifyEnvelope Norito serialization must work");
        FixtureEnvelope {
            proof_bytes,
            public_inputs,
            schema_hash,
            vk_bytes: Some(material.vk_bytes.clone()),
        }
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[must_use]
    fn halo2_ivm_execution_bind_v1_envelope(
        circuit_id: &str,
        code_hash: CryptoHash,
        overlay_hash: CryptoHash,
        events_commitment: CryptoHash,
        gas_policy_commitment: CryptoHash,
    ) -> FixtureEnvelope {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{ProvingKey, create_proof, keygen_pk, keygen_vk},
            poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
            transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
        };
        #[derive(Clone)]
        struct KeyMaterial {
            k: u32,
            pk: ProvingKey<Curve>,
            vk_bytes: Vec<u8>,
        }
        fn keys() -> &'static KeyMaterial {
            static CACHE: OnceLock<KeyMaterial> = OnceLock::new();
            CACHE.get_or_init(|| {
                let k = 7u32;
                let params = pasta_params_new(k);
                let circuit = super::pasta_tiny::IvmExecutionBindV1::default();
                let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
                let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");
                let mut vk_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_ipa_k(&mut vk_bytes, k);
                super::zk1::wrap_append_circuit_id(
                    &mut vk_bytes,
                    super::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
                );
                super::zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
                KeyMaterial { k, pk, vk_bytes }
            })
        }
        fn limbs(hash: &CryptoHash) -> [u64; 4] {
            let bytes: &[u8; 32] = hash.as_ref();
            let mut out = [0u64; 4];
            for (i, limb) in out.iter_mut().enumerate() {
                let start = i * 8;
                let end = start + 8;
                *limb = u64::from_le_bytes(bytes[start..end].try_into().expect("8 bytes"));
            }
            out
        }
        let code_limbs = limbs(&code_hash);
        let overlay_limbs = limbs(&overlay_hash);
        let events_limbs = limbs(&events_commitment);
        let gas_limbs = limbs(&gas_policy_commitment);
        let values: [Scalar; 16] = [
            Scalar::from(code_limbs[0]),
            Scalar::from(code_limbs[1]),
            Scalar::from(code_limbs[2]),
            Scalar::from(code_limbs[3]),
            Scalar::from(overlay_limbs[0]),
            Scalar::from(overlay_limbs[1]),
            Scalar::from(overlay_limbs[2]),
            Scalar::from(overlay_limbs[3]),
            Scalar::from(events_limbs[0]),
            Scalar::from(events_limbs[1]),
            Scalar::from(events_limbs[2]),
            Scalar::from(events_limbs[3]),
            Scalar::from(gas_limbs[0]),
            Scalar::from(gas_limbs[1]),
            Scalar::from(gas_limbs[2]),
            Scalar::from(gas_limbs[3]),
        ];
        let inst_cols_owned: Vec<Vec<Scalar>> = values.iter().map(|v| vec![*v]).collect();
        let inst_cols: Vec<&[Scalar]> = inst_cols_owned.iter().map(Vec::as_slice).collect();
        let inst_refs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
        let circuit = super::pasta_tiny::IvmExecutionBindV1 { values };
        let material = keys();
        let params = pasta_params_new(material.k);
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let mut rng = fixture_rng(0x5EED_F1C7_1234_5691);
        create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(
            &params,
            &material.pk,
            &[circuit],
            &inst_refs,
            &mut rng,
            &mut transcript,
        )
        .expect("create proof");
        let proof_raw = transcript.finalize();
        let mut proof_bytes = super::zk1::wrap_start();
        super::zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
        super::zk1::wrap_append_instances_pasta_fp_cols(inst_cols.as_slice(), &mut proof_bytes);
        let public_inputs = super::ivm_execution_public_inputs_schema_descriptor().to_vec();
        let schema_hash: [u8; 32] = CryptoHash::new(&public_inputs).into();
        let vk_hash = {
            let vk_box = VerifyingKeyBox::new("halo2/ipa".into(), material.vk_bytes.clone());
            super::hash_vk(&vk_box)
        };
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: circuit_id.to_owned(),
            vk_hash,
            public_inputs: public_inputs.clone(),
            proof_bytes,
            aux: Vec::new(),
        };
        let proof_bytes = norito::encode_canonical(&envelope)
            .expect("OpenVerifyEnvelope Norito serialization must work");
        FixtureEnvelope {
            proof_bytes,
            public_inputs,
            schema_hash,
            vk_bytes: Some(material.vk_bytes.clone()),
        }
    }
    /// Deterministic Halo2 IPA fixture for the historical `ivm-overlay-bind` circuit.
    ///
    /// The circuit exposes 8 instance columns (1 row each) and constrains witness
    /// values to equal those instances. This fixture is retained for regression/
    /// negative tests; `Executable::IvmProved` admission no longer accepts this
    /// binding-only stand-in circuit.
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[must_use]
    pub fn halo2_ivm_overlay_bind_envelope(
        code_hash: CryptoHash,
        overlay_hash: CryptoHash,
    ) -> FixtureEnvelope {
        halo2_ivm_binding_envelope("halo2/ipa:ivm-overlay-bind", code_hash, overlay_hash)
    }
    /// Deterministic Halo2 IPA fixture for `ivm-execution-v1` proof attachments.
    ///
    /// The circuit exposes 16 instance columns (1 row each) corresponding to:
    /// - `code_hash` (4 `u64` limbs, little-endian)
    /// - `overlay_hash` (4 `u64` limbs, little-endian)
    /// - `events_commitment` (4 `u64` limbs, little-endian)
    /// - `gas_policy_commitment` (4 `u64` limbs, little-endian)
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[must_use]
    pub fn halo2_ivm_execution_envelope(
        code_hash: CryptoHash,
        overlay_hash: CryptoHash,
        events_commitment: CryptoHash,
        gas_policy_commitment: CryptoHash,
    ) -> FixtureEnvelope {
        halo2_ivm_execution_bind_v1_envelope(
            super::IVM_EXECUTION_V1_CIRCUIT_ID,
            code_hash,
            overlay_hash,
            events_commitment,
            gas_policy_commitment,
        )
    }
    type FixtureBundle = fn() -> (Vec<u8>, Vec<u8>, Vec<u8>);
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn fixture_circuit_from_id(circuit_id: &str) -> Option<FixtureBundle> {
        let backend = super::normalize_halo2_ipa_circuit_id(circuit_id)?;
        let name = backend.rsplit('/').next()?;
        match name {
            "tiny-add" => Some(tiny_add_bundle),
            "tiny-add-public" => Some(tiny_add_public_bundle),
            "tiny-add2inst-public" => Some(tiny_add2inst_public_bundle),
            "tiny-add-2rows" => Some(tiny_add_2rows_bundle),
            _ => None,
        }
    }
    #[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
    fn fixture_circuit_from_id(_circuit_id: &str) -> Option<FixtureBundle> {
        None
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    struct FixtureRng(u64);
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    impl FixtureRng {
        const fn new(seed: u64) -> Self {
            Self(seed)
        }
        fn next_word(&mut self) -> u64 {
            // Simple LCG for deterministic, fast test entropy.
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0
        }
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    impl RngCore for FixtureRng {
        fn next_u32(&mut self) -> u32 {
            let word = self.next_word();
            u32::try_from(word & u64::from(u32::MAX)).expect("word masked to u32")
        }
        fn next_u64(&mut self) -> u64 {
            self.next_word()
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            let mut offset = 0;
            while offset < dest.len() {
                let chunk = self.next_u64().to_le_bytes();
                let remaining = dest.len() - offset;
                let take = remaining.min(chunk.len());
                dest[offset..offset + take].copy_from_slice(&chunk[..take]);
                offset += take;
            }
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RandError> {
            self.fill_bytes(dest);
            Ok(())
        }
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    impl CryptoRng for FixtureRng {}
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn fixture_rng(seed: u64) -> FixtureRng {
        FixtureRng::new(seed)
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn tiny_add_bundle() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use halo2_proofs::{
            halo2curves::pasta::EqAffine as Curve,
            plonk::{create_proof, keygen_pk, keygen_vk},
            poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
            transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
        };
        static CACHE: OnceLock<(Vec<u8>, Vec<u8>, Vec<u8>)> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                // Proof generation is expensive; cache the fixture and use a deterministic RNG.
                let k = 5u32;
                let params = pasta_params_new(k);
                let circuit = super::pasta_tiny::Add;
                let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
                let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");
                let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
                let mut rng = fixture_rng(0x5EED_F1C7_1234_5678);
                create_proof::<
                    IPACommitmentScheme<Curve>,
                    ProverIPA<'_, Curve>,
                    Challenge255<Curve>,
                    _,
                    _,
                    _,
                >(
                    &params,
                    &pk,
                    &[circuit],
                    &[&[][..]],
                    &mut rng,
                    &mut transcript,
                )
                .expect("create proof");
                let proof_raw = transcript.finalize();
                let mut proof_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
                let mut vk_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_ipa_k(&mut vk_bytes, k);
                super::zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
                let public_inputs = Vec::new();
                (proof_bytes, public_inputs, vk_bytes)
            })
            .clone()
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn tiny_add_public_bundle() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use ff::PrimeField as _;
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{create_proof, keygen_pk, keygen_vk},
            poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
            transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
        };
        static CACHE: OnceLock<(Vec<u8>, Vec<u8>, Vec<u8>)> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                // Proof generation is expensive; cache the fixture and use a deterministic RNG.
                let k = 5u32;
                let params = pasta_params_new(k);
                let circuit = super::pasta_tiny::AddPublic;
                let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
                let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");
                let inst_col = vec![Scalar::from(4u64)];
                let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
                let inst_refs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
                let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
                let mut rng = fixture_rng(0x5EED_F1C7_1234_5679);
                create_proof::<
                    IPACommitmentScheme<Curve>,
                    ProverIPA<'_, Curve>,
                    Challenge255<Curve>,
                    _,
                    _,
                    _,
                >(
                    &params,
                    &pk,
                    &[circuit],
                    &inst_refs,
                    &mut rng,
                    &mut transcript,
                )
                .expect("create proof");
                let proof_raw = transcript.finalize();
                let mut proof_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
                super::zk1::wrap_append_instances_pasta_fp_cols(&inst_cols, &mut proof_bytes);
                let mut vk_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_ipa_k(&mut vk_bytes, k);
                super::zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
                let mut public_inputs = Vec::with_capacity(inst_col.len() * 32);
                for value in inst_col {
                    public_inputs.extend_from_slice(value.to_repr().as_ref());
                }
                (proof_bytes, public_inputs, vk_bytes)
            })
            .clone()
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn tiny_add2inst_public_bundle() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use ff::PrimeField as _;
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{create_proof, keygen_pk, keygen_vk},
            poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
            transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
        };
        static CACHE: OnceLock<(Vec<u8>, Vec<u8>, Vec<u8>)> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                let k = 6u32;
                let params = pasta_params_new(k);
                let circuit = super::pasta_tiny::AddTwoInstPublic;
                let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
                let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");
                let inst0 = vec![Scalar::from(5u64)];
                let inst1 = vec![Scalar::from(8u64)];
                let inst_cols: Vec<&[Scalar]> = vec![inst0.as_slice(), inst1.as_slice()];
                let inst_refs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
                let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
                let mut rng = fixture_rng(0x5EED_F1C7_1234_5681);
                create_proof::<
                    IPACommitmentScheme<Curve>,
                    ProverIPA<'_, Curve>,
                    Challenge255<Curve>,
                    _,
                    _,
                    _,
                >(
                    &params,
                    &pk,
                    &[circuit],
                    &inst_refs,
                    &mut rng,
                    &mut transcript,
                )
                .expect("create proof");
                let proof_raw = transcript.finalize();
                let mut proof_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
                super::zk1::wrap_append_instances_pasta_fp_cols(&inst_cols, &mut proof_bytes);
                let mut vk_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_ipa_k(&mut vk_bytes, k);
                super::zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
                let mut public_inputs = Vec::with_capacity(inst_cols.len() * 32);
                for value in inst0.iter().chain(inst1.iter()) {
                    public_inputs.extend_from_slice(value.to_repr().as_ref());
                }
                (proof_bytes, public_inputs, vk_bytes)
            })
            .clone()
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn tiny_add_2rows_bundle() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        use halo2_proofs::{
            halo2curves::pasta::EqAffine as Curve,
            plonk::{create_proof, keygen_pk, keygen_vk},
            poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
            transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
        };
        static CACHE: OnceLock<(Vec<u8>, Vec<u8>, Vec<u8>)> = OnceLock::new();
        CACHE
            .get_or_init(|| {
                // Proof generation is expensive; cache the fixture and use a deterministic RNG.
                let k = 5u32;
                let params = pasta_params_new(k);
                let circuit = super::pasta_tiny::AddTwoRows;
                let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
                let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");
                let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
                let mut rng = fixture_rng(0x5EED_F1C7_1234_5680);
                create_proof::<
                    IPACommitmentScheme<Curve>,
                    ProverIPA<'_, Curve>,
                    Challenge255<Curve>,
                    _,
                    _,
                    _,
                >(
                    &params,
                    &pk,
                    &[circuit],
                    &[&[][..]],
                    &mut rng,
                    &mut transcript,
                )
                .expect("create proof");
                let proof_raw = transcript.finalize();
                let mut proof_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_proof(&mut proof_bytes, &proof_raw);
                let mut vk_bytes = super::zk1::wrap_start();
                super::zk1::wrap_append_ipa_k(&mut vk_bytes, k);
                super::zk1::wrap_append_vk_pasta(&mut vk_bytes, &vk_h2);
                let public_inputs = Vec::new();
                (proof_bytes, public_inputs, vk_bytes)
            })
            .clone()
    }
    #[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
    #[test]
    fn halo2_fixture_envelope_is_stable_for_tiny_add() {
        let first = halo2_fixture_envelope("halo2/ipa:tiny-add", [0u8; 32]);
        let second = halo2_fixture_envelope("halo2/ipa:tiny-add", [0u8; 32]);
        assert_eq!(first.proof_bytes, second.proof_bytes);
        assert_eq!(first.vk_bytes, second.vk_bytes);
        assert!(!first.proof_bytes.is_empty());
        assert!(first.vk_bytes.is_some());
    }
    #[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
    #[test]
    fn halo2_fixture_envelope_is_stable_for_tiny_add_public() {
        let first = halo2_fixture_envelope("halo2/ipa:tiny-add-public", [0u8; 32]);
        let second = halo2_fixture_envelope("halo2/ipa:tiny-add-public", [0u8; 32]);
        assert_eq!(first.proof_bytes, second.proof_bytes);
        assert_eq!(first.vk_bytes, second.vk_bytes);
        assert!(!first.proof_bytes.is_empty());
        assert!(first.vk_bytes.is_some());
        assert!(!first.public_inputs.is_empty());
    }
    #[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
    #[test]
    fn halo2_fixture_envelope_is_stable_for_tiny_add2inst_public() {
        let first = halo2_fixture_envelope("halo2/ipa:tiny-add2inst-public", [0u8; 32]);
        let second = halo2_fixture_envelope("halo2/ipa:tiny-add2inst-public", [0u8; 32]);
        assert_eq!(first.proof_bytes, second.proof_bytes);
        assert_eq!(first.vk_bytes, second.vk_bytes);
        assert!(!first.proof_bytes.is_empty());
        assert!(first.vk_bytes.is_some());
        assert_eq!(first.public_inputs.len(), 64);
    }
    #[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
    #[test]
    fn halo2_fixture_envelope_is_stable_for_tiny_add_2rows() {
        let first = halo2_fixture_envelope("halo2/ipa:tiny-add-2rows", [0u8; 32]);
        let second = halo2_fixture_envelope("halo2/ipa:tiny-add-2rows", [0u8; 32]);
        assert_eq!(first.proof_bytes, second.proof_bytes);
        assert_eq!(first.vk_bytes, second.vk_bytes);
        assert!(!first.proof_bytes.is_empty());
        assert!(first.vk_bytes.is_some());
    }
    fn fixture_public_inputs_bytes() -> Vec<u8> {
        const STRIDE: usize = 32;
        // anchor root + 1 nullifier + 1 commitment + asset id + policy digest = 5 entries
        const COUNT: usize = 5;
        let mut bytes = vec![0u8; STRIDE * COUNT];
        for (idx, chunk) in bytes.chunks_mut(STRIDE).enumerate() {
            let idx = u8::try_from(idx).expect("fixture chunk index fits in a u8");
            chunk[0] = idx;
        }
        bytes
    }
    fn halo2_proof_payload(_public_inputs: &[u8]) -> Vec<u8> {
        vec![0xAB; HALO2_PROOF_BYTES_LEN]
    }
}
/// Verifier trait for backend-agnostic proof verification.
///
/// Implementations must be deterministic and must not introduce nondeterminism across hardware.
pub trait Verifier {
    /// Return true if this verifier accepts the given backend identifier.
    fn accepts(&self, backend: &str) -> bool;
    /// Verify a proof with an optional verifying key. Returns true on success.
    fn verify(&self, proof: &ProofBox, vk: Option<&VerifyingKeyBox>) -> bool;
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct VkCacheKey {
    backend: String,
    circuit_type: &'static str,
    params_fingerprint: [u8; 32],
    vk_hash: [u8; 32],
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
type CachedVk = Arc<halo2_backend::VerifyingKey>;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
static VK_CACHE: OnceLock<Mutex<BTreeMap<VkCacheKey, CachedVk>>> = OnceLock::new();
#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct BuiltinVkCacheKey {
    backend: String,
    params_fingerprint: [u8; 32],
}
#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
static BUILTIN_VK_CACHE: OnceLock<Mutex<BTreeMap<BuiltinVkCacheKey, CachedVk>>> = OnceLock::new();
#[cfg(feature = "telemetry")]
fn record_vk_cache_event(cache: &'static str, event: &'static str) {
    if let Some(metrics) = iroha_telemetry::metrics::global() {
        metrics
            .zk_verifier_cache_events_total
            .with_label_values(&[cache, event])
            .inc();
    }
}
#[cfg(not(feature = "telemetry"))]
fn record_vk_cache_event(_: &'static str, _: &'static str) {}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn lock_cache<T>(cache: &Mutex<T>) -> Result<MutexGuard<'_, T>, halo2_backend::Error> {
    cache
        .lock()
        .map_err(|_| halo2_backend::constraint_system_failure())
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn resolve_vk_cached_for_type<C, F>(
    backend: &str,
    params: &PastaParams,
    vk_box: &VerifyingKeyBox,
    builder: F,
) -> Result<CachedVk, halo2_backend::Error>
where
    C: halo2_proofs::plonk::Circuit<halo2_backend::Scalar>,
    F: FnOnce() -> Result<halo2_backend::VerifyingKey, halo2_backend::Error>,
{
    let cache = VK_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let params_fp = params_fingerprint(params);
    let vk_hash = hash_vk(vk_box);
    let key = VkCacheKey {
        backend: backend.to_string(),
        circuit_type: core::any::type_name::<C>(),
        params_fingerprint: params_fp,
        vk_hash,
    };
    // Fast path: existing cache entry whose hash matches.
    {
        let guard = lock_cache(cache)?;
        if let Some(entry) = guard.get(&key).cloned() {
            record_vk_cache_event("vk", "hit");
            return Ok(entry);
        }
    }
    record_vk_cache_event("vk", "miss");
    // A registry circuit identifier is a semantic security boundary, not a
    // caller-supplied label for an arbitrary Halo2 constraint system. Build the
    // canonical key for the selected circuit and compare the packaged H2VK
    // bytes before invoking any Halo2 reader on attacker-controlled counts.
    let built = builder()?;
    let packaged = zk1::h2vk_payload(vk_box.bytes.as_slice())
        .map_err(|_| halo2_backend::constraint_system_failure())?;
    let canonical = halo2_backend::verifying_key_to_processed_bytes(&built);
    if packaged != canonical.as_slice() {
        return Err(halo2_backend::constraint_system_failure());
    }
    let arc = Arc::new(built);
    let mut guard = lock_cache(cache)?;
    let entry = match guard.entry(key) {
        Entry::Occupied(existing) => existing.get().clone(),
        Entry::Vacant(slot) => Arc::clone(slot.insert(Arc::clone(&arc))),
    };
    Ok(entry)
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn resolve_vk_cached<C, F>(
    backend: &str,
    params: &PastaParams,
    vk_box: &VerifyingKeyBox,
    _circuit: &C,
    builder: F,
) -> Result<CachedVk, halo2_backend::Error>
where
    C: halo2_proofs::plonk::Circuit<halo2_backend::Scalar>,
    F: FnOnce() -> Result<halo2_backend::VerifyingKey, halo2_backend::Error>,
{
    resolve_vk_cached_for_type::<C, F>(backend, params, vk_box, builder)
}
#[cfg(all(
    test,
    feature = "halo2-dev-tests",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa"
))]
/// Resolve a packaged verifier key without falling back to runtime keygen.
fn resolve_packaged_vk_cached<C>(
    backend: &str,
    params: &PastaParams,
    vk_box: &VerifyingKeyBox,
    _circuit: &C,
) -> Result<CachedVk, halo2_backend::Error>
where
    C: halo2_proofs::plonk::Circuit<halo2_backend::Scalar>,
    C::Params: Default,
{
    let cache = VK_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let params_fp = params_fingerprint(params);
    let vk_hash = hash_vk(vk_box);
    let key = VkCacheKey {
        backend: backend.to_string(),
        circuit_type: core::any::type_name::<C>(),
        params_fingerprint: params_fp,
        vk_hash,
    };
    {
        let guard = lock_cache(cache)?;
        if let Some(entry) = guard.get(&key).cloned() {
            record_vk_cache_event("vk", "hit");
            return Ok(entry);
        }
    }
    record_vk_cache_event("vk", "miss");
    let parsed = zkparse::vk_from_bytes::<C>(vk_box.bytes.as_slice(), params)
        .ok_or_else(halo2_backend::constraint_system_failure)?;
    let arc = Arc::new(parsed);
    let mut guard = lock_cache(cache)?;
    let entry = match guard.entry(key) {
        Entry::Occupied(existing) => existing.get().clone(),
        Entry::Vacant(slot) => Arc::clone(slot.insert(Arc::clone(&arc))),
    };
    Ok(entry)
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
macro_rules! cached_vk_for {
    ($params:expr, $backend:expr, $vk_box:expr, $circuit:expr, |$vk:ident| $body:block) => {{
        let params_ref = $params;
        let vk_ref = $vk_box;
        let circuit = $circuit;
        match resolve_vk_cached($backend, params_ref, vk_ref, &circuit, || {
            halo2_backend::keygen_vk(params_ref, &circuit)
        }) {
            Ok(arc) => {
                let $vk = arc.as_ref();
                $body
            }
            Err(_) => false,
        }
    }};
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn validate_canonical_halo2_ipa_circuit_key<C>(
    backend: &str,
    params: &PastaParams,
    vk_box: &VerifyingKeyBox,
    circuit: C,
) -> Result<(), String>
where
    C: halo2_proofs::plonk::Circuit<halo2_backend::Scalar>,
{
    resolve_vk_cached_for_type::<C, _>(backend, params, vk_box, || {
        halo2_backend::keygen_vk(params, &circuit)
    })
    .map(|_| ())
    .map_err(|_| {
        "Halo2 IPA verifier key does not match the canonical compiled circuit key".to_owned()
    })
}
/// Validate the exact compiled verifier key for a built-in Halo2 IPA V1 circuit.
///
/// Fixed circuit metadata is checked before deterministic parameter
/// construction. The processed `H2VK` bytes are then compared with the key
/// generated from the same concrete circuit type used by proof verification,
/// before attacker-controlled key bytes reach the Halo2 reader.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub(crate) fn validate_builtin_halo2_ipa_verifying_key_v1(
    backend: &str,
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    if vk_box.bytes.len() > HALO2_IPA_VERIFYING_KEY_V1_MAX_BYTES {
        return Err(format!(
            "Halo2 IPA verifying-key container exceeds the {}-byte limit",
            HALO2_IPA_VERIFYING_KEY_V1_MAX_BYTES
        ));
    }
    if vk_box.backend != backend {
        return Err("Halo2 IPA verifier-key backend does not match registry id".to_owned());
    }
    if !halo2_open_verify_circuit_id_matches_backend(backend, circuit_id) {
        return Err("Halo2 IPA circuit id is not admitted for the registry backend".to_owned());
    }
    let params = zkparse::params_for_circuit_v1(&vk_box.bytes, circuit_id)
        .ok_or_else(|| "invalid fixed Halo2 IPA verifier-key metadata".to_owned())?;
    let canonical_circuit_id = normalize_halo2_ipa_circuit_id(circuit_id)
        .ok_or_else(|| "invalid Halo2 IPA circuit id".to_owned())?;
    if confidential_v2::is_confidential_transfer_v2_circuit_id(&canonical_circuit_id) {
        return validate_canonical_halo2_ipa_circuit_key(
            backend,
            &params,
            vk_box,
            confidential_v2::secure_relation_v3::ConfidentialTransferCircuitV3::<
                { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
        );
    }
    if confidential_v2::is_kagemusha_topup_shield_v2_circuit_id(&canonical_circuit_id) {
        return validate_canonical_halo2_ipa_circuit_key(
            backend,
            &params,
            vk_box,
            confidential_v2::secure_relation_v3::KagemushaTopUpShieldCircuitV3::<
                { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
        );
    }
    if confidential_v2::is_confidential_unshield_v2_circuit_id(&canonical_circuit_id) {
        return validate_canonical_halo2_ipa_circuit_key(
            backend,
            &params,
            vk_box,
            confidential_v2::secure_relation_v3::ConfidentialUnshieldFullCircuitV3::<
                { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
        );
    }
    if confidential_v2::is_confidential_unshield_v3_circuit_id(&canonical_circuit_id) {
        return validate_canonical_halo2_ipa_circuit_key(
            backend,
            &params,
            vk_box,
            confidential_v2::secure_relation_v3::ConfidentialUnshieldChangeCircuitV4::<
                { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
        );
    }
    let verifier_backend = canonical_circuit_id.replace("/ipa/", "/");
    if verifier_backend == IVM_EXECUTION_V1_HALO2_BACKEND {
        return validate_canonical_halo2_ipa_circuit_key(
            backend,
            &params,
            vk_box,
            pasta_tiny::IvmExecutionBindV1::default(),
        );
    }
    #[cfg(feature = "zk-halo2")]
    {
        if verifier_backend == KAIGI_ROSTER_BACKEND {
            return validate_canonical_halo2_ipa_circuit_key(
                backend,
                &params,
                vk_box,
                KaigiRosterJoinCircuit::default(),
            );
        }
        if verifier_backend == KAIGI_USAGE_BACKEND {
            return validate_canonical_halo2_ipa_circuit_key(
                backend,
                &params,
                vk_box,
                KaigiUsageCommitmentCircuit::default(),
            );
        }
    }
    Err("Halo2 IPA circuit has no compiled V1 verifier-key validator".to_owned())
}
/// Reject built-in Halo2 IPA verifier-key registration when the verifier
/// backend is not compiled into this binary.
#[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
pub(crate) fn validate_builtin_halo2_ipa_verifying_key_v1(
    _backend: &str,
    _circuit_id: &str,
    _vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    Err("Halo2 IPA verifier-key validation requires the Halo2 backend".to_owned())
}
#[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
#[allow(unused_macros)]
macro_rules! cached_vk_for {
    ($params:expr, $backend:expr, $vk_box:expr, $circuit:expr, |$vk:ident| $body:block) => {{
        let _ = ($params, $backend, $vk_box, $circuit);
        false
    }};
}
#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
fn keygen_vk_cached<C>(
    backend: &str,
    params: &PastaParams,
    circuit: &C,
) -> Result<CachedVk, halo2_backend::Error>
where
    C: halo2_proofs::plonk::Circuit<halo2_backend::Scalar>,
{
    let cache = BUILTIN_VK_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let key = BuiltinVkCacheKey {
        backend: backend.to_owned(),
        params_fingerprint: params_fingerprint(params),
    };
    {
        let guard = lock_cache(cache)?;
        if let Some(existing) = guard.get(&key).cloned() {
            record_vk_cache_event("builtin", "hit");
            return Ok(existing);
        }
    }
    record_vk_cache_event("builtin", "miss");
    let vk = halo2_backend::keygen_vk(params, circuit)?;
    let arc = Arc::new(vk);
    let mut guard = lock_cache(cache)?;
    let entry = match guard.entry(key) {
        Entry::Occupied(existing) => existing.get().clone(),
        Entry::Vacant(slot) => Arc::clone(slot.insert(Arc::clone(&arc))),
    };
    Ok(entry)
}
// Parsed verifying keys are cached above and keyed by backend, parameter fingerprint, and
// verifying-key hash so repeated proofs avoid repeated strict parsing.
/// Built-in verifier: native IPA polynomial opening (transparent) using Norito envelope.
#[cfg(feature = "zk-ipa-native")]
struct IpaNativeVerifier;
#[cfg(feature = "zk-ipa-native")]
impl Verifier for IpaNativeVerifier {
    fn accepts(&self, backend: &str) -> bool {
        backend == "halo2/ipa/poly-open"
    }
    fn verify(&self, proof: &ProofBox, _vk: Option<&VerifyingKeyBox>) -> bool {
        verify_ipa_open_envelope(proof)
    }
}
/// Return a static registry of built-in verifiers enabled by features.
fn verifier_registry() -> Vec<&'static dyn Verifier> {
    #[cfg(feature = "zk-ipa-native")]
    {
        static IPA_NATIVE: IpaNativeVerifier = IpaNativeVerifier;
        vec![&IPA_NATIVE]
    }
    #[cfg(not(feature = "zk-ipa-native"))]
    {
        Vec::new()
    }
}
/// Try to verify using the built-in registry. Returns Some(result) if a matching
/// verifier exists, otherwise None so callers may fall back to other integrations.
fn verify_with_registry(
    backend: &str,
    proof: &ProofBox,
    vk: Option<&VerifyingKeyBox>,
) -> Option<bool> {
    for ver in verifier_registry() {
        if ver.accepts(backend) {
            return Some(ver.verify(proof, vk));
        }
    }
    None
}
/// Unified ZK envelope helpers (`ZK1 | TLV*`).
///
/// The envelope is a linear sequence:
///  - Magic: `b"ZK1\0"` (4 bytes)
///  - Zero or more TLVs, each: `tag[4] || len[u32 LE] || payload[len]`.
///
/// Recognized tags:
///  - `b"PROF"`: raw proof transcript bytes (opaque to this module).
///  - `b"IPAK"`: Halo2 IPA params, payload is `u32 k` (little-endian).
///  - `b"CID1"`: circuit-family identifier bytes for commitment domain separation.
///  - `b"I10P"`: Instance columns over Pasta Fp (cols[u32], rows[u32], rows*cols scalars).
///
/// Notes:
///  - Backends remain identified outside of the envelope via `ProofBox.backend`.
mod zk1 {
    use super::*;
    use std::io::{Cursor, Read};
    const MAGIC: &[u8; 4] = b"ZK1\0";
    const HALO2_PASTA_PROCESSED_VK_HEADER_LEN: usize = 10;
    const HALO2_PASTA_PROCESSED_POINT_LEN: usize = 32;
    #[allow(dead_code)]
    fn read_u32(r: &mut Cursor<&[u8]>) -> Option<u32> {
        let mut le = [0u8; 4];
        r.read_exact(&mut le).ok()?;
        Some(u32::from_le_bytes(le))
    }
    #[allow(dead_code)]
    fn read_tlv<'a>(r: &mut Cursor<&'a [u8]>) -> Option<([u8; 4], &'a [u8])> {
        let mut tag = [0u8; 4];
        r.read_exact(&mut tag).ok()?;
        let len = usize::try_from(read_u32(r)?).ok()?;
        if len > MAX_PROOF_LEN {
            return None;
        }
        let pos = usize::try_from(r.position()).ok()?;
        let end = pos.checked_add(len)?;
        if end > r.get_ref().len() {
            return None;
        }
        r.set_position(u64::try_from(end).ok()?);
        let bytes = r.get_ref();
        Some((tag, &bytes[pos..end]))
    }
    #[allow(dead_code)]
    /// Append a TLV entry to the envelope buffer. This helper is used by
    /// zk-specific tests and feature-gated code paths that manufacture
    /// synthetic transcripts for validation.
    fn write_tlv(buf: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
        buf.extend_from_slice(&tag);
        let len = u32::try_from(payload.len()).expect("ZK1 TLV payload length must fit into a u32");
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(payload);
    }
    #[allow(dead_code)]
    pub fn is_envelope(bytes: &[u8]) -> bool {
        bytes.len() >= 4 && &bytes[..4] == MAGIC
    }
    #[allow(dead_code)]
    pub fn wrap_start() -> Vec<u8> {
        MAGIC.to_vec()
    }
    /// Append a `PROF` TLV (raw transcript bytes) to an envelope buffer.
    #[allow(dead_code)]
    pub fn wrap_append_proof(buf: &mut Vec<u8>, transcript_bytes: &[u8]) {
        write_tlv(buf, *b"PROF", transcript_bytes);
    }
    /// Append an `IPAK` TLV (u32 k) to an envelope buffer.
    #[allow(dead_code)]
    pub fn wrap_append_ipa_k(buf: &mut Vec<u8>, k: u32) {
        let mut tmp = Vec::with_capacity(4);
        tmp.extend_from_slice(&k.to_le_bytes());
        write_tlv(buf, *b"IPAK", &tmp);
    }
    /// Append a circuit identifier (`CID1`) for verifier-key commitment domain separation.
    #[allow(dead_code)]
    pub fn wrap_append_circuit_id(buf: &mut Vec<u8>, circuit_id: &str) {
        write_tlv(buf, *b"CID1", circuit_id.as_bytes());
    }
    /// Parse the one canonical Halo2 IPA verifier-key carrier.
    fn parse_halo2_ipa_vk_envelope(bytes: &[u8]) -> Result<(&str, u32, &[u8]), String> {
        if !is_envelope(bytes) || bytes.len() < 4 {
            return Err("invalid CID1/Halo2 IPA verifier-key envelope".to_owned());
        }
        let mut cursor = Cursor::new(&bytes[4..]);
        let mut circuit_id = None;
        let mut ipa_k = None;
        let mut h2vk = None;
        let mut position = 0_u8;
        while usize::try_from(cursor.position())
            .map_err(|_| "invalid CID1/Halo2 IPA verifier-key envelope".to_owned())?
            < cursor.get_ref().len()
        {
            let Some((tag, payload)) = read_tlv(&mut cursor) else {
                return Err("invalid CID1/Halo2 IPA verifier-key envelope".to_owned());
            };
            match (position, &tag) {
                (0, b"IPAK") => {
                    if payload.len() != 4 {
                        return Err("invalid IPAK payload".to_owned());
                    }
                    ipa_k = Some(u32::from_le_bytes([
                        payload[0], payload[1], payload[2], payload[3],
                    ]));
                }
                (1, b"CID1") => {
                    let value = std::str::from_utf8(payload)
                        .map_err(|_| "invalid CID1 payload".to_owned())?;
                    if !iroha_data_model::zk::open_verify_circuit_id_is_portable(value)
                        || iroha_data_model::zk::open_verify_circuit_id_uses_reserved_privacy_protocol_label_v1(value)
                    {
                        return Err("invalid CID1 payload".to_owned());
                    }
                    circuit_id = Some(value);
                }
                (2, b"H2VK") => {
                    if payload.is_empty() {
                        return Err("empty H2VK payload".to_owned());
                    }
                    h2vk = Some(payload);
                }
                _ => return Err("verifier-key TLVs are not in canonical order".to_owned()),
            }
            position = position.saturating_add(1);
        }
        if position != 3 {
            return Err("verifier-key envelope must contain IPAK, CID1, H2VK".to_owned());
        }
        let circuit_id = circuit_id.ok_or_else(|| "CID1 is missing".to_owned())?;
        let ipa_k = ipa_k.ok_or_else(|| "IPAK is missing".to_owned())?;
        let h2vk = h2vk.ok_or_else(|| "H2VK is missing".to_owned())?;
        Ok((circuit_id, ipa_k, h2vk))
    }
    /// Require a strict Halo2 IPA verifier-key envelope and return its `IPAK`.
    ///
    /// The accepted verifier-key container is exactly `IPAK`, `CID1`, `H2VK`
    /// in that order. This keeps reserved circuit profiles from accepting
    /// arbitrary key bytes or alternate encodings under a matching commitment.
    pub fn ensure_halo2_ipa_vk_envelope_shape_any_k(
        bytes: &[u8],
        expected_circuit_id: &str,
    ) -> Result<u32, String> {
        let (circuit_id, ipa_k, _) = parse_halo2_ipa_vk_envelope(bytes)?;
        if circuit_id != expected_circuit_id {
            return Err(format!(
                "CID1 `{circuit_id}` is not `{expected_circuit_id}`"
            ));
        }
        Ok(ipa_k)
    }
    /// Return the unique Halo2 verifier-key payload from a bounded ZK1 envelope.
    /// Production callers first enforce the strict carrier shape above; the
    /// looser extraction remains available only to in-crate tiny-circuit tests.
    pub fn h2vk_payload(bytes: &[u8]) -> Result<&[u8], String> {
        if !is_envelope(bytes) || bytes.len() < 4 {
            return Err("invalid Halo2 IPA verifier-key envelope".to_owned());
        }
        let mut cursor = Cursor::new(&bytes[4..]);
        let mut h2vk = None;
        while usize::try_from(cursor.position())
            .map_err(|_| "invalid Halo2 IPA verifier-key envelope".to_owned())?
            < cursor.get_ref().len()
        {
            let Some((tag, payload)) = read_tlv(&mut cursor) else {
                return Err("invalid Halo2 IPA verifier-key envelope".to_owned());
            };
            if &tag == b"H2VK" {
                if h2vk.is_some() {
                    return Err("duplicate H2VK payload".to_owned());
                }
                if payload.is_empty() {
                    return Err("empty H2VK payload".to_owned());
                }
                h2vk = Some(payload);
            }
        }
        h2vk.ok_or_else(|| "H2VK is missing".to_owned())
    }
    /// Parse the cheap header carried by Halo2/Axiom processed verifier keys.
    pub fn halo2_pasta_vk_header(payload: &[u8]) -> Result<(u32, bool, u32), String> {
        if payload.len() < HALO2_PASTA_PROCESSED_VK_HEADER_LEN {
            return Err("H2VK payload is too short".to_owned());
        }
        if payload[0] != 0x02 {
            return Err("H2VK payload has unexpected version byte".to_owned());
        }
        let k = u32::from_le_bytes([payload[1], payload[2], payload[3], payload[4]]);
        let compress_selectors = match payload[5] {
            0 => false,
            1 => true,
            _ => return Err("H2VK payload has non-boolean selector compression flag".to_owned()),
        };
        let fixed_columns = u32::from_le_bytes([payload[6], payload[7], payload[8], payload[9]]);
        if fixed_columns == 0 {
            return Err("H2VK payload has no fixed-column commitments".to_owned());
        }
        let fixed_column_commitments_len = usize::try_from(fixed_columns)
            .ok()
            .and_then(|count| count.checked_mul(HALO2_PASTA_PROCESSED_POINT_LEN))
            .ok_or_else(|| "H2VK payload fixed-column commitment length overflow".to_owned())?;
        let min_payload_len = HALO2_PASTA_PROCESSED_VK_HEADER_LEN
            .checked_add(fixed_column_commitments_len)
            .ok_or_else(|| "H2VK payload fixed-column commitment length overflow".to_owned())?;
        if payload.len() < min_payload_len {
            return Err("H2VK payload is truncated before fixed-column commitments".to_owned());
        }
        Ok((k, compress_selectors, fixed_columns))
    }
    /// Append a Halo2 verifying key (`H2VK`) for Pasta/IPA circuits.
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[allow(dead_code)]
    pub fn wrap_append_vk_pasta(buf: &mut Vec<u8>, vk: &super::halo2_backend::VerifyingKey) {
        let bytes = super::halo2_backend::verifying_key_to_processed_bytes(vk);
        write_tlv(buf, *b"H2VK", &bytes);
    }
    /// Append an `I10P` TLV (Pasta Fp instances) to an envelope buffer.
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[allow(dead_code)]
    pub fn wrap_append_instances_pasta_fp(
        instances: &[halo2_proofs::halo2curves::pasta::Fp],
        buf: &mut Vec<u8>,
    ) {
        use ff::PrimeField as _;
        let cols: u32 = 1;
        let rows: u32 =
            u32::try_from(instances.len()).expect("instance row count must fit into a u32");
        let mut payload = Vec::with_capacity(8 + instances.len() * 32);
        payload.extend_from_slice(&cols.to_le_bytes());
        payload.extend_from_slice(&rows.to_le_bytes());
        for s in instances {
            payload.extend_from_slice(s.to_repr().as_ref());
        }
        write_tlv(buf, *b"I10P", &payload);
    }
    /// Append a multi-column `I10P` TLV (Pasta Fp instances) to an envelope buffer.
    ///
    /// The layout matches the reader in `extract_proof_pasta` and
    /// `zkparse::strict_proof_and_instances`:
    ///  - `u32 cols`, `u32 rows`, followed by `rows * cols` canonical 32-byte scalars in
    ///    row-major order (i.e., all column 0 row 0..rows-1, then column 1, etc.).
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[allow(dead_code)]
    pub fn wrap_append_instances_pasta_fp_cols(
        columns: &[&[halo2_proofs::halo2curves::pasta::Fp]],
        buf: &mut Vec<u8>,
    ) {
        use ff::PrimeField as _;
        if columns.is_empty() {
            return;
        }
        let cols: u32 =
            u32::try_from(columns.len()).expect("instance column count must fit into a u32");
        let rows: u32 =
            u32::try_from(columns[0].len()).expect("instance row count must fit into a u32");
        // Require equal row counts across all columns; if not, do nothing (caller error).
        if columns
            .iter()
            .any(|c| u32::try_from(c.len()).ok() != Some(rows))
        {
            return;
        }
        let row_count = usize::try_from(rows).expect("instance row count must fit into usize");
        let col_count = usize::try_from(cols).expect("instance column count must fit into usize");
        let mut payload = Vec::with_capacity(8 + row_count * col_count * 32);
        payload.extend_from_slice(&cols.to_le_bytes());
        payload.extend_from_slice(&rows.to_le_bytes());
        for r in 0..row_count {
            for column in columns.iter().take(col_count) {
                payload.extend_from_slice(column[r].to_repr().as_ref());
            }
        }
        write_tlv(buf, *b"I10P", &payload);
    }
}
#[cfg(test)]
#[path = "zk/zk1_test_helpers.rs"]
/// Test-only helpers for constructing canonical and retired proof carriers.
pub mod zk1_test_helpers;
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
macro_rules! advice {
    (@call $region:ident, $annotation:expr, $column:expr, $offset:expr, $value:expr) => {
        crate::zk::assign_advice_compat(
            &mut $region,
            $annotation,
            $column,
            $offset,
            $value,
        )
    };
    ($region:ident, $label:literal, $column:expr => $value:expr) => {
        advice!(
            @call $region,
            || $label,
            $column,
            0,
            || halo2_proofs::circuit::Value::known($value)
        )
    };
    ($region:ident, $label:literal, $column:expr, $offset:expr => $value:expr) => {
        advice!(
            @call $region,
            || $label,
            $column,
            $offset,
            || halo2_proofs::circuit::Value::known($value)
        )
    };
    ($region:ident, move $label:literal, $column:expr => $value:expr) => {
        advice!(
            @call $region,
            move || format!($label),
            $column,
            0,
            || halo2_proofs::circuit::Value::known($value)
        )
    };
}
#[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
macro_rules! advice_dev {
    ($region:ident, $label:literal, $column:expr => value $value:expr) => {
        advice!(@call $region, || $label, $column, 0, || $value)
    };
    ($region:ident, format $label:literal, $column:expr, $offset:expr => $value:expr) => {
        advice!(
            @call $region,
            || format!($label),
            $column,
            $offset,
            || halo2_proofs::circuit::Value::known($value)
        )
    };
}
// Generic, fixed-depth variants consolidated here to enable easy parameterization
// and future chip-backed swaps under the `zk-halo2-ipa-poseidon` feature flag.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Depth-parameterized example circuits over Halo2 (Pasta).
///
/// These tiny circuits exist solely for internal tests and pre-verifier smoke
/// checks. They are not consensus-critical and are compiled only when Halo2
/// backends are enabled.
pub mod depth {
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    #[allow(unused_imports)]
    use crate::zk::pasta_tiny::poseidon::{Poseidon2ChipWrapper, Pow5Chip, Pow5Config};
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    #[allow(unused_imports)]
    use crate::zk::pasta_tiny::poseidon_compress2_native;
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner},
        halo2curves::pasta::Fp as Scalar,
        plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector},
        poly::Rotation,
    };
    /// Vote-bool commit with a toy Merkle membership chain of fixed depth.
    #[derive(Clone, Default)]
    pub struct VoteBoolCommitMerkle<const DEPTH: usize>;
    impl<const DEPTH: usize> Circuit<Scalar> for VoteBoolCommitMerkle<DEPTH> {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // v
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // rho
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sibs
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w nodes
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        fn configure(
            meta: &mut ConstraintSystem<Scalar>,
        ) -> <VoteBoolCommitMerkle<DEPTH> as Circuit<Scalar>>::Config {
            meta.set_minimum_degree(6);
            let v = meta.advice_column();
            let rho = meta.advice_column();
            let sibs = std::array::from_fn(|_| meta.advice_column());
            let ws = std::array::from_fn(|_| meta.advice_column());
            let inst_cm = meta.instance_column();
            let inst_root = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("vote_commit_merkle_depth", |meta| {
                let s = meta.query_selector(s);
                let vq = meta.query_advice(v, Rotation::cur());
                let rhoq = meta.query_advice(rho, Rotation::cur());
                let cmq = meta.query_instance(inst_cm, Rotation::cur());
                let rootq = meta.query_instance(inst_root, Rotation::cur());
                let constant =
                    |value: u64| halo2_proofs::plonk::Expression::Constant(Scalar::from(value));
                let shift = |expr: halo2_proofs::plonk::Expression<Scalar>, offset: u64| {
                    expr + constant(offset)
                };
                let pow5 = |expr: halo2_proofs::plonk::Expression<Scalar>| {
                    let squared = expr.clone() * expr.clone();
                    let fourth = squared.clone() * squared.clone();
                    fourth * expr
                };
                let pedersen_like =
                    |lhs: halo2_proofs::plonk::Expression<Scalar>,
                     rhs: halo2_proofs::plonk::Expression<Scalar>| {
                        constant(2) * pow5(lhs) + constant(3) * pow5(rhs)
                    };
                let boolc = vq.clone() * (vq.clone() - constant(1));
                let commit_hash = pedersen_like(shift(vq.clone(), 7), shift(rhoq.clone(), 13));
                let commit_diff = commit_hash.clone() - cmq.clone();
                let mut cons = vec![s.clone() * boolc, s.clone() * commit_diff];
                let mut prev = commit_hash;
                for i in 0..DEPTH {
                    let sibling = meta.query_advice(sibs[i], Rotation::cur());
                    let witness = meta.query_advice(ws[i], Rotation::cur());
                    let branch_hash =
                        pedersen_like(shift(prev.clone(), 7), shift(sibling.clone(), 13));
                    cons.push(s.clone() * (witness.clone() - branch_hash));
                    prev = witness;
                }
                cons.push(s * (prev - rootq));
                cons
            });
            (v, rho, sibs, ws, inst_cm, inst_root, s)
        }
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        fn synthesize(
            &self,
            (v, rho, sibs, ws, _inst_cm, _inst_root, s): <VoteBoolCommitMerkle<DEPTH> as Circuit<
                Scalar,
            >>::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let compress = |left: Scalar, right: Scalar| {
                let rc0 = Scalar::from(7);
                let rc1 = Scalar::from(13);
                let two = Scalar::from(2);
                let three = Scalar::from(3);
                let a = left + rc0;
                let b = right + rc1;
                let a2 = a * a;
                let a4 = a2 * a2;
                let a5 = a4 * a;
                let b2 = b * b;
                let b4 = b2 * b2;
                let b5 = b4 * b;
                two * a5 + three * b5
            };
            layouter.assign_region(
                || "vote_commit_merkle_depth",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "v", v => Scalar::from(1))?;
                    advice!(region, "rho", rho => Scalar::from(12345))?;
                    for (i, col) in sibs.iter().enumerate() {
                        advice!(region, move "sib{i}", *col => Scalar::from(20 + i as u64))?;
                    }
                    let mut acc = compress(Scalar::one(), Scalar::from(12345));
                    for (i, col) in ws.iter().enumerate() {
                        let sibling = Scalar::from(20 + i as u64);
                        acc = compress(acc, sibling);
                        #[cfg(debug_assertions)]
                        {
                            println!("vote_merkle witness w{i} = {acc:?}");
                        }
                        advice!(region, move "w{i}", *col => acc)?;
                    }
                    Ok(())
                },
            )
        }
    }
    /// Anonymous transfer (2 inputs, 2 outputs) with commit + Merkle membership.
    #[derive(Clone, Default)]
    pub struct AnonTransfer2x2CommitMerkle<const DEPTH: usize>;
    impl<const DEPTH: usize> Circuit<Scalar> for AnonTransfer2x2CommitMerkle<DEPTH> {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sk
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // serial
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sib_a
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // dir_a
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w_a
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sib_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // dir_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 5], // cm_in0..cm_out1, nf
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        fn configure(
            meta: &mut ConstraintSystem<Scalar>,
        ) -> <AnonTransfer2x2CommitMerkle<DEPTH> as Circuit<Scalar>>::Config {
            let in0 = meta.advice_column();
            let in1 = meta.advice_column();
            let out0 = meta.advice_column();
            let out1 = meta.advice_column();
            let r0 = meta.advice_column();
            let r1 = meta.advice_column();
            let r2 = meta.advice_column();
            let r3 = meta.advice_column();
            let sk = meta.advice_column();
            let serial = meta.advice_column();
            let sib_a = std::array::from_fn(|_| meta.advice_column());
            let dir_a = std::array::from_fn(|_| meta.advice_column());
            let w_a = std::array::from_fn(|_| meta.advice_column());
            let sib_b = std::array::from_fn(|_| meta.advice_column());
            let dir_b = std::array::from_fn(|_| meta.advice_column());
            let w_b = std::array::from_fn(|_| meta.advice_column());
            let cm_cols = [
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
            ];
            let root = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("anon_transfer_commit_merkle_depth", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(in0, Rotation::cur());
                let b = meta.query_advice(in1, Rotation::cur());
                let c = meta.query_advice(out0, Rotation::cur());
                let d = meta.query_advice(out1, Rotation::cur());
                let r0q = meta.query_advice(r0, Rotation::cur());
                let r1q = meta.query_advice(r1, Rotation::cur());
                let r2q = meta.query_advice(r2, Rotation::cur());
                let r3q = meta.query_advice(r3, Rotation::cur());
                let skq = meta.query_advice(sk, Rotation::cur());
                let serq = meta.query_advice(serial, Rotation::cur());
                let cm_in0 = meta.query_instance(cm_cols[0], Rotation::cur());
                let cm_in1 = meta.query_instance(cm_cols[1], Rotation::cur());
                let cm_out0 = meta.query_instance(cm_cols[2], Rotation::cur());
                let cm_out1 = meta.query_instance(cm_cols[3], Rotation::cur());
                let nf = meta.query_instance(cm_cols[4], Rotation::cur());
                let rootq = meta.query_instance(root, Rotation::cur());
                let h = |x: halo2_proofs::plonk::Expression<Scalar>,
                         r: halo2_proofs::plonk::Expression<Scalar>| {
                    let x2 = x.clone() * x.clone();
                    let x4 = x2.clone() * x2.clone();
                    let x5 = x4 * x.clone();
                    let r2 = r.clone() * r.clone();
                    let r4 = r2.clone() * r2.clone();
                    let r5 = r4 * r.clone();
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * x5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * r5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(7))
                };
                // cm constraints and conservation
                let cm0 = h(a.clone(), r0q.clone());
                let cm1 = h(b.clone(), r1q.clone());
                let cm2 = h(c.clone(), r2q.clone());
                let cm3 = h(d.clone(), r3q.clone());
                let nf_exp = h(skq.clone(), serq.clone());
                let mut cons = vec![
                    s.clone() * (a.clone() + b.clone() - (c.clone() + d.clone())),
                    s.clone() * (cm0.clone() - cm_in0),
                    s.clone() * (cm1.clone() - cm_in1),
                    s.clone() * (cm2 - cm_out0),
                    s.clone() * (cm3 - cm_out1),
                    s.clone() * (nf_exp - nf),
                ];
                let constant =
                    |value: u64| halo2_proofs::plonk::Expression::Constant(Scalar::from(value));
                let shift = |expr: halo2_proofs::plonk::Expression<Scalar>, offset: u64| {
                    expr + constant(offset)
                };
                let pow5 = |expr: halo2_proofs::plonk::Expression<Scalar>| {
                    let squared = expr.clone() * expr.clone();
                    let fourth = squared.clone() * squared.clone();
                    fourth * expr
                };
                let pedersen_pair =
                    |lhs: halo2_proofs::plonk::Expression<Scalar>,
                     rhs: halo2_proofs::plonk::Expression<Scalar>| {
                        constant(2) * pow5(lhs) + constant(3) * pow5(rhs)
                    };
                let one = constant(1);
                // membership for cm0
                let mut prev = cm0;
                for i in 0..DEPTH {
                    let sibling = meta.query_advice(sib_a[i], Rotation::cur());
                    let direction_bit = meta.query_advice(dir_a[i], Rotation::cur());
                    let witness = meta.query_advice(w_a[i], Rotation::cur());
                    cons.push(
                        s.clone() * (direction_bit.clone() * (direction_bit.clone() - one.clone())),
                    );
                    let left_branch =
                        pedersen_pair(shift(prev.clone(), 7), shift(sibling.clone(), 13));
                    let right_branch =
                        pedersen_pair(shift(sibling.clone(), 7), shift(prev.clone(), 13));
                    let expected_branch = (one.clone() - direction_bit.clone())
                        * left_branch.clone()
                        + direction_bit.clone() * right_branch;
                    cons.push(s.clone() * (witness.clone() - expected_branch));
                    prev = witness;
                }
                // membership for cm1
                let mut prev_b = cm1;
                for i in 0..DEPTH {
                    let sibling = meta.query_advice(sib_b[i], Rotation::cur());
                    let direction_bit = meta.query_advice(dir_b[i], Rotation::cur());
                    let witness = meta.query_advice(w_b[i], Rotation::cur());
                    cons.push(
                        s.clone() * (direction_bit.clone() * (direction_bit.clone() - one.clone())),
                    );
                    let left_branch =
                        pedersen_pair(shift(prev_b.clone(), 7), shift(sibling.clone(), 13));
                    let right_branch =
                        pedersen_pair(shift(sibling.clone(), 7), shift(prev_b.clone(), 13));
                    let expected_branch = (one.clone() - direction_bit.clone())
                        * left_branch.clone()
                        + direction_bit.clone() * right_branch;
                    cons.push(s.clone() * (witness.clone() - expected_branch));
                    prev_b = witness;
                }
                cons.push(s.clone() * (prev - rootq.clone()));
                cons.push(s * (prev_b - rootq));
                cons
            });
            (
                in0, in1, out0, out1, r0, r1, r2, r3, sk, serial, sib_a, dir_a, w_a, sib_b, dir_b,
                w_b, cm_cols, root, s,
            )
        }
        #[allow(clippy::too_many_lines)]
        fn synthesize(
            &self,
            cfg: <AnonTransfer2x2CommitMerkle<DEPTH> as Circuit<Scalar>>::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let (
                in0,
                in1,
                out0,
                out1,
                r0,
                r1,
                r2,
                r3,
                sk,
                serial,
                sib_a,
                dir_a,
                w_a,
                sib_b,
                dir_b,
                w_b,
                _cm_cols,
                _root,
                s,
            ) = cfg;
            layouter.assign_region(
                || "anon_transfer_commit_merkle_depth",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "in0", in0 => Scalar::from(7))?;
                    advice!(region, "in1", in1 => Scalar::from(5))?;
                    advice!(region, "out0", out0 => Scalar::from(6))?;
                    advice!(region, "out1", out1 => Scalar::from(6))?;
                    advice!(region, "r0", r0 => Scalar::from(11))?;
                    advice!(region, "r1", r1 => Scalar::from(13))?;
                    advice!(region, "r2", r2 => Scalar::from(17))?;
                    advice!(region, "r3", r3 => Scalar::from(19))?;
                    advice!(region, "sk", sk => Scalar::from(1_234_567))?;
                    advice!(region, "serial", serial => Scalar::from(42))?;
                    for (i, col) in sib_a.iter().enumerate() {
                        advice!(region, move "sib_a{i}", *col => Scalar::from(20 + i as u64))?;
                    }
                    for (i, col) in dir_a.iter().enumerate() {
                        advice!(region, move "dir_a{i}", *col => Scalar::from(0))?;
                    }
                    let mut acc = Scalar::from(0);
                    for (i, col) in w_a.iter().enumerate() {
                        acc += Scalar::from(20 + i as u64);
                        advice!(region, move "w_a{i}", *col => acc)?;
                    }
                    for (i, col) in sib_b.iter().enumerate() {
                        advice!(region, move "sib_b{i}", *col => Scalar::from(30 + i as u64))?;
                    }
                    for (i, col) in dir_b.iter().enumerate() {
                        advice!(region, move "dir_b{i}", *col => Scalar::from(0))?;
                    }
                    let mut acc_b = Scalar::from(0);
                    for (i, col) in w_b.iter().enumerate() {
                        acc_b += Scalar::from(30 + i as u64);
                        advice!(region, move "w_b{i}", *col => acc_b)?;
                    }
                    Ok(())
                },
            )
        }
    }
}
// Poseidon-backed depth-param circuits.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
/// Poseidon-like depth-parameterized circuits (Pow5 S-box) for internal tests.
///
/// These circuits mimic Poseidon permutation behaviour with small, fixed
/// round parameters and are used to exercise backends that implement
/// transparent hashing (e.g., IPA over Pasta) in our verifier dispatch.
pub mod poseidon_depth {
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    #[allow(unused_imports)]
    use crate::zk::pasta_tiny::poseidon::{Poseidon2ChipWrapper, Pow5Chip, Pow5Config};
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    #[allow(unused_imports)]
    use crate::zk::pasta_tiny::poseidon_compress2_native;
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner},
        halo2curves::pasta::Fp as Scalar,
        plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector},
        poly::Rotation,
    };
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    // Simple Pow5 helpers (constraint expressions) used as a local gadget when the Poseidon
    // feature is disabled.
    #[inline]
    fn sbox5(
        x: halo2_proofs::plonk::Expression<Scalar>,
    ) -> halo2_proofs::plonk::Expression<Scalar> {
        let x2 = x.clone() * x.clone();
        let x4 = x2.clone() * x2;
        x4 * x
    }
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    fn h2(
        a: halo2_proofs::plonk::Expression<Scalar>,
        b: halo2_proofs::plonk::Expression<Scalar>,
    ) -> halo2_proofs::plonk::Expression<Scalar> {
        // Round constants rc0=7, rc1=13; MDS [[2,3],[3,5]] with full S-box (Pow5)
        let a = a + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
        let b = b + halo2_proofs::plonk::Expression::Constant(Scalar::from(13u64));
        let a5 = sbox5(a);
        let b5 = sbox5(b);
        halo2_proofs::plonk::Expression::Constant(Scalar::from(2u64)) * a5
            + halo2_proofs::plonk::Expression::Constant(Scalar::from(3u64)) * b5
    }
    /// Vote-bool commit with Poseidon-style hashing and fixed-depth membership.
    #[derive(Clone, Default)]
    pub struct VoteBoolCommitMerklePoseidon<const DEPTH: usize>;
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    impl<const DEPTH: usize> Circuit<Scalar> for VoteBoolCommitMerklePoseidon<DEPTH> {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // v
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // rho
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sibs
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // dirs
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w nodes
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let v = meta.advice_column();
            let rho = meta.advice_column();
            let sibs = std::array::from_fn(|_| meta.advice_column());
            let dirs = std::array::from_fn(|_| meta.advice_column());
            let ws = std::array::from_fn(|_| meta.advice_column());
            let inst_cm = meta.instance_column();
            let inst_root = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("vote_commit_merkle_poseidon_depth", |meta| {
                let s = meta.query_selector(s);
                let vq = meta.query_advice(v, Rotation::cur());
                let rhoq = meta.query_advice(rho, Rotation::cur());
                let cmq = meta.query_instance(inst_cm, Rotation::cur());
                let rootq = meta.query_instance(inst_root, Rotation::cur());
                let one = halo2_proofs::plonk::Expression::Constant(Scalar::from(1u64));
                let boolc = vq.clone() * (vq.clone() - one.clone());
                // commit = Pow5 hash of (v,rho)
                let commit = h2(vq, rhoq);
                let mut cons = vec![s.clone() * boolc, s.clone() * (commit.clone() - cmq)];
                // Chain membership with dir-bit mux
                let mut prev = commit;
                for i in 0..DEPTH {
                    let si = meta.query_advice(sibs[i], Rotation::cur());
                    let di = meta.query_advice(dirs[i], Rotation::cur());
                    let wi = meta.query_advice(ws[i], Rotation::cur());
                    cons.push(s.clone() * (di.clone() * (di.clone() - one.clone())));
                    // left = h(prev, sib), right = h(sib, prev)
                    let h_l = h2(prev.clone(), si.clone());
                    let h_r = h2(si, prev.clone());
                    let wi_exp = (one.clone() - di.clone()) * h_l + di * h_r;
                    cons.push(s.clone() * (wi.clone() - wi_exp));
                    prev = wi;
                }
                cons.push(s * (prev - rootq));
                cons
            });
            (v, rho, sibs, dirs, ws, inst_cm, inst_root, s)
        }
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        fn synthesize(
            &self,
            (v, rho, sibs, dirs, ws, _inst_cm, _inst_root, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "vote_commit_merkle_poseidon",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "v", v => Scalar::from(1u64))?;
                    advice!(region, "rho", rho => Scalar::from(12345u64))?;
                    for (i, col) in sibs.iter().enumerate() {
                        advice!(region, move "sib{i}", *col => Scalar::from(20 + i as u64))?;
                    }
                    for (i, col) in dirs.iter().enumerate() {
                        advice!(region, move "dir{i}", *col => Scalar::from(0))?;
                    }
                    let mut acc = Scalar::from(0);
                    for (i, col) in ws.iter().enumerate() {
                        acc += Scalar::from(20 + i as u64);
                        advice!(region, move "w{i}", *col => acc)?;
                    }
                    Ok(())
                },
            )
        }
    }
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    impl<const DEPTH: usize> Circuit<Scalar> for VoteBoolCommitMerklePoseidon<DEPTH> {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // v
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // rho
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // commit_left
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // commit_right
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // commit_hash
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sibs
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // dirs
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w nodes
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // poseidon_left
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // poseidon_right
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
            Selector,
            Pow5Config<Scalar, 3, 2>,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let v = meta.advice_column();
            let rho = meta.advice_column();
            let commit_left = meta.advice_column();
            meta.enable_equality(commit_left);
            let commit_right = meta.advice_column();
            meta.enable_equality(commit_right);
            let commit_hash = meta.advice_column();
            meta.enable_equality(commit_hash);
            let sibs = std::array::from_fn(|_| meta.advice_column());
            let dirs = std::array::from_fn(|_| meta.advice_column());
            let ws = std::array::from_fn(|_| {
                let col = meta.advice_column();
                meta.enable_equality(col);
                col
            });
            let poseidon_left = std::array::from_fn(|_| {
                let col = meta.advice_column();
                meta.enable_equality(col);
                col
            });
            let poseidon_right = std::array::from_fn(|_| {
                let col = meta.advice_column();
                meta.enable_equality(col);
                col
            });
            let inst_cm = meta.instance_column();
            let inst_root = meta.instance_column();
            let s = meta.selector();
            // Gadget config
            let st0 = meta.advice_column();
            let st1 = meta.advice_column();
            let st2 = meta.advice_column();
            let partial = meta.advice_column();
            let rc_a = meta.fixed_column();
            let rc_b = meta.fixed_column();
            let poseidon_cfg = Pow5Chip::configure(meta, [st0, st1, st2], partial, rc_a, rc_b);
            meta.create_gate("vote_commit_merkle_poseidon_depth", |meta| {
                let s = meta.query_selector(s);
                let vq = meta.query_advice(v, Rotation::cur());
                let rhoq = meta.query_advice(rho, Rotation::cur());
                let commit_left_q = meta.query_advice(commit_left, Rotation::cur());
                let commit_right_q = meta.query_advice(commit_right, Rotation::cur());
                let commit_hash_q = meta.query_advice(commit_hash, Rotation::cur());
                let cmq = meta.query_instance(inst_cm, Rotation::cur());
                let rootq = meta.query_instance(inst_root, Rotation::cur());
                let one = halo2_proofs::plonk::Expression::Constant(Scalar::from(1u64));
                let mut constraints = vec![
                    s.clone() * (vq.clone() * (vq.clone() - one.clone())),
                    s.clone() * (commit_left_q.clone() - vq.clone()),
                    s.clone() * (commit_right_q.clone() - rhoq.clone()),
                    s.clone() * (commit_hash_q.clone() - cmq),
                ];
                let mut prev = commit_hash_q;
                for i in 0..DEPTH {
                    let sib = meta.query_advice(sibs[i], Rotation::cur());
                    let dir = meta.query_advice(dirs[i], Rotation::cur());
                    let wi = meta.query_advice(ws[i], Rotation::cur());
                    let left = meta.query_advice(poseidon_left[i], Rotation::cur());
                    let right = meta.query_advice(poseidon_right[i], Rotation::cur());
                    let one_minus_dir = one.clone() - dir.clone();
                    constraints.push(s.clone() * (dir.clone() * (dir.clone() - one.clone())));
                    constraints.push(
                        s.clone()
                            * (left.clone()
                                - (one_minus_dir.clone() * prev.clone()
                                    + dir.clone() * sib.clone())),
                    );
                    constraints.push(
                        s.clone()
                            * (right.clone()
                                - (dir.clone() * prev.clone() + one_minus_dir * sib.clone())),
                    );
                    constraints.push(s.clone() * (wi.clone() - wi.clone()));
                    prev = wi;
                }
                constraints.push(s * (prev - rootq));
                constraints
            });
            (
                v,
                rho,
                commit_left,
                commit_right,
                commit_hash,
                sibs,
                dirs,
                ws,
                poseidon_left,
                poseidon_right,
                inst_cm,
                inst_root,
                s,
                poseidon_cfg,
            )
        }
        #[allow(clippy::too_many_lines)]
        fn synthesize(
            &self,
            (
                v,
                rho,
                commit_left,
                commit_right,
                commit_hash,
                sibs,
                dirs,
                ws,
                poseidon_left,
                poseidon_right,
                _inst_cm,
                _inst_root,
                s,
                poseidon_cfg,
            ): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            use halo2_proofs::circuit::Value;
            let v_val = Scalar::from(1);
            let rho_val = Scalar::from(12345);
            let commit_digest = poseidon_compress2_native(v_val, rho_val);
            let (
                commit_left_cell,
                commit_right_cell,
                commit_hash_cell,
                sib_cells,
                dir_cells,
                w_cells,
                left_cells,
                right_cells,
            ) = layouter.assign_region(
                || "vote_commit_merkle_poseidon_depth",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "v", v => v_val)?;
                    advice!(region, "rho", rho => rho_val)?;
                    let commit_left_cell = advice!(region, "commit_left", commit_left => v_val)?;
                    let commit_right_cell =
                        advice!(region, "commit_right", commit_right => rho_val)?;
                    let commit_hash_cell =
                        advice!(region, "commit_hash", commit_hash => commit_digest)?;
                    let mut sib_cells = Vec::with_capacity(DEPTH);
                    let mut dir_cells = Vec::with_capacity(DEPTH);
                    let mut w_cells = Vec::with_capacity(DEPTH);
                    let mut left_cells = Vec::with_capacity(DEPTH);
                    let mut right_cells = Vec::with_capacity(DEPTH);
                    let mut prev_val = commit_digest;
                    for i in 0..DEPTH {
                        let sib_val = Scalar::from(20 + i as u64);
                        let dir_val = if i % 2 == 0 {
                            Scalar::zero()
                        } else {
                            Scalar::one()
                        };
                        sib_cells.push(advice!(region, move "sib{i}", sibs[i] => sib_val)?);
                        dir_cells.push(advice!(region, move "dir{i}", dirs[i] => dir_val)?);
                        let (left_val, right_val) = if dir_val == Scalar::one() {
                            (sib_val, prev_val)
                        } else {
                            (prev_val, sib_val)
                        };
                        left_cells.push(advice!(
                            region,
                            move "poseidon_left{i}",
                            poseidon_left[i] => left_val
                        )?);
                        right_cells.push(advice!(
                            region,
                            move "poseidon_right{i}",
                            poseidon_right[i] => right_val
                        )?);
                        let digest_val = poseidon_compress2_native(left_val, right_val);
                        w_cells.push(advice!(region, move "w{i}", ws[i] => digest_val)?);
                        prev_val = digest_val;
                    }
                    Ok((
                        commit_left_cell,
                        commit_right_cell,
                        commit_hash_cell,
                        sib_cells,
                        dir_cells,
                        w_cells,
                        left_cells,
                        right_cells,
                    ))
                },
            )?;
            let commit_digest_cells = Poseidon2ChipWrapper::new().hash2_chip(
                &mut layouter,
                &poseidon_cfg,
                Value::known(v_val),
                Value::known(rho_val),
            )?;
            layouter.constrain_equal(commit_digest_cells.left.cell(), commit_left_cell.cell())?;
            layouter.constrain_equal(commit_digest_cells.right.cell(), commit_right_cell.cell())?;
            layouter.constrain_equal(commit_digest_cells.digest.cell(), commit_hash_cell.cell())?;
            let mut prev_val = commit_digest;
            for i in 0..DEPTH {
                let dir_val = dir_cells[i].value().copied().unwrap_or_else(Scalar::zero);
                let sib_val = sib_cells[i].value().copied().unwrap_or_else(Scalar::zero);
                let (left_val, right_val) = if dir_val == Scalar::one() {
                    (sib_val, prev_val)
                } else {
                    (prev_val, sib_val)
                };
                let digest_cells = Poseidon2ChipWrapper::new().hash2_chip(
                    &mut layouter,
                    &poseidon_cfg,
                    Value::known(left_val),
                    Value::known(right_val),
                )?;
                layouter.constrain_equal(digest_cells.left.cell(), left_cells[i].cell())?;
                layouter.constrain_equal(digest_cells.right.cell(), right_cells[i].cell())?;
                layouter.constrain_equal(digest_cells.digest.cell(), w_cells[i].cell())?;
                prev_val = w_cells[i].value().copied().unwrap_or(prev_val);
            }
            Ok(())
        }
    }
    /// Anonymous transfer (2x2) with Poseidon-style commit + membership chain.
    #[derive(Clone, Default)]
    pub struct AnonTransfer2x2CommitMerklePoseidon<const DEPTH: usize>;
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    impl<const DEPTH: usize> Circuit<Scalar> for AnonTransfer2x2CommitMerklePoseidon<DEPTH> {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sk
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // serial
            // membership A
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sib_a
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // dir_a
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w_a
            // membership B
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sib_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // dir_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 5], // cm_in0..cm_out1, nf
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,      // root
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let in0 = meta.advice_column();
            let in1 = meta.advice_column();
            let out0 = meta.advice_column();
            let out1 = meta.advice_column();
            let r0 = meta.advice_column();
            let r1 = meta.advice_column();
            let r2 = meta.advice_column();
            let r3 = meta.advice_column();
            let sk = meta.advice_column();
            let serial = meta.advice_column();
            let sib_a = std::array::from_fn(|_| meta.advice_column());
            let dir_a = std::array::from_fn(|_| meta.advice_column());
            let w_a = std::array::from_fn(|_| meta.advice_column());
            let sib_b = std::array::from_fn(|_| meta.advice_column());
            let dir_b = std::array::from_fn(|_| meta.advice_column());
            let w_b = std::array::from_fn(|_| meta.advice_column());
            let cm_cols = [
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
            ];
            let root = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("anon_transfer_commit_merkle_poseidon_depth", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(in0, Rotation::cur());
                let b = meta.query_advice(in1, Rotation::cur());
                let c = meta.query_advice(out0, Rotation::cur());
                let d = meta.query_advice(out1, Rotation::cur());
                let r0q = meta.query_advice(r0, Rotation::cur());
                let r1q = meta.query_advice(r1, Rotation::cur());
                let r2q = meta.query_advice(r2, Rotation::cur());
                let r3q = meta.query_advice(r3, Rotation::cur());
                let skq = meta.query_advice(sk, Rotation::cur());
                let serq = meta.query_advice(serial, Rotation::cur());
                let cm_in0 = meta.query_instance(cm_cols[0], Rotation::cur());
                let cm_in1 = meta.query_instance(cm_cols[1], Rotation::cur());
                let cm_out0 = meta.query_instance(cm_cols[2], Rotation::cur());
                let cm_out1 = meta.query_instance(cm_cols[3], Rotation::cur());
                let nf = meta.query_instance(cm_cols[4], Rotation::cur());
                let rootq = meta.query_instance(root, Rotation::cur());
                // commit-like h2(x, r)
                let cm0 = h2(a.clone(), r0q.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
                let cm1 = h2(b.clone(), r1q.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
                let cm2 = h2(c.clone(), r2q.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
                let cm3 = h2(d.clone(), r3q.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
                let nf_exp = h2(skq.clone(), serq.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
                let mut cons = vec![
                    s.clone() * (a.clone() + b.clone() - (c.clone() + d.clone())),
                    s.clone() * (cm0.clone() - cm_in0),
                    s.clone() * (cm1.clone() - cm_in1),
                    s.clone() * (cm2 - cm_out0),
                    s.clone() * (cm3 - cm_out1),
                    s.clone() * (nf_exp - nf),
                ];
                let one = halo2_proofs::plonk::Expression::Constant(Scalar::from(1));
                // membership A for cm0
                let mut prev = cm0;
                for i in 0..DEPTH {
                    let si = meta.query_advice(sib_a[i], Rotation::cur());
                    let di = meta.query_advice(dir_a[i], Rotation::cur());
                    let wi = meta.query_advice(w_a[i], Rotation::cur());
                    cons.push(s.clone() * (di.clone() * (di.clone() - one.clone())));
                    let h_l = h2(prev.clone(), si.clone());
                    let h_r = h2(si.clone(), prev.clone());
                    let wi_exp = (one.clone() - di.clone()) * h_l + di.clone() * h_r;
                    cons.push(s.clone() * (wi.clone() - wi_exp));
                    prev = wi;
                }
                // membership B for cm1
                let mut prev_b = cm1;
                for i in 0..DEPTH {
                    let si = meta.query_advice(sib_b[i], Rotation::cur());
                    let di = meta.query_advice(dir_b[i], Rotation::cur());
                    let wi = meta.query_advice(w_b[i], Rotation::cur());
                    cons.push(s.clone() * (di.clone() * (di.clone() - one.clone())));
                    let h_l = h2(prev_b.clone(), si.clone());
                    let h_r = h2(si.clone(), prev_b.clone());
                    let wi_exp = (one.clone() - di.clone()) * h_l + di.clone() * h_r;
                    cons.push(s.clone() * (wi.clone() - wi_exp));
                    prev_b = wi;
                }
                cons.push(s.clone() * (prev - rootq.clone()));
                cons.push(s * (prev_b - rootq));
                cons
            });
            (
                in0, in1, out0, out1, r0, r1, r2, r3, sk, serial, sib_a, dir_a, w_a, sib_b, dir_b,
                w_b, cm_cols, root, s,
            )
        }
        #[allow(clippy::too_many_lines)]
        #[allow(clippy::too_many_lines)]
        fn synthesize(
            &self,
            cfg: Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let (
                in0,
                in1,
                out0,
                out1,
                r0,
                r1,
                r2,
                r3,
                sk,
                serial,
                sib_a,
                dir_a,
                w_a,
                sib_b,
                dir_b,
                w_b,
                _cm_cols,
                _root,
                s,
            ) = cfg;
            layouter.assign_region(
                || "anon_transfer_commit_merkle_poseidon",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "in0", in0 => Scalar::from(7))?;
                    advice!(region, "in1", in1 => Scalar::from(5))?;
                    advice!(region, "out0", out0 => Scalar::from(6))?;
                    advice!(region, "out1", out1 => Scalar::from(6))?;
                    advice!(region, "r0", r0 => Scalar::from(11))?;
                    advice!(region, "r1", r1 => Scalar::from(13))?;
                    advice!(region, "r2", r2 => Scalar::from(17))?;
                    advice!(region, "r3", r3 => Scalar::from(19))?;
                    advice!(region, "sk", sk => Scalar::from(1_234_567))?;
                    advice!(region, "serial", serial => Scalar::from(42))?;
                    for (i, col) in sib_a.iter().enumerate() {
                        advice!(region, move "sib_a{i}", *col => Scalar::from(20 + i as u64))?;
                    }
                    for (i, col) in dir_a.iter().enumerate() {
                        advice!(region, move "dir_a{i}", *col => Scalar::from(0))?;
                    }
                    let mut acc = Scalar::from(0);
                    for (i, col) in w_a.iter().enumerate() {
                        acc += Scalar::from(20 + i as u64);
                        advice!(region, move "w_a{i}", *col => acc)?;
                    }
                    for (i, col) in sib_b.iter().enumerate() {
                        advice!(region, move "sib_b{i}", *col => Scalar::from(30 + i as u64))?;
                    }
                    for (i, col) in dir_b.iter().enumerate() {
                        advice!(region, move "dir_b{i}", *col => Scalar::from(0))?;
                    }
                    let mut acc_b = Scalar::from(0);
                    for (i, col) in w_b.iter().enumerate() {
                        acc_b += Scalar::from(30 + i as u64);
                        advice!(region, move "w_b{i}", *col => acc_b)?;
                    }
                    Ok(())
                },
            )
        }
    }
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    impl<const DEPTH: usize> Circuit<Scalar> for AnonTransfer2x2CommitMerklePoseidon<DEPTH> {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sk
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // serial
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sib_a
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // dir_a
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w_a
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // sib_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // dir_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; DEPTH], // w_b
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 5], // cm_in0..cm_out1, nf
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
            Selector,
            Pow5Config<Scalar, 3, 2>,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let in0 = meta.advice_column();
            let in1 = meta.advice_column();
            let out0 = meta.advice_column();
            let out1 = meta.advice_column();
            let r0 = meta.advice_column();
            let r1 = meta.advice_column();
            let r2 = meta.advice_column();
            let r3 = meta.advice_column();
            let sk = meta.advice_column();
            let serial = meta.advice_column();
            let sib_a = std::array::from_fn(|_| meta.advice_column());
            let dir_a = std::array::from_fn(|_| meta.advice_column());
            let w_a = std::array::from_fn(|_| meta.advice_column());
            let sib_b = std::array::from_fn(|_| meta.advice_column());
            let dir_b = std::array::from_fn(|_| meta.advice_column());
            let w_b = std::array::from_fn(|_| meta.advice_column());
            for w in &w_a {
                meta.enable_equality(*w);
            }
            for w in &w_b {
                meta.enable_equality(*w);
            }
            let cm_cols = [
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
            ];
            let root = meta.instance_column();
            let s = meta.selector();
            // Poseidon chip config
            let st0 = meta.advice_column();
            let st1 = meta.advice_column();
            let st2 = meta.advice_column();
            let partial = meta.advice_column();
            let rc_a = meta.fixed_column();
            let rc_b = meta.fixed_column();
            let poseidon_cfg = Pow5Chip::configure(meta, [st0, st1, st2], partial, rc_a, rc_b);
            // Constraints unchanged
            meta.create_gate("anon_transfer_commit_merkle_poseidon_depth", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(in0, Rotation::cur());
                let b = meta.query_advice(in1, Rotation::cur());
                let c = meta.query_advice(out0, Rotation::cur());
                let d = meta.query_advice(out1, Rotation::cur());
                let r0q = meta.query_advice(r0, Rotation::cur());
                let r1q = meta.query_advice(r1, Rotation::cur());
                let r2q = meta.query_advice(r2, Rotation::cur());
                let r3q = meta.query_advice(r3, Rotation::cur());
                let skq = meta.query_advice(sk, Rotation::cur());
                let serq = meta.query_advice(serial, Rotation::cur());
                let cm_in0 = meta.query_instance(cm_cols[0], Rotation::cur());
                let cm_in1 = meta.query_instance(cm_cols[1], Rotation::cur());
                let cm_out0 = meta.query_instance(cm_cols[2], Rotation::cur());
                let cm_out1 = meta.query_instance(cm_cols[3], Rotation::cur());
                let nf = meta.query_instance(cm_cols[4], Rotation::cur());
                let rootq = meta.query_instance(root, Rotation::cur());
                let cm0 = h2(a.clone(), r0q.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7));
                let cm1 = h2(b.clone(), r1q.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7));
                let cm2 = h2(c.clone(), r2q.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7));
                let cm3 = h2(d.clone(), r3q.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7));
                let nf_exp = h2(skq.clone(), serq.clone())
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7));
                let mut cons = vec![
                    s.clone() * (a.clone() + b.clone() - (c.clone() + d.clone())),
                    s.clone() * (cm0.clone() - cm_in0),
                    s.clone() * (cm1.clone() - cm_in1),
                    s.clone() * (cm2 - cm_out0),
                    s.clone() * (cm3 - cm_out1),
                    s.clone() * (nf_exp - nf),
                ];
                let one = halo2_proofs::plonk::Expression::Constant(Scalar::from(1));
                let mut prev = cm0;
                for i in 0..DEPTH {
                    let si = meta.query_advice(sib_a[i], Rotation::cur());
                    let di = meta.query_advice(dir_a[i], Rotation::cur());
                    let wi = meta.query_advice(w_a[i], Rotation::cur());
                    cons.push(s.clone() * (di.clone() * (di.clone() - one.clone())));
                    let h_l = h2(prev.clone(), si.clone());
                    let h_r = h2(si.clone(), prev.clone());
                    let wi_exp = (one.clone() - di.clone()) * h_l + di.clone() * h_r;
                    cons.push(s.clone() * (wi.clone() - wi_exp));
                    prev = wi;
                }
                let mut prev_b = cm1;
                for i in 0..DEPTH {
                    let si = meta.query_advice(sib_b[i], Rotation::cur());
                    let di = meta.query_advice(dir_b[i], Rotation::cur());
                    let wi = meta.query_advice(w_b[i], Rotation::cur());
                    cons.push(s.clone() * (di.clone() * (di.clone() - one.clone())));
                    let h_l = h2(prev_b.clone(), si.clone());
                    let h_r = h2(si.clone(), prev_b.clone());
                    let wi_exp = (one.clone() - di.clone()) * h_l + di.clone() * h_r;
                    cons.push(s.clone() * (wi.clone() - wi_exp));
                    prev_b = wi;
                }
                cons.push(s.clone() * (prev - rootq.clone()));
                cons.push(s * (prev_b - rootq));
                cons
            });
            (
                in0,
                in1,
                out0,
                out1,
                r0,
                r1,
                r2,
                r3,
                sk,
                serial,
                sib_a,
                dir_a,
                w_a,
                sib_b,
                dir_b,
                w_b,
                cm_cols,
                root,
                s,
                poseidon_cfg,
            )
        }
        fn synthesize(
            &self,
            cfg: Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            use halo2_proofs::circuit::Value;
            let (
                in0,
                in1,
                out0,
                out1,
                r0,
                r1,
                r2,
                r3,
                sk,
                serial,
                sib_a,
                dir_a,
                w_a,
                sib_b,
                dir_b,
                w_b,
                _cm_cols,
                _root,
                s,
                poseidon_cfg,
            ) = cfg;
            layouter.assign_region(
                || "anon_transfer_commit_merkle_poseidon",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "in0", in0 => Scalar::from(7))?;
                    advice!(region, "in1", in1 => Scalar::from(5))?;
                    advice!(region, "out0", out0 => Scalar::from(6))?;
                    advice!(region, "out1", out1 => Scalar::from(6))?;
                    advice!(region, "r0", r0 => Scalar::from(11))?;
                    advice!(region, "r1", r1 => Scalar::from(13))?;
                    advice!(region, "r2", r2 => Scalar::from(17))?;
                    advice!(region, "r3", r3 => Scalar::from(19))?;
                    advice!(region, "sk", sk => Scalar::from(1_234_567))?;
                    advice!(region, "serial", serial => Scalar::from(42))?;
                    for (i, col) in sib_a.iter().enumerate() {
                        advice!(region, move "sib_a{i}", *col => Scalar::from(20 + i as u64))?;
                    }
                    for (i, col) in dir_a.iter().enumerate() {
                        advice!(region, move "dir_a{i}", *col => Scalar::from(0))?;
                    }
                    for (i, col) in sib_b.iter().enumerate() {
                        advice!(region, move "sib_b{i}", *col => Scalar::from(30 + i as u64))?;
                    }
                    for (i, col) in dir_b.iter().enumerate() {
                        advice!(region, move "dir_b{i}", *col => Scalar::from(0))?;
                    }
                    Ok(())
                },
            )?;
            // Use concrete witness values assigned above
            let in0v = Scalar::from(7);
            let in1v = Scalar::from(5);
            let out0v = Scalar::from(6);
            let out1v = Scalar::from(6);
            let r0v = Scalar::from(11);
            let r1v = Scalar::from(13);
            let r2v = Scalar::from(17);
            let r3v = Scalar::from(19);
            // Commit-like hashes (+7 offset) for cm and nf
            let cm0 = poseidon_compress2_native(in0v, r0v) + Scalar::from(7);
            let cm1 = poseidon_compress2_native(in1v, r1v) + Scalar::from(7);
            let _cm2 = poseidon_compress2_native(out0v, r2v) + Scalar::from(7);
            let _cm3 = poseidon_compress2_native(out1v, r3v) + Scalar::from(7);
            // Chain A via gadget
            let mut prev_a = cm0;
            for (i, w_col) in w_a.iter().enumerate() {
                let sib_val = Scalar::from(20 + i as u64);
                let hash_cells = Poseidon2ChipWrapper::new().hash2_chip(
                    &mut layouter,
                    &poseidon_cfg,
                    Value::known(prev_a),
                    Value::known(sib_val),
                )?;
                let digest = hash_cells.digest;
                let w_val = poseidon_compress2_native(prev_a, sib_val);
                let w_cell = layouter.assign_region(
                    || format!("w_a_{i}"),
                    |mut region| advice!(region, "w_a", *w_col => w_val),
                )?;
                layouter.constrain_equal(digest.cell(), w_cell.cell())?;
                prev_a = w_val;
            }
            // Chain B via gadget
            let mut prev_b = cm1;
            for (i, w_col) in w_b.iter().enumerate() {
                let sib_val = Scalar::from(30 + i as u64);
                let hash_cells = Poseidon2ChipWrapper::new().hash2_chip(
                    &mut layouter,
                    &poseidon_cfg,
                    Value::known(prev_b),
                    Value::known(sib_val),
                )?;
                let digest = hash_cells.digest;
                let w_val = poseidon_compress2_native(prev_b, sib_val);
                let w_cell = layouter.assign_region(
                    || format!("w_b_{i}"),
                    |mut region| advice!(region, "w_b", *w_col => w_val),
                )?;
                layouter.constrain_equal(digest.cell(), w_cell.cell())?;
                prev_b = w_val;
            }
            Ok(())
        }
    }
}
/// Batch-local deduplication cache keyed by proof hash.
#[derive(Default)]
pub struct DedupCache {
    seen: BTreeSet<[u8; 32]>,
}
#[cfg(feature = "zk-preverify")]
const TRACE_DIGEST_BACKEND: &str = "zk-trace/digest";
#[cfg(feature = "zk-preverify")]
static TRACE_PROOF_QUEUE: OnceLock<Mutex<BTreeMap<u64, Vec<PipelineProofSnapshot>>>> =
    OnceLock::new();
#[cfg(feature = "zk-preverify")]
fn trace_proof_queue() -> &'static Mutex<BTreeMap<u64, Vec<PipelineProofSnapshot>>> {
    TRACE_PROOF_QUEUE.get_or_init(|| Mutex::new(BTreeMap::new()))
}
/// Construct a trace-proof snapshot representing a verified IVM trace digest.
#[cfg(feature = "zk-preverify")]
pub fn make_trace_digest_artifact(
    code_hash: [u8; 32],
    tx_hash: Option<&iroha_crypto::Hash>,
    digest: [u8; 32],
) -> PipelineProofSnapshot {
    let tx_hash_bytes = tx_hash.map(|hash| {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(hash.as_ref());
        arr
    });
    PipelineProofSnapshot {
        backend: TRACE_DIGEST_BACKEND.to_string(),
        proof: digest.to_vec(),
        code_hash,
        tx_hash: tx_hash_bytes,
    }
}
#[cfg(feature = "zk-preverify")]
const TRACE_QUEUE_MAX_SPINS: usize = 20;
#[cfg(feature = "zk-preverify")]
const TRACE_QUEUE_SLEEP_MS: u64 = 10;
#[cfg(feature = "zk-preverify")]
/// Captured trace metadata awaiting background validation and future proof generation.
#[derive(Clone)]
pub struct TraceForProving {
    digest: [u8; 32],
    program: Arc<[u8]>,
    trace: Vec<ivm::zk::RegisterState>,
    constraints: Vec<ivm::zk::Constraint>,
    code_hash: [u8; 32],
    tx_hash: Option<[u8; 32]>,
}
#[cfg(feature = "zk-preverify")]
impl TraceForProving {
    /// Construct a proving job from a verified ZK lane task.
    pub fn from_task(task: &crate::pipeline::zk_lane::ZkTask, digest: [u8; 32]) -> Self {
        Self {
            digest,
            program: Arc::clone(&task.program),
            trace: task.trace.clone(),
            constraints: task.constraints.clone(),
            code_hash: task.code_hash,
            tx_hash: task.tx_hash.as_ref().map(|hash| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(hash.as_ref());
                arr
            }),
        }
    }
    fn validate(&self) -> Result<(), String> {
        VMExecutionCircuit::new(self.program.as_ref(), &self.trace, &self.constraints)
            .verify()
            .map_err(|err| err.to_string())
    }
}
#[cfg(feature = "zk-preverify")]
static TRACE_PROVING_QUEUE: OnceLock<Mutex<BTreeMap<u64, Vec<TraceForProving>>>> = OnceLock::new();
#[cfg(feature = "zk-preverify")]
fn trace_proving_queue() -> &'static Mutex<BTreeMap<u64, Vec<TraceForProving>>> {
    TRACE_PROVING_QUEUE.get_or_init(|| Mutex::new(BTreeMap::new()))
}
#[cfg(feature = "zk-preverify")]
/// Persist a trace-validation job until the background lane receives the matching block header.
pub fn queue_trace_for_proving(height: u64, job: TraceForProving) {
    let mut guard = trace_proving_queue()
        .lock()
        .expect("trace proving queue poisoned");
    guard.entry(height).or_default().push(job);
}
#[cfg(feature = "zk-preverify")]
fn try_take_traces_for_height(height: u64) -> Option<Vec<TraceForProving>> {
    let mut guard = trace_proving_queue()
        .lock()
        .expect("trace proving queue poisoned");
    guard.remove(&height)
}
#[cfg(feature = "zk-preverify")]
/// Attempt to drain all proving jobs queued for `height`, waiting briefly for in-flight verifiers.
pub fn collect_traces_for_proving(height: u64) -> Vec<TraceForProving> {
    for attempt in 0..TRACE_QUEUE_MAX_SPINS {
        if let Some(entries) = try_take_traces_for_height(height) {
            return entries;
        }
        if attempt + 1 == TRACE_QUEUE_MAX_SPINS {
            break;
        }
        std::thread::sleep(Duration::from_millis(TRACE_QUEUE_SLEEP_MS));
    }
    Vec::new()
}
/// Record a verified trace proof artifact for a block height.
#[cfg(feature = "zk-preverify")]
pub fn queue_trace_proof(height: u64, artifact: PipelineProofSnapshot) {
    let mut guard = trace_proof_queue()
        .lock()
        .expect("trace proof queue poisoned");
    guard.entry(height).or_default().push(artifact);
}
/// Drain all queued trace proof artifacts for the given block height.
#[cfg(feature = "zk-preverify")]
pub fn collect_trace_proofs_for_height(height: u64) -> Vec<PipelineProofSnapshot> {
    const MAX_SPINS: usize = 20;
    const SLEEP_MS: u64 = 10;
    for attempt in 0..MAX_SPINS {
        if let Some(proofs) = {
            let mut guard = trace_proof_queue()
                .lock()
                .expect("trace proof queue poisoned");
            guard.remove(&height)
        } {
            return proofs;
        }
        if attempt + 1 == MAX_SPINS {
            break;
        }
        std::thread::sleep(Duration::from_millis(SLEEP_MS));
    }
    Vec::new()
}
/// Clear the trace proof queue (test helper).
#[cfg(all(test, feature = "zk-preverify"))]
pub(crate) fn reset_trace_proof_state_for_tests() {
    if let Some(lock) = TRACE_PROOF_QUEUE.get() {
        let mut guard = lock.lock().expect("trace proof queue poisoned");
        guard.clear();
    }
}
#[cfg(all(test, feature = "zk-preverify"))]
pub(crate) fn reset_trace_proving_state_for_tests() {
    if let Some(lock) = TRACE_PROVING_QUEUE.get() {
        let mut guard = lock.lock().expect("trace proving queue poisoned");
        guard.clear();
    }
}
#[cfg(feature = "zk-preverify")]
static ZK_SENDER: OnceLock<mpsc::Sender<iroha_data_model::block::BlockHeader>> = OnceLock::new();
/// Start the background ZK trace lane that revalidates queued traces.
#[cfg(feature = "zk-preverify")]
pub fn start_lane() {
    if ZK_SENDER.get().is_some() {
        return;
    }
    let (tx, mut rx) = mpsc::channel::<iroha_data_model::block::BlockHeader>(128);
    let _ = ZK_SENDER.set(tx);
    tokio::spawn(async move {
        while let Some(header) = rx.recv().await {
            let height = header.height().get();
            let entries = {
                let mut attempt = 0usize;
                loop {
                    if let Some(entries) = try_take_traces_for_height(height) {
                        break entries;
                    }
                    if attempt >= TRACE_QUEUE_MAX_SPINS {
                        break Vec::new();
                    }
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(TRACE_QUEUE_SLEEP_MS)).await;
                }
            };
            if entries.is_empty() {
                iroha_logger::debug!(height, "zk_lane: no verified traces queued for block");
                continue;
            }
            for entry in entries {
                match entry.validate() {
                    Ok(()) => {
                        let digest = entry.digest;
                        let trace_len = entry.trace.len();
                        let constraint_len = entry.constraints.len();
                        let code_hash_hex = hex::encode(entry.code_hash);
                        let tx_hash_hex = entry
                            .tx_hash
                            .map(|bytes| hex::encode(bytes))
                            .unwrap_or_else(|| "none".to_string());
                        iroha_logger::info!(
                            height,
                            %code_hash_hex,
                            %tx_hash_hex,
                            digest = %hex::encode(digest),
                            trace_len,
                            constraint_len,
                            "zk_lane: validated queued block trace"
                        );
                    }
                    Err(err) => {
                        iroha_logger::warn!(
                            height,
                            error = err,
                            "zk_lane: failed to revalidate queued block trace"
                        );
                    }
                }
            }
        }
    });
}
/// Enqueue a block header for background proving. No-op if the lane is not started.
#[cfg(feature = "zk-preverify")]
pub fn enqueue_block_for_proving(header: &iroha_data_model::block::BlockHeader) {
    if let Some(tx) = ZK_SENDER.get() {
        let _ = tx.try_send(header.clone());
    }
}
// Future work (zk-lane): implement real proving over IVM traces and attach proofs to
// blocks non-consensus-critically. Configuration knobs and end-to-end tests for the
// native verifiers will ship alongside that feature.
impl DedupCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            seen: BTreeSet::new(),
        }
    }
}
#[cfg(all(
    test,
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
use halo2_proofs::transcript::TranscriptWriterBuffer;
#[cfg(all(
    test,
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
use rand_core_06::OsRng;
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_with_instance_noncanonical_ipa() {
    // Generate a valid proof, then wrap a non-canonical instance scalar in ZK1.
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner},
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{
            Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
        },
        poly::Rotation,
        transcript::{Blake2bWrite, Challenge255},
    };
    #[derive(Clone, Default)]
    struct TinyAddPublic;
    impl Circuit<Scalar> for TinyAddPublic {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
            halo2_proofs::plonk::Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let inst = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("add_pub", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                let pubv = meta.query_instance(inst, Rotation::cur());
                vec![s.clone() * (a + b - c.clone()), s * (c - pubv)]
            });
            (a, b, c, inst, s)
        }
        fn synthesize(
            &self,
            (a, b, c, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "tiny_pub",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "a", a => Scalar::from(2))?;
                    advice!(region, "b", b => Scalar::from(2))?;
                    advice!(region, "c", c => Scalar::from(4))?;
                    Ok(())
                },
            )
        }
    }
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &TinyAddPublic::default()).expect("vk");
    let pk = keygen_pk(&params, vk_h2.clone(), &TinyAddPublic::default()).expect("pk");
    let inst_col = vec![Scalar::from(4u64)];
    let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
    let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[TinyAddPublic::default()],
        &inst_proofs,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    let mut vk_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_ipa_k(&mut vk_env, k);
    crate::zk::zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    let mut prf_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
    let mut payload = Vec::with_capacity(8 + 32);
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&1u32.to_le_bytes());
    payload.extend_from_slice(&[0xFFu8; 32]);
    prf_env.extend_from_slice(b"I10P");
    prf_env.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    prf_env.extend_from_slice(&payload);
    let backend = "halo2/pasta/ipa/tiny-add-public";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(!verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn ipa_vote_bool_commit_zk1() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    // Build circuit and params
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::VoteBoolCommit::default()).expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &pasta_tiny::VoteBoolCommit::default(),
    )
    .expect("pk");
    // Compute expected commit (same toy hash as in circuit)
    let v = Scalar::from(1u64);
    let rho = Scalar::from(12345u64);
    let commit = {
        let v2 = v * v;
        let v4 = v2 * v2;
        let v5 = v4 * v;
        let r2 = rho * rho;
        let r4 = r2 * r2;
        let r5 = r4 * rho;
        let t0 = Scalar::from(2) * v5 + Scalar::from(3) * r5 + Scalar::from(7);
        let t1 = v + Scalar::from(13);
        let t12 = t1 * t1;
        let t14 = t12 * t12;
        let t15 = t14 * t1; // t1^5
        Scalar::from(3) * t0 + Scalar::from(5) * t15 + Scalar::from(11)
    };
    // Create proof with public instance [commit]
    let inst_col = vec![commit];
    let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
    let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[pasta_tiny::VoteBoolCommit::default()],
        &inst_proofs,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    // Build ZK1 envelopes and verify via backend
    let mut vk_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_ipa_k(&mut vk_env, k);
    crate::zk::zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    let mut prf_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
    crate::zk::zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut prf_env);
    let backend = "halo2/pasta/ipa/vote-bool-commit";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_rejects_vk_without_bytes() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::VoteBoolCommit::default()).expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &pasta_tiny::VoteBoolCommit::default(),
    )
    .expect("pk");
    // Build deterministic commit identical to circuit synthesize logic
    let v = Scalar::from(1u64);
    let rho = Scalar::from(12345u64);
    let commit = {
        let v2 = v * v;
        let v4 = v2 * v2;
        let v5 = v4 * v;
        let r2 = rho * rho;
        let r4 = r2 * r2;
        let r5 = r4 * rho;
        let t0 = Scalar::from(2) * v5 + Scalar::from(3) * r5 + Scalar::from(7);
        let t1 = v + Scalar::from(13);
        let t12 = t1 * t1;
        let t14 = t12 * t12;
        let t15 = t14 * t1;
        Scalar::from(3) * t0 + Scalar::from(5) * t15 + Scalar::from(11)
    };
    let inst_col = vec![commit];
    let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
    let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[pasta_tiny::VoteBoolCommit::default()],
        &inst_proofs,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    let backend = "halo2/pasta/ipa/vote-bool-commit";
    let mut vk_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_ipa_k(&mut vk_env, k);
    crate::zk::zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    let mut prf_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
    crate::zk::zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut prf_env);
    let vk_box_good = VerifyingKeyBox::new(backend.into(), vk_env.clone());
    let prf_box = ProofBox::new(backend.into(), prf_env.clone());
    assert!(verify_halo2_ipa(backend, &prf_box, Some(&vk_box_good)));
    // Create VK envelope lacking the H2VK TLV — verification must fail.
    let mut vk_env_missing = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_ipa_k(&mut vk_env_missing, k);
    let vk_box_missing = VerifyingKeyBox::new(backend.into(), vk_env_missing);
    assert!(!verify_halo2_ipa(backend, &prf_box, Some(&vk_box_missing)));
    // Tamper with the VK bytes while keeping the TLV present → hash mismatch → reject.
    let mut vk_tampered = vk_env;
    if let Some(last) = vk_tampered.last_mut() {
        *last ^= 0xAA;
    }
    let vk_box_tampered = VerifyingKeyBox::new(backend.into(), vk_tampered);
    assert!(!verify_halo2_ipa(backend, &prf_box, Some(&vk_box_tampered)));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn ipa_anon_transfer_commit_zk1() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    let k = 5u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::AnonTransfer2x2Commit::default()).expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &pasta_tiny::AnonTransfer2x2Commit::default(),
    )
    .expect("pk");
    // Compute commitments externally using the same Pow5 pair hash as the circuit.
    let in0 = Scalar::from(7u64);
    let rin0 = Scalar::from(11u64);
    let in1 = Scalar::from(5u64);
    let rin1 = Scalar::from(13u64);
    let out0 = Scalar::from(6u64);
    let rout0 = Scalar::from(17u64);
    let out1 = Scalar::from(6u64);
    let rout1 = Scalar::from(19u64);
    let sk = Scalar::from(1_234_567u64);
    let serial = Scalar::from(42u64);
    let h = |a: Scalar, r: Scalar| {
        let a = a + Scalar::from(7u64);
        let r = r + Scalar::from(13u64);
        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let r2 = r * r;
        let r4 = r2 * r2;
        let r5 = r4 * r;
        Scalar::from(2) * a5 + Scalar::from(3) * r5
    };
    let cm_in0 = h(in0, rin0);
    let cm_in1 = h(in1, rin1);
    let cm_out0 = h(out0, rout0);
    let cm_out1 = h(out1, rout1);
    let nullifier = h(sk, serial);
    let col0 = vec![cm_in0];
    let col1 = vec![cm_in1];
    let col2 = vec![cm_out0];
    let col3 = vec![cm_out1];
    let col4 = vec![nullifier];
    let inst_cols: Vec<&[Scalar]> = vec![&col0, &col1, &col2, &col3, &col4];
    let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[pasta_tiny::AnonTransfer2x2Commit::default()],
        &inst_proofs,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    let mut vk_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_ipa_k(&mut vk_env, k);
    crate::zk::zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    let mut prf_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
    // Pack instances as a single I10P with 5 columns * 1 row in ZK1
    let cols: [&[Scalar]; 5] = [
        col0.as_slice(),
        col1.as_slice(),
        col2.as_slice(),
        col3.as_slice(),
        col4.as_slice(),
    ];
    crate::zk::zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut prf_env);
    let backend = "halo2/pasta/ipa/anon-transfer-2x2";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}
#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn ipa_vote_bool_commit_merkle2_zk1() {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{VerifyingKey, keygen_pk, keygen_vk},
        transcript::{Blake2bWrite, Challenge255},
    };
    let k = 6u32;
    let params: PastaParams = pasta_params_new(k);
    let vk_h2: VerifyingKey<Curve> =
        keygen_vk(&params, &pasta_tiny::VoteBoolCommitMerkle2::default()).expect("vk");
    let pk = keygen_pk(
        &params,
        vk_h2.clone(),
        &pasta_tiny::VoteBoolCommitMerkle2::default(),
    )
    .expect("pk");
    // Compute commit and Merkle root using the same fallback Pow5 pair hash as the circuit.
    let v = Scalar::from(1u64);
    let rho = Scalar::from(12345u64);
    let commit = pasta_tiny::poseidon_pair(v, rho);
    let sib0 = Scalar::from(5u64);
    let sib1 = Scalar::from(7u64);
    // w0 = h(commit, sib0), w1 = h(w0, sib1)
    let w0 = pasta_tiny::poseidon_pair(commit, sib0);
    let root = pasta_tiny::poseidon_pair(w0, sib1);
    let col0 = vec![commit];
    let col1 = vec![root];
    let inst_cols: Vec<&[Scalar]> = vec![col0.as_slice(), col1.as_slice()];
    let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];
    // Make proof with public instance columns [commit], [root].
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    halo2_proofs::plonk::create_proof::<
        IPACommitmentScheme<Curve>,
        ProverIPA<'_, Curve>,
        Challenge255<Curve>,
        _,
        _,
        _,
    >(
        &params,
        &pk,
        &[pasta_tiny::VoteBoolCommitMerkle2::default()],
        &inst_proofs,
        OsRng,
        &mut transcript,
    )
    .expect("proof created");
    let proof_bytes = transcript.finalize();
    // Wrap as ZK1: IPAK + PROF + I10P(2 cols, 1 row)
    let mut vk_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_ipa_k(&mut vk_env, k);
    crate::zk::zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
    let mut prf_env = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
    let cols: [&[Scalar]; 2] = [col0.as_slice(), col1.as_slice()];
    crate::zk::zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut prf_env);
    let backend = "halo2/pasta/ipa/vote-bool-commit-merkle2";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(verify_halo2_ipa(backend, &prf_box, Some(&vk_box)));
}
impl DedupCache {
    /// Return true if this proof is new to the cache and insert it; false if duplicate.
    pub fn check_and_insert(&mut self, proof: &ProofBox) -> bool {
        self.seen.insert(hash_proof(proof))
    }
    /// Compute and insert a combined dedup key from the proof and optional vk commitment.
    /// Returns true if not seen before.
    pub fn check_and_insert_with_commitment(
        &mut self,
        proof: &ProofBox,
        vk_commitment: Option<[u8; 32]>,
    ) -> bool {
        let mut h = Sha256::new();
        h.update(b"iroha:zk:v1:preverify-dedup");
        h.update(hash_proof(proof));
        if let Some(c) = vk_commitment {
            h.update(c);
        }
        let key: [u8; 32] = h.finalize().into();
        self.seen.insert(key)
    }
}
fn expected_preverify_envelope_backend_tag(
    backend: &str,
) -> Option<iroha_data_model::zk::BackendTag> {
    production_verify_backend_tag(backend)
}
fn preverify_open_verify_envelope_metadata(
    proof: &ProofBox,
    vk: Option<&VerifyingKeyBox>,
    vk_commitment: Option<[u8; 32]>,
    expected_vk_commitment: Option<[u8; 32]>,
) -> Result<(), PreverifyResult> {
    let Some(expected_tag) = expected_preverify_envelope_backend_tag(proof.backend.as_str()) else {
        return Ok(());
    };
    let envelope: iroha_data_model::zk::OpenVerifyEnvelope =
        norito::decode_canonical(&proof.bytes).map_err(|_| PreverifyResult::MalformedProof)?;
    envelope.validate_for_admission().map_err(|err| {
        if err == iroha_data_model::zk::OpenVerifyEnvelopeValidationError::ZeroVerifierKeyHash {
            PreverifyResult::VerifyingKeyMismatch
        } else {
            PreverifyResult::MalformedProof
        }
    })?;
    if envelope.backend != expected_tag {
        return Err(PreverifyResult::MalformedProof);
    }
    if expected_tag == iroha_data_model::zk::BackendTag::Halo2IpaPasta
        && !halo2_open_verify_circuit_id_matches_backend(&proof.backend, &envelope.circuit_id)
    {
        return Err(PreverifyResult::MalformedProof);
    }
    if expected_tag == iroha_data_model::zk::BackendTag::Halo2IpaPasta {
        let Some(expected_schema) = halo2_ipa_public_inputs_schema_v1(&envelope.circuit_id) else {
            return Err(PreverifyResult::MalformedProof);
        };
        if envelope.public_inputs.as_slice() != expected_schema {
            return Err(PreverifyResult::MalformedProof);
        }
    }
    if expected_tag == iroha_data_model::zk::BackendTag::Stark {
        if !stark_open_verify_circuit_id_matches_backend(&proof.backend, &envelope.circuit_id) {
            return Err(PreverifyResult::MalformedProof);
        }
        let Some(env_circuit_id) =
            normalize_stark_fri_circuit_id_for_backend(&proof.backend, &envelope.circuit_id)
        else {
            return Err(PreverifyResult::MalformedProof);
        };
        if normalized_retired_zk_ace_stark_circuit_id_for_backend(&proof.backend).as_deref()
            == Some(env_circuit_id.as_str())
        {
            return Err(PreverifyResult::MalformedProof);
        }
        if normalized_bfv_full_bootstrap_stark_circuit_id_for_backend(&proof.backend).as_deref()
            == Some(env_circuit_id.as_str())
        {
            return Err(PreverifyResult::MalformedProof);
        }
        if normalized_circuit_is_governance_vote_relation_for_backend(
            &proof.backend,
            &env_circuit_id,
        ) {
            return Err(PreverifyResult::MalformedProof);
        }
        if normalized_circuit_is_soracloud_fhe_relation_for_backend(&proof.backend, &env_circuit_id)
        {
            return Err(PreverifyResult::MalformedProof);
        }
        if normalized_ivm_execution_stark_circuit_id_for_backend(&proof.backend).as_deref()
            == Some(env_circuit_id.as_str())
        {
            if envelope.public_inputs.as_slice() != ivm_execution_public_inputs_schema_descriptor()
            {
                return Err(PreverifyResult::MalformedProof);
            }
            let open: iroha_data_model::zk::StarkFriOpenProofV1 =
                norito::decode_canonical(&envelope.proof_bytes)
                    .map_err(|_| PreverifyResult::MalformedProof)?;
            if open.version != 1
                || open.envelope_bytes.is_empty()
                || open.public_inputs.len() != 16
                || !open.public_inputs.iter().all(|column| column.len() == 1)
            {
                return Err(PreverifyResult::MalformedProof);
            }
        }
    }
    if let Some(vk_box) = vk
        && hash_vk(vk_box) != envelope.vk_hash
    {
        return Err(PreverifyResult::VerifyingKeyMismatch);
    }
    if let Some(commitment) = vk_commitment
        && commitment != envelope.vk_hash
    {
        return Err(PreverifyResult::VerifyingKeyMismatch);
    }
    if let Some(expected) = expected_vk_commitment
        && expected != envelope.vk_hash
    {
        return Err(PreverifyResult::VerifyingKeyMismatch);
    }
    Ok(())
}
fn preverify_bound_vk_commitment(
    vk_commitment: Option<[u8; 32]>,
    expected_vk_commitment: Option<[u8; 32]>,
) -> Result<[u8; 32], PreverifyResult> {
    let Some(expected) = expected_vk_commitment else {
        return Err(PreverifyResult::VerifyingKeyMissing);
    };
    if expected == [0u8; 32] {
        return Err(PreverifyResult::VerifyingKeyMismatch);
    }
    let Some(commitment) = vk_commitment else {
        return Ok(expected);
    };
    if commitment == [0u8; 32] || commitment != expected {
        return Err(PreverifyResult::VerifyingKeyMismatch);
    }
    Ok(commitment)
}
/// Result of a pre-verification step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreverifyResult {
    /// Proof accepted by lightweight pre-verification and not seen before in this batch.
    Accepted,
    /// Duplicate proof encountered within the same batch.
    Duplicate,
    /// Backend tag is empty or not recognized by the pre-verifier.
    UnsupportedBackend,
    /// Backend curve is not allowed by node configuration/policy.
    CurveNotAllowed,
    /// Proof payload exceeds the locally accepted maximum size for pre-verify.
    ProofTooBig,
    /// Malformed proof payload (e.g., empty bytes or structurally invalid header for the backend).
    MalformedProof,
    /// Pre-verification exceeded the provided cost budget.
    PreverifyBudgetExceeded,
    /// Proof references a verifying key that is missing or inactive.
    VerifyingKeyMissing,
    /// Proof references a verifying key whose commitment/schema do not match the envelope.
    VerifyingKeyMismatch,
    /// Proof references a verifying key bound to another namespace/manifest.
    NamespaceMismatch,
    /// Proof references a verifying key that is inactive or withdrawn.
    VerifyingKeyInactive,
}
/// Pre-verify a proof under a simple cost budget and deduplication cache.
///
/// This lightweight stage performs backend/tag admission, verifier-key binding
/// checks, bounded envelope parsing where applicable, and batch deduplication.
/// Full cryptographic verification is deferred to lane/overlay execution.
pub fn preverify_with_budget(
    proof: &ProofBox,
    vk: Option<&VerifyingKeyBox>,
    dedup: &mut DedupCache,
    budget: u64,
    vk_commitment: Option<[u8; 32]>,
    expected_vk_commitment: Option<[u8; 32]>,
    vk_active: bool,
) -> PreverifyResult {
    // Basic sanity: require non-empty backend tag
    if proof.backend.is_empty() {
        return PreverifyResult::UnsupportedBackend;
    }
    if is_production_claim_backend_label(proof.backend.as_str()) {
        return PreverifyResult::UnsupportedBackend;
    }
    if is_trusted_setup_backend_label(proof.backend.as_str()) {
        return PreverifyResult::UnsupportedBackend;
    }
    if is_developer_only_backend_label(proof.backend.as_str()) {
        return PreverifyResult::UnsupportedBackend;
    }
    if expected_preverify_envelope_backend_tag(proof.backend.as_str()).is_none() {
        return PreverifyResult::UnsupportedBackend;
    }
    if !vk_active {
        return PreverifyResult::VerifyingKeyInactive;
    }
    // Extremely lightweight budget model: count raw bytes processed.
    // When budget is 0, treat as unlimited.
    if budget > 0 {
        let limit = usize::try_from(budget).unwrap_or(usize::MAX);
        if limit < proof.bytes.len() {
            return PreverifyResult::PreverifyBudgetExceeded;
        }
    }
    let bound_vk_commitment =
        match preverify_bound_vk_commitment(vk_commitment, expected_vk_commitment) {
            Ok(commitment) => commitment,
            Err(err) => return err,
        };
    if let Some(vk_box) = vk
        && vk_box.backend != proof.backend
    {
        return PreverifyResult::VerifyingKeyMismatch;
    }
    // If we have both VK bytes and expected commitment, enforce the match early.
    if let (Some(expected), Some(vk_box)) = (expected_vk_commitment, vk) {
        let actual = crate::zk::hash_vk(vk_box);
        if actual != expected {
            return PreverifyResult::VerifyingKeyMismatch;
        }
    }
    if let (Some(expected), Some(commit)) = (expected_vk_commitment, vk_commitment) {
        if expected != commit {
            return PreverifyResult::VerifyingKeyMismatch;
        }
    }
    if let Err(err) = preverify_open_verify_envelope_metadata(
        proof,
        vk,
        Some(bound_vk_commitment),
        expected_vk_commitment,
    ) {
        return err;
    }
    if !dedup.check_and_insert_with_commitment(proof, Some(bound_vk_commitment)) {
        return PreverifyResult::Duplicate;
    }
    PreverifyResult::Accepted
}
/// Normalize one portable, non-reserved Halo2 IPA circuit identifier.
///
/// The same canonicalizer is used by proof admission, verifier selection, and
/// overlay binding so those security boundaries cannot accept different alias
/// sets.
pub(crate) fn normalize_halo2_ipa_circuit_id(circuit_id: &str) -> Option<String> {
    if circuit_id.len() > iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES
        || !iroha_data_model::zk::open_verify_circuit_id_is_portable(circuit_id)
        || iroha_data_model::zk::open_verify_circuit_id_uses_reserved_privacy_protocol_label_v1(
            circuit_id,
        )
    {
        return None;
    }
    let trimmed = circuit_id.trim();
    if trimmed.is_empty()
        || trimmed == ZK_BACKEND_HALO2_IPA
        || matches!(trimmed, "halo2/pasta" | "halo2/pasta/ipa")
    {
        return None;
    }
    if let Some(rest) = trimmed.strip_prefix("halo2/pasta/ipa/") {
        return (!rest.is_empty()).then(|| trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("halo2/pasta/") {
        return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
    }
    if let Some(rest) = trimmed.strip_prefix(ZK_BACKEND_HALO2_IPA) {
        if let Some(rest) = rest.strip_prefix("::") {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        if let Some(rest) = rest.strip_prefix(':') {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
        if let Some(rest) = rest.strip_prefix('/') {
            return (!rest.is_empty()).then(|| format!("halo2/pasta/ipa/{rest}"));
        }
    }
    Some(format!("halo2/pasta/ipa/{trimmed}"))
}
#[cfg(feature = "zk-halo2-ipa")]
fn verify_halo2_ipa_envelope(proof: &ProofBox, vk: Option<&VerifyingKeyBox>) -> bool {
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope};
    let Some(vk_box) = vk else {
        return false;
    };
    let env: OpenVerifyEnvelope = match norito::decode_canonical(&proof.bytes) {
        Ok(env) => env,
        Err(_) => return false,
    };
    if env.backend != BackendTag::Halo2IpaPasta {
        return false;
    }
    if env.validate_for_admission().is_err() {
        return false;
    }
    if !halo2_open_verify_circuit_id_matches_backend(proof.backend.as_str(), &env.circuit_id) {
        return false;
    }
    let Some(expected_schema) = halo2_ipa_public_inputs_schema_v1(&env.circuit_id) else {
        return false;
    };
    if env.public_inputs.as_slice() != expected_schema {
        return false;
    }
    let expected_vk_hash = hash_vk(vk_box);
    if env.vk_hash != expected_vk_hash {
        return false;
    }
    if !matches!(
        validate_and_prepare_verifying_key_material_v1(
            proof.backend.as_str(),
            &env.circuit_id,
            BackendTag::Halo2IpaPasta,
            vk_box,
        ),
        Ok(PreparedVerifyingKeyMaterialV1::Halo2IpaPasta { .. })
    ) {
        return false;
    }
    let backend = match normalize_halo2_ipa_circuit_id(&env.circuit_id) {
        Some(tag) => tag,
        None => return false,
    };
    let proof_box = ProofBox::new(proof.backend.clone(), env.proof_bytes);
    verify_halo2_ipa(&backend, &proof_box, Some(vk_box))
}
#[cfg(feature = "zk-stark")]
fn verify_stark_fri_open_verify_envelope(
    backend: &str,
    proof: &ProofBox,
    vk: Option<&VerifyingKeyBox>,
) -> bool {
    verify_stark_fri_open_verify_envelope_with_limits(
        backend,
        proof,
        vk,
        &crate::zk_stark::StarkVerifierLimits::default(),
    )
}
#[cfg(feature = "zk-stark")]
fn verify_stark_fri_open_verify_envelope_with_limits(
    backend: &str,
    proof: &ProofBox,
    vk: Option<&VerifyingKeyBox>,
    limits: &crate::zk_stark::StarkVerifierLimits,
) -> bool {
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};
    let reject = |reason: &'static str| {
        iroha_logger::debug!(
            backend,
            reason,
            "stark/fri proof rejected (metadata/integrity check failed)"
        );
        false
    };
    let env: OpenVerifyEnvelope = match norito::decode_canonical(&proof.bytes) {
        Ok(env) => env,
        Err(_) => return reject("invalid OpenVerifyEnvelope payload"),
    };
    if env.backend != BackendTag::Stark {
        return reject("unexpected OpenVerifyEnvelope backend tag");
    }
    if env.validate_for_admission().is_err() {
        return reject("invalid OpenVerifyEnvelope shape");
    }
    if !stark_open_verify_circuit_id_matches_backend(backend, &env.circuit_id) {
        return reject("STARK OpenVerifyEnvelope circuit_id does not match backend family");
    }
    let Some(vk_box) = vk else {
        return reject("missing verifying key");
    };
    if vk_box.backend != backend {
        return reject("STARK verifying key backend mismatch");
    }
    let expected_vk_hash = hash_vk(vk_box);
    if env.vk_hash != expected_vk_hash {
        return reject("verifying key commitment mismatch");
    }
    let expected_hash_fn = match stark_fri_backend_hash_policy_v1(backend) {
        Some(policy) => policy.expected(),
        None => return reject("unsupported stark/fri backend variant"),
    };
    // Reuse the registry/state-hydration material gate at proof dispatch. This
    // pins the parameters before any proof-controlled STARK payload is decoded.
    let (
        vk_circuit_id_raw,
        vk_n_log2,
        vk_blowup_log2,
        vk_fold_arity,
        vk_queries,
        vk_merkle_arity,
        vk_hash_fn,
    ) = match validate_and_prepare_verifying_key_material_v1(
        backend,
        &env.circuit_id,
        iroha_data_model::zk::BackendTag::Stark,
        vk_box,
    ) {
        Ok(PreparedVerifyingKeyMaterialV1::StarkFri {
            circuit_id,
            n_log2,
            blowup_log2,
            fold_arity,
            queries,
            merkle_arity,
            hash_fn,
        }) => (
            circuit_id,
            n_log2,
            blowup_log2,
            fold_arity,
            queries,
            merkle_arity,
            hash_fn,
        ),
        Ok(PreparedVerifyingKeyMaterialV1::Halo2IpaPasta { .. }) => {
            return reject("STARK registry key prepared as Halo2");
        }
        Err(_) => return reject("invalid STARK verifying key payload"),
    };
    let env_circuit_id = match normalize_stark_fri_circuit_id_for_backend(backend, &env.circuit_id)
    {
        Some(id) => id,
        None => return reject("invalid STARK envelope circuit_id"),
    };
    if normalized_retired_zk_ace_stark_circuit_id_for_backend(backend).as_deref()
        == Some(env_circuit_id.as_str())
    {
        return reject("retired generic ZK-ACE relation requires typed privacy verification");
    }
    if normalized_circuit_is_governance_vote_relation_for_backend(backend, &env_circuit_id) {
        return reject("governance vote roles require dedicated semantic verification");
    }
    let is_bfv_full_bootstrap_circuit =
        normalized_bfv_full_bootstrap_stark_circuit_id_for_backend(backend).as_deref()
            == Some(env_circuit_id.as_str());
    let is_ivm_execution_circuit = normalized_ivm_execution_stark_circuit_id_for_backend(backend)
        .as_deref()
        == Some(env_circuit_id.as_str());
    let vk_circuit_id =
        match normalize_stark_fri_circuit_id_for_backend(backend, &vk_circuit_id_raw) {
            Some(id) => id,
            None => return reject("invalid STARK verifying key circuit_id"),
        };
    if env_circuit_id != vk_circuit_id {
        return reject("STARK verifying key circuit_id mismatch");
    }
    if is_bfv_full_bootstrap_circuit {
        return reject("BFV full-bootstrap STARK circuit requires BFV-specific verification");
    }
    if normalized_circuit_is_soracloud_fhe_relation_for_backend(backend, &env_circuit_id) {
        return reject("Soracloud FHE relation requires dedicated typed Soracloud verification");
    }
    // Decode the STARK wrapper payload.
    let open: StarkFriOpenProofV1 = match norito::decode_canonical(&env.proof_bytes) {
        Ok(open) => open,
        Err(_) => return reject("invalid STARK wrapper payload"),
    };
    if open.version != 1 {
        return reject("unsupported STARK wrapper version");
    }
    if open.envelope_bytes.len() > limits.max_envelope_bytes {
        return reject("inner STARK envelope exceeds verifier limits");
    }
    if is_ivm_execution_circuit {
        if env.public_inputs.as_slice() != ivm_execution_public_inputs_schema_descriptor() {
            return reject("IVM execution STARK public-input schema mismatch");
        }
        if open.public_inputs.len() != 16
            || !open.public_inputs.iter().all(|column| column.len() == 1)
        {
            return reject("IVM execution STARK public input shape mismatch");
        }
    }
    // Bind the inner STARK envelope to the outer OpenVerifyEnvelope metadata and public inputs by
    // requiring `params.domain_tag` to equal the SHA-256 digest (hex, 64 chars) of:
    // `backend || circuit_id || vk_hash || schema/aux public_inputs || wrapper public inputs`.
    //
    // This prevents re-wrapping a valid STARK envelope under a different circuit/vk/public-inputs
    // header without detection.
    let expected_domain_tag = stark_open_verify_domain_tag_current(
        backend,
        &env.circuit_id,
        env.vk_hash,
        &env.public_inputs,
        &open.public_inputs,
    );
    let inner: crate::zk_stark::StarkVerifyEnvelopeV1 =
        match norito::decode_canonical(&open.envelope_bytes) {
            Ok(inner) => inner,
            Err(_) => return reject("invalid inner STARK envelope payload"),
        };
    if inner.transcript_label != STARK_OPEN_VERIFY_AIR_TRANSCRIPT_LABEL_V1 {
        return reject("STARK OpenVerifyEnvelope transcript label mismatch");
    }
    if inner.proof.commits.comp_root.is_some() || inner.proof.comp_values.is_some() {
        return reject(
            "STARK OpenVerifyEnvelope inner proof carries auxiliary composition commitments",
        );
    }
    // Verify that the prover is using the parameters pinned by the verifying key.
    if inner.params.hash_fn != vk_hash_fn
        || inner.params.n_log2 != vk_n_log2
        || inner.params.blowup_log2 != vk_blowup_log2
        || inner.params.fold_arity != vk_fold_arity
        || inner.params.queries != vk_queries
        || inner.params.merkle_arity != vk_merkle_arity
    {
        return reject("STARK proof parameters do not match verifying key");
    }
    if let Some(expected_hash_fn) = expected_hash_fn {
        if inner.params.hash_fn != expected_hash_fn {
            return reject("STARK proof hash_fn does not match backend");
        }
    }
    if inner.params.domain_tag != expected_domain_tag {
        return reject("domain tag integrity mismatch");
    }
    let expected_terms = stark_binding_air_terms(
        backend,
        &env.circuit_id,
        env.vk_hash,
        &env.public_inputs,
        &open.public_inputs,
    );
    let expected_public_digest = match crate::zk_stark::stark_air_public_digest_from_composition(
        STARK_BINDING_AIR_CONSTANT,
        STARK_BINDING_AIR_Z_COEFF,
        &expected_terms,
    ) {
        Ok(digest) => digest,
        Err(_) => return reject("STARK AIR public digest reconstruction failed"),
    };
    let Some(air) = inner.proof.air.as_ref() else {
        return reject("missing STARK AIR section");
    };
    let air_circuit_id = match normalize_stark_fri_circuit_id_for_backend(backend, &air.circuit_id)
    {
        Some(id) => id,
        None => return reject("invalid STARK AIR circuit_id"),
    };
    if air_circuit_id != env_circuit_id {
        return reject("STARK AIR circuit_id mismatch");
    }
    if air.public_digest != expected_public_digest {
        return reject("STARK AIR public digest mismatch");
    }
    let stark_ok =
        crate::zk_stark::verify_stark_fri_envelope_with_limits(&open.envelope_bytes, limits);
    if !stark_ok {
        return reject("inner STARK/FRI verifier rejected proof");
    }
    true
}
/// Verify a zero-knowledge proof using the requested backend, returning `true` when supported.
pub fn verify_backend(backend: &str, proof: &ProofBox, vk: Option<&VerifyingKeyBox>) -> bool {
    if proof.backend.as_str() != backend {
        return false;
    }
    if !is_production_verify_backend_label(backend) {
        return false;
    }
    // All production Halo2 labels share one authenticated outer-envelope boundary.
    if production_verify_backend_tag(backend)
        == Some(iroha_data_model::zk::BackendTag::Halo2IpaPasta)
    {
        #[cfg(feature = "zk-halo2-ipa")]
        {
            return verify_halo2_ipa_envelope(proof, vk);
        }
        #[cfg(not(feature = "zk-halo2-ipa"))]
        {
            return false;
        }
    }
    // Prefer built-in registry when available.
    if let Some(ok) = verify_with_registry(backend, proof, vk) {
        return ok;
    }
    // Native IPA polynomial-open verifier (transparent, no external libs)
    // Backend tag: "halo2/ipa/poly-open" with proof bytes = Norito `OpenVerifyEnvelope`.
    #[cfg(feature = "zk-ipa-native")]
    if backend == "halo2/ipa/poly-open" {
        return verify_ipa_open_envelope(proof);
    }
    // STARK/FRI family: native multi-fold verifier
    if is_stark_fri_v1_backend(backend) {
        #[cfg(feature = "zk-stark")]
        {
            // STARK proofs must use `OpenVerifyEnvelope` so the verifier can bind the
            // backend/circuit metadata and verifying-key hash into the inner STARK envelope.
            return verify_stark_fri_open_verify_envelope(backend, proof, vk);
        }
        #[cfg(not(feature = "zk-stark"))]
        {
            iroha_logger::debug!(
                backend,
                "stark/fri backend requested but binary was built without `zk-stark`"
            );
            return false;
        }
    }
    // Groth16 family: unsupported until native verifier is added under `zk-groth16`.
    if backend.starts_with("groth16/") {
        return false;
    }
    // Unknown backend tag
    false
}
#[cfg(test)]
mod debug_backend_tests {
    use super::*;
    #[test]
    fn developer_only_backends_are_unsupported() {
        for backend in [
            "debug/ok",
            "debug/reject",
            "debug/sleep",
            "stark/fri/dev-fixture",
            "stark/fri/d-e-v-f-i-x-t-u-r-e",
            "stark/fri/dev",
            "stark/fri/d-e-v",
            "stark/fri/test",
            "stark/fri/t-e-s-t",
            "stark/fri/todo",
            "stark/fri/t-o-d-o",
            "stark/fri/draft-only",
            "stark/fri/d-r-a-f-t",
            "stark/fri/pending-audit",
            "stark/fri/replace-before-mainnet",
            "stark/fri/not-production-ready",
            "stark/fri/placeholder",
            "miden-stark:dev-fixture",
            "halo2/ipa:dev-fixture",
            "halo2/ipa:dev",
            "halo2/ipa:todo-proof",
            "halo2/ipa:t-o-d-o-proof",
            "halo2/ipa:draft-proof",
            "halo2/ipa:d-r-a-f-t-proof",
            "halo2/ipa:pending-audit",
            "halo2/ipa:replace-before-production",
            "halo2/ipa:not-for-production",
            "halo2/ipa:dummy",
            "halo2/ipa:f-a-k-e",
            "halo2/ipa:stub",
            "halo2/ipa:s-a-m-p-l-e",
        ] {
            let proof = ProofBox::new(backend.into(), vec![0x01]);
            let vk = VerifyingKeyBox::new(backend.into(), vec![0x02]);
            assert!(!verify_backend(backend, &proof, Some(&vk)));
        }
    }
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn halo2_ivm_execution_fixtures_verify_for_restart_markers() {
        fn hash_for_live_proof(domain: &[u8], seed: [u8; 32]) -> iroha_crypto::Hash {
            let mut preimage = Vec::with_capacity(domain.len() + seed.len());
            preimage.extend_from_slice(domain);
            preimage.extend_from_slice(&seed);
            iroha_crypto::Hash::new(preimage)
        }
        for seed in [132_u8, 133_u8, 134_u8] {
            let marker = [seed; 32];
            let fixture = test_utils::halo2_ivm_execution_envelope(
                hash_for_live_proof(b"zk-confidential-localnet/code", marker),
                hash_for_live_proof(b"zk-confidential-localnet/overlay", marker),
                hash_for_live_proof(b"zk-confidential-localnet/events", marker),
                hash_for_live_proof(b"zk-confidential-localnet/gas-policy", marker),
            );
            let proof = fixture.proof_box(ZK_BACKEND_HALO2_IPA);
            let vk = fixture
                .vk_box(ZK_BACKEND_HALO2_IPA)
                .expect("fixture must include a verifying key");
            assert!(
                verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk)),
                "fixture proof should verify for seed {seed}"
            );
            #[cfg(feature = "zk-halo2-ipa")]
            {
                let mut wrong_schema: iroha_data_model::zk::OpenVerifyEnvelope =
                    norito::decode_canonical(&proof.bytes).expect("fixture envelope");
                wrong_schema.public_inputs = b"noncanonical-but-nonzero-schema".to_vec();
                let wrong_schema_proof = ProofBox::new(
                    ZK_BACKEND_HALO2_IPA.to_owned(),
                    norito::encode_canonical(&wrong_schema).expect("encode wrong-schema proof"),
                );
                assert!(
                    !verify_backend(ZK_BACKEND_HALO2_IPA, &wrong_schema_proof, Some(&vk)),
                    "generic Halo2 verifier must reject altered outer schema for seed {seed}"
                );

                let (exact_proof, exact_vk) = relabel_halo2_ipa_open_verify_fixture(
                    &proof,
                    &vk,
                    IVM_EXECUTION_V1_HALO2_BACKEND,
                );
                assert!(
                    verify_backend(
                        IVM_EXECUTION_V1_HALO2_BACKEND,
                        &exact_proof,
                        Some(&exact_vk),
                    ),
                    "exact IVM registry label should reach the IVM execution verifier for seed {seed}"
                );
                let mut exact_wrong_schema: iroha_data_model::zk::OpenVerifyEnvelope =
                    norito::decode_canonical(&exact_proof.bytes).expect("exact fixture envelope");
                exact_wrong_schema.public_inputs = b"different-nonzero-schema".to_vec();
                let exact_wrong_schema_proof = ProofBox::new(
                    IVM_EXECUTION_V1_HALO2_BACKEND.to_owned(),
                    norito::encode_canonical(&exact_wrong_schema)
                        .expect("encode exact wrong-schema proof"),
                );
                assert!(
                    !verify_backend(
                        IVM_EXECUTION_V1_HALO2_BACKEND,
                        &exact_wrong_schema_proof,
                        Some(&exact_vk),
                    ),
                    "exact Halo2 verifier must reject altered outer schema for seed {seed}"
                );
            }
        }
    }
    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn halo2_ivm_execution_rejects_legacy_unauthenticated_inner_headers() {
        use halo2_proofs::halo2curves::ff::PrimeField as _;

        let fixture = test_utils::halo2_ivm_execution_envelope(
            iroha_crypto::Hash::new(b"legacy-inner-header/code"),
            iroha_crypto::Hash::new(b"legacy-inner-header/overlay"),
            iroha_crypto::Hash::new(b"legacy-inner-header/events"),
            iroha_crypto::Hash::new(b"legacy-inner-header/gas-policy"),
        );
        let proof = fixture.proof_box(ZK_BACKEND_HALO2_IPA);
        let vk = fixture
            .vk_box(ZK_BACKEND_HALO2_IPA)
            .expect("fixture must include a verifying key");
        assert!(verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk)));

        let mut outer: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_canonical(&proof.bytes).expect("fixture envelope");
        let (raw_proof, columns) = zkparse::strict_proof_and_instances(&outer.proof_bytes)
            .expect("fixture must use strict ZK1");
        let public_inputs: Vec<[u8; 32]> = columns
            .iter()
            .map(|column| {
                let [value] = column.as_slice() else {
                    panic!("IVM fixture columns must contain one row");
                };
                let mut bytes = [0_u8; 32];
                bytes.copy_from_slice(value.to_repr().as_ref());
                bytes
            })
            .collect();
        assert_eq!(public_inputs.len(), 16);

        const RETIRED_LOOKUP_FLAG: u8 = 0x01;
        let legacy_headers = [(13, 0, 0), (0, 13, RETIRED_LOOKUP_FLAG)];
        let mut encoded_headers = Vec::new();
        for (n_in, n_out, flags) in legacy_headers {
            outer.proof_bytes = zk1_test_helpers::retired_halo2_envelope(
                u8::try_from(IVM_EXECUTION_V1_IPA_K).expect("IVM IPA k fits u8"),
                n_in,
                n_out,
                flags,
                &public_inputs,
                &raw_proof,
            );
            assert!(
                extract_pasta_instance_columns_bytes(&outer.proof_bytes).is_none(),
                "byte instance extraction must reject the retired carrier"
            );
            assert!(
                extract_pasta_fp_instances(&outer.proof_bytes).is_none(),
                "field instance extraction must reject the retired carrier"
            );
            encoded_headers.push(outer.proof_bytes.clone());
            let legacy_proof = ProofBox::new(
                ZK_BACKEND_HALO2_IPA.to_owned(),
                norito::encode_canonical(&outer).expect("encode legacy-carrier proof"),
            );
            assert!(
                !verify_backend(ZK_BACKEND_HALO2_IPA, &legacy_proof, Some(&vk)),
                "production verification must reject the unauthenticated legacy inner header"
            );
        }
        assert_ne!(encoded_headers[0], encoded_headers[1]);
    }
    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn halo2_ivm_execution_rejects_relabelled_demo_verifying_key() {
        let fixture = test_utils::halo2_ivm_execution_envelope(
            iroha_crypto::Hash::new(b"ivm-key-binding/code"),
            iroha_crypto::Hash::new(b"ivm-key-binding/overlay"),
            iroha_crypto::Hash::new(b"ivm-key-binding/events"),
            iroha_crypto::Hash::new(b"ivm-key-binding/gas-policy"),
        );
        let proof = fixture.proof_box(ZK_BACKEND_HALO2_IPA);
        let mut envelope: iroha_data_model::zk::OpenVerifyEnvelope =
            norito::decode_canonical(&proof.bytes).expect("fixture envelope");
        let params = pasta_params_new(IVM_EXECUTION_V1_IPA_K);
        let demo_vk =
            halo2_backend::keygen_vk(&params, &pasta_tiny::Add).expect("demo verifier key");
        let mut demo_vk_bytes = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut demo_vk_bytes, IVM_EXECUTION_V1_IPA_K);
        zk1::wrap_append_vk_pasta(&mut demo_vk_bytes, &demo_vk);
        let relabelled_vk = VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), demo_vk_bytes);
        assert!(
            resolve_vk_cached_for_type::<pasta_tiny::Add, _>(
                ZK_BACKEND_HALO2_IPA,
                &params,
                &relabelled_vk,
                || halo2_backend::keygen_vk(&params, &pasta_tiny::Add),
            )
            .is_ok(),
            "the demo key must populate only its own circuit-typed cache entry"
        );
        assert!(
            resolve_vk_cached_for_type::<pasta_tiny::IvmExecutionBindV1, _>(
                ZK_BACKEND_HALO2_IPA,
                &params,
                &relabelled_vk,
                || {
                    halo2_backend::keygen_vk(&params, &pasta_tiny::IvmExecutionBindV1::default())
                },
            )
            .is_err(),
            "a cache hit for one circuit type must not bypass canonical equality for another"
        );
        // Keep every caller-controlled binding internally consistent. The
        // verifier must still reject because this is not the canonical key for
        // the selected IVM constraint system.
        envelope.vk_hash = hash_vk(&relabelled_vk);
        let relabelled_proof = ProofBox::new(
            ZK_BACKEND_HALO2_IPA.to_owned(),
            norito::encode_canonical(&envelope).expect("encode relabelled envelope"),
        );
        assert!(
            !verify_backend(
                ZK_BACKEND_HALO2_IPA,
                &relabelled_proof,
                Some(&relabelled_vk)
            ),
            "a parseable demo key must not be relabelled as ivm-execution-v1"
        );
    }
}
#[cfg(test)]
mod stark_backend_tag_tests {
    use super::{
        ZK_BACKEND_HALO2_IPA, ZK_BACKEND_STARK_FRI_V1,
        halo2_open_verify_circuit_id_is_production_v1,
        halo2_open_verify_circuit_id_matches_backend, is_developer_only_backend_label,
        is_ivm_execution_backend, is_production_claim_backend_label,
        is_production_verify_backend_label, is_stark_fri_v1_backend,
        is_trusted_setup_backend_label, production_verify_backend_tag,
        stark_open_verify_circuit_id_matches_backend, verify_backend,
    };
    use iroha_data_model::privacy::{PRIVACY_RETIRED_PROTOCOL_LABELS_V1, PrivacyProtocolIdV1};
    use iroha_data_model::proof::{ProofBox, VerifyingKeyBox};
    use iroha_data_model::zk::BackendTag;
    #[test]
    fn detects_base_and_variant_backends() {
        assert!(is_stark_fri_v1_backend("stark/fri"));
        assert!(is_stark_fri_v1_backend("stark/fri/sha256-goldilocks"));
        assert!(is_stark_fri_v1_backend("stark/fri/poseidon2-goldilocks"));
        assert!(is_stark_fri_v1_backend("stark/fri/sha256_goldilocks.v1"));
        assert!(!is_stark_fri_v1_backend("stark/fri/latest"));
        assert!(!is_stark_fri_v1_backend("stark/fri/attestation"));
        assert!(!is_stark_fri_v1_backend("stark/fri/contest"));
        assert!(!is_stark_fri_v1_backend("stark/fri/random-profile"));
        assert!(!is_stark_fri_v1_backend("stark/fri/sha512-goldilocks"));
        assert!(!is_stark_fri_v1_backend("stark/fri/audit-proof-v1"));
        assert!(!is_stark_fri_v1_backend("stark/fri/"));
        assert!(!is_stark_fri_v1_backend("stark/fri/ "));
        assert!(!is_stark_fri_v1_backend("stark/fri/\t\n"));
        assert!(!is_stark_fri_v1_backend("stark/fri/ sha256-goldilocks"));
        assert!(!is_stark_fri_v1_backend("stark/fri/sha256-goldilocks "));
        assert!(!is_stark_fri_v1_backend("stark/fri/sha256 goldilocks"));
        assert!(!is_stark_fri_v1_backend(
            "stark\u{FF0F}fri/sha256-goldilocks"
        ));
        assert!(!is_stark_fri_v1_backend(
            "stark/fri/\u{200B}sha256-goldilocks"
        ));
        assert!(!is_stark_fri_v1_backend(
            "st\u{0430}rk/fri/sha256-goldilocks"
        ));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod;foo"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod,foo"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod+foo"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod/foo"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod(foo)"));
        assert!(!is_stark_fri_v1_backend("stark/fri/Δ"));
        assert!(!is_stark_fri_v1_backend("stark/fri/kzg"));
        assert!(!is_stark_fri_v1_backend("stark/fri/KZG"));
        assert!(!is_stark_fri_v1_backend("stark/fri/ KZG"));
        assert!(!is_stark_fri_v1_backend("stark/fri:kzg"));
        assert!(!is_stark_fri_v1_backend("stark/fri: KZG"));
        assert!(!is_stark_fri_v1_backend("stark/fri/bn254"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-bn-254"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-groth-16"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-k-z-g"));
        assert!(!is_stark_fri_v1_backend("stark/fri/bls12_381"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-b.l.s.12.381"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-srs"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-s-r-s"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod.crs"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-ptau"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-powers-of-tau"));
        assert!(!is_stark_fri_v1_backend("stark/fri/prod-ceremony"));
        assert!(!is_stark_fri_v1_backend(
            "stark/fri/structured-reference-string"
        ));
        assert!(!is_stark_fri_v1_backend("stark/fri/debug"));
        assert!(!is_stark_fri_v1_backend("stark/fri/Debug"));
        assert!(!is_stark_fri_v1_backend("stark/fri/debug-proof"));
        assert!(!is_stark_fri_v1_backend("stark/fri/d-e-b-u-g"));
        assert!(!is_stark_fri_v1_backend("stark/fri/mock"));
        assert!(!is_stark_fri_v1_backend("stark/fri/Mock"));
        assert!(!is_stark_fri_v1_backend("stark/fri/mock-proof"));
        assert!(!is_stark_fri_v1_backend("stark/fri/m-o-c-k"));
        assert!(!is_stark_fri_v1_backend("stark/fri/dev-fixture"));
        assert!(!is_stark_fri_v1_backend("stark/fri/d-e-v-f-i-x-t-u-r-e"));
        assert!(!is_stark_fri_v1_backend("stark/fri/dev"));
        assert!(!is_stark_fri_v1_backend("stark/fri/d-e-v"));
        assert!(!is_stark_fri_v1_backend("stark/fri/test"));
        assert!(!is_stark_fri_v1_backend("stark/fri/t-e-s-t"));
        assert!(!is_stark_fri_v1_backend("stark/fri/placeholder"));
        assert!(!is_stark_fri_v1_backend("stark/fri/miden"));
        assert!(!is_stark_fri_v1_backend("stark/fri/pq-masp-stark-fri"));
        assert!(!is_stark_fri_v1_backend("stark/fri/post-quantum-masp"));
        assert!(!is_stark_fri_v1_backend("stark/fri-v2"));
        assert!(!is_stark_fri_v1_backend("stark/fri-v10"));
    }
    #[test]
    fn production_claim_classifier_catches_readiness_and_audit_labels() {
        for backend in [
            "halo2/ipa:production-ready",
            "halo2/ipa:claimed-production",
            "halo2/ipa:mainnet-ready",
            "halo2/ipa:mainnet-complete",
            "halo2/ipa:production-certified",
            "stark/fri/audit-signoff",
            "stark/fri/externally-audited",
            "stark/fri/security-review-passed",
            "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
            "stark/fri/a-u-d-i-t-c-l-a-i-m",
            "halo2/ipa:release-ready",
            "halo2/ipa:release-approved",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa:third-party-audited",
            "halo2/ipa/orchard:production-ready",
            "orchard:mainnet-ready",
            "penumbra-masp:external-security-review",
            "jindo-lattice-pcs-zk:release-ready",
            "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
        ] {
            assert!(
                is_production_claim_backend_label(backend),
                "production-claim backend {backend} must be classified before allowlists"
            );
            assert!(
                !is_stark_fri_v1_backend(backend),
                "production-claim backend {backend} must not match the STARK family allowlist"
            );
            assert_eq!(
                production_verify_backend_tag(backend),
                None,
                "production-claim backend {backend} must not map to an OpenVerify tag"
            );
            assert!(
                !is_production_verify_backend_label(backend),
                "production-claim backend {backend} must stay fail-closed"
            );
        }
        for backend in [
            "halo2/ipa",
            "halo2/ipa:ivm-execution-v1",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1",
            "stark/fri/audit-proof-v1",
        ] {
            assert!(
                !is_production_claim_backend_label(backend),
                "backend {backend} must not be rejected by production-claim text alone"
            );
        }
    }
    #[test]
    fn ivm_execution_backend_allowlist_is_explicit() {
        assert!(is_ivm_execution_backend("halo2/ipa"));
        assert!(is_ivm_execution_backend("halo2/pasta/ivm-execution-v1"));
        assert!(is_ivm_execution_backend("stark/fri"));
        assert!(is_ivm_execution_backend("stark/fri/sha256-goldilocks"));
        assert!(is_ivm_execution_backend("stark/fri/poseidon2-goldilocks"));
        assert!(is_ivm_execution_backend("stark/fri/sha256_goldilocks.v1"));
        assert!(!is_ivm_execution_backend("halo2/ipa/orchard"));
        assert!(!is_ivm_execution_backend("halo2/ipa:ivm-execution-v1"));
        assert!(!is_ivm_execution_backend(
            "halo2/pasta/ipa/ivm-execution-v1"
        ));
        assert!(!is_ivm_execution_backend(
            "halo2/ipa/orchard:production-ready"
        ));
        assert!(!is_ivm_execution_backend("orchard:mainnet-ready"));
        assert!(!is_ivm_execution_backend(
            "penumbra-masp:external-security-review"
        ));
        assert!(!is_ivm_execution_backend(
            "jindo-lattice-pcs-zk:release-ready"
        ));
        assert!(!is_ivm_execution_backend("stark/fri/miden"));
        assert!(!is_ivm_execution_backend("miden-stark:dev-fixture"));
        assert!(!is_ivm_execution_backend("stark/fri/pq-masp-stark-fri"));
        assert!(!is_ivm_execution_backend(
            "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d"
        ));
        assert!(!is_ivm_execution_backend("stark/fri/kzg"));
        assert!(!is_ivm_execution_backend("stark/fri/prod-bn-254"));
        assert!(!is_ivm_execution_backend("stark/fri/prod-groth-16"));
        assert!(!is_ivm_execution_backend("stark/fri/random-profile"));
        assert!(!is_ivm_execution_backend("stark/fri/sha512-goldilocks"));
        assert!(!is_ivm_execution_backend("stark/fri/audit-proof-v1"));
        assert!(!is_ivm_execution_backend("stark/fri/debug"));
        assert!(!is_ivm_execution_backend("stark/fri/debug-proof"));
        assert!(!is_ivm_execution_backend("stark/fri/d-e-b-u-g"));
        assert!(!is_ivm_execution_backend("groth16/bn254"));
        assert!(!is_ivm_execution_backend("halo2/kzg"));
    }
    #[test]
    fn production_verify_backend_allowlist_is_explicit() {
        for (backend, expected_tag) in [
            ("halo2/ipa", BackendTag::Halo2IpaPasta),
            ("halo2/pasta/ivm-execution-v1", BackendTag::Halo2IpaPasta),
            ("halo2/pasta/kaigi-roster-v1", BackendTag::Halo2IpaPasta),
            ("halo2/pasta/kaigi-usage-v1", BackendTag::Halo2IpaPasta),
            (
                "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
                BackendTag::Halo2IpaPasta,
            ),
            (
                "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
                BackendTag::Halo2IpaPasta,
            ),
            (
                "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
                BackendTag::Halo2IpaPasta,
            ),
            (
                "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
                BackendTag::Halo2IpaPasta,
            ),
            ("stark/fri", BackendTag::Stark),
            ("stark/fri/sha256-goldilocks", BackendTag::Stark),
            ("stark/fri/poseidon2-goldilocks", BackendTag::Stark),
            ("stark/fri/sha256_goldilocks.v1", BackendTag::Stark),
        ] {
            assert_eq!(
                production_verify_backend_tag(backend),
                Some(expected_tag),
                "production label {backend} must map to its OpenVerify tag"
            );
            assert!(
                is_production_verify_backend_label(backend),
                "production label {backend} must be admitted"
            );
        }
        for backend in [
            "unknown/privacy/backend",
            "halo2/unknown-native-v1",
            "halo2/ipa:unknown-native-v1",
            "halo2/pasta/ivm-overlay-bind",
            "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
            "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2",
            "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
            "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
            "halo2/pasta/ipa/confidential-unshield-full-merkle16-axiom-poseidon-v3",
            "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
            "halo2/ipa:ivm-execution-v1",
            "HALO2/IPA",
            "stark/FRI",
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "halo2/ipa\0",
            "halo2\u{FF0F}ipa",
            "halo2/\u{200B}ipa",
            "h\u{0430}lo2/ipa",
            "../halo2/ipa",
            "halo2/ipa/../tiny-add",
            "halo2/ipa::ivm-execution-v1",
            "halo2//ipa",
            "halo2/ipa:",
            "halo2/ipa.",
            "halo2/ipa/.ivm-execution-v1",
            "halo2/ipa:ivm..execution-v1",
            "stark//fri/sha256-goldilocks",
            "stark/fri//sha256-goldilocks",
            "stark/fri/sha256..goldilocks",
            "stark/fri/sha256-goldilocks.",
            "halo2/ipa:ivm-execution-v1 ",
            "halo2/ipa/orchard",
            "halo2/ipa/penumbra",
            "halo2/ipa/masp",
            "halo2/ipa/monero",
            "halo2/ipa/curve-tree",
            "halo2/pasta/tiny-add",
            "halo2/ipa/tiny-add",
            "halo2/ipa:tiny-add",
            "halo2/pasta/tiny-anon-transfer-2x2",
            "halo2/pasta/tiny-commit-open",
            "halo2/pasta/anon-transfer-2x2",
            "halo2/ipa/anon-transfer-2x2",
            "halo2/ipa:anon-transfer-2x2",
            "halo2/pasta/anon-transfer-2x2-merkle2",
            "halo2/ipa/anon-transfer-2x2-merkle8",
            "halo2/ipa:anon-transfer-2x2-merkle16",
            "halo2/pasta/vote-bool-commit",
            "halo2/ipa/vote-bool-commit",
            "halo2/ipa:vote-bool-commit",
            "halo2/pasta/vote-bool-commit-merkle2",
            "halo2/ipa/vote-bool-commit-merkle8",
            "halo2/ipa:vote-bool-commit-merkle16",
            "halo2/ipa:dev-fixture",
            "halo2/ipa:dev",
            "halo2/ipa:d-e-v",
            "halo2/ipa:dummy",
            "halo2/ipa:f-a-k-e",
            "halo2/ipa:stub",
            "halo2/ipa:s-a-m-p-l-e",
            "halo2/ipa:production-ready",
            "halo2/ipa:claimed-production",
            "halo2/ipa:mainnet-ready",
            "halo2/ipa:production-certified",
            "halo2/ipa:release-ready",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa:third-party-audited",
            "stark/fri/miden",
            "stark/fri/latest",
            "stark/fri/attestation",
            "stark/fri/contest",
            "stark/fri/random-profile",
            "stark/fri/sha512-goldilocks",
            "stark/fri/audit-proof-v1",
            "stark/fri/dev-fixture",
            "stark/fri/d-e-v-f-i-x-t-u-r-e",
            "stark/fri/dev",
            "stark/fri/d-e-v",
            "stark/fri/test",
            "stark/fri/t-e-s-t",
            "stark/fri/placeholder",
            "stark/fri/audit-signoff",
            "stark/fri/externally-audited",
            "stark/fri/security-review-passed",
            "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
            "stark/fri/a-u-d-i-t-c-l-a-i-m",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            " stark/fri/sha256-goldilocks",
            "stark/fri/sha256-goldilocks ",
            "stark/fri/sha256-goldilocks\0",
            "stark\u{FF0F}fri/sha256-goldilocks",
            "stark/fri/\u{200B}sha256-goldilocks",
            "st\u{0430}rk/fri/sha256-goldilocks",
            "../stark/fri",
            "stark/fri/../sha256-goldilocks",
            "halo2/kzg",
            "halo2/mock",
        ] {
            assert_eq!(
                production_verify_backend_tag(backend),
                None,
                "unsupported backend {backend} must not map to an OpenVerify tag"
            );
            assert!(
                !is_production_verify_backend_label(backend),
                "unsupported backend {backend} must stay fail-closed"
            );
        }
    }
    #[test]
    fn verify_backend_rejects_protocol_names_before_dispatch() {
        for backend in [
            "halo2/ipa/orchard",
            "halo2/ipa/penumbra",
            "halo2/ipa/masp",
            "halo2/ipa/monero",
            "halo2/ipa/curve-tree",
            "stark/fri/miden",
            "stark/fri/pq-masp-stark-fri",
        ] {
            let proof = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
            let vk = VerifyingKeyBox::new(backend.to_owned(), vec![5, 6, 7, 8]);
            assert!(
                !verify_backend(backend, &proof, Some(&vk)),
                "protocol name {backend} must not reach a native verifier"
            );
        }
    }
    #[test]
    fn verify_backend_rejects_production_claim_labels_before_dispatch() {
        for backend in [
            "halo2/ipa:production-ready",
            "halo2/ipa:claimed-production",
            "halo2/ipa:mainnet-ready",
            "stark/fri/audit-signoff",
            "stark/fri/externally-audited",
            "stark/fri/security-review-passed",
            "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
            "stark/fri/a-u-d-i-t-c-l-a-i-m",
            "halo2/ipa:release-ready",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa:third-party-audited",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
        ] {
            let proof = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
            let vk = VerifyingKeyBox::new(backend.to_owned(), vec![5, 6, 7, 8]);
            assert!(
                !verify_backend(backend, &proof, Some(&vk)),
                "production-claim backend {backend} must not reach a native verifier"
            );
        }
    }
    #[test]
    fn trusted_setup_classifier_catches_standalone_and_profile_labels() {
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
            "BLS12_381",
            "bls12-381",
            "halo2/ipa:kzg",
            "halo2/ipa:KZG",
            "halo2/ipa: KZG",
            "Halo2/IPA:KZG",
            "halo2/pasta/ipa:kzg",
            "stark/fri:kzg",
            "stark/fri:KZG",
            "stark/fri: KZG",
            "stark/fri/prod;kzg",
            "stark/fri/prod,kzg",
            "stark/fri/prod+kzg",
            "stark/fri/prod.kzg",
            "stark/fri/prod-k-z-g",
            "stark/fri/prod(kzg)",
            "halo2/ipa:bn254",
            "halo2/ipa:BN254",
            "halo2/ipa: BN254",
            "stark/fri/prod;bn254",
            "stark/fri/prod-bn-254",
            "stark/fri/prod+bn256",
            "stark/fri/prod-bn-256",
            "stark/fri:bls12_381",
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
            "halo2/ipa/orchard:kzg",
            "orchard:universal-srs",
            "penumbra-masp:kzg",
            "jindo-lattice-pcs-zk:trusted-setup",
            "miden-stark:ptau",
            "sis-with-hints:groth16",
            "pq-masp-stark-fri:kzg",
        ] {
            assert!(
                is_trusted_setup_backend_label(backend),
                "trusted-setup backend {backend} must be classified before allowlist checks"
            );
        }
        for backend in [
            "halo2/ipa",
            "halo2/pasta/ipa/tiny-add",
            "stark/fri",
            "stark/fri/poseidon2-goldilocks",
        ] {
            assert!(
                !is_trusted_setup_backend_label(backend),
                "transparent backend {backend} must not be classified as trusted setup"
            );
        }
    }
    #[test]
    fn stark_open_verify_circuit_id_rejects_trusted_setup_family_aliases() {
        assert!(stark_open_verify_circuit_id_matches_backend(
            ZK_BACKEND_STARK_FRI_V1,
            "generic-binding-air"
        ));
        assert!(stark_open_verify_circuit_id_matches_backend(
            "stark/fri/sha256-goldilocks",
            "stark/fri/sha256-goldilocks:binding-air"
        ));
        for (backend, circuit_id) in [
            (ZK_BACKEND_STARK_FRI_V1, "bn254"),
            (ZK_BACKEND_STARK_FRI_V1, "BN254"),
            (ZK_BACKEND_STARK_FRI_V1, "b-n-254"),
            (ZK_BACKEND_STARK_FRI_V1, "bls12_381"),
            (ZK_BACKEND_STARK_FRI_V1, "universal-srs"),
            (ZK_BACKEND_STARK_FRI_V1, "structured-reference-string"),
            (ZK_BACKEND_STARK_FRI_V1, "stark/fri:bn254"),
            (ZK_BACKEND_STARK_FRI_V1, "stark/fri/prod-b.l.s.12.381"),
            (
                ZK_BACKEND_STARK_FRI_V1,
                "stark/fri/sha256-goldilocks:universal-srs",
            ),
            ("stark/fri/sha256-goldilocks", "bn254"),
            ("stark/fri/sha256-goldilocks", "stark/fri:bn254"),
            (
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:bn254",
            ),
            (
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks/srs",
            ),
            (
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:structured-reference-string",
            ),
        ] {
            assert!(
                !stark_open_verify_circuit_id_matches_backend(backend, circuit_id),
                "backend {backend} must reject trusted-setup circuit alias {circuit_id}"
            );
        }
    }
    #[test]
    fn generic_open_verify_matchers_reserve_all_privacy_protocol_labels() {
        fn assert_reserved(label: &str) {
            for circuit_id in [
                label.to_owned(),
                format!("halo2/ipa::{label}"),
                format!("halo2/pasta/{label}"),
                format!("stark/fri:{label}"),
                format!("stark/fri/sha256-goldilocks:{label}"),
                format!("generic/namespace/{label}"),
            ] {
                assert!(
                    !halo2_open_verify_circuit_id_is_production_v1(&circuit_id),
                    "Halo2 generic admission must reject privacy circuit id {circuit_id:?}"
                );
                assert!(
                    !halo2_open_verify_circuit_id_matches_backend(
                        ZK_BACKEND_HALO2_IPA,
                        &circuit_id,
                    ),
                    "Halo2 backend matching must reject privacy circuit id {circuit_id:?}"
                );
                assert!(
                    !stark_open_verify_circuit_id_matches_backend(
                        ZK_BACKEND_STARK_FRI_V1,
                        &circuit_id,
                    ),
                    "base STARK generic admission must reject privacy circuit id {circuit_id:?}"
                );
                assert!(
                    !stark_open_verify_circuit_id_matches_backend(
                        "stark/fri/sha256-goldilocks",
                        &circuit_id,
                    ),
                    "profile STARK admission must reject privacy circuit id {circuit_id:?}"
                );
            }
            for malformed_alias in [
                format!(" {label}"),
                format!("{label} "),
                label.to_ascii_uppercase(),
            ] {
                assert!(
                    !halo2_open_verify_circuit_id_matches_backend(
                        ZK_BACKEND_HALO2_IPA,
                        &malformed_alias,
                    ),
                    "non-portable Halo2 alias {malformed_alias:?} must fail closed"
                );
                assert!(
                    !stark_open_verify_circuit_id_matches_backend(
                        ZK_BACKEND_STARK_FRI_V1,
                        &malformed_alias,
                    ),
                    "non-portable STARK alias {malformed_alias:?} must fail closed"
                );
            }
            for near_miss in [format!("generic-{label}"), format!("{label}-generic")] {
                assert!(
                    !halo2_open_verify_circuit_id_is_production_v1(&near_miss),
                    "unregistered Halo2 near miss {near_miss:?} must fail closed"
                );
                assert!(
                    stark_open_verify_circuit_id_matches_backend(
                        ZK_BACKEND_STARK_FRI_V1,
                        &near_miss,
                    ),
                    "portable STARK near miss {near_miss:?} must remain available"
                );
            }
        }
        for protocol in PrivacyProtocolIdV1::ALL {
            assert_reserved(protocol.canonical_label());
        }
        for label in PRIVACY_RETIRED_PROTOCOL_LABELS_V1 {
            assert_reserved(label);
        }
    }
    #[test]
    fn developer_only_classifier_is_ascii_case_insensitive() {
        for backend in [
            "debug",
            "Debug",
            "DEBUG",
            "mock",
            "Mock",
            "MOCK",
            "halo2/ipa:Debug-Proof",
            "halo2/ipa:D-e-b-u-g-Proof",
            "halo2/ipa:Mock-Proof",
            "halo2/ipa:M-o-c-k-Proof",
            "stark/fri/Debug",
            "stark/fri/D-e-b-u-g",
            "stark/fri/Mock",
            "stark/fri/M-o-c-k",
            "stark/fri/dev-fixture",
            "stark/fri/D-e-v-F-i-x-t-u-r-e",
            "stark/fri/dev",
            "stark/fri/D-e-v",
            "stark/fri/Test",
            "stark/fri/T-e-s-t",
            "stark/fri/Placeholder",
            "miden-stark:DevFixture",
            "halo2/ipa:DevFixture",
            "halo2/ipa:d-e-v-f-i-x-t-u-r-e",
            "halo2/ipa:Dev",
            "halo2/ipa:d-e-v",
            "halo2/ipa:Dummy",
            "halo2/ipa:F-a-k-e",
            "halo2/ipa:Stub",
            "halo2/ipa:S-a-m-p-l-e",
        ] {
            assert!(
                is_developer_only_backend_label(backend),
                "developer-only backend {backend} must be classified before allowlist checks"
            );
        }
    }
    #[test]
    fn developer_only_classifier_does_not_reject_embedded_text_fragments() {
        for backend in [
            "stark/fri/latest",
            "stark/fri/attestation",
            "stark/fri/contest",
            "halo2/ipa:attestation",
        ] {
            assert!(
                !is_developer_only_backend_label(backend),
                "backend {backend} must not be rejected because a normal word contains `test`"
            );
        }
    }
}
#[cfg(all(test, feature = "zk-stark"))]
macro_rules! consensus_stark_vk {
    ($circuit_id:expr, $hash_fn:expr $(,)?) => {
        $crate::zk_stark::StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: $circuit_id,
            n_log2: $crate::zk_stark::STARK_FRI_CONSENSUS_MIN_N_LOG2,
            blowup_log2: $crate::zk_stark::STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
            fold_arity: 2,
            queries: $crate::zk_stark::STARK_FRI_CONSENSUS_MIN_QUERIES,
            merkle_arity: 2,
            hash_fn: $hash_fn,
        }
    };
}
#[cfg(all(test, feature = "zk-stark"))]
mod stark_prover_tests {
    use super::{
        STARK_BINDING_AIR_CONSTANT, STARK_BINDING_AIR_Z_COEFF, STARK_GOLDILOCKS_MODULUS,
        STARK_OPEN_VERIFY_AIR_TRANSCRIPT_LABEL_V1, ZK_BACKEND_STARK_FRI_V1, limb_as_instance_bytes,
        normalize_stark_fri_circuit_id_for_backend, prove_stark_fri_ivm_execution_envelope,
        prove_stark_fri_open_verify_envelope, stark_binding_air_terms,
        stark_open_verify_air_public_digest_current, stark_open_verify_domain_tag_current,
        verify_backend_with_timing,
    };
    use crate::zk_stark::{
        STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2, STARK_FRI_CONSENSUS_MIN_N_LOG2,
        STARK_FRI_CONSENSUS_MIN_QUERIES, STARK_HASH_POSEIDON2_V1, STARK_HASH_SHA256_V1,
        StarkCompositionValueV1, StarkFriParamsV1, StarkFriVerifyingKeyV1, StarkVerifyEnvelopeV1,
    };
    use iroha_crypto::Hash;
    use iroha_data_model::proof::{ProofBox, VerifyingKeyBox};
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};
    #[test]
    fn instance_limb_bytes_are_little_endian_and_zero_extended() {
        let limb = 0x0123_4567_89ab_cdef;
        let encoded = limb_as_instance_bytes(limb);
        assert_eq!(&encoded[..8], &limb.to_le_bytes());
        assert_eq!(encoded[8..], [0; 24]);
    }
    fn sample_stark_open_verify_proof() -> (&'static str, String, VerifyingKeyBox, ProofBox) {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:tiny-open");
        let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
        let vk_bytes = norito::to_bytes(&vk_payload).expect("encode vk payload");
        let vk_box = VerifyingKeyBox::new(backend.to_owned(), vk_bytes);
        let proof = prove_stark_fri_open_verify_envelope(
            backend,
            &circuit_id,
            &vk_box,
            b"tiny:schema:v1",
            vec![vec![[0x11; 32]], vec![[0x22; 32]]],
        )
        .expect("binding AIR STARK proof");
        (backend, circuit_id, vk_box, proof)
    }
    fn weak_stark_vk_payload(backend: &str, circuit_id: String) -> StarkFriVerifyingKeyV1 {
        StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id,
            n_log2: STARK_FRI_CONSENSUS_MIN_N_LOG2 - 1,
            blowup_log2: STARK_FRI_CONSENSUS_MIN_BLOWUP_LOG2,
            fold_arity: 2,
            queries: STARK_FRI_CONSENSUS_MIN_QUERIES,
            merkle_arity: 2,
            hash_fn: if backend.contains("/poseidon2-") {
                crate::zk_stark::STARK_HASH_POSEIDON2_V1
            } else {
                STARK_HASH_SHA256_V1
            },
        }
    }
    fn weak_stark_open_verify_proof(
        backend: &str,
        circuit_id: &str,
        vk_box: &VerifyingKeyBox,
        schema_descriptor: Vec<u8>,
        public_inputs: Vec<Vec<[u8; 32]>>,
    ) -> ProofBox {
        stark_open_verify_proof_with_transcript_label(
            backend,
            circuit_id,
            vk_box,
            schema_descriptor,
            public_inputs,
            STARK_OPEN_VERIFY_AIR_TRANSCRIPT_LABEL_V1,
        )
    }
    fn stark_open_verify_proof_with_transcript_label(
        backend: &str,
        circuit_id: &str,
        vk_box: &VerifyingKeyBox,
        schema_descriptor: Vec<u8>,
        public_inputs: Vec<Vec<[u8; 32]>>,
        transcript_label: &str,
    ) -> ProofBox {
        let vk_payload: StarkFriVerifyingKeyV1 =
            norito::decode_from_bytes(&vk_box.bytes).expect("decode weak STARK VK payload");
        let vk_hash = super::hash_vk(vk_box);
        let domain_tag = stark_open_verify_domain_tag_current(
            backend,
            circuit_id,
            vk_hash,
            &schema_descriptor,
            &public_inputs,
        );
        let params = StarkFriParamsV1 {
            version: 1,
            n_log2: vk_payload.n_log2,
            blowup_log2: vk_payload.blowup_log2,
            fold_arity: vk_payload.fold_arity,
            queries: vk_payload.queries,
            merkle_arity: vk_payload.merkle_arity,
            hash_fn: vk_payload.hash_fn,
            domain_tag,
        };
        let env_circuit_id = normalize_stark_fri_circuit_id_for_backend(backend, circuit_id)
            .expect("normalize weak STARK circuit id");
        let public_digest = stark_open_verify_air_public_digest_current(
            backend,
            circuit_id,
            vk_hash,
            &schema_descriptor,
            &public_inputs,
        )
        .expect("derive weak STARK AIR public digest");
        let envelope_bytes = match crate::zk_stark::prove_stark_fri_air_envelope_bytes(
            params.clone(),
            transcript_label.to_owned(),
            env_circuit_id.clone(),
            public_digest,
        ) {
            Ok(envelope_bytes) => envelope_bytes,
            Err(err)
                if err.contains("BFV full-bootstrap")
                    || err.contains("ZK-ACE")
                    || err.contains("IVM execution")
                    || err.contains("Soracloud") =>
            {
                let fixture_air_circuit_id = format!("{env_circuit_id}:generic-binding-fixture");
                let envelope_bytes = crate::zk_stark::prove_stark_fri_air_envelope_bytes(
                    params,
                    transcript_label.to_owned(),
                    fixture_air_circuit_id,
                    public_digest,
                )
                .expect("build weak STARK AIR proof with fixture circuit id");
                let mut inner: StarkVerifyEnvelopeV1 = norito::decode_from_bytes(&envelope_bytes)
                    .expect("decode weak STARK AIR proof");
                inner
                    .proof
                    .air
                    .as_mut()
                    .expect("weak STARK AIR proof carries AIR section")
                    .circuit_id = env_circuit_id;
                norito::to_bytes(&inner).expect("encode retargeted weak STARK AIR proof")
            }
            Err(err) => panic!("build weak STARK AIR proof: {err}"),
        };
        let open = StarkFriOpenProofV1 {
            version: 1,
            public_inputs,
            envelope_bytes,
        };
        let outer = OpenVerifyEnvelope {
            backend: BackendTag::Stark,
            circuit_id: circuit_id.to_owned(),
            vk_hash,
            public_inputs: schema_descriptor,
            proof_bytes: norito::to_bytes(&open).expect("encode weak STARK open proof"),
            aux: Vec::new(),
        };
        ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&outer).expect("encode weak STARK OpenVerifyEnvelope"),
        )
    }
    fn mutate_outer_stark_open_verify_proof(
        backend: &str,
        proof: &ProofBox,
        mutate: impl FnOnce(&mut OpenVerifyEnvelope),
    ) -> ProofBox {
        let mut outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        mutate(&mut outer);
        ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&outer).expect("encode tampered outer STARK envelope"),
        )
    }
    #[test]
    fn prove_stark_open_verify_envelope_rejects_below_floor_verifying_key_payload() {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:weak-open");
        let vk_payload = weak_stark_vk_payload(backend, circuit_id.clone());
        let vk_box = VerifyingKeyBox::new(
            backend.to_owned(),
            norito::to_bytes(&vk_payload).expect("encode weak STARK VK payload"),
        );
        let err = prove_stark_fri_open_verify_envelope(
            backend,
            &circuit_id,
            &vk_box,
            b"weak:schema:v1",
            vec![vec![[0x11; 32]]],
        )
        .expect_err("generic STARK builder must reject below-floor VK payloads");
        assert!(
            err.contains("below consensus floor"),
            "unexpected below-floor VK rejection: {err}"
        );
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_below_floor_verifying_key_payload() {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:weak-open");
        let vk_payload = weak_stark_vk_payload(backend, circuit_id.clone());
        let vk_box = VerifyingKeyBox::new(
            backend.to_owned(),
            norito::to_bytes(&vk_payload).expect("encode weak STARK VK payload"),
        );
        let proof = weak_stark_open_verify_proof(
            backend,
            &circuit_id,
            &vk_box,
            b"weak:schema:v1".to_vec(),
            vec![vec![[0x11; 32]]],
        );
        let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
        assert!(
            !report.ok,
            "generic STARK verifier must reject below-floor VK payloads"
        );
    }
    #[test]
    fn prove_stark_open_verify_envelope_rejects_verifying_key_backend_mismatch() {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:backend-mismatch");
        let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
        let vk_box = VerifyingKeyBox::new(
            "stark/fri".to_owned(),
            norito::to_bytes(&vk_payload).expect("encode backend-mismatched STARK VK payload"),
        );
        let err = prove_stark_fri_open_verify_envelope(
            backend,
            &circuit_id,
            &vk_box,
            b"backend-mismatch:schema:v1",
            vec![vec![[0x11; 32]]],
        )
        .expect_err("generic STARK builder must reject verifier-key backend mismatch");
        assert!(
            err.contains("backend mismatch"),
            "unexpected VK backend mismatch rejection: {err}"
        );
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_verifying_key_backend_mismatch() {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:backend-mismatch");
        let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
        let vk_box = VerifyingKeyBox::new(
            "stark/fri".to_owned(),
            norito::to_bytes(&vk_payload).expect("encode backend-mismatched STARK VK payload"),
        );
        let proof = weak_stark_open_verify_proof(
            backend,
            &circuit_id,
            &vk_box,
            b"backend-mismatch:schema:v1".to_vec(),
            vec![vec![[0x11; 32]]],
        );
        let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
        assert!(
            !report.ok,
            "generic STARK verifier must reject a verifier key tagged for another backend"
        );
    }
    #[test]
    fn prove_stark_open_verify_envelope_rejects_circuit_family_mismatch() {
        for (case, backend, circuit_id) in [
            (
                "profile backend with sibling STARK profile",
                "stark/fri/sha256-goldilocks",
                "stark/fri/poseidon2-goldilocks:family-spoof",
            ),
            (
                "profile backend with generic STARK prefix",
                "stark/fri/sha256-goldilocks",
                "stark/fri:family-spoof",
            ),
            (
                "profile backend with bare generic STARK family",
                "stark/fri/sha256-goldilocks",
                "stark/fri",
            ),
            (
                "generic STARK backend with halo2 circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "halo2/ipa:family-spoof",
            ),
            (
                "generic STARK backend with colon-form halo2 circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "halo2:family-spoof",
            ),
            (
                "generic STARK backend with colon-form kzg circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "kzg:trusted-setup-spoof",
            ),
            (
                "generic STARK backend with bare trusted-setup curve circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "bn254",
            ),
            (
                "generic STARK backend with separated trusted-setup curve circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "b.l.s.12.381",
            ),
            (
                "generic STARK backend with STARK-prefixed trusted-setup circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "stark/fri:universal-srs",
            ),
            (
                "profile backend with bare trusted-setup circuit",
                "stark/fri/sha256-goldilocks",
                "bn254",
            ),
            (
                "profile backend with profile-prefixed trusted-setup circuit",
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:structured-reference-string",
            ),
        ] {
            let vk_payload = consensus_stark_vk!(circuit_id.to_owned(), STARK_HASH_SHA256_V1);
            let vk_box = VerifyingKeyBox::new(
                backend.to_owned(),
                norito::to_bytes(&vk_payload)
                    .expect("encode circuit-family-mismatched STARK VK payload"),
            );
            let err = prove_stark_fri_open_verify_envelope(
                backend,
                circuit_id,
                &vk_box,
                b"family-mismatch:schema:v1",
                vec![vec![[0x11; 32]]],
            )
            .expect_err("generic STARK prover must reject circuit ids from another family/profile");
            assert!(
                err.contains("backend family"),
                "unexpected circuit family rejection for {case}: {err}"
            );
        }
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_circuit_family_mismatch() {
        for (case, backend, circuit_id) in [
            (
                "profile backend with sibling STARK profile",
                "stark/fri/sha256-goldilocks",
                "stark/fri/poseidon2-goldilocks:family-spoof",
            ),
            (
                "profile backend with generic STARK prefix",
                "stark/fri/sha256-goldilocks",
                "stark/fri:family-spoof",
            ),
            (
                "profile backend with bare generic STARK family",
                "stark/fri/sha256-goldilocks",
                "stark/fri",
            ),
            (
                "generic STARK backend with halo2 circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "halo2/ipa:family-spoof",
            ),
            (
                "generic STARK backend with colon-form halo2 circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "halo2:family-spoof",
            ),
            (
                "generic STARK backend with colon-form kzg circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "kzg:trusted-setup-spoof",
            ),
            (
                "generic STARK backend with bare trusted-setup curve circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "bn254",
            ),
            (
                "generic STARK backend with separated trusted-setup curve circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "b.l.s.12.381",
            ),
            (
                "generic STARK backend with STARK-prefixed trusted-setup circuit",
                super::ZK_BACKEND_STARK_FRI_V1,
                "stark/fri:universal-srs",
            ),
            (
                "profile backend with bare trusted-setup circuit",
                "stark/fri/sha256-goldilocks",
                "bn254",
            ),
            (
                "profile backend with profile-prefixed trusted-setup circuit",
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:structured-reference-string",
            ),
        ] {
            let vk_payload = consensus_stark_vk!(circuit_id.to_owned(), STARK_HASH_SHA256_V1);
            let vk_box = VerifyingKeyBox::new(
                backend.to_owned(),
                norito::to_bytes(&vk_payload)
                    .expect("encode circuit-family-mismatched STARK VK payload"),
            );
            let proof = weak_stark_open_verify_proof(
                backend,
                circuit_id,
                &vk_box,
                b"family-mismatch:schema:v1".to_vec(),
                vec![vec![[0x11; 32]]],
            );
            let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
            assert!(
                !report.ok,
                "generic STARK verifier must reject circuit id family/profile mismatch for {case}"
            );
        }
    }
    #[test]
    fn prove_stark_open_verify_envelope_rejects_zk_ace_circuit_aliases() {
        let backend = "stark/fri/sha256-goldilocks";
        let canonical = iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID;
        let prefixed_alias = format!("{backend}:{canonical}");
        let slash_alias = format!("{backend}/{canonical}");
        for circuit_id in [canonical.to_owned(), prefixed_alias, slash_alias] {
            let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
            let vk_box = VerifyingKeyBox::new(
                backend.to_owned(),
                norito::to_bytes(&vk_payload).expect("encode ZK-ACE alias STARK VK payload"),
            );
            let err = prove_stark_fri_open_verify_envelope(
                backend,
                &circuit_id,
                &vk_box,
                b"zk-ace:generic-schema:v1",
                vec![vec![[0x33; 32]]],
            )
            .expect_err("generic STARK prover must not target ZK-ACE circuit aliases");
            assert!(
                err.contains("ZK-ACE"),
                "unexpected ZK-ACE alias rejection for {circuit_id}: {err}"
            );
        }
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_zk_ace_alias_generic_binding_air() {
        let backend = "stark/fri/sha256-goldilocks";
        let canonical = iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID;
        let prefixed_alias = format!("{backend}:{canonical}");
        let slash_alias = format!("{backend}/{canonical}");
        for circuit_id in [prefixed_alias, slash_alias] {
            let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
            let vk_box = VerifyingKeyBox::new(
                backend.to_owned(),
                norito::to_bytes(&vk_payload).expect("encode ZK-ACE alias STARK VK payload"),
            );
            let proof = weak_stark_open_verify_proof(
                backend,
                &circuit_id,
                &vk_box,
                b"zk-ace:forged-generic-schema:v1".to_vec(),
                vec![vec![[0x44; 32]]],
            );
            let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
            assert!(
                !report.ok,
                "ZK-ACE circuit alias {circuit_id} must not verify as generic binding AIR"
            );
        }
    }
    #[test]
    fn prove_stark_open_verify_envelope_rejects_ivm_execution_circuit_aliases() {
        let backend = "stark/fri/sha256-goldilocks";
        let canonical = super::IVM_EXECUTION_V1_CIRCUIT_ID;
        let prefixed_alias = format!("{backend}:{canonical}");
        let slash_alias = format!("{backend}/{canonical}");
        for circuit_id in [canonical.to_owned(), prefixed_alias, slash_alias] {
            let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
            let vk_box = VerifyingKeyBox::new(
                backend.to_owned(),
                norito::to_bytes(&vk_payload).expect("encode IVM alias STARK VK payload"),
            );
            let err = prove_stark_fri_open_verify_envelope(
                backend,
                &circuit_id,
                &vk_box,
                b"ivm:generic-schema:v1",
                vec![vec![[0x66; 32]]],
            )
            .expect_err("generic STARK prover must not target IVM execution circuit aliases");
            assert!(
                err.contains("IVM execution"),
                "unexpected IVM alias rejection for {circuit_id}: {err}"
            );
        }
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_ivm_alias_generic_binding_air() {
        let backend = "stark/fri/sha256-goldilocks";
        let canonical = super::IVM_EXECUTION_V1_CIRCUIT_ID;
        let prefixed_alias = format!("{backend}:{canonical}");
        let slash_alias = format!("{backend}/{canonical}");
        for circuit_id in [canonical.to_owned(), prefixed_alias, slash_alias] {
            let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
            let vk_box = VerifyingKeyBox::new(
                backend.to_owned(),
                norito::to_bytes(&vk_payload).expect("encode IVM alias STARK VK payload"),
            );
            let proof = weak_stark_open_verify_proof(
                backend,
                &circuit_id,
                &vk_box,
                b"ivm:forged-generic-schema:v1".to_vec(),
                vec![vec![[0x77; 32]]],
            );
            let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
            assert!(
                !report.ok,
                "IVM execution circuit alias {circuit_id} must not verify with generic schema"
            );
        }
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_namespaced_reserved_aliases() {
        let backend = "stark/fri/sha256-goldilocks";
        for canonical in [
            super::IVM_EXECUTION_V1_CIRCUIT_ID,
            iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
            iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID,
        ] {
            for circuit_id in [
                format!("tenant:{canonical}"),
                format!("{backend}:tenant:{canonical}"),
            ] {
                let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
                let vk_box = VerifyingKeyBox::new(
                    backend.to_owned(),
                    norito::to_bytes(&vk_payload)
                        .expect("encode namespaced reserved STARK VK payload"),
                );
                let proof = weak_stark_open_verify_proof(
                    backend,
                    &circuit_id,
                    &vk_box,
                    b"reserved:forged-generic-schema:v1".to_vec(),
                    vec![vec![[0x79; 32]]],
                );
                let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
                assert!(
                    !report.ok,
                    "reserved circuit alias {circuit_id} must not verify as generic binding AIR"
                );
            }
        }
    }
    #[test]
    fn prove_stark_open_verify_envelope_rejects_bfv_full_bootstrap_circuit_aliases() {
        let canonical = iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1;
        for backend in [
            ZK_BACKEND_STARK_FRI_V1,
            iroha_crypto::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1,
        ] {
            let prefixed_alias = format!("{backend}:{canonical}");
            let slash_alias = format!("{backend}/{canonical}");
            for circuit_id in [canonical.to_owned(), prefixed_alias, slash_alias] {
                let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
                let vk_box = VerifyingKeyBox::new(
                    backend.to_owned(),
                    norito::to_bytes(&vk_payload).expect("encode BFV alias STARK VK payload"),
                );
                let err = prove_stark_fri_open_verify_envelope(
                    backend,
                    &circuit_id,
                    &vk_box,
                    b"bfv:generic-schema:v1",
                    vec![vec![[0x55; 32]]],
                )
                .expect_err("generic STARK prover must not target BFV full-bootstrap aliases");
                assert!(
                    err.contains("BFV full-bootstrap"),
                    "unexpected BFV alias rejection for {backend} / {circuit_id}: {err}"
                );
            }
        }
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_bfv_full_bootstrap_alias_generic_binding_air() {
        let canonical = iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1;
        for backend in [
            ZK_BACKEND_STARK_FRI_V1,
            iroha_crypto::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1,
            "stark/fri/poseidon2-goldilocks",
        ] {
            let prefixed_alias = format!("{backend}:{canonical}");
            let slash_alias = format!("{backend}/{canonical}");
            for circuit_id in [canonical.to_owned(), prefixed_alias, slash_alias] {
                let vk_payload = consensus_stark_vk!(
                    circuit_id.clone(),
                    if backend.contains("/poseidon2-") {
                        STARK_HASH_POSEIDON2_V1
                    } else {
                        STARK_HASH_SHA256_V1
                    }
                );
                let vk_box = VerifyingKeyBox::new(
                    backend.to_owned(),
                    norito::to_bytes(&vk_payload).expect("encode BFV alias STARK VK payload"),
                );
                let proof = weak_stark_open_verify_proof(
                    backend,
                    &circuit_id,
                    &vk_box,
                    b"bfv:forged-generic-schema:v1".to_vec(),
                    vec![vec![[0x66; 32]]],
                );
                let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
                assert!(
                    !report.ok,
                    "BFV full-bootstrap alias {backend} / {circuit_id} must not verify as generic binding AIR"
                );
            }
        }
    }
    fn soracloud_fhe_proof_relations() -> [(&'static str, &'static [u8]); 4] {
        use iroha_data_model::soracloud::{
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
            SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
        };
        [
            (
                SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
                SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1,
            ),
            (
                SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
                SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            ),
            (
                SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
                SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            ),
            (
                SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
                SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1,
            ),
        ]
    }
    #[test]
    fn generic_stark_prover_rejects_every_soracloud_fhe_relation_alias() {
        let backend = "stark/fri/sha256-goldilocks";
        for (canonical, schema) in soracloud_fhe_proof_relations() {
            for circuit_id in [
                canonical.to_owned(),
                format!("{backend}:{canonical}"),
                format!("{backend}/{canonical}"),
            ] {
                let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
                let vk_box = VerifyingKeyBox::new(
                    backend.to_owned(),
                    norito::to_bytes(&vk_payload).expect("encode Soracloud STARK VK payload"),
                );
                let err = prove_stark_fri_open_verify_envelope(
                    backend,
                    &circuit_id,
                    &vk_box,
                    schema,
                    vec![vec![[0xA7; 32]]],
                )
                .expect_err("generic STARK prover must not target a Soracloud FHE relation");
                assert!(
                    err.contains("Soracloud") || err.contains("BFV full-bootstrap"),
                    "unexpected Soracloud relation rejection for {circuit_id}: {err}"
                );
            }
        }
    }
    #[test]
    fn generic_stark_verifier_rejects_public_metadata_only_soracloud_fhe_proofs() {
        let backend = "stark/fri/sha256-goldilocks";
        for (circuit_id, schema) in soracloud_fhe_proof_relations() {
            let vk_payload = consensus_stark_vk!(circuit_id.to_owned(), STARK_HASH_SHA256_V1);
            let vk_box = VerifyingKeyBox::new(
                backend.to_owned(),
                norito::to_bytes(&vk_payload).expect("encode Soracloud STARK VK payload"),
            );
            // This is exactly the vacuous construction under regression: it
            // carries a claimed public statement hash and the public schema,
            // but no ciphertext, key, refresh, material, or execution witness.
            let proof = weak_stark_open_verify_proof(
                backend,
                circuit_id,
                &vk_box,
                schema.to_vec(),
                vec![vec![[0xA7; 32]]],
            );
            let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
            assert!(
                !report.ok,
                "public metadata alone must not prove Soracloud FHE relation {circuit_id}"
            );
        }
    }
    fn stark_field_add_for_test(a: u64, b: u64) -> u64 {
        (((a as u128) + (b as u128)) % STARK_GOLDILOCKS_MODULUS) as u64
    }
    fn stark_field_mul_for_test(a: u64, b: u64) -> u64 {
        (((a as u128) * (b as u128)) % STARK_GOLDILOCKS_MODULUS) as u64
    }
    fn attach_valid_auxiliary_composition_to_open_verify_proof(
        backend: &str,
        outer: &mut OpenVerifyEnvelope,
    ) {
        let mut open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&outer.proof_bytes).expect("decode STARK open proof");
        let mut inner: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&open.envelope_bytes).expect("decode inner STARK envelope");
        let terms = stark_binding_air_terms(
            backend,
            &outer.circuit_id,
            outer.vk_hash,
            &outer.public_inputs,
            &open.public_inputs,
        );
        let z_final = inner
            .proof
            .queries
            .first()
            .and_then(|chain| chain.last())
            .map(|decommit| decommit.z)
            .expect("generated STARK proof carries a final fold value");
        let mut leaf = stark_field_add_for_test(
            STARK_BINDING_AIR_CONSTANT,
            stark_field_mul_for_test(STARK_BINDING_AIR_Z_COEFF, z_final),
        );
        for term in &terms {
            leaf = stark_field_add_for_test(leaf, stark_field_mul_for_test(term.coeff, term.value));
        }
        let (comp_root, path) = crate::zk_stark::stark_merkle_root_and_path_from_field_values_v1(
            &inner.params,
            &[leaf],
            0,
        )
        .expect("derive auxiliary composition commitment");
        let comp_value = StarkCompositionValueV1 {
            leaf,
            constant: STARK_BINDING_AIR_CONSTANT,
            z_coeff: STARK_BINDING_AIR_Z_COEFF,
            aux_terms: terms,
            path,
        };
        inner.proof.commits.comp_root = Some(comp_root);
        inner.proof.comp_values = Some(vec![comp_value; inner.proof.queries.len()]);
        open.envelope_bytes =
            norito::to_bytes(&inner).expect("encode auxiliary inner STARK envelope");
        outer.proof_bytes = norito::to_bytes(&open).expect("encode auxiliary STARK open proof");
    }
    #[test]
    fn prove_stark_open_verify_envelope_emits_binding_air_proof() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
        assert!(report.ok);
    }
    #[test]
    fn prove_stark_open_verify_envelope_rejects_alternate_layout_verifying_key() {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:alternate-layout-vk");
        let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
        let canonical_vk =
            norito::encode_canonical(&vk_payload).expect("encode canonical STARK VK");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_vk = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&vk_payload).expect("encode alternate-layout STARK VK")
        };
        assert_ne!(alternate_vk, canonical_vk);
        norito::decode_from_bytes::<StarkFriVerifyingKeyV1>(&alternate_vk)
            .expect("ordinary Norito accepts the advertised layout");
        let vk_box = VerifyingKeyBox::new(backend.to_owned(), alternate_vk);
        let err = prove_stark_fri_open_verify_envelope(
            backend,
            &circuit_id,
            &vk_box,
            b"alternate-layout-vk:schema:v1",
            vec![vec![[0x11; 32]]],
        )
        .expect_err("alternate-layout STARK VK must be rejected before proving");
        assert!(
            err.contains("invalid STARK verifying key payload"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_alternate_layout_outer() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let outer: OpenVerifyEnvelope =
            norito::decode_canonical(&proof.bytes).expect("decode canonical outer");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_outer = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&outer).expect("encode alternate-layout outer envelope")
        };
        assert_ne!(alternate_outer, proof.bytes);
        norito::decode_from_bytes::<OpenVerifyEnvelope>(&alternate_outer)
            .expect("ordinary Norito accepts the advertised layout");
        let alternate_proof = ProofBox::new(backend.to_owned(), alternate_outer);
        assert!(
            !verify_backend_with_timing(backend, &alternate_proof, Some(&vk_box)).ok,
            "alternate-layout outer envelope must be rejected"
        );
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_alternate_layout_wrapper() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let mut outer: OpenVerifyEnvelope =
            norito::decode_canonical(&proof.bytes).expect("decode canonical outer");
        let open: StarkFriOpenProofV1 =
            norito::decode_canonical(&outer.proof_bytes).expect("decode canonical STARK wrapper");
        let canonical_wrapper = outer.proof_bytes.clone();
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_wrapper = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&open).expect("encode alternate-layout STARK wrapper")
        };
        assert_ne!(alternate_wrapper, canonical_wrapper);
        assert_eq!(
            norito::decode_from_bytes::<StarkFriOpenProofV1>(&alternate_wrapper)
                .expect("ordinary Norito accepts the advertised layout"),
            open
        );
        outer.proof_bytes = alternate_wrapper;
        let alternate_proof = ProofBox::new(
            backend.to_owned(),
            norito::encode_canonical(&outer)
                .expect("encode canonical outer around alternate wrapper"),
        );
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert!(
            verify_backend_with_timing(backend, &proof, Some(&vk_box)).ok,
            "canonical STARK proof must verify independently of ambient layout"
        );
        assert!(
            !verify_backend_with_timing(backend, &alternate_proof, Some(&vk_box)).ok,
            "alternate-layout STARK wrapper must be rejected inside a canonical outer envelope"
        );
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_bound_public_input_tampering() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let mut outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        let mut open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&outer.proof_bytes).expect("decode STARK open proof");
        open.public_inputs[0][0][0] ^= 0x01;
        outer.proof_bytes = norito::to_bytes(&open).expect("encode tampered STARK open proof");
        let tampered = ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&outer).expect("encode tampered outer STARK envelope"),
        );
        let report = verify_backend_with_timing(backend, &tampered, Some(&vk_box));
        assert!(!report.ok);
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_bound_schema_tampering() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let mut outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        outer.public_inputs.push(0xAA);
        let tampered = ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&outer).expect("encode tampered outer STARK envelope"),
        );
        let report = verify_backend_with_timing(backend, &tampered, Some(&vk_box));
        assert!(!report.ok);
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_noncanonical_binding_air_transcript_label() {
        let (backend, circuit_id, vk_box, _proof) = sample_stark_open_verify_proof();
        let proof = stark_open_verify_proof_with_transcript_label(
            backend,
            &circuit_id,
            &vk_box,
            b"tiny:schema:v1".to_vec(),
            vec![vec![[0x11; 32]], vec![[0x22; 32]]],
            "IROHA-STARK-AIR-V1-ALT",
        );
        let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
        assert!(
            !report.ok,
            "generic STARK OpenVerify wrappers must use the canonical binding AIR transcript label"
        );
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_vk_hash_tampering() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let mut outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        outer.vk_hash[0] ^= 0x01;
        let tampered = ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&outer).expect("encode tampered outer STARK envelope"),
        );
        let report = verify_backend_with_timing(backend, &tampered, Some(&vk_box));
        assert!(!report.ok);
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_noncanonical_outer_shape() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let cases: [(&str, fn(&mut OpenVerifyEnvelope)); 7] = [
            ("backend tag", |outer| {
                outer.backend = BackendTag::Halo2IpaPasta
            }),
            ("empty circuit id", |outer| outer.circuit_id.clear()),
            ("zero verifier-key hash", |outer| outer.vk_hash = [0u8; 32]),
            ("empty public inputs", |outer| outer.public_inputs.clear()),
            ("empty proof bytes", |outer| outer.proof_bytes.clear()),
            ("all-zero proof bytes", |outer| {
                outer.proof_bytes = vec![0u8; 16]
            }),
            ("auxiliary bytes", |outer| {
                outer.aux = b"side-channel".to_vec()
            }),
        ];
        for (case, mutate) in cases {
            let tampered = mutate_outer_stark_open_verify_proof(backend, &proof, mutate);
            let report = verify_backend_with_timing(backend, &tampered, Some(&vk_box));
            assert!(!report.ok, "case {case}");
        }
        let tampered = mutate_outer_stark_open_verify_proof(backend, &proof, |outer| {
            outer.public_inputs =
                vec![0xA5; iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_PUBLIC_INPUT_BYTES + 1];
        });
        let report = verify_backend_with_timing(backend, &tampered, Some(&vk_box));
        assert!(!report.ok, "oversized public inputs");
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_missing_vk() {
        let (backend, _circuit_id, _vk_box, proof) = sample_stark_open_verify_proof();
        let report = verify_backend_with_timing(backend, &proof, None);
        assert!(!report.ok);
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_inner_air_circuit_tampering() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let mut outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        let mut open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&outer.proof_bytes).expect("decode STARK open proof");
        let mut inner: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&open.envelope_bytes).expect("decode inner STARK envelope");
        inner
            .proof
            .air
            .as_mut()
            .expect("AIR section")
            .circuit_id
            .push_str(":tampered");
        open.envelope_bytes = norito::to_bytes(&inner).expect("encode tampered inner STARK proof");
        outer.proof_bytes = norito::to_bytes(&open).expect("encode tampered STARK open proof");
        let tampered = ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&outer).expect("encode tampered outer STARK envelope"),
        );
        let report = verify_backend_with_timing(backend, &tampered, Some(&vk_box));
        assert!(!report.ok);
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_inner_parameter_tampering() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let mut outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        let mut open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&outer.proof_bytes).expect("decode STARK open proof");
        let mut inner: StarkVerifyEnvelopeV1 =
            norito::decode_from_bytes(&open.envelope_bytes).expect("decode inner STARK envelope");
        inner.params.queries = inner.params.queries.saturating_add(1);
        open.envelope_bytes = norito::to_bytes(&inner).expect("encode tampered inner STARK proof");
        outer.proof_bytes = norito::to_bytes(&open).expect("encode tampered STARK open proof");
        let tampered = ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&outer).expect("encode tampered outer STARK envelope"),
        );
        let report = verify_backend_with_timing(backend, &tampered, Some(&vk_box));
        assert!(!report.ok);
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_inner_auxiliary_composition_commitments() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let mut outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        attach_valid_auxiliary_composition_to_open_verify_proof(backend, &mut outer);
        let tampered = ProofBox::new(
            backend.to_owned(),
            norito::to_bytes(&outer).expect("encode auxiliary outer STARK envelope"),
        );
        let report = verify_backend_with_timing(backend, &tampered, Some(&vk_box));
        assert!(!report.ok);
    }
    #[test]
    fn prove_stark_ivm_execution_envelope_emits_binding_air_proof() {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:ivm-execution-v1");
        let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
        let vk_bytes = norito::to_bytes(&vk_payload).expect("encode vk payload");
        let vk_box = VerifyingKeyBox::new(backend.to_owned(), vk_bytes);
        let proof = prove_stark_fri_ivm_execution_envelope(
            backend,
            &circuit_id,
            &vk_box,
            Hash::new(b"code"),
            Hash::new(b"overlay"),
            Hash::new(b"events"),
            Hash::new(b"gas"),
        )
        .expect("binding AIR STARK proof");
        let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
        assert!(report.ok);
    }
    #[test]
    fn prove_stark_ivm_execution_envelope_rejects_non_ivm_circuit_with_matching_vk() {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:not-ivm-execution-v1");
        let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
        let vk_box = VerifyingKeyBox::new(
            backend.to_owned(),
            norito::to_bytes(&vk_payload).expect("encode non-IVM STARK VK payload"),
        );
        let err = prove_stark_fri_ivm_execution_envelope(
            backend,
            &circuit_id,
            &vk_box,
            Hash::new(b"code"),
            Hash::new(b"overlay"),
            Hash::new(b"events"),
            Hash::new(b"gas"),
        )
        .expect_err("STARK IVM helper must reject non-IVM circuit ids even when the VK matches");
        assert!(
            err.contains("ivm-execution-v1"),
            "unexpected non-IVM circuit rejection: {err}"
        );
    }
    #[test]
    fn verify_stark_open_verify_envelope_rejects_malformed_payload_without_panic() {
        let backend = "stark/fri/sha256-goldilocks";
        let vk_payload = StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: format!("{backend}:tiny-open"),
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
        };
        let vk_bytes = norito::to_bytes(&vk_payload).expect("encode vk payload");
        let vk_box = VerifyingKeyBox::new(backend.to_owned(), vk_bytes);
        let malformed = ProofBox::new(backend.to_owned(), vec![0xAA, 0xBB, 0xCC]);
        let report = verify_backend_with_timing(backend, &malformed, Some(&vk_box));
        assert!(!report.ok);
    }
}
/// Result produced by [`verify_backend_with_timing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyReport {
    /// Outcome of the backend verification.
    pub ok: bool,
    /// Time spent verifying.
    pub elapsed: Duration,
}
const REJECTED_VERIFY_REPORT: VerifyReport = VerifyReport {
    ok: false,
    elapsed: Duration::ZERO,
};
/// Configuration guardrails for proof verification (enabled flags + payload size caps).
///
/// This struct is intentionally scalar-only so it can be sourced both from node configuration
/// (`iroha_config::parameters::actual::Zk`) and from host-local verification caps (e.g. IVM host).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZkVerifyGuardrails {
    /// Whether halo2 verification is enabled.
    pub halo2_enabled: bool,
    /// Maximum accepted halo2 envelope payload size (bytes).
    pub halo2_max_envelope_bytes: usize,
    /// Maximum accepted halo2 proof payload size (bytes).
    pub halo2_max_proof_bytes: usize,
    /// Whether STARK verification is enabled.
    pub stark_enabled: bool,
    /// Maximum accepted outer STARK OpenVerifyEnvelope size (bytes).
    pub stark_max_envelope_bytes: usize,
    /// Maximum accepted backend-native STARK proof payload size (bytes).
    pub stark_max_proof_bytes: usize,
}
impl ZkVerifyGuardrails {
    /// Build guardrails from node configuration.
    pub fn from_cfg(cfg: &iroha_config::parameters::actual::Zk) -> Self {
        Self {
            halo2_enabled: cfg.halo2.enabled,
            halo2_max_envelope_bytes: cfg.halo2.max_envelope_bytes,
            halo2_max_proof_bytes: cfg.halo2.max_proof_bytes,
            stark_enabled: cfg.stark.enabled,
            stark_max_envelope_bytes: cfg.stark.max_envelope_bytes,
            stark_max_proof_bytes: cfg.stark.max_proof_bytes,
        }
    }
}
/// Verify a backend and report the elapsed time.
pub fn verify_backend_with_timing(
    backend: &str,
    proof: &ProofBox,
    vk: Option<&VerifyingKeyBox>,
) -> VerifyReport {
    let started = Instant::now();
    let ok = verify_backend(backend, proof, vk);
    VerifyReport {
        ok,
        elapsed: started.elapsed(),
    }
}
/// Verify a backend under explicit configuration guardrails (enabled flags + payload size caps).
///
/// This helper exists to prevent accidentally accepting proofs for a backend that is
/// compiled in but disabled at runtime.
pub fn verify_backend_with_timing_guardrails(
    backend: &str,
    proof: &ProofBox,
    vk: Option<&VerifyingKeyBox>,
    guardrails: ZkVerifyGuardrails,
) -> VerifyReport {
    if is_production_claim_backend_label(backend) {
        iroha_logger::debug!(
            backend,
            "production-claim proof backends are not admitted by node verifier guardrails"
        );
        return REJECTED_VERIFY_REPORT;
    }
    if is_trusted_setup_backend_label(backend) {
        iroha_logger::debug!(
            backend,
            "trusted-setup proof backends are not admitted by node verifier guardrails"
        );
        return REJECTED_VERIFY_REPORT;
    }
    if is_developer_only_backend_label(backend) {
        iroha_logger::debug!(
            backend,
            "developer-only proof backends are not admitted by node verifier guardrails"
        );
        return REJECTED_VERIFY_REPORT;
    }
    if !is_production_verify_backend_label(backend) {
        iroha_logger::debug!(
            backend,
            "unsupported proof backends are not admitted by node verifier guardrails"
        );
        return REJECTED_VERIFY_REPORT;
    }
    if proof.backend.as_str() != backend {
        iroha_logger::debug!(
            backend,
            proof_backend = proof.backend.as_str(),
            "proof backend label does not match requested verifier backend"
        );
        return REJECTED_VERIFY_REPORT;
    }
    if let Some(vk_box) = vk
        && vk_box.backend.as_str() != backend
    {
        iroha_logger::debug!(
            backend,
            vk_backend = vk_box.backend.as_str(),
            "verifying key backend label does not match requested verifier backend"
        );
        return REJECTED_VERIFY_REPORT;
    }
    if production_verify_backend_tag(backend)
        == Some(iroha_data_model::zk::BackendTag::Halo2IpaPasta)
    {
        if !guardrails.halo2_enabled {
            iroha_logger::debug!(
                backend,
                "halo2 verification is disabled in node configuration"
            );
            return REJECTED_VERIFY_REPORT;
        }
        if proof.bytes.len() > guardrails.halo2_max_envelope_bytes {
            iroha_logger::debug!(
                backend,
                "halo2 payload exceeds node-configured max_envelope_bytes"
            );
            return REJECTED_VERIFY_REPORT;
        }
        // V1 Halo2 proof inputs are always canonical `OpenVerifyEnvelope` frames.
        // Raw backend-native payloads are accepted only after this boundary has
        // decoded and authenticated the outer envelope.
        let env = match norito::decode_canonical::<iroha_data_model::zk::OpenVerifyEnvelope>(
            &proof.bytes,
        ) {
            Ok(env) => env,
            Err(err) => {
                iroha_logger::debug!(
                    backend,
                    error = %err,
                    "halo2 proof payload is not a canonical OpenVerifyEnvelope"
                );
                return REJECTED_VERIFY_REPORT;
            }
        };
        if env.backend != iroha_data_model::zk::BackendTag::Halo2IpaPasta {
            iroha_logger::debug!(
                backend,
                "halo2 OpenVerifyEnvelope backend tag does not match verifier backend"
            );
            return REJECTED_VERIFY_REPORT;
        }
        if let Err(err) = env.validate_with_bounds(iroha_data_model::zk::OpenVerifyEnvelopeBounds {
            max_proof_bytes: guardrails.halo2_max_proof_bytes,
            ..iroha_data_model::zk::OpenVerifyEnvelopeBounds::default()
        }) {
            iroha_logger::debug!(
                backend,
                error = %err,
                "halo2 OpenVerifyEnvelope failed guardrail validation"
            );
            return REJECTED_VERIFY_REPORT;
        }
        if !halo2_open_verify_circuit_id_matches_backend(backend, &env.circuit_id) {
            iroha_logger::debug!(
                backend,
                circuit_id = env.circuit_id.as_str(),
                "halo2 OpenVerifyEnvelope circuit id does not match verifier backend"
            );
            return REJECTED_VERIFY_REPORT;
        }
        let Some(expected_schema) = halo2_ipa_public_inputs_schema_v1(&env.circuit_id) else {
            iroha_logger::debug!(
                backend,
                circuit_id = env.circuit_id.as_str(),
                "halo2 circuit has no canonical outer public-input schema"
            );
            return REJECTED_VERIFY_REPORT;
        };
        if env.public_inputs.as_slice() != expected_schema {
            iroha_logger::debug!(
                backend,
                circuit_id = env.circuit_id.as_str(),
                "halo2 OpenVerifyEnvelope public-input schema is not canonical"
            );
            return REJECTED_VERIFY_REPORT;
        }
    }
    if is_stark_fri_v1_backend(backend) {
        if !guardrails.stark_enabled {
            iroha_logger::debug!(
                backend,
                "stark verification is disabled in node configuration"
            );
            return REJECTED_VERIFY_REPORT;
        }
        if proof.bytes.len() > guardrails.stark_max_envelope_bytes {
            iroha_logger::debug!(
                backend,
                "stark payload exceeds node-configured max_envelope_bytes"
            );
            return REJECTED_VERIFY_REPORT;
        }
        let env = match norito::decode_canonical::<iroha_data_model::zk::OpenVerifyEnvelope>(
            &proof.bytes,
        ) {
            Ok(env) => env,
            Err(err) => {
                iroha_logger::debug!(
                    backend,
                    error = %err,
                    "stark proof payload is not an OpenVerifyEnvelope"
                );
                return REJECTED_VERIFY_REPORT;
            }
        };
        if env.backend != iroha_data_model::zk::BackendTag::Stark {
            iroha_logger::debug!(
                backend,
                "stark OpenVerifyEnvelope backend tag does not match verifier backend"
            );
            return REJECTED_VERIFY_REPORT;
        }
        if let Err(err) = env.validate_with_bounds(iroha_data_model::zk::OpenVerifyEnvelopeBounds {
            max_proof_bytes: guardrails.stark_max_envelope_bytes,
            ..iroha_data_model::zk::OpenVerifyEnvelopeBounds::default()
        }) {
            iroha_logger::debug!(
                backend,
                error = %err,
                "stark OpenVerifyEnvelope failed guardrail validation"
            );
            return REJECTED_VERIFY_REPORT;
        }
        if !stark_open_verify_circuit_id_matches_backend(backend, &env.circuit_id) {
            iroha_logger::debug!(
                backend,
                circuit_id = env.circuit_id.as_str(),
                "stark OpenVerifyEnvelope circuit id does not match verifier backend"
            );
            return REJECTED_VERIFY_REPORT;
        }
        let open = match norito::decode_canonical::<iroha_data_model::zk::StarkFriOpenProofV1>(
            &env.proof_bytes,
        ) {
            Ok(open) => open,
            Err(err) => {
                iroha_logger::debug!(
                    backend,
                    error = %err,
                    "stark OpenVerifyEnvelope wrapper payload is malformed"
                );
                return REJECTED_VERIFY_REPORT;
            }
        };
        if open.version != 1 {
            iroha_logger::debug!(
                backend,
                version = open.version,
                "stark OpenVerifyEnvelope wrapper version is unsupported"
            );
            return REJECTED_VERIFY_REPORT;
        }
        if open.envelope_bytes.is_empty() {
            iroha_logger::debug!(
                backend,
                "stark OpenVerifyEnvelope wrapper has empty native proof bytes"
            );
            return REJECTED_VERIFY_REPORT;
        }
        if open.envelope_bytes.len() > guardrails.stark_max_proof_bytes {
            iroha_logger::debug!(
                backend,
                "stark envelope proof bytes exceed node-configured max_proof_bytes"
            );
            return REJECTED_VERIFY_REPORT;
        }
        #[cfg(feature = "zk-stark")]
        {
            let started = Instant::now();
            let mut limits = crate::zk_stark::StarkVerifierLimits::default();
            limits.max_envelope_bytes = guardrails.stark_max_proof_bytes;
            let ok = verify_stark_fri_open_verify_envelope_with_limits(backend, proof, vk, &limits);
            return VerifyReport {
                ok,
                elapsed: started.elapsed(),
            };
        }
        #[cfg(not(feature = "zk-stark"))]
        {
            iroha_logger::debug!(
                backend,
                "stark/fri backend requested but binary was built without `zk-stark`"
            );
            return REJECTED_VERIFY_REPORT;
        }
    }
    verify_backend_with_timing(backend, proof, vk)
}
/// Verify a backend under node configuration guardrails (enabled flags + payload size caps).
///
/// This helper exists to prevent accidentally accepting proofs for a backend that is
/// compiled in but disabled at runtime.
pub fn verify_backend_with_timing_checked(
    backend: &str,
    proof: &ProofBox,
    vk: Option<&VerifyingKeyBox>,
    cfg: &iroha_config::parameters::actual::Zk,
) -> VerifyReport {
    verify_backend_with_timing_guardrails(backend, proof, vk, ZkVerifyGuardrails::from_cfg(cfg))
}
#[cfg(test)]
mod guardrails_tests {
    use super::*;
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};
    const ENABLED_GUARDRAILS: ZkVerifyGuardrails = ZkVerifyGuardrails {
        halo2_enabled: true,
        halo2_max_envelope_bytes: 1024,
        halo2_max_proof_bytes: 1024,
        stark_enabled: true,
        stark_max_envelope_bytes: 1024,
        stark_max_proof_bytes: 1024,
    };
    macro_rules! assert_guardrails_reject {
        ($backend:expr, $proof:expr, $vk:expr, $guardrails:expr $(, $message:literal)? $(,)?) => {{
            let report =
                verify_backend_with_timing_guardrails($backend, $proof, $vk, $guardrails);
            assert!(!report.ok $(, $message)?);
            assert_eq!(report.elapsed, Duration::ZERO $(, $message)?);
        }};
    }
    fn halo2_guardrail_envelope() -> OpenVerifyEnvelope {
        OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: IVM_EXECUTION_V1_CIRCUIT_ID.to_owned(),
            vk_hash: [0x11; 32],
            public_inputs: IVM_EXECUTION_PUBLIC_INPUTS_SCHEMA_V1.to_vec(),
            proof_bytes: vec![0xBB; 10],
            aux: Vec::new(),
        }
    }
    #[test]
    fn guardrails_disable_halo2_returns_zero_duration() {
        let proof = ProofBox::new("halo2/ipa".into(), vec![0xAA; 8]);
        assert_guardrails_reject!(
            "halo2/ipa",
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_enabled: false,
                ..ENABLED_GUARDRAILS
            },
        );
    }
    #[test]
    fn guardrails_reject_trusted_setup_backends_before_dispatch() {
        for backend in [
            "kzg",
            "KZG",
            " kzg ",
            "bn254",
            "BN254",
            "bls12_381",
            "halo2/kzg",
            "halo2/ipa:kzg",
            "halo2/ipa:KZG",
            "halo2/ipa: KZG",
            "stark/fri/prod;kzg",
            "stark/fri/prod,kzg",
            "stark/fri/prod+kzg",
            "stark/fri/prod.kzg",
            "stark/fri/prod-k-z-g",
            "stark/fri/prod(kzg)",
            "stark/fri/prod;bn254",
            "stark/fri/prod-bn-254",
            "stark/fri/prod+bn256",
            "stark/fri/prod-bn-256",
            "stark/fri/prod-bls12-381",
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
            "halo2/ipa/orchard:kzg",
            "orchard:universal-srs",
            "penumbra-masp:kzg",
            "jindo-lattice-pcs-zk:trusted-setup",
            "miden-stark:ptau",
            "sis-with-hints:groth16",
            "pq-masp-stark-fri:kzg",
            "halo2/bn254",
            "groth16/bn254",
        ] {
            let proof = ProofBox::new(backend.into(), vec![1, 2, 3]);
            assert_guardrails_reject!(backend, &proof, None, ENABLED_GUARDRAILS, "case {backend}",);
        }
    }
    #[test]
    fn guardrails_reject_developer_only_backends_before_dispatch() {
        for backend in [
            "debug",
            "debug-proof",
            "Debug-Proof",
            "debug/ok",
            "halo2/debug",
            "halo2/ipa:debug-proof",
            "halo2/ipa:DEBUG-Proof",
            "halo2/ipa:d-e-b-u-g-proof",
            "stark/fri/debug",
            "stark/fri/Debug",
            "stark/fri/d-e-b-u-g",
            "mock",
            "mock-proof",
            "Mock-Proof",
            "halo2/mock",
            "halo2/ipa:mock-proof",
            "halo2/ipa:Mock-Proof",
            "halo2/ipa:m-o-c-k-proof",
            "stark/fri/m-o-c-k",
            "stark/fri/dev-fixture",
            "stark/fri/d-e-v-f-i-x-t-u-r-e",
            "stark/fri/dev",
            "stark/fri/d-e-v",
            "stark/fri/test",
            "stark/fri/t-e-s-t",
            "stark/fri/placeholder",
            "halo2/ipa:dev-fixture",
            "halo2/ipa:d-e-v-f-i-x-t-u-r-e",
            "halo2/ipa:dev",
            "halo2/ipa:d-e-v",
            "halo2/ipa:dummy",
            "halo2/ipa:f-a-k-e",
            "halo2/ipa:stub",
            "halo2/ipa:s-a-m-p-l-e",
            "zk-trace/mock-proof",
        ] {
            let proof = ProofBox::new(backend.into(), vec![1, 2, 3]);
            assert_guardrails_reject!(backend, &proof, None, ENABLED_GUARDRAILS, "case {backend}",);
        }
    }
    #[test]
    fn guardrails_reject_protocol_names_before_dispatch() {
        for backend in [
            "halo2/ipa/orchard",
            "stark/fri/miden",
            "stark/fri/pq-masp-stark-fri",
            "groth16/bls12-377",
            "anonymous-pgc",
            "verange",
            "zk-ams-recursive-admission-v0",
            "zk-x509-onchain-identity-v0",
            "sis-hints-anoncred-pq-v0",
            "sis-with-hints",
        ] {
            let proof = ProofBox::new(backend.into(), vec![1, 2, 3]);
            assert_guardrails_reject!(backend, &proof, None, ENABLED_GUARDRAILS, "case {backend}",);
        }
    }
    #[test]
    fn guardrails_reject_production_claim_backends_before_dispatch() {
        for backend in [
            "halo2/ipa:production-ready",
            "halo2/ipa:claimed-production",
            "halo2/ipa:mainnet-ready",
            "halo2/ipa:mainnet-complete",
            "stark/fri/audit-signoff",
            "stark/fri/externally-audited",
            "stark/fri/security-review-passed",
            "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
            "stark/fri/a-u-d-i-t-c-l-a-i-m",
            "halo2/ipa:release-ready",
            "halo2/ipa:release-approved",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa:third-party-audited",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
        ] {
            let proof = ProofBox::new(backend.into(), vec![1, 2, 3]);
            assert_guardrails_reject!(backend, &proof, None, ENABLED_GUARDRAILS, "case {backend}",);
        }
    }
    #[test]
    fn guardrails_reject_unsupported_backends_before_dispatch() {
        for backend in [
            "unknown/privacy/backend",
            "halo2/unknown-native-v1",
            "halo2/ipa:unknown-native-v1",
            "HALO2/IPA",
            "stark/FRI",
            "halo2/ipa::ivm-execution-v1",
            "halo2//ipa",
            "halo2/ipa.",
            "stark//fri/sha256-goldilocks",
            "stark/fri/sha256..goldilocks",
            "h\u{0430}lo2/ipa",
            "halo2/pasta/tiny-add",
            "halo2/ipa/tiny-add",
            "halo2/ipa:tiny-add",
            "halo2/pasta/tiny-commit-open",
            "zk/open-verify-unregistered",
        ] {
            let proof = ProofBox::new(backend.into(), vec![1, 2, 3]);
            assert_guardrails_reject!(backend, &proof, None, ENABLED_GUARDRAILS, "case {backend}",);
        }
    }
    #[test]
    fn guardrails_reject_proof_and_vk_backend_mismatch_before_dispatch() {
        let envelope_bytes =
            norito::to_bytes(&halo2_guardrail_envelope()).expect("encode halo2 envelope");
        let wrong_proof_backend =
            ProofBox::new("halo2/ipa:ivm-execution-v1".into(), envelope_bytes);
        assert_guardrails_reject!("halo2/ipa", &wrong_proof_backend, None, ENABLED_GUARDRAILS,);
        let proof = ProofBox::new(
            "halo2/ipa".into(),
            norito::to_bytes(&halo2_guardrail_envelope()).expect("encode halo2 envelope"),
        );
        let wrong_vk_backend =
            VerifyingKeyBox::new("halo2/ipa:ivm-execution-v1".into(), vec![0x55]);
        assert_guardrails_reject!(
            "halo2/ipa",
            &proof,
            Some(&wrong_vk_backend),
            ENABLED_GUARDRAILS,
        );
    }
    #[test]
    fn guardrails_reject_halo2_open_verify_circuit_mismatch_before_dispatch() {
        for (case, backend, circuit_id) in [
            (
                "concrete backend with sibling circuit",
                "halo2/pasta/ivm-execution-v1",
                "halo2/pasta/tiny-add-public",
            ),
            (
                "concrete backend with aliased sibling circuit",
                "halo2/ipa:ivm-execution-v1",
                "halo2/ipa:tiny-add-public",
            ),
            (
                "generic halo2 backend with cross-family circuit",
                "halo2/ipa",
                "stark/fri/sha256-goldilocks:spoof",
            ),
            (
                "generic halo2 backend with tiny demo circuit",
                "halo2/ipa",
                "halo2/ipa:tiny-add",
            ),
            (
                "generic halo2 backend with anonymous-transfer demo circuit",
                "halo2/ipa",
                "halo2/pasta/anon-transfer-2x2",
            ),
            (
                "generic halo2 backend with retired vote circuit",
                "halo2/ipa",
                "halo2/ipa:vote-bool-commit-merkle8",
            ),
            (
                "generic halo2 backend with historical IVM overlay circuit",
                "halo2/ipa",
                "halo2/ipa:ivm-overlay-bind",
            ),
            (
                "generic halo2 backend with retired recursive-spend circuit",
                "halo2/ipa",
                "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
            ),
            (
                "generic halo2 backend with bare trusted-setup circuit",
                "halo2/ipa",
                "kzg",
            ),
            (
                "generic halo2 backend with prefixed trusted-setup circuit",
                "halo2/ipa",
                "halo2/ipa:kzg",
            ),
            (
                "generic halo2 backend with prefixed STARK circuit",
                "halo2/ipa",
                "halo2/ipa:stark/fri",
            ),
        ] {
            let mut env = halo2_guardrail_envelope();
            env.circuit_id = circuit_id.to_owned();
            let proof = ProofBox::new(
                backend.to_owned(),
                norito::to_bytes(&env).expect("encode halo2 envelope"),
            );
            assert_guardrails_reject!(backend, &proof, None, ENABLED_GUARDRAILS, "case {case}",);
        }
    }
    #[test]
    fn guardrails_enforce_halo2_max_envelope_bytes() {
        let proof = ProofBox::new("halo2/ipa".into(), vec![0xAA; 9]);
        assert_guardrails_reject!(
            "halo2/ipa",
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_max_envelope_bytes: 8,
                ..ENABLED_GUARDRAILS
            },
        );
    }
    #[test]
    fn guardrails_enforce_halo2_max_proof_bytes_for_open_verify_envelopes() {
        let env = halo2_guardrail_envelope();
        let bytes = norito::to_bytes(&env).expect("encode envelope");
        let proof = ProofBox::new("halo2/ipa".into(), bytes);
        assert_guardrails_reject!(
            "halo2/ipa",
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_max_proof_bytes: 5,
                ..ENABLED_GUARDRAILS
            },
        );
    }
    #[test]
    fn guardrails_reject_open_verify_shape_failures_before_dispatch() {
        let cases: [(&str, fn(&mut OpenVerifyEnvelope)); 6] = [
            ("empty circuit id", |env| env.circuit_id.clear()),
            ("zero verifier-key hash", |env| env.vk_hash = [0u8; 32]),
            ("empty public inputs", |env| env.public_inputs.clear()),
            ("wrong nonzero public-input schema", |env| {
                env.public_inputs = b"noncanonical-but-nonzero-schema".to_vec()
            }),
            ("empty proof bytes", |env| env.proof_bytes.clear()),
            ("auxiliary bytes", |env| env.aux = b"ignored-hint".to_vec()),
        ];
        for (label, mutate) in cases {
            let mut env = halo2_guardrail_envelope();
            mutate(&mut env);
            let proof = ProofBox::new(
                "halo2/ipa".into(),
                norito::to_bytes(&env).expect("encode envelope"),
            );
            assert_guardrails_reject!(
                "halo2/ipa",
                &proof,
                None,
                ENABLED_GUARDRAILS,
                "case {label}",
            );
        }
        let mut env = halo2_guardrail_envelope();
        env.public_inputs =
            vec![0xA5; iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_PUBLIC_INPUT_BYTES + 1];
        let proof = ProofBox::new(
            "halo2/ipa".into(),
            norito::to_bytes(&env).expect("encode envelope"),
        );
        assert_guardrails_reject!(
            "halo2/ipa",
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_max_envelope_bytes: usize::MAX,
                halo2_max_proof_bytes: usize::MAX,
                stark_max_envelope_bytes: usize::MAX,
                stark_max_proof_bytes: usize::MAX,
                ..ENABLED_GUARDRAILS
            },
            "oversized public inputs",
        );
    }
    #[test]
    fn guardrails_reject_open_verify_backend_tag_mismatch_before_dispatch() {
        let mut halo2_env = halo2_guardrail_envelope();
        halo2_env.backend = BackendTag::Stark;
        let halo2_proof = ProofBox::new(
            "halo2/ipa".into(),
            norito::to_bytes(&halo2_env).expect("encode mismatched halo2 envelope"),
        );
        assert_guardrails_reject!("halo2/ipa", &halo2_proof, None, ENABLED_GUARDRAILS,);
        let open = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: Vec::new(),
            envelope_bytes: vec![0xCC; 10],
        };
        let stark_env = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "stark/fri/sha256-goldilocks:dummy".to_owned(),
            vk_hash: [0x11; 32],
            public_inputs: vec![0xAA; 32],
            proof_bytes: norito::to_bytes(&open).expect("encode stark wrapper"),
            aux: Vec::new(),
        };
        let stark_proof = ProofBox::new(
            ZK_BACKEND_STARK_FRI_V1.into(),
            norito::to_bytes(&stark_env).expect("encode mismatched stark envelope"),
        );
        assert_guardrails_reject!(
            ZK_BACKEND_STARK_FRI_V1,
            &stark_proof,
            None,
            ENABLED_GUARDRAILS,
        );
    }
    #[test]
    fn guardrails_disable_stark_returns_zero_duration() {
        let proof = ProofBox::new(ZK_BACKEND_STARK_FRI_V1.into(), vec![0xAA; 8]);
        assert_guardrails_reject!(
            ZK_BACKEND_STARK_FRI_V1,
            &proof,
            None,
            ZkVerifyGuardrails {
                stark_enabled: false,
                ..ENABLED_GUARDRAILS
            },
        );
    }
    #[test]
    fn guardrails_enforce_stark_max_envelope_bytes() {
        let proof = ProofBox::new(ZK_BACKEND_STARK_FRI_V1.into(), vec![0xAA; 9]);
        assert_guardrails_reject!(
            ZK_BACKEND_STARK_FRI_V1,
            &proof,
            None,
            ZkVerifyGuardrails {
                stark_max_envelope_bytes: 8,
                ..ENABLED_GUARDRAILS
            },
        );
    }
    #[test]
    fn guardrails_reject_malformed_stark_outer_envelope_before_dispatch() {
        let proof = ProofBox::new(ZK_BACKEND_STARK_FRI_V1.into(), vec![0xAA, 0xBB, 0xCC]);
        assert_guardrails_reject!(ZK_BACKEND_STARK_FRI_V1, &proof, None, ENABLED_GUARDRAILS,);
    }
    #[test]
    fn guardrails_enforce_stark_max_proof_bytes_inside_open_verify_envelope() {
        let open = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: Vec::new(),
            envelope_bytes: vec![0xCC; 10],
        };
        let env = OpenVerifyEnvelope {
            backend: BackendTag::Stark,
            circuit_id: "stark/fri/sha256-goldilocks:dummy".to_owned(),
            vk_hash: [0x11; 32],
            public_inputs: vec![0xAA; 32],
            proof_bytes: norito::to_bytes(&open).expect("encode stark wrapper"),
            aux: Vec::new(),
        };
        let proof = ProofBox::new(
            ZK_BACKEND_STARK_FRI_V1.into(),
            norito::to_bytes(&env).expect("encode envelope"),
        );
        assert_guardrails_reject!(
            ZK_BACKEND_STARK_FRI_V1,
            &proof,
            None,
            ZkVerifyGuardrails {
                stark_max_proof_bytes: 8,
                ..ENABLED_GUARDRAILS
            },
        );
    }
    #[test]
    fn guardrails_reject_malformed_stark_wrapper_before_dispatch() {
        let cases = [
            ("malformed wrapper bytes", vec![0xAA, 0xBB, 0xCC]),
            (
                "unsupported wrapper version",
                norito::to_bytes(&StarkFriOpenProofV1 {
                    version: 2,
                    public_inputs: Vec::new(),
                    envelope_bytes: vec![0xCC; 10],
                })
                .expect("encode unsupported STARK wrapper"),
            ),
            (
                "empty native proof bytes",
                norito::to_bytes(&StarkFriOpenProofV1 {
                    version: 1,
                    public_inputs: Vec::new(),
                    envelope_bytes: Vec::new(),
                })
                .expect("encode empty STARK wrapper"),
            ),
        ];
        for (case, proof_bytes) in cases {
            let env = OpenVerifyEnvelope {
                backend: BackendTag::Stark,
                circuit_id: "stark/fri/sha256-goldilocks:dummy".to_owned(),
                vk_hash: [0x11; 32],
                public_inputs: vec![0xAA; 32],
                proof_bytes,
                aux: Vec::new(),
            };
            let proof = ProofBox::new(
                ZK_BACKEND_STARK_FRI_V1.into(),
                norito::to_bytes(&env).expect("encode envelope"),
            );
            assert_guardrails_reject!(
                ZK_BACKEND_STARK_FRI_V1,
                &proof,
                None,
                ENABLED_GUARDRAILS,
                "case {case}",
            );
        }
    }
    #[test]
    fn guardrails_reject_stark_open_verify_circuit_mismatch_before_dispatch() {
        let open = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: Vec::new(),
            envelope_bytes: vec![0xCC; 10],
        };
        for (case, backend, circuit_id) in [
            (
                "profile backend with sibling STARK profile",
                "stark/fri/sha256-goldilocks",
                "stark/fri/poseidon2-goldilocks:dummy",
            ),
            (
                "profile backend with generic STARK prefix",
                "stark/fri/sha256-goldilocks",
                "stark/fri:dummy",
            ),
            (
                "generic STARK backend with halo2 circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "halo2/ipa:ivm-execution-v1",
            ),
            (
                "generic STARK backend with colon-form halo2 circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "halo2:ivm-execution-v1",
            ),
            (
                "generic STARK backend with colon-form kzg circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "kzg:trusted-setup-spoof",
            ),
            (
                "generic STARK backend with bare trusted-setup curve circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "bn254",
            ),
            (
                "generic STARK backend with STARK-prefixed trusted-setup circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "stark/fri:universal-srs",
            ),
            (
                "profile backend with profile-prefixed trusted-setup circuit",
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:structured-reference-string",
            ),
        ] {
            let env = OpenVerifyEnvelope {
                backend: BackendTag::Stark,
                circuit_id: circuit_id.to_owned(),
                vk_hash: [0x11; 32],
                public_inputs: vec![0xAA; 32],
                proof_bytes: norito::to_bytes(&open).expect("encode stark wrapper"),
                aux: Vec::new(),
            };
            let proof = ProofBox::new(
                backend.to_owned(),
                norito::to_bytes(&env).expect("encode envelope"),
            );
            assert_guardrails_reject!(backend, &proof, None, ENABLED_GUARDRAILS, "case {case}",);
        }
    }
    #[cfg(feature = "zk-stark")]
    #[test]
    fn guardrails_stark_proof_limit_applies_to_inner_envelope_not_outer_wrapper() {
        use crate::zk_stark::STARK_HASH_SHA256_V1;
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:guardrail-split");
        let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
        let vk_box = VerifyingKeyBox::new(
            backend.to_owned(),
            norito::to_bytes(&vk_payload).expect("encode STARK verifying key"),
        );
        let proof = prove_stark_fri_open_verify_envelope(
            backend,
            &circuit_id,
            &vk_box,
            b"guardrail:schema:v1",
            vec![vec![[0x11; 32]], vec![[0x22; 32]]],
        )
        .expect("STARK OpenVerify proof");
        let outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        let open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&outer.proof_bytes).expect("decode STARK open proof");
        assert!(
            proof.bytes.len() > open.envelope_bytes.len(),
            "outer wrapper should be larger than the native STARK proof bytes"
        );
        let report = verify_backend_with_timing_guardrails(
            backend,
            &proof,
            Some(&vk_box),
            ZkVerifyGuardrails {
                stark_max_envelope_bytes: proof.bytes.len(),
                stark_max_proof_bytes: open.envelope_bytes.len(),
                ..ENABLED_GUARDRAILS
            },
        );
        assert!(report.ok);
    }
    #[cfg(feature = "zk-stark")]
    #[test]
    fn guardrails_reject_stark_proof_backend_alias_mismatch_before_dispatch() {
        use crate::zk_stark::STARK_HASH_SHA256_V1;
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:guardrail-backend-mismatch");
        let vk_payload = consensus_stark_vk!(circuit_id.clone(), STARK_HASH_SHA256_V1);
        let vk_box = VerifyingKeyBox::new(
            backend.to_owned(),
            norito::to_bytes(&vk_payload).expect("encode STARK verifying key"),
        );
        let mut proof = prove_stark_fri_open_verify_envelope(
            backend,
            &circuit_id,
            &vk_box,
            b"guardrail:schema:v1",
            vec![vec![[0x11; 32]], vec![[0x22; 32]]],
        )
        .expect("STARK OpenVerify proof");
        let outer: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("decode outer STARK envelope");
        let open: StarkFriOpenProofV1 =
            norito::decode_from_bytes(&outer.proof_bytes).expect("decode STARK open proof");
        proof.backend = ZK_BACKEND_STARK_FRI_V1.into();
        assert_guardrails_reject!(
            backend,
            &proof,
            Some(&vk_box),
            ZkVerifyGuardrails {
                stark_max_envelope_bytes: proof.bytes.len(),
                stark_max_proof_bytes: open.envelope_bytes.len(),
                ..ENABLED_GUARDRAILS
            },
        );
    }
}
#[cfg(test)]
mod halo2_ipa_alias_tests {
    use super::*;
    use iroha_data_model::privacy::{PRIVACY_RETIRED_PROTOCOL_LABELS_V1, PrivacyProtocolIdV1};
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope};
    #[test]
    fn halo2_ipa_circuit_id_maps_to_pasta_backend() {
        assert_eq!(
            normalize_halo2_ipa_circuit_id("halo2/ipa:tiny-add").as_deref(),
            Some("halo2/pasta/ipa/tiny-add")
        );
        assert_eq!(
            normalize_halo2_ipa_circuit_id("halo2/pasta/tiny-add").as_deref(),
            Some("halo2/pasta/ipa/tiny-add")
        );
        assert_eq!(
            normalize_halo2_ipa_circuit_id("halo2/pasta/ipa/tiny-add").as_deref(),
            Some("halo2/pasta/ipa/tiny-add")
        );
        assert_eq!(
            normalize_halo2_ipa_circuit_id("tiny-add").as_deref(),
            Some("halo2/pasta/ipa/tiny-add")
        );
        assert!(normalize_halo2_ipa_circuit_id("").is_none());
        assert!(normalize_halo2_ipa_circuit_id("halo2/ipa").is_none());
        assert!(normalize_halo2_ipa_circuit_id("halo2/pasta").is_none());
        assert!(normalize_halo2_ipa_circuit_id("halo2/pasta/ipa").is_none());
        assert!(
            normalize_halo2_ipa_circuit_id(
                &"a".repeat(iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES + 1)
            )
            .is_none()
        );
    }
    #[test]
    fn halo2_backend_mapping_rejects_every_reserved_privacy_label() {
        let assert_reserved = |label: &str| {
            for circuit_id in [
                label.to_owned(),
                format!("halo2/ipa::{label}"),
                format!("halo2/pasta/{label}"),
                format!("generic/namespace/{label}"),
            ] {
                assert!(
                    normalize_halo2_ipa_circuit_id(&circuit_id).is_none(),
                    "reserved privacy circuit id {circuit_id:?} must not map to Halo2"
                );
            }
            for near_miss in [format!("generic-{label}"), format!("{label}-generic")] {
                assert!(
                    normalize_halo2_ipa_circuit_id(&near_miss).is_some(),
                    "portable near miss {near_miss:?} must remain mappable"
                );
            }
        };
        for protocol in PrivacyProtocolIdV1::ALL {
            assert_reserved(protocol.canonical_label());
        }
        for label in PRIVACY_RETIRED_PROTOCOL_LABELS_V1 {
            assert_reserved(label);
        }
    }
    #[test]
    fn halo2_open_verify_circuit_id_uses_closed_production_registry() {
        for circuit_id in [
            "ivm-execution-v1",
            "halo2/ipa:ivm-execution-v1",
            "halo2/pasta/kaigi-roster-v1",
            "halo2/pasta/ipa/kaigi-usage-v1",
            "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
            "halo2/pasta/ipa/confidential-unshield-full-merkle16-axiom-poseidon-v3",
            "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
            "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
        ] {
            assert!(
                halo2_open_verify_circuit_id_is_production_v1(circuit_id),
                "production circuit id {circuit_id} must be admitted"
            );
        }
        for circuit_id in [
            "tiny-add",
            "halo2/ipa:tiny-add",
            "halo2/pasta/anon-transfer-2x2",
            "halo2/ipa:vote-bool-commit-merkle8",
            "halo2/ipa:ivm-overlay-bind",
            "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
            "kzg",
            "k-z-g",
            "groth16",
            "bn254",
            "halo2/ipa:kzg",
            "halo2/ipa:groth16",
            "halo2/ipa:stark/fri",
            "halo2/pasta/kzg",
            "stark",
            "stark/fri/sha256-goldilocks",
        ] {
            assert!(
                !halo2_open_verify_circuit_id_is_production_v1(circuit_id),
                "unregistered circuit id {circuit_id} must not be admitted as Halo2"
            );
        }
    }
    #[test]
    fn halo2_open_verify_circuit_registry_covers_each_exact_halo2_backend() {
        for backend in iroha_data_model::zk::ZK_VERIFIER_BACKEND_REGISTRY_LABELS_V1
            .iter()
            .copied()
            .filter(|backend| {
                *backend != ZK_BACKEND_HALO2_IPA
                    && verifier_backend_registry_tag_v1(backend)
                        == Some(iroha_data_model::zk::BackendTag::Halo2IpaPasta)
            })
        {
            assert!(
                halo2_open_verify_circuit_id_matches_backend(ZK_BACKEND_HALO2_IPA, backend),
                "generic Halo2 entry point must admit exact production circuit {backend}"
            );
            assert!(
                halo2_open_verify_circuit_id_matches_backend(backend, backend),
                "concrete Halo2 backend must admit only its own production circuit {backend}"
            );
        }
    }
    #[cfg(all(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[test]
    fn every_production_halo2_circuit_has_one_canonical_outer_schema() {
        for circuit_id in HALO2_IPA_PRODUCTION_CIRCUIT_IDS_V1 {
            let schema = halo2_ipa_public_inputs_schema_v1(circuit_id)
                .unwrap_or_else(|| panic!("missing canonical schema for {circuit_id}"));
            assert!(
                !schema.is_empty(),
                "empty canonical schema for {circuit_id}"
            );
        }
        for backend in iroha_data_model::zk::ZK_VERIFIER_BACKEND_REGISTRY_LABELS_V1
            .iter()
            .copied()
            .filter(|backend| {
                verifier_backend_registry_tag_v1(backend)
                    == Some(iroha_data_model::zk::BackendTag::Halo2IpaPasta)
                    && *backend != ZK_BACKEND_HALO2_IPA
            })
        {
            let canonical = normalize_halo2_ipa_circuit_id(backend)
                .unwrap_or_else(|| panic!("failed to normalize exact Halo2 backend {backend}"));
            assert_eq!(
                halo2_ipa_public_inputs_schema_v1(backend),
                halo2_ipa_public_inputs_schema_v1(&canonical),
                "exact Halo2 backend and canonical circuit must select the same schema"
            );
        }
        for alias in [
            IVM_EXECUTION_V1_CIRCUIT_ID,
            "halo2/ipa:ivm-execution-v1",
            IVM_EXECUTION_V1_HALO2_BACKEND,
            IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        ] {
            assert_eq!(
                halo2_ipa_public_inputs_schema_v1(alias),
                Some(IVM_EXECUTION_PUBLIC_INPUTS_SCHEMA_V1),
                "generic and exact Halo2 normalization must select the same schema for {alias}"
            );
        }
    }
    #[test]
    fn halo2_ipa_rejects_missing_vk() {
        let env = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: IVM_EXECUTION_V1_CIRCUIT_ID.into(),
            vk_hash: [0u8; 32],
            public_inputs: Vec::new(),
            proof_bytes: vec![0xAA, 0xBB],
            aux: Vec::new(),
        };
        let proof_bytes = norito::to_bytes(&env).expect("encode envelope");
        let proof = ProofBox::new("halo2/ipa".into(), proof_bytes);
        assert!(!verify_backend("halo2/ipa", &proof, None));
    }
    #[test]
    fn verifier_rejects_proof_backend_mismatch_before_dispatch() {
        let env = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: IVM_EXECUTION_V1_CIRCUIT_ID.into(),
            vk_hash: [0x42; 32],
            public_inputs: Vec::new(),
            proof_bytes: vec![0xAA, 0xBB],
            aux: Vec::new(),
        };
        let proof_bytes = norito::to_bytes(&env).expect("encode envelope");
        let proof = ProofBox::new("halo2/ipa/other".into(), proof_bytes);
        let vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![0xCC, 0xDD]);
        assert!(!verify_backend("halo2/ipa", &proof, Some(&vk)));
    }
    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn halo2_ipa_rejects_noncanonical_outer_shape_before_backend_verify() {
        let vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![0xCC, 0xDD]);
        let cases: [(&str, fn(&mut OpenVerifyEnvelope)); 6] = [
            ("backend tag", |env| env.backend = BackendTag::Stark),
            ("empty circuit id", |env| env.circuit_id.clear()),
            ("zero verifier-key hash", |env| env.vk_hash = [0u8; 32]),
            ("empty public inputs", |env| env.public_inputs.clear()),
            ("empty proof bytes", |env| env.proof_bytes.clear()),
            ("auxiliary bytes", |env| env.aux = b"side-channel".to_vec()),
        ];
        for (case, mutate) in cases {
            let mut env = OpenVerifyEnvelope {
                backend: BackendTag::Halo2IpaPasta,
                circuit_id: IVM_EXECUTION_V1_CIRCUIT_ID.into(),
                vk_hash: hash_vk(&vk),
                public_inputs: vec![0xA5],
                proof_bytes: vec![0xAA, 0xBB],
                aux: Vec::new(),
            };
            mutate(&mut env);
            let proof = ProofBox::new(
                "halo2/ipa".into(),
                norito::to_bytes(&env).expect("encode envelope"),
            );
            assert!(
                !verify_backend("halo2/ipa", &proof, Some(&vk)),
                "case {case}"
            );
        }
        let oversized = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: IVM_EXECUTION_V1_CIRCUIT_ID.into(),
            vk_hash: hash_vk(&vk),
            public_inputs: vec![
                0xA5;
                iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_PUBLIC_INPUT_BYTES + 1
            ],
            proof_bytes: vec![0xAA, 0xBB],
            aux: Vec::new(),
        };
        let proof = ProofBox::new(
            "halo2/ipa".into(),
            norito::to_bytes(&oversized).expect("encode envelope"),
        );
        assert!(
            !verify_backend("halo2/ipa", &proof, Some(&vk)),
            "oversized public inputs"
        );
    }
}
#[cfg(all(test, feature = "zk-halo2-ipa"))]
mod halo2_ipa_proving_key_archive_tests {
    use super::*;
    #[test]
    fn ivm_execution_prover_rejects_wrong_circuit_family() {
        let vk_box = halo2_ipa_ivm_execution_vk_box().expect("ivm execution verifying key");
        ensure_halo2_ipa_ivm_execution_canonical_vk_box(&vk_box)
            .expect("generated IVM verifier key must have canonical parameter provenance");
        let err = prove_halo2_ipa_ivm_execution_envelope(
            "halo2/ipa:not-ivm-execution-v1",
            &vk_box,
            iroha_crypto::Hash::new(b"code"),
            iroha_crypto::Hash::new(b"overlay"),
            iroha_crypto::Hash::new(b"events"),
            iroha_crypto::Hash::new(b"gas"),
            None,
        )
        .expect_err("wrong IVM circuit family must reject before proving");
        assert!(
            err.contains("unsupported IVM execution circuit id"),
            "unexpected circuit id error: {err}"
        );
    }
    #[test]
    fn halo2_ipa_proving_key_archive_binds_family_and_verifier_commitment() {
        let vk_commitment = [0x42; 32];
        let archive =
            encode_halo2_ipa_proving_key_archive("proof-family-a", vk_commitment, vec![1, 2])
                .expect("encode proving key archive");
        assert_eq!(
            decode_halo2_ipa_proving_key_archive(&archive, "proof-family-a", vk_commitment)
                .expect("decode matching archive"),
            vec![1, 2]
        );
        let family_err =
            decode_halo2_ipa_proving_key_archive(&archive, "proof-family-b", vk_commitment)
                .expect_err("wrong circuit family must reject");
        assert!(
            family_err.contains("circuit family"),
            "unexpected family error: {family_err}"
        );
        let commitment_err =
            decode_halo2_ipa_proving_key_archive(&archive, "proof-family-a", [0x24; 32])
                .expect_err("wrong verifier-key commitment must reject");
        assert!(
            commitment_err.contains("verifier-key commitment mismatch"),
            "unexpected commitment error: {commitment_err}"
        );
        let raw_err =
            decode_halo2_ipa_proving_key_archive(&[1, 2], "proof-family-a", vk_commitment)
                .expect_err("raw Halo2 key bytes must not decode as an archive");
        assert!(
            raw_err.contains("failed to decode proving key archive"),
            "unexpected raw-key error: {raw_err}"
        );
        let mut noncanonical = archive;
        noncanonical.push(0);
        let canonical_err =
            decode_halo2_ipa_proving_key_archive(&noncanonical, "proof-family-a", vk_commitment)
                .expect_err("archive with trailing bytes must not be accepted");
        assert!(
            canonical_err.contains("failed to decode proving key archive"),
            "unexpected non-canonical archive error: {canonical_err}"
        );
    }
    #[test]
    fn halo2_ipa_proving_key_archive_rejects_oversized_circuit_family() {
        let family = "x".repeat(HALO2_IPA_PROVING_KEY_ARCHIVE_MAX_CIRCUIT_FAMILY_BYTES + 1);
        let err = encode_halo2_ipa_proving_key_archive(&family, [0x42; 32], vec![1])
            .expect_err("oversized circuit family must reject before encoding");
        assert!(
            err.contains("circuit family exceeds"),
            "unexpected circuit-family error: {err}"
        );
    }
    #[test]
    fn halo2_ipa_proving_key_preflight_rejects_untrusted_polynomial_lengths() {
        let vk_box = halo2_ipa_ivm_execution_vk_box().expect("ivm execution verifying key");
        let params = zkparse::params_for_circuit_v1(&vk_box.bytes, IVM_EXECUTION_V1_CIRCUIT_ID)
            .expect("canonical IVM parameters");
        let parsed_vk =
            zkparse::vk_from_bytes::<pasta_tiny::IvmExecutionBindV1>(&vk_box.bytes, &params)
                .expect("canonical IVM verifying key");
        let archive =
            derive_halo2_ipa_ivm_execution_proving_key_bytes(&vk_box).expect("derive proving key");
        let mut proving_key = decode_halo2_ipa_proving_key_archive(
            &archive,
            IVM_EXECUTION_V1_CIRCUIT_ID,
            hash_vk(&vk_box),
        )
        .expect("decode canonical proving key");
        preflight_halo2_ipa_processed_proving_key(&proving_key, &parsed_vk, &params)
            .expect("canonical proving key passes structural preflight");
        let first_polynomial = halo2_backend::verifying_key_to_processed_bytes(&parsed_vk).len();
        proving_key[first_polynomial..first_polynomial + 4]
            .copy_from_slice(&u32::MAX.to_be_bytes());
        let err = preflight_halo2_ipa_processed_proving_key(&proving_key, &parsed_vk, &params)
            .expect_err("attacker-selected polynomial length must fail before Halo2 parsing");
        assert!(
            err.contains("does not match domain"),
            "unexpected proving-key preflight error: {err}"
        );
    }
    #[test]
    fn halo2_ipa_proving_key_archive_writer_matches_byte_encoder() {
        let vk_commitment = [0x51; 32];
        let proving_key = (0u8..=63).collect::<Vec<_>>();
        let expected = encode_halo2_ipa_proving_key_archive(
            "proof-family-a",
            vk_commitment,
            proving_key.clone(),
        )
        .expect("encode proving key archive");
        let mut writer = std::io::Cursor::new(Vec::new());
        write_halo2_ipa_proving_key_archive(
            &mut writer,
            "proof-family-a",
            vk_commitment,
            proving_key,
        )
        .expect("stream proving key archive");
        assert_eq!(writer.into_inner(), expected);
    }
}
#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
mod halo2_ipa_parameter_source_tests {
    use super::*;
    fn append_raw_tlv(bytes: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
        bytes.extend_from_slice(&tag);
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("test TLV length fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(payload);
    }
    fn ivm_vk_metadata(ipa_k: u32, h2vk_k: u32) -> Vec<u8> {
        let mut bytes = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut bytes, ipa_k);
        zk1::wrap_append_circuit_id(&mut bytes, IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID);
        let mut h2vk = vec![0u8; 10 + 32];
        h2vk[0] = 0x02;
        h2vk[1..5].copy_from_slice(&h2vk_k.to_le_bytes());
        h2vk[5] = 0;
        h2vk[6..10].copy_from_slice(&1u32.to_le_bytes());
        append_raw_tlv(&mut bytes, *b"H2VK", &h2vk);
        bytes
    }
    #[test]
    fn production_parameter_source_rejects_unbounded_k_before_construction() {
        let oversized = ivm_vk_metadata(u32::MAX, u32::MAX);
        let result = std::panic::catch_unwind(|| {
            zkparse::params_for_circuit_v1(&oversized, IVM_EXECUTION_V1_CIRCUIT_ID)
        })
        .expect("invalid IPAK must be rejected without entering ParamsIPA::new");
        assert!(result.is_none());
    }
    #[test]
    fn production_parameter_source_rejects_duplicate_and_mismatched_metadata() {
        let mut duplicate = ivm_vk_metadata(IVM_EXECUTION_V1_IPA_K, IVM_EXECUTION_V1_IPA_K);
        zk1::wrap_append_ipa_k(&mut duplicate, IVM_EXECUTION_V1_IPA_K);
        assert!(zkparse::params_for_circuit_v1(&duplicate, IVM_EXECUTION_V1_CIRCUIT_ID).is_none());
        let mismatched_header = ivm_vk_metadata(IVM_EXECUTION_V1_IPA_K, IVM_EXECUTION_V1_IPA_K + 1);
        assert!(
            zkparse::params_for_circuit_v1(&mismatched_header, IVM_EXECUTION_V1_CIRCUIT_ID,)
                .is_none()
        );
        let mut malformed = ivm_vk_metadata(IVM_EXECUTION_V1_IPA_K, IVM_EXECUTION_V1_IPA_K);
        malformed.push(0);
        assert!(zkparse::params_for_circuit_v1(&malformed, IVM_EXECUTION_V1_CIRCUIT_ID).is_none());
    }
    #[cfg(feature = "zk-halo2")]
    #[test]
    fn production_parameter_map_matches_kaigi_circuit_constants() {
        assert_eq!(KAIGI_IPA_K_V1, kaigi_zk::KAIGI_ROSTER_CIRCUIT_K);
        assert_eq!(KAIGI_IPA_K_V1, kaigi_zk::KAIGI_USAGE_CIRCUIT_K);
    }
}
/// Native IPA polynomial-opening verifier using internal `iroha_zkp_halo2`.
/// Expects proof bytes to be a Norito-encoded `OpenVerifyEnvelope`.
#[cfg(feature = "zk-ipa-native")]
fn verify_ipa_open_envelope(proof: &ProofBox) -> bool {
    use iroha_zkp_halo2::{
        OpenVerifyEnvelope, Transcript,
        backend::{bn254, pallas},
        norito_helpers::{self as nh, DecodedEnvelope},
    };
    // Decode Norito envelope
    let env: OpenVerifyEnvelope = match norito::decode_canonical(&proof.bytes) {
        Ok(x) => x,
        Err(_) => return false,
    };
    // Convert wire types to internal types
    let decoded = match nh::decode_envelope(&env) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let mut tr = Transcript::new(&env.transcript_label);
    let metadata = env.transcript_metadata();
    let res = match decoded {
        DecodedEnvelope::Pallas {
            params,
            proof,
            z,
            t,
            p_g,
        } => pallas::Polynomial::verify_open_with_metadata(
            params.as_ref(),
            &mut tr,
            z,
            p_g,
            t,
            proof.as_ref(),
            metadata,
        ),
        #[cfg(feature = "goldilocks_backend")]
        DecodedEnvelope::Goldilocks { .. } => return false,
        #[cfg(not(feature = "goldilocks_backend"))]
        DecodedEnvelope::Goldilocks => return false,
        DecodedEnvelope::Bn254 {
            params,
            proof,
            z,
            t,
            p_g,
        } => bn254::Polynomial::verify_open_with_metadata(
            params.as_ref(),
            &mut tr,
            z,
            p_g,
            t,
            proof.as_ref(),
            metadata,
        ),
    };
    res.is_ok()
}
/// Halo2 envelope parsing helpers.
///
/// These routines keep proof/VK payload handling deterministic and bounded while
/// delegating cryptographic verification to the concrete Halo2 backends.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
mod zkparse {
    use super::{PastaParams, pasta_params_new};
    use halo2_proofs::poly::commitment::Params as _;
    use std::{
        convert::TryFrom,
        io::{Cursor, Read},
    };
    fn envelope_cursor(bytes: &[u8]) -> Option<Cursor<&[u8]>> {
        if !super::zk1::is_envelope(bytes) || bytes.len() < 4 {
            return None;
        }
        Some(Cursor::new(&bytes[4..]))
    }
    fn read_u32(cursor: &mut Cursor<&[u8]>) -> Option<u32> {
        let mut le = [0u8; 4];
        cursor.read_exact(&mut le).ok()?;
        Some(u32::from_le_bytes(le))
    }
    fn read_tlv<'a>(cursor: &mut Cursor<&'a [u8]>) -> Option<([u8; 4], &'a [u8])> {
        let mut tag = [0u8; 4];
        cursor.read_exact(&mut tag).ok()?;
        let len = read_u32(cursor)? as usize;
        if len > super::MAX_PROOF_LEN {
            return None;
        }
        let start = usize::try_from(cursor.position()).ok()?;
        let end = start.checked_add(len)?;
        if end > cursor.get_ref().len() {
            return None;
        }
        cursor.set_position(u64::try_from(end).ok()?);
        let bytes = cursor.get_ref();
        Some((tag, &bytes[start..end]))
    }
    /// Parse a Halo2 `VerifyingKey` (Pasta) from a ZK1 envelope embedding an `H2VK` TLV.
    /// Returns `None` if parsing fails.
    pub fn vk_from_bytes<C>(
        vk_bytes: &[u8],
        params: &PastaParams,
    ) -> Option<super::halo2_backend::VerifyingKey>
    where
        C: halo2_proofs::plonk::Circuit<super::halo2_backend::Scalar>,
        C::Params: Default,
    {
        let mut cursor = envelope_cursor(vk_bytes)?;
        while let Some((tag, payload)) = read_tlv(&mut cursor) {
            if &tag == b"H2VK" {
                let mut payload_cursor = Cursor::new(payload);
                let vk = super::read_verifying_key::<C, _>(&mut payload_cursor).ok()?;
                if usize::try_from(payload_cursor.position()).ok()? != payload.len() {
                    return None;
                }
                if vk.get_domain().k() != params.k() {
                    return None;
                }
                return Some(vk);
            }
        }
        None
    }
    /// Validate a production V1 verifier-key envelope before deriving transparent parameters.
    ///
    /// The circuit identifier selects one fixed `k`. Both `IPAK` and the
    /// processed `H2VK` header must repeat that value in a strict
    /// `IPAK`/`CID1`/`H2VK` envelope. Generator construction happens only
    /// after these cheap checks, so key metadata cannot select an unbounded
    /// domain.
    pub fn params_for_circuit_v1(vk_bytes: &[u8], circuit_id: &str) -> Option<PastaParams> {
        let canonical_circuit_id = super::normalize_halo2_ipa_circuit_id(circuit_id)?;
        let expected_k = super::halo2_ipa_canonical_k_v1(&canonical_circuit_id)?;
        let ipa_k =
            super::zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(vk_bytes, &canonical_circuit_id)
                .ok()?;
        if ipa_k != expected_k {
            return None;
        }
        let h2vk = super::zk1::h2vk_payload(vk_bytes).ok()?;
        let (h2vk_k, _compress_selectors, _fixed_columns) =
            super::zk1::halo2_pasta_vk_header(h2vk).ok()?;
        if h2vk_k != expected_k {
            return None;
        }
        Some(pasta_params_new(expected_k))
    }
    /// Parse bounded Params from a developer/test VK container carrying an `IPAK` TLV.
    ///
    /// Production circuits use [`params_for_circuit_v1`]. This fallback is
    /// retained for in-crate tiny-circuit tests and still rejects duplicate,
    /// malformed, unknown, and above-production-limit metadata before
    /// generator construction.
    #[cfg(test)]
    pub fn params_any(vk_bytes: &[u8]) -> Option<PastaParams> {
        let mut cursor = envelope_cursor(vk_bytes)?;
        let mut ipa_k: Option<u32> = None;
        while usize::try_from(cursor.position()).ok()? < cursor.get_ref().len() {
            let (tag, payload) = read_tlv(&mut cursor)?;
            match &tag {
                b"IPAK" => {
                    if ipa_k.is_some() || payload.len() != 4 {
                        return None;
                    }
                    ipa_k = Some(u32::from_le_bytes(payload.try_into().ok()?));
                }
                b"CID1" | b"H2VK" => {}
                _ => return None,
            }
        }
        let ipa_k = ipa_k?;
        if ipa_k > super::HALO2_IPA_MAX_K_V1 {
            return None;
        }
        Some(pasta_params_new(ipa_k))
    }
    /// Parse a canonical proof envelope containing `PROF` followed by an
    /// optional `I10P`, with no unrecognized metadata.
    ///
    /// Circuits with no public instances omit `I10P`; circuits with instances
    /// must carry one non-empty, exactly consumed payload. The verifier still
    /// enforces the circuit-specific column shape.
    pub fn strict_proof_and_instances(
        bytes: &[u8],
    ) -> Result<(Vec<u8>, Vec<Vec<halo2_proofs::halo2curves::pasta::Fp>>), &'static str> {
        let mut cursor = envelope_cursor(bytes).ok_or("invalid ZK1 proof envelope")?;
        let mut proof_payload: Option<Vec<u8>> = None;
        let mut inst_cols: Option<Vec<Vec<halo2_proofs::halo2curves::pasta::Fp>>> = None;
        let mut position = 0_u8;
        while (cursor.position() as usize) < cursor.get_ref().len() {
            let Some((tag, payload)) = read_tlv(&mut cursor) else {
                return Err("malformed ZK1 TLV");
            };
            match (position, &tag) {
                (0, b"PROF") => {
                    if payload.is_empty() {
                        return Err("empty PROF TLV");
                    }
                    proof_payload = Some(payload.to_vec());
                }
                (1, b"I10P") => {
                    let mut inner = Cursor::new(payload);
                    let cols = read_u32(&mut inner).ok_or("malformed I10P TLV")? as usize;
                    let rows = read_u32(&mut inner).ok_or("malformed I10P TLV")? as usize;
                    if cols == 0 || rows == 0 {
                        return Err("empty I10P TLV");
                    }
                    if cols > super::MAX_INST_COLS || rows > super::MAX_INST_ROWS {
                        return Err("oversized I10P TLV");
                    }
                    let mut columns = vec![Vec::with_capacity(rows); cols];
                    for _ in 0..rows {
                        for column in &mut columns {
                            let mut b32 = [0u8; 32];
                            inner
                                .read_exact(&mut b32)
                                .map_err(|_| "truncated I10P TLV")?;
                            let mut repr =
                                <halo2_proofs::halo2curves::pasta::Fp as ff::PrimeField>::Repr::default();
                            repr.as_mut().copy_from_slice(&b32);
                            let val = Option::from(
                                <halo2_proofs::halo2curves::pasta::Fp as ff::PrimeField>::from_repr(
                                    repr,
                                ),
                            )
                            .ok_or("non-canonical I10P scalar")?;
                            column.push(val);
                        }
                    }
                    if inner.position() as usize != payload.len() {
                        return Err("trailing I10P bytes");
                    }
                    inst_cols = Some(columns);
                }
                _ => return Err("proof TLVs are not in canonical order"),
            }
            position = position.saturating_add(1);
        }
        let payload = proof_payload.ok_or("missing PROF TLV")?;
        let inst_cols = inst_cols.unwrap_or_default();
        Ok((payload, inst_cols))
    }
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn halo2_params_for_verifier_v1(vk_bytes: &[u8], circuit_id: &str) -> Option<PastaParams> {
    if halo2_ipa_canonical_k_v1(circuit_id).is_some() {
        return zkparse::params_for_circuit_v1(vk_bytes, circuit_id);
    }
    #[cfg(test)]
    {
        zkparse::params_any(vk_bytes)
    }
    #[cfg(not(test))]
    {
        None
    }
}
#[allow(dead_code)]
pub(crate) fn extract_pasta_fp_instances(
    proof_bytes: &[u8],
) -> Option<Vec<Vec<halo2_proofs::halo2curves::pasta::Fp>>> {
    extract_pasta_fp_instances_impl(proof_bytes)
}
/// Extract instance columns as raw 32-byte little-endian field elements.
pub(crate) fn extract_pasta_instance_columns_bytes(
    proof_bytes: &[u8],
) -> Option<Vec<Vec<[u8; 32]>>> {
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    {
        use halo2_proofs::halo2curves::ff::PrimeField as _;
        if let Ok((_, cols)) = zkparse::strict_proof_and_instances(proof_bytes) {
            let mut columns = Vec::with_capacity(cols.len());
            for col in cols {
                let mut out_col = Vec::with_capacity(col.len());
                for value in col {
                    let mut buf = [0u8; 32];
                    buf.copy_from_slice(value.to_repr().as_ref());
                    out_col.push(buf);
                }
                columns.push(out_col);
            }
            return Some(columns);
        }
    }
    None
}
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn extract_pasta_fp_instances_impl(
    proof_bytes: &[u8],
) -> Option<Vec<Vec<halo2_proofs::halo2curves::pasta::Fp>>> {
    zkparse::strict_proof_and_instances(proof_bytes)
        .ok()
        .map(|(_, cols)| cols)
}
#[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
fn extract_pasta_fp_instances_impl(
    _proof_bytes: &[u8],
) -> Option<Vec<Vec<halo2_proofs::halo2curves::pasta::Fp>>> {
    None
}
// Tiny pasta circuits used for dispatch verification across transparent IPA paths.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
mod pasta_tiny {
    #![cfg_attr(not(test), allow(dead_code))]
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner},
        halo2curves::pasta::Fp as Scalar,
        plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector},
        poly::Rotation,
    };
    #[derive(Clone, Default)]
    pub struct Add;
    impl Circuit<Scalar> for Add {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let s = meta.selector();
            meta.create_gate("add", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                vec![s * (a + b - c)]
            });
            (a, b, c, s)
        }
        fn synthesize(
            &self,
            (a, b, c, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "tiny_add",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "a", a => Scalar::from(2))?;
                    advice!(region, "b", b => Scalar::from(2))?;
                    advice!(region, "c", c => Scalar::from(4))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct Mul;
    impl Circuit<Scalar> for Mul {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let s = meta.selector();
            meta.create_gate("mul", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                vec![s * (a * b - c)]
            });
            (a, b, c, s)
        }
        fn synthesize(
            &self,
            (a, b, c, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "tiny_mul",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "a", a => Scalar::from(3))?;
                    advice!(region, "b", b => Scalar::from(3))?;
                    advice!(region, "c", c => Scalar::from(9))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct AddPublic;
    impl Circuit<Scalar> for AddPublic {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let inst = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("add_pub", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                let pubv = meta.query_instance(inst, Rotation::cur());
                vec![s.clone() * (a + b - c.clone()), s * (c - pubv)]
            });
            (a, b, c, inst, s)
        }
        fn synthesize(
            &self,
            (a, b, c, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "tiny_add_pub",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "a", a => Scalar::from(2))?;
                    advice!(region, "b", b => Scalar::from(2))?;
                    advice!(region, "c", c => Scalar::from(4))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct MulPublic;
    impl Circuit<Scalar> for MulPublic {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let inst = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("mul_pub", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                let pubv = meta.query_instance(inst, Rotation::cur());
                vec![s.clone() * (a * b - c.clone()), s * (c - pubv)]
            });
            (a, b, c, inst, s)
        }
        fn synthesize(
            &self,
            (a, b, c, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "tiny_mul_pub",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "a", a => Scalar::from(3))?;
                    advice!(region, "b", b => Scalar::from(3))?;
                    advice!(region, "c", c => Scalar::from(9))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct IdPublic;
    impl Circuit<Scalar> for IdPublic {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let c = meta.advice_column();
            let inst = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("id_pub", |meta| {
                let s = meta.query_selector(s);
                let c = meta.query_advice(c, Rotation::cur());
                let pubv = meta.query_instance(inst, Rotation::cur());
                vec![s * (c - pubv)]
            });
            (c, inst, s)
        }
        fn synthesize(
            &self,
            (c, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "id_pub",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "c", c => Scalar::from(7))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct AddTwoRows;
    impl Circuit<Scalar> for AddTwoRows {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let s = meta.selector();
            meta.create_gate("add_2rows", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                vec![s * (a + b - c)]
            });
            (a, b, c, s)
        }
        fn synthesize(
            &self,
            (a, b, c, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "tiny_add_2rows",
                |mut region| {
                    // Row 0: 2 + 2 = 4
                    s.enable(&mut region, 0)?;
                    advice!(region, "a0", a => Scalar::from(2))?;
                    advice!(region, "b0", b => Scalar::from(2))?;
                    advice!(region, "c0", c => Scalar::from(4))?;
                    // Row 1: 5 + 7 = 12
                    s.enable(&mut region, 1)?;
                    advice!(region, "a1", a, 1 => Scalar::from(5))?;
                    advice!(region, "b1", b, 1 => Scalar::from(7))?;
                    advice!(region, "c1", c, 1 => Scalar::from(12))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct AddThree;
    impl Circuit<Scalar> for AddThree {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let d = meta.advice_column();
            let c = meta.advice_column();
            let s = meta.selector();
            meta.create_gate("add3", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let d = meta.query_advice(d, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                vec![s * (a + b + d - c)]
            });
            (a, b, d, c, s)
        }
        fn synthesize(
            &self,
            (a, b, d, c, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "tiny_add3",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "a", a => Scalar::from(1))?;
                    advice!(region, "b", b => Scalar::from(2))?;
                    advice!(region, "d", d => Scalar::from(3))?;
                    advice!(region, "c", c => Scalar::from(6))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct AddTwoInstPublic;
    impl Circuit<Scalar> for AddTwoInstPublic {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let a = meta.advice_column();
            let b = meta.advice_column();
            let c = meta.advice_column();
            let inst0 = meta.instance_column();
            let inst1 = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("add2inst_pub", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(a, Rotation::cur());
                let b = meta.query_advice(b, Rotation::cur());
                let c = meta.query_advice(c, Rotation::cur());
                let i0 = meta.query_instance(inst0, Rotation::cur());
                let i1 = meta.query_instance(inst1, Rotation::cur());
                // Enforce: c = a + b, and i0 = a, i1 = b
                vec![
                    s.clone() * (a.clone() + b.clone() - c),
                    s.clone() * (a - i0),
                    s * (b - i1),
                ]
            });
            (a, b, c, inst0, inst1, s)
        }
        fn synthesize(
            &self,
            (a, b, c, _i0, _i1, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "tiny_add2inst_pub",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "a", a => Scalar::from(5))?;
                    advice!(region, "b", b => Scalar::from(8))?;
                    advice!(region, "c", c => Scalar::from(13))?;
                    Ok(())
                },
            )
        }
    }
    /// Circuit binding eight single-row instance columns to witness values.
    ///
    /// Historical binding gadget retained for tests and fixture generation. It proves
    /// only that the supplied proof is tied to public inputs carried in the proof
    /// envelope (for Iroha, `(code_hash, overlay_hash)` split into `u64` limbs).
    ///
    /// Note: This circuit does **not** prove correct IVM execution by itself.
    #[derive(Clone)]
    pub struct IvmOverlayBind {
        /// Witness values constrained to equal the corresponding public instances.
        pub values: [Scalar; 8],
    }
    impl Default for IvmOverlayBind {
        fn default() -> Self {
            Self {
                values: [Scalar::from(0); 8],
            }
        }
    }
    impl Circuit<Scalar> for IvmOverlayBind {
        type Config = (
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8],
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 8],
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self::default()
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            meta.set_minimum_degree(3);
            let adv = std::array::from_fn(|_| meta.advice_column());
            let inst = std::array::from_fn(|_| meta.instance_column());
            let s = meta.selector();
            meta.create_gate("ivm_overlay_bind", |meta| {
                let s = meta.query_selector(s);
                let mut cons = Vec::with_capacity(8);
                for i in 0..8 {
                    let a = meta.query_advice(adv[i], Rotation::cur());
                    let p = meta.query_instance(inst[i], Rotation::cur());
                    cons.push(s.clone() * (a - p));
                }
                cons
            });
            (adv, inst, s)
        }
        fn synthesize(
            &self,
            (adv, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let values = self.values;
            layouter.assign_region(
                || "ivm_overlay_bind",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    for (i, column) in adv.iter().enumerate() {
                        advice!(region, move "a{i}", *column => values[i])?;
                    }
                    Ok(())
                },
            )
        }
    }
    /// Circuit binding sixteen single-row instance columns to witness values.
    ///
    /// This is used by `ivm-execution-v1` fixtures to ensure the proof is bound to
    /// all public commitments required by `Executable::IvmProved` admission.
    ///
    /// Note: This circuit does **not** prove correct IVM execution by itself.
    #[derive(Clone)]
    pub struct IvmExecutionBindV1 {
        /// Witness values constrained to equal the corresponding public instances.
        pub values: [Scalar; 16],
    }
    impl Default for IvmExecutionBindV1 {
        fn default() -> Self {
            Self {
                values: [Scalar::from(0); 16],
            }
        }
    }
    impl Circuit<Scalar> for IvmExecutionBindV1 {
        type Config = (
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 16],
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 16],
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self::default()
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            meta.set_minimum_degree(3);
            let adv = std::array::from_fn(|_| meta.advice_column());
            let inst = std::array::from_fn(|_| meta.instance_column());
            let s = meta.selector();
            meta.create_gate("ivm_execution_bind_current", |meta| {
                let s = meta.query_selector(s);
                let mut cons = Vec::with_capacity(16);
                for i in 0..16 {
                    let a = meta.query_advice(adv[i], Rotation::cur());
                    let p = meta.query_instance(inst[i], Rotation::cur());
                    cons.push(s.clone() * (a - p));
                }
                cons
            });
            (adv, inst, s)
        }
        fn synthesize(
            &self,
            (adv, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let values = self.values;
            layouter.assign_region(
                || "ivm_execution_bind_current",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    for (i, column) in adv.iter().enumerate() {
                        advice!(region, move "a{i}", *column => values[i])?;
                    }
                    Ok(())
                },
            )
        }
    }
    pub struct AnonTransfer2x2;
    impl Circuit<Scalar> for AnonTransfer2x2 {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let in0 = meta.advice_column();
            let in1 = meta.advice_column();
            let out0 = meta.advice_column();
            let out1 = meta.advice_column();
            let s = meta.selector();
            meta.create_gate("anon_transfer_2x2_conserve", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(in0, Rotation::cur());
                let b = meta.query_advice(in1, Rotation::cur());
                let c = meta.query_advice(out0, Rotation::cur());
                let d = meta.query_advice(out1, Rotation::cur());
                vec![s * (a + b - (c + d))]
            });
            (in0, in1, out0, out1, s)
        }
        fn synthesize(
            &self,
            (in0, in1, out0, out1, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "anon_transfer_2x2",
                |mut region| {
                    // Example transfer: 7 + 5 = 6 + 6
                    s.enable(&mut region, 0)?;
                    advice!(region, "in0", in0 => Scalar::from(7))?;
                    advice!(region, "in1", in1 => Scalar::from(5))?;
                    advice!(region, "out0", out0 => Scalar::from(6))?;
                    advice!(region, "out1", out1 => Scalar::from(6))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct VoteBool;
    impl Circuit<Scalar> for VoteBool {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let v = meta.advice_column();
            let s = meta.selector();
            meta.create_gate("vote_bool", |meta| {
                let s = meta.query_selector(s);
                let v = meta.query_advice(v, Rotation::cur());
                // Enforce v in {0,1}: v * (v - 1) = 0
                let one = halo2_proofs::plonk::Expression::Constant(Scalar::from(1u64));
                vec![s * (v.clone() * (v - one))]
            });
            (v, s)
        }
        fn synthesize(
            &self,
            (v, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "vote_bool",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    // Example vote: 1 (YES)
                    advice!(region, "v", v => Scalar::from(1u64))?;
                    Ok(())
                },
            )
        }
    }
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    #[derive(Clone, Default)]
    pub struct CommitOpen; // algebraic test relation; not a cryptographic commitment
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    impl Circuit<Scalar> for CommitOpen {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // m
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit (public)
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let m = meta.advice_column();
            let r = meta.advice_column();
            let inst = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("commit_open", |meta| {
                let s = meta.query_selector(s);
                let m = meta.query_advice(m, Rotation::cur());
                let r = meta.query_advice(r, Rotation::cur());
                let c = meta.query_instance(inst, Rotation::cur());
                vec![s * (poseidon_pair_expr(m, r) - c)]
            });
            (m, r, inst, s)
        }
        fn synthesize(
            &self,
            (m, r, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "commit_open",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "m", m => Scalar::from(11))?;
                    advice!(region, "r", r => Scalar::from(31))?;
                    Ok(())
                },
            )
        }
    }
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    #[derive(Clone, Default)]
    pub struct Merkle2; // algebraic test tree; not a collision-resistant Merkle tree
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    impl Circuit<Scalar> for Merkle2 {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // leaf
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sib0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sib1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // w0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // w1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root (public)
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let leaf = meta.advice_column();
            let sib0 = meta.advice_column();
            let sib1 = meta.advice_column();
            let w0 = meta.advice_column();
            let w1 = meta.advice_column();
            let root = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("merkle2", |meta| {
                let s = meta.query_selector(s);
                let leaf = meta.query_advice(leaf, Rotation::cur());
                let sib0 = meta.query_advice(sib0, Rotation::cur());
                let sib1 = meta.query_advice(sib1, Rotation::cur());
                let w0 = meta.query_advice(w0, Rotation::cur());
                let w1 = meta.query_advice(w1, Rotation::cur());
                let root = meta.query_instance(root, Rotation::cur());
                vec![
                    s.clone() * (w0.clone() - poseidon_pair_expr(leaf, sib0)),
                    s.clone() * (w1.clone() - poseidon_pair_expr(w0, sib1)),
                    s * (root - w1),
                ]
            });
            (leaf, sib0, sib1, w0, w1, root, s)
        }
        fn synthesize(
            &self,
            (leaf, sib0, sib1, w0, w1, _root, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "merkle2",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    let l = Scalar::from(9);
                    let s0 = Scalar::from(5);
                    let s1 = Scalar::from(7);
                    let w0v = poseidon_pair(l, s0);
                    let w1v = poseidon_pair(w0v, s1);
                    advice!(region, "leaf", leaf => l)?;
                    advice!(region, "sib0", sib0 => s0)?;
                    advice!(region, "sib1", sib1 => s1)?;
                    advice!(region, "w0", w0 => w0v)?;
                    advice!(region, "w1", w1 => w1v)?;
                    Ok(())
                },
            )
        }
    }
    // INSECURE DEV-TEST COMPATIBILITY ONLY. This is a single quintic expression,
    // not Poseidon: it has no full permutation rounds or MDS schedule, and the
    // assignment wrapper below does not constrain its digest. No production
    // verifier, key generator, or release artifact may select these circuits.
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    pub mod poseidon {
        use super::*;
        use halo2_proofs::{
            circuit::{AssignedCell, Cell, Layouter, Value},
            plonk::{
                Advice, Circuit, Column, ConstraintSystem, Error as PlonkError, Fixed, Selector,
            },
            poly::Rotation,
        };
        pub(super) fn compress2_native(
            a: halo2_proofs::halo2curves::pasta::Fp,
            b: halo2_proofs::halo2curves::pasta::Fp,
        ) -> halo2_proofs::halo2curves::pasta::Fp {
            use halo2_proofs::halo2curves::pasta::Fp as F;
            let t0 = a + F::from(7u64);
            let t1 = b + F::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            F::from(2) * t0_5 + F::from(3) * t1_5
        }
        /// Local Pow5 configuration retained for old Poseidon-gadget call sites.
        #[derive(Clone, Debug)]
        pub struct Pow5Config<F, const WIDTH: usize, const RATE: usize> {
            /// State advice columns used by the compatibility wrapper.
            pub state: [Column<Advice>; WIDTH],
            _marker: std::marker::PhantomData<fn() -> F>,
        }
        /// Local Pow5 chip marker retained for old Poseidon-gadget call sites.
        #[derive(Clone, Debug)]
        pub struct Pow5Chip<F, const WIDTH: usize, const RATE: usize> {
            _config: Pow5Config<F, WIDTH, RATE>,
        }
        impl<F, const WIDTH: usize, const RATE: usize> Pow5Chip<F, WIDTH, RATE>
        where
            F: halo2_proofs::halo2curves::ff::Field,
        {
            /// Build a local compatibility config without depending on upstream gadgets.
            pub fn configure(
                meta: &mut ConstraintSystem<F>,
                state: [Column<Advice>; WIDTH],
                partial: Column<Advice>,
                rc_a: Column<Fixed>,
                rc_b: Column<Fixed>,
            ) -> Pow5Config<F, WIDTH, RATE> {
                let _ = (partial, rc_a, rc_b);
                for column in state {
                    meta.enable_equality(column);
                }
                Pow5Config {
                    state,
                    _marker: std::marker::PhantomData,
                }
            }
            /// Construct a compatibility chip wrapper from its config.
            pub fn construct(config: Pow5Config<F, WIDTH, RATE>) -> Self {
                Self { _config: config }
            }
        }
        /// Assigned cell handle returned by the local Poseidon wrapper.
        #[derive(Clone, Debug)]
        pub struct PoseidonCell<F> {
            cell: Cell,
            _marker: std::marker::PhantomData<fn() -> F>,
        }
        impl<F> PoseidonCell<F> {
            fn new(cell: Cell) -> Self {
                Self {
                    cell,
                    _marker: std::marker::PhantomData,
                }
            }
            /// Return the underlying Halo2 cell.
            pub fn cell(&self) -> Cell {
                self.cell
            }
        }
        /// Unconstrained dev-test assignment wrapper retained for compatibility tests.
        ///
        /// This type is not a hash gadget and must never be used by a release circuit.
        #[derive(Clone, Default)]
        pub struct Poseidon2ChipWrapper;
        #[derive(Clone)]
        pub struct PoseidonHashCells<F> {
            /// Assigned digest cell.
            pub digest: PoseidonCell<F>,
            /// Assigned left input cell.
            pub left: PoseidonCell<F>,
            /// Assigned right input cell.
            pub right: PoseidonCell<F>,
        }
        impl<F> PoseidonHashCells<F> {
            /// Return the digest cell for call sites that only need the hash output.
            pub fn cell(&self) -> Cell {
                self.digest.cell()
            }
        }
        impl Poseidon2ChipWrapper {
            pub fn new() -> Self {
                Self
            }
            /// Assign the retired quintic expression without hash constraints.
            ///
            /// Callers may use this only in negative/dev-test scaffolding.
            pub fn hash2_chip(
                &self,
                layouter: &mut impl Layouter<halo2_proofs::halo2curves::pasta::Fp>,
                poseidon_cfg: &Pow5Config<halo2_proofs::halo2curves::pasta::Fp, 3, 2>,
                a: Value<halo2_proofs::halo2curves::pasta::Fp>,
                b: Value<halo2_proofs::halo2curves::pasta::Fp>,
            ) -> Result<PoseidonHashCells<halo2_proofs::halo2curves::pasta::Fp>, PlonkError>
            {
                let digest = a.zip(b).map(|(left, right)| compress2_native(left, right));
                let (a_cell, b_cell, digest_cell) = layouter.assign_region(
                    || "poseidon2_inputs",
                    |mut region| {
                        let a_cell = advice_dev!(region, "a", poseidon_cfg.state[0] => value a)?;
                        let b_cell = advice_dev!(region, "b", poseidon_cfg.state[1] => value b)?;
                        let digest_cell =
                            advice_dev!(region, "digest", poseidon_cfg.state[2] => value digest)?;
                        Ok((a_cell, b_cell, digest_cell))
                    },
                )?;
                Ok(PoseidonHashCells {
                    digest: PoseidonCell::new(digest_cell.cell()),
                    left: PoseidonCell::new(a_cell.cell()),
                    right: PoseidonCell::new(b_cell.cell()),
                })
            }
        }
        #[derive(Clone, Default)]
        pub struct CommitOpenPoseidon;
        impl Circuit<halo2_proofs::halo2curves::pasta::Fp> for CommitOpenPoseidon {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // m
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // s0
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // s1
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit (public)
                Selector,
                Pow5Config<halo2_proofs::halo2curves::pasta::Fp, 3, 2>, // Poseidon chip config
            );
            type FloorPlanner = SimpleFloorPlanner;
            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(
                meta: &mut ConstraintSystem<halo2_proofs::halo2curves::pasta::Fp>,
            ) -> Self::Config {
                let m = meta.advice_column();
                let r = meta.advice_column();
                let s0 = meta.advice_column();
                meta.enable_equality(s0);
                let s1 = meta.advice_column();
                let inst = meta.instance_column();
                let sel = meta.selector();
                // Configure Poseidon Pow5 chip (T=3, RATE=2)
                let st0 = meta.advice_column();
                let st1 = meta.advice_column();
                let st2 = meta.advice_column();
                let partial = meta.advice_column();
                let rc_a = meta.fixed_column();
                let rc_b = meta.fixed_column();
                let poseidon_cfg = Pow5Chip::configure(meta, [st0, st1, st2], partial, rc_a, rc_b);
                meta.create_gate("poseidon2_commit", |meta| {
                    let s = meta.query_selector(sel);
                    let m = meta.query_advice(m, Rotation::cur());
                    let r = meta.query_advice(r, Rotation::cur());
                    let s0_cur = meta.query_advice(s0, Rotation::cur());
                    let s1_cur = meta.query_advice(s1, Rotation::cur());
                    let c = meta.query_instance(inst, Rotation::cur());
                    // s0 is the compressor output; s1 is a secondary mix value
                    let rc0 = halo2_proofs::plonk::Expression::Constant(
                        halo2_proofs::halo2curves::pasta::Fp::from(7u64),
                    );
                    let rc1 = halo2_proofs::plonk::Expression::Constant(
                        halo2_proofs::halo2curves::pasta::Fp::from(13u64),
                    );
                    let three = halo2_proofs::plonk::Expression::Constant(
                        halo2_proofs::halo2curves::pasta::Fp::from(3u64),
                    );
                    let five = halo2_proofs::plonk::Expression::Constant(
                        halo2_proofs::halo2curves::pasta::Fp::from(5u64),
                    );
                    let t0 = m + rc0;
                    let t0_2 = t0.clone() * t0.clone();
                    let t0_4 = t0_2.clone() * t0_2;
                    let t0_5 = t0_4 * t0;
                    let t1 = r + rc1;
                    let t1_2 = t1.clone() * t1.clone();
                    let t1_4 = t1_2.clone() * t1_2;
                    let t1_5 = t1_4 * t1;
                    let exp_s1 = three * t0_5 + five * t1_5;
                    // Constrain s0_cur,s1_cur equal to exp and s0_cur == c
                    vec![s.clone() * (s1_cur - exp_s1), s * (c - s0_cur)]
                });
                (m, r, s0, s1, inst, sel, poseidon_cfg)
            }
            fn synthesize(
                &self,
                (m, r, s0, s1, _inst, sel, poseidon_cfg): Self::Config,
                mut layouter: impl Layouter<halo2_proofs::halo2curves::pasta::Fp>,
            ) -> Result<(), PlonkError> {
                use halo2_proofs::halo2curves::pasta::Fp as F;
                layouter.assign_region(
                    || "poseidon2_commit",
                    |mut region| {
                        sel.enable(&mut region, 0)?;
                        let m_v = F::from(11u64);
                        let r_v = F::from(31u64);
                        // assign inputs
                        let _m_cell = advice!(region, "m", m => m_v)?;
                        let _r_cell = advice!(region, "r", r => r_v)?;
                        // assign s0 via native helper for constraints, then constrain equal to gadget digest
                        let s0_v = compress2_native(m_v, r_v);
                        let s0_cell = advice!(region, "s0", s0 => s0_v)?;
                        // s1 remains secondary mix value for the circuit
                        let t0 = m_v + F::from(7u64);
                        let t1 = r_v + F::from(13u64);
                        let t0_2 = t0 * t0;
                        let t0_4 = t0_2 * t0_2;
                        let t0_5 = t0_4 * t0;
                        let t1_2 = t1 * t1;
                        let t1_4 = t1_2 * t1_2;
                        let t1_5 = t1_4 * t1;
                        let s1_v = F::from(3u64) * t0_5 + F::from(5u64) * t1_5;
                        advice!(region, "s1", s1 => s1_v)?;
                        // Compute gadget digest and constrain equality to s0
                        let hash_cells = Poseidon2ChipWrapper::new().hash2_chip(
                            &mut layouter,
                            &poseidon_cfg,
                            Value::known(m_v),
                            Value::known(r_v),
                        )?;
                        layouter.constrain_equal(hash_cells.digest.cell(), s0_cell.cell())?;
                        Ok(())
                    },
                )
            }
        }
        const MERKLE2_POSEIDON_DEPTH: usize = 8;
        const MERKLE2_POSEIDON_SAMPLE_LEAF: u64 = 9;
        const MERKLE2_POSEIDON_SAMPLE_SIBS: [u64; MERKLE2_POSEIDON_DEPTH] =
            [5, 11, 7, 13, 17, 23, 19, 29];
        const MERKLE2_POSEIDON_SAMPLE_DIRS: [u64; MERKLE2_POSEIDON_DEPTH] =
            [0, 1, 1, 0, 1, 0, 1, 0];
        pub(crate) fn merkle2_poseidon_sample_path() -> (
            halo2_proofs::halo2curves::pasta::Fp,
            [halo2_proofs::halo2curves::pasta::Fp; MERKLE2_POSEIDON_DEPTH],
            [halo2_proofs::halo2curves::pasta::Fp; MERKLE2_POSEIDON_DEPTH],
        ) {
            use halo2_proofs::halo2curves::pasta::Fp as F;
            let leaf = F::from(MERKLE2_POSEIDON_SAMPLE_LEAF);
            let siblings = MERKLE2_POSEIDON_SAMPLE_SIBS.map(F::from);
            let dirs = MERKLE2_POSEIDON_SAMPLE_DIRS.map(F::from);
            (leaf, siblings, dirs)
        }
        pub(crate) fn merkle2_poseidon_sample_root() -> halo2_proofs::halo2curves::pasta::Fp {
            use halo2_proofs::halo2curves::pasta::Fp as F;
            let (mut current, siblings, dirs) = merkle2_poseidon_sample_path();
            for (sib, dir) in siblings.iter().zip(dirs.iter()) {
                let left = current + *dir * (*sib - current);
                let right = *sib + *dir * (current - *sib);
                current = compress2_native(left, right);
            }
            current
        }
        #[derive(Clone, Default)]
        pub struct Merkle2Poseidon;
        impl Circuit<halo2_proofs::halo2curves::pasta::Fp> for Merkle2Poseidon {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // node (current value)
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sibling
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // direction bit
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // left input
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // right input
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // hash output
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
                Selector,
                Pow5Config<halo2_proofs::halo2curves::pasta::Fp, 3, 2>,
            );
            type FloorPlanner = SimpleFloorPlanner;
            type Params = ();
            fn without_witnesses(&self) -> Self {
                Self
            }
            fn configure(
                meta: &mut ConstraintSystem<halo2_proofs::halo2curves::pasta::Fp>,
            ) -> Self::Config {
                let node = meta.advice_column();
                meta.enable_equality(node);
                let sibling = meta.advice_column();
                let dir = meta.advice_column();
                let left = meta.advice_column();
                meta.enable_equality(left);
                let right = meta.advice_column();
                meta.enable_equality(right);
                let out = meta.advice_column();
                meta.enable_equality(out);
                let inst = meta.instance_column();
                let sel = meta.selector();
                let st0 = meta.advice_column();
                let st1 = meta.advice_column();
                let st2 = meta.advice_column();
                let partial = meta.advice_column();
                let rc_a = meta.fixed_column();
                let rc_b = meta.fixed_column();
                let poseidon_cfg = Pow5Chip::configure(meta, [st0, st1, st2], partial, rc_a, rc_b);
                meta.create_gate("merkle_poseidon_layer", |meta| {
                    use halo2_proofs::halo2curves::pasta::Fp as F;
                    let s = meta.query_selector(sel);
                    let node_q = meta.query_advice(node, Rotation::cur());
                    let sibling_q = meta.query_advice(sibling, Rotation::cur());
                    let dir_q = meta.query_advice(dir, Rotation::cur());
                    let left_q = meta.query_advice(left, Rotation::cur());
                    let right_q = meta.query_advice(right, Rotation::cur());
                    let one = halo2_proofs::plonk::Expression::Constant(F::from(1u64));
                    let left_expected =
                        node_q.clone() + dir_q.clone() * (sibling_q.clone() - node_q.clone());
                    let right_expected =
                        sibling_q.clone() + dir_q.clone() * (node_q.clone() - sibling_q.clone());
                    vec![
                        s.clone() * dir_q.clone() * (dir_q.clone() - one.clone()),
                        s.clone() * (left_q - left_expected),
                        s * (right_q - right_expected),
                    ]
                });
                (
                    node,
                    sibling,
                    dir,
                    left,
                    right,
                    out,
                    inst,
                    sel,
                    poseidon_cfg,
                )
            }
            fn synthesize(
                &self,
                (node, sibling, dir, left, right, out, inst, sel, poseidon_cfg): Self::Config,
                mut layouter: impl Layouter<halo2_proofs::halo2curves::pasta::Fp>,
            ) -> Result<(), PlonkError> {
                use halo2_proofs::halo2curves::pasta::Fp as F;
                layouter.assign_region(
                    || "merkle_poseidon_layers",
                    |mut region| {
                        let mut current = F::from(MERKLE2_POSEIDON_SAMPLE_LEAF);
                        let mut previous_output: Option<AssignedCell<F, F>> = None;
                        let chip = Poseidon2ChipWrapper::new();
                        for (row, (&sib_raw, &dir_raw)) in MERKLE2_POSEIDON_SAMPLE_SIBS
                            .iter()
                            .zip(MERKLE2_POSEIDON_SAMPLE_DIRS.iter())
                            .enumerate()
                        {
                            let sib_val = F::from(sib_raw);
                            let dir_val = F::from(dir_raw);
                            let left_val = current + dir_val * (sib_val - current);
                            let right_val = sib_val + dir_val * (current - sib_val);
                            let hash_val = compress2_native(left_val, right_val);
                            let node_cell =
                                advice_dev!(region, format "node_{row}", node, row => current)?;
                            if let Some(ref prev) = previous_output {
                                layouter.constrain_equal(node_cell.cell(), prev.cell())?;
                            }
                            advice_dev!(region, format "sibling_{row}", sibling, row => sib_val)?;
                            advice_dev!(region, format "dir_{row}", dir, row => dir_val)?;
                            let left_cell =
                                advice_dev!(region, format "left_{row}", left, row => left_val)?;
                            let right_cell =
                                advice_dev!(region, format "right_{row}", right, row => right_val)?;
                            sel.enable(&mut region, row)?;
                            let hash_cells = chip.hash2_chip(
                                &mut layouter,
                                &poseidon_cfg,
                                Value::known(left_val),
                                Value::known(right_val),
                            )?;
                            layouter.constrain_equal(left_cell.cell(), hash_cells.left.cell())?;
                            layouter.constrain_equal(right_cell.cell(), hash_cells.right.cell())?;
                            let out_cell =
                                advice_dev!(region, format "out_{row}", out, row => hash_val)?;
                            layouter.constrain_equal(out_cell.cell(), hash_cells.digest.cell())?;
                            previous_output = Some(out_cell.clone());
                            current = hash_val;
                        }
                        if let Some(ref root_cell) = previous_output {
                            layouter.constrain_instance(root_cell.cell(), inst, 0)?;
                        } else {
                            return Err(PlonkError::Synthesis);
                        }
                        Ok(())
                    },
                )?;
                Ok(())
            }
        }
    }
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    #[allow(unused_imports)]
    pub use self::poseidon::CommitOpenPoseidon as CommitOpen;
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    #[allow(unused_imports)]
    pub use self::poseidon::Merkle2Poseidon as Merkle2;
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    pub fn poseidon_compress2_native(a: Scalar, b: Scalar) -> Scalar {
        poseidon::compress2_native(a, b)
    }
    #[derive(Clone, Default)]
    pub struct VoteBoolCommit; // dev-test quintic relation; not a cryptographic commitment
    impl Circuit<Scalar> for VoteBoolCommit {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // v
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // rho
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit (public)
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let v = meta.advice_column();
            let rho = meta.advice_column();
            let inst = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("vote_bool_commit", |meta| {
                let s = meta.query_selector(s);
                let vq = meta.query_advice(v, Rotation::cur());
                let rhoq = meta.query_advice(rho, Rotation::cur());
                let cq = meta.query_instance(inst, Rotation::cur());
                let one = halo2_proofs::plonk::Expression::Constant(Scalar::from(1u64));
                // v*(v-1)=0 plus a dev-test quintic expression; this is not Poseidon.
                // Recompute limited Pow5 terms inline
                let v2 = vq.clone() * vq.clone();
                let v4 = v2.clone() * v2.clone();
                let v5 = v4.clone() * vq.clone();
                let r2 = rhoq.clone() * rhoq.clone();
                let r4 = r2.clone() * r2.clone();
                let r5 = r4 * rhoq.clone();
                let t0 = halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * v5
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * r5
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(7));
                let t1 = vq.clone() + halo2_proofs::plonk::Expression::Constant(Scalar::from(13));
                let t12 = t1.clone() * t1.clone();
                let t14 = t12.clone() * t12;
                let t15 = t14 * t1;
                let s_hash = halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * t0
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(5)) * t15
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(11));
                vec![s.clone() * (vq.clone() * (vq - one)), s * (s_hash - cq)]
            });
            (v, rho, inst, s)
        }
        fn synthesize(
            &self,
            (v, rho, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "vote_bool_commit",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "v", v => Scalar::from(1))?;
                    advice!(region, "rho", rho => Scalar::from(12345))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct AnonTransfer2x2Commit; // commit(in/out) and sum conservation
    impl Circuit<Scalar> for AnonTransfer2x2Commit {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sk (for nf)
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // serial
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // cm_in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // cm_in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // cm_out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // cm_out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // nullifier
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let in0 = meta.advice_column();
            let in1 = meta.advice_column();
            let out0 = meta.advice_column();
            let out1 = meta.advice_column();
            let r_in0 = meta.advice_column();
            let r_in1 = meta.advice_column();
            let r_out0 = meta.advice_column();
            let r_out1 = meta.advice_column();
            let sk = meta.advice_column();
            let serial = meta.advice_column();
            let cm_in0 = meta.instance_column();
            let cm_in1 = meta.instance_column();
            let cm_out0 = meta.instance_column();
            let cm_out1 = meta.instance_column();
            let nf = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("conserve_and_commit", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(in0, Rotation::cur());
                let b = meta.query_advice(in1, Rotation::cur());
                let c = meta.query_advice(out0, Rotation::cur());
                let d = meta.query_advice(out1, Rotation::cur());
                let r0 = meta.query_advice(r_in0, Rotation::cur());
                let r1 = meta.query_advice(r_in1, Rotation::cur());
                let r2 = meta.query_advice(r_out0, Rotation::cur());
                let r3 = meta.query_advice(r_out1, Rotation::cur());
                let skq = meta.query_advice(sk, Rotation::cur());
                let serq = meta.query_advice(serial, Rotation::cur());
                let input_commitment_slot0 = meta.query_instance(cm_in0, Rotation::cur());
                let input_commitment_slot1 = meta.query_instance(cm_in1, Rotation::cur());
                let output_commitment_slot0 = meta.query_instance(cm_out0, Rotation::cur());
                let output_commitment_slot1 = meta.query_instance(cm_out1, Rotation::cur());
                let nullifier_instance = meta.query_instance(nf, Rotation::cur());
                // cm_in0 = H(a, r0); cm_in1 = H(b, r1); cm_out0 = H(c, r2); cm_out1 = H(d, r3)
                let h_in0 = poseidon_pair_expr(a.clone(), r0);
                let h_in1 = poseidon_pair_expr(b.clone(), r1);
                let h_out0 = poseidon_pair_expr(c.clone(), r2);
                let h_out1 = poseidon_pair_expr(d.clone(), r3);
                let h_nf = poseidon_pair_expr(skq.clone(), serq.clone());
                vec![
                    s.clone() * (a.clone() + b.clone() - (c.clone() + d.clone())),
                    s.clone() * (h_in0 - input_commitment_slot0),
                    s.clone() * (h_in1 - input_commitment_slot1),
                    s.clone() * (h_out0 - output_commitment_slot0),
                    s.clone() * (h_out1 - output_commitment_slot1),
                    s * (h_nf - nullifier_instance),
                ]
            });
            (
                in0, in1, out0, out1, r_in0, r_in1, r_out0, r_out1, sk, serial, cm_in0, cm_in1,
                cm_out0, cm_out1, nf, s,
            )
        }
        fn synthesize(
            &self,
            cfg: Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let (
                in0,
                in1,
                out0,
                out1,
                r_in0,
                r_in1,
                r_out0,
                r_out1,
                sk,
                serial,
                _cm0,
                _cm1,
                _cmo0,
                _cmo1,
                _nf,
                s,
            ) = cfg;
            layouter.assign_region(
                || "anon_transfer_commit",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "in0", in0 => Scalar::from(7))?;
                    advice!(region, "in1", in1 => Scalar::from(5))?;
                    advice!(region, "out0", out0 => Scalar::from(6))?;
                    advice!(region, "out1", out1 => Scalar::from(6))?;
                    advice!(region, "r_in0", r_in0 => Scalar::from(11))?;
                    advice!(region, "r_in1", r_in1 => Scalar::from(13))?;
                    advice!(region, "r_out0", r_out0 => Scalar::from(17))?;
                    advice!(region, "r_out1", r_out1 => Scalar::from(19))?;
                    advice!(region, "sk", sk => Scalar::from(1_234_567))?;
                    advice!(region, "serial", serial => Scalar::from(42))?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct VoteBoolCommitMerkle2; // commit = Poseidon(v,rho); root = Merkle2(commit, sib0, sib1)
    impl Circuit<Scalar> for VoteBoolCommitMerkle2 {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // v
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // rho
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sib0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sib1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // w0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // w1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit (public)
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root (public)
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let v = meta.advice_column();
            let rho = meta.advice_column();
            let sib0 = meta.advice_column();
            let sib1 = meta.advice_column();
            let w0 = meta.advice_column();
            let w1 = meta.advice_column();
            let cm = meta.instance_column();
            let root = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("vote_commit_merkle2", |meta| {
                let s = meta.query_selector(s);
                let vq = meta.query_advice(v, Rotation::cur());
                let rhoq = meta.query_advice(rho, Rotation::cur());
                let sib0q = meta.query_advice(sib0, Rotation::cur());
                let sib1q = meta.query_advice(sib1, Rotation::cur());
                let w0q = meta.query_advice(w0, Rotation::cur());
                let w1q = meta.query_advice(w1, Rotation::cur());
                let cmq = meta.query_instance(cm, Rotation::cur());
                let rootq = meta.query_instance(root, Rotation::cur());
                let one = halo2_proofs::plonk::Expression::Constant(Scalar::from(1u64));
                // Boolean v
                let boolc = vq.clone() * (vq.clone() - one);
                // commit = H(v,rho)
                let h = poseidon_pair_expr(vq, rhoq);
                let commitment_delta = h.clone() - cmq.clone();
                // merkle2: w0 = H(cm, sib0); w1 = H(w0, sib1) = root
                let expected_first_hash = poseidon_pair_expr(h, sib0q);
                let expected_second_hash = poseidon_pair_expr(w0q.clone(), sib1q);
                vec![
                    s.clone() * boolc,
                    s.clone() * commitment_delta,
                    s.clone() * (w0q - expected_first_hash),
                    s.clone() * (w1q.clone() - expected_second_hash),
                    s * (w1q - rootq),
                ]
            });
            (v, rho, sib0, sib1, w0, w1, cm, root, s)
        }
        fn synthesize(
            &self,
            (v, rho, sib0, sib1, w0, w1, _cm, _root, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "vote_commit_merkle2",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    let v_v = Scalar::from(1);
                    let rho_v = Scalar::from(12345);
                    let sib0_v = Scalar::from(5);
                    let sib1_v = Scalar::from(7);
                    let commit_v = poseidon_pair(v_v, rho_v);
                    let w0_v = poseidon_pair(commit_v, sib0_v);
                    let w1_v = poseidon_pair(w0_v, sib1_v);
                    advice!(region, "v", v => v_v)?;
                    advice!(region, "rho", rho => rho_v)?;
                    advice!(region, "sib0", sib0 => sib0_v)?;
                    advice!(region, "sib1", sib1 => sib1_v)?;
                    advice!(region, "w0", w0 => w0_v)?;
                    advice!(region, "w1", w1 => w1_v)?;
                    Ok(())
                },
            )
        }
    }
    #[derive(Clone, Default)]
    pub struct AnonTransfer2x2CommitMerkle2;
    impl Circuit<Scalar> for AnonTransfer2x2CommitMerkle2 {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sk
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // serial
            // siblings for two-level proofs for in0 and in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sib0_0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sib0_1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sib1_0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sib1_1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // cm_in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // cm_in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // cm_out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // cm_out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // nullifier
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let in0 = meta.advice_column();
            let in1 = meta.advice_column();
            let out0 = meta.advice_column();
            let out1 = meta.advice_column();
            let r_in0 = meta.advice_column();
            let r_in1 = meta.advice_column();
            let r_out0 = meta.advice_column();
            let r_out1 = meta.advice_column();
            let sk = meta.advice_column();
            let serial = meta.advice_column();
            let sib0_0 = meta.advice_column();
            let sib0_1 = meta.advice_column();
            let sib1_0 = meta.advice_column();
            let sib1_1 = meta.advice_column();
            let cm_in0 = meta.instance_column();
            let cm_in1 = meta.instance_column();
            let cm_out0 = meta.instance_column();
            let cm_out1 = meta.instance_column();
            let nf = meta.instance_column();
            let root = meta.instance_column();
            let s = meta.selector();
            meta.create_gate("anon_transfer_commit_merkle2", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(in0, Rotation::cur());
                let b = meta.query_advice(in1, Rotation::cur());
                let c = meta.query_advice(out0, Rotation::cur());
                let d = meta.query_advice(out1, Rotation::cur());
                let r0 = meta.query_advice(r_in0, Rotation::cur());
                let r1 = meta.query_advice(r_in1, Rotation::cur());
                let r2 = meta.query_advice(r_out0, Rotation::cur());
                let r3 = meta.query_advice(r_out1, Rotation::cur());
                let skq = meta.query_advice(sk, Rotation::cur());
                let serq = meta.query_advice(serial, Rotation::cur());
                let s0_0 = meta.query_advice(sib0_0, Rotation::cur());
                let s0_1 = meta.query_advice(sib0_1, Rotation::cur());
                let _s1_0 = meta.query_advice(sib1_0, Rotation::cur());
                let _s1_1 = meta.query_advice(sib1_1, Rotation::cur());
                let input_commitment_slot0 = meta.query_instance(cm_in0, Rotation::cur());
                let input_commitment_slot1 = meta.query_instance(cm_in1, Rotation::cur());
                let output_commitment_slot0 = meta.query_instance(cm_out0, Rotation::cur());
                let output_commitment_slot1 = meta.query_instance(cm_out1, Rotation::cur());
                let nullifier_instance = meta.query_instance(nf, Rotation::cur());
                let rootq = meta.query_instance(root, Rotation::cur());
                let computed_cm0 = poseidon_pair_expr(a.clone(), r0.clone());
                let computed_cm1 = poseidon_pair_expr(b.clone(), r1.clone());
                let computed_cm2 = poseidon_pair_expr(c.clone(), r2.clone());
                let computed_cm3 = poseidon_pair_expr(d.clone(), r3.clone());
                let cm0_root =
                    poseidon_pair_expr(poseidon_pair_expr(computed_cm0.clone(), s0_0), s0_1);
                let nf_exp = poseidon_pair_expr(skq.clone(), serq.clone());
                vec![
                    s.clone() * (a.clone() + b.clone() - (c.clone() + d.clone())),
                    s.clone() * (computed_cm0 - input_commitment_slot0),
                    s.clone() * (computed_cm1 - input_commitment_slot1),
                    s.clone() * (computed_cm2 - output_commitment_slot0),
                    s.clone() * (computed_cm3 - output_commitment_slot1),
                    s.clone() * (nf_exp - nullifier_instance),
                    s * (cm0_root - rootq),
                ]
            });
            (
                in0, in1, out0, out1, r_in0, r_in1, r_out0, r_out1, sk, serial, sib0_0, sib0_1,
                sib1_0, sib1_1, cm_in0, cm_in1, cm_out0, cm_out1, nf, root, s,
            )
        }
        #[allow(clippy::too_many_lines)]
        fn synthesize(
            &self,
            cfg: Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let (
                in0,
                in1,
                out0,
                out1,
                r_in0,
                r_in1,
                r_out0,
                r_out1,
                sk,
                serial,
                sib0_0,
                sib0_1,
                sib1_0,
                sib1_1,
                _cm_in0,
                _cm_in1,
                _cm_out0,
                _cm_out1,
                _nf,
                _root,
                s,
            ) = cfg;
            layouter.assign_region(
                || "anon_transfer_commit_merkle2",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "in0", in0 => Scalar::from(7))?;
                    advice!(region, "in1", in1 => Scalar::from(5))?;
                    advice!(region, "out0", out0 => Scalar::from(6))?;
                    advice!(region, "out1", out1 => Scalar::from(6))?;
                    advice!(region, "r_in0", r_in0 => Scalar::from(11))?;
                    advice!(region, "r_in1", r_in1 => Scalar::from(13))?;
                    advice!(region, "r_out0", r_out0 => Scalar::from(17))?;
                    advice!(region, "r_out1", r_out1 => Scalar::from(19))?;
                    advice!(region, "sk", sk => Scalar::from(1_234_567))?;
                    advice!(region, "serial", serial => Scalar::from(42))?;
                    advice!(region, "sib0_0", sib0_0 => Scalar::from(23))?;
                    advice!(region, "sib0_1", sib0_1 => Scalar::from(29))?;
                    advice!(region, "sib1_0", sib1_0 => Scalar::from(31))?;
                    advice!(region, "sib1_1", sib1_1 => Scalar::from(37))?;
                    Ok(())
                },
            )
        }
    }
    // Depth-8 membership variants with optional Poseidon gadget backing.
    #[derive(Clone, Default)]
    #[allow(dead_code)] // circuit scaffolding, constructed in gated tests/examples
    pub struct VoteBoolCommitMerkle8; // instances: [commit, root]
    const VOTE_BOOL_COMMIT_MERKLE8_SAMPLE_V: u64 = 1;
    const VOTE_BOOL_COMMIT_MERKLE8_SAMPLE_RHO: u64 = 12_345;
    const VOTE_BOOL_COMMIT_MERKLE8_SAMPLE_SIBS: [u64; 8] = [10, 11, 12, 13, 14, 15, 16, 17];
    const VOTE_BOOL_COMMIT_MERKLE8_SAMPLE_DIRS: [u64; 8] = [0; 8];
    fn poseidon_pow5(x: Scalar) -> Scalar {
        let x2 = x * x;
        let x4 = x2 * x2;
        x4 * x
    }
    pub(super) fn poseidon_pair(lhs: Scalar, rhs: Scalar) -> Scalar {
        let lhs = lhs + Scalar::from(7u64);
        let rhs = rhs + Scalar::from(13u64);
        Scalar::from(2u64) * poseidon_pow5(lhs) + Scalar::from(3u64) * poseidon_pow5(rhs)
    }
    fn poseidon_pow5_expr(
        expr: halo2_proofs::plonk::Expression<Scalar>,
    ) -> halo2_proofs::plonk::Expression<Scalar> {
        let squared = expr.clone() * expr.clone();
        let fourth = squared.clone() * squared;
        fourth * expr
    }
    fn poseidon_pair_expr(
        lhs: halo2_proofs::plonk::Expression<Scalar>,
        rhs: halo2_proofs::plonk::Expression<Scalar>,
    ) -> halo2_proofs::plonk::Expression<Scalar> {
        let lhs = lhs + halo2_proofs::plonk::Expression::Constant(Scalar::from(7u64));
        let rhs = rhs + halo2_proofs::plonk::Expression::Constant(Scalar::from(13u64));
        halo2_proofs::plonk::Expression::Constant(Scalar::from(2u64)) * poseidon_pow5_expr(lhs)
            + halo2_proofs::plonk::Expression::Constant(Scalar::from(3u64))
                * poseidon_pow5_expr(rhs)
    }
    pub(super) fn vote_bool_commit_merkle8_witnesses(
        v: Scalar,
        rho: Scalar,
        siblings: [Scalar; 8],
        dirs: [Scalar; 8],
    ) -> (Scalar, [Scalar; 8], Scalar) {
        let one = Scalar::from(1u64);
        let commit = poseidon_pair(v, rho);
        let mut prev = commit;
        let mut witnesses = [Scalar::from(0u64); 8];
        for i in 0..8 {
            let sib = siblings[i];
            let dir = dirs[i];
            let forward = poseidon_pair(prev, sib);
            let reverse = poseidon_pair(sib, prev);
            let witness = (one - dir) * forward + dir * reverse;
            witnesses[i] = witness;
            prev = witness;
        }
        (commit, witnesses, prev)
    }
    pub(super) fn vote_bool_commit_merkle8_sample_inputs()
    -> (Scalar, Scalar, [Scalar; 8], [Scalar; 8]) {
        (
            Scalar::from(VOTE_BOOL_COMMIT_MERKLE8_SAMPLE_V),
            Scalar::from(VOTE_BOOL_COMMIT_MERKLE8_SAMPLE_RHO),
            VOTE_BOOL_COMMIT_MERKLE8_SAMPLE_SIBS.map(Scalar::from),
            VOTE_BOOL_COMMIT_MERKLE8_SAMPLE_DIRS.map(Scalar::from),
        )
    }
    #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
    impl Circuit<Scalar> for VoteBoolCommitMerkle8 {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // v
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // rho
            // 8 siblings
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8],
            // 8 direction bits
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8],
            // 8 intermediate nodes
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8],
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let v = meta.advice_column();
            let rho = meta.advice_column();
            let mut sibs = [v; 8];
            let mut dirs = [rho; 8];
            let mut ws = [rho; 8];
            for column in &mut sibs {
                *column = meta.advice_column();
            }
            for column in &mut dirs {
                *column = meta.advice_column();
            }
            for column in &mut ws {
                *column = meta.advice_column();
            }
            let inst_cm = meta.instance_column();
            let inst_root = meta.instance_column();
            let s = meta.selector();
            // Inline Pow5 constraints keep the fallback path dependency-light while matching
            // the Poseidon round function used by the gadget-enabled build.
            meta.create_gate("vote_commit_merkle8", |meta| {
                let s = meta.query_selector(s);
                let vq = meta.query_advice(v, Rotation::cur());
                let rhoq = meta.query_advice(rho, Rotation::cur());
                let cmq = meta.query_instance(inst_cm, Rotation::cur());
                let rootq = meta.query_instance(inst_root, Rotation::cur());
                let constant =
                    |value: u64| halo2_proofs::plonk::Expression::Constant(Scalar::from(value));
                let shift = |expr: halo2_proofs::plonk::Expression<Scalar>, offset: u64| {
                    expr + constant(offset)
                };
                let pow5 = |expr: halo2_proofs::plonk::Expression<Scalar>| {
                    let squared = expr.clone() * expr.clone();
                    let fourth = squared.clone() * squared.clone();
                    fourth * expr
                };
                let pedersen_pair =
                    |lhs: halo2_proofs::plonk::Expression<Scalar>,
                     rhs: halo2_proofs::plonk::Expression<Scalar>| {
                        constant(2) * pow5(lhs) + constant(3) * pow5(rhs)
                    };
                let one = constant(1);
                let boolc = vq.clone() * (vq.clone() - one.clone());
                let commit_hash = pedersen_pair(shift(vq.clone(), 7), shift(rhoq.clone(), 13));
                let commitment_delta = commit_hash.clone() - cmq.clone();
                // chain 8 levels: w0 = H(cm, sib0); w7 == root
                let mut cons = vec![s.clone() * boolc, s.clone() * commitment_delta];
                let mut prev = commit_hash;
                for i in 0..8 {
                    let sibling = meta.query_advice(sibs[i], Rotation::cur());
                    let direction_bit = meta.query_advice(dirs[i], Rotation::cur());
                    let witness = meta.query_advice(ws[i], Rotation::cur());
                    // boolean direction bit
                    cons.push(
                        s.clone() * (direction_bit.clone() * (direction_bit.clone() - one.clone())),
                    );
                    let forward_hash =
                        pedersen_pair(shift(prev.clone(), 7), shift(sibling.clone(), 13));
                    let reverse_hash =
                        pedersen_pair(shift(sibling.clone(), 7), shift(prev.clone(), 13));
                    let expected_branch = (one.clone() - direction_bit.clone())
                        * forward_hash.clone()
                        + direction_bit.clone() * reverse_hash;
                    cons.push(s.clone() * (witness.clone() - expected_branch));
                    prev = witness;
                }
                cons.push(s * (prev - rootq));
                cons
            });
            (v, rho, sibs, dirs, ws, inst_cm, inst_root, s)
        }
        fn synthesize(
            &self,
            (v, rho, sibs, dirs, ws, _cm, _root, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            layouter.assign_region(
                || "vote_commit_merkle8",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    let (v_val, rho_val, sibling_vals, dir_vals) =
                        vote_bool_commit_merkle8_sample_inputs();
                    let (_commit, witness_vals, _root) =
                        vote_bool_commit_merkle8_witnesses(v_val, rho_val, sibling_vals, dir_vals);
                    advice!(region, "v", v => v_val)?;
                    advice!(region, "rho", rho => rho_val)?;
                    for (i, col) in sibs.iter().enumerate() {
                        let sib_val = sibling_vals[i];
                        advice!(region, move "sib{i}", *col => sib_val)?;
                    }
                    for (i, col) in dirs.iter().enumerate() {
                        let dir_val = dir_vals[i];
                        advice!(region, move "dir{i}", *col => dir_val)?;
                    }
                    for (i, col) in ws.iter().enumerate() {
                        let w_val = witness_vals[i];
                        advice!(region, move "w{i}", *col => w_val)?;
                    }
                    Ok(())
                },
            )
        }
    }
    #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
    impl Circuit<Scalar> for VoteBoolCommitMerkle8 {
        type Config = (
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // v
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // rho
            // 8 siblings
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8],
            // 8 direction bits
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8],
            // 8 intermediate nodes
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8],
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // commit
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>, // root
            Selector,
            poseidon::Pow5Config<Scalar, 3, 2>,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let v = meta.advice_column();
            let rho = meta.advice_column();
            let mut sibs = [v; 8];
            let mut dirs = [rho; 8];
            let mut ws = [rho; 8];
            for i in 0..8 {
                sibs[i] = meta.advice_column();
            }
            for i in 0..8 {
                dirs[i] = meta.advice_column();
            }
            for i in 0..8 {
                ws[i] = meta.advice_column();
                meta.enable_equality(ws[i]);
            }
            let inst_cm = meta.instance_column();
            let inst_root = meta.instance_column();
            let s = meta.selector();
            let st0 = meta.advice_column();
            let st1 = meta.advice_column();
            let st2 = meta.advice_column();
            let partial = meta.advice_column();
            let rc_a = meta.fixed_column();
            let rc_b = meta.fixed_column();
            let poseidon_cfg =
                poseidon::Pow5Chip::configure(meta, [st0, st1, st2], partial, rc_a, rc_b);
            meta.create_gate("vote_commit_merkle8", |meta| {
                let s = meta.query_selector(s);
                let vq = meta.query_advice(v, Rotation::cur());
                let rhoq = meta.query_advice(rho, Rotation::cur());
                let cmq = meta.query_instance(inst_cm, Rotation::cur());
                let rootq = meta.query_instance(inst_root, Rotation::cur());
                let constant =
                    |value: u64| halo2_proofs::plonk::Expression::Constant(Scalar::from(value));
                let shift = |expr: halo2_proofs::plonk::Expression<Scalar>, offset: u64| {
                    expr + constant(offset)
                };
                let pow5 = |expr: halo2_proofs::plonk::Expression<Scalar>| {
                    let squared = expr.clone() * expr.clone();
                    let fourth = squared.clone() * squared.clone();
                    fourth * expr
                };
                let pedersen_pair =
                    |lhs: halo2_proofs::plonk::Expression<Scalar>,
                     rhs: halo2_proofs::plonk::Expression<Scalar>| {
                        constant(2) * pow5(lhs) + constant(3) * pow5(rhs)
                    };
                let one = constant(1);
                let boolc = vq.clone() * (vq.clone() - one.clone());
                let commit_hash = pedersen_pair(shift(vq.clone(), 7), shift(rhoq.clone(), 13));
                let commitment_delta = commit_hash.clone() - cmq.clone();
                let mut cons = vec![s.clone() * boolc, s.clone() * commitment_delta];
                let mut prev = commit_hash;
                for i in 0..8 {
                    let sibling = meta.query_advice(sibs[i], Rotation::cur());
                    let direction_bit = meta.query_advice(dirs[i], Rotation::cur());
                    let witness = meta.query_advice(ws[i], Rotation::cur());
                    cons.push(
                        s.clone() * (direction_bit.clone() * (direction_bit.clone() - one.clone())),
                    );
                    let forward_hash =
                        pedersen_pair(shift(prev.clone(), 7), shift(sibling.clone(), 13));
                    let reverse_hash =
                        pedersen_pair(shift(sibling.clone(), 7), shift(prev.clone(), 13));
                    let expected_branch = (one.clone() - direction_bit.clone())
                        * forward_hash.clone()
                        + direction_bit.clone() * reverse_hash;
                    cons.push(s.clone() * (witness.clone() - expected_branch));
                    prev = witness;
                }
                cons.push(s * (prev - rootq));
                cons
            });
            (v, rho, sibs, dirs, ws, inst_cm, inst_root, s, poseidon_cfg)
        }
        fn synthesize(
            &self,
            (v, rho, sibs, dirs, ws, _cm, _root, s, poseidon_cfg): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            use halo2_proofs::circuit::Value;
            let (v_val, rho_val, sibling_vals, dir_vals) = vote_bool_commit_merkle8_sample_inputs();
            let (commit_val, witness_vals, _root_val) =
                vote_bool_commit_merkle8_witnesses(v_val, rho_val, sibling_vals, dir_vals);
            let mut w_cells = Vec::with_capacity(8);
            layouter.assign_region(
                || "vote_commit_merkle8",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "v", v => v_val)?;
                    advice!(region, "rho", rho => rho_val)?;
                    for (i, col) in sibs.iter().enumerate() {
                        let sib_val = sibling_vals[i];
                        advice!(region, move "sib{i}", *col => sib_val)?;
                    }
                    for (i, col) in dirs.iter().enumerate() {
                        let dir_val = dir_vals[i];
                        advice!(region, move "dir{i}", *col => dir_val)?;
                    }
                    for (i, col) in ws.iter().enumerate() {
                        let w_val = witness_vals[i];
                        let cell = advice!(region, move "w{i}", *col => w_val)?;
                        w_cells.push(cell);
                    }
                    Ok(())
                },
            )?;
            let poseidon_chip = poseidon::Poseidon2ChipWrapper::new();
            let mut prev_scalar = commit_val;
            for (i, witness_cell) in w_cells.iter().enumerate() {
                let dir_val = dir_vals[i];
                let sib_val = sibling_vals[i];
                let is_right = dir_val == Scalar::from(1u64);
                let (lhs, rhs) = if is_right {
                    (sib_val, prev_scalar)
                } else {
                    (prev_scalar, sib_val)
                };
                let mut ns = layouter.namespace(|| format!("poseidon_vote_merkle8_layer_{i}"));
                let digest = poseidon_chip.hash2_chip(
                    &mut ns,
                    &poseidon_cfg,
                    Value::known(lhs),
                    Value::known(rhs),
                )?;
                layouter.constrain_equal(digest.cell(), witness_cell.cell())?;
                prev_scalar = witness_vals[i];
            }
            Ok(())
        }
    }
    #[derive(Clone, Default)]
    #[allow(dead_code)] // circuit scaffolding, constructed in gated tests/examples
    pub struct AnonTransfer2x2CommitMerkle8; // instances: [cm_in0, cm_in1, cm_out0, cm_out1, nf, root]
    impl Circuit<Scalar> for AnonTransfer2x2CommitMerkle8 {
        type Config = (
            // values and randomness
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_in1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out0
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // r_out1
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // sk
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>, // serial
            // siblings for in0 depth-8 path
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8], // sibs
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8], // dirs
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>; 8], // w nodes
            // instances
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>; 5], // cm_in0, cm_in1, cm_out0, cm_out1, nf
            halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,      // root
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;
        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self
        }
        #[allow(clippy::too_many_lines)]
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            let in0 = meta.advice_column();
            let in1 = meta.advice_column();
            let out0 = meta.advice_column();
            let out1 = meta.advice_column();
            let r0 = meta.advice_column();
            let r1 = meta.advice_column();
            let r2 = meta.advice_column();
            let r3 = meta.advice_column();
            let sk = meta.advice_column();
            let serial = meta.advice_column();
            let mut sib = [in0; 8];
            let mut dir = [in1; 8];
            let mut w = [out0; 8];
            for column in &mut sib {
                *column = meta.advice_column();
            }
            for column in &mut dir {
                *column = meta.advice_column();
            }
            for column in &mut w {
                *column = meta.advice_column();
            }
            let cm_cols = [
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
                meta.instance_column(),
            ];
            let root = meta.instance_column();
            let s = meta.selector();
            // Poseidon gadget wiring mirrors the halo2-ecc interface; once `zk-halo2-ipa-poseidon`
            // lands we can swap the inline Pow5 equations below without changing the layout.
            meta.create_gate("anon_transfer_commit_merkle8", |meta| {
                let s = meta.query_selector(s);
                let a = meta.query_advice(in0, Rotation::cur());
                let b = meta.query_advice(in1, Rotation::cur());
                let c = meta.query_advice(out0, Rotation::cur());
                let d = meta.query_advice(out1, Rotation::cur());
                let r0q = meta.query_advice(r0, Rotation::cur());
                let r1q = meta.query_advice(r1, Rotation::cur());
                let r2q = meta.query_advice(r2, Rotation::cur());
                let r3q = meta.query_advice(r3, Rotation::cur());
                let skq = meta.query_advice(sk, Rotation::cur());
                let serq = meta.query_advice(serial, Rotation::cur());
                let cm_in0 = meta.query_instance(cm_cols[0], Rotation::cur());
                let cm_in1 = meta.query_instance(cm_cols[1], Rotation::cur());
                let cm_out0 = meta.query_instance(cm_cols[2], Rotation::cur());
                let cm_out1 = meta.query_instance(cm_cols[3], Rotation::cur());
                let nf = meta.query_instance(cm_cols[4], Rotation::cur());
                let rootq = meta.query_instance(root, Rotation::cur());
                let h = |x: halo2_proofs::plonk::Expression<Scalar>,
                         r: halo2_proofs::plonk::Expression<Scalar>| {
                    let x2 = x.clone() * x.clone();
                    let x4 = x2.clone() * x2.clone();
                    let x5 = x4 * x.clone();
                    let r2 = r.clone() * r.clone();
                    let r4 = r2.clone() * r2.clone();
                    let r5 = r4 * r.clone();
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * x5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * r5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(7))
                };
                // cm constraints and conservation
                let cm0 = h(a.clone(), r0q.clone());
                let cm1 = h(b.clone(), r1q.clone());
                let cm2 = h(c.clone(), r2q.clone());
                let cm3 = h(d.clone(), r3q.clone());
                let nf_exp = h(skq.clone(), serq.clone());
                let mut cons = vec![
                    s.clone() * (a.clone() + b.clone() - (c.clone() + d.clone())),
                    s.clone() * (cm0.clone() - cm_in0),
                    s.clone() * (cm1 - cm_in1),
                    s.clone() * (cm2 - cm_out0),
                    s.clone() * (cm3 - cm_out1),
                    s.clone() * (nf_exp - nf),
                ];
                let constant =
                    |value: u64| halo2_proofs::plonk::Expression::Constant(Scalar::from(value));
                let shift = |expr: halo2_proofs::plonk::Expression<Scalar>, offset: u64| {
                    expr + constant(offset)
                };
                let pow5 = |expr: halo2_proofs::plonk::Expression<Scalar>| {
                    let squared = expr.clone() * expr.clone();
                    let fourth = squared.clone() * squared.clone();
                    fourth * expr
                };
                let pedersen_pair =
                    |lhs: halo2_proofs::plonk::Expression<Scalar>,
                     rhs: halo2_proofs::plonk::Expression<Scalar>| {
                        constant(2) * pow5(lhs) + constant(3) * pow5(rhs)
                    };
                // depth-8 membership for cm0
                let mut prev = cm0;
                for i in 0..8 {
                    let sibling = meta.query_advice(sib[i], Rotation::cur());
                    let direction_bit = meta.query_advice(dir[i], Rotation::cur());
                    let witness = meta.query_advice(w[i], Rotation::cur());
                    cons.push(
                        s.clone() * (direction_bit.clone() * (direction_bit.clone() - constant(1))),
                    );
                    let forward_hash =
                        pedersen_pair(shift(prev.clone(), 7), shift(sibling.clone(), 13));
                    let reverse_hash =
                        pedersen_pair(shift(sibling.clone(), 7), shift(prev.clone(), 13));
                    let expected_branch = (constant(1) - direction_bit.clone())
                        * forward_hash.clone()
                        + direction_bit.clone() * reverse_hash;
                    cons.push(s.clone() * (witness.clone() - expected_branch));
                    prev = witness;
                }
                cons.push(s * (prev - rootq));
                cons
            });
            (
                in0, in1, out0, out1, r0, r1, r2, r3, sk, serial, sib, dir, w, cm_cols, root, s,
            )
        }
        #[allow(clippy::too_many_lines)]
        fn synthesize(
            &self,
            cfg: Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let (in0, in1, out0, out1, r0, r1, r2, r3, sk, serial, sib, dir, w, _cm_cols, _root, s) =
                cfg;
            layouter.assign_region(
                || "anon_transfer_commit_merkle8",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    advice!(region, "in0", in0 => Scalar::from(7))?;
                    advice!(region, "in1", in1 => Scalar::from(5))?;
                    advice!(region, "out0", out0 => Scalar::from(6))?;
                    advice!(region, "out1", out1 => Scalar::from(6))?;
                    advice!(region, "r0", r0 => Scalar::from(11))?;
                    advice!(region, "r1", r1 => Scalar::from(13))?;
                    advice!(region, "r2", r2 => Scalar::from(17))?;
                    advice!(region, "r3", r3 => Scalar::from(19))?;
                    advice!(region, "sk", sk => Scalar::from(1_234_567))?;
                    advice!(region, "serial", serial => Scalar::from(42))?;
                    for (i, col) in sib.iter().enumerate() {
                        advice!(region, move "sib{i}", *col => Scalar::from(20 + i as u64))?;
                    }
                    for (i, col) in dir.iter().enumerate() {
                        advice!(region, move "dir{i}", *col => Scalar::from(0))?;
                    }
                    let mut acc = Scalar::from(0);
                    for (i, col) in w.iter().enumerate() {
                        acc += Scalar::from(20 + i as u64);
                        advice!(region, move "w{i}", *col => acc)?;
                    }
                    Ok(())
                },
            )
        }
    }
}
#[cfg(all(
    test,
    feature = "zk-tests",
    feature = "halo2-dev-tests",
    feature = "zk-halo2"
))]
#[allow(clippy::too_many_lines)]
fn verify_halo2(backend: &str, proof: &ProofBox, vk: Option<&VerifyingKeyBox>) -> bool {
    use halo2_backend::Scalar;
    let Some(vk_box) = vk else { return false };
    // Sanity: backends must match
    // Note: caller already checked `proof.backend == attachment.backend` in ISI and executor
    // paths, but double-check here for robustness.
    // Also require non-empty payloads.
    if vk_box.backend != proof.backend || proof.bytes.is_empty() || vk_box.bytes.is_empty() {
        return false;
    }
    // Parse params and proof/instances using shared helpers
    let params = match halo2_params_for_verifier_v1(vk_box.bytes.as_slice(), backend) {
        Some(p) => p,
        None => return false,
    };
    let (proof_payload, inst_cols) =
        match zkparse::strict_proof_and_instances(proof.bytes.as_slice()) {
            Ok(x) => x,
            Err(_) => return false,
        };
    let col_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
    let normalized = backend.replace("/ipa/", "/");
    match normalized.as_str() {
        #[cfg(test)]
        "halo2/pasta/tiny-add" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::Add,
                |vk| {
                    verify_halo2_ipa_payload_no_instances(&params, vk, proof_payload.as_slice())
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-mul" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::Mul,
                |vk| {
                    verify_halo2_ipa_payload_optional_columns(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    )
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-add-2rows" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AddTwoRows,
                |vk| {
                    verify_halo2_ipa_payload_no_instances(&params, vk, proof_payload.as_slice())
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-add-public" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AddPublic,
                |vk| {
                    verify_halo2_ipa_payload_optional_columns(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    )
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-mul-public" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::MulPublic,
                |vk| {
                    verify_halo2_ipa_payload_optional_columns(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    )
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-id-public" => {
            if col_refs.is_empty() {
                // requires a public instance
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::IdPublic,
                |vk| {
                    verify_halo2_ipa_payload_columns(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    )
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-add3" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AddThree,
                |vk| {
                    verify_halo2_ipa_payload_no_instances(&params, vk, proof_payload.as_slice())
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-add2inst-public" => {
            if col_refs.len() < 2 {
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AddTwoInstPublic,
                |vk| {
                    verify_halo2_ipa_payload_columns(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    )
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-anon-transfer-2x2" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AnonTransfer2x2,
                |vk| {
                    verify_halo2_ipa_payload_no_instances(&params, vk, proof_payload.as_slice())
                }
            )
        }
        KAIGI_ROSTER_BACKEND => {
            if col_refs.len() < 2 {
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                KaigiRosterJoinCircuit::default(),
                |vk| {
                    match verify_halo2_ipa_payload_columns_result(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    ) {
                        Ok(()) => true,
                        Err(err) => {
                            iroha_logger::debug!(
                                backend,
                                normalized = normalized.as_str(),
                                error = ?err,
                                "halo2 kaigi roster proof rejected (verify_proof failed)"
                            );
                            false
                        }
                    }
                }
            )
        }
        KAIGI_USAGE_BACKEND => {
            if col_refs.is_empty() {
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                KaigiUsageCommitmentCircuit::default(),
                |vk| {
                    match verify_halo2_ipa_payload_columns_result(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    ) {
                        Ok(()) => true,
                        Err(err) => {
                            iroha_logger::debug!(
                                backend,
                                normalized = normalized.as_str(),
                                error = ?err,
                                "halo2 kaigi usage proof rejected (verify_proof failed)"
                            );
                            false
                        }
                    }
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-vote-bool" => {
            let circuit = pasta_tiny::VoteBool;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            verify_halo2_ipa_payload_no_instances(&params, vk_h2.as_ref(), proof_payload.as_slice())
        }
        _ => false,
    }
}
/// Transparent Halo2 IPA over Pasta (no trusted setup).
///
/// Accepts a ZK1 envelope containing an `IPAK` TLV to derive Params.
#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_lines)]
fn verify_halo2_ipa(backend: &str, proof: &ProofBox, vk: Option<&VerifyingKeyBox>) -> bool {
    let reject = |reason: &'static str| {
        iroha_logger::debug!(backend, reason, "halo2 ipa proof rejected");
        false
    };
    let Some(vk_box) = vk else {
        return reject("missing verifying key");
    };
    if vk_box.backend != proof.backend {
        return reject("verifying key backend mismatch");
    }
    if proof.bytes.is_empty() {
        return reject("empty proof bytes");
    }
    if vk_box.bytes.is_empty() {
        return reject("empty verifying key bytes");
    }
    let params: PastaParams = match halo2_params_for_verifier_v1(vk_box.bytes.as_slice(), backend) {
        Some(p) => p,
        None => return reject("missing/invalid IPAK parameters in verifying key envelope"),
    };
    // Production proofs use one strict ZK1 carrier. The older binary envelope
    // has caller-controlled `n_in`, `n_out`, and `flags` header fields that are
    // not absorbed by Halo2's transcript, so accepting it would leave multiple
    // unauthenticated encodings for the same proof and instance columns.
    let (proof_payload, inst_cols) =
        match zkparse::strict_proof_and_instances(proof.bytes.as_slice()) {
            Ok(x) => x,
            Err(_) => return reject("invalid ZK1 proof envelope payload"),
        };
    let col_refs: Vec<&[halo2_backend::Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
    // These canonical identifiers already carry the `/ipa/` component.
    // Dispatch them before the legacy built-in normalization below removes
    // that component; otherwise exact circuit predicates can never match and
    // a proof that passed raw IPA verification is rejected at the envelope
    // boundary.
    if confidential_v2::is_confidential_transfer_v2_circuit_id(backend) {
        if col_refs.len() != 9 || col_refs.iter().any(|col| col.len() != 1) {
            return false;
        }
        return cached_vk_for!(
            &params,
            backend,
            vk_box,
            confidential_v2::secure_relation_v3::ConfidentialTransferCircuitV3::<
                { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
            |vk| {
                verify_halo2_ipa_payload_columns(&params, vk, proof_payload.as_slice(), &col_refs)
            }
        );
    }
    if confidential_v2::is_kagemusha_topup_shield_v2_circuit_id(backend) {
        if col_refs.len() != 11 || col_refs.iter().any(|col| col.len() != 1) {
            return false;
        }
        return cached_vk_for!(
            &params,
            backend,
            vk_box,
            confidential_v2::secure_relation_v3::KagemushaTopUpShieldCircuitV3::<
                { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
            |vk| {
                verify_halo2_ipa_payload_columns(&params, vk, proof_payload.as_slice(), &col_refs)
            }
        );
    }
    if confidential_v2::is_confidential_unshield_v2_circuit_id(backend) {
        if col_refs.len() != 8 || col_refs.iter().any(|col| col.len() != 1) {
            return false;
        }
        return cached_vk_for!(
            &params,
            backend,
            vk_box,
            confidential_v2::secure_relation_v3::ConfidentialUnshieldFullCircuitV3::<
                { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
            |vk| {
                verify_halo2_ipa_payload_columns(&params, vk, proof_payload.as_slice(), &col_refs)
            }
        );
    }
    if confidential_v2::is_confidential_unshield_v3_circuit_id(backend) {
        if col_refs.len() != 9 || col_refs.iter().any(|col| col.len() != 1) {
            return false;
        }
        return cached_vk_for!(
            &params,
            backend,
            vk_box,
            confidential_v2::secure_relation_v3::ConfidentialUnshieldChangeCircuitV4::<
                { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
            >::default(),
            |vk| {
                verify_halo2_ipa_payload_columns(&params, vk, proof_payload.as_slice(), &col_refs)
            }
        );
    }
    // For IPA, we normalize backend tag to reuse circuit mapping
    let normalized = backend.replace("/ipa/", "/");
    #[cfg(test)]
    macro_rules! verify_test_circuit {
        ($circuit:expr, $mode:ident $(, $reject:expr)?) => {{
            let circuit = $circuit;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            $(if $reject {
                return false;
            })?
            verify_test_circuit!(@verify $mode, vk_h2.as_ref())
        }};
        (using $circuit:ident, $mode:ident $(, $reject:expr)?) => {{
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &$circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            $(if $reject {
                return false;
            })?
            verify_test_circuit!(@verify $mode, vk_h2.as_ref())
        }};
        (@verify no_instances, $vk:expr) => {
            verify_halo2_ipa_payload_no_instances(&params, $vk, proof_payload.as_slice())
        };
        (@verify optional_columns, $vk:expr) => {
            verify_halo2_ipa_payload_optional_columns(
                &params,
                $vk,
                proof_payload.as_slice(),
                &col_refs,
            )
        };
        (@verify columns, $vk:expr) => {
            verify_halo2_ipa_payload_columns(
                &params,
                $vk,
                proof_payload.as_slice(),
                &col_refs,
            )
        };
    }
    match normalized.as_str() {
        #[cfg(test)]
        "halo2/pasta/tiny-add" => {
            verify_test_circuit!(pasta_tiny::Add, no_instances)
        }
        #[cfg(test)]
        "halo2/pasta/tiny-mul" => {
            verify_test_circuit!(pasta_tiny::Mul, optional_columns)
        }
        #[cfg(test)]
        "halo2/pasta/tiny-add-2rows" => {
            verify_test_circuit!(pasta_tiny::AddTwoRows, no_instances)
        }
        #[cfg(test)]
        "halo2/pasta/tiny-add-public" => {
            verify_test_circuit!(pasta_tiny::AddPublic, optional_columns)
        }
        #[cfg(test)]
        "halo2/pasta/tiny-mul-public" => {
            verify_test_circuit!(pasta_tiny::MulPublic, optional_columns)
        }
        #[cfg(test)]
        "halo2/pasta/tiny-id-public" => {
            verify_test_circuit!(pasta_tiny::IdPublic, columns, col_refs.is_empty())
        }
        #[cfg(test)]
        "halo2/pasta/tiny-add3" => {
            verify_test_circuit!(pasta_tiny::AddThree, no_instances)
        }
        #[cfg(test)]
        "halo2/pasta/tiny-add2inst-public" => {
            verify_test_circuit!(pasta_tiny::AddTwoInstPublic, columns, col_refs.len() < 2)
        }
        #[cfg(test)]
        "halo2/pasta/ivm-overlay-bind" => {
            // Instances: 8 columns (code_hash limbs + overlay_hash limbs), 1 row each.
            if col_refs.len() != 8 || col_refs.iter().any(|col| col.len() != 1) {
                return false;
            }
            verify_test_circuit!(pasta_tiny::IvmOverlayBind::default(), columns)
        }
        "halo2/pasta/ivm-execution-v1" => {
            // Instances: 16 columns (code_hash limbs + overlay_hash limbs + events_commitment limbs + gas_policy_commitment limbs), 1 row each.
            if col_refs.len() != 16 || col_refs.iter().any(|col| col.len() != 1) {
                return false;
            }
            cached_vk_for!(
                &params,
                &vk_box.backend,
                vk_box,
                pasta_tiny::IvmExecutionBindV1::default(),
                |vk| {
                    verify_halo2_ipa_payload_columns(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    )
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/tiny-anon-transfer-2x2" => {
            verify_test_circuit!(pasta_tiny::AnonTransfer2x2, no_instances)
        }
        #[cfg(test)]
        "halo2/pasta/anon-transfer-2x2" => {
            // Instances: 5 columns [cm_in0, cm_in1, cm_out0, cm_out1, nf], 1 row
            if col_refs.len() < 5 {
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AnonTransfer2x2Commit,
                |vk| {
                    verify_halo2_ipa_payload_columns(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    )
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/anon-transfer-2x2-merkle2" => {
            // Instances: 6 columns [cm_in0, cm_in1, cm_out0, cm_out1, nf, root], 1 row
            if col_refs.len() < 6 {
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AnonTransfer2x2CommitMerkle2,
                |vk| {
                    verify_halo2_ipa_payload_columns(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    )
                }
            )
        }
        #[cfg(test)]
        "halo2/pasta/anon-transfer-2x2-merkle8" => {
            // Use depth-8 generic with dual membership; select algorithm by backend suffix
            let use_poseidon = backend.ends_with("-poseidon");
            if use_poseidon {
                verify_test_circuit!(
                    poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>,
                    columns,
                    col_refs.len() < 6
                )
            } else {
                verify_test_circuit!(
                    depth::AnonTransfer2x2CommitMerkle::<8>,
                    columns,
                    col_refs.len() < 6
                )
            }
        }
        #[cfg(test)]
        "halo2/pasta/tiny-vote-bool" => {
            verify_test_circuit!(pasta_tiny::VoteBool, no_instances)
        }
        #[cfg(test)]
        "halo2/pasta/tiny-commit-open" => {
            // If Poseidon gadgets are enabled, use the Poseidon-backed variant.
            #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
            let circuit = pasta_tiny::poseidon::CommitOpenPoseidon;
            #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
            let circuit = pasta_tiny::CommitOpen;
            verify_test_circuit!(using circuit, columns, col_refs.is_empty())
        }
        #[cfg(test)]
        "halo2/pasta/tiny-merkle2" => {
            // If Poseidon gadgets are enabled, use the Poseidon-backed variant.
            #[cfg(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests"))]
            let circuit = pasta_tiny::poseidon::Merkle2Poseidon;
            #[cfg(not(all(feature = "zk-halo2-ipa-poseidon", feature = "halo2-dev-tests")))]
            let circuit = pasta_tiny::Merkle2;
            verify_test_circuit!(using circuit, columns, col_refs.is_empty())
        }
        #[cfg(test)]
        "halo2/pasta/vote-bool-commit" => {
            // Instances: [commit], 1 row
            verify_test_circuit!(pasta_tiny::VoteBoolCommit, columns, col_refs.is_empty())
        }
        #[cfg(test)]
        "halo2/pasta/vote-bool-commit-merkle2" => {
            // Instances: [commit, root], 1 row
            verify_test_circuit!(
                pasta_tiny::VoteBoolCommitMerkle2,
                columns,
                col_refs.len() < 2
            )
        }
        #[cfg(test)]
        "halo2/pasta/vote-bool-commit-merkle8" => {
            // Use depth-8 generic; select algorithm by backend suffix
            let use_poseidon = backend.ends_with("-poseidon");
            if use_poseidon {
                verify_test_circuit!(
                    poseidon_depth::VoteBoolCommitMerklePoseidon::<8>,
                    columns,
                    col_refs.len() < 2
                )
            } else {
                verify_test_circuit!(
                    depth::VoteBoolCommitMerkle::<8>,
                    columns,
                    col_refs.len() < 2
                )
            }
        }
        // Depth-16 variants
        #[cfg(test)]
        "halo2/pasta/anon-transfer-2x2-merkle16" => {
            let use_poseidon = backend.ends_with("-poseidon");
            if use_poseidon {
                verify_test_circuit!(
                    poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>,
                    columns,
                    col_refs.len() < 6
                )
            } else {
                verify_test_circuit!(
                    depth::AnonTransfer2x2CommitMerkle::<16>,
                    columns,
                    col_refs.len() < 6
                )
            }
        }
        #[cfg(test)]
        "halo2/pasta/vote-bool-commit-merkle16" => {
            let use_poseidon = backend.ends_with("-poseidon");
            if use_poseidon {
                verify_test_circuit!(
                    poseidon_depth::VoteBoolCommitMerklePoseidon::<16>,
                    columns,
                    col_refs.len() < 2
                )
            } else {
                verify_test_circuit!(
                    depth::VoteBoolCommitMerkle::<16>,
                    columns,
                    col_refs.len() < 2
                )
            }
        }
        KAIGI_ROSTER_BACKEND => {
            if col_refs.len() < 2 {
                return false;
            }
            cached_vk_for!(
                &params,
                &vk_box.backend,
                vk_box,
                KaigiRosterJoinCircuit::default(),
                |vk| {
                    match verify_halo2_ipa_payload_columns_result(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    ) {
                        Ok(()) => true,
                        Err(err) => {
                            iroha_logger::debug!(
                                backend,
                                normalized = normalized.as_str(),
                                error = ?err,
                                "halo2 kaigi roster proof rejected (verify_proof failed)"
                            );
                            false
                        }
                    }
                }
            )
        }
        KAIGI_USAGE_BACKEND => {
            if col_refs.is_empty() {
                return false;
            }
            cached_vk_for!(
                &params,
                &vk_box.backend,
                vk_box,
                KaigiUsageCommitmentCircuit::default(),
                |vk| {
                    match verify_halo2_ipa_payload_columns_result(
                        &params,
                        vk,
                        proof_payload.as_slice(),
                        &col_refs,
                    ) {
                        Ok(()) => true,
                        Err(err) => {
                            iroha_logger::debug!(
                                backend,
                                normalized = normalized.as_str(),
                                error = ?err,
                                "halo2 kaigi usage proof rejected (verify_proof failed)"
                            );
                            false
                        }
                    }
                }
            )
        }
        _ => false,
    }
}
#[cfg(all(
    test,
    feature = "zk-tests",
    feature = "halo2-dev-tests",
    not(feature = "zk-halo2")
))]
fn verify_halo2(_backend: &str, _proof: &ProofBox, _vk: Option<&VerifyingKeyBox>) -> bool {
    // Feature disabled: refuse Halo2 proofs to avoid silent acceptance of forged transcripts.
    false
}
#[cfg(all(test, feature = "zk-preverify"))]
#[path = "zk/trace_queue_tests.rs"]
mod trace_queue_tests;
#[cfg(test)]
mod preverify_tests {
    use super::*;
    use PreverifyResult::*;
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};
    macro_rules! assert_preverify {
        (
            $proof:ident,
            $vk:ident,
            $dedup:ident,
            $vk_hash:ident,
            $result:expr
            $(, $message:expr)?
        ) => {
            assert_eq!(
                preverify_with_budget(
                    &$proof,
                    Some(&$vk),
                    &mut $dedup,
                    0,
                    Some($vk_hash),
                    Some($vk_hash),
                    true,
                ),
                $result
                $(, $message)?
            )
        };
        (
            $proof:ident,
            $vk:ident,
            $dedup:ident,
            $budget:expr,
            $resolved:expr,
            $expected:expr,
            $active:expr,
            $result:expr
            $(, $message:expr)?
        ) => {
            assert_eq!(
                preverify_with_budget(
                    &$proof,
                    Some(&$vk),
                    &mut $dedup,
                    $budget,
                    $resolved,
                    $expected,
                    $active,
                ),
                $result
                $(, $message)?
            )
        };
    }
    fn preverify_enveloped_proof(vk_hash: [u8; 32]) -> ProofBox {
        preverify_enveloped_proof_for_backend(
            ZK_BACKEND_HALO2_IPA,
            BackendTag::Halo2IpaPasta,
            IVM_EXECUTION_V1_CIRCUIT_ID,
            vk_hash,
        )
    }
    fn preverify_enveloped_proof_for_backend(
        backend: &str,
        envelope_backend: BackendTag,
        circuit_id: &str,
        vk_hash: [u8; 32],
    ) -> ProofBox {
        let public_inputs = if envelope_backend == BackendTag::Halo2IpaPasta {
            halo2_ipa_public_inputs_schema_v1(circuit_id)
                .map_or_else(|| vec![0x55; 32], |schema| schema.to_vec())
        } else {
            vec![0x55; 32]
        };
        let envelope = OpenVerifyEnvelope {
            backend: envelope_backend,
            circuit_id: circuit_id.to_owned(),
            vk_hash,
            public_inputs,
            proof_bytes: vec![0xAA, 0xBB, 0xCC],
            aux: Vec::new(),
        };
        ProofBox::new(
            backend.to_owned(),
            norito::encode_canonical(&envelope).expect("encode OpenVerifyEnvelope"),
        )
    }
    fn preverify_stark_ivm_execution_proof_for_circuit(
        backend: &str,
        circuit_id: &str,
        vk_hash: [u8; 32],
    ) -> ProofBox {
        let open = StarkFriOpenProofV1 {
            version: 1,
            public_inputs: vec![vec![[0x11; 32]]; 16],
            envelope_bytes: vec![0xAA, 0xBB, 0xCC],
        };
        let envelope = OpenVerifyEnvelope {
            backend: BackendTag::Stark,
            circuit_id: circuit_id.to_owned(),
            vk_hash,
            public_inputs: ivm_execution_public_inputs_schema_descriptor().to_vec(),
            proof_bytes: norito::encode_canonical(&open).expect("encode IVM STARK wrapper"),
            aux: Vec::new(),
        };
        ProofBox::new(
            backend.to_owned(),
            norito::encode_canonical(&envelope).expect("encode IVM STARK OpenVerifyEnvelope"),
        )
    }
    fn mutate_preverify_envelope(
        mut proof: ProofBox,
        mutate: impl FnOnce(&mut OpenVerifyEnvelope),
    ) -> ProofBox {
        let mut envelope: OpenVerifyEnvelope =
            norito::decode_canonical(&proof.bytes).expect("decode OpenVerifyEnvelope");
        mutate(&mut envelope);
        proof.bytes = norito::encode_canonical(&envelope).expect("encode OpenVerifyEnvelope");
        proof
    }
    #[test]
    fn proof_hash_length_prefixes_backend_and_payload() {
        let proof_a = ProofBox::new("ab".into(), b"cdef".to_vec());
        let proof_b = ProofBox::new("abc".into(), b"def".to_vec());
        assert_ne!(hash_proof(&proof_a), hash_proof(&proof_b));
    }
    #[test]
    fn verifying_key_hash_length_prefixes_backend_and_payload() {
        let vk_a = VerifyingKeyBox::new("ab".into(), b"cdef".to_vec());
        let vk_b = VerifyingKeyBox::new("abc".into(), b"def".to_vec());
        assert_ne!(hash_vk(&vk_a), hash_vk(&vk_b));
    }
    #[test]
    fn preverify_dedup_key_length_prefixes_backend_and_payload() {
        let mut dedup = DedupCache::new();
        let proof_a = ProofBox::new("ab".into(), b"cdef".to_vec());
        let proof_b = ProofBox::new("abc".into(), b"def".to_vec());
        let commitment = Some([0x42; 32]);
        assert!(dedup.check_and_insert_with_commitment(&proof_a, commitment));
        assert!(
            dedup.check_and_insert_with_commitment(&proof_b, commitment),
            "distinct backend/payload boundaries must not collide in preverify dedup"
        );
    }
    #[test]
    fn preverify_dedup_key_separates_absent_and_present_commitment() {
        let mut dedup = DedupCache::new();
        let proof = ProofBox::new("halo2/ipa".into(), b"same-proof".to_vec());
        assert!(dedup.check_and_insert_with_commitment(&proof, None));
        assert!(
            dedup.check_and_insert_with_commitment(&proof, Some([0u8; 32])),
            "missing commitment and all-zero commitment must use distinct preverify dedup keys"
        );
    }
    #[test]
    fn failed_preverify_attempts_do_not_poison_dedup_cache() {
        let vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![5, 6, 7, 8]);
        let expected = hash_vk(&vk);
        let proof = preverify_enveloped_proof(expected);
        let mut budget_dedup = DedupCache::new();
        assert_preverify!(
            proof,
            vk,
            budget_dedup,
            1,
            Some(expected),
            Some(expected),
            true,
            PreverifyBudgetExceeded
        );
        assert_preverify!(proof, vk, budget_dedup, expected, Accepted);
        let mut resolved_commitment_dedup = DedupCache::new();
        assert_preverify!(
            proof,
            vk,
            resolved_commitment_dedup,
            0,
            None,
            Some(expected),
            true,
            Accepted
        );
        assert_preverify!(proof, vk, resolved_commitment_dedup, expected, Duplicate);
        let mut missing_expected_dedup = DedupCache::new();
        assert_preverify!(
            proof,
            vk,
            missing_expected_dedup,
            0,
            None,
            None,
            true,
            VerifyingKeyMissing
        );
        assert_preverify!(
            proof,
            vk,
            missing_expected_dedup,
            0,
            Some(expected),
            None,
            true,
            VerifyingKeyMissing
        );
        assert_preverify!(proof, vk, missing_expected_dedup, expected, Accepted);
        let mut zero_commitment_dedup = DedupCache::new();
        assert_preverify!(
            proof,
            vk,
            zero_commitment_dedup,
            0,
            Some([0u8; 32]),
            Some(expected),
            true,
            VerifyingKeyMismatch
        );
        assert_preverify!(
            proof,
            vk,
            zero_commitment_dedup,
            0,
            Some(expected),
            Some([0u8; 32]),
            true,
            VerifyingKeyMismatch
        );
        assert_preverify!(proof, vk, zero_commitment_dedup, expected, Accepted);
        let mut wrong_backend_dedup = DedupCache::new();
        let wrong_backend_vk = VerifyingKeyBox::new("stark/fri".into(), vk.bytes.clone());
        let wrong_backend_expected = hash_vk(&wrong_backend_vk);
        let wrong_backend_proof = preverify_enveloped_proof(wrong_backend_expected);
        assert_preverify!(
            wrong_backend_proof,
            wrong_backend_vk,
            wrong_backend_dedup,
            wrong_backend_expected,
            VerifyingKeyMismatch
        );
        assert_preverify!(proof, vk, wrong_backend_dedup, expected, Accepted);
        let mut mismatch_dedup = DedupCache::new();
        let mut wrong = expected;
        wrong[0] ^= 0x80;
        assert_preverify!(
            proof,
            vk,
            mismatch_dedup,
            0,
            Some(wrong),
            Some(expected),
            true,
            VerifyingKeyMismatch
        );
        assert_preverify!(proof, vk, mismatch_dedup, expected, Accepted);
        let mut wrong_vk_dedup = DedupCache::new();
        let wrong_vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![8, 7, 6, 5]);
        assert_preverify!(
            proof,
            wrong_vk,
            wrong_vk_dedup,
            expected,
            VerifyingKeyMismatch
        );
        assert_preverify!(proof, vk, wrong_vk_dedup, expected, Accepted);
        let mut inactive_dedup = DedupCache::new();
        assert_preverify!(
            proof,
            vk,
            inactive_dedup,
            0,
            Some(expected),
            Some(expected),
            false,
            VerifyingKeyInactive
        );
        assert_preverify!(proof, vk, inactive_dedup, expected, Accepted);
    }
    #[test]
    fn preverify_rejects_noncanonical_envelope_metadata_before_dedup() {
        let vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![0xA5, 0x5A]);
        let expected = hash_vk(&vk);
        let proof = preverify_enveloped_proof(expected);
        let envelope: OpenVerifyEnvelope =
            norito::decode_canonical(&proof.bytes).expect("decode canonical Halo2 envelope");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_layout_proof = {
            let alternate_bytes = {
                let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
                norito::to_bytes(&envelope).expect("encode alternate-layout Halo2 envelope")
            };
            assert_ne!(alternate_bytes, proof.bytes);
            norito::decode_from_bytes::<OpenVerifyEnvelope>(&alternate_bytes)
                .expect("ordinary Norito accepts the advertised layout");
            ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), alternate_bytes)
        };
        for (case, tampered, expected_result) in [
            (
                "raw_payload",
                ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), vec![1, 2, 3, 4]),
                PreverifyResult::MalformedProof,
            ),
            (
                "alternate_layout",
                alternate_layout_proof,
                PreverifyResult::MalformedProof,
            ),
            (
                "backend_tag",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.backend = BackendTag::Stark;
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "aux",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.aux = b"side-channel".to_vec();
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "empty_circuit_id",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.circuit_id.clear();
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "invalid_circuit_id",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.circuit_id = "halo2/ipa:::preverify-test".to_owned();
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "oversized_circuit_id",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.circuit_id = "a"
                        .repeat(iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_CIRCUIT_ID_BYTES + 1);
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "empty_public_inputs",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.public_inputs.clear();
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "all_zero_public_inputs",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.public_inputs = vec![0; 4];
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "wrong_nonzero_public_input_schema",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.public_inputs = b"noncanonical-but-nonzero-schema".to_vec();
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "oversized_public_inputs",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.public_inputs = vec![
                        0xA5;
                        iroha_data_model::zk::OPEN_VERIFY_DEFAULT_MAX_PUBLIC_INPUT_BYTES
                            + 1
                    ];
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "empty_proof_bytes",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.proof_bytes.clear();
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "all_zero_proof_bytes",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.proof_bytes = vec![0; 16];
                }),
                PreverifyResult::MalformedProof,
            ),
            (
                "zero_vk_hash",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.vk_hash = [0u8; 32];
                }),
                PreverifyResult::VerifyingKeyMismatch,
            ),
            (
                "wrong_vk_hash",
                mutate_preverify_envelope(proof.clone(), |envelope| {
                    envelope.vk_hash[0] ^= 0x80;
                }),
                PreverifyResult::VerifyingKeyMismatch,
            ),
        ] {
            let mut dedup = DedupCache::new();
            assert_preverify!(
                tampered,
                vk,
                dedup,
                expected,
                expected_result,
                "case {case}"
            );
            assert_preverify!(
                proof,
                vk,
                dedup,
                expected,
                Accepted,
                "case {case} should not poison dedup cache"
            );
        }
    }
    #[test]
    fn preverify_rejects_reserved_bfv_stark_open_verify_circuit_before_dedup() {
        let backend = iroha_crypto::BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1;
        let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xA5, 0x5A, 0xC3]);
        let expected = hash_vk(&vk);
        let canonical = preverify_enveloped_proof_for_backend(
            backend,
            BackendTag::Stark,
            iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1,
            expected,
        );
        let prefixed_circuit_id = format!(
            "{}:{}",
            backend,
            iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1
        );
        let prefixed = preverify_enveloped_proof_for_backend(
            backend,
            BackendTag::Stark,
            &prefixed_circuit_id,
            expected,
        );
        let slash_circuit_id = format!(
            "{}/{}",
            backend,
            iroha_crypto::BFV_FULL_BOOTSTRAP_CIRCUIT_ID_V1
        );
        let slash = preverify_enveloped_proof_for_backend(
            backend,
            BackendTag::Stark,
            &slash_circuit_id,
            expected,
        );
        let accepted = preverify_enveloped_proof_for_backend(
            backend,
            BackendTag::Stark,
            &format!("{backend}:preverify-test"),
            expected,
        );
        for (case, proof) in [
            ("canonical BFV circuit id", canonical),
            ("backend-prefixed BFV circuit id", prefixed),
            ("slash-form BFV circuit id", slash),
        ] {
            let mut dedup = DedupCache::new();
            assert_preverify!(proof, vk, dedup, expected, MalformedProof, "case {case}");
            assert_preverify!(
                accepted,
                vk,
                dedup,
                expected,
                Accepted,
                "case {case} must not poison dedup"
            );
        }
    }
    #[test]
    fn preverify_rejects_every_generic_soracloud_fhe_relation_alias_before_dedup() {
        use iroha_data_model::soracloud::{
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
            SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
        };
        let backend = "stark/fri/sha256-goldilocks";
        let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0x3C, 0xA5, 0x5A]);
        let expected = hash_vk(&vk);
        let accepted = preverify_enveloped_proof_for_backend(
            backend,
            BackendTag::Stark,
            &format!("{backend}:soracloud-near-miss"),
            expected,
        );
        for canonical in [
            SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1,
            SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1,
            SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1,
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1,
        ] {
            for circuit_id in [
                canonical.to_owned(),
                format!("{backend}:{canonical}"),
                format!("{backend}/{canonical}"),
            ] {
                let proof = preverify_enveloped_proof_for_backend(
                    backend,
                    BackendTag::Stark,
                    &circuit_id,
                    expected,
                );
                let mut dedup = DedupCache::new();
                assert_preverify!(
                    proof,
                    vk,
                    dedup,
                    expected,
                    MalformedProof,
                    "generic Soracloud relation alias {circuit_id}"
                );
                assert_preverify!(
                    accepted,
                    vk,
                    dedup,
                    expected,
                    Accepted,
                    "rejected alias {circuit_id} must not poison dedup"
                );
            }
        }
    }
    #[test]
    fn preverify_rejects_every_retired_generic_zk_ace_alias_before_dedup() {
        let backend = iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_BACKEND;
        let retired = iroha_data_model::zk::ZK_ACE_PQ_AUTHORIZATION_V0_CIRCUIT_ID;
        let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0x5A, 0xC3, 0xA5]);
        let expected = hash_vk(&vk);
        let accepted = preverify_enveloped_proof_for_backend(
            backend,
            BackendTag::Stark,
            &format!("{backend}:zk_ace_near_miss"),
            expected,
        );
        for (case, circuit_id) in [
            ("bare retired relation", retired.to_owned()),
            (
                "backend-prefixed retired relation",
                format!("{backend}:{retired}"),
            ),
            (
                "slash-prefixed retired relation",
                format!("{backend}/{retired}"),
            ),
        ] {
            let proof = preverify_enveloped_proof_for_backend(
                backend,
                BackendTag::Stark,
                &circuit_id,
                expected,
            );
            let mut dedup = DedupCache::new();
            assert_preverify!(proof, vk, dedup, expected, MalformedProof, "case {case}");
            assert_preverify!(
                accepted,
                vk,
                dedup,
                expected,
                Accepted,
                "case {case} must not poison dedup"
            );
        }
    }
    #[test]
    fn preverify_rejects_malformed_ivm_stark_open_verify_shape_before_dedup() {
        let backend = "stark/fri/sha256-goldilocks";
        let canonical = IVM_EXECUTION_V1_CIRCUIT_ID;
        let prefixed_circuit_id = format!("{backend}:{canonical}");
        let slash_circuit_id = format!("{backend}/{canonical}");
        let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xC3, 0xA5, 0x5A]);
        let expected = hash_vk(&vk);
        let accepted = preverify_stark_ivm_execution_proof_for_circuit(
            backend,
            &prefixed_circuit_id,
            expected,
        );
        for (case, proof) in [
            (
                "canonical IVM circuit id with generic schema",
                preverify_enveloped_proof_for_backend(
                    backend,
                    BackendTag::Stark,
                    canonical,
                    expected,
                ),
            ),
            (
                "backend-prefixed IVM circuit id with generic schema",
                preverify_enveloped_proof_for_backend(
                    backend,
                    BackendTag::Stark,
                    &prefixed_circuit_id,
                    expected,
                ),
            ),
            (
                "slash-form IVM circuit id with generic schema",
                preverify_enveloped_proof_for_backend(
                    backend,
                    BackendTag::Stark,
                    &slash_circuit_id,
                    expected,
                ),
            ),
            (
                "IVM circuit id with malformed wrapper",
                mutate_preverify_envelope(accepted.clone(), |envelope| {
                    envelope.proof_bytes = vec![0xAA, 0xBB, 0xCC];
                }),
            ),
            (
                "IVM circuit id with wrong wrapper version",
                mutate_preverify_envelope(accepted.clone(), |envelope| {
                    let open = StarkFriOpenProofV1 {
                        version: 2,
                        public_inputs: vec![vec![[0x11; 32]]; 16],
                        envelope_bytes: vec![0xAA, 0xBB, 0xCC],
                    };
                    envelope.proof_bytes =
                        norito::to_bytes(&open).expect("encode wrong-version wrapper");
                }),
            ),
            (
                "IVM circuit id with empty inner envelope",
                mutate_preverify_envelope(accepted.clone(), |envelope| {
                    let open = StarkFriOpenProofV1 {
                        version: 1,
                        public_inputs: vec![vec![[0x11; 32]]; 16],
                        envelope_bytes: Vec::new(),
                    };
                    envelope.proof_bytes =
                        norito::to_bytes(&open).expect("encode empty-inner wrapper");
                }),
            ),
            (
                "IVM circuit id with short public input columns",
                mutate_preverify_envelope(accepted.clone(), |envelope| {
                    let open = StarkFriOpenProofV1 {
                        version: 1,
                        public_inputs: vec![vec![[0x11; 32]]; 15],
                        envelope_bytes: vec![0xAA, 0xBB, 0xCC],
                    };
                    envelope.proof_bytes =
                        norito::to_bytes(&open).expect("encode short-column wrapper");
                }),
            ),
            (
                "IVM circuit id with multi-row public input column",
                mutate_preverify_envelope(accepted.clone(), |envelope| {
                    let mut public_inputs = vec![vec![[0x11; 32]]; 16];
                    public_inputs[0].push([0x22; 32]);
                    let open = StarkFriOpenProofV1 {
                        version: 1,
                        public_inputs,
                        envelope_bytes: vec![0xAA, 0xBB, 0xCC],
                    };
                    envelope.proof_bytes =
                        norito::to_bytes(&open).expect("encode multi-row wrapper");
                }),
            ),
        ] {
            let mut dedup = DedupCache::new();
            assert_preverify!(proof, vk, dedup, expected, MalformedProof, "case {case}");
            assert_preverify!(
                accepted,
                vk,
                dedup,
                expected,
                Accepted,
                "case {case} must not poison dedup"
            );
        }
    }
    #[test]
    fn preverify_rejects_halo2_open_verify_circuit_mismatch_before_dedup() {
        for (case, backend, accepted_circuit_id, mismatched_circuit_id) in [
            (
                "concrete backend with sibling circuit",
                "halo2/pasta/ivm-execution-v1",
                "halo2/pasta/ivm-execution-v1",
                "halo2/pasta/tiny-add-public",
            ),
            (
                "generic halo2 backend with cross-family circuit",
                ZK_BACKEND_HALO2_IPA,
                IVM_EXECUTION_V1_CIRCUIT_ID,
                "stark/fri/sha256-goldilocks:spoof",
            ),
            (
                "generic halo2 backend with bare trusted-setup circuit",
                ZK_BACKEND_HALO2_IPA,
                IVM_EXECUTION_V1_CIRCUIT_ID,
                "kzg",
            ),
            (
                "generic halo2 backend with prefixed trusted-setup circuit",
                ZK_BACKEND_HALO2_IPA,
                IVM_EXECUTION_V1_CIRCUIT_ID,
                "halo2/ipa:kzg",
            ),
            (
                "generic halo2 backend with prefixed STARK circuit",
                ZK_BACKEND_HALO2_IPA,
                IVM_EXECUTION_V1_CIRCUIT_ID,
                "halo2/ipa:stark/fri",
            ),
        ] {
            let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xA5, 0x5A, 0xC3]);
            let expected = hash_vk(&vk);
            let accepted = preverify_enveloped_proof_for_backend(
                backend,
                BackendTag::Halo2IpaPasta,
                accepted_circuit_id,
                expected,
            );
            let mismatched = preverify_enveloped_proof_for_backend(
                backend,
                BackendTag::Halo2IpaPasta,
                mismatched_circuit_id,
                expected,
            );
            let mut dedup = DedupCache::new();
            assert_preverify!(
                mismatched,
                vk,
                dedup,
                expected,
                MalformedProof,
                "case {case}"
            );
            assert_preverify!(
                accepted,
                vk,
                dedup,
                expected,
                Accepted,
                "case {case} must not poison dedup"
            );
        }
    }
    #[test]
    fn preverify_rejects_stark_open_verify_circuit_mismatch_before_dedup() {
        for (case, backend, accepted_circuit_id, mismatched_circuit_id) in [
            (
                "profile backend with sibling STARK profile",
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:preverify-test",
                "stark/fri/poseidon2-goldilocks:preverify-test",
            ),
            (
                "profile backend with generic STARK prefix",
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:preverify-test",
                "stark/fri:preverify-test",
            ),
            (
                "profile backend with bare generic STARK family",
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:preverify-test",
                "stark/fri",
            ),
            (
                "generic STARK backend with halo2 circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "stark/fri:preverify-test",
                "halo2/ipa:preverify-test",
            ),
            (
                "generic STARK backend with colon-form halo2 circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "stark/fri:preverify-test",
                "halo2:preverify-test",
            ),
            (
                "generic STARK backend with colon-form kzg circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "stark/fri:preverify-test",
                "kzg:trusted-setup-spoof",
            ),
            (
                "generic STARK backend with bare trusted-setup curve circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "stark/fri:preverify-test",
                "bn254",
            ),
            (
                "generic STARK backend with STARK-prefixed trusted-setup circuit",
                ZK_BACKEND_STARK_FRI_V1,
                "stark/fri:preverify-test",
                "stark/fri:universal-srs",
            ),
            (
                "profile backend with profile-prefixed trusted-setup circuit",
                "stark/fri/sha256-goldilocks",
                "stark/fri/sha256-goldilocks:preverify-test",
                "stark/fri/sha256-goldilocks:structured-reference-string",
            ),
        ] {
            let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xA5, 0x5A, 0xC3]);
            let expected = hash_vk(&vk);
            let accepted = preverify_enveloped_proof_for_backend(
                backend,
                BackendTag::Stark,
                accepted_circuit_id,
                expected,
            );
            let mismatched = preverify_enveloped_proof_for_backend(
                backend,
                BackendTag::Stark,
                mismatched_circuit_id,
                expected,
            );
            let mut dedup = DedupCache::new();
            assert_preverify!(
                mismatched,
                vk,
                dedup,
                expected,
                MalformedProof,
                "case {case}"
            );
            assert_preverify!(
                accepted,
                vk,
                dedup,
                expected,
                Accepted,
                "case {case} must not poison dedup"
            );
        }
    }
    #[test]
    fn preverify_binds_open_verify_metadata_for_all_production_labels() {
        for (backend, envelope_backend, circuit_id) in [
            (
                ZK_BACKEND_HALO2_IPA,
                BackendTag::Halo2IpaPasta,
                IVM_EXECUTION_V1_CIRCUIT_ID,
            ),
            (
                "halo2/pasta/ivm-execution-v1",
                BackendTag::Halo2IpaPasta,
                "halo2/pasta/ivm-execution-v1",
            ),
            (
                ZK_BACKEND_STARK_FRI_V1,
                BackendTag::Stark,
                "stark/fri:preverify-test",
            ),
            (
                "stark/fri/sha256-goldilocks",
                BackendTag::Stark,
                "stark/fri/sha256-goldilocks:preverify-test",
            ),
            (
                "stark/fri/poseidon2-goldilocks",
                BackendTag::Stark,
                "stark/fri/poseidon2-goldilocks:preverify-test",
            ),
            (
                "stark/fri/sha256_goldilocks.v1",
                BackendTag::Stark,
                "stark/fri/sha256_goldilocks.v1:preverify-test",
            ),
        ] {
            let vk = VerifyingKeyBox::new(backend.to_owned(), vec![0xA5, 0x5A, 0xC3]);
            let expected = hash_vk(&vk);
            let proof = preverify_enveloped_proof_for_backend(
                backend,
                envelope_backend,
                circuit_id,
                expected,
            );
            let mut dedup = DedupCache::new();
            assert_preverify!(
                proof,
                vk,
                dedup,
                expected,
                Accepted,
                "registry backend {backend} should preverify with a matching envelope"
            );
            let mut raw_dedup = DedupCache::new();
            let raw = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
            assert_preverify!(
                raw,
                vk,
                raw_dedup,
                expected,
                MalformedProof,
                "registry backend {backend} must require OpenVerifyEnvelope metadata"
            );
            let wrong_envelope_backend = match envelope_backend {
                BackendTag::Halo2IpaPasta => BackendTag::Stark,
                BackendTag::Stark => BackendTag::Halo2IpaPasta,
            };
            let wrong_backend_proof = mutate_preverify_envelope(proof.clone(), |envelope| {
                envelope.backend = wrong_envelope_backend;
            });
            let mut wrong_backend_dedup = DedupCache::new();
            assert_preverify!(
                wrong_backend_proof,
                vk,
                wrong_backend_dedup,
                expected,
                MalformedProof,
                "registry backend {backend} must reject mismatched envelope backend tags"
            );
            assert_preverify!(
                proof,
                vk,
                wrong_backend_dedup,
                expected,
                Accepted,
                "mismatched envelope backend for {backend} must not poison dedup"
            );
        }
    }
    #[test]
    fn preverify_rejects_trusted_setup_backends_before_dedup() {
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
            "halo2/bn254",
            "halo2/bn254/vote",
            "halo2/kzg",
            "halo2/ipa:kzg",
            "halo2/ipa:KZG",
            "halo2/ipa: KZG",
            "stark/fri/prod;kzg",
            "stark/fri/prod,kzg",
            "stark/fri/prod+kzg",
            "stark/fri/prod.kzg",
            "stark/fri/prod(kzg)",
            "stark/fri/prod;bn254",
            "stark/fri/prod+bn256",
            "stark/fri/prod-bls12-381",
            "halo2/ipa;groth16",
            "halo2/ipa/orchard:kzg",
            "orchard:universal-srs",
            "penumbra-masp:kzg",
            "jindo-lattice-pcs-zk:trusted-setup",
            "miden-stark:ptau",
            "sis-with-hints:groth16",
            "pq-masp-stark-fri:kzg",
            "groth16/bn254",
        ] {
            let mut dedup = DedupCache::new();
            let proof = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
            assert_eq!(
                preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
                PreverifyResult::UnsupportedBackend,
                "case {backend}"
            );
            assert_eq!(
                preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
                PreverifyResult::UnsupportedBackend,
                "case {backend} should not poison dedup cache"
            );
        }
    }
    #[test]
    fn preverify_rejects_developer_only_backends_before_dedup() {
        for backend in [
            "debug",
            "debug-proof",
            "Debug-Proof",
            "debug/ok",
            "halo2/debug",
            "halo2/ipa:debug-proof",
            "halo2/ipa:DEBUG-Proof",
            "halo2/ipa:d-e-b-u-g-proof",
            "stark/fri/debug",
            "stark/fri/Debug",
            "stark/fri/d-e-b-u-g",
            "mock",
            "mock-proof",
            "Mock-Proof",
            "halo2/mock",
            "halo2/ipa:mock-proof",
            "halo2/ipa:Mock-Proof",
            "halo2/ipa:m-o-c-k-proof",
            "stark/fri/m-o-c-k",
            "stark/fri/dev-fixture",
            "stark/fri/d-e-v-f-i-x-t-u-r-e",
            "stark/fri/dev",
            "stark/fri/d-e-v",
            "stark/fri/test",
            "stark/fri/t-e-s-t",
            "stark/fri/placeholder",
            "halo2/ipa:dev-fixture",
            "halo2/ipa:d-e-v-f-i-x-t-u-r-e",
            "halo2/ipa:dev",
            "halo2/ipa:d-e-v",
            "halo2/ipa:dummy",
            "halo2/ipa:f-a-k-e",
            "halo2/ipa:stub",
            "halo2/ipa:s-a-m-p-l-e",
            "zk-trace/mock-proof",
        ] {
            let mut dedup = DedupCache::new();
            let proof = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
            assert_eq!(
                preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
                PreverifyResult::UnsupportedBackend,
                "case {backend}"
            );
            assert_eq!(
                preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
                PreverifyResult::UnsupportedBackend,
                "case {backend} should not poison dedup cache"
            );
        }
    }
    #[test]
    fn preverify_rejects_production_claim_backends_before_dedup() {
        for backend in [
            "halo2/ipa:production-ready",
            "halo2/ipa:claimed-production",
            "halo2/ipa:mainnet-ready",
            "halo2/ipa:mainnet-complete",
            "stark/fri/audit-signoff",
            "stark/fri/externally-audited",
            "stark/fri/security-review-passed",
            "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
            "stark/fri/a-u-d-i-t-c-l-a-i-m",
            "halo2/ipa:release-ready",
            "halo2/ipa:release-approved",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa:third-party-audited",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
        ] {
            let mut dedup = DedupCache::new();
            let proof = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
            assert_eq!(
                preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
                PreverifyResult::UnsupportedBackend,
                "case {backend}"
            );
            assert_eq!(
                preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
                PreverifyResult::UnsupportedBackend,
                "case {backend} should not poison dedup cache"
            );
        }
    }
    #[test]
    fn preverify_rejects_unknown_and_protocol_names_before_dedup() {
        for backend in [
            "not-a-production-backend",
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "halo2/ipa\0",
            "halo2\u{FF0F}ipa",
            "halo2/\u{200B}ipa",
            "h\u{0430}lo2/ipa",
            "../halo2/ipa",
            "halo2/ipa/../tiny-add",
            "halo2/ipa:ivm-execution-v1 ",
            " stark/fri/sha256-goldilocks",
            "stark/fri/sha256-goldilocks ",
            "stark/fri/sha256-goldilocks\0",
            "stark\u{FF0F}fri/sha256-goldilocks",
            "stark/fri/\u{200B}sha256-goldilocks",
            "st\u{0430}rk/fri/sha256-goldilocks",
            "../stark/fri",
            "stark/fri/../sha256-goldilocks",
            "stark/fri/random-profile",
            "stark/fri/sha512-goldilocks",
            "stark/fri/audit-proof-v1",
            "halo2-ipa-orchard",
            "halo2/ipa/orchard",
            "orchard",
            "groth16-bls12-377",
            "groth16/bls12-377",
            "penumbra-masp",
            "monero-fcmp++",
            "fcmp++",
            "fcmp-plus-plus-curve-tree",
            "lattice-pcs-sis",
            "sis-hints-anoncred-pq-v0",
            "sis-with-hints",
            "miden-stark",
            "stark/fri/miden",
            "aztec-plonkish-private-kernel",
            "pq-masp-stark-fri",
            "stark/fri/pq-masp-stark-fri",
            "post-quantum-masp",
        ] {
            let mut dedup = DedupCache::new();
            let proof = ProofBox::new(backend.to_owned(), vec![1, 2, 3, 4]);
            assert_eq!(
                preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
                PreverifyResult::UnsupportedBackend,
                "case {backend}"
            );
            assert_eq!(
                preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
                PreverifyResult::UnsupportedBackend,
                "case {backend} should not poison dedup cache"
            );
        }
    }
    #[test]
    fn unsupported_backend_preverify_attempts_do_not_poison_dedup_cache() {
        let mut dedup = DedupCache::new();
        let proof = ProofBox::new(String::new(), vec![1, 2, 3, 4]);
        assert_eq!(
            preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
            PreverifyResult::UnsupportedBackend
        );
        assert_eq!(
            preverify_with_budget(&proof, None, &mut dedup, 0, None, None, true),
            PreverifyResult::UnsupportedBackend,
            "unsupported proofs should keep failing as unsupported, not become dedup duplicates"
        );
    }
}
#[cfg(all(test, feature = "zk-tests", feature = "halo2-dev-tests"))]
include!("zk/halo2_backend_tests.rs");
