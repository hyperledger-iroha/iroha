#![allow(clippy::many_single_char_names)]
//! Minimal ZK verification utilities and batch de-duplication scaffolding.
//!
//! This module provides:
//! - Stable proof/verifying-key hash helpers (`hash_proof`, `hash_vk`).
//! - Batch-local de-duplication cache (`DedupCache`) and a light pre-verifier.
//! - Backend dispatch for Halo2 (Pasta/KZG, IPA) with tiny-circuit smoke tests.
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
    io,
    time::{Duration, Instant},
};

/// Confidential transfer v2 helpers, circuits, and proof builders.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub mod confidential_v2;

#[cfg(feature = "zk-preverify")]
use iroha_crypto::streaming::TransportCapabilityResolutionSnapshot;
use iroha_data_model::proof::{ProofBox, VerifyingKeyBox, VerifyingKeyId};
#[cfg(feature = "zk-preverify")]
use ivm::halo2::VMExecutionCircuit;
#[cfg(feature = "zk-halo2")]
use kaigi_zk::{
    KAIGI_ROSTER_BACKEND, KAIGI_USAGE_BACKEND, KaigiRosterJoinCircuit, KaigiUsageCommitmentCircuit,
};
#[cfg(feature = "zk-preverify")]
use norito::streaming::CapabilityFlags;
use sha2::{Digest, Sha256};
#[cfg(feature = "zk-preverify")]
use tokio::sync::mpsc;

#[cfg(feature = "zk-preverify")]
use crate::kura::PipelineProofSnapshot;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
type PastaParams =
    halo2_proofs::poly::ipa::commitment::ParamsIPA<halo2_proofs::halo2curves::pasta::EqAffine>;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn pasta_params_new(k: u32) -> PastaParams {
    use halo2_proofs::poly::commitment::ParamsProver as _;

    halo2_proofs::poly::ipa::commitment::ParamsIPA::<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >::new(k)
}

#[cfg(all(
    test,
    feature = "zk-halo2-ipa-poseidon",
    any(feature = "zk-halo2", feature = "zk-halo2-ipa")
))]
use halo2_proofs::poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA};
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
use halo2_proofs::{
    SerdeFormat, poly::VerificationStrategy, poly::commitment::Params as _,
    transcript::TranscriptReadBuffer,
};

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
pub(crate) fn assign_advice_compat<'r, F, A, AR, V, T>(
    region: &mut halo2_proofs::circuit::Region<'r, F>,
    annotation: A,
    column: halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
    offset: usize,
    mut to: V,
) -> Result<
    halo2_proofs::circuit::AssignedCell<&'r halo2_proofs::plonk::Assigned<F>, F>,
    halo2_proofs::plonk::Error,
>
where
    F: halo2_proofs::halo2curves::ff::Field,
    A: Fn() -> AR,
    AR: Into<String>,
    V: FnMut() -> halo2_proofs::circuit::Value<T>,
    T: Into<halo2_proofs::plonk::Assigned<F>>,
{
    let _ = annotation;
    let value = to().map(Into::into);
    Ok(halo2_proofs::circuit::Region::assign_advice(
        region, column, offset, value,
    ))
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn read_verifying_key<C, R>(
    reader: &mut R,
) -> io::Result<halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>>
where
    R: io::Read,
    C: halo2_proofs::plonk::Circuit<halo2_proofs::halo2curves::pasta::Fp>,
    C::Params: Default,
{
    #[cfg(feature = "circuit-params")]
    {
        halo2_proofs::plonk::VerifyingKey::<halo2_proofs::halo2curves::pasta::EqAffine>::read::<_, C>(
            reader,
            SerdeFormat::Processed,
            C::Params::default(),
        )
    }
    #[cfg(not(feature = "circuit-params"))]
    {
        halo2_proofs::plonk::VerifyingKey::<halo2_proofs::halo2curves::pasta::EqAffine>::read::<_, C>(
            reader,
            SerdeFormat::Processed,
        )
    }
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn read_proving_key<C, R>(
    reader: &mut R,
) -> io::Result<halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>>
where
    R: io::Read,
    C: halo2_proofs::plonk::Circuit<halo2_proofs::halo2curves::pasta::Fp>,
    C::Params: Default,
{
    #[cfg(feature = "circuit-params")]
    {
        halo2_proofs::plonk::ProvingKey::<halo2_proofs::halo2curves::pasta::EqAffine>::read::<_, C>(
            reader,
            SerdeFormat::Processed,
            C::Params::default(),
        )
    }
    #[cfg(not(feature = "circuit-params"))]
    {
        halo2_proofs::plonk::ProvingKey::<halo2_proofs::halo2curves::pasta::EqAffine>::read::<_, C>(
            reader,
            SerdeFormat::Processed,
        )
    }
}

/// Hard caps for TLV sections to preserve bounded parsing and determinism.
/// These are generous relative to current tests and examples.
const MAX_PROOF_LEN: usize = 8 * 1024 * 1024; // 8 MiB

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
const MAX_INST_COLS: usize = 16;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
const MAX_INST_ROWS: usize = 8192;

/// Canonical backend identifier for Halo2 IPA verification.
pub const ZK_BACKEND_HALO2_IPA: &str = "halo2/ipa";
/// Canonical backend family identifier for native STARK/FRI verification.
pub const ZK_BACKEND_STARK_FRI_V1: &str = "stark/fri";
/// Canonical circuit identifier suffix for proved IVM execution commitments.
pub const IVM_EXECUTION_V1_CIRCUIT_ID: &str = "ivm-execution-v1";
/// Halo2 IPA parameter degree used by the canonical IVM execution binding circuit.
pub const IVM_EXECUTION_V1_IPA_K: u32 = 7;
/// Maximum encoded proof payload accepted for IVM execution proofs.
pub const IVM_EXECUTION_V1_MAX_PROOF_BYTES: u32 = 8 * 1024 * 1024;

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
/// The returned key is a real Halo2 IPA verifier key envelope (`IPAK` + `H2VK`)
/// for the current IVM execution binding circuit, suitable for WSV registration.
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
fn build_halo2_ipa_ivm_execution_vk_box() -> Result<VerifyingKeyBox, halo2_proofs::plonk::Error> {
    use halo2_proofs::plonk::keygen_vk;

    let params = pasta_params_new(IVM_EXECUTION_V1_IPA_K);
    let circuit = pasta_tiny::IvmExecutionBindV1::default();
    let vk = keygen_vk(&params, &circuit)?;
    let mut bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut bytes, IVM_EXECUTION_V1_IPA_K);
    zk1::wrap_append_vk_pasta(&mut bytes, &vk);
    Ok(VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), bytes))
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

/// Canonical circuit identifier for Offline recursive note proofs.
pub const OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID: &str = "offline-note-recursive";
/// Halo2 IPA parameter degree used by the canonical Offline recursive note circuit.
pub const OFFLINE_NOTE_RECURSIVE_IPA_K: u32 = 7;
/// Maximum encoded proof payload accepted for Offline recursive note proofs.
pub const OFFLINE_NOTE_MAX_PROOF_BYTES: u32 = 8 * 1024 * 1024;

const OFFLINE_NOTE_MODE_REDEEM: u64 = 1;
const OFFLINE_NOTE_MODE_AUDIT: u64 = 2;
/// Number of public instance columns exposed by Offline recursive note proofs.
pub const OFFLINE_NOTE_INSTANCE_COLUMNS: usize = 16;
/// Maximum number of input amount witness slots supported by Offline proofs.
pub const OFFLINE_NOTE_MAX_INPUT_AMOUNTS: usize = 4;
/// Maximum number of output amount witness slots supported by Offline proofs.
pub const OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS: usize = 2;

/// Public and private witness values for the canonical Offline semantic circuit.
///
/// The first sixteen values are public instance columns. The private amount slots
/// let the circuit enforce bounded input/output counts and amount conservation
/// without exposing each individual note amount as a public instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineNoteInstanceValues {
    /// Public instance values encoded as single-row Pasta field columns.
    pub public_values: [u64; OFFLINE_NOTE_INSTANCE_COLUMNS],
    /// Private input amount slots, normalized to a common decimal scale.
    pub input_amounts: [u64; OFFLINE_NOTE_MAX_INPUT_AMOUNTS],
    /// Private output amount slots, normalized to the same decimal scale.
    pub output_amounts: [u64; OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS],
}

impl OfflineNoteInstanceValues {
    /// Return public instances in the byte layout carried by Halo2/STARK proof envelopes.
    #[must_use]
    pub fn public_instance_columns(&self) -> Vec<Vec<[u8; 32]>> {
        self.public_values
            .iter()
            .copied()
            .map(limb_as_instance_bytes)
            .map(|value| vec![value])
            .collect()
    }

    #[cfg(feature = "zk-halo2-ipa")]
    fn public_scalars(
        &self,
    ) -> [halo2_proofs::halo2curves::pasta::Fp; OFFLINE_NOTE_INSTANCE_COLUMNS] {
        self.public_values
            .map(halo2_proofs::halo2curves::pasta::Fp::from)
    }

    #[cfg(feature = "zk-halo2-ipa")]
    fn input_amount_scalars(
        &self,
    ) -> [halo2_proofs::halo2curves::pasta::Fp; OFFLINE_NOTE_MAX_INPUT_AMOUNTS] {
        self.input_amounts
            .map(halo2_proofs::halo2curves::pasta::Fp::from)
    }

    #[cfg(feature = "zk-halo2-ipa")]
    fn output_amount_scalars(
        &self,
    ) -> [halo2_proofs::halo2curves::pasta::Fp; OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS] {
        self.output_amounts
            .map(halo2_proofs::halo2curves::pasta::Fp::from)
    }
}

#[cfg(feature = "zk-halo2-ipa")]
fn is_offline_note_recursive_circuit_id(circuit_id: &str) -> bool {
    matches!(
        circuit_id,
        OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID
            | "halo2/ipa:offline-note-recursive"
            | "halo2/ipa/offline-note-recursive"
            | "halo2/pasta/offline-note-recursive"
            | "halo2/pasta/ipa/offline-note-recursive"
    )
}

/// Build the canonical inline verifier key for Offline recursive note proofs.
///
/// The returned key is a real Halo2 IPA verifier key envelope (`IPAK` + `H2VK`)
/// for `offline-note-recursive`; it is suitable for WSV registration.
///
/// # Errors
///
/// Returns an error if Halo2 verifier-key generation fails.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn offline_note_recursive_vk_box() -> Result<VerifyingKeyBox, String> {
    static CACHE: std::sync::OnceLock<Result<VerifyingKeyBox, String>> = std::sync::OnceLock::new();

    CACHE
        .get_or_init(|| {
            build_offline_note_recursive_vk_box().map_err(|err| {
                format!("failed to generate offline-note-recursive verifying key: {err}")
            })
        })
        .clone()
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn build_offline_note_recursive_vk_box() -> Result<VerifyingKeyBox, halo2_proofs::plonk::Error> {
    use halo2_proofs::plonk::keygen_vk;

    let params = pasta_params_new(OFFLINE_NOTE_RECURSIVE_IPA_K);
    let circuit = pasta_tiny::OfflineNoteSemantic::default();
    let vk = keygen_vk(&params, &circuit)?;
    let mut bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut bytes, OFFLINE_NOTE_RECURSIVE_IPA_K);
    zk1::wrap_append_vk_pasta(&mut bytes, &vk);
    Ok(VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), bytes))
}

/// Build a governance/WSV verifier-key record for Offline recursive note proofs.
///
/// The record is active, embeds the real Halo2 IPA verifier key inline, and
/// binds to the canonical Offline public-input schema hash.
///
/// # Errors
///
/// Returns an error if verifier-key generation fails or the key length cannot be encoded.
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
pub fn offline_note_recursive_vk_record(
    namespace: impl Into<String>,
    version: u32,
) -> Result<iroha_data_model::proof::VerifyingKeyRecord, String> {
    use iroha_data_model::{
        confidential::ConfidentialStatus,
        offline::offline_note_recursive_public_inputs_schema_hash, zk::BackendTag,
    };

    let vk_box = offline_note_recursive_vk_box()?;
    let mut record = iroha_data_model::proof::VerifyingKeyRecord::new(
        version,
        OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
        BackendTag::Halo2IpaPasta,
        "pallas",
        offline_note_recursive_public_inputs_schema_hash(),
        hash_vk(&vk_box),
    );
    record.vk_len = u32::try_from(vk_box.bytes.len())
        .map_err(|_| "offline verifying key length overflowed u32".to_owned())?;
    record.max_proof_bytes = OFFLINE_NOTE_MAX_PROOF_BYTES;
    record.gas_schedule_id = Some("halo2_default".to_owned());
    record.key = Some(vk_box);
    record.status = ConfidentialStatus::Active;
    record.namespace = namespace.into();
    Ok(record)
}

fn hash_limb0(hash: &iroha_crypto::Hash) -> u64 {
    hash_to_u64_limbs_le(hash)[0]
}

fn hash_limb0_sum(hashes: &[iroha_crypto::Hash]) -> u64 {
    hashes
        .iter()
        .fold(0u64, |sum, hash| sum.wrapping_add(hash_limb0(hash)))
}

fn validate_offline_note_count(count: usize, max: usize, label: &str) -> Result<u64, String> {
    if count == 0 || count > max {
        return Err(format!("offline {label} count must be in 1..={max}"));
    }
    Ok(u64::try_from(count).expect("bounded offline count fits into u64"))
}

fn trimmed_numeric_scale(value: &iroha_primitives::Numeric) -> u32 {
    value.clone().trim_trailing_zeros().scale()
}

fn normalized_numeric_to_u64(value: &iroha_primitives::Numeric, target_scale: u32) -> Option<u64> {
    let value = value.clone().trim_trailing_zeros();
    if value.mantissa().is_negative() || value.scale() > target_scale {
        return None;
    }

    let scale_delta = target_scale - value.scale();
    let factor = iroha_primitives::BigInt::pow10(scale_delta)?;
    let scaled = value.mantissa().checked_mul(&factor).ok()?;
    scaled.to_string().parse::<u64>().ok()
}

fn normalized_amount_vec(amounts: &[&iroha_primitives::Numeric]) -> Result<Vec<u64>, String> {
    let target_scale = amounts
        .iter()
        .copied()
        .map(trimmed_numeric_scale)
        .max()
        .unwrap_or(0);
    amounts
        .iter()
        .copied()
        .map(|amount| {
            normalized_numeric_to_u64(amount, target_scale).ok_or_else(|| {
                "offline proof amount does not fit into u64 witness units".to_owned()
            })
        })
        .collect()
}

fn checked_u64_sum(values: &[u64], label: &str) -> Result<u64, String> {
    values.iter().try_fold(0u64, |sum, value| {
        sum.checked_add(*value)
            .ok_or_else(|| format!("offline {label} amount sum overflows u64 witness units"))
    })
}

fn offline_note_public_values(
    public_inputs_hash: &iroha_crypto::Hash,
    mode: u64,
    input_count: u64,
    output_count: u64,
    input_sum: u64,
    output_sum: u64,
    input_nullifier_sum: u64,
    output_commitment_sum: u64,
    key_certificate_payload_hash: &iroha_crypto::Hash,
    source_or_token: &iroha_crypto::Hash,
    input_claim_hash_sum: u64,
    output_claim_hash_sum: u64,
) -> [u64; OFFLINE_NOTE_INSTANCE_COLUMNS] {
    let hash_limbs = hash_to_u64_limbs_le(public_inputs_hash);
    [
        hash_limbs[0],
        hash_limbs[1],
        hash_limbs[2],
        hash_limbs[3],
        mode,
        input_count,
        output_count,
        input_sum,
        output_sum,
        input_nullifier_sum,
        output_commitment_sum,
        hash_limb0(key_certificate_payload_hash),
        hash_limb0(source_or_token),
        input_claim_hash_sum,
        output_claim_hash_sum,
        0,
    ]
}

/// Build the public/private instance values expected for an Offline redemption proof.
///
/// # Errors
///
/// Returns an error if the redemption shape is outside the circuit corridor, if
/// Norito hashing fails, or if normalized amounts do not fit in `u64` witness units.
pub fn offline_note_redeem_instance_values(
    redemption: &iroha_data_model::offline::OfflineNoteRedeem,
) -> Result<OfflineNoteInstanceValues, String> {
    use iroha_data_model::offline::OfflineNoteIssuedClaim;

    let input_count = validate_offline_note_count(
        redemption.input_nullifiers.len(),
        OFFLINE_NOTE_MAX_INPUT_AMOUNTS,
        "redemption input",
    )?;
    let output_count = 1;
    let public_inputs_hash = redemption
        .public_inputs_hash()
        .map_err(|err| format!("failed to encode Offline redemption public inputs: {err}"))?;
    let key_certificate_payload_hash = redemption
        .sender_key_certificate
        .payload_hash()
        .map_err(|err| format!("failed to encode Offline key certificate payload: {err}"))?;
    let issued_claim_hash = OfflineNoteIssuedClaim::from_redemption(redemption)
        .and_then(|claim| claim.claim_hash())
        .map_err(|err| format!("failed to encode Offline redemption issued claim: {err}"))?;

    let normalized_amounts = normalized_amount_vec(&[&redemption.amount, &redemption.amount])?;
    let input_sum = normalized_amounts[0];
    let output_sum = normalized_amounts[1];
    let public_values = offline_note_public_values(
        &public_inputs_hash,
        OFFLINE_NOTE_MODE_REDEEM,
        input_count,
        output_count,
        input_sum,
        output_sum,
        hash_limb0_sum(&redemption.input_nullifiers),
        0,
        &key_certificate_payload_hash,
        &redemption.source_note_commitment,
        hash_limb0(&issued_claim_hash),
        0,
    );

    let mut input_amounts = [0u64; OFFLINE_NOTE_MAX_INPUT_AMOUNTS];
    input_amounts[0] = input_sum;
    let mut output_amounts = [0u64; OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS];
    output_amounts[0] = output_sum;

    Ok(OfflineNoteInstanceValues {
        public_values,
        input_amounts,
        output_amounts,
    })
}

/// Build the public/private instance values expected for an Offline audit proof.
///
/// # Errors
///
/// Returns an error if the audit shape is outside the circuit corridor, if Norito
/// hashing fails, if amounts do not fit in witness units, or if audited input and
/// output amounts are not conserved.
pub fn offline_note_audit_instance_values(
    audit: &iroha_data_model::offline::OfflineNoteAuditBundle,
) -> Result<OfflineNoteInstanceValues, String> {
    use iroha_data_model::offline::OfflineNoteIssuedClaim;

    let input_count = validate_offline_note_count(
        audit.input_claims.len(),
        OFFLINE_NOTE_MAX_INPUT_AMOUNTS,
        "audit input",
    )?;
    let output_count = validate_offline_note_count(
        audit.output_claims.len(),
        OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS,
        "audit output",
    )?;
    if audit.input_nullifiers.len() != audit.input_claims.len() {
        return Err("offline audit input claim count must match input nullifier count".to_owned());
    }

    let public_inputs_hash = audit
        .public_inputs_hash()
        .map_err(|err| format!("failed to encode Offline audit public inputs: {err}"))?;
    let key_certificate_payload_hash = audit
        .sender_key_certificate
        .payload_hash()
        .map_err(|err| format!("failed to encode Offline key certificate payload: {err}"))?;

    let input_claim_hashes = audit
        .input_claims
        .iter()
        .map(|claim| {
            claim
                .claim_hash()
                .map_err(|err| format!("failed to encode Offline audit input claim: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let output_claim_hashes = audit
        .output_claims
        .iter()
        .map(|claim| {
            OfflineNoteIssuedClaim::from_audit_output(claim)
                .and_then(|claim| claim.claim_hash())
                .map_err(|err| format!("failed to encode Offline audit output claim: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let amount_refs = audit
        .input_claims
        .iter()
        .map(|claim| &claim.amount)
        .chain(audit.output_claims.iter().map(|claim| &claim.amount))
        .collect::<Vec<_>>();
    let normalized_amounts = normalized_amount_vec(&amount_refs)?;
    let input_len = audit.input_claims.len();
    let input_units = &normalized_amounts[..input_len];
    let output_units = &normalized_amounts[input_len..];
    let input_sum = checked_u64_sum(input_units, "input")?;
    let output_sum = checked_u64_sum(output_units, "output")?;
    if input_sum != output_sum {
        return Err("offline audit proof amounts are not conserved".to_owned());
    }

    let mut input_amounts = [0u64; OFFLINE_NOTE_MAX_INPUT_AMOUNTS];
    for (slot, amount) in input_amounts.iter_mut().zip(input_units.iter().copied()) {
        *slot = amount;
    }
    let mut output_amounts = [0u64; OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS];
    for (slot, amount) in output_amounts.iter_mut().zip(output_units.iter().copied()) {
        *slot = amount;
    }

    let public_values = offline_note_public_values(
        &public_inputs_hash,
        OFFLINE_NOTE_MODE_AUDIT,
        input_count,
        output_count,
        input_sum,
        output_sum,
        hash_limb0_sum(&audit.input_nullifiers),
        hash_limb0_sum(&audit.output_commitments),
        &key_certificate_payload_hash,
        &audit.token_id,
        hash_limb0_sum(&input_claim_hashes),
        hash_limb0_sum(&output_claim_hashes),
    );

    Ok(OfflineNoteInstanceValues {
        public_values,
        input_amounts,
        output_amounts,
    })
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

/// Block-local key for cryptographically preverified proof results.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PreverifiedProofKey {
    /// Domain-separated hash of the proof payload.
    pub proof_hash: [u8; 32],
    /// Registered verifying-key reference used for the verification result.
    pub vk_ref: VerifyingKeyId,
    /// Commitment of the registered verifying-key bytes used for verification.
    pub vk_commitment: [u8; 32],
}

impl PreverifiedProofKey {
    /// Build a preverified-result key from the proof payload and registered key binding.
    #[must_use]
    pub fn new(proof: &ProofBox, vk_ref: &VerifyingKeyId, vk_commitment: [u8; 32]) -> Self {
        Self {
            proof_hash: hash_proof(proof),
            vk_ref: vk_ref.clone(),
            vk_commitment,
        }
    }
}

/// Block-local preverified proof result map.
pub type PreverifiedProofMap = std::collections::BTreeMap<PreverifiedProofKey, bool>;

/// Compute a stable, domain-separated 32-byte hash of the verifying key payload.
pub fn hash_vk(vk: &VerifyingKeyBox) -> [u8; 32] {
    hash_vk_bytes(&vk.backend, &vk.bytes)
}

pub(crate) fn hash_vk_bytes(backend: &str, bytes: &[u8]) -> [u8; 32] {
    hash_domain_separated_payload(b"iroha:zk:v1:vk", backend, bytes)
}

/// Returns `true` when `backend` denotes the native STARK/FRI verifier family.
///
/// The canonical family identifier is `stark/fri`. Implementations may use
/// additional suffixes under this prefix (e.g., `stark/fri/sha256-goldilocks`).
#[inline]
pub(crate) fn is_stark_fri_v1_backend(backend: &str) -> bool {
    backend == ZK_BACKEND_STARK_FRI_V1 || backend.starts_with("stark/fri/")
}

/// Returns `true` when `backend` is accepted for `ivm-execution-v1` proofs.
#[inline]
#[must_use]
pub fn is_ivm_execution_backend(backend: &str) -> bool {
    backend == ZK_BACKEND_HALO2_IPA || is_stark_fri_v1_backend(backend)
}

fn hash_to_u64_limbs_le(hash: &iroha_crypto::Hash) -> [u64; 4] {
    let mut limbs = [0u64; 4];
    let bytes: &[u8; 32] = hash.as_ref();
    for (idx, limb) in limbs.iter_mut().enumerate() {
        let start = idx * 8;
        let end = start + 8;
        *limb = u64::from_le_bytes(bytes[start..end].try_into().expect("8-byte limb"));
    }
    limbs
}

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
    use std::io::Cursor;

    use halo2_proofs::{
        SerdeFormat,
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{ProvingKey, VerifyingKey, create_proof, keygen_pk},
        poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
    };
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope};
    use rand_core_06::OsRng;

    if vk_box.backend.as_str() != ZK_BACKEND_HALO2_IPA {
        return Err("ivm execution proving requires halo2/ipa verifying key backend".to_owned());
    }

    let params = zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed_vk: VerifyingKey<Curve> =
        zkparse::vk_from_bytes::<pasta_tiny::IvmExecutionBindV1>(vk_box.bytes.as_slice(), &params)
            .ok_or_else(|| {
                "missing/invalid H2VK payload for ivm-execution-v1 verifying key".to_owned()
            })?;

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

    let proving_key: ProvingKey<Curve> = if let Some(bytes) = proving_key_bytes {
        let mut cursor = Cursor::new(bytes);
        let pk = read_proving_key::<pasta_tiny::IvmExecutionBindV1, _>(&mut cursor)
            .map_err(|err| format!("failed to decode proving key: {err}"))?;
        let consumed = usize::try_from(cursor.position()).unwrap_or(usize::MAX);
        if consumed != bytes.len() {
            return Err("failed to decode proving key: trailing bytes".to_owned());
        }
        if pk.get_vk().get_domain().k() != params.k() {
            return Err("proving key domain does not match IPAK parameters".to_owned());
        }
        if pk.get_vk().to_bytes(SerdeFormat::Processed)
            != parsed_vk.to_bytes(SerdeFormat::Processed)
        {
            return Err("proving key verifying key does not match vk_ref bytes".to_owned());
        }
        pk
    } else {
        keygen_pk(
            &params,
            parsed_vk.clone(),
            &pasta_tiny::IvmExecutionBindV1::default(),
        )
        .map_err(|err| format!("failed to derive proving key: {err}"))?
    };

    let circuit = pasta_tiny::IvmExecutionBindV1 { values };
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    create_proof::<IPACommitmentScheme<Curve>, ProverIPA<'_, Curve>, Challenge255<Curve>, _, _, _>(
        &params,
        &proving_key,
        &[circuit],
        &instance_refs,
        OsRng,
        &mut transcript,
    )
    .map_err(|err| format!("failed to create ivm-execution-v1 proof: {err}"))?;
    let proof_raw = transcript.finalize();

    let mut proof_payload = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_payload, &proof_raw);
    zk1::wrap_append_instances_pasta_fp_cols(instance_columns.as_slice(), &mut proof_payload);

    let public_inputs = ivm_execution_public_inputs_schema_descriptor().to_vec();
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: circuit_id.to_owned(),
        vk_hash: hash_vk(vk_box),
        public_inputs,
        proof_bytes: proof_payload,
        aux: Vec::new(),
    };
    let encoded = norito::to_bytes(&envelope)
        .map_err(|err| format!("failed to encode OpenVerifyEnvelope: {err}"))?;
    Ok(ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), encoded))
}

/// Derive Halo2 IPA proving-key bytes for the canonical `ivm-execution-v1` circuit.
///
/// The returned bytes are the Halo2 `ProvingKey` serialization using `SerdeFormat::Processed`,
/// suitable for persistence in the Torii prover key store (`<backend>__<name>.pk`).
///
/// Note: this operation is expensive (key generation) and should be performed offline.
#[cfg(feature = "zk-halo2-ipa")]
pub fn derive_halo2_ipa_ivm_execution_proving_key_bytes(
    vk_box: &VerifyingKeyBox,
) -> Result<Vec<u8>, String> {
    use halo2_proofs::{
        SerdeFormat,
        halo2curves::pasta::EqAffine as Curve,
        plonk::{VerifyingKey, keygen_pk},
    };

    if vk_box.backend.as_str() != ZK_BACKEND_HALO2_IPA {
        return Err(
            "ivm execution proving key derivation requires halo2/ipa verifying key backend"
                .to_owned(),
        );
    }

    let params = zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed_vk: VerifyingKey<Curve> =
        zkparse::vk_from_bytes::<pasta_tiny::IvmExecutionBindV1>(vk_box.bytes.as_slice(), &params)
            .ok_or_else(|| {
                "missing/invalid H2VK payload for ivm-execution-v1 verifying key".to_owned()
            })?;

    let pk = keygen_pk(
        &params,
        parsed_vk,
        &pasta_tiny::IvmExecutionBindV1::default(),
    )
    .map_err(|err| format!("failed to derive proving key: {err}"))?;
    Ok(pk.to_bytes(SerdeFormat::Processed))
}

#[cfg(feature = "zk-halo2-ipa")]
fn prove_halo2_ipa_offline_note_envelope(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    instance_values: OfflineNoteInstanceValues,
    proving_key_bytes: Option<&[u8]>,
) -> Result<ProofBox, String> {
    use std::io::Cursor;

    use halo2_proofs::{
        SerdeFormat,
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{ProvingKey, VerifyingKey, create_proof, keygen_pk},
        poly::ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
        transcript::{Blake2bWrite, Challenge255, TranscriptWriterBuffer as _},
    };
    use iroha_data_model::{
        offline::OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA,
        zk::{BackendTag, OpenVerifyEnvelope},
    };
    use rand_core_06::OsRng;

    if !is_offline_note_recursive_circuit_id(circuit_id) {
        return Err(format!(
            "unsupported Offline recursive circuit id `{circuit_id}`"
        ));
    }
    if vk_box.backend.as_str() != ZK_BACKEND_HALO2_IPA {
        return Err("offline proving requires halo2/ipa verifying key backend".to_owned());
    }

    let params = zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed_vk: VerifyingKey<Curve> =
        zkparse::vk_from_bytes::<pasta_tiny::OfflineNoteSemantic>(vk_box.bytes.as_slice(), &params)
            .ok_or_else(|| {
                "missing/invalid H2VK payload for offline-note-recursive verifying key".to_owned()
            })?;

    let public_values = instance_values.public_scalars();
    let instance_columns_owned: Vec<Vec<Scalar>> =
        public_values.iter().map(|value| vec![*value]).collect();
    let instance_columns: Vec<&[Scalar]> =
        instance_columns_owned.iter().map(Vec::as_slice).collect();
    let instance_refs: Vec<&[&[Scalar]]> = vec![instance_columns.as_slice()];

    let proving_key: ProvingKey<Curve> = if let Some(bytes) = proving_key_bytes {
        let mut cursor = Cursor::new(bytes);
        let pk = read_proving_key::<pasta_tiny::OfflineNoteSemantic, _>(&mut cursor)
            .map_err(|err| format!("failed to decode proving key: {err}"))?;
        let consumed = usize::try_from(cursor.position()).unwrap_or(usize::MAX);
        if consumed != bytes.len() {
            return Err("failed to decode proving key: trailing bytes".to_owned());
        }
        if pk.get_vk().get_domain().k() != params.k() {
            return Err("proving key domain does not match IPAK parameters".to_owned());
        }
        if pk.get_vk().to_bytes(SerdeFormat::Processed)
            != parsed_vk.to_bytes(SerdeFormat::Processed)
        {
            return Err("proving key verifying key does not match vk_ref bytes".to_owned());
        }
        pk
    } else {
        keygen_pk(
            &params,
            parsed_vk.clone(),
            &pasta_tiny::OfflineNoteSemantic::default(),
        )
        .map_err(|err| format!("failed to derive proving key: {err}"))?
    };

    let circuit = pasta_tiny::OfflineNoteSemantic {
        public_values,
        input_amounts: instance_values.input_amount_scalars(),
        output_amounts: instance_values.output_amount_scalars(),
    };
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    create_proof::<IPACommitmentScheme<Curve>, ProverIPA<'_, Curve>, Challenge255<Curve>, _, _, _>(
        &params,
        &proving_key,
        &[circuit],
        &instance_refs,
        OsRng,
        &mut transcript,
    )
    .map_err(|err| format!("failed to create offline-note-recursive proof: {err}"))?;
    let proof_raw = transcript.finalize();

    let mut proof_payload = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_payload, &proof_raw);
    zk1::wrap_append_instances_pasta_fp_cols(instance_columns.as_slice(), &mut proof_payload);

    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: circuit_id.to_owned(),
        vk_hash: hash_vk(vk_box),
        public_inputs: OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA.to_vec(),
        proof_bytes: proof_payload,
        aux: Vec::new(),
    };
    let encoded = norito::to_bytes(&envelope)
        .map_err(|err| format!("failed to encode OpenVerifyEnvelope: {err}"))?;
    Ok(ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), encoded))
}

/// Derive Halo2 IPA proving-key bytes for the canonical Offline recursive note circuit.
///
/// The returned bytes are the Halo2 `ProvingKey` serialization using
/// `SerdeFormat::Processed`. They are intended to be generated offline and
/// supplied to the real prover path; no mock or debug prover is used.
#[cfg(feature = "zk-halo2-ipa")]
pub fn derive_halo2_ipa_offline_note_proving_key_bytes(
    vk_box: &VerifyingKeyBox,
) -> Result<Vec<u8>, String> {
    use halo2_proofs::{
        SerdeFormat,
        halo2curves::pasta::EqAffine as Curve,
        plonk::{VerifyingKey, keygen_pk},
    };

    if vk_box.backend.as_str() != ZK_BACKEND_HALO2_IPA {
        return Err(
            "offline proving key derivation requires halo2/ipa verifying key backend".to_owned(),
        );
    }

    let params = zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed_vk: VerifyingKey<Curve> =
        zkparse::vk_from_bytes::<pasta_tiny::OfflineNoteSemantic>(vk_box.bytes.as_slice(), &params)
            .ok_or_else(|| {
                "missing/invalid H2VK payload for offline-note-recursive verifying key".to_owned()
            })?;

    let pk = keygen_pk(
        &params,
        parsed_vk,
        &pasta_tiny::OfflineNoteSemantic::default(),
    )
    .map_err(|err| format!("failed to derive proving key: {err}"))?;
    Ok(pk.to_bytes(SerdeFormat::Processed))
}

/// Prove an Offline redemption with the real Halo2 IPA recursive-note circuit.
///
/// # Errors
///
/// Returns an error if the verifier/proving key is incompatible, the redemption
/// cannot be converted to the semantic instance layout, or proof generation fails.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_offline_note_redeem(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    redemption: &iroha_data_model::offline::OfflineNoteRedeem,
    proving_key_bytes: Option<&[u8]>,
) -> Result<ProofBox, String> {
    let instance_values = offline_note_redeem_instance_values(redemption)?;
    prove_halo2_ipa_offline_note_envelope(circuit_id, vk_box, instance_values, proving_key_bytes)
}

/// Prove an Offline audit with the real Halo2 IPA recursive-note circuit.
///
/// # Errors
///
/// Returns an error if the verifier/proving key is incompatible, the audit cannot
/// be converted to the semantic instance layout, or proof generation fails.
#[cfg(feature = "zk-halo2-ipa")]
pub fn prove_offline_note_audit(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    audit: &iroha_data_model::offline::OfflineNoteAuditBundle,
    proving_key_bytes: Option<&[u8]>,
) -> Result<ProofBox, String> {
    let instance_values = offline_note_audit_instance_values(audit)?;
    prove_halo2_ipa_offline_note_envelope(circuit_id, vk_box, instance_values, proving_key_bytes)
}

#[cfg(feature = "zk-stark")]
fn normalize_stark_fri_circuit_id_for_backend(backend: &str, raw: &str) -> Option<String> {
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

#[cfg(feature = "zk-stark")]
fn stark_open_verify_domain_tag_current(
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
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};

    if !is_stark_fri_v1_backend(backend) {
        return Err("backend is not a STARK/FRI V1 backend".to_owned());
    }
    let vk_payload: crate::zk_stark::StarkFriVerifyingKeyV1 =
        norito::decode_from_bytes(&vk_box.bytes)
            .map_err(|err| format!("invalid STARK verifying key payload: {err}"))?;
    if vk_payload.version != 1 {
        return Err("unsupported STARK verifying key payload version".to_owned());
    }
    let env_circuit_id = normalize_stark_fri_circuit_id_for_backend(backend, circuit_id)
        .ok_or_else(|| "invalid STARK circuit_id".to_owned())?;
    let vk_circuit_id = normalize_stark_fri_circuit_id_for_backend(backend, &vk_payload.circuit_id)
        .ok_or_else(|| "invalid STARK verifying key circuit_id".to_owned())?;
    if env_circuit_id != vk_circuit_id {
        return Err("STARK verifying key circuit_id mismatch".to_owned());
    }
    let expected_hash_fn = if backend == ZK_BACKEND_STARK_FRI_V1 {
        vk_payload.hash_fn
    } else if backend.contains("/sha256-") {
        crate::zk_stark::STARK_HASH_SHA256_V1
    } else if backend.contains("/poseidon2-") {
        crate::zk_stark::STARK_HASH_POSEIDON2_V1
    } else {
        return Err("unsupported STARK/FRI backend variant".to_owned());
    };
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
    let envelope_bytes = crate::zk_stark::prove_stark_fri_air_envelope_bytes(
        params,
        "IROHA-STARK-AIR-V1".to_owned(),
        env_circuit_id.clone(),
        public_digest,
    )?;
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
        proof_bytes: norito::to_bytes(&open)
            .map_err(|err| format!("failed to encode STARK wrapper payload: {err}"))?,
        aux: Vec::new(),
    };
    let bytes = norito::to_bytes(&env)
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
    let public_inputs = ivm_execution_public_inputs_columns(
        code_hash,
        overlay_hash,
        events_commitment,
        gas_policy_commitment,
    );
    prove_stark_fri_open_verify_envelope(
        backend,
        circuit_id,
        vk_box,
        ivm_execution_public_inputs_schema_descriptor(),
        public_inputs,
    )
}

#[cfg(any(test, feature = "iroha-core-tests"))]
/// Test fixtures and helpers for constructing deterministic `OpenVerifyEnvelope` payloads.
pub mod test_utils {
    use iroha_crypto::Hash as CryptoHash;
    use iroha_data_model::{
        proof::{ProofBox, VerifyingKeyBox},
        zk::{BackendTag, OpenVerifyEnvelope},
    };

    #[allow(unused_imports)]
    use super::*;

    #[cfg(feature = "iroha_zkp_halo2")]
    const HALO2_N_IN: u8 = 1;
    #[cfg(feature = "iroha_zkp_halo2")]
    const HALO2_N_OUT: u8 = 1;
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
        let proof_bytes =
            norito::to_bytes(&envelope).expect("OpenVerifyEnvelope Norito serialization must work");
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
        let proof_bytes =
            norito::to_bytes(&envelope).expect("OpenVerifyEnvelope Norito serialization must work");
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
        let proof_bytes =
            norito::to_bytes(&envelope).expect("OpenVerifyEnvelope Norito serialization must work");
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
            "halo2/ipa:ivm-execution-v1",
            code_hash,
            overlay_hash,
            events_commitment,
            gas_policy_commitment,
        )
    }

    type FixtureBundle = fn() -> (Vec<u8>, Vec<u8>, Vec<u8>);

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    fn fixture_circuit_from_id(circuit_id: &str) -> Option<FixtureBundle> {
        let backend = super::halo2_ipa_backend_from_circuit_id(circuit_id)?;
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

    #[cfg(feature = "iroha_zkp_halo2")]
    fn halo2_proof_payload(public_inputs: &[u8]) -> Vec<u8> {
        use iroha_zkp_halo2::{
            FLAG_LOOKUPS, Halo2ProofEnvelope, Halo2ProofEnvelopeHeader, PUBLIC_INPUT_STRIDE,
        };

        let stride = PUBLIC_INPUT_STRIDE;
        let arrays = public_inputs
            .chunks(stride)
            .map(|chunk| {
                let mut arr = [0u8; PUBLIC_INPUT_STRIDE];
                arr.copy_from_slice(chunk);
                arr
            })
            .collect::<Vec<_>>();
        let expected =
            Halo2ProofEnvelopeHeader::expected_pi_count(HALO2_N_IN, HALO2_N_OUT) as usize;
        debug_assert_eq!(arrays.len(), expected);
        Halo2ProofEnvelope::new(
            18,
            HALO2_N_IN,
            HALO2_N_OUT,
            FLAG_LOOKUPS,
            arrays,
            vec![0xAB; HALO2_PROOF_BYTES_LEN],
        )
        .expect("construct Halo2 envelope fixture")
        .to_bytes()
    }

    #[cfg(not(feature = "iroha_zkp_halo2"))]
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
    params_fingerprint: [u8; 32],
    vk_hash: [u8; 32],
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
type CachedVk = Arc<halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>>;

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn params_fingerprint(params: &PastaParams) -> [u8; 32] {
    use std::io::{self, Write};

    use halo2_proofs::poly::commitment::Params as _;
    use sha2::{Digest, Sha256};

    struct HashWriter<'a, H: Digest>(&'a mut H);

    impl<H: Digest> Write for HashWriter<'_, H> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.update(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let mut hasher = Sha256::new();
    let mut writer = HashWriter(&mut hasher);
    params
        .write(&mut writer)
        .expect("failed to hash Halo2 params");
    hasher.finalize().into()
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
static VK_CACHE: OnceLock<Mutex<BTreeMap<VkCacheKey, CachedVk>>> = OnceLock::new();

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct BuiltinVkCacheKey {
    backend: String,
    params_fingerprint: [u8; 32],
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
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
fn lock_cache<T>(cache: &Mutex<T>) -> Result<MutexGuard<'_, T>, halo2_proofs::plonk::Error> {
    cache
        .lock()
        .map_err(|_| halo2_proofs::plonk::Error::ConstraintSystemFailure)
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn resolve_vk_cached<C, F>(
    backend: &str,
    params: &PastaParams,
    vk_box: &VerifyingKeyBox,
    _circuit: &C,
    builder: F,
) -> Result<CachedVk, halo2_proofs::plonk::Error>
where
    C: halo2_proofs::plonk::Circuit<halo2_proofs::halo2curves::pasta::Fp>,
    F: FnOnce() -> Result<
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
        halo2_proofs::plonk::Error,
    >,
{
    use halo2_proofs::SerdeFormat;

    let cache = VK_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let params_fp = params_fingerprint(params);
    let vk_hash = hash_vk(vk_box);
    let key = VkCacheKey {
        backend: backend.to_string(),
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

    // Try to parse the verifying key from the provided bytes first.
    if let Some(parsed) = zkparse::vk_from_bytes::<C>(vk_box.bytes.as_slice(), params) {
        let arc = Arc::new(parsed);
        let mut guard = lock_cache(cache)?;
        let entry = match guard.entry(key.clone()) {
            Entry::Occupied(existing) => existing.get().clone(),
            Entry::Vacant(slot) => Arc::clone(slot.insert(Arc::clone(&arc))),
        };
        return Ok(entry);
    }

    // Fall back to synthesising the verifying key via keygen.
    let built = builder()?;
    let built_hash = {
        let bytes = built.to_bytes(SerdeFormat::Processed);
        hash_vk_bytes(backend, &bytes)
    };
    if built_hash != vk_hash {
        return Err(halo2_proofs::plonk::Error::ConstraintSystemFailure);
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
macro_rules! cached_vk_for {
    ($params:expr, $backend:expr, $vk_box:expr, $circuit:expr, |$vk:ident| $body:block) => {{
        let params_ref = $params;
        let vk_ref = $vk_box;
        let circuit = $circuit;
        match resolve_vk_cached($backend, params_ref, vk_ref, &circuit, || {
            halo2_proofs::plonk::keygen_vk(params_ref, &circuit)
        }) {
            Ok(arc) => {
                let $vk = arc.as_ref();
                $body
            }
            Err(_) => false,
        }
    }};
}

#[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
#[allow(unused_macros)]
macro_rules! cached_vk_for {
    ($params:expr, $backend:expr, $vk_box:expr, $circuit:expr, |$vk:ident| $body:block) => {{
        let _ = ($params, $backend, $vk_box, $circuit);
        false
    }};
}

#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn keygen_vk_cached<C>(
    backend: &str,
    params: &PastaParams,
    circuit: &C,
) -> Result<CachedVk, halo2_proofs::plonk::Error>
where
    C: halo2_proofs::plonk::Circuit<halo2_proofs::halo2curves::pasta::Fp>,
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
    let vk = halo2_proofs::plonk::keygen_vk(params, circuit)?;
    let arc = Arc::new(vk);
    let mut guard = lock_cache(cache)?;
    let entry = match guard.entry(key) {
        Entry::Occupied(existing) => existing.get().clone(),
        Entry::Vacant(slot) => Arc::clone(slot.insert(Arc::clone(&arc))),
    };
    Ok(entry)
}

// Verifying keys are cached above (`VK_CACHE` / `BUILTIN_VK_CACHE`) and keyed by backend,
// parameter fingerprint, and verifying-key hash so repeated proofs avoid recomputing `keygen_vk`.

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
///  - `b"I10P"`: Instance columns over Pasta Fp (cols[u32], rows[u32], rows*cols scalars).
///
/// Notes:
///  - Backends remain identified outside of the envelope via `ProofBox.backend`.
mod zk1 {
    use std::io::{Cursor, Read};

    use super::*;

    const MAGIC: &[u8; 4] = b"ZK1\0";

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

    /// Append a Halo2 verifying key (`H2VK`) for Pasta/IPA circuits.
    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    #[allow(dead_code)]
    pub fn wrap_append_vk_pasta(
        buf: &mut Vec<u8>,
        vk: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    ) {
        let bytes = vk.to_bytes(SerdeFormat::Processed);
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
    /// The layout matches the reader in `extract_proof_pasta` and `zkparse::proof_and_instances`:
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

#[cfg(feature = "zk-tests")]
/// Test-only wrappers around the ZK1 envelope helpers.
///
/// These functions expose the internal encoding utilities used to build ZK1 proof
/// envelopes so integration tests can assemble deterministic fixtures without
/// leaking the implementation details into the public API surface.
pub mod zk1_test_helpers {
    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp},
        plonk::VerifyingKey,
    };

    /// Begin a new ZK1 envelope.
    #[inline]
    pub fn wrap_start() -> Vec<u8> {
        super::zk1::wrap_start()
    }

    /// Append raw proof bytes to a ZK1 envelope.
    #[inline]
    pub fn wrap_append_proof(buf: &mut Vec<u8>, transcript_bytes: &[u8]) {
        super::zk1::wrap_append_proof(buf, transcript_bytes)
    }

    /// Append the Halo2 IPA parameter `k` TLV to a ZK1 envelope.
    #[inline]
    pub fn wrap_append_ipa_k(buf: &mut Vec<u8>, k: u32) {
        super::zk1::wrap_append_ipa_k(buf, k)
    }

    /// Append a verifying key payload encoded for Pasta curves.
    #[inline]
    pub fn wrap_append_vk_pasta(buf: &mut Vec<u8>, vk: &VerifyingKey<Curve>) {
        super::zk1::wrap_append_vk_pasta(buf, vk)
    }

    /// Append Pasta-Fp instance columns to a ZK1 envelope.
    #[inline]
    pub fn wrap_append_instances_pasta_fp(instances: &[Fp], buf: &mut Vec<u8>) {
        super::zk1::wrap_append_instances_pasta_fp(instances, buf)
    }

    /// Append Pasta-Fp instance column slices to a ZK1 envelope.
    #[inline]
    pub fn wrap_append_instances_pasta_fp_cols(cols: &[&[Fp]], buf: &mut Vec<u8>) {
        super::zk1::wrap_append_instances_pasta_fp_cols(cols, buf)
    }
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
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::pasta::Fp as Scalar,
        plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector},
        poly::Rotation,
    };

    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    #[allow(unused_imports)]
    use crate::zk::pasta_tiny::poseidon::{Poseidon2ChipWrapper, Pow5Chip, Pow5Config};
    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    #[allow(unused_imports)]
    use crate::zk::pasta_tiny::poseidon_compress2_native;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "v",
                        v,
                        0,
                        || Value::known(Scalar::from(1)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "rho",
                        rho,
                        0,
                        || Value::known(Scalar::from(12345)),
                    )?;
                    for (i, col) in sibs.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(20 + i as u64)),
                        )?;
                    }
                    let mut acc = compress(Scalar::one(), Scalar::from(12345));
                    for (i, col) in ws.iter().enumerate() {
                        let sibling = Scalar::from(20 + i as u64);
                        acc = compress(acc, sibling);
                        #[cfg(debug_assertions)]
                        {
                            println!("vote_merkle witness w{i} = {acc:?}");
                        }
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w{i}"),
                            *col,
                            0,
                            || Value::known(acc),
                        )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in0",
                        in0,
                        0,
                        || Value::known(Scalar::from(7)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in1",
                        in1,
                        0,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out0",
                        out0,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out1",
                        out1,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r0",
                        r0,
                        0,
                        || Value::known(Scalar::from(11)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r1",
                        r1,
                        0,
                        || Value::known(Scalar::from(13)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r2",
                        r2,
                        0,
                        || Value::known(Scalar::from(17)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r3",
                        r3,
                        0,
                        || Value::known(Scalar::from(19)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sk",
                        sk,
                        0,
                        || Value::known(Scalar::from(1_234_567)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "serial",
                        serial,
                        0,
                        || Value::known(Scalar::from(42)),
                    )?;
                    for (i, col) in sib_a.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib_a{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(20 + i as u64)),
                        )?;
                    }
                    for (i, col) in dir_a.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir_a{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(0)),
                        )?;
                    }
                    let mut acc = Scalar::from(0);
                    for (i, col) in w_a.iter().enumerate() {
                        acc += Scalar::from(20 + i as u64);
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w_a{i}"),
                            *col,
                            0,
                            || Value::known(acc),
                        )?;
                    }
                    for (i, col) in sib_b.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib_b{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(30 + i as u64)),
                        )?;
                    }
                    for (i, col) in dir_b.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir_b{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(0)),
                        )?;
                    }
                    let mut acc_b = Scalar::from(0);
                    for (i, col) in w_b.iter().enumerate() {
                        acc_b += Scalar::from(30 + i as u64);
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w_b{i}"),
                            *col,
                            0,
                            || Value::known(acc_b),
                        )?;
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
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner},
        halo2curves::pasta::Fp as Scalar,
        plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector},
        poly::Rotation,
    };

    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    #[allow(unused_imports)]
    use crate::zk::pasta_tiny::poseidon::{Poseidon2ChipWrapper, Pow5Chip, Pow5Config};
    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    #[allow(unused_imports)]
    use crate::zk::pasta_tiny::poseidon_compress2_native;

    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
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
    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
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
    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "v",
                        v,
                        0,
                        || halo2_proofs::circuit::Value::known(Scalar::from(1u64)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "rho",
                        rho,
                        0,
                        || halo2_proofs::circuit::Value::known(Scalar::from(12345u64)),
                    )?;
                    for (i, col) in sibs.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib{i}"),
                            *col,
                            0,
                            || halo2_proofs::circuit::Value::known(Scalar::from(20 + i as u64)),
                        )?;
                    }
                    for (i, col) in dirs.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir{i}"),
                            *col,
                            0,
                            || halo2_proofs::circuit::Value::known(Scalar::from(0)),
                        )?;
                    }
                    let mut acc = Scalar::from(0);
                    for (i, col) in ws.iter().enumerate() {
                        acc += Scalar::from(20 + i as u64);
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w{i}"),
                            *col,
                            0,
                            || halo2_proofs::circuit::Value::known(acc),
                        )?;
                    }
                    Ok(())
                },
            )
        }
    }

    #[cfg(feature = "zk-halo2-ipa-poseidon")]
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "v",
                        v,
                        0,
                        || Value::known(v_val),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "rho",
                        rho,
                        0,
                        || Value::known(rho_val),
                    )?;
                    let commit_left_cell = crate::zk::assign_advice_compat(
                        &mut region,
                        || "commit_left",
                        commit_left,
                        0,
                        || Value::known(v_val),
                    )?;
                    let commit_right_cell = crate::zk::assign_advice_compat(
                        &mut region,
                        || "commit_right",
                        commit_right,
                        0,
                        || Value::known(rho_val),
                    )?;
                    let commit_hash_cell = crate::zk::assign_advice_compat(
                        &mut region,
                        || "commit_hash",
                        commit_hash,
                        0,
                        || Value::known(commit_digest),
                    )?;

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
                        sib_cells.push(crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib{i}"),
                            sibs[i],
                            0,
                            || Value::known(sib_val),
                        )?);
                        dir_cells.push(crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir{i}"),
                            dirs[i],
                            0,
                            || Value::known(dir_val),
                        )?);

                        let (left_val, right_val) = if dir_val == Scalar::one() {
                            (sib_val, prev_val)
                        } else {
                            (prev_val, sib_val)
                        };

                        left_cells.push(crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("poseidon_left{i}"),
                            poseidon_left[i],
                            0,
                            || Value::known(left_val),
                        )?);
                        right_cells.push(crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("poseidon_right{i}"),
                            poseidon_right[i],
                            0,
                            || Value::known(right_val),
                        )?);

                        let digest_val = poseidon_compress2_native(left_val, right_val);
                        w_cells.push(crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w{i}"),
                            ws[i],
                            0,
                            || Value::known(digest_val),
                        )?);

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
    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
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
                    use halo2_proofs::circuit::Value;
                    s.enable(&mut region, 0)?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in0",
                        in0,
                        0,
                        || Value::known(Scalar::from(7)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in1",
                        in1,
                        0,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out0",
                        out0,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out1",
                        out1,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r0",
                        r0,
                        0,
                        || Value::known(Scalar::from(11)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r1",
                        r1,
                        0,
                        || Value::known(Scalar::from(13)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r2",
                        r2,
                        0,
                        || Value::known(Scalar::from(17)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r3",
                        r3,
                        0,
                        || Value::known(Scalar::from(19)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sk",
                        sk,
                        0,
                        || Value::known(Scalar::from(1_234_567)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "serial",
                        serial,
                        0,
                        || Value::known(Scalar::from(42)),
                    )?;
                    for (i, col) in sib_a.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib_a{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(20 + i as u64)),
                        )?;
                    }
                    for (i, col) in dir_a.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir_a{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(0)),
                        )?;
                    }
                    let mut acc = Scalar::from(0);
                    for (i, col) in w_a.iter().enumerate() {
                        acc += Scalar::from(20 + i as u64);
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w_a{i}"),
                            *col,
                            0,
                            || Value::known(acc),
                        )?;
                    }
                    for (i, col) in sib_b.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib_b{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(30 + i as u64)),
                        )?;
                    }
                    for (i, col) in dir_b.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir_b{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(0)),
                        )?;
                    }
                    let mut acc_b = Scalar::from(0);
                    for (i, col) in w_b.iter().enumerate() {
                        acc_b += Scalar::from(30 + i as u64);
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w_b{i}"),
                            *col,
                            0,
                            || Value::known(acc_b),
                        )?;
                    }
                    Ok(())
                },
            )
        }
    }

    #[cfg(feature = "zk-halo2-ipa-poseidon")]
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in0",
                        in0,
                        0,
                        || Value::known(Scalar::from(7)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in1",
                        in1,
                        0,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out0",
                        out0,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out1",
                        out1,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r0",
                        r0,
                        0,
                        || Value::known(Scalar::from(11)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r1",
                        r1,
                        0,
                        || Value::known(Scalar::from(13)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r2",
                        r2,
                        0,
                        || Value::known(Scalar::from(17)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r3",
                        r3,
                        0,
                        || Value::known(Scalar::from(19)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sk",
                        sk,
                        0,
                        || Value::known(Scalar::from(1_234_567)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "serial",
                        serial,
                        0,
                        || Value::known(Scalar::from(42)),
                    )?;
                    for (i, col) in sib_a.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib_a{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(20 + i as u64)),
                        )?;
                    }
                    for (i, col) in dir_a.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir_a{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(0)),
                        )?;
                    }
                    for (i, col) in sib_b.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib_b{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(30 + i as u64)),
                        )?;
                    }
                    for (i, col) in dir_b.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir_b{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(0)),
                        )?;
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
                    |mut region| {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "w_a",
                            *w_col,
                            0,
                            || Value::known(w_val),
                        )
                    },
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
                    |mut region| {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "w_b",
                            *w_col,
                            0,
                            || Value::known(w_val),
                        )
                    },
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
const TRACE_MOCK_BLOB_HEADER: &[u8; 4] = b"ZKMP";
#[cfg(feature = "zk-preverify")]
const TRACE_MOCK_BACKEND: &str = "zk-trace/mock-proof";
#[cfg(feature = "zk-preverify")]
const TRACE_QUEUE_MAX_SPINS: usize = 20;
#[cfg(feature = "zk-preverify")]
const TRACE_QUEUE_SLEEP_MS: u64 = 10;

#[cfg(feature = "zk-preverify")]
/// Captured trace metadata awaiting background proof generation.
#[derive(Clone)]
pub struct TraceForProving {
    digest: [u8; 32],
    program: Arc<Vec<u8>>,
    trace: Vec<ivm::zk::RegisterState>,
    constraints: Vec<ivm::zk::Constraint>,
    code_hash: [u8; 32],
    tx_hash: Option<[u8; 32]>,
    transport_capabilities: Option<TransportCapabilityResolutionSnapshot>,
    negotiated_capabilities: Option<CapabilityFlags>,
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
            transport_capabilities: task.transport_capabilities,
            negotiated_capabilities: task.negotiated_capabilities,
        }
    }

    fn encode_mock_proof_bytes(&self) -> Vec<u8> {
        let mut buf =
            Vec::with_capacity(4 + 1 + 32 + 4 + 4 + 1 + 32 + 1 + 2 + 1 + 2 + 2 + 1 + 1 + 8);
        buf.extend_from_slice(TRACE_MOCK_BLOB_HEADER);
        buf.push(1);
        buf.extend_from_slice(&self.digest);
        buf.extend_from_slice(&(self.trace.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(self.constraints.len() as u32).to_le_bytes());
        match self.tx_hash {
            Some(hash) => {
                buf.push(1);
                buf.extend_from_slice(&hash);
            }
            None => buf.push(0),
        }
        if let Some(caps) = self.transport_capabilities {
            buf.push(1);
            buf.extend_from_slice(&caps.hpke_suite.suite_id().to_le_bytes());
            buf.push(u8::from(caps.use_datagram));
            buf.extend_from_slice(&caps.max_segment_datagram_size.to_le_bytes());
            buf.extend_from_slice(&caps.fec_feedback_interval_ms.to_le_bytes());
            buf.push(caps.privacy_bucket_granularity as u8);
        } else {
            buf.push(0);
        }
        if let Some(flags) = self.negotiated_capabilities {
            buf.push(1);
            buf.extend_from_slice(&flags.bits().to_le_bytes());
        } else {
            buf.push(0);
        }
        buf
    }

    fn into_snapshot(self) -> PipelineProofSnapshot {
        PipelineProofSnapshot {
            backend: TRACE_MOCK_BACKEND.to_string(),
            proof: self.encode_mock_proof_bytes(),
            code_hash: self.code_hash,
            tx_hash: self.tx_hash,
        }
    }
}

#[cfg(feature = "zk-preverify")]
static TRACE_PROVING_QUEUE: OnceLock<Mutex<BTreeMap<u64, Vec<TraceForProving>>>> = OnceLock::new();

#[cfg(feature = "zk-preverify")]
fn trace_proving_queue() -> &'static Mutex<BTreeMap<u64, Vec<TraceForProving>>> {
    TRACE_PROVING_QUEUE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[cfg(feature = "zk-preverify")]
/// Persist a proving job until the background lane receives the matching block header.
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

/// Start the background ZK proving lane that enriches blocks with mock proofs.
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
                let circuit = VMExecutionCircuit::new(
                    entry.program.as_slice(),
                    &entry.trace,
                    &entry.constraints,
                );
                match circuit.verify() {
                    Ok(()) => {
                        let digest = entry.digest;
                        let trace_len = entry.trace.len();
                        let constraint_len = entry.constraints.len();
                        let tx_hash_hex = entry
                            .tx_hash
                            .map(|bytes| hex::encode(bytes))
                            .unwrap_or_else(|| "none".to_string());
                        let artifact = entry.into_snapshot();
                        queue_trace_proof(height, artifact);
                        iroha_logger::info!(
                            height,
                            %tx_hash_hex,
                            digest = %hex::encode(digest),
                            trace_len,
                            constraint_len,
                            "zk_lane: generated mock proof for block trace"
                        );
                    }
                    Err(err) => {
                        iroha_logger::warn!(
                            height,
                            error = err,
                            "zk_lane: failed to validate trace prior to proof emission"
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
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn halo2_verify_with_instance_noncanonical_ipa() {
    // Generate a valid proof, then wrap a non-canonical instance scalar in ZK1.
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{
            Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
        },
        poly::{Rotation, commitment::Params as _},
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a",
                        a,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b",
                        b,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c",
                        c,
                        0,
                        || Value::known(Scalar::from(4)),
                    )?;
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
    assert!(!verify_backend(backend, &prf_box, Some(&vk_box)));
}

#[cfg(all(
    feature = "zk-halo2-ipa",
    feature = "zk-halo2",
    feature = "zk-halo2-ipa-poseidon"
))]
#[test]
fn ipa_vote_bool_commit_zk1() {
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::{
            Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
        },
        poly::Rotation,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

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
    assert!(verify_backend(backend, &prf_box, Some(&vk_box)));
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
        poly::commitment::Params as _,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

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
    assert!(verify_backend(backend, &prf_box, Some(&vk_box_good)));

    // Create VK envelope lacking the H2VK TLV — verification must fail.
    let mut vk_env_missing = crate::zk::zk1::wrap_start();
    crate::zk::zk1::wrap_append_ipa_k(&mut vk_env_missing, k);
    let vk_box_missing = VerifyingKeyBox::new(backend.into(), vk_env_missing);
    assert!(!verify_backend(backend, &prf_box, Some(&vk_box_missing)));

    // Tamper with the VK bytes while keeping the TLV present → hash mismatch → reject.
    let mut vk_tampered = vk_env;
    if let Some(last) = vk_tampered.last_mut() {
        *last ^= 0xAA;
    }
    let vk_box_tampered = VerifyingKeyBox::new(backend.into(), vk_tampered);
    assert!(!verify_backend(backend, &prf_box, Some(&vk_box_tampered)));
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
        poly::commitment::Params as _,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

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

    // Compute toy commitments externally using the same formula
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
        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let r2 = r * r;
        let r4 = r2 * r2;
        let r5 = r4 * r;
        Scalar::from(2) * a5 + Scalar::from(3) * r5 + Scalar::from(7)
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
    assert!(verify_backend(backend, &prf_box, Some(&vk_box)));
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
        poly::commitment::Params as _,
        transcript::{Blake2bWrite, Challenge255},
    };
    use rand_core_06::OsRng;

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

    // Compute commit and merkle2 root using the same inline pow5-like hash as circuit
    let v = Scalar::from(1u64);
    let rho = Scalar::from(12345u64);
    let h2 = |a: Scalar, b: Scalar| {
        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let b2 = b * b;
        let b4 = b2 * b2;
        let b5 = b4 * b;
        Scalar::from(2) * a5 + Scalar::from(3) * b5
    };
    let commit = h2(v + Scalar::from(7u64), rho + Scalar::from(13u64));
    let sib0 = Scalar::from(23u64);
    let sib1 = Scalar::from(29u64);
    // w0 = h(commit+7, sib0+13), w1 = h(sib1+7, w0+13)
    let w0 = h2(commit + Scalar::from(7u64), sib0 + Scalar::from(13u64));
    let root = h2(sib1 + Scalar::from(7u64), w0 + Scalar::from(13u64));

    // Make proof with public instances [commit, root]
    let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
    let insts: [&[&[Scalar]]; 1] = [&[&[commit, root]]];
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
        &insts,
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
    let cols: [&[Scalar]; 2] = [&[commit][..], &[root][..]];
    crate::zk::zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut prf_env);

    let backend = "halo2/pasta/ipa/vote-bool-commit-merkle2";
    let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
    let prf_box = ProofBox::new(backend.into(), prf_env);
    assert!(verify_backend(backend, &prf_box, Some(&vk_box)));
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
/// Current implementation only performs deduplication and backend tag sanity checks.
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
    if !dedup.check_and_insert_with_commitment(proof, vk_commitment) {
        return PreverifyResult::Duplicate;
    }
    PreverifyResult::Accepted
}

/// Minimal verifier selector. Dispatches to concrete verifiers behind feature flags.
///
/// Backends without a real verifier return `false` (unsupported).
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
fn halo2_ipa_backend_from_circuit_id(circuit_id: &str) -> Option<String> {
    let trimmed = circuit_id.trim();
    if trimmed.is_empty() {
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
    let env: OpenVerifyEnvelope = match norito::decode_from_bytes(&proof.bytes) {
        Ok(env) => env,
        Err(_) => return false,
    };
    if env.backend != BackendTag::Halo2IpaPasta {
        return false;
    }
    let backend = match halo2_ipa_backend_from_circuit_id(&env.circuit_id) {
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
    use crate::zk_stark::{STARK_HASH_POSEIDON2_V1, STARK_HASH_SHA256_V1, StarkFriVerifyingKeyV1};
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope, StarkFriOpenProofV1};

    let reject = |reason: &'static str| {
        iroha_logger::debug!(
            backend,
            reason,
            "stark/fri proof rejected (metadata/integrity check failed)"
        );
        false
    };

    let env: OpenVerifyEnvelope = match norito::decode_from_bytes(&proof.bytes) {
        Ok(env) => env,
        Err(_) => return reject("invalid OpenVerifyEnvelope payload"),
    };
    if env.backend != BackendTag::Stark {
        return reject("unexpected OpenVerifyEnvelope backend tag");
    }

    let Some(vk_box) = vk else {
        return reject("missing verifying key");
    };
    let expected_vk_hash = hash_vk(vk_box);
    if env.vk_hash == [0u8; 32] || env.vk_hash != expected_vk_hash {
        return reject("verifying key commitment mismatch");
    }

    let expected_hash_fn = if backend == ZK_BACKEND_STARK_FRI_V1 {
        None
    } else if backend.contains("/sha256-") {
        Some(STARK_HASH_SHA256_V1)
    } else if backend.contains("/poseidon2-") {
        Some(STARK_HASH_POSEIDON2_V1)
    } else {
        return reject("unsupported stark/fri backend variant");
    };

    // Decode and validate the STARK verifying key payload. This pins the verifier
    // parameters so a prover cannot select weaker hash/query settings at runtime.
    let vk_payload: StarkFriVerifyingKeyV1 = match norito::decode_from_bytes(&vk_box.bytes) {
        Ok(vk) => vk,
        Err(_) => return reject("invalid STARK verifying key payload"),
    };
    if vk_payload.version != 1 {
        return reject("unsupported STARK verifying key payload version");
    }
    if vk_payload.hash_fn != STARK_HASH_SHA256_V1 && vk_payload.hash_fn != STARK_HASH_POSEIDON2_V1 {
        return reject("unsupported STARK verifying key hash_fn");
    }
    if let Some(expected_hash_fn) = expected_hash_fn {
        if vk_payload.hash_fn != expected_hash_fn {
            return reject("STARK verifying key hash_fn mismatch");
        }
    }
    let env_circuit_id = match normalize_stark_fri_circuit_id_for_backend(backend, &env.circuit_id)
    {
        Some(id) => id,
        None => return reject("invalid STARK envelope circuit_id"),
    };
    let vk_circuit_id =
        match normalize_stark_fri_circuit_id_for_backend(backend, &vk_payload.circuit_id) {
            Some(id) => id,
            None => return reject("invalid STARK verifying key circuit_id"),
        };
    if env_circuit_id != vk_circuit_id {
        return reject("STARK verifying key circuit_id mismatch");
    }

    // Decode the STARK wrapper payload.
    let open: StarkFriOpenProofV1 = match norito::decode_from_bytes(&env.proof_bytes) {
        Ok(open) => open,
        Err(_) => return reject("invalid STARK wrapper payload"),
    };
    if open.version != 1 {
        return reject("unsupported STARK wrapper version");
    }
    if open.envelope_bytes.len() > limits.max_envelope_bytes {
        return reject("inner STARK envelope exceeds verifier limits");
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
        match norito::decode_from_bytes(&open.envelope_bytes) {
            Ok(inner) => inner,
            Err(_) => return reject("invalid inner STARK envelope payload"),
        };
    // Verify that the prover is using the parameters pinned by the verifying key.
    if inner.params.hash_fn != vk_payload.hash_fn
        || inner.params.n_log2 != vk_payload.n_log2
        || inner.params.blowup_log2 != vk_payload.blowup_log2
        || inner.params.fold_arity != vk_payload.fold_arity
        || inner.params.queries != vk_payload.queries
        || inner.params.merkle_arity != vk_payload.merkle_arity
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
    if !crate::zk_stark::verify_stark_fri_envelope_with_limits(&open.envelope_bytes, limits) {
        return reject("inner STARK/FRI verifier rejected proof");
    }
    true
}

/// Verify a zero-knowledge proof using the requested backend, returning `true` when supported.
pub fn verify_backend(backend: &str, proof: &ProofBox, vk: Option<&VerifyingKeyBox>) -> bool {
    // Prefer built-in registry when available
    if let Some(ok) = verify_with_registry(backend, proof, vk) {
        return ok;
    }

    if backend == ZK_BACKEND_HALO2_IPA {
        #[cfg(feature = "zk-halo2-ipa")]
        {
            return verify_halo2_ipa_envelope(proof, vk);
        }
        #[cfg(not(feature = "zk-halo2-ipa"))]
        {
            return false;
        }
    }

    // Native IPA polynomial-open verifier (transparent, no external libs)
    // Backend tag: "halo2/ipa/poly-open" with proof bytes = Norito `OpenVerifyEnvelope`.
    #[cfg(feature = "zk-ipa-native")]
    if backend == "halo2/ipa/poly-open" {
        return verify_ipa_open_envelope(proof);
    }

    // Halo2 family (external halo2 backends)
    if backend.starts_with("halo2/") {
        // Transparent IPA over Pasta (default-on)
        #[cfg(feature = "zk-halo2-ipa")]
        if backend.starts_with("halo2/pasta/ipa-") {
            return verify_halo2_ipa(backend, proof, vk);
        }
        // Pasta and other built-ins handled here
        return verify_halo2(backend, proof, vk);
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
    fn debug_backends_are_unsupported() {
        for backend in ["debug/ok", "debug/reject", "debug/sleep"] {
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
        }
    }
}

#[cfg(test)]
mod stark_backend_tag_tests {
    use super::{is_ivm_execution_backend, is_stark_fri_v1_backend};

    #[test]
    fn detects_base_and_variant_backends() {
        assert!(is_stark_fri_v1_backend("stark/fri"));
        assert!(is_stark_fri_v1_backend("stark/fri/sha256-goldilocks"));
        assert!(is_stark_fri_v1_backend("stark/fri/poseidon2-goldilocks"));
        assert!(!is_stark_fri_v1_backend("stark/fri-v2"));
        assert!(!is_stark_fri_v1_backend("stark/fri-v10"));
    }

    #[test]
    fn ivm_execution_backend_allowlist_is_explicit() {
        assert!(is_ivm_execution_backend("halo2/ipa"));
        assert!(is_ivm_execution_backend("stark/fri"));
        assert!(is_ivm_execution_backend("stark/fri/sha256-goldilocks"));
        assert!(!is_ivm_execution_backend("groth16/bn254"));
        assert!(!is_ivm_execution_backend("halo2/kzg"));
    }
}

#[cfg(all(test, feature = "zk-stark"))]
mod stark_prover_tests {
    use super::{
        prove_stark_fri_ivm_execution_envelope, prove_stark_fri_open_verify_envelope,
        verify_backend_with_timing,
    };
    use crate::zk_stark::{STARK_HASH_SHA256_V1, StarkFriVerifyingKeyV1, StarkVerifyEnvelopeV1};
    use iroha_crypto::Hash;
    use iroha_data_model::proof::{ProofBox, VerifyingKeyBox};
    use iroha_data_model::zk::{OpenVerifyEnvelope, StarkFriOpenProofV1};

    fn sample_stark_open_verify_proof() -> (&'static str, String, VerifyingKeyBox, ProofBox) {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:tiny-open");
        let vk_payload = StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.clone(),
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
        };
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

    #[test]
    fn prove_stark_open_verify_envelope_emits_binding_air_proof() {
        let (backend, _circuit_id, vk_box, proof) = sample_stark_open_verify_proof();
        let report = verify_backend_with_timing(backend, &proof, Some(&vk_box));
        assert!(report.ok);
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
    fn prove_stark_ivm_execution_envelope_emits_binding_air_proof() {
        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:ivm-execution-v1");
        let vk_payload = StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.clone(),
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
        };
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
    if backend == ZK_BACKEND_HALO2_IPA || backend.starts_with("halo2/") {
        if !guardrails.halo2_enabled {
            iroha_logger::debug!(
                backend,
                "halo2 verification is disabled in node configuration"
            );
            return VerifyReport {
                ok: false,
                elapsed: Duration::ZERO,
            };
        }
        if proof.bytes.len() > guardrails.halo2_max_envelope_bytes {
            iroha_logger::debug!(
                backend,
                "halo2 payload exceeds node-configured max_envelope_bytes"
            );
            return VerifyReport {
                ok: false,
                elapsed: Duration::ZERO,
            };
        }

        // Best-effort: when the payload is a Norito `OpenVerifyEnvelope`, enforce the inner
        // proof-bytes cap. (Some halo2 backends may accept raw proof bytes, so decode failure
        // is not treated as a hard reject here.)
        if let Ok(env) =
            norito::decode_from_bytes::<iroha_data_model::zk::OpenVerifyEnvelope>(&proof.bytes)
            && env.proof_bytes.len() > guardrails.halo2_max_proof_bytes
        {
            iroha_logger::debug!(
                backend,
                "halo2 envelope proof_bytes exceeds node-configured max_proof_bytes"
            );
            return VerifyReport {
                ok: false,
                elapsed: Duration::ZERO,
            };
        }
    }

    if is_stark_fri_v1_backend(backend) {
        if !guardrails.stark_enabled {
            iroha_logger::debug!(
                backend,
                "stark verification is disabled in node configuration"
            );
            return VerifyReport {
                ok: false,
                elapsed: Duration::ZERO,
            };
        }
        if proof.bytes.len() > guardrails.stark_max_envelope_bytes {
            iroha_logger::debug!(
                backend,
                "stark payload exceeds node-configured max_envelope_bytes"
            );
            return VerifyReport {
                ok: false,
                elapsed: Duration::ZERO,
            };
        }
        if let Ok(env) =
            norito::decode_from_bytes::<iroha_data_model::zk::OpenVerifyEnvelope>(&proof.bytes)
            && let Ok(open) = norito::decode_from_bytes::<iroha_data_model::zk::StarkFriOpenProofV1>(
                &env.proof_bytes,
            )
            && open.envelope_bytes.len() > guardrails.stark_max_proof_bytes
        {
            iroha_logger::debug!(
                backend,
                "stark envelope proof bytes exceed node-configured max_proof_bytes"
            );
            return VerifyReport {
                ok: false,
                elapsed: Duration::ZERO,
            };
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
            return VerifyReport {
                ok: false,
                elapsed: Duration::ZERO,
            };
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

    #[test]
    fn guardrails_disable_halo2_returns_zero_duration() {
        let proof = ProofBox::new("halo2/ipa".into(), vec![0xAA; 8]);
        let report = verify_backend_with_timing_guardrails(
            "halo2/ipa",
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_enabled: false,
                halo2_max_envelope_bytes: 1024,
                halo2_max_proof_bytes: 1024,
                stark_enabled: true,
                stark_max_envelope_bytes: 1024,
                stark_max_proof_bytes: 1024,
            },
        );
        assert!(!report.ok);
        assert_eq!(report.elapsed, Duration::ZERO);
    }

    #[test]
    fn guardrails_enforce_halo2_max_envelope_bytes() {
        let proof = ProofBox::new("halo2/ipa".into(), vec![0xAA; 9]);
        let report = verify_backend_with_timing_guardrails(
            "halo2/ipa",
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_enabled: true,
                halo2_max_envelope_bytes: 8,
                halo2_max_proof_bytes: 1024,
                stark_enabled: true,
                stark_max_envelope_bytes: 1024,
                stark_max_proof_bytes: 1024,
            },
        );
        assert!(!report.ok);
        assert_eq!(report.elapsed, Duration::ZERO);
    }

    #[test]
    fn guardrails_enforce_halo2_max_proof_bytes_for_open_verify_envelopes() {
        let env = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:dummy".to_owned(),
            vk_hash: [0u8; 32],
            public_inputs: Vec::new(),
            proof_bytes: vec![0xBB; 10],
            aux: Vec::new(),
        };
        let bytes = norito::to_bytes(&env).expect("encode envelope");
        let proof = ProofBox::new("halo2/ipa".into(), bytes);
        let report = verify_backend_with_timing_guardrails(
            "halo2/ipa",
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_enabled: true,
                halo2_max_envelope_bytes: 1024,
                halo2_max_proof_bytes: 5,
                stark_enabled: true,
                stark_max_envelope_bytes: 1024,
                stark_max_proof_bytes: 1024,
            },
        );
        assert!(!report.ok);
        assert_eq!(report.elapsed, Duration::ZERO);
    }

    #[test]
    fn guardrails_disable_stark_returns_zero_duration() {
        let proof = ProofBox::new(ZK_BACKEND_STARK_FRI_V1.into(), vec![0xAA; 8]);
        let report = verify_backend_with_timing_guardrails(
            ZK_BACKEND_STARK_FRI_V1,
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_enabled: true,
                halo2_max_envelope_bytes: 1024,
                halo2_max_proof_bytes: 1024,
                stark_enabled: false,
                stark_max_envelope_bytes: 1024,
                stark_max_proof_bytes: 1024,
            },
        );
        assert!(!report.ok);
        assert_eq!(report.elapsed, Duration::ZERO);
    }

    #[test]
    fn guardrails_enforce_stark_max_envelope_bytes() {
        let proof = ProofBox::new(ZK_BACKEND_STARK_FRI_V1.into(), vec![0xAA; 9]);
        let report = verify_backend_with_timing_guardrails(
            ZK_BACKEND_STARK_FRI_V1,
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_enabled: true,
                halo2_max_envelope_bytes: 1024,
                halo2_max_proof_bytes: 1024,
                stark_enabled: true,
                stark_max_envelope_bytes: 8,
                stark_max_proof_bytes: 1024,
            },
        );
        assert!(!report.ok);
        assert_eq!(report.elapsed, Duration::ZERO);
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
        let report = verify_backend_with_timing_guardrails(
            ZK_BACKEND_STARK_FRI_V1,
            &proof,
            None,
            ZkVerifyGuardrails {
                halo2_enabled: true,
                halo2_max_envelope_bytes: 1024,
                halo2_max_proof_bytes: 1024,
                stark_enabled: true,
                stark_max_envelope_bytes: 1024,
                stark_max_proof_bytes: 8,
            },
        );
        assert!(!report.ok);
        assert_eq!(report.elapsed, Duration::ZERO);
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn guardrails_stark_proof_limit_applies_to_inner_envelope_not_outer_wrapper() {
        use crate::zk_stark::{STARK_HASH_SHA256_V1, StarkFriVerifyingKeyV1};

        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = format!("{backend}:guardrail-split");
        let vk_payload = StarkFriVerifyingKeyV1 {
            version: 1,
            circuit_id: circuit_id.clone(),
            n_log2: 4,
            blowup_log2: 2,
            fold_arity: 2,
            queries: 2,
            merkle_arity: 2,
            hash_fn: STARK_HASH_SHA256_V1,
        };
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
                halo2_enabled: true,
                halo2_max_envelope_bytes: 1024,
                halo2_max_proof_bytes: 1024,
                stark_enabled: true,
                stark_max_envelope_bytes: proof.bytes.len(),
                stark_max_proof_bytes: open.envelope_bytes.len(),
            },
        );
        assert!(report.ok);
    }
}

#[cfg(all(test, any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
mod halo2_ipa_alias_tests {
    use iroha_data_model::zk::{BackendTag, OpenVerifyEnvelope};

    use super::*;

    #[test]
    fn halo2_ipa_circuit_id_maps_to_pasta_backend() {
        assert_eq!(
            halo2_ipa_backend_from_circuit_id("halo2/ipa:tiny-add").as_deref(),
            Some("halo2/pasta/ipa/tiny-add")
        );
        assert_eq!(
            halo2_ipa_backend_from_circuit_id("halo2/pasta/tiny-add").as_deref(),
            Some("halo2/pasta/ipa/tiny-add")
        );
        assert_eq!(
            halo2_ipa_backend_from_circuit_id("halo2/pasta/ipa/tiny-add").as_deref(),
            Some("halo2/pasta/ipa/tiny-add")
        );
        assert_eq!(
            halo2_ipa_backend_from_circuit_id("tiny-add").as_deref(),
            Some("halo2/pasta/ipa/tiny-add")
        );
        assert!(halo2_ipa_backend_from_circuit_id("").is_none());
    }

    #[test]
    fn halo2_ipa_rejects_missing_vk() {
        let env = OpenVerifyEnvelope {
            backend: BackendTag::Halo2IpaPasta,
            circuit_id: "halo2/ipa:tiny-add".into(),
            vk_hash: [0u8; 32],
            public_inputs: Vec::new(),
            proof_bytes: vec![0xAA, 0xBB],
            aux: Vec::new(),
        };
        let proof_bytes = norito::to_bytes(&env).expect("encode envelope");
        let proof = ProofBox::new("halo2/ipa".into(), proof_bytes);
        assert!(!verify_backend("halo2/ipa", &proof, None));
    }
}

#[cfg(all(test, feature = "zk-halo2-ipa"))]
mod offline_note_real_prover_tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
    use iroha_data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        domain::DomainId,
        offline::{
            OfflineNoteAuditBundle, OfflineNoteAuditOutputClaim, OfflineNoteIssue,
            OfflineNoteIssuedClaim, OfflineNoteKeyCertificate, OfflineNoteRecursiveProof,
            OfflineNoteRedeem,
        },
        proof::{ProofBox, VerifyingKeyBox, VerifyingKeyId},
        zk::OpenVerifyEnvelope,
    };
    use iroha_primitives::numeric::Numeric;

    fn offline_note_vk_box() -> VerifyingKeyBox {
        offline_note_recursive_vk_box().expect("offline note verifying key")
    }

    fn sample_signature(seed: u8) -> Signature {
        let mut bytes = [0u8; 64];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = seed.wrapping_add(u8::try_from(index).expect("signature index fits"));
        }
        Signature::from_bytes(&bytes)
    }

    fn sample_account(seed: u8) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    fn sample_asset(account: AccountId) -> AssetId {
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("domain id"),
            "xor".parse().expect("asset definition name"),
        );
        AssetId::new(definition, account)
    }

    fn sample_certificate(account: &AccountId, seed: u8) -> OfflineNoteKeyCertificate {
        let note_keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let (_algorithm, public_key) = note_keypair.public_key().to_bytes();
        OfflineNoteKeyCertificate {
            version: 2,
            platform: "ios-appattest".to_owned(),
            key_id: format!("one-use-key-{seed}"),
            device_id: "device-1".to_owned(),
            account_id: account.clone(),
            public_key: public_key.to_vec(),
            assertion_scheme: "apple-appattest-counter".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: sample_signature(seed.wrapping_add(1)),
        }
    }

    fn placeholder_recursive_proof() -> OfflineNoteRecursiveProof {
        OfflineNoteRecursiveProof {
            verifier_key_id: VerifyingKeyId::new(
                ZK_BACKEND_HALO2_IPA,
                OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            ),
            public_inputs_hash: Hash::new(b"placeholder-offline-note-public-inputs"),
            proof: ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new()),
        }
    }

    fn sample_redemption() -> OfflineNoteRedeem {
        let account = sample_account(0xA1);
        let asset = sample_asset(account.clone());
        OfflineNoteRedeem {
            source_note_commitment: Hash::new(b"offline-note-source-note"),
            input_nullifiers: vec![Hash::new(b"offline-note-redeem-nullifier")],
            sender_key_certificate: sample_certificate(&account, 0xB1),
            recipient: account,
            asset,
            amount: Numeric::new(10, 0),
            recursive_proof: placeholder_recursive_proof(),
        }
    }

    fn sample_audit() -> OfflineNoteAuditBundle {
        let account = sample_account(0xC1);
        let asset = sample_asset(account.clone());
        let certificate = sample_certificate(&account, 0xD1);
        let issue = OfflineNoteIssue {
            note_commitment: Hash::new(b"offline-note-audit-input-note"),
            key_certificate: certificate.clone(),
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
        };
        OfflineNoteAuditBundle {
            token_id: Hash::new(b"offline-note-audit-token"),
            sender_key_certificate: certificate.clone(),
            input_nullifiers: vec![Hash::new(b"offline-note-audit-nullifier")],
            input_claims: vec![OfflineNoteIssuedClaim::from_issue(&issue).expect("input claim")],
            output_commitments: vec![Hash::new(b"offline-note-audit-output-note")],
            output_claims: vec![OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"offline-note-audit-output-note"),
                key_certificate: certificate,
                asset,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: placeholder_recursive_proof(),
        }
    }

    fn envelope_instances(proof: &ProofBox) -> Vec<Vec<[u8; 32]>> {
        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("OpenVerifyEnvelope");
        extract_pasta_instance_columns_bytes(&envelope.proof_bytes).expect("public instances")
    }

    #[test]
    fn swift_native_offline_note_proof_verifies_against_rust() {
        let Some(path) = std::env::var_os("SWIFT_OFFLINE_PROOF") else {
            return;
        };
        let bytes = std::fs::read(path).expect("swift proof artifact");
        let proof = ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), bytes);
        let vk_box = offline_note_vk_box();
        assert!(verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk_box)));
    }

    #[test]
    fn prove_offline_note_redeem_emits_real_halo2_ipa_proof() {
        let vk_box = offline_note_vk_box();
        let proving_key = derive_halo2_ipa_offline_note_proving_key_bytes(&vk_box)
            .expect("offline note proving key");
        let redemption = sample_redemption();
        let proof = prove_offline_note_redeem(
            OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            &vk_box,
            &redemption,
            Some(&proving_key),
        )
        .expect("real offline note redemption proof");

        assert!(verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk_box)));
        assert_eq!(
            envelope_instances(&proof),
            offline_note_redeem_instance_values(&redemption)
                .expect("redemption instance values")
                .public_instance_columns()
        );
    }

    #[test]
    fn prove_offline_note_audit_emits_real_halo2_ipa_proof() {
        let vk_box = offline_note_vk_box();
        let audit = sample_audit();
        let proof =
            prove_offline_note_audit(OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID, &vk_box, &audit, None)
                .expect("real offline note audit proof");

        assert!(verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk_box)));
        assert_eq!(
            envelope_instances(&proof),
            offline_note_audit_instance_values(&audit)
                .expect("audit instance values")
                .public_instance_columns()
        );
    }

    #[test]
    fn offline_note_real_proof_rejects_tampered_bytes() {
        let vk_box = offline_note_vk_box();
        let redemption = sample_redemption();
        let mut proof = prove_offline_note_redeem(
            OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            &vk_box,
            &redemption,
            None,
        )
        .expect("real offline note redemption proof");
        let last = proof.bytes.last_mut().expect("proof bytes");
        *last ^= 0x01;

        assert!(!verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk_box)));
    }

    #[test]
    fn offline_note_vk_record_embeds_real_active_verifier_key() {
        let record = offline_note_recursive_vk_record("offline_note", 7).expect("vk record");
        let vk_box = record.key.as_ref().expect("inline verifier key");

        assert_eq!(record.version, 7);
        assert_eq!(record.circuit_id, OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
        assert_eq!(record.namespace, "offline_note");
        assert!(record.is_active());
        assert_eq!(record.max_proof_bytes, OFFLINE_NOTE_MAX_PROOF_BYTES);
        assert_eq!(record.vk_len as usize, vk_box.bytes.len());
        assert_eq!(record.commitment, hash_vk(vk_box));
        assert_eq!(vk_box.backend, ZK_BACKEND_HALO2_IPA);

        let redemption = sample_redemption();
        let proof = prove_offline_note_redeem(&record.circuit_id, vk_box, &redemption, None)
            .expect("real redeem proof");
        assert!(verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(vk_box)));
    }
}

/// Native IPA polynomial-opening verifier using internal `iroha_zkp_halo2`.
/// Expects proof bytes to be a Norito-encoded `OpenVerifyEnvelope`.
#[cfg(feature = "zk-ipa-native")]
fn verify_ipa_open_envelope(proof: &ProofBox) -> bool {
    #[cfg(feature = "goldilocks_backend")]
    use iroha_zkp_halo2::backend::goldilocks;
    use iroha_zkp_halo2::{
        OpenVerifyEnvelope, Transcript,
        backend::{bn254, pallas},
        norito_helpers::{self as nh, DecodedEnvelope},
    };
    // Decode Norito envelope
    let env: OpenVerifyEnvelope = match norito::decode_from_bytes(&proof.bytes) {
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
        DecodedEnvelope::Goldilocks {
            params,
            proof,
            z,
            t,
            p_g,
        } => goldilocks::Polynomial::verify_open_with_metadata(
            params.as_ref(),
            &mut tr,
            z,
            p_g,
            t,
            proof.as_ref(),
            metadata,
        ),
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
    use std::{
        convert::{TryFrom, TryInto},
        io::{Cursor, Read},
    };

    use halo2_proofs::poly::commitment::Params as _;

    use super::{PastaParams, pasta_params_new};

    fn envelope_cursor(bytes: &[u8]) -> Option<Cursor<&[u8]>> {
        if !super::zk1::is_envelope(bytes) {
            return None;
        }
        if bytes.len() < 4 {
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
    ) -> Option<halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>>
    where
        C: halo2_proofs::plonk::Circuit<halo2_proofs::halo2curves::pasta::Fp>,
        C::Params: Default,
    {
        let mut cursor = envelope_cursor(vk_bytes)?;
        while let Some((tag, payload)) = read_tlv(&mut cursor) {
            if &tag == b"H2VK" {
                let mut payload_cursor = Cursor::new(payload);
                let vk = super::read_verifying_key::<C, _>(&mut payload_cursor).ok()?;
                if vk.get_domain().k() != params.k() {
                    return None;
                }
                return Some(vk);
            }
        }
        None
    }

    /// Parse Params from a VK container carrying an `IPAK` TLV.
    /// Returns `None` if the container is malformed or missing `IPAK`.
    pub fn params_any(vk_bytes: &[u8]) -> Option<PastaParams> {
        let mut cursor = envelope_cursor(vk_bytes)?;
        let mut ipa_k: Option<u32> = None;
        while let Some((tag, payload)) = read_tlv(&mut cursor) {
            if &tag == b"IPAK" && payload.len() == 4 {
                ipa_k = Some(u32::from_le_bytes(payload.try_into().ok()?));
            }
        }
        ipa_k.map(pasta_params_new)
    }

    /// Parse proof payload and optional `I10P` instances from a ZK1 envelope.
    /// Returns `(proof_payload, instance_columns_owning)`.
    pub fn proof_and_instances(
        bytes: &[u8],
    ) -> Option<(Vec<u8>, Vec<Vec<halo2_proofs::halo2curves::pasta::Fp>>)> {
        let mut cursor = envelope_cursor(bytes)?;
        let mut proof_payload: Option<Vec<u8>> = None;
        let mut inst_cols: Vec<Vec<halo2_proofs::halo2curves::pasta::Fp>> = Vec::new();
        while let Some((tag, payload)) = read_tlv(&mut cursor) {
            match &tag {
                b"PROF" => {
                    proof_payload = Some(payload.to_vec());
                }
                b"I10P" => {
                    let mut inner = Cursor::new(payload);
                    let cols = read_u32(&mut inner)? as usize;
                    let rows = read_u32(&mut inner)? as usize;
                    if cols > super::MAX_INST_COLS || rows > super::MAX_INST_ROWS {
                        return None;
                    }
                    let mut columns = vec![Vec::with_capacity(rows); cols];
                    for _ in 0..rows {
                        for column in &mut columns {
                            let mut b32 = [0u8; 32];
                            inner.read_exact(&mut b32).ok()?;
                            let mut repr =
                                <halo2_proofs::halo2curves::pasta::Fp as ff::PrimeField>::Repr::default();
                            repr.as_mut().copy_from_slice(&b32);
                            let val = Option::from(
                                <halo2_proofs::halo2curves::pasta::Fp as ff::PrimeField>::from_repr(
                                    repr,
                                ),
                            )?;
                            column.push(val);
                        }
                    }
                    inst_cols = columns;
                }
                _ => {}
            }
        }
        let payload = proof_payload?;
        Some((payload, inst_cols))
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
    use iroha_zkp_halo2::Halo2ProofEnvelope;

    if let Ok(env) = Halo2ProofEnvelope::from_bytes(proof_bytes) {
        let mut columns = Vec::with_capacity(env.public_inputs.len());
        for input in env.public_inputs {
            columns.push(vec![input]);
        }
        return Some(columns);
    }

    #[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
    {
        use halo2_proofs::halo2curves::ff::PrimeField as _;

        if let Some((_, cols)) = zkparse::proof_and_instances(proof_bytes) {
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
    use iroha_zkp_halo2::Halo2ProofEnvelope;

    if let Ok(env) = Halo2ProofEnvelope::from_bytes(proof_bytes) {
        let mut columns = Vec::with_capacity(env.public_inputs.len());
        for chunk in &env.public_inputs {
            let mut repr =
                <halo2_proofs::halo2curves::pasta::Fp as halo2_proofs::halo2curves::ff::PrimeField>::Repr::default();
            repr.as_mut().copy_from_slice(chunk);
            let scalar = Option::from(
                <halo2_proofs::halo2curves::pasta::Fp as halo2_proofs::halo2curves::ff::PrimeField>::from_repr(repr),
            )?;
            columns.push(vec![scalar]);
        }
        return Some(columns);
    }

    zkparse::proof_and_instances(proof_bytes).map(|(_, cols)| cols)
}

#[cfg(not(any(feature = "zk-halo2", feature = "zk-halo2-ipa")))]
fn extract_pasta_fp_instances_impl(
    _proof_bytes: &[u8],
) -> Option<Vec<Vec<halo2_proofs::halo2curves::pasta::Fp>>> {
    None
}

// Tiny pasta circuits used for dispatch verification across KZG and IPA
#[cfg(any(feature = "zk-halo2", feature = "zk-halo2-ipa"))]
mod pasta_tiny {
    use halo2_proofs::{
        circuit::{Layouter, SimpleFloorPlanner, Value},
        halo2curves::pasta::Fp as Scalar,
        plonk::{Circuit, ConstraintSystem, Error as PlonkError, Expression, Selector},
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a",
                        a,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b",
                        b,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c",
                        c,
                        0,
                        || Value::known(Scalar::from(4)),
                    )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a",
                        a,
                        0,
                        || Value::known(Scalar::from(3)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b",
                        b,
                        0,
                        || Value::known(Scalar::from(3)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c",
                        c,
                        0,
                        || Value::known(Scalar::from(9)),
                    )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a",
                        a,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b",
                        b,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c",
                        c,
                        0,
                        || Value::known(Scalar::from(4)),
                    )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a",
                        a,
                        0,
                        || Value::known(Scalar::from(3)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b",
                        b,
                        0,
                        || Value::known(Scalar::from(3)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c",
                        c,
                        0,
                        || Value::known(Scalar::from(9)),
                    )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c",
                        c,
                        0,
                        || Value::known(Scalar::from(7)),
                    )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a0",
                        a,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b0",
                        b,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c0",
                        c,
                        0,
                        || Value::known(Scalar::from(4)),
                    )?;
                    // Row 1: 5 + 7 = 12
                    s.enable(&mut region, 1)?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a1",
                        a,
                        1,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b1",
                        b,
                        1,
                        || Value::known(Scalar::from(7)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c1",
                        c,
                        1,
                        || Value::known(Scalar::from(12)),
                    )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a",
                        a,
                        0,
                        || Value::known(Scalar::from(1)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b",
                        b,
                        0,
                        || Value::known(Scalar::from(2)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "d",
                        d,
                        0,
                        || Value::known(Scalar::from(3)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c",
                        c,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "a",
                        a,
                        0,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "b",
                        b,
                        0,
                        || Value::known(Scalar::from(8)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "c",
                        c,
                        0,
                        || Value::known(Scalar::from(13)),
                    )?;
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("a{i}"),
                            *column,
                            0,
                            || Value::known(values[i]),
                        )?;
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("a{i}"),
                            *column,
                            0,
                            || Value::known(values[i]),
                        )?;
                    }
                    Ok(())
                },
            )
        }
    }

    /// Real semantic circuit for Offline recursive note proofs.
    ///
    /// The circuit binds all public instance columns and proves the bounded
    /// note-transfer corridor used by the runtime: mode is redeem/audit,
    /// input/output counts are in range, unused amount slots are zero, and
    /// normalized input/output amount sums are conserved.
    #[derive(Clone)]
    pub struct OfflineNoteSemantic {
        /// Public instance values constrained to equal the proof envelope.
        pub public_values: [Scalar; super::OFFLINE_NOTE_INSTANCE_COLUMNS],
        /// Private normalized input amount slots.
        pub input_amounts: [Scalar; super::OFFLINE_NOTE_MAX_INPUT_AMOUNTS],
        /// Private normalized output amount slots.
        pub output_amounts: [Scalar; super::OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS],
    }

    impl Default for OfflineNoteSemantic {
        fn default() -> Self {
            Self {
                public_values: [Scalar::from(0); super::OFFLINE_NOTE_INSTANCE_COLUMNS],
                input_amounts: [Scalar::from(0); super::OFFLINE_NOTE_MAX_INPUT_AMOUNTS],
                output_amounts: [Scalar::from(0); super::OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS],
            }
        }
    }

    impl Circuit<Scalar> for OfflineNoteSemantic {
        type Config = (
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>;
                super::OFFLINE_NOTE_INSTANCE_COLUMNS],
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>;
                super::OFFLINE_NOTE_MAX_INPUT_AMOUNTS],
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>;
                super::OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS],
            [halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>;
                super::OFFLINE_NOTE_INSTANCE_COLUMNS],
            Selector,
        );
        type FloorPlanner = SimpleFloorPlanner;

        type Params = ();
        fn without_witnesses(&self) -> Self {
            Self::default()
        }
        fn configure(meta: &mut ConstraintSystem<Scalar>) -> Self::Config {
            meta.set_minimum_degree(6);
            let public_adv = std::array::from_fn(|_| meta.advice_column());
            let input_adv = std::array::from_fn(|_| meta.advice_column());
            let output_adv = std::array::from_fn(|_| meta.advice_column());
            let inst = std::array::from_fn(|_| meta.instance_column());
            let s = meta.selector();
            meta.create_gate("offline_note_semantic", |meta| {
                let s = meta.query_selector(s);
                let constant = |value: u64| Expression::Constant(Scalar::from(value));
                let mut public = Vec::with_capacity(super::OFFLINE_NOTE_INSTANCE_COLUMNS);
                for column in &public_adv {
                    public.push(meta.query_advice(*column, Rotation::cur()));
                }
                let mut inputs = Vec::with_capacity(super::OFFLINE_NOTE_MAX_INPUT_AMOUNTS);
                for column in &input_adv {
                    inputs.push(meta.query_advice(*column, Rotation::cur()));
                }
                let mut outputs = Vec::with_capacity(super::OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS);
                for column in &output_adv {
                    outputs.push(meta.query_advice(*column, Rotation::cur()));
                }

                let mode = public[4].clone();
                let input_count = public[5].clone();
                let output_count = public[6].clone();
                let input_sum_public = public[7].clone();
                let output_sum_public = public[8].clone();

                let mut cons = Vec::with_capacity(30);
                for i in 0..super::OFFLINE_NOTE_INSTANCE_COLUMNS {
                    let instance = meta.query_instance(inst[i], Rotation::cur());
                    cons.push(s.clone() * (public[i].clone() - instance));
                }

                cons.push(
                    s.clone()
                        * (mode.clone() - constant(super::OFFLINE_NOTE_MODE_REDEEM))
                        * (mode - constant(super::OFFLINE_NOTE_MODE_AUDIT)),
                );
                cons.push(
                    s.clone()
                        * (input_count.clone() - constant(1))
                        * (input_count.clone() - constant(2))
                        * (input_count.clone() - constant(3))
                        * (input_count.clone() - constant(4)),
                );
                cons.push(
                    s.clone()
                        * (output_count.clone() - constant(1))
                        * (output_count.clone() - constant(2)),
                );
                cons.push(
                    s.clone()
                        * (public[4].clone() - constant(super::OFFLINE_NOTE_MODE_AUDIT))
                        * (output_count.clone() - constant(1)),
                );

                let input_sum_private =
                    inputs[0].clone() + inputs[1].clone() + inputs[2].clone() + inputs[3].clone();
                let output_sum_private = outputs[0].clone() + outputs[1].clone();
                cons.push(s.clone() * (input_sum_private - input_sum_public.clone()));
                cons.push(s.clone() * (output_sum_private - output_sum_public.clone()));
                cons.push(s.clone() * (input_sum_public - output_sum_public));

                cons.push(
                    s.clone()
                        * inputs[1].clone()
                        * (input_count.clone() - constant(2))
                        * (input_count.clone() - constant(3))
                        * (input_count.clone() - constant(4)),
                );
                cons.push(
                    s.clone()
                        * inputs[2].clone()
                        * (input_count.clone() - constant(3))
                        * (input_count.clone() - constant(4)),
                );
                cons.push(s.clone() * inputs[3].clone() * (input_count - constant(4)));
                cons.push(s * outputs[1].clone() * (output_count - constant(2)));
                cons
            });
            (public_adv, input_adv, output_adv, inst, s)
        }
        fn synthesize(
            &self,
            (public_adv, input_adv, output_adv, _inst, s): Self::Config,
            mut layouter: impl Layouter<Scalar>,
        ) -> Result<(), PlonkError> {
            let public_values = self.public_values;
            let input_amounts = self.input_amounts;
            let output_amounts = self.output_amounts;
            layouter.assign_region(
                || "offline_note_semantic",
                |mut region| {
                    s.enable(&mut region, 0)?;
                    for (i, column) in public_adv.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("public{i}"),
                            *column,
                            0,
                            || Value::known(public_values[i]),
                        )?;
                    }
                    for (i, column) in input_adv.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("input_amount{i}"),
                            *column,
                            0,
                            || Value::known(input_amounts[i]),
                        )?;
                    }
                    for (i, column) in output_adv.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("output_amount{i}"),
                            *column,
                            0,
                            || Value::known(output_amounts[i]),
                        )?;
                    }
                    Ok(())
                },
            )
        }
    }

    #[derive(Clone, Default)]
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in0",
                        in0,
                        0,
                        || Value::known(Scalar::from(7)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in1",
                        in1,
                        0,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out0",
                        out0,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out1",
                        out1,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "v",
                        v,
                        0,
                        || Value::known(Scalar::from(1u64)),
                    )?;
                    Ok(())
                },
            )
        }
    }

    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
    #[derive(Clone, Default)]
    pub struct CommitOpen; // commitment = Poseidon2(m, r) fallback when Poseidon gadgets disabled
    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
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
                vec![s * (m + r - c)]
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "m",
                        m,
                        0,
                        || Value::known(Scalar::from(11)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r",
                        r,
                        0,
                        || Value::known(Scalar::from(31)),
                    )?;
                    Ok(())
                },
            )
        }
    }

    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
    #[derive(Clone, Default)]
    pub struct Merkle2; // root = Poseidon2(Poseidon2(leaf, sib0), sib1) fallback when Poseidon gadgets disabled
    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
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
                    s.clone() * (w0.clone() - (leaf + sib0)),
                    s.clone() * (w1.clone() - (w0 + sib1)),
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
                    let w0v = l + s0; // placeholder hash
                    let w1v = w0v + s1; // placeholder hash
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "leaf",
                        leaf,
                        0,
                        || Value::known(l),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sib0",
                        sib0,
                        0,
                        || Value::known(s0),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sib1",
                        sib1,
                        0,
                        || Value::known(s1),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "w0",
                        w0,
                        0,
                        || Value::known(w0v),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "w1",
                        w1,
                        0,
                        || Value::known(w1v),
                    )?;
                    Ok(())
                },
            )
        }
    }

    // Real Poseidon-like gadgets (x^5 S-box, small MDS) implemented directly
    // Note: This is a self-contained permutation suitable for proofs without trusted setup.
    // It is independent from external poseidon crates to keep determinism and avoid version drift.
    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    pub mod poseidon {
        #[cfg(feature = "zk-halo2-ipa-poseidon")]
        use halo2_gadgets::poseidon::primitives::P128Pow5T3;
        #[cfg(feature = "zk-halo2-ipa-poseidon")]
        pub use halo2_gadgets::poseidon::{
            Hash as PoseidonHash, Pow5Chip, Pow5Config, primitives::ConstantLength,
        };
        use halo2_proofs::{
            circuit::{AssignedCell, Layouter, Region, Value},
            plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector},
            poly::Rotation,
        };

        use super::*;

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

        // Two-element state, 2 full rounds, x^5 S-box
        fn sbox5<E: std::ops::Mul<Output = E> + Copy>(x: E) -> E {
            // x^5 = x * x^2 * x^2
            // We'll build constraints in-gate; this helper is only for witness calc.
            x
        }

        /// Minimal wrapper intended to be swapped with halo2-ecc Poseidon gadget.
        /// For now, it assigns the compressor output using the native helper to a target
        /// advice column. This preserves circuit layout and determinism.
        #[derive(Clone, Default)]
        pub struct Poseidon2ChipWrapper;

        #[derive(Clone)]
        pub struct PoseidonHashCells<F> {
            pub digest: AssignedCell<F, F>,
            pub left: AssignedCell<F, F>,
            pub right: AssignedCell<F, F>,
        }
        impl Poseidon2ChipWrapper {
            pub fn new() -> Self {
                Self
            }

            /// Compute Poseidon Pow5 hash of two field elements using the halo2_gadgets chip.
            /// Returns the assigned digest cell.
            #[allow(unused_variables)]
            pub fn hash2_chip(
                &self,
                layouter: &mut impl Layouter<halo2_proofs::halo2curves::pasta::Fp>,
                poseidon_cfg: &Pow5Config<halo2_proofs::halo2curves::pasta::Fp, 3, 2>,
                a: Value<halo2_proofs::halo2curves::pasta::Fp>,
                b: Value<halo2_proofs::halo2curves::pasta::Fp>,
            ) -> Result<PoseidonHashCells<halo2_proofs::halo2curves::pasta::Fp>, PlonkError>
            {
                // Construct chip and initialize hasher
                let chip = Pow5Chip::<halo2_proofs::halo2curves::pasta::Fp, 3, 2>::construct(
                    poseidon_cfg.clone(),
                );
                let mut hasher = PoseidonHash::<
                    halo2_proofs::halo2curves::pasta::Fp,
                    P128Pow5T3,
                    ConstantLength<2>,
                    3,
                    2,
                >::init(chip, layouter.namespace(|| "poseidon2"))
                .map_err(|_| PlonkError::Synthesis)?;

                // Assign inputs in a small region and feed into the hasher
                let (a_cell, b_cell) = layouter.assign_region(
                    || "poseidon2_inputs",
                    |mut region| {
                        let a_cell = crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            poseidon_cfg.state[0],
                            0,
                            || a,
                        )?;
                        let b_cell = crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            poseidon_cfg.state[1],
                            0,
                            || b,
                        )?;
                        Ok((a_cell, b_cell))
                    },
                )?;
                hasher
                    .update(&[a_cell, b_cell])
                    .map_err(|_| PlonkError::Synthesis)?;
                let digest = hasher.squeeze().map_err(|_| PlonkError::Synthesis)?;
                Ok(PoseidonHashCells {
                    digest,
                    left: a_cell,
                    right: b_cell,
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
                        let _m_cell = crate::zk::assign_advice_compat(
                            &mut region,
                            || "m",
                            m,
                            0,
                            || Value::known(m_v),
                        )?;
                        let _r_cell = crate::zk::assign_advice_compat(
                            &mut region,
                            || "r",
                            r,
                            0,
                            || Value::known(r_v),
                        )?;
                        // assign s0 via native helper for constraints, then constrain equal to gadget digest
                        let s0_v = compress2_native(m_v, r_v);
                        let s0_cell = crate::zk::assign_advice_compat(
                            &mut region,
                            || "s0",
                            s0,
                            0,
                            || Value::known(s0_v),
                        )?;
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "s1",
                            s1,
                            0,
                            || Value::known(s1_v),
                        )?;
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

                            let node_cell = crate::zk::assign_advice_compat(
                                &mut region,
                                || format!("node_{row}"),
                                node,
                                row,
                                || Value::known(current),
                            )?;
                            if let Some(ref prev) = previous_output {
                                layouter.constrain_equal(node_cell.cell(), prev.cell())?;
                            }
                            crate::zk::assign_advice_compat(
                                &mut region,
                                || format!("sibling_{row}"),
                                sibling,
                                row,
                                || Value::known(sib_val),
                            )?;
                            crate::zk::assign_advice_compat(
                                &mut region,
                                || format!("dir_{row}"),
                                dir,
                                row,
                                || Value::known(dir_val),
                            )?;
                            let left_cell = crate::zk::assign_advice_compat(
                                &mut region,
                                || format!("left_{row}"),
                                left,
                                row,
                                || Value::known(left_val),
                            )?;
                            let right_cell = crate::zk::assign_advice_compat(
                                &mut region,
                                || format!("right_{row}"),
                                right,
                                row,
                                || Value::known(right_val),
                            )?;

                            sel.enable(&mut region, row)?;

                            let hash_cells = chip.hash2_chip(
                                &mut layouter,
                                &poseidon_cfg,
                                Value::known(left_val),
                                Value::known(right_val),
                            )?;
                            layouter.constrain_equal(left_cell.cell(), hash_cells.left.cell())?;
                            layouter.constrain_equal(right_cell.cell(), hash_cells.right.cell())?;

                            let out_cell = crate::zk::assign_advice_compat(
                                &mut region,
                                || format!("out_{row}"),
                                out,
                                row,
                                || Value::known(hash_val),
                            )?;
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

    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    #[allow(unused_imports)]
    pub use self::poseidon::CommitOpenPoseidon as CommitOpen;
    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    #[allow(unused_imports)]
    pub use self::poseidon::Merkle2Poseidon as Merkle2;
    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    pub fn poseidon_compress2_native(a: Scalar, b: Scalar) -> Scalar {
        poseidon::compress2_native(a, b)
    }

    #[derive(Clone, Default)]
    pub struct VoteBoolCommit; // commit = Poseidon2_like(v, rho) public
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
                // v*(v-1)=0 and Poseidon2_like(v,rho)=commit (approximate gadget)
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "v",
                        v,
                        0,
                        || Value::known(Scalar::from(1)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "rho",
                        rho,
                        0,
                        || Value::known(Scalar::from(12345)),
                    )?;
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

                // hash-like commitment checks (placeholder pow5)
                // cm_in0 = H(a, r0); cm_in1 = H(b, r1); cm_out0 = H(c, r2); cm_out1 = H(d, r3)
                let h_in0 = {
                    let a2 = a.clone() * a.clone();
                    let a4 = a2.clone() * a2.clone();
                    let a5 = a4 * a.clone();
                    let r2 = r0.clone() * r0.clone();
                    let r4 = r2.clone() * r2.clone();
                    let r5 = r4 * r0.clone();
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * a5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * r5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(7))
                };
                let h_in1 = {
                    let a2 = b.clone() * b.clone();
                    let a4 = a2.clone() * a2.clone();
                    let a5 = a4 * b.clone();
                    let r2 = r1.clone() * r1.clone();
                    let r4 = r2.clone() * r2.clone();
                    let r5 = r4 * r1.clone();
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * a5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * r5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(7))
                };
                let h_out0 = {
                    let a2 = c.clone() * c.clone();
                    let a4 = a2.clone() * a2.clone();
                    let a5 = a4 * c.clone();
                    let r2 = r2.clone() * r2.clone();
                    let r4 = r2.clone() * r2.clone();
                    let r5 = r4 * r2.clone();
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * a5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * r5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(7))
                };
                let h_out1 = {
                    let a2 = d.clone() * d.clone();
                    let a4 = a2.clone() * a2.clone();
                    let a5 = a4 * d.clone();
                    let r2x = r3.clone() * r3.clone();
                    let r4 = r2x.clone() * r2x.clone();
                    let r5 = r4 * r3.clone();
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * a5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * r5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(7))
                };
                // nullifier = H(sk, serial)
                let h_nf = {
                    let a2 = skq.clone() * skq.clone();
                    let a4 = a2.clone() * a2.clone();
                    let a5 = a4 * skq.clone();
                    let r2 = serq.clone() * serq.clone();
                    let r4 = r2.clone() * r2.clone();
                    let r5 = r4 * serq.clone();
                    halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * a5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * r5
                        + halo2_proofs::plonk::Expression::Constant(Scalar::from(7))
                };
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in0",
                        in0,
                        0,
                        || Value::known(Scalar::from(7)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in1",
                        in1,
                        0,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out0",
                        out0,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out1",
                        out1,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r_in0",
                        r_in0,
                        0,
                        || Value::known(Scalar::from(11)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r_in1",
                        r_in1,
                        0,
                        || Value::known(Scalar::from(13)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r_out0",
                        r_out0,
                        0,
                        || Value::known(Scalar::from(17)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r_out1",
                        r_out1,
                        0,
                        || Value::known(Scalar::from(19)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sk",
                        sk,
                        0,
                        || Value::known(Scalar::from(1_234_567)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "serial",
                        serial,
                        0,
                        || Value::known(Scalar::from(42)),
                    )?;
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
                let a = vq + halo2_proofs::plonk::Expression::Constant(Scalar::from(7));
                let b = rhoq + halo2_proofs::plonk::Expression::Constant(Scalar::from(13));
                let a2 = a.clone() * a.clone();
                let a4 = a2.clone() * a2.clone();
                let a5 = a4 * a.clone();
                let b2 = b.clone() * b.clone();
                let b4 = b2.clone() * b2.clone();
                let b5 = b4 * b.clone();
                let h = halo2_proofs::plonk::Expression::Constant(Scalar::from(2)) * a5
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(3)) * b5;
                let commitment_delta = h.clone() - cmq.clone();
                // merkle2: w0 = H(cm, sib0); w1 = H(w0, sib1) = root
                let _t0a2 = h.clone() * h.clone(); // not exact; reuse a-like pattern for simplicity
                let expected_first_hash =
                    h + sib0q.clone() + halo2_proofs::plonk::Expression::Constant(Scalar::from(5));
                let _w1e = w0q.clone()
                    + sib1q.clone()
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(11));
                vec![
                    s.clone() * boolc,
                    s.clone() * commitment_delta,
                    s.clone() * (w0q - expected_first_hash),
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
                    let w0_v = v_v + rho_v + Scalar::from(5);
                    let w1_v = w0_v + sib1_v + Scalar::from(11);
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "v",
                        v,
                        0,
                        || Value::known(v_v),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "rho",
                        rho,
                        0,
                        || Value::known(rho_v),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sib0",
                        sib0,
                        0,
                        || Value::known(sib0_v),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sib1",
                        sib1,
                        0,
                        || Value::known(sib1_v),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "w0",
                        w0,
                        0,
                        || Value::known(w0_v),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "w1",
                        w1,
                        0,
                        || Value::known(w1_v),
                    )?;
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
                let computed_cm0 = h(a.clone(), r0.clone());
                let computed_cm1 = h(b.clone(), r1.clone());
                let computed_cm2 = h(c.clone(), r2.clone());
                let computed_cm3 = h(d.clone(), r3.clone());
                // Merkle2 on cm0 with s0_0 and s0_1
                let w0 = computed_cm0.clone()
                    + s0_0
                    + halo2_proofs::plonk::Expression::Constant(Scalar::from(5));
                let w1 =
                    w0.clone() + s0_1 + halo2_proofs::plonk::Expression::Constant(Scalar::from(11));
                // Nullifier
                let nf_exp = h(skq.clone(), serq.clone());
                vec![
                    s.clone() * (a.clone() + b.clone() - (c.clone() + d.clone())),
                    s.clone() * (computed_cm0 - input_commitment_slot0),
                    s.clone() * (computed_cm1 - input_commitment_slot1),
                    s.clone() * (computed_cm2 - output_commitment_slot0),
                    s.clone() * (computed_cm3 - output_commitment_slot1),
                    s.clone() * (nf_exp - nullifier_instance),
                    s * (w1 - rootq),
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in0",
                        in0,
                        0,
                        || Value::known(Scalar::from(7)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in1",
                        in1,
                        0,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out0",
                        out0,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out1",
                        out1,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r_in0",
                        r_in0,
                        0,
                        || Value::known(Scalar::from(11)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r_in1",
                        r_in1,
                        0,
                        || Value::known(Scalar::from(13)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r_out0",
                        r_out0,
                        0,
                        || Value::known(Scalar::from(17)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r_out1",
                        r_out1,
                        0,
                        || Value::known(Scalar::from(19)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sk",
                        sk,
                        0,
                        || Value::known(Scalar::from(1_234_567)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "serial",
                        serial,
                        0,
                        || Value::known(Scalar::from(42)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sib0_0",
                        sib0_0,
                        0,
                        || Value::known(Scalar::from(23)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sib0_1",
                        sib0_1,
                        0,
                        || Value::known(Scalar::from(29)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sib1_0",
                        sib1_0,
                        0,
                        || Value::known(Scalar::from(31)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sib1_1",
                        sib1_1,
                        0,
                        || Value::known(Scalar::from(37)),
                    )?;
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

    fn poseidon_pair(lhs: Scalar, rhs: Scalar) -> Scalar {
        let lhs = lhs + Scalar::from(7u64);
        let rhs = rhs + Scalar::from(13u64);
        Scalar::from(2u64) * poseidon_pow5(lhs) + Scalar::from(3u64) * poseidon_pow5(rhs)
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

    #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "v",
                        v,
                        0,
                        || Value::known(v_val),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "rho",
                        rho,
                        0,
                        || Value::known(rho_val),
                    )?;
                    for (i, col) in sibs.iter().enumerate() {
                        let sib_val = sibling_vals[i];
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib{i}"),
                            *col,
                            0,
                            || Value::known(sib_val),
                        )?;
                    }
                    for (i, col) in dirs.iter().enumerate() {
                        let dir_val = dir_vals[i];
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir{i}"),
                            *col,
                            0,
                            || Value::known(dir_val),
                        )?;
                    }
                    for (i, col) in ws.iter().enumerate() {
                        let w_val = witness_vals[i];
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w{i}"),
                            *col,
                            0,
                            || Value::known(w_val),
                        )?;
                    }
                    Ok(())
                },
            )
        }
    }

    #[cfg(feature = "zk-halo2-ipa-poseidon")]
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "v",
                        v,
                        0,
                        || Value::known(v_val),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "rho",
                        rho,
                        0,
                        || Value::known(rho_val),
                    )?;
                    for (i, col) in sibs.iter().enumerate() {
                        let sib_val = sibling_vals[i];
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib{i}"),
                            *col,
                            0,
                            || Value::known(sib_val),
                        )?;
                    }
                    for (i, col) in dirs.iter().enumerate() {
                        let dir_val = dir_vals[i];
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir{i}"),
                            *col,
                            0,
                            || Value::known(dir_val),
                        )?;
                    }
                    for (i, col) in ws.iter().enumerate() {
                        let w_val = witness_vals[i];
                        let cell = crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w{i}"),
                            *col,
                            0,
                            || Value::known(w_val),
                        )?;
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
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in0",
                        in0,
                        0,
                        || Value::known(Scalar::from(7)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "in1",
                        in1,
                        0,
                        || Value::known(Scalar::from(5)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out0",
                        out0,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "out1",
                        out1,
                        0,
                        || Value::known(Scalar::from(6)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r0",
                        r0,
                        0,
                        || Value::known(Scalar::from(11)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r1",
                        r1,
                        0,
                        || Value::known(Scalar::from(13)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r2",
                        r2,
                        0,
                        || Value::known(Scalar::from(17)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "r3",
                        r3,
                        0,
                        || Value::known(Scalar::from(19)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "sk",
                        sk,
                        0,
                        || Value::known(Scalar::from(1_234_567)),
                    )?;
                    crate::zk::assign_advice_compat(
                        &mut region,
                        || "serial",
                        serial,
                        0,
                        || Value::known(Scalar::from(42)),
                    )?;
                    for (i, col) in sib.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("sib{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(20 + i as u64)),
                        )?;
                    }
                    for (i, col) in dir.iter().enumerate() {
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("dir{i}"),
                            *col,
                            0,
                            || Value::known(Scalar::from(0)),
                        )?;
                    }
                    let mut acc = Scalar::from(0);
                    for (i, col) in w.iter().enumerate() {
                        acc += Scalar::from(20 + i as u64);
                        crate::zk::assign_advice_compat(
                            &mut region,
                            move || format!("w{i}"),
                            *col,
                            0,
                            || Value::known(acc),
                        )?;
                    }
                    Ok(())
                },
            )
        }
    }
}

#[cfg(feature = "zk-halo2")]
#[allow(clippy::too_many_lines)]
fn verify_halo2(backend: &str, proof: &ProofBox, vk: Option<&VerifyingKeyBox>) -> bool {
    // Minimal circuits used for dispatch sanity-checks.
    #[allow(dead_code)]
    mod tiny {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(3)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(3)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(9)),
                        )?;
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(3)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(3)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(9)),
                        )?;
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(7)),
                        )?;
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a0",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b0",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c0",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        // Row 1: 5 + 7 = 12
                        s.enable(&mut region, 1)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a1",
                            a,
                            1,
                            || Value::known(Scalar::from(5)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b1",
                            b,
                            1,
                            || Value::known(Scalar::from(7)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c1",
                            c,
                            1,
                            || Value::known(Scalar::from(12)),
                        )?;
                        Ok(())
                    },
                )
            }
        }
    }
    use std::io::Cursor;

    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::verify_proof,
        poly::ipa::strategy::SingleStrategy as SingleVerifier,
        transcript::Blake2bRead,
    };

    let Some(vk_box) = vk else { return false };
    // Sanity: backends must match
    // Note: caller already checked `proof.backend == attachment.backend` in ISI and executor
    // paths, but double-check here for robustness.
    // Also require non-empty payloads.
    if vk_box.backend != proof.backend || proof.bytes.is_empty() || vk_box.bytes.is_empty() {
        return false;
    }

    // Parse params and proof/instances using shared helpers
    let params = match zkparse::params_any(vk_box.bytes.as_slice()) {
        Some(p) => p,
        None => return false,
    };
    let (proof_payload, inst_cols) = match zkparse::proof_and_instances(proof.bytes.as_slice()) {
        Some(x) => x,
        None => return false,
    };
    let col_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
    let normalized = backend.replace("/ipa/", "/");
    match normalized.as_str() {
        "halo2/pasta/tiny-add" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::Add,
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let instances: [&[Scalar]; 0] = [];
                    verify_proof(&params, vk, strategy, &[&instances], &mut transcript).is_ok()
                }
            )
        }
        "halo2/pasta/tiny-mul" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::Mul,
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    if col_refs.is_empty() {
                        let instances: [&[Scalar]; 0] = [];
                        verify_proof(&params, vk, strategy, &[&instances], &mut transcript).is_ok()
                    } else {
                        let proofs_instances = [&col_refs[..]];
                        verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript)
                            .is_ok()
                    }
                }
            )
        }
        "halo2/pasta/tiny-add-2rows" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AddTwoRows,
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let instances: [&[Scalar]; 0] = [];
                    verify_proof(&params, vk, strategy, &[&instances], &mut transcript).is_ok()
                }
            )
        }
        "halo2/pasta/tiny-add-public" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AddPublic,
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    if col_refs.is_empty() {
                        let instances: [&[Scalar]; 0] = [];
                        verify_proof(&params, vk, strategy, &[&instances], &mut transcript).is_ok()
                    } else {
                        let proofs_instances = [&col_refs[..]];
                        verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript)
                            .is_ok()
                    }
                }
            )
        }
        "halo2/pasta/tiny-mul-public" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::MulPublic,
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    if col_refs.is_empty() {
                        let instances: [&[Scalar]; 0] = [];
                        verify_proof(&params, vk, strategy, &[&instances], &mut transcript).is_ok()
                    } else {
                        let proofs_instances = [&col_refs[..]];
                        verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript)
                            .is_ok()
                    }
                }
            )
        }
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
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript).is_ok()
                }
            )
        }
        "halo2/pasta/tiny-add3" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AddThree,
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let instances: [&[Scalar]; 0] = [];
                    verify_proof(&params, vk, strategy, &[&instances], &mut transcript).is_ok()
                }
            )
        }
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
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript).is_ok()
                }
            )
        }
        "halo2/pasta/tiny-anon-transfer-2x2" => {
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::AnonTransfer2x2,
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let instances: [&[Scalar]; 0] = [];
                    verify_proof(&params, vk, strategy, &[&instances], &mut transcript).is_ok()
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
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    match verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript) {
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
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    match verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript) {
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
        "halo2/pasta/tiny-vote-bool" => {
            let circuit = pasta_tiny::VoteBool;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let instances: [&[Scalar]; 0] = [];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &[&instances],
                &mut transcript,
            )
            .is_ok()
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
    use std::io::Cursor;

    use halo2_proofs::{
        halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
        plonk::verify_proof,
        poly::ipa::strategy::SingleStrategy as SingleVerifier,
        transcript::Blake2bRead,
    };
    use iroha_zkp_halo2::Halo2ProofEnvelope;

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

    let params: PastaParams = match zkparse::params_any(vk_box.bytes.as_slice()) {
        Some(p) => p,
        None => return reject("missing/invalid IPAK parameters in verifying key envelope"),
    };

    // Parse proof payload + instances via shared helper
    let parsed_envelope = Halo2ProofEnvelope::from_bytes(proof.bytes.as_slice())
        .ok()
        .and_then(|env| {
            let mut columns = Vec::with_capacity(env.public_inputs.len());
            for chunk in &env.public_inputs {
                let mut repr = <Scalar as ff::PrimeField>::Repr::default();
                repr.as_mut().copy_from_slice(chunk);
                let scalar = Option::from(<Scalar as ff::PrimeField>::from_repr(repr))?;
                columns.push(vec![scalar]);
            }
            Some((env.proof, columns, env.header))
        });

    let (proof_payload, inst_cols, envelope_header) =
        if let Some((payload, cols, header)) = parsed_envelope {
            (payload, cols, Some(header))
        } else {
            let (payload, cols) = match zkparse::proof_and_instances(proof.bytes.as_slice()) {
                Some(x) => x,
                None => return reject("invalid ZK1 proof envelope payload"),
            };
            (payload, cols, None)
        };
    let col_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();

    if let Some(header) = envelope_header
        && params.k() != u32::from(header.k)
    {
        return reject("proof header k does not match verifying key IPAK");
    }
    // For IPA, we normalize backend tag to reuse circuit mapping
    let normalized = backend.replace("/ipa/", "/");
    match normalized.as_str() {
        "halo2/pasta/tiny-add" => {
            let circuit = pasta_tiny::Add;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let instances: [&[Scalar]; 0] = [];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &[&instances],
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/tiny-mul" => {
            let circuit = pasta_tiny::Mul;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            if col_refs.is_empty() {
                let instances: [&[Scalar]; 0] = [];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &[&instances],
                    &mut transcript,
                )
                .is_ok()
            } else {
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            }
        }
        "halo2/pasta/tiny-add-2rows" => {
            let circuit = pasta_tiny::AddTwoRows;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let instances: [&[Scalar]; 0] = [];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &[&instances],
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/tiny-add-public" => {
            let circuit = pasta_tiny::AddPublic;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            if col_refs.is_empty() {
                let instances: [&[Scalar]; 0] = [];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &[&instances],
                    &mut transcript,
                )
                .is_ok()
            } else {
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            }
        }
        "halo2/pasta/tiny-mul-public" => {
            let circuit = pasta_tiny::MulPublic;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            if col_refs.is_empty() {
                let instances: [&[Scalar]; 0] = [];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &[&instances],
                    &mut transcript,
                )
                .is_ok()
            } else {
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            }
        }
        "halo2/pasta/tiny-id-public" => {
            let circuit = pasta_tiny::IdPublic;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if col_refs.is_empty() {
                return false;
            }
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let proofs_instances = [&col_refs[..]];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &proofs_instances,
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/tiny-add3" => {
            let circuit = pasta_tiny::AddThree;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let instances: [&[Scalar]; 0] = [];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &[&instances],
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/tiny-add2inst-public" => {
            let circuit = pasta_tiny::AddTwoInstPublic;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if col_refs.len() < 2 {
                return false;
            }
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let proofs_instances = [&col_refs[..]];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &proofs_instances,
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/ivm-overlay-bind" => {
            // Instances: 8 columns (code_hash limbs + overlay_hash limbs), 1 row each.
            if col_refs.len() != 8 || col_refs.iter().any(|col| col.len() != 1) {
                return false;
            }
            let circuit = pasta_tiny::IvmOverlayBind::default();
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let proofs_instances = [&col_refs[..]];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &proofs_instances,
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/ivm-execution-v1" => {
            // Instances: 16 columns (code_hash limbs + overlay_hash limbs + events_commitment limbs + gas_policy_commitment limbs), 1 row each.
            if col_refs.len() != 16 || col_refs.iter().any(|col| col.len() != 1) {
                return false;
            }
            let circuit = pasta_tiny::IvmExecutionBindV1::default();
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let proofs_instances = [&col_refs[..]];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &proofs_instances,
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/offline-note-recursive" => {
            if col_refs.len() != OFFLINE_NOTE_INSTANCE_COLUMNS
                || col_refs.iter().any(|col| col.len() != 1)
            {
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                pasta_tiny::OfflineNoteSemantic::default(),
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript).is_ok()
                }
            )
        }
        "halo2/pasta/tiny-anon-transfer-2x2" => {
            let circuit = pasta_tiny::AnonTransfer2x2;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let instances: [&[Scalar]; 0] = [];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &[&instances],
                &mut transcript,
            )
            .is_ok()
        }
        circuit_id if confidential_v2::is_confidential_transfer_v2_circuit_id(circuit_id) => {
            if col_refs.len() != 9 || col_refs.iter().any(|col| col.len() != 1) {
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                confidential_v2::ConfidentialTransferCircuitV2::<
                    { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
                >::default(),
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript).is_ok()
                }
            )
        }
        circuit_id if confidential_v2::is_confidential_unshield_v2_circuit_id(circuit_id) => {
            if col_refs.len() != 8 || col_refs.iter().any(|col| col.len() != 1) {
                return false;
            }
            cached_vk_for!(
                &params,
                normalized.as_str(),
                vk_box,
                confidential_v2::ConfidentialUnshieldCircuitV2::<
                    { confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2 },
                >::default(),
                |vk| {
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript).is_ok()
                }
            )
        }
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
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript).is_ok()
                }
            )
        }
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
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript).is_ok()
                }
            )
        }
        "halo2/pasta/anon-transfer-2x2-merkle8" => {
            // Use depth-8 generic with dual membership; select algorithm by backend suffix
            let use_poseidon = backend.ends_with("-poseidon");
            if use_poseidon {
                let circuit = poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>;
                let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if col_refs.len() < 6 {
                    return false;
                }
                let mut transcript =
                    Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                let strategy = SingleVerifier::new(&params);
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            } else {
                let circuit = depth::AnonTransfer2x2CommitMerkle::<8>;
                let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if col_refs.len() < 6 {
                    return false;
                }
                let mut transcript =
                    Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                let strategy = SingleVerifier::new(&params);
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            }
        }
        "halo2/pasta/tiny-vote-bool" => {
            let circuit = pasta_tiny::VoteBool;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let instances: [&[Scalar]; 0] = [];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &[&instances],
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/tiny-commit-open" => {
            // If Poseidon gadgets are enabled, use the Poseidon-backed variant.
            #[cfg(feature = "zk-halo2-ipa-poseidon")]
            let circuit = pasta_tiny::poseidon::CommitOpenPoseidon;
            #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
            let circuit = pasta_tiny::CommitOpen;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if col_refs.is_empty() {
                return false;
            }
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let proofs_instances = [&col_refs[..]];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &proofs_instances,
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/tiny-merkle2" => {
            // If Poseidon gadgets are enabled, use the Poseidon-backed variant.
            #[cfg(feature = "zk-halo2-ipa-poseidon")]
            let circuit = pasta_tiny::poseidon::Merkle2Poseidon;
            #[cfg(not(feature = "zk-halo2-ipa-poseidon"))]
            let circuit = pasta_tiny::Merkle2;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if col_refs.is_empty() {
                return false;
            }
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let proofs_instances = [&col_refs[..]];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &proofs_instances,
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/vote-bool-commit" => {
            // Instances: [commit], 1 row
            let circuit = pasta_tiny::VoteBoolCommit;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if col_refs.is_empty() {
                return false;
            }
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let proofs_instances = [&col_refs[..]];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &proofs_instances,
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/vote-bool-commit-merkle2" => {
            // Instances: [commit, root], 1 row
            let circuit = pasta_tiny::VoteBoolCommitMerkle2;
            let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if col_refs.len() < 2 {
                return false;
            }
            let mut transcript =
                Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
            let strategy = SingleVerifier::new(&params);
            let proofs_instances = [&col_refs[..]];
            verify_proof(
                &params,
                vk_h2.as_ref(),
                strategy,
                &proofs_instances,
                &mut transcript,
            )
            .is_ok()
        }
        "halo2/pasta/vote-bool-commit-merkle8" => {
            // Use depth-8 generic; select algorithm by backend suffix
            let use_poseidon = backend.ends_with("-poseidon");
            if use_poseidon {
                let circuit = poseidon_depth::VoteBoolCommitMerklePoseidon::<8>;
                let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if col_refs.len() < 2 {
                    return false;
                }
                let mut transcript =
                    Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                let strategy = SingleVerifier::new(&params);
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            } else {
                let circuit = depth::VoteBoolCommitMerkle::<8>;
                let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if col_refs.len() < 2 {
                    return false;
                }
                let mut transcript =
                    Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                let strategy = SingleVerifier::new(&params);
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            }
        }
        // Depth-16 variants
        "halo2/pasta/anon-transfer-2x2-merkle16" => {
            let use_poseidon = backend.ends_with("-poseidon");
            if use_poseidon {
                let circuit = poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>;
                let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if col_refs.len() < 6 {
                    return false;
                }
                let mut transcript =
                    Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                let strategy = SingleVerifier::new(&params);
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            } else {
                let circuit = depth::AnonTransfer2x2CommitMerkle::<16>;
                let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if col_refs.len() < 6 {
                    return false;
                }
                let mut transcript =
                    Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                let strategy = SingleVerifier::new(&params);
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            }
        }
        "halo2/pasta/vote-bool-commit-merkle16" => {
            let use_poseidon = backend.ends_with("-poseidon");
            if use_poseidon {
                let circuit = poseidon_depth::VoteBoolCommitMerklePoseidon::<16>;
                let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if col_refs.len() < 2 {
                    return false;
                }
                let mut transcript =
                    Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                let strategy = SingleVerifier::new(&params);
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
            } else {
                let circuit = depth::VoteBoolCommitMerkle::<16>;
                let vk_h2 = match keygen_vk_cached(normalized.as_str(), &params, &circuit) {
                    Ok(v) => v,
                    Err(_) => return false,
                };
                if col_refs.len() < 2 {
                    return false;
                }
                let mut transcript =
                    Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                let strategy = SingleVerifier::new(&params);
                let proofs_instances = [&col_refs[..]];
                verify_proof(
                    &params,
                    vk_h2.as_ref(),
                    strategy,
                    &proofs_instances,
                    &mut transcript,
                )
                .is_ok()
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
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    match verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript) {
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
                    let mut transcript =
                        Blake2bRead::<_, Curve, _>::init(Cursor::new(proof_payload.as_slice()));
                    let strategy = SingleVerifier::new(&params);
                    let proofs_instances = [&col_refs[..]];
                    match verify_proof(&params, vk, strategy, &proofs_instances, &mut transcript) {
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

#[cfg(not(feature = "zk-halo2"))]
fn verify_halo2(_backend: &str, _proof: &ProofBox, _vk: Option<&VerifyingKeyBox>) -> bool {
    // Feature disabled: refuse Halo2 proofs to avoid silent acceptance of forged transcripts.
    false
}

#[cfg(all(test, feature = "zk-preverify"))]
mod trace_proof_queue_tests {
    use super::*;

    #[test]
    fn queue_and_collect_trace_proofs() {
        reset_trace_proof_state_for_tests();
        let code_hash = [0x11; 32];
        let digest = [0xAA; 32];
        let artifact = make_trace_digest_artifact(code_hash, None, digest);
        queue_trace_proof(7, artifact.clone());

        let collected = collect_trace_proofs_for_height(7);
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].backend, TRACE_DIGEST_BACKEND);
        assert_eq!(collected[0].proof, digest.to_vec());
        assert_eq!(collected[0].code_hash, code_hash);
        assert!(collected[0].tx_hash.is_none());

        // Subsequent collection should be empty once drained.
        assert!(collect_trace_proofs_for_height(7).is_empty());
    }
}

#[cfg(all(test, feature = "zk-preverify"))]
mod trace_proving_queue_tests {
    use std::{num::NonZeroU64, sync::Arc};

    use iroha_crypto::Hash;
    use ivm::encoding;

    use super::*;

    fn assemble_zk(code: &[u8], max_cycles: u64) -> Vec<u8> {
        use ivm::ProgramMetadata;
        let meta = ProgramMetadata {
            mode: ivm::ivm_mode::ZK,
            vector_length: 0,
            max_cycles,
            abi_version: 1,
            ..ProgramMetadata::default()
        };
        let mut program = meta.encode();
        program.extend_from_slice(code);
        program
    }

    fn sample_zk_task() -> crate::pipeline::zk_lane::ZkTask {
        let halt = encoding::wide::encode_halt().to_le_bytes();
        let program = assemble_zk(&halt, 4);
        let metadata = ivm::ProgramMetadata::parse(&program).expect("metadata parse");
        let code_bytes = &program[metadata.code_offset..];
        let code_hash = Hash::new(code_bytes);
        let trace = vec![
            ivm::zk::RegisterState {
                pc: 0,
                gpr: [0u64; 256],
                tags: [false; 256],
            },
            ivm::zk::RegisterState {
                pc: 4,
                gpr: [0u64; 256],
                tags: [false; 256],
            },
        ];
        let constraints: Vec<ivm::zk::Constraint> = Vec::new();
        let circuit = VMExecutionCircuit::new(&program, &trace, &constraints);
        assert!(circuit.verify().is_ok(), "sample trace must verify");

        crate::pipeline::zk_lane::ZkTask {
            tx_hash: None,
            code_hash: *code_hash.as_ref(),
            program: Arc::new(program),
            header: None,
            trace,
            constraints,
            mem_log: Vec::new(),
            reg_log: Vec::new(),
            step_log: Vec::new(),
            transport_capabilities: None,
            negotiated_capabilities: None,
        }
    }

    #[test]
    fn queue_and_collect_trace_jobs() {
        reset_trace_proving_state_for_tests();
        let task = sample_zk_task();
        let digest = task.digest();
        queue_trace_for_proving(3, TraceForProving::from_task(&task, digest));
        let collected = collect_traces_for_proving(3);
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].digest, digest);
        assert_eq!(collected[0].code_hash, task.code_hash);
    }

    #[test]
    fn mock_proof_snapshot_encodes_digest() {
        reset_trace_proof_state_for_tests();
        reset_trace_proving_state_for_tests();

        let mut task = sample_zk_task();
        let height = NonZeroU64::new(9).expect("non-zero");
        task.header = Some(iroha_data_model::block::BlockHeader::new(
            height, None, None, None, 0, 0,
        ));

        let digest = task.digest();
        queue_trace_for_proving(height.get(), TraceForProving::from_task(&task, digest));

        let mut entries = collect_traces_for_proving(height.get());
        assert_eq!(entries.len(), 1);
        let entry = entries.pop().expect("trace entry");
        let circuit = VMExecutionCircuit::new(&entry.program, &entry.trace, &entry.constraints);
        assert!(circuit.verify().is_ok());
        let snapshot = entry.into_snapshot();
        assert_eq!(snapshot.backend, TRACE_MOCK_BACKEND);
        queue_trace_proof(height.get(), snapshot);

        let collected = collect_trace_proofs_for_height(height.get());
        assert!(
            collected.iter().any(|p| p.backend == TRACE_MOCK_BACKEND),
            "expected mock proof backend, got {:?}",
            collected
                .iter()
                .map(|p| p.backend.clone())
                .collect::<Vec<_>>()
        );
    }
}

#[cfg(test)]
mod preverified_key_tests {
    use super::*;

    #[test]
    fn preverified_proof_key_binds_vk_reference_and_commitment() {
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let vk_ref = VerifyingKeyId::new("halo2/ipa", "vk_a");
        let commitment = [7u8; 32];
        let key = PreverifiedProofKey::new(&proof, &vk_ref, commitment);
        let mut map = PreverifiedProofMap::new();
        map.insert(key, true);

        assert_eq!(
            map.get(&PreverifiedProofKey::new(&proof, &vk_ref, commitment)),
            Some(&true)
        );
        assert_eq!(
            map.get(&PreverifiedProofKey::new(
                &proof,
                &VerifyingKeyId::new("halo2/ipa", "vk_b"),
                commitment
            )),
            None
        );
        assert_eq!(
            map.get(&PreverifiedProofKey::new(
                &proof,
                &VerifyingKeyId::new("stark/fri", "vk_a"),
                commitment
            )),
            None
        );
        assert_eq!(
            map.get(&PreverifiedProofKey::new(&proof, &vk_ref, [8u8; 32])),
            None
        );
    }

    #[test]
    fn preverified_proof_key_binds_proof_backend_and_bytes() {
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3]);
        let vk_ref = VerifyingKeyId::new("halo2/ipa", "vk_a");
        let commitment = [7u8; 32];
        let mut map = PreverifiedProofMap::new();
        map.insert(PreverifiedProofKey::new(&proof, &vk_ref, commitment), true);

        assert_eq!(
            map.get(&PreverifiedProofKey::new(
                &ProofBox::new("stark/fri".into(), proof.bytes.clone()),
                &vk_ref,
                commitment
            )),
            None
        );
        assert_eq!(
            map.get(&PreverifiedProofKey::new(
                &ProofBox::new(proof.backend.clone(), vec![1, 2, 4]),
                &vk_ref,
                commitment
            )),
            None
        );
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
        let proof = ProofBox::new("halo2/ipa".into(), vec![1, 2, 3, 4]);
        let vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![5, 6, 7, 8]);
        let expected = hash_vk(&vk);

        let mut budget_dedup = DedupCache::new();
        assert_eq!(
            preverify_with_budget(
                &proof,
                Some(&vk),
                &mut budget_dedup,
                1,
                Some(expected),
                Some(expected),
                true,
            ),
            PreverifyResult::PreverifyBudgetExceeded
        );
        assert_eq!(
            preverify_with_budget(
                &proof,
                Some(&vk),
                &mut budget_dedup,
                0,
                Some(expected),
                Some(expected),
                true,
            ),
            PreverifyResult::Accepted
        );

        let mut mismatch_dedup = DedupCache::new();
        let mut wrong = expected;
        wrong[0] ^= 0x80;
        assert_eq!(
            preverify_with_budget(
                &proof,
                Some(&vk),
                &mut mismatch_dedup,
                0,
                Some(wrong),
                Some(expected),
                true,
            ),
            PreverifyResult::VerifyingKeyMismatch
        );
        assert_eq!(
            preverify_with_budget(
                &proof,
                Some(&vk),
                &mut mismatch_dedup,
                0,
                Some(expected),
                Some(expected),
                true,
            ),
            PreverifyResult::Accepted
        );

        let mut wrong_vk_dedup = DedupCache::new();
        let wrong_vk = VerifyingKeyBox::new("halo2/ipa".into(), vec![8, 7, 6, 5]);
        assert_eq!(
            preverify_with_budget(
                &proof,
                Some(&wrong_vk),
                &mut wrong_vk_dedup,
                0,
                Some(expected),
                Some(expected),
                true,
            ),
            PreverifyResult::VerifyingKeyMismatch
        );
        assert_eq!(
            preverify_with_budget(
                &proof,
                Some(&vk),
                &mut wrong_vk_dedup,
                0,
                Some(expected),
                Some(expected),
                true,
            ),
            PreverifyResult::Accepted
        );

        let mut inactive_dedup = DedupCache::new();
        assert_eq!(
            preverify_with_budget(
                &proof,
                Some(&vk),
                &mut inactive_dedup,
                0,
                Some(expected),
                Some(expected),
                false,
            ),
            PreverifyResult::VerifyingKeyInactive
        );
        assert_eq!(
            preverify_with_budget(
                &proof,
                Some(&vk),
                &mut inactive_dedup,
                0,
                Some(expected),
                Some(expected),
                true,
            ),
            PreverifyResult::Accepted
        );
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
#[allow(unused_imports)]
mod tests {
    #[cfg(all(
        feature = "halo2-dev-tests",
        any(feature = "zk-halo2", feature = "zk-halo2-ipa")
    ))]
    use std::sync::Arc;

    #[cfg(feature = "zk-halo2")]
    use ff::PrimeField;
    #[cfg(feature = "zk-halo2")]
    use halo2_proofs::poly::{
        commitment::ParamsProver,
        ipa::{commitment::IPACommitmentScheme, multiopen::ProverIPA},
    };
    #[cfg(feature = "zk-halo2")]
    use halo2_proofs::transcript::TranscriptWriterBuffer;
    #[cfg(feature = "zk-halo2")]
    use rand_core_06::OsRng;

    use super::*;
    #[cfg(all(feature = "zk-tests", feature = "halo2-dev-tests"))]
    use crate::zk::pasta_tiny::{
        VoteBoolCommitMerkle8, vote_bool_commit_merkle8_sample_inputs,
        vote_bool_commit_merkle8_witnesses,
    };

    #[test]
    fn vote_bool_commit_merkle8_mock_prover_succeeds() {
        use halo2_proofs::dev::MockProver;

        let circuit = VoteBoolCommitMerkle8::default();
        let (v_val, rho_val, sibling_vals, dir_vals) = vote_bool_commit_merkle8_sample_inputs();
        let (commit, _witnesses, root) =
            vote_bool_commit_merkle8_witnesses(v_val, rho_val, sibling_vals, dir_vals);
        let public_inputs = vec![vec![commit], vec![root]];
        let prover = MockProver::run(8, &circuit, public_inputs).expect("mock prover");
        prover.assert_satisfied();
    }

    #[cfg(feature = "zk-halo2-ipa-poseidon")]
    #[test]
    fn vote_bool_commit_merkle8_poseidon_mock_prover() {
        use halo2_proofs::dev::MockProver;

        let circuit = VoteBoolCommitMerkle8::default();
        let (v_val, rho_val, sibling_vals, dir_vals) = vote_bool_commit_merkle8_sample_inputs();
        let (commit, _witnesses, root) =
            vote_bool_commit_merkle8_witnesses(v_val, rho_val, sibling_vals, dir_vals);
        let public_inputs = vec![vec![commit], vec![root]];
        let prover = MockProver::run(8, &circuit, public_inputs).expect("mock prover");
        prover.assert_satisfied();
    }

    #[cfg(all(
        feature = "halo2-dev-tests",
        any(feature = "zk-halo2", feature = "zk-halo2-ipa")
    ))]
    #[allow(dead_code)]
    fn backend_tag_vote_bool_commit_merkle(depth: usize, use_poseidon: bool) -> String {
        let algo = if use_poseidon { "-poseidon" } else { "" };
        format!("halo2/pasta/ipa/vote-bool-commit-merkle{depth}{algo}")
    }

    #[cfg(all(
        feature = "halo2-dev-tests",
        any(feature = "zk-halo2", feature = "zk-halo2-ipa")
    ))]
    #[allow(dead_code)]
    fn backend_tag_anon_transfer_merkle(depth: usize, use_poseidon: bool) -> String {
        let algo = if use_poseidon { "-poseidon" } else { "" };
        format!("halo2/pasta/ipa/anon-transfer-2x2-merkle{depth}{algo}")
    }
    #[cfg(all(
        feature = "halo2-dev-tests",
        any(feature = "zk-halo2", feature = "zk-halo2-ipa")
    ))]
    #[test]
    fn vk_cache_reuses_entries() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{Circuit, ConstraintSystem, Error as PlonkError, Selector},
            poly::{Rotation, commitment::Params as _},
        };

        #[derive(Clone, Default)]
        struct CacheCircuit;

        impl Circuit<Scalar> for CacheCircuit {
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
                meta.create_gate("cache_add", |meta| {
                    let s = meta.query_selector(s);
                    let a_cur = meta.query_advice(a, Rotation::cur());
                    let b_cur = meta.query_advice(b, Rotation::cur());
                    let c_cur = meta.query_advice(c, Rotation::cur());
                    vec![s * (a_cur + b_cur - c_cur)]
                });
                (a, b, c, s)
            }
            fn synthesize(
                &self,
                (a, b, c, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "cache_add_region",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(1)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(3)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let params: PastaParams = pasta_params_new(5);
        let backend = "halo2/pasta/cache-test";
        let circuit = CacheCircuit;
        let first = keygen_vk_cached(backend, &params, &circuit).expect("vk");
        let second = keygen_vk_cached(backend, &params, &circuit).expect("vk");
        assert!(Arc::ptr_eq(&first, &second));

        if let Some(cache) = super::BUILTIN_VK_CACHE.get() {
            let guard = cache.lock().expect("cache poisoned");
            let key = super::BuiltinVkCacheKey {
                backend: backend.to_owned(),
                params_fingerprint: super::params_fingerprint(&params),
            };
            let Some(cached) = guard.get(&key) else {
                panic!("expected builtin verifying key cache entry for {backend}");
            };
            assert!(Arc::ptr_eq(&first, cached));
        } else {
            panic!("cache not initialized");
        }
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn zk1_envelope_pasta_ipa_verify_add_public() {
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::Rotation,
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
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

        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
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

        // Build ZK1 envelopes: VK has IPAK(k); proof has PROF + I10P(inst)
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-add-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn kaigi_roster_backend_accepts_valid_proof() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{keygen_pk, keygen_vk},
            transcript::{Blake2bWrite, Challenge255},
        };
        use kaigi_zk::{
            KAIGI_ROSTER_CIRCUIT_K, compute_commitment, compute_nullifier, empty_roster_root_hash,
            roster_root_limbs,
        };

        let k = KAIGI_ROSTER_CIRCUIT_K;
        let params: PastaParams = pasta_params_new(k);

        let account = Scalar::from(3u64);
        let domain_salt = Scalar::from(17u64);
        let nullifier_seed = Scalar::from(25u64);

        let root_hash = empty_roster_root_hash();
        let circuit = KaigiRosterJoinCircuit::new(
            account,
            domain_salt,
            nullifier_seed,
            roster_root_limbs(&root_hash),
        );
        let commitment = compute_commitment(account, domain_salt);
        let nullifier = compute_nullifier(account, nullifier_seed);

        let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");

        let mut inst_cols = vec![vec![commitment], vec![nullifier]];
        for limb in roster_root_limbs(&root_hash) {
            inst_cols.push(vec![limb]);
        }
        let inst_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
        let proof_instances = vec![inst_refs.as_slice()];

        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
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
            &[circuit],
            &proof_instances,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp_cols(&inst_refs, &mut prf_env);

        let vk_box = VerifyingKeyBox::new(KAIGI_ROSTER_BACKEND.into(), vk_env);
        let prf_box = ProofBox::new(KAIGI_ROSTER_BACKEND.into(), prf_env);
        assert!(
            super::verify_backend(KAIGI_ROSTER_BACKEND, &prf_box, Some(&vk_box)),
            "kaigi roster backend should accept valid proof"
        );
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn kaigi_usage_backend_accepts_valid_proof() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{keygen_pk, keygen_vk},
            transcript::{Blake2bWrite, Challenge255},
        };
        use kaigi_zk::{
            KAIGI_USAGE_BACKEND, KAIGI_USAGE_CIRCUIT_K, KaigiUsageCommitmentCircuit,
            compute_usage_commitment,
        };

        let params: PastaParams = pasta_params_new(KAIGI_USAGE_CIRCUIT_K);
        let duration = Scalar::from(1_200u64);
        let billed = Scalar::from(345u64);
        let segment = Scalar::from(2u64);

        let circuit = KaigiUsageCommitmentCircuit::new(duration, billed, segment);
        let vk_h2 = keygen_vk(&params, &circuit).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &circuit).expect("pk");

        let commitment = compute_usage_commitment(duration, billed, segment);
        let inst_cols = vec![vec![commitment]];
        let inst_refs: Vec<&[Scalar]> = inst_cols.iter().map(Vec::as_slice).collect();
        let proof_instances = vec![inst_refs.as_slice()];

        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
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
            &[circuit],
            &proof_instances,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, KAIGI_USAGE_CIRCUIT_K);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp_cols(&inst_refs, &mut prf_env);

        let vk_box = VerifyingKeyBox::new(KAIGI_USAGE_BACKEND.into(), vk_env);
        let prf_box = ProofBox::new(KAIGI_USAGE_BACKEND.into(), prf_env);
        assert!(
            super::verify_backend(KAIGI_USAGE_BACKEND, &prf_box, Some(&vk_box)),
            "kaigi usage backend should accept valid proof"
        );
    }

    #[test]
    fn proof_hash_stable() {
        let p1 = ProofBox::new("halo2/pasta".into(), vec![1, 2, 3, 4]);
        let p2 = ProofBox::new("halo2/pasta".into(), vec![1, 2, 3, 4]);
        assert_eq!(hash_proof(&p1), hash_proof(&p2));
    }

    #[test]
    fn proof_and_vk_hash_domains_are_distinct() {
        let proof = ProofBox::new("halo2/pasta".into(), vec![1, 2, 3, 4]);
        let vk = VerifyingKeyBox::new("halo2/pasta".into(), vec![1, 2, 3, 4]);
        assert_ne!(hash_proof(&proof), hash_vk(&vk));
    }

    #[test]
    fn proof_hash_length_prefixes_backend_and_payload() {
        let p1 = ProofBox::new("ab".into(), b"cdef".to_vec());
        let p2 = ProofBox::new("abc".into(), b"def".to_vec());
        assert_ne!(hash_proof(&p1), hash_proof(&p2));
    }

    #[test]
    fn dedup_works() {
        let mut d = DedupCache::new();
        let p1 = ProofBox::new("halo2/pasta".into(), vec![9, 9, 9]);
        let p2 = ProofBox::new("halo2/pasta".into(), vec![9, 9, 9]);
        let p3 = ProofBox::new("groth16/bls12_381".into(), vec![9, 9, 9]);
        assert!(d.check_and_insert(&p1));
        assert!(!d.check_and_insert(&p2), "duplicate should be rejected");
        assert!(d.check_and_insert(&p3), "different backend is distinct");
    }

    #[test]
    fn hash_vk_stable() {
        let v1 = VerifyingKeyBox::new("halo2/pasta".into(), vec![5, 5]);
        let v2 = VerifyingKeyBox::new("halo2/pasta".into(), vec![5, 5]);
        assert_eq!(hash_vk(&v1), hash_vk(&v2));
    }

    #[test]
    fn preverify_basic() {
        let mut d = DedupCache::new();
        let p = ProofBox::new("halo2/pasta".into(), vec![1]);
        assert_eq!(
            preverify_with_budget(&p, None, &mut d, 100, None, None, true),
            PreverifyResult::Accepted
        );
        assert_eq!(
            preverify_with_budget(&p, None, &mut d, 100, None, None, true),
            PreverifyResult::Duplicate
        );
        // Different commitment should allow same proof bytes
        let p2 = ProofBox::new("halo2/pasta".into(), vec![1]);
        assert_eq!(
            preverify_with_budget(&p2, None, &mut d, 100, Some([1u8; 32]), None, true),
            PreverifyResult::Accepted
        );
        assert_eq!(
            preverify_with_budget(&p2, None, &mut d, 100, Some([1u8; 32]), None, true),
            PreverifyResult::Duplicate
        );
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_gate_requires_vk_and_valid_encoding() {
        let backend = "halo2/pasta/tiny-add";
        let proof = ProofBox::new(backend.into(), vec![1, 2, 3]);
        // Missing VK → reject
        assert!(!super::verify_backend(backend, &proof, None));
        // Wrong backend in VK → reject
        let vk_wrong = VerifyingKeyBox::new("halo2/bls12_381".into(), vec![9]);
        assert!(!super::verify_backend(backend, &proof, Some(&vk_wrong)));
        // Bad encoding → reject
        let vk_bad = VerifyingKeyBox::new(backend.into(), b"BAD!".to_vec());
        assert!(!super::verify_backend(backend, &proof, Some(&vk_bad)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_end_to_end_proof_verification() {
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::Rotation,
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        // A tiny circuit with no public inputs: enforce 2 + 2 = 4
        #[derive(Clone, Default)]
        struct Tiny;

        impl Circuit<Scalar> for Tiny {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
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
                    || "tiny",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2u64)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2u64)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4u64)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        // Setup params and keys
        let k = 5u32; // small-ish circuit size
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &Tiny::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &Tiny::default()).expect("pk");

        // Create proof (no public inputs)
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
            &[Tiny::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // Serialize VK/proof in a ZK1 envelope (IPAK + H2VK/H2PF)
        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);

        // Wrap into data model boxes
        let backend = "halo2/pasta/tiny-add";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);

        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_with_instance_add_kzg() {
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::Rotation,
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        // Params and keys
        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &TinyAddPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &TinyAddPublic::default()).expect("pk");

        // Instances: one column, one row (public value 4)
        let inst_col = vec![Scalar::from(4u64)];
        let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
        let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];

        // Create proof
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

        // Build VK container (ZK1)
        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        // Build proof container + INST TLV (ZK1)
        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut proof_container);

        // Verify via backend dispatch
        let backend = "halo2/pasta/tiny-add-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_add_2rows_kzg() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct Add2Rows;
        impl Circuit<Scalar> for Add2Rows {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
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
                    || "two_rows",
                    |mut region| {
                        // row 0: 2 + 2 = 4
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a0",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b0",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c0",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        // row 1: 5 + 7 = 12
                        s.enable(&mut region, 1)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a1",
                            a,
                            1,
                            || Value::known(Scalar::from(5)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b1",
                            b,
                            1,
                            || Value::known(Scalar::from(7)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c1",
                            c,
                            1,
                            || Value::known(Scalar::from(12)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 6u32; // two-row small circuit
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &Add2Rows::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &Add2Rows::default()).expect("pk");

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
            &[Add2Rows::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);

        let backend = "halo2/pasta/tiny-add-2rows";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_id_public_kzg_with_and_without_inst() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct IdPub;
        impl Circuit<Scalar> for IdPub {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(7)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &IdPub::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &IdPub::default()).expect("pk");
        // Create proof with a public instance value present (7). We will
        // construct two proof containers below: one without the INST TLV
        // (must be rejected by the verifier) and one with INST (must pass).
        let inst_binding = [Scalar::from(7u64)];
        let inst_cols: Vec<&[Scalar]> = vec![&inst_binding[..]];
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
            &[IdPub::default()],
            &inst_proofs,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        // Backend/tag
        let backend = "halo2/pasta/tiny-id-public";

        // Case 1: Missing INST → must fail
        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container.clone());
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));

        // Case 2: With INST → should succeed
        let inst_val = Scalar::from(7u64);
        let mut proof_container2 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container2, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(&[inst_val], &mut proof_container2);
        let vk_box2 = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box2 = ProofBox::new(backend.into(), proof_container2);
        assert!(super::verify_backend(backend, &prf_box2, Some(&vk_box2)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_ipa_acceptance_variants() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        // Tiny add (no INST)
        #[derive(Clone, Default)]
        struct TinyAdd;
        impl Circuit<Scalar> for TinyAdd {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
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
                    || "add",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        // IdPublic (needs INST to truly verify; IPA accepts if well formed)
        #[derive(Clone, Default)]
        struct IdPub;
        impl Circuit<Scalar> for IdPub {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(7)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);

        // TinyAdd
        let vk_add: VerifyingKey<Curve> = keygen_vk(&params, &TinyAdd::default()).expect("vk");
        let pk_add = keygen_pk(&params, vk_add.clone(), &TinyAdd::default()).expect("pk");
        let mut t_add = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        halo2_proofs::plonk::create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(
            &params,
            &pk_add,
            &[TinyAdd::default()],
            &[&[][..]],
            OsRng,
            &mut t_add,
        )
        .expect("proof add");
        let p_add = t_add.finalize();

        // IdPub (+INST)
        let vk_id: VerifyingKey<Curve> = keygen_vk(&params, &IdPub::default()).expect("vk");
        let pk_id = keygen_pk(&params, vk_id.clone(), &IdPub::default()).expect("pk");
        let mut t_id = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        halo2_proofs::plonk::create_proof::<
            IPACommitmentScheme<Curve>,
            ProverIPA<'_, Curve>,
            Challenge255<Curve>,
            _,
            _,
            _,
        >(
            &params,
            &pk_id,
            &[IdPub::default()],
            &[&[&[Scalar::from(7u64)][..]][..]],
            OsRng,
            &mut t_id,
        )
        .expect("proof id");
        let p_id = t_id.finalize();

        // ZK1 envelopes
        let mut vk_add_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_add_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_add_env, &vk_add);
        let mut vk_id_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_id_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_id_env, &vk_id);

        let mut pr_add_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut pr_add_env, &p_add);

        let mut pr_id_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut pr_id_env, &p_id);
        zk1::wrap_append_instances_pasta_fp(&[Scalar::from(7u64)], &mut pr_id_env);

        let b_add = "halo2/pasta/ipa/tiny-add";
        let b_id = "halo2/pasta/ipa/tiny-id-public";
        let vk_add_box = VerifyingKeyBox::new(b_add.into(), vk_add_env);
        let vk_id_box = VerifyingKeyBox::new(b_id.into(), vk_id_env);
        let pr_add_box = ProofBox::new(b_add.into(), pr_add_env);
        let pr_id_box = ProofBox::new(b_id.into(), pr_id_env);

        assert!(super::verify_backend(b_add, &pr_add_box, Some(&vk_add_box)));
        assert!(super::verify_backend(b_id, &pr_id_box, Some(&vk_id_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_add_2rows_ipa() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct AddTwoRows;
        impl Circuit<Scalar> for AddTwoRows {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
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
                    || "two_rows",
                    |mut region| {
                        // row 0: 2 + 2 = 4
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a0",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b0",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c0",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        // row 1: 5 + 7 = 12
                        s.enable(&mut region, 1)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a1",
                            a,
                            1,
                            || Value::known(Scalar::from(5)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b1",
                            b,
                            1,
                            || Value::known(Scalar::from(7)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c1",
                            c,
                            1,
                            || Value::known(Scalar::from(12)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &AddTwoRows::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &AddTwoRows::default()).expect("pk");

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
            &[AddTwoRows::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);

        let backend = "halo2/pasta/ipa/tiny-add-2rows";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_add3_ipa() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct AddThree;
        impl Circuit<Scalar> for AddThree {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
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
                    || "add3",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(1)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "d",
                            d,
                            0,
                            || Value::known(Scalar::from(3)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(6)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &AddThree::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &AddThree::default()).expect("pk");

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
            &[AddThree::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);

        let backend = "halo2/pasta/ipa/tiny-add3";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // Chip-backed Poseidon circuits (IPA): commit-open and merkle2
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2-ipa-poseidon",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_poseidon_commit_open_chip_ipa() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        // Params
        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);

        // Keys for Poseidon-backed circuit
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Create proof with 1 public instance (commitment)
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[{
            // Expected commitment as computed in-circuit
            // Constants must match the circuit: rc0=7, rc1=13; MDS [[2,3],[3,5]]; x^5 S-box
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            let s0_v = Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5;
            s0_v
        }][..]]];

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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(insts[0][0], &mut proof_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2-ipa-poseidon",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_poseidon_merkle2_chip_ipa() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);

        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &pasta_tiny::poseidon::Merkle2Poseidon::default()).expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::Merkle2Poseidon::default(),
        )
        .expect("pk");

        // Prepare expected root instance using the same poseidon path as circuit.
        let root = pasta_tiny::poseidon::merkle2_poseidon_sample_root();
        let insts: [&[&[Scalar]]; 1] = [&[&[root][..]]];

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
            &[pasta_tiny::poseidon::Merkle2Poseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(insts[0][0], &mut proof_env);

        let backend = "halo2/pasta/ipa/tiny-merkle2";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // Depth-8 end-to-end checks for vote/transfer circuits (IPA)
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle8_ipa() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &depth::VoteBoolCommitMerkle::<8>::default()).expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &depth::VoteBoolCommitMerkle::<8>::default(),
        )
        .expect("pk");

        // Compute expected [commit, root]
        let v = Scalar::from(1u64);
        let rho = Scalar::from(12345u64);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        // H(v, rho)
        let a = v + rc0;
        let b = rho + rc1;
        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let b2 = b * b;
        let b4 = b2 * b2;
        let b5 = b4 * b;
        let commit = two * a5 + three * b5;
        // chain 8 levels with siblings 20..27 and dir=0
        let mut prev = commit;
        for i in 0..8u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[commit, root]]];
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
            &[depth::VoteBoolCommitMerkle::<8>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        let cols: [&[Scalar]; 2] = [&[commit][..], &[root][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut proof_env);

        let backend = "halo2/pasta/ipa/vote-bool-commit-merkle8";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle8_ipa() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 7u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &depth::AnonTransfer2x2CommitMerkle::<8>::default()).expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &depth::AnonTransfer2x2CommitMerkle::<8>::default(),
        )
        .expect("pk");

        // Witness constants
        let in0 = Scalar::from(7u64);
        let in1 = Scalar::from(5u64);
        let out0 = Scalar::from(6u64);
        let out1 = Scalar::from(6u64);
        let r0 = Scalar::from(11u64);
        let r1 = Scalar::from(13u64);
        let r2 = Scalar::from(17u64);
        let r3 = Scalar::from(19u64);
        let sk = Scalar::from(1_234_567u64);
        let serial = Scalar::from(42u64);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        // h(x,r)
        let h2 = |x: Scalar, r: Scalar| {
            let x2 = x * x;
            let x4 = x2 * x2;
            let x5 = x4 * x;
            let r2 = r * r;
            let r4 = r2 * r2;
            let r5 = r4 * r;
            two * x5 + three * r5 + Scalar::from(7u64)
        };
        let cm_in0 = h2(in0, r0);
        let cm_in1 = h2(in1, r1);
        let cm_out0 = h2(out0, r2);
        let cm_out1 = h2(out1, r3);
        let nf = h2(sk, serial);
        // root from cm_in0 path with siblings 20..27
        let mut prev = cm_in0;
        for i in 0..8u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[cm_in0, cm_in1, cm_out0, cm_out1, nf, root]]];
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
            &[depth::AnonTransfer2x2CommitMerkle::<8>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        let cols: [&[Scalar]; 6] = [
            &[cm_in0][..],
            &[cm_in1][..],
            &[cm_out0][..],
            &[cm_out1][..],
            &[nf][..],
            &[root][..],
        ];
        zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut proof_env);

        let backend = "halo2/pasta/ipa/anon-transfer-2x2-merkle8";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // Poseidon-tagged (runtime-selected) variants: vote/transfer @ depth-8
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle8_poseidon_ipa() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        )
        .expect("pk");

        // Compute expected [commit, root] using the same internal round as the circuit
        let v = Scalar::from(1u64);
        let rho = Scalar::from(12345u64);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        let a = v + rc0;
        let b = rho + rc1;
        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let b2 = b * b;
        let b4 = b2 * b2;
        let b5 = b4 * b;
        let commit = two * a5 + three * b5;
        let mut prev = commit;
        for i in 0..8u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[commit, root]]];
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
            &[poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        let cols: [&[Scalar]; 2] = [&[commit][..], &[root][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut proof_env);

        let backend = backend_tag_vote_bool_commit_merkle(8, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), proof_env);
        assert!(super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_permutation_harness() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Copy)]
        enum Step2 {
            ProfGood,
            ProfBadTrunc,
            I10p2Good,
            I10p2BadShort,
            Unknown(u32),
        }

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        )
        .expect("pk");

        // Compute expected instances
        let v = Scalar::from(1u64);
        let rho = Scalar::from(12345u64);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        let a = v + rc0;
        let b = rho + rc1;
        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let b2 = b * b;
        let b4 = b2 * b2;
        let b5 = b4 * b;
        let commit = two * a5 + three * b5;
        let mut prev = commit;
        for i in 0..8u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[commit, root]]];
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
            &[poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let backend = backend_tag_vote_bool_commit_merkle(8, true);
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let run_case = |steps: &[Step2], ok_expected: bool| {
            let mut prf_env = zk1::wrap_start();
            for s in steps {
                match *s {
                    Step2::ProfGood => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                    Step2::ProfBadTrunc => {
                        prf_env.extend_from_slice(b"PROF");
                        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                        let cut = proof_bytes.len().saturating_sub(1);
                        prf_env.extend_from_slice(&proof_bytes[..cut]);
                    }
                    Step2::I10p2Good => zk1::wrap_append_instances_pasta_fp_cols(
                        &[&[commit][..], &[root][..]],
                        &mut prf_env,
                    ),
                    Step2::I10p2BadShort => {
                        prf_env.extend_from_slice(b"I10P");
                        prf_env.extend_from_slice(&(2u32).to_le_bytes());
                        prf_env.extend_from_slice(&(1u32).to_le_bytes());
                        prf_env.extend_from_slice(&[0u8; 32]);
                    }
                    Step2::Unknown(len) => {
                        prf_env.extend_from_slice(b"UKNW");
                        prf_env.extend_from_slice(&len.to_le_bytes());
                        prf_env.extend_from_slice(&vec![0xCC; len as usize]);
                    }
                }
            }
            let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env.clone());
            let prf_box = ProofBox::new(backend.clone().into(), prf_env);
            assert_eq!(
                super::verify_backend(&backend, &prf_box, Some(&vk_box)),
                ok_expected
            );
        };

        let cases: &[(&[Step2], bool)] = &[
            (&[Step2::ProfGood, Step2::I10p2Good], true),
            (&[Step2::I10p2Good, Step2::ProfGood], true),
            (
                &[Step2::ProfBadTrunc, Step2::ProfGood, Step2::I10p2Good],
                true,
            ),
            (
                &[Step2::ProfGood, Step2::ProfBadTrunc, Step2::I10p2Good],
                false,
            ),
            (
                &[Step2::ProfGood, Step2::I10p2BadShort, Step2::I10p2Good],
                true,
            ),
            (
                &[Step2::ProfGood, Step2::I10p2Good, Step2::I10p2BadShort],
                false,
            ),
            (
                &[
                    Step2::Unknown(0),
                    Step2::ProfGood,
                    Step2::Unknown(3),
                    Step2::I10p2Good,
                ],
                true,
            ),
        ];
        for (steps, ok) in cases {
            run_case(steps, *ok);
        }
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_malformed_inst() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        )
        .expect("pk");

        // Produce a valid proof with no instances for malformed INST test
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
            &[poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: VK with IPAK; proof with malformed I10P (declares 2 cols, provides 1 value)
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        prf_env.extend_from_slice(b"I10P");
        prf_env.extend_from_slice(&(2u32).to_le_bytes()); // cols
        prf_env.extend_from_slice(&(1u32).to_le_bytes()); // rows
        prf_env.extend_from_slice(&[0u8; 32]); // only 1 scalar appended

        let backend = backend_tag_vote_bool_commit_merkle(8, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle8_poseidon_ipa_zk1_truncated_prof() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default(),
        )
        .expect("pk");
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
            &[poseidon_depth::VoteBoolCommitMerklePoseidon::<8>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        // Truncated PROF TLV
        let mut prf_env = zk1::wrap_start();
        prf_env.extend_from_slice(b"PROF");
        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
        let cut = proof_bytes.len().saturating_sub(1);
        prf_env.extend_from_slice(&proof_bytes[..cut]);

        let backend = backend_tag_vote_bool_commit_merkle(8, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        )
        .expect("pk");

        // Expected instances
        let v = Scalar::from(1u64);
        let rho = Scalar::from(12345u64);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        let a = v + rc0;
        let b = rho + rc1;
        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let b2 = b * b;
        let b4 = b2 * b2;
        let b5 = b4 * b;
        let commit = two * a5 + three * b5;
        let mut prev = commit;
        for i in 0..16u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        // Create proof
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[commit, root]]];
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
            &[poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        let cols: [&[Scalar]; 2] = [&[commit][..], &[root][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut prf_env);

        let backend = backend_tag_vote_bool_commit_merkle(16, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 7u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        )
        .expect("pk");

        let in0 = Scalar::from(7u64);
        let in1 = Scalar::from(5u64);
        let out0 = Scalar::from(6u64);
        let out1 = Scalar::from(6u64);
        let r0 = Scalar::from(11u64);
        let r1 = Scalar::from(13u64);
        let r2 = Scalar::from(17u64);
        let r3 = Scalar::from(19u64);
        let sk = Scalar::from(1_234_567u64);
        let serial = Scalar::from(42u64);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        let h2 = |x: Scalar, r: Scalar| {
            let x2 = x * x;
            let x4 = x2 * x2;
            let x5 = x4 * x;
            let r2 = r * r;
            let r4 = r2 * r2;
            let r5 = r4 * r;
            two * x5 + three * r5 + Scalar::from(7u64)
        };
        let cm_in0 = h2(in0, r0);
        let cm_in1 = h2(in1, r1);
        let cm_out0 = h2(out0, r2);
        let cm_out1 = h2(out1, r3);
        let nf = h2(sk, serial);
        let mut prev = cm_in0;
        for i in 0..16u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[cm_in0, cm_in1, cm_out0, cm_out1, nf, root]]];
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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        let cols: [&[Scalar]; 6] = [
            &[cm_in0][..],
            &[cm_in1][..],
            &[cm_out0][..],
            &[cm_out1][..],
            &[nf][..],
            &[root][..],
        ];
        zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut prf_env);

        let backend = backend_tag_anon_transfer_merkle(16, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_malformed_inst() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        )
        .expect("pk");

        // Produce a valid proof (instances irrelevant for malformed INST test)
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
            &[poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 VK (IPAK)
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        // ZK1 Proof with malformed I10P: declare 2 cols, 1 row but provide only 1 value
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        prf_env.extend_from_slice(b"I10P");
        prf_env.extend_from_slice(&(2u32).to_le_bytes()); // cols
        prf_env.extend_from_slice(&(1u32).to_le_bytes()); // rows
        // append only one 32-byte scalar (all zero) — insufficient for 2 cols
        prf_env.extend_from_slice(&[0u8; 32]);

        let backend = backend_tag_vote_bool_commit_merkle(16, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_truncated_prof() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        )
        .expect("pk");
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
            &[poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK: ZK1 IPAK
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        // Proof: ZK1 with PROF TLV declaring full length but payload truncated
        let mut prf_env = zk1::wrap_start();
        prf_env.extend_from_slice(b"PROF");
        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
        // append one byte less than declared
        let cut = proof_bytes.len().saturating_sub(1);
        prf_env.extend_from_slice(&proof_bytes[..cut]);

        let backend = backend_tag_vote_bool_commit_merkle(16, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_noncanonical() {
        use ff::PrimeField as _;
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 7u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        )
        .expect("pk");

        // Produce a valid proof
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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK: ZK1 IPAK
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        // Proof: ZK1 PROF + I10P with non-canonical scalar among 6 values
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        prf_env.extend_from_slice(b"I10P");
        prf_env.extend_from_slice(&(6u32).to_le_bytes()); // cols
        prf_env.extend_from_slice(&(1u32).to_le_bytes()); // rows
        // Append 5 valid zero scalars and one non-canonical 0xFF.. value
        for _ in 0..5 {
            prf_env.extend_from_slice(Scalar::ZERO.to_repr().as_ref());
        }
        prf_env.extend_from_slice(&[0xFFu8; 32]); // non-canonical

        let backend = backend_tag_anon_transfer_merkle(16, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_invalid_header() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 7u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        )
        .expect("pk");
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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK: ZK1 IPAK
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        // Proof: ZK1 with PROF ok, I10P invalid header (cols=0)
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        prf_env.extend_from_slice(b"I10P");
        prf_env.extend_from_slice(&(0u32).to_le_bytes()); // invalid cols=0
        prf_env.extend_from_slice(&(1u32).to_le_bytes()); // rows=1

        let backend = backend_tag_anon_transfer_merkle(16, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle8_poseidon_ipa_zk1_permutation_harness() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Copy)]
        enum Step6 {
            ProfGood,
            ProfBadTrunc,
            I10p6Good,
            I10p6BadShort,
            Unknown(u32),
        }

        let k = 7u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default(),
        )
        .expect("pk");

        let in0 = Scalar::from(7u64);
        let in1 = Scalar::from(5u64);
        let out0 = Scalar::from(6u64);
        let out1 = Scalar::from(6u64);
        let r0 = Scalar::from(11u64);
        let r1 = Scalar::from(13u64);
        let r2 = Scalar::from(17u64);
        let r3 = Scalar::from(19u64);
        let sk = Scalar::from(1_234_567u64);
        let serial = Scalar::from(42u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        let h2 = |x: Scalar, r: Scalar| {
            let x2 = x * x;
            let x4 = x2 * x2;
            let x5 = x4 * x;
            let r2 = r * r;
            let r4 = r2 * r2;
            let r5 = r4 * r;
            two * x5 + three * r5 + Scalar::from(7u64)
        };
        let cm_in0 = h2(in0, r0);
        let cm_in1 = h2(in1, r1);
        let cm_out0 = h2(out0, r2);
        let cm_out1 = h2(out1, r3);
        let nf = h2(sk, serial);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let mut prev = cm_in0;
        for i in 0..8u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[cm_in0, cm_in1, cm_out0, cm_out1, nf, root]]];
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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let backend = backend_tag_anon_transfer_merkle(8, true);
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let run_case = |steps: &[Step6], ok_expected: bool| {
            let mut prf_env = zk1::wrap_start();
            for s in steps {
                match *s {
                    Step6::ProfGood => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                    Step6::ProfBadTrunc => {
                        prf_env.extend_from_slice(b"PROF");
                        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                        let cut = proof_bytes.len().saturating_sub(1);
                        prf_env.extend_from_slice(&proof_bytes[..cut]);
                    }
                    Step6::I10p6Good => zk1::wrap_append_instances_pasta_fp_cols(
                        &[
                            &[cm_in0][..],
                            &[cm_in1][..],
                            &[cm_out0][..],
                            &[cm_out1][..],
                            &[nf][..],
                            &[root][..],
                        ],
                        &mut prf_env,
                    ),
                    Step6::I10p6BadShort => {
                        prf_env.extend_from_slice(b"I10P");
                        prf_env.extend_from_slice(&(6u32).to_le_bytes());
                        prf_env.extend_from_slice(&(1u32).to_le_bytes());
                        prf_env.extend_from_slice(&[0u8; 32 * 3]);
                    }
                    Step6::Unknown(len) => {
                        prf_env.extend_from_slice(b"UKNW");
                        prf_env.extend_from_slice(&len.to_le_bytes());
                        prf_env.extend_from_slice(&vec![0x99; len as usize]);
                    }
                }
            }
            let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env.clone());
            let prf_box = ProofBox::new(backend.clone().into(), prf_env);
            assert_eq!(
                super::verify_backend(&backend, &prf_box, Some(&vk_box)),
                ok_expected
            );
        };

        let cases: &[(&[Step6], bool)] = &[
            (&[Step6::ProfGood, Step6::I10p6Good], true),
            (&[Step6::I10p6Good, Step6::ProfGood], true),
            (
                &[Step6::ProfBadTrunc, Step6::ProfGood, Step6::I10p6Good],
                true,
            ),
            (
                &[Step6::ProfGood, Step6::ProfBadTrunc, Step6::I10p6Good],
                false,
            ),
            (
                &[Step6::ProfGood, Step6::I10p6BadShort, Step6::I10p6Good],
                true,
            ),
            (
                &[Step6::ProfGood, Step6::I10p6Good, Step6::I10p6BadShort],
                false,
            ),
            (
                &[
                    Step6::Unknown(0),
                    Step6::ProfGood,
                    Step6::Unknown(3),
                    Step6::I10p6Good,
                ],
                true,
            ),
        ];
        for (steps, ok) in cases {
            run_case(steps, *ok);
        }
    }

    // Minimal randomized harness for depth-16 (ZK1). Few cases to keep runtime reasonable.
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_commit_merkle16_poseidon_ipa_zk1_randomized_min() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Copy)]
        enum S {
            Prof,
            ProfTrunc,
            I2,
            I2Short,
            Uk(u32),
        }

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default(),
        )
        .expect("pk");

        // Expected [commit, root]
        let v = Scalar::from(1u64);
        let rho = Scalar::from(12345u64);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        let a = v + rc0;
        let b = rho + rc1;
        let a2 = a * a;
        let a4 = a2 * a2;
        let a5 = a4 * a;
        let b2 = b * b;
        let b4 = b2 * b2;
        let b5 = b4 * b;
        let commit = two * a5 + three * b5;
        let mut prev = commit;
        for i in 0..16u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[commit, root]]];
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
            &[poseidon_depth::VoteBoolCommitMerklePoseidon::<16>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let backend = backend_tag_vote_bool_commit_merkle(16, true);
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        // Deterministic small random-ish scenarios
        let scenarios: &[(&[S], bool)] = &[
            (&[S::Prof, S::I2], true),
            (&[S::Uk(3), S::Prof, S::Uk(7), S::I2], true),
            (&[S::ProfTrunc, S::Uk(2), S::Prof, S::I2], true),
            (&[S::Prof, S::I2Short], false),
            (&[S::ProfTrunc, S::I2], false),
        ];

        let run = |steps: &[S], ok: bool| {
            let mut prf_env = zk1::wrap_start();
            for s in steps {
                match *s {
                    S::Prof => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                    S::ProfTrunc => {
                        prf_env.extend_from_slice(b"PROF");
                        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                        let cut = proof_bytes.len().saturating_sub(1);
                        prf_env.extend_from_slice(&proof_bytes[..cut]);
                    }
                    S::I2 => zk1::wrap_append_instances_pasta_fp_cols(
                        &[&[commit][..], &[root][..]],
                        &mut prf_env,
                    ),
                    S::I2Short => {
                        prf_env.extend_from_slice(b"I10P");
                        prf_env.extend_from_slice(&(2u32).to_le_bytes());
                        prf_env.extend_from_slice(&(1u32).to_le_bytes());
                        prf_env.extend_from_slice(&[0u8; 32]);
                    }
                    S::Uk(n) => {
                        prf_env.extend_from_slice(b"UNKN");
                        prf_env.extend_from_slice(&n.to_le_bytes());
                        prf_env.extend_from_slice(&vec![0xAD; n as usize]);
                    }
                }
            }
            let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env.clone());
            let prf_box = ProofBox::new(backend.clone().into(), prf_env);
            assert_eq!(super::verify_backend(&backend, &prf_box, Some(&vk_box)), ok);
        };

        for (steps, ok) in scenarios {
            run(steps, *ok);
        }
    }

    // ZK1 permutation harness for depth-16 anon (6-column), trimmed cases
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle16_poseidon_ipa_zk1_permutation_harness() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Copy)]
        enum S6 {
            Prof,
            ProfTrunc,
            I6,
            I6Short,
            Uk(u32),
        }

        let k = 7u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default(),
        )
        .expect("pk");

        // Expected instances
        let in0 = Scalar::from(7u64);
        let in1 = Scalar::from(5u64);
        let out0 = Scalar::from(6u64);
        let out1 = Scalar::from(6u64);
        let r0 = Scalar::from(11u64);
        let r1 = Scalar::from(13u64);
        let r2 = Scalar::from(17u64);
        let r3 = Scalar::from(19u64);
        let sk = Scalar::from(1_234_567u64);
        let serial = Scalar::from(42u64);
        let two = Scalar::from(2u64);
        let three = Scalar::from(3u64);
        let h2 = |x: Scalar, r: Scalar| {
            let x2 = x * x;
            let x4 = x2 * x2;
            let x5 = x4 * x;
            let r2 = r * r;
            let r4 = r2 * r2;
            let r5 = r4 * r;
            two * x5 + three * r5 + Scalar::from(7u64)
        };
        let cm_in0 = h2(in0, r0);
        let cm_in1 = h2(in1, r1);
        let cm_out0 = h2(out0, r2);
        let cm_out1 = h2(out1, r3);
        let nf = h2(sk, serial);
        let rc0 = Scalar::from(7u64);
        let rc1 = Scalar::from(13u64);
        let mut prev = cm_in0;
        for i in 0..16u64 {
            let sib = Scalar::from(20 + i);
            let t0 = prev + rc0;
            let t1 = sib + rc1;
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            prev = two * t0_5 + three * t1_5;
        }
        let root = prev;

        // Build proof
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[cm_in0, cm_in1, cm_out0, cm_out1, nf, root]]];
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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<16>::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let backend = backend_tag_anon_transfer_merkle(16, true);
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        // Trimmed scenarios
        let scenarios: &[(&[S6], bool)] = &[
            (&[S6::Prof, S6::I6], true),
            (&[S6::Uk(4), S6::Prof, S6::Uk(8), S6::I6], true),
            (&[S6::ProfTrunc, S6::I6], false),
            (&[S6::Prof, S6::I6Short], false),
        ];

        let run = |steps: &[S6], ok: bool| {
            let mut prf_env = zk1::wrap_start();
            for s in steps {
                match *s {
                    S6::Prof => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                    S6::ProfTrunc => {
                        prf_env.extend_from_slice(b"PROF");
                        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                        let cut = proof_bytes.len().saturating_sub(1);
                        prf_env.extend_from_slice(&proof_bytes[..cut]);
                    }
                    S6::I6 => zk1::wrap_append_instances_pasta_fp_cols(
                        &[
                            &[cm_in0][..],
                            &[cm_in1][..],
                            &[cm_out0][..],
                            &[cm_out1][..],
                            &[nf][..],
                            &[root][..],
                        ],
                        &mut prf_env,
                    ),
                    S6::I6Short => {
                        prf_env.extend_from_slice(b"I10P");
                        prf_env.extend_from_slice(&(6u32).to_le_bytes());
                        prf_env.extend_from_slice(&(1u32).to_le_bytes());
                        prf_env.extend_from_slice(&[0u8; 32 * 3]);
                    }
                    S6::Uk(n) => {
                        prf_env.extend_from_slice(b"UKN2");
                        prf_env.extend_from_slice(&n.to_le_bytes());
                        prf_env.extend_from_slice(&vec![0x5A; n as usize]);
                    }
                }
            }
            let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env.clone());
            let prf_box = ProofBox::new(backend.clone().into(), prf_env);
            assert_eq!(super::verify_backend(&backend, &prf_box, Some(&vk_box)), ok);
        };

        for (steps, ok) in scenarios {
            run(steps, *ok);
        }
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle8_poseidon_ipa_zk1_noncanonical() {
        use ff::PrimeField as _;
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 7u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default(),
        )
        .expect("pk");

        // Valid proof
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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: VK with IPAK; proof with I10P (6 cols) where one scalar is non-canonical (0xFF..)
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        prf_env.extend_from_slice(b"I10P");
        prf_env.extend_from_slice(&(6u32).to_le_bytes()); // cols
        prf_env.extend_from_slice(&(1u32).to_le_bytes()); // rows
        // Append 5 zeros, then one non-canonical
        for _ in 0..5 {
            prf_env.extend_from_slice(Scalar::ZERO.to_repr().as_ref());
        }
        prf_env.extend_from_slice(&[0xFFu8; 32]);

        let backend = backend_tag_anon_transfer_merkle(8, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_2x2_merkle8_poseidon_ipa_zk1_invalid_header() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 7u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default(),
        )
        .expect("pk");
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
            &[poseidon_depth::AnonTransfer2x2CommitMerklePoseidon::<8>::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        // PROF ok, I10P invalid (rows=0)
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        prf_env.extend_from_slice(b"I10P");
        prf_env.extend_from_slice(&(1u32).to_le_bytes()); // cols=1
        prf_env.extend_from_slice(&(0u32).to_le_bytes()); // rows=0 (invalid)

        let backend = backend_tag_anon_transfer_merkle(8, true);
        let vk_box = VerifyingKeyBox::new(backend.clone().into(), vk_env);
        let prf_box = ProofBox::new(backend.clone().into(), prf_env);
        assert!(!super::verify_backend(&backend, &prf_box, Some(&vk_box)));
    }

    // --- Tiny Poseidon circuits (base) negative ZK1 tests ---

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_truncated_prof() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK: ZK1 IPAK; Proof: truncated PROF
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        prf_env.extend_from_slice(b"PROF");
        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
        let cut = proof_bytes.len().saturating_sub(1);
        prf_env.extend_from_slice(&proof_bytes[..cut]);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_merkle2_ipa_zk1_invalid_header_extreme() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &pasta_tiny::poseidon::Merkle2Poseidon::default()).expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::Merkle2Poseidon::default(),
        )
        .expect("pk");

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
            &[pasta_tiny::poseidon::Merkle2Poseidon::default()],
            &[&[]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK: ZK1 IPAK; Proof: PROF ok, I10P with extreme cols/rows beyond caps
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        // Case 1: cols > MAX_INST_COLS
        let mut prf_env1 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env1, &proof_bytes);
        prf_env1.extend_from_slice(b"I10P");
        let cols_ext = (super::MAX_INST_COLS as u32).saturating_add(1);
        prf_env1.extend_from_slice(&cols_ext.to_le_bytes());
        prf_env1.extend_from_slice(&(1u32).to_le_bytes());
        // No data appended (should fail on header alone)

        let backend = "halo2/pasta/ipa/tiny-merkle2";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env.clone());
        let prf_box1 = ProofBox::new(backend.into(), prf_env1);
        assert!(!super::verify_backend(backend, &prf_box1, Some(&vk_box)));

        // Case 2: rows > MAX_INST_ROWS
        let mut prf_env2 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env2, &proof_bytes);
        prf_env2.extend_from_slice(b"I10P");
        prf_env2.extend_from_slice(&(1u32).to_le_bytes());
        let rows_ext = (super::MAX_INST_ROWS as u32).saturating_add(1);
        prf_env2.extend_from_slice(&rows_ext.to_le_bytes());

        let prf_box2 = ProofBox::new(backend.into(), prf_env2);
        assert!(!super::verify_backend(backend, &prf_box2, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_positive() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Create proof with expected commitment as instance (computed in-circuit)
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[{
            // Same commit as in circuit synthesize: m=11, r=31; Pow5 compressor
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        }][..]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: VK IPAK; Proof PROF + I10P with 1 col (commit)
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        let col: [&[Scalar]; 1] = [&[insts[0][0][0]][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_merkle2_ipa_zk1_positive() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &pasta_tiny::poseidon::Merkle2Poseidon::default()).expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::Merkle2Poseidon::default(),
        )
        .expect("pk");

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let root = pasta_tiny::poseidon::merkle2_poseidon_sample_root();
        let insts: [&[&[Scalar]]; 1] = [&[&[root]]];
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
            &[pasta_tiny::poseidon::Merkle2Poseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        let col: [&[Scalar]; 1] = [&[root][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-merkle2";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // ZK1 tag order tolerance: multiple PROF (last wins) and unknown tags are ignored.
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_multiple_prof_and_unknown_ok() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Create proof with 1 public instance
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[{
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        }][..]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: multiple PROF (first truncated), unknown TLV in between, then correct PROF
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        // PROF #1 truncated
        prf_env.extend_from_slice(b"PROF");
        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
        let cut = proof_bytes.len().saturating_sub(1);
        prf_env.extend_from_slice(&proof_bytes[..cut]);
        // Unknown TLV
        prf_env.extend_from_slice(b"UKNW");
        prf_env.extend_from_slice(&(4u32).to_le_bytes());
        prf_env.extend_from_slice(&[1, 2, 3, 4]);
        // PROF #2 correct
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        // Instances (1 col)
        let col: [&[Scalar]; 1] = [&[insts[0][0][0]][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_unknown_tlv_stress_ok() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Proof with 1 instance (commit)
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[{
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        }][..]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: many unknown TLVs interleaved, then valid PROF + I10P
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        // Unknown 0-len
        prf_env.extend_from_slice(b"U0__");
        prf_env.extend_from_slice(&(0u32).to_le_bytes());
        // Unknown 1-len
        prf_env.extend_from_slice(b"U1__");
        prf_env.extend_from_slice(&(1u32).to_le_bytes());
        prf_env.extend_from_slice(&[0xAB]);
        // Unknown 4-len
        prf_env.extend_from_slice(b"U4__");
        prf_env.extend_from_slice(&(4u32).to_le_bytes());
        prf_env.extend_from_slice(&[1, 2, 3, 4]);
        // Unknown 17-len
        prf_env.extend_from_slice(b"UQ__");
        prf_env.extend_from_slice(&(17u32).to_le_bytes());
        prf_env.extend_from_slice(&[0x55; 17]);
        // Unknown 256-len
        prf_env.extend_from_slice(b"UB__");
        prf_env.extend_from_slice(&(256u32).to_le_bytes());
        prf_env.extend_from_slice(&vec![0x42; 256]);
        // Valid PROF
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        // Instances
        let col: [&[Scalar]; 1] = [&[insts[0][0][0]][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // ZK1 duplicate I10P policy: last wins (accept when last is correct)
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_duplicate_i10p_last_wins() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Create proof with expected commitment as instance
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let commit = {
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        };
        let insts: [&[&[Scalar]]; 1] = [&[&[commit]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: duplicate I10P — first wrong, second correct → accept (last wins)
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        // I10P #1 (wrong commit = 0)
        zk1::wrap_append_instances_pasta_fp_cols(&[&[Scalar::ZERO][..]], &mut prf_env);
        // I10P #2 (correct)
        zk1::wrap_append_instances_pasta_fp_cols(&[&[commit][..]], &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // ZK1 duplicate I10P policy: last wins (reject when last is wrong)
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_duplicate_i10p_last_wrong_rejects() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let commit = {
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        };
        let insts: [&[&[Scalar]]; 1] = [&[&[commit]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1: duplicate I10P — first correct, second wrong → reject
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
        // I10P #1 correct
        zk1::wrap_append_instances_pasta_fp_cols(&[&[commit][..]], &mut prf_env);
        // I10P #2 wrong
        zk1::wrap_append_instances_pasta_fp_cols(&[&[Scalar::ZERO][..]], &mut prf_env);

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    // Randomized (deterministic) unknown TLV stress around PROF/I10P → accept
    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_unknown_tlv_randomized_ok() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Proof with 1 instance
        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let commit = {
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        };
        let insts: [&[&[Scalar]]; 1] = [&[&[commit]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // Deterministic PRNG
        let mut seed: u64 = 0xC0FFEE_F00D_BAAD;
        for _round in 0..4 {
            let mut vk_env = zk1::wrap_start();
            zk1::wrap_append_ipa_k(&mut vk_env, k);
            zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
            let mut prf_env = zk1::wrap_start();
            // Generate N unknown TLVs with varying lengths ≤ 64
            let n = (seed as usize % 5) + 1;
            for i in 0..n {
                // xorshift64*
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let len = (seed as usize % 64) as u32;
                let tag = [
                    b'U',
                    b'A' + (i as u8 % 26),
                    b'0' + ((seed as u8) % 10),
                    b'X',
                ];
                prf_env.extend_from_slice(&tag);
                prf_env.extend_from_slice(&len.to_le_bytes());
                prf_env.extend_from_slice(&vec![0xA5; len as usize]);
            }
            // Valid PROF + I10P
            zk1::wrap_append_proof(&mut prf_env, &proof_bytes);
            let col: [&[Scalar]; 1] = [&[commit][..]];
            zk1::wrap_append_instances_pasta_fp_cols(&col, &mut prf_env);

            let backend = "halo2/pasta/ipa/tiny-commit-open";
            let vk_box = VerifyingKeyBox::new(backend.into(), vk_env.clone());
            let prf_box = ProofBox::new(backend.into(), prf_env);
            assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
        }
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_tiny_commit_open_ipa_zk1_permutation_harness() {
        use halo2_proofs::{
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{VerifyingKey, keygen_pk, keygen_vk},
            poly::commitment::Params as _,
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Copy)]
        enum Step {
            ProfGood,
            ProfBadTrunc,
            I10pGood,
            I10pBadShort,
            Unknown(u32),
        }

        // Prepare circuit, proof and expected instance (commit)
        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(
            &params,
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("vk");
        let pk = keygen_pk(
            &params,
            vk_h2.clone(),
            &pasta_tiny::poseidon::CommitOpenPoseidon::default(),
        )
        .expect("pk");

        // Expected commitment (same as in synthesize)
        let commit = {
            let m = Scalar::from(11u64);
            let r = Scalar::from(31u64);
            let t0 = m + Scalar::from(7u64);
            let t1 = r + Scalar::from(13u64);
            let t0_2 = t0 * t0;
            let t0_4 = t0_2 * t0_2;
            let t0_5 = t0_4 * t0;
            let t1_2 = t1 * t1;
            let t1_4 = t1_2 * t1_2;
            let t1_5 = t1_4 * t1;
            Scalar::from(2u64) * t0_5 + Scalar::from(3u64) * t1_5
        };

        let mut transcript = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
        let insts: [&[&[Scalar]]; 1] = [&[&[commit]]];
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
            &[pasta_tiny::poseidon::CommitOpenPoseidon::default()],
            &insts,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        let backend = "halo2/pasta/ipa/tiny-commit-open";
        let mut vk_env_base = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env_base, k);
        zk1::wrap_append_vk_pasta(&mut vk_env_base, &vk_h2);

        // Helper to build envelope from a sequence of steps and check expected outcome
        let run_case = |steps: &[Step], expect_ok: bool| {
            let mut prf_env = zk1::wrap_start();
            for s in steps {
                match *s {
                    Step::ProfGood => zk1::wrap_append_proof(&mut prf_env, &proof_bytes),
                    Step::ProfBadTrunc => {
                        prf_env.extend_from_slice(b"PROF");
                        prf_env.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
                        let cut = proof_bytes.len().saturating_sub(1);
                        prf_env.extend_from_slice(&proof_bytes[..cut]);
                    }
                    Step::I10pGood => {
                        zk1::wrap_append_instances_pasta_fp_cols(&[&[commit][..]], &mut prf_env)
                    }
                    Step::I10pBadShort => {
                        prf_env.extend_from_slice(b"I10P");
                        prf_env.extend_from_slice(&(1u32).to_le_bytes());
                        prf_env.extend_from_slice(&(1u32).to_le_bytes());
                        // no scalar payload -> short
                    }
                    Step::Unknown(len) => {
                        prf_env.extend_from_slice(b"UKNW");
                        prf_env.extend_from_slice(&len.to_le_bytes());
                        prf_env.extend_from_slice(&vec![0xEE; len as usize]);
                    }
                }
            }
            let vk_box = VerifyingKeyBox::new(backend.into(), vk_env_base.clone());
            let prf_box = ProofBox::new(backend.into(), prf_env);
            assert_eq!(
                super::verify_backend(backend, &prf_box, Some(&vk_box)),
                expect_ok
            );
        };

        // Cases
        let cases: &[(&[Step], bool)] = &[
            (&[Step::ProfGood, Step::I10pGood], true),
            (&[Step::I10pGood, Step::ProfGood], true),
            (&[Step::ProfBadTrunc, Step::ProfGood, Step::I10pGood], true),
            (&[Step::ProfGood, Step::ProfBadTrunc, Step::I10pGood], false),
            (&[Step::ProfGood, Step::I10pBadShort, Step::I10pGood], true),
            (&[Step::ProfGood, Step::I10pGood, Step::I10pBadShort], false),
            (
                &[
                    Step::Unknown(0),
                    Step::ProfGood,
                    Step::Unknown(3),
                    Step::I10pGood,
                ],
                true,
            ),
            (
                &[Step::Unknown(8), Step::ProfBadTrunc, Step::I10pGood],
                false,
            ),
            (
                &[Step::ProfGood, Step::Unknown(16), Step::I10pBadShort],
                false,
            ),
            (
                &[Step::I10pGood, Step::ProfBadTrunc, Step::Unknown(1)],
                false,
            ),
        ];

        for (steps, expect_ok) in cases {
            run_case(steps, *expect_ok);
        }
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_zk1_prof_length_exceeds_cap_rejected() {
        // Build minimal ZK1 with PROF len > MAX_PROOF_LEN. Parser must reject.
        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2 =
            keygen_vk_cached("halo2/pasta/ipa/tiny-add", &params, &pasta_tiny::Add).expect("vk");
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut prf_env = zk1::wrap_start();
        prf_env.extend_from_slice(b"PROF");
        let too_big = (super::MAX_PROOF_LEN as u32).saturating_add(1);
        prf_env.extend_from_slice(&too_big.to_le_bytes());
        // no payload appended

        let backend = "halo2/pasta/ipa/tiny-add"; // any recognized halo2 backend tag
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), prf_env);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_add2inst_public_ipa() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct AddTwoInstPublic;
        impl Circuit<Scalar> for AddTwoInstPublic {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
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
                let i0 = meta.instance_column();
                let i1 = meta.instance_column();
                let s = meta.selector();
                meta.create_gate("add2inst_pub", |meta| {
                    let s = meta.query_selector(s);
                    let a = meta.query_advice(a, Rotation::cur());
                    let b = meta.query_advice(b, Rotation::cur());
                    let c = meta.query_advice(c, Rotation::cur());
                    let inst0 = meta.query_instance(i0, Rotation::cur());
                    let inst1 = meta.query_instance(i1, Rotation::cur());
                    vec![
                        s.clone() * (a.clone() + b.clone() - c),
                        s.clone() * (a - inst0),
                        s * (b - inst1),
                    ]
                });
                (a, b, c, i0, i1, s)
            }
            fn synthesize(
                &self,
                (a, b, c, _i0, _i1, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "add2inst_pub",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(5)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(8)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(13)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> =
            keygen_vk(&params, &AddTwoInstPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &AddTwoInstPublic::default()).expect("pk");

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
            &[AddTwoInstPublic::default()],
            &[&[&[Scalar::from(5u64)][..], &[Scalar::from(8u64)][..]]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        let cols: [&[Scalar]; 2] = [&[Scalar::from(5u64)][..], &[Scalar::from(8u64)][..]];
        zk1::wrap_append_instances_pasta_fp_cols(&cols, &mut proof_env);

        let backend = "halo2/pasta/ipa/tiny-add2inst-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_anon_transfer_ipa() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct AT;
        impl Circuit<Scalar> for AT {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
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
                meta.create_gate("anon_transfer", |meta| {
                    let s = meta.query_selector(s);
                    let in0 = meta.query_advice(in0, Rotation::cur());
                    let in1 = meta.query_advice(in1, Rotation::cur());
                    let out0 = meta.query_advice(out0, Rotation::cur());
                    let out1 = meta.query_advice(out1, Rotation::cur());
                    vec![s * (in0 + in1 - (out0 + out1))]
                });
                (in0, in1, out0, out1, s)
            }
            fn synthesize(
                &self,
                (in0, in1, out0, out1, s): Self::Config,
                mut layouter: impl Layouter<Scalar>,
            ) -> Result<(), PlonkError> {
                layouter.assign_region(
                    || "anon_transfer",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "in0",
                            in0,
                            0,
                            || Value::known(Scalar::from(7)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "in1",
                            in1,
                            0,
                            || Value::known(Scalar::from(5)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "out0",
                            out0,
                            0,
                            || Value::known(Scalar::from(6)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "out1",
                            out1,
                            0,
                            || Value::known(Scalar::from(6)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 6u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &AT::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &AT::default()).expect("pk");

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
            &[AT::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);

        let backend = "halo2/pasta/ipa/tiny-anon-transfer-2x2";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_vote_bool_ipa() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct VoteBool;
        impl Circuit<Scalar> for VoteBool {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Selector,
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "v",
                            v,
                            0,
                            || Value::known(Scalar::from(1u64)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &VoteBool::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &VoteBool::default()).expect("pk");

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
            &[VoteBool::default()],
            &[&[][..]],
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);

        let backend = "halo2/pasta/ipa/tiny-vote-bool";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_id_public_ipa_with_and_without_inst() {
        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct IdPub;
        impl Circuit<Scalar> for IdPub {
            type Config = (
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Advice>,
                halo2_proofs::plonk::Column<halo2_proofs::plonk::Instance>,
                halo2_proofs::plonk::Selector,
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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(7)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &IdPub::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &IdPub::default()).expect("pk");

        let mut t = Blake2bWrite::<_, Curve, Challenge255<Curve>>::init(vec![]);
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
            &[IdPub::default()],
            &[&[&[Scalar::from(7u64)][..]][..]],
            OsRng,
            &mut t,
        )
        .expect("proof created");
        let proof_bytes = t.finalize();

        // ZK1 envelopes
        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);

        let backend = "halo2/pasta/ipa/tiny-id-public";
        // Case 1: Missing INST → must fail
        let mut proof_env1 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env1, &proof_bytes);
        let vk_box1 = VerifyingKeyBox::new(backend.into(), vk_env.clone());
        let prf_box1 = ProofBox::new(backend.into(), proof_env1);
        assert!(!super::verify_backend(backend, &prf_box1, Some(&vk_box1)));

        // Case 2: With INST → should succeed
        let mut proof_env2 = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env2, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(&[Scalar::from(7u64)], &mut proof_env2);
        let vk_box2 = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box2 = ProofBox::new(backend.into(), proof_env2);
        assert!(super::verify_backend(backend, &prf_box2, Some(&vk_box2)));
    }

    #[cfg(all(
        feature = "zk-halo2-ipa",
        feature = "zk-halo2",
        feature = "zk-halo2-ipa-poseidon"
    ))]
    #[test]
    fn halo2_verify_with_instance_add_ipa() {
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::Rotation,
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        // ZK1 envelopes
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

        let mut vk_env = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_env, k);
        zk1::wrap_append_vk_pasta(&mut vk_env, &vk_h2);
        let mut proof_env = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_env, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut proof_env);

        let backend = "halo2/pasta/ipa/tiny-add-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_env);
        let prf_box = ProofBox::new(backend.into(), proof_env);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_with_instance_mul_kzg() {
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

        #[derive(Clone, Default)]
        struct TinyMulPublic;
        impl Circuit<Scalar> for TinyMulPublic {
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
                    || "tiny_pub",
                    |mut region| {
                        s.enable(&mut region, 0)?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(3)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(6)),
                        )?;
                        Ok(())
                    },
                )
            }
        }

        // Params and keys
        let k = 5u32;
        let params: PastaParams = pasta_params_new(k);
        let vk_h2: VerifyingKey<Curve> = keygen_vk(&params, &TinyMulPublic::default()).expect("vk");
        let pk = keygen_pk(&params, vk_h2.clone(), &TinyMulPublic::default()).expect("pk");

        // Instance: public value 6
        let inst_col = vec![Scalar::from(6u64)];
        let inst_cols: Vec<&[Scalar]> = vec![inst_col.as_slice()];
        let inst_proofs: Vec<&[&[Scalar]]> = vec![inst_cols.as_slice()];

        // Create proof
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
            &[TinyMulPublic::default()],
            &inst_proofs,
            OsRng,
            &mut transcript,
        )
        .expect("proof created");
        let proof_bytes = transcript.finalize();

        // VK container
        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        // Proof + INST
        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
        zk1::wrap_append_instances_pasta_fp(inst_col.as_slice(), &mut proof_container);

        let backend = "halo2/pasta/tiny-mul-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_with_instance_malformed_length_kzg() {
        // Generate a valid proof for add-public, but craft INST with wrong lengths
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
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

        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
        // Malformed INST: declare 2 columns, 1 row but provide only 1 value (should fail)
        proof_container.extend_from_slice(b"I10P");
        proof_container.extend_from_slice(&(2u32).to_le_bytes());
        proof_container.extend_from_slice(&(1u32).to_le_bytes());
        proof_container.extend_from_slice(inst_col[0].to_repr().as_ref());

        let backend = "halo2/pasta/tiny-add-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }

    #[cfg(feature = "zk-halo2")]
    #[test]
    fn halo2_verify_with_instance_noncanonical_kzg() {
        // Generate a valid proof, but use non-canonical instance encoding (all 0xFF)
        use std::io::Cursor;

        use halo2_proofs::{
            circuit::{Layouter, SimpleFloorPlanner, Value},
            halo2curves::pasta::{EqAffine as Curve, Fp as Scalar},
            plonk::{
                Circuit, ConstraintSystem, Error as PlonkError, VerifyingKey, keygen_pk, keygen_vk,
            },
            poly::{Rotation, commitment::Params as _},
            transcript::{Blake2bRead, Blake2bWrite, Challenge255},
        };
        use rand_core_06::OsRng;

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
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "a",
                            a,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "b",
                            b,
                            0,
                            || Value::known(Scalar::from(2)),
                        )?;
                        crate::zk::assign_advice_compat(
                            &mut region,
                            || "c",
                            c,
                            0,
                            || Value::known(Scalar::from(4)),
                        )?;
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

        let mut vk_container = zk1::wrap_start();
        zk1::wrap_append_ipa_k(&mut vk_container, k);
        zk1::wrap_append_vk_pasta(&mut vk_container, &vk_h2);

        // Proof with non-canonical INST value
        let mut proof_container = zk1::wrap_start();
        zk1::wrap_append_proof(&mut proof_container, &proof_bytes);
        proof_container.extend_from_slice(b"I10P");
        proof_container.extend_from_slice(&(1u32).to_le_bytes());
        proof_container.extend_from_slice(&(1u32).to_le_bytes());
        proof_container.extend_from_slice(&[0xFFu8; 32]); // invalid field repr

        let backend = "halo2/pasta/tiny-add-public";
        let vk_box = VerifyingKeyBox::new(backend.into(), vk_container);
        let prf_box = ProofBox::new(backend.into(), proof_container);
        assert!(!super::verify_backend(backend, &prf_box, Some(&vk_box)));
    }
}
