//! Test-only builders for canonical and retired Halo2 proof carriers.

use halo2_proofs::{
    halo2curves::{
        ff::PrimeField as _,
        pasta::{EqAffine as Curve, Fp},
    },
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
    super::zk1::wrap_append_proof(buf, transcript_bytes);
}

/// Append the Halo2 IPA parameter `k` TLV to a ZK1 envelope.
#[inline]
pub fn wrap_append_ipa_k(buf: &mut Vec<u8>, k: u32) {
    super::zk1::wrap_append_ipa_k(buf, k);
}

/// Append the circuit identifier TLV used for verifier-key commitment domain separation.
#[inline]
pub fn wrap_append_circuit_id(buf: &mut Vec<u8>, circuit_id: &str) {
    super::zk1::wrap_append_circuit_id(buf, circuit_id);
}

/// Append a verifying key payload encoded for Pasta curves.
#[inline]
pub fn wrap_append_vk_pasta(buf: &mut Vec<u8>, vk: &VerifyingKey<Curve>) {
    super::zk1::wrap_append_vk_pasta(buf, vk);
}

/// Append Pasta-Fp instances to a ZK1 envelope.
#[inline]
pub fn wrap_append_instances_pasta_fp(instances: &[Fp], buf: &mut Vec<u8>) {
    super::zk1::wrap_append_instances_pasta_fp(instances, buf);
}

/// Append Pasta-Fp instance column slices to a ZK1 envelope.
#[inline]
pub fn wrap_append_instances_pasta_fp_cols(cols: &[&[Fp]], buf: &mut Vec<u8>) {
    super::zk1::wrap_append_instances_pasta_fp_cols(cols, buf);
}

/// Build the canonical strict ZK1 carrier for a proof and its instance columns.
pub fn proof_with_pasta_fp_columns(proof: &[u8], columns: &[Vec<Fp>]) -> Vec<u8> {
    let column_refs: Vec<&[Fp]> = columns.iter().map(Vec::as_slice).collect();
    let mut bytes = wrap_start();
    wrap_append_proof(&mut bytes, proof);
    wrap_append_instances_pasta_fp_cols(&column_refs, &mut bytes);
    bytes
}

/// Encode Pasta-Fp instance columns as canonical scalar byte arrays.
pub fn pasta_fp_columns_as_bytes(columns: &[Vec<Fp>]) -> Vec<Vec<[u8; 32]>> {
    columns
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| {
                    let mut bytes = [0_u8; 32];
                    bytes.copy_from_slice(value.to_repr().as_ref());
                    bytes
                })
                .collect()
        })
        .collect()
}

/// Build single-row Pasta-Fp columns from integer fixture values.
pub fn pasta_fp_single_row_columns(values: &[u64]) -> Vec<Vec<Fp>> {
    values.iter().map(|value| vec![Fp::from(*value)]).collect()
}

/// Encode bytes in the retired caller-shaped Halo2 carrier for rejection tests.
pub fn retired_halo2_envelope(
    k: u8,
    n_in: u8,
    n_out: u8,
    flags: u8,
    public_inputs: &[[u8; 32]],
    proof: &[u8],
) -> Vec<u8> {
    let input_count = u16::try_from(public_inputs.len()).expect("fixture count fits u16");
    let input_bytes = u32::from(input_count) * 32;
    let proof_bytes = u32::try_from(proof.len()).expect("fixture proof length fits u32");
    let mut bytes = Vec::with_capacity(
        18 + usize::try_from(input_bytes).expect("fixture input length fits usize") + proof.len(),
    );
    bytes.extend_from_slice(&[1, 1, 1, 1, k, n_in, n_out, flags]);
    bytes.extend_from_slice(&input_count.to_le_bytes());
    bytes.extend_from_slice(&input_bytes.to_le_bytes());
    for input in public_inputs {
        bytes.extend_from_slice(input);
    }
    bytes.extend_from_slice(&proof_bytes.to_le_bytes());
    bytes.extend_from_slice(proof);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    const CIRCUIT_ID: &str = "halo2/pasta/ipa/canonical-order-test";

    fn append_raw_tlv(bytes: &mut Vec<u8>, tag: [u8; 4], payload: &[u8]) {
        bytes.extend_from_slice(&tag);
        bytes.extend_from_slice(
            &u32::try_from(payload.len())
                .expect("fixture TLV length fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(payload);
    }

    #[test]
    fn verifier_key_shape_requires_exact_cid_and_tlv_order() {
        let mut canonical = wrap_start();
        wrap_append_ipa_k(&mut canonical, 7);
        wrap_append_circuit_id(&mut canonical, CIRCUIT_ID);
        append_raw_tlv(&mut canonical, *b"H2VK", &[1]);
        assert_eq!(
            super::super::zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(&canonical, CIRCUIT_ID,),
            Ok(7)
        );

        let mut reordered = wrap_start();
        wrap_append_circuit_id(&mut reordered, CIRCUIT_ID);
        wrap_append_ipa_k(&mut reordered, 7);
        append_raw_tlv(&mut reordered, *b"H2VK", &[1]);
        assert!(
            super::super::zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(&reordered, CIRCUIT_ID,)
                .is_err()
        );

        let mut padded = wrap_start();
        wrap_append_ipa_k(&mut padded, 7);
        wrap_append_circuit_id(&mut padded, &format!(" {CIRCUIT_ID}"));
        append_raw_tlv(&mut padded, *b"H2VK", &[1]);
        assert!(
            super::super::zk1::ensure_halo2_ipa_vk_envelope_shape_any_k(&padded, CIRCUIT_ID)
                .is_err()
        );
    }

    #[test]
    fn proof_shape_requires_prof_before_instances() {
        let columns = pasta_fp_single_row_columns(&[1, 2]);
        let canonical = proof_with_pasta_fp_columns(&[0xaa], &columns);
        assert!(super::super::zkparse::strict_proof_and_instances(&canonical).is_ok());

        let column_refs: Vec<&[Fp]> = columns.iter().map(Vec::as_slice).collect();
        let mut reordered = wrap_start();
        wrap_append_instances_pasta_fp_cols(&column_refs, &mut reordered);
        wrap_append_proof(&mut reordered, &[0xaa]);
        assert!(super::super::zkparse::strict_proof_and_instances(&reordered).is_err());
    }
}
