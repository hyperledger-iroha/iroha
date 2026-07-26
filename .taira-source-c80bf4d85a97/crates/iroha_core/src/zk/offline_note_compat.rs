//! Real Halo2 compatibility proof for BOI's legacy load/audit/redeem calls.
//!
//! This module deliberately does not restore a legacy peer-payment mode or
//! readiness fallback. ABI-21/V4 `cash_handoff_v1` remains the sole peer-cash
//! runtime path. These helpers only keep old BOI participant clients able to
//! construct and settle their existing online note operations while migrating.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    io::Cursor,
    sync::{Arc, Mutex, OnceLock},
};

use halo2_proofs::poly::commitment::Params as _;
use iroha_data_model::proof::{ProofBox, VerifyingKeyBox};

use super::{
    PastaParams, ZK_BACKEND_HALO2_IPA, create_halo2_ipa_proof,
    decode_halo2_ipa_proving_key_archive, encode_halo2_ipa_proving_key_archive, halo2_backend,
    hash_to_u64_limbs_le, hash_vk, pasta_params_new, pasta_tiny, read_proving_key, zk1, zkparse,
};

/// Canonical circuit identifier retained by legacy BOI SDKs.
pub const OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID: &str = "offline-note-recursive";
/// Canonical Pasta IPA parameter degree for the compatibility circuit.
pub const OFFLINE_NOTE_RECURSIVE_IPA_K: u32 = 7;
/// Maximum accepted legacy recursive proof size.
pub const OFFLINE_NOTE_MAX_PROOF_BYTES: u32 = 8 * 1024 * 1024;

pub(super) const OFFLINE_NOTE_MODE_REDEEM: u64 = 1;
pub(super) const OFFLINE_NOTE_MODE_AUDIT: u64 = 2;
/// Number of public instance columns in the compatibility circuit.
pub const OFFLINE_NOTE_INSTANCE_COLUMNS: usize = 16;
/// Maximum number of input amount witness slots.
pub const OFFLINE_NOTE_MAX_INPUT_AMOUNTS: usize = 4;
/// Maximum number of output amount witness slots.
pub const OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS: usize = 2;

/// Public and private witness values for the compatibility circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfflineNoteInstanceValues {
    /// Public single-row Pasta values.
    pub public_values: [u64; OFFLINE_NOTE_INSTANCE_COLUMNS],
    /// Private normalized input amount slots.
    pub input_amounts: [u64; OFFLINE_NOTE_MAX_INPUT_AMOUNTS],
    /// Private normalized output amount slots.
    pub output_amounts: [u64; OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS],
}

impl OfflineNoteInstanceValues {
    /// Encode the public single-row columns exactly as carried by the proof envelope.
    #[must_use]
    pub fn public_instance_columns(&self) -> Vec<Vec<[u8; 32]>> {
        self.public_values
            .iter()
            .map(|value| {
                let mut encoded = [0_u8; 32];
                encoded[..8].copy_from_slice(&value.to_le_bytes());
                vec![encoded]
            })
            .collect()
    }

    fn public_scalars(
        &self,
    ) -> [halo2_proofs::halo2curves::pasta::Fp; OFFLINE_NOTE_INSTANCE_COLUMNS] {
        self.public_values
            .map(halo2_proofs::halo2curves::pasta::Fp::from)
    }

    fn input_amount_scalars(
        &self,
    ) -> [halo2_proofs::halo2curves::pasta::Fp; OFFLINE_NOTE_MAX_INPUT_AMOUNTS] {
        self.input_amounts
            .map(halo2_proofs::halo2curves::pasta::Fp::from)
    }

    fn output_amount_scalars(
        &self,
    ) -> [halo2_proofs::halo2curves::pasta::Fp; OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS] {
        self.output_amounts
            .map(halo2_proofs::halo2curves::pasta::Fp::from)
    }
}

/// Build the canonical inline verifier key for legacy BOI note proofs.
///
/// # Errors
///
/// Returns an error if Halo2 verifier-key generation fails.
pub fn offline_note_recursive_vk_box() -> Result<VerifyingKeyBox, String> {
    static CACHE: OnceLock<Result<VerifyingKeyBox, String>> = OnceLock::new();

    CACHE
        .get_or_init(|| {
            build_offline_note_recursive_vk_box().map_err(|error| {
                format!("failed to generate offline-note-recursive verifying key: {error}")
            })
        })
        .clone()
}

pub(crate) fn ensure_offline_note_recursive_canonical_vk_box(
    vk_box: &VerifyingKeyBox,
) -> Result<(), String> {
    if vk_box.backend.as_str() != ZK_BACKEND_HALO2_IPA {
        return Err(format!(
            "offline recursive verifier key backend `{}` is not `{ZK_BACKEND_HALO2_IPA}`",
            vk_box.backend
        ));
    }
    if vk_box.bytes.is_empty() {
        return Err("offline recursive verifier key must be non-empty".to_owned());
    }
    let canonical = offline_note_recursive_vk_box()?;
    if hash_vk(vk_box) != hash_vk(&canonical) || vk_box.bytes != canonical.bytes {
        return Err(
            "offline recursive verifier key must match the canonical semantic circuit key"
                .to_owned(),
        );
    }
    Ok(())
}

fn build_offline_note_recursive_vk_box() -> Result<VerifyingKeyBox, halo2_backend::Error> {
    let params = pasta_params_new(OFFLINE_NOTE_RECURSIVE_IPA_K);
    let circuit = pasta_tiny::OfflineNoteSemantic::default();
    let vk = halo2_backend::keygen_vk(&params, &circuit)?;
    let mut bytes = zk1::wrap_start();
    zk1::wrap_append_ipa_k(&mut bytes, OFFLINE_NOTE_RECURSIVE_IPA_K);
    zk1::wrap_append_circuit_id(&mut bytes, OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID);
    zk1::wrap_append_vk_pasta(&mut bytes, &vk);
    Ok(VerifyingKeyBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), bytes))
}

/// Build the governance/WSV verifier-key record expected by legacy BOI clients.
///
/// # Errors
///
/// Returns an error if key generation or length conversion fails.
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
        .fold(0_u64, |sum, hash| sum.wrapping_add(hash_limb0(hash)))
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
    values.iter().try_fold(0_u64, |sum, value| {
        sum.checked_add(*value)
            .ok_or_else(|| format!("offline {label} amount sum overflows u64 witness units"))
    })
}

#[allow(clippy::too_many_arguments)]
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

/// Derive the circuit witnesses for one legacy BOI redemption.
///
/// # Errors
///
/// Returns an error for an invalid proof shape or amount representation.
pub fn offline_note_redeem_instance_values(
    redemption: &iroha_data_model::offline::OfflineNoteRedeem,
) -> Result<OfflineNoteInstanceValues, String> {
    use iroha_data_model::offline::OfflineNoteIssuedClaim;

    let input_count = validate_offline_note_count(
        redemption.input_nullifiers.len(),
        OFFLINE_NOTE_MAX_INPUT_AMOUNTS,
        "redemption input",
    )?;
    let public_inputs_hash = redemption
        .public_inputs_hash()
        .map_err(|error| format!("failed to encode offline redemption public inputs: {error}"))?;
    let key_certificate_payload_hash = redemption
        .sender_key_certificate
        .payload_hash()
        .map_err(|error| format!("failed to encode offline key certificate payload: {error}"))?;
    let issued_claim_hash = OfflineNoteIssuedClaim::from_redemption(redemption)
        .and_then(|claim| claim.claim_hash())
        .map_err(|error| format!("failed to encode offline redemption issued claim: {error}"))?;

    let normalized_amounts = normalized_amount_vec(&[&redemption.amount, &redemption.amount])?;
    let input_sum = normalized_amounts[0];
    let output_sum = normalized_amounts[1];
    let public_values = offline_note_public_values(
        &public_inputs_hash,
        OFFLINE_NOTE_MODE_REDEEM,
        input_count,
        1,
        input_sum,
        output_sum,
        hash_limb0_sum(&redemption.input_nullifiers),
        0,
        &key_certificate_payload_hash,
        &redemption.source_note_commitment,
        hash_limb0(&issued_claim_hash),
        0,
    );

    let mut input_amounts = [0_u64; OFFLINE_NOTE_MAX_INPUT_AMOUNTS];
    input_amounts[0] = input_sum;
    let mut output_amounts = [0_u64; OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS];
    output_amounts[0] = output_sum;
    Ok(OfflineNoteInstanceValues {
        public_values,
        input_amounts,
        output_amounts,
    })
}

/// Derive the circuit witnesses for one optional legacy BOI audit.
///
/// # Errors
///
/// Returns an error for mismatched claims, invalid counts, or non-conserved amounts.
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
    if audit.output_commitments.len() != audit.output_claims.len() {
        return Err(
            "offline audit output claim count must match output commitment count".to_owned(),
        );
    }
    if audit
        .output_commitments
        .iter()
        .zip(&audit.output_claims)
        .any(|(commitment, claim)| commitment != &claim.note_commitment)
    {
        return Err(
            "offline audit output claims must be ordered one-to-one with output commitments"
                .to_owned(),
        );
    }

    let public_inputs_hash = audit
        .public_inputs_hash()
        .map_err(|error| format!("failed to encode offline audit public inputs: {error}"))?;
    let key_certificate_payload_hash = audit
        .sender_key_certificate
        .payload_hash()
        .map_err(|error| format!("failed to encode offline key certificate payload: {error}"))?;
    for claim in &audit.input_claims {
        if claim.key_certificate_payload_hash != key_certificate_payload_hash {
            return Err("offline audit input claims must match sender key certificate".to_owned());
        }
    }
    if let Some(input_claim) = audit.input_claims.first() {
        let input_definition = input_claim.asset.definition();
        if audit
            .input_claims
            .iter()
            .any(|claim| claim.asset.definition() != input_definition)
            || audit
                .output_claims
                .iter()
                .any(|claim| claim.asset.definition() != input_definition)
        {
            return Err("offline audit input and output asset definitions must match".to_owned());
        }
    }

    let input_claim_hashes = audit
        .input_claims
        .iter()
        .map(|claim| {
            claim
                .claim_hash()
                .map_err(|error| format!("failed to encode offline audit input claim: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let output_claim_hashes = audit
        .output_claims
        .iter()
        .map(|claim| {
            OfflineNoteIssuedClaim::from_audit_output(claim)
                .and_then(|claim| claim.claim_hash())
                .map_err(|error| format!("failed to encode offline audit output claim: {error}"))
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

    let mut input_amounts = [0_u64; OFFLINE_NOTE_MAX_INPUT_AMOUNTS];
    for (slot, amount) in input_amounts.iter_mut().zip(input_units.iter().copied()) {
        *slot = amount;
    }
    let mut output_amounts = [0_u64; OFFLINE_NOTE_MAX_OUTPUT_AMOUNTS];
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

fn cached_offline_note_proving_key(
    params: &PastaParams,
    parsed_vk: &halo2_backend::VerifyingKey,
    vk_commitment: [u8; 32],
) -> Result<Arc<halo2_backend::ProvingKey>, String> {
    static CACHE: OnceLock<Mutex<BTreeMap<[u8; 32], Arc<halo2_backend::ProvingKey>>>> =
        OnceLock::new();

    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(proving_key) = cache
        .lock()
        .expect("offline note proving key cache mutex poisoned")
        .get(&vk_commitment)
        .cloned()
    {
        return Ok(proving_key);
    }

    let proving_key = halo2_backend::keygen_pk(
        params,
        parsed_vk.clone(),
        &pasta_tiny::OfflineNoteSemantic::default(),
    )
    .map_err(|error| format!("failed to derive proving key: {error}"))?;
    let proving_key = Arc::new(proving_key);
    let mut cache = cache
        .lock()
        .expect("offline note proving key cache mutex poisoned");
    match cache.entry(vk_commitment) {
        Entry::Occupied(entry) => Ok(Arc::clone(entry.get())),
        Entry::Vacant(entry) => Ok(Arc::clone(entry.insert(proving_key))),
    }
}

fn prove_halo2_ipa_offline_note_envelope(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    instance_values: OfflineNoteInstanceValues,
    proving_key_bytes: Option<&[u8]>,
) -> Result<ProofBox, String> {
    use iroha_data_model::{
        offline::OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA,
        zk::{BackendTag, OpenVerifyEnvelope},
    };

    if circuit_id != OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID {
        return Err(format!(
            "offline recursive proving requires canonical circuit id `{OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID}` (found `{circuit_id}`)"
        ));
    }
    ensure_offline_note_recursive_canonical_vk_box(vk_box)?;

    let params = zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed_vk: halo2_backend::VerifyingKey =
        zkparse::vk_from_bytes::<pasta_tiny::OfflineNoteSemantic>(vk_box.bytes.as_slice(), &params)
            .ok_or_else(|| {
                "missing/invalid H2VK payload for offline-note-recursive verifying key".to_owned()
            })?;

    let public_values = instance_values.public_scalars();
    let instance_columns_owned = public_values
        .iter()
        .map(|value| vec![*value])
        .collect::<Vec<Vec<halo2_backend::Scalar>>>();
    let instance_columns = instance_columns_owned
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let instance_refs = vec![instance_columns.as_slice()];

    let vk_commitment = hash_vk(vk_box);
    let proving_key = if let Some(bytes) = proving_key_bytes {
        let proving_key_raw = decode_halo2_ipa_proving_key_archive(
            bytes,
            OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            vk_commitment,
        )?;
        let mut cursor = Cursor::new(proving_key_raw.as_slice());
        let key = read_proving_key::<pasta_tiny::OfflineNoteSemantic, _>(&mut cursor)
            .map_err(|error| format!("failed to decode proving key: {error}"))?;
        let consumed = usize::try_from(cursor.position()).unwrap_or(usize::MAX);
        if consumed != proving_key_raw.len() {
            return Err("failed to decode proving key: trailing bytes".to_owned());
        }
        if halo2_backend::proving_key_domain_k(&key) != params.k() {
            return Err("proving key domain does not match IPAK parameters".to_owned());
        }
        if halo2_backend::proving_key_vk_to_processed_bytes(&key)
            != halo2_backend::verifying_key_to_processed_bytes(&parsed_vk)
        {
            return Err("proving key verifying key does not match vk_ref bytes".to_owned());
        }
        Arc::new(key)
    } else {
        cached_offline_note_proving_key(&params, &parsed_vk, vk_commitment)?
    };

    let circuit = pasta_tiny::OfflineNoteSemantic {
        public_values,
        input_amounts: instance_values.input_amount_scalars(),
        output_amounts: instance_values.output_amount_scalars(),
    };
    let proof_raw = create_halo2_ipa_proof(
        &params,
        proving_key.as_ref(),
        circuit,
        &instance_refs,
        "offline-note-recursive",
    )?;

    let mut proof_payload = zk1::wrap_start();
    zk1::wrap_append_proof(&mut proof_payload, &proof_raw);
    zk1::wrap_append_instances_pasta_fp_cols(&instance_columns, &mut proof_payload);
    let envelope = OpenVerifyEnvelope {
        backend: BackendTag::Halo2IpaPasta,
        circuit_id: circuit_id.to_owned(),
        vk_hash: vk_commitment,
        public_inputs: OFFLINE_NOTE_RECURSIVE_PUBLIC_INPUTS_SCHEMA.to_vec(),
        proof_bytes: proof_payload,
        aux: Vec::new(),
    };
    let encoded = norito::to_bytes(&envelope)
        .map_err(|error| format!("failed to encode OpenVerifyEnvelope: {error}"))?;
    Ok(ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), encoded))
}

/// Derive a canonical serialized Halo2 proving-key archive.
///
/// # Errors
///
/// Returns an error if the verifier key is noncanonical or key generation fails.
pub fn derive_halo2_ipa_offline_note_proving_key_bytes(
    vk_box: &VerifyingKeyBox,
) -> Result<Vec<u8>, String> {
    ensure_offline_note_recursive_canonical_vk_box(vk_box)?;
    let params = zkparse::params_any(vk_box.bytes.as_slice())
        .ok_or_else(|| "missing/invalid IPAK parameters in verifying key envelope".to_owned())?;
    let parsed_vk =
        zkparse::vk_from_bytes::<pasta_tiny::OfflineNoteSemantic>(vk_box.bytes.as_slice(), &params)
            .ok_or_else(|| {
                "missing/invalid H2VK payload for offline-note-recursive verifying key".to_owned()
            })?;
    let proving_key = halo2_backend::keygen_pk(
        &params,
        parsed_vk,
        &pasta_tiny::OfflineNoteSemantic::default(),
    )
    .map_err(|error| format!("failed to derive proving key: {error}"))?;
    encode_halo2_ipa_proving_key_archive(
        OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
        hash_vk(vk_box),
        halo2_backend::proving_key_to_processed_bytes(&proving_key),
    )
}

/// Prove a legacy BOI online redemption with the real Halo2 circuit.
///
/// # Errors
///
/// Returns an error for incompatible key material or invalid redemption data.
pub fn prove_offline_note_redeem(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    redemption: &iroha_data_model::offline::OfflineNoteRedeem,
    proving_key_bytes: Option<&[u8]>,
) -> Result<ProofBox, String> {
    let instance_values = offline_note_redeem_instance_values(redemption)?;
    prove_halo2_ipa_offline_note_envelope(circuit_id, vk_box, instance_values, proving_key_bytes)
}

/// Prove an optional legacy BOI audit with the real Halo2 circuit.
///
/// # Errors
///
/// Returns an error for incompatible key material or invalid audit data.
pub fn prove_offline_note_audit(
    circuit_id: &str,
    vk_box: &VerifyingKeyBox,
    audit: &iroha_data_model::offline::OfflineNoteAuditBundle,
    proving_key_bytes: Option<&[u8]>,
) -> Result<ProofBox, String> {
    let instance_values = offline_note_audit_instance_values(audit)?;
    prove_halo2_ipa_offline_note_envelope(circuit_id, vk_box, instance_values, proving_key_bytes)
}

#[cfg(test)]
mod tests {
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
        proof::{ProofBox, VerifyingKeyId},
        zk::OpenVerifyEnvelope,
    };
    use iroha_primitives::Numeric;

    use super::*;

    fn fixture_account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("checked fixture key")
                .public_key()
                .clone(),
        )
    }

    fn fixture_asset(account: AccountId) -> AssetId {
        let definition = AssetDefinitionId::new(
            DomainId::try_new("offline", "universal").expect("fixture domain"),
            "ils".parse().expect("fixture asset name"),
        );
        AssetId::new(definition, account)
    }

    fn fixture_signature(seed: u8) -> Signature {
        let mut bytes = [0_u8; 64];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = seed.wrapping_add(u8::try_from(index).expect("signature index"));
        }
        Signature::try_from_bytes(&bytes).expect("admitted fixture signature")
    }

    fn fixture_certificate(account: &AccountId, seed: u8) -> OfflineNoteKeyCertificate {
        let note_key =
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("checked note key");
        let (_, public_key) = note_key
            .public_key()
            .try_to_bytes()
            .expect("serializable note key");
        OfflineNoteKeyCertificate {
            version: iroha_data_model::offline::OFFLINE_NOTE_KEY_CERTIFICATE_VERSION,
            platform: "ios-appattest".to_owned(),
            key_id: format!("one-use-{seed}"),
            device_id: "boi-demo-device".to_owned(),
            account_id: account.clone(),
            public_key: public_key.to_vec(),
            assertion_scheme: "apple-appattest-counter-v1".to_owned(),
            assertion_key_algorithm: "app-attest-p256".to_owned(),
            assertion_public_key: vec![0x04; 65],
            assertion_usage_count_limit: None,
            one_use: true,
            issuer_signature: fixture_signature(seed.wrapping_add(1)),
        }
    }

    fn placeholder_proof() -> OfflineNoteRecursiveProof {
        OfflineNoteRecursiveProof {
            verifier_key_id: VerifyingKeyId::new(
                ZK_BACKEND_HALO2_IPA,
                OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID,
            ),
            public_inputs_hash: Hash::new(b"placeholder"),
            proof: ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), Vec::new()),
        }
    }

    fn fixture_redemption() -> OfflineNoteRedeem {
        let account = fixture_account(0x31);
        OfflineNoteRedeem {
            source_note_commitment: Hash::new(b"boi-legacy-source-note"),
            input_nullifiers: vec![Hash::new(b"boi-legacy-current-bearer-nullifier")],
            sender_key_certificate: fixture_certificate(&account, 0x41),
            recipient: account.clone(),
            asset: fixture_asset(account),
            amount: Numeric::new(10, 0),
            recursive_proof: placeholder_proof(),
        }
    }

    fn fixture_audit() -> OfflineNoteAuditBundle {
        let account = fixture_account(0x51);
        let asset = fixture_asset(account.clone());
        let certificate = fixture_certificate(&account, 0x61);
        let issue = OfflineNoteIssue {
            note_commitment: Hash::new(b"boi-legacy-audit-input"),
            key_certificate: certificate.clone(),
            asset: asset.clone(),
            amount: Numeric::new(10, 0),
        };
        OfflineNoteAuditBundle {
            token_id: Hash::new(b"boi-legacy-optional-audit"),
            sender_key_certificate: certificate.clone(),
            input_nullifiers: vec![Hash::new(b"boi-legacy-audit-nullifier")],
            input_claims: vec![OfflineNoteIssuedClaim::from_issue(&issue).expect("input claim")],
            output_commitments: vec![Hash::new(b"boi-legacy-audit-output")],
            output_claims: vec![OfflineNoteAuditOutputClaim {
                note_commitment: Hash::new(b"boi-legacy-audit-output"),
                key_certificate: certificate,
                asset,
                amount: Numeric::new(10, 0),
            }],
            recursive_proof: placeholder_proof(),
        }
    }

    fn envelope_instances(proof: &ProofBox) -> Vec<Vec<[u8; 32]>> {
        let envelope: OpenVerifyEnvelope =
            norito::decode_from_bytes(&proof.bytes).expect("open verify envelope");
        crate::zk::extract_pasta_instance_columns_bytes(&envelope.proof_bytes)
            .expect("proof instances")
    }

    #[test]
    fn real_redeem_proof_verifies_and_tampering_rejects() {
        let vk = offline_note_recursive_vk_box().expect("canonical verifier key");
        let redemption = fixture_redemption();
        let mut proof =
            prove_offline_note_redeem(OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID, &vk, &redemption, None)
                .expect("real Halo2 redemption proof");
        assert!(crate::zk::verify_backend(
            ZK_BACKEND_HALO2_IPA,
            &proof,
            Some(&vk)
        ));
        assert_eq!(
            envelope_instances(&proof),
            offline_note_redeem_instance_values(&redemption)
                .expect("redemption witness")
                .public_instance_columns()
        );

        *proof.bytes.last_mut().expect("proof byte") ^= 0x01;
        assert!(!crate::zk::verify_backend(
            ZK_BACKEND_HALO2_IPA,
            &proof,
            Some(&vk)
        ));
    }

    #[test]
    fn real_optional_audit_proof_verifies_without_becoming_a_peer_mode() {
        let vk = offline_note_recursive_vk_box().expect("canonical verifier key");
        let audit = fixture_audit();
        let proof = prove_offline_note_audit(OFFLINE_NOTE_RECURSIVE_CIRCUIT_ID, &vk, &audit, None)
            .expect("real Halo2 audit proof");
        assert!(crate::zk::verify_backend(
            ZK_BACKEND_HALO2_IPA,
            &proof,
            Some(&vk)
        ));
        assert_eq!(
            envelope_instances(&proof),
            offline_note_audit_instance_values(&audit)
                .expect("audit witness")
                .public_instance_columns()
        );
    }
}
