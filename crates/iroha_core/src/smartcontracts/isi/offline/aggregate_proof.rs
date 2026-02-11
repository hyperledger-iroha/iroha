use std::sync::OnceLock;

use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT,
    ristretto::{CompressedRistretto, RistrettoPoint},
    scalar::Scalar,
};
use iroha_crypto::Hash;
use iroha_data_model::offline::{
    OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN, OFFLINE_FASTPQ_HKDF_DOMAIN,
    OFFLINE_FASTPQ_PROOF_VERSION_V1, OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN,
    OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN, OFFLINE_FASTPQ_SUM_NONCE_DOMAIN,
    OFFLINE_FASTPQ_SUM_PROOF_DOMAIN, OfflineFastpqCounterProof, OfflineFastpqReplayProof,
    OfflineFastpqSumProof, OfflineProofBlindingSeed, OfflineSpendReceipt, OfflineToOnlineTransfer,
    PoseidonDigest, canonical_receipts,
};
use iroha_primitives::numeric::Numeric;
use norito::decode_from_bytes;
use sha2::Sha512;

const OFFLINE_GENERATOR_LABEL: &[u8] = b"iroha.offline.balance.generator.H.v1";

static PEDERSEN_H: OnceLock<RistrettoPoint> = OnceLock::new();

fn pedersen_generator_h() -> &'static RistrettoPoint {
    PEDERSEN_H.get_or_init(|| RistrettoPoint::hash_from_bytes::<Sha512>(OFFLINE_GENERATOR_LABEL))
}

fn decode_commitment_point(bytes: &[u8]) -> Result<RistrettoPoint, String> {
    if bytes.len() != 32 {
        return Err(format!("commitment must be 32 bytes (got {})", bytes.len()));
    }
    let compressed = CompressedRistretto::from_slice(bytes)
        .map_err(|_| "commitment encoding invalid".to_string())?;
    compressed
        .decompress()
        .ok_or_else(|| "commitment encoding invalid".to_string())
}

fn aggregate_receipt_amounts(receipts: &[OfflineSpendReceipt]) -> Result<Numeric, String> {
    if receipts.is_empty() {
        return Err("receipt list is empty".into());
    }
    let mut total = Numeric::zero();
    for receipt in receipts {
        total = total
            .checked_add(receipt.amount.clone())
            .ok_or_else(|| "aggregate amount overflow".to_string())?;
    }
    Ok(total)
}

fn numeric_to_scalar(value: &Numeric) -> Result<Scalar, String> {
    let mantissa = value
        .try_mantissa_u128()
        .ok_or_else(|| "amount out of range".to_string())?;
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(&mantissa.to_le_bytes());
    Ok(Scalar::from_bytes_mod_order(bytes))
}

fn numeric_to_le_bytes(value: &Numeric) -> Result<[u8; 16], String> {
    let mantissa = value
        .try_mantissa_u128()
        .ok_or_else(|| "amount out of range".to_string())?;
    let signed = i128::try_from(mantissa).map_err(|_| "amount out of range".to_string())?;
    Ok(signed.to_le_bytes())
}

fn blinding_scalar_from_seed(seed: &OfflineProofBlindingSeed) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
    hasher.update(OFFLINE_FASTPQ_HKDF_DOMAIN);
    hasher.update(seed.hkdf_salt.as_ref());
    hasher.update(&seed.counter.to_be_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    Scalar::from_bytes_mod_order_wide(&output)
}

fn sum_proof_nonce(
    bundle_id: &Hash,
    certificate_id: &Hash,
    receipts_root: &PoseidonDigest,
    delta_le: &[u8; 16],
    blind_sum: &Scalar,
) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
    hasher.update(OFFLINE_FASTPQ_SUM_NONCE_DOMAIN);
    hasher.update(bundle_id.as_ref());
    hasher.update(certificate_id.as_ref());
    hasher.update(receipts_root.as_bytes());
    hasher.update(delta_le);
    hasher.update(&blind_sum.to_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    Scalar::from_bytes_mod_order_wide(&output)
}

fn sum_proof_challenge(
    bundle_id: &Hash,
    certificate_id: &Hash,
    receipts_root: &PoseidonDigest,
    c_init: &RistrettoPoint,
    c_res: &RistrettoPoint,
    delta_le: &[u8; 16],
    r_point: &RistrettoPoint,
) -> Scalar {
    let mut hasher = Blake2bVar::new(64).expect("valid Blake2b length");
    hasher.update(OFFLINE_FASTPQ_SUM_PROOF_DOMAIN);
    hasher.update(bundle_id.as_ref());
    hasher.update(certificate_id.as_ref());
    hasher.update(receipts_root.as_bytes());
    hasher.update(c_init.compress().as_bytes());
    hasher.update(c_res.compress().as_bytes());
    hasher.update(delta_le);
    hasher.update(r_point.compress().as_bytes());
    let mut output = [0u8; 64];
    hasher
        .finalize_variable(&mut output)
        .expect("output size matches");
    Scalar::from_bytes_mod_order_wide(&output)
}

fn replay_chain(head: Hash, tx_ids: &[Hash]) -> Hash {
    let mut current = head;
    for tx_id in tx_ids {
        let mut buf =
            Vec::with_capacity(OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN.len() + Hash::LENGTH * 2);
        buf.extend_from_slice(OFFLINE_FASTPQ_REPLAY_CHAIN_DOMAIN);
        buf.extend_from_slice(current.as_ref());
        buf.extend_from_slice(tx_id.as_ref());
        current = Hash::new(buf);
    }
    current
}

fn counter_digest(
    bundle_id: &Hash,
    receipts_root: &PoseidonDigest,
    checkpoint: u64,
    counters: &[u64],
) -> Hash {
    let mut buf = Vec::with_capacity(
        OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN.len()
            + Hash::LENGTH
            + Hash::LENGTH
            + 8
            + counters.len() * 8,
    );
    buf.extend_from_slice(OFFLINE_FASTPQ_COUNTER_PROOF_DOMAIN);
    buf.extend_from_slice(bundle_id.as_ref());
    buf.extend_from_slice(receipts_root.as_bytes());
    buf.extend_from_slice(&checkpoint.to_be_bytes());
    for counter in counters {
        buf.extend_from_slice(&counter.to_be_bytes());
    }
    Hash::new(buf)
}

fn replay_digest(
    bundle_id: &Hash,
    receipts_root: &PoseidonDigest,
    head: &Hash,
    tail: &Hash,
    tx_ids: &[Hash],
) -> Hash {
    let mut buf = Vec::with_capacity(
        OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN.len() + Hash::LENGTH * 3 + Hash::LENGTH * tx_ids.len(),
    );
    buf.extend_from_slice(OFFLINE_FASTPQ_REPLAY_PROOF_DOMAIN);
    buf.extend_from_slice(bundle_id.as_ref());
    buf.extend_from_slice(receipts_root.as_bytes());
    buf.extend_from_slice(head.as_ref());
    buf.extend_from_slice(tail.as_ref());
    for tx_id in tx_ids {
        buf.extend_from_slice(tx_id.as_ref());
    }
    Hash::new(buf)
}

fn certificate_id_for_receipts(receipts: &[OfflineSpendReceipt]) -> Result<Hash, String> {
    let Some(first) = receipts.first() else {
        return Err("receipt list is empty".into());
    };
    let certificate_id = first.sender_certificate_id;
    for receipt in receipts.iter().skip(1) {
        if receipt.sender_certificate_id != certificate_id {
            return Err("receipts reference multiple certificates".into());
        }
    }
    Ok(certificate_id)
}

fn derived_blind_sum(certificate_id: Hash, receipts: &[OfflineSpendReceipt]) -> Scalar {
    receipts
        .iter()
        .map(|receipt| {
            OfflineProofBlindingSeed::derive(certificate_id, receipt.platform_proof.counter())
        })
        .map(|seed| blinding_scalar_from_seed(&seed))
        .fold(Scalar::ZERO, |acc, scalar| acc + scalar)
}

pub(super) fn verify_fastpq_sum_proof(
    transfer: &OfflineToOnlineTransfer,
    receipts_root: &PoseidonDigest,
    proof_bytes: &[u8],
) -> Result<(), String> {
    if proof_bytes.is_empty() {
        return Err("sum proof bytes are empty".into());
    }
    let proof: OfflineFastpqSumProof =
        decode_from_bytes(proof_bytes).map_err(|err| format!("sum proof decode failed: {err}"))?;
    if proof.version != OFFLINE_FASTPQ_PROOF_VERSION_V1 {
        return Err(format!(
            "sum proof version {} is not supported",
            proof.version
        ));
    }
    if proof.receipts_root != *receipts_root {
        return Err("sum proof receipts_root mismatch".into());
    }
    let total = aggregate_receipt_amounts(&transfer.receipts)?;
    if total != transfer.balance_proof.claimed_delta {
        return Err("sum proof delta does not match receipts".into());
    }
    let certificate_id = certificate_id_for_receipts(&transfer.receipts)?;
    let blind_sum = derived_blind_sum(certificate_id, &transfer.receipts);
    let c_init = decode_commitment_point(&transfer.balance_proof.initial_commitment.commitment)?;
    let c_res = decode_commitment_point(&transfer.balance_proof.resulting_commitment)?;
    let delta_scalar = numeric_to_scalar(&total)?;
    let delta_le = numeric_to_le_bytes(&total)?;
    let expected_delta =
        RISTRETTO_BASEPOINT_POINT * delta_scalar + *pedersen_generator_h() * blind_sum;
    if c_init - c_res != expected_delta {
        return Err("sum proof commitment delta mismatch".into());
    }
    let nonce = sum_proof_nonce(
        &transfer.bundle_id,
        &certificate_id,
        receipts_root,
        &delta_le,
        &blind_sum,
    );
    let expected_r = *pedersen_generator_h() * nonce;
    if proof.r_point != expected_r.compress().to_bytes() {
        return Err("sum proof R mismatch".into());
    }
    let challenge = sum_proof_challenge(
        &transfer.bundle_id,
        &certificate_id,
        receipts_root,
        &c_init,
        &c_res,
        &delta_le,
        &expected_r,
    );
    let expected_s = nonce + challenge * blind_sum;
    if proof.s_scalar != expected_s.to_bytes() {
        return Err("sum proof s mismatch".into());
    }
    Ok(())
}

pub(super) fn verify_fastpq_counter_proof(
    transfer: &OfflineToOnlineTransfer,
    receipts_root: &PoseidonDigest,
    proof_bytes: &[u8],
) -> Result<(), String> {
    if proof_bytes.is_empty() {
        return Err("counter proof bytes are empty".into());
    }
    let proof: OfflineFastpqCounterProof = decode_from_bytes(proof_bytes)
        .map_err(|err| format!("counter proof decode failed: {err}"))?;
    if proof.version != OFFLINE_FASTPQ_PROOF_VERSION_V1 {
        return Err(format!(
            "counter proof version {} is not supported",
            proof.version
        ));
    }
    if proof.receipts_root != *receipts_root {
        return Err("counter proof receipts_root mismatch".into());
    }
    if transfer.receipts.is_empty() {
        return Err("counter proof requires receipts".into());
    }
    let ordered_receipts = canonical_receipts(&transfer.receipts);
    let counters: Vec<u64> = ordered_receipts
        .iter()
        .map(|receipt| receipt.platform_proof.counter())
        .collect();
    let mut expected = proof.counter_checkpoint;
    for counter in &counters {
        expected = expected
            .checked_add(1)
            .ok_or_else(|| "counter checkpoint overflow".to_string())?;
        if *counter != expected {
            return Err("counter proof sequence mismatch".into());
        }
    }
    let digest = counter_digest(
        &transfer.bundle_id,
        receipts_root,
        proof.counter_checkpoint,
        &counters,
    );
    if proof.digest != digest {
        return Err("counter proof digest mismatch".into());
    }
    Ok(())
}

pub(super) fn verify_fastpq_replay_proof(
    transfer: &OfflineToOnlineTransfer,
    receipts_root: &PoseidonDigest,
    proof_bytes: &[u8],
) -> Result<(), String> {
    if proof_bytes.is_empty() {
        return Err("replay proof bytes are empty".into());
    }
    let proof: OfflineFastpqReplayProof = decode_from_bytes(proof_bytes)
        .map_err(|err| format!("replay proof decode failed: {err}"))?;
    if proof.version != OFFLINE_FASTPQ_PROOF_VERSION_V1 {
        return Err(format!(
            "replay proof version {} is not supported",
            proof.version
        ));
    }
    if proof.receipts_root != *receipts_root {
        return Err("replay proof receipts_root mismatch".into());
    }
    if transfer.receipts.is_empty() {
        return Err("replay proof requires receipts".into());
    }
    let ordered_receipts = canonical_receipts(&transfer.receipts);
    let tx_ids: Vec<Hash> = ordered_receipts
        .iter()
        .map(|receipt| receipt.tx_id)
        .collect();
    let computed_tail = replay_chain(proof.replay_log_head, &tx_ids);
    if computed_tail != proof.replay_log_tail {
        return Err("replay proof tail mismatch".into());
    }
    let digest = replay_digest(
        &transfer.bundle_id,
        receipts_root,
        &proof.replay_log_head,
        &proof.replay_log_tail,
        &tx_ids,
    );
    if proof.digest != digest {
        return Err("replay proof digest mismatch".into());
    }
    Ok(())
}
