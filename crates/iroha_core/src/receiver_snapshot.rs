//! Witness-side proof construction for consensus-authenticated synthetic writes.
use crate::sumeragi::smt::KvPair;
use iroha_crypto::Hash;
use iroha_data_model::block::consensus::ExecWitness;
use iroha_data_model::isi::kagemusha_v1::{
    KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1, KagemushaReserveReceiptV1,
    KagemushaReserveReceiptWitnessV1,
};
use iroha_data_model::parliament_casting::{
    PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1, ParliamentTimedOvnCastingWitnessProofV1,
};
use iroha_data_model::validation_fee::{
    VALIDATION_FEE_POLICY_WITNESS_KEY_V1, ValidationFeePolicyWitnessProofV1,
};
use std::collections::{BTreeMap, BTreeSet};

/// Construct every Kagemusha V1 reserve-receipt proof in one execution witness.
///
/// The returned entries are sorted by operation id and are derived from the same
/// last-write-wins ordinary-write set used by the consensus commitment. Duplicate
/// receipt keys fail closed instead of being hidden by last-write-wins projection.
pub(crate) fn kagemusha_reserve_receipt_witnesses_v1(
    witness: &ExecWitness,
) -> Result<(Vec<KagemushaReserveReceiptWitnessV1>, Hash), String> {
    let tagged_write_count = witness
        .writes
        .iter()
        .filter(|entry| entry.key.first() == Some(&KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1))
        .count();
    let mut canonical = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    for entry in &witness.writes {
        canonical.insert(entry.key.clone(), entry.value.clone());
    }
    let ordinary = canonical
        .into_iter()
        .map(|(key, value)| KvPair::new(key, value))
        .collect::<Vec<_>>();
    let targets = ordinary
        .iter()
        .filter(|pair| pair.key.first() == Some(&KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1))
        .collect::<Vec<_>>();
    if targets.len() != tagged_write_count {
        return Err("execution witness contains duplicate Kagemusha V1 receipt writes".to_owned());
    }
    let ordinary_root = crate::sumeragi::smt::compute_post_state_root(&[], &ordinary);
    let mut proofs = Vec::with_capacity(targets.len());
    for target in targets {
        let receipt: KagemushaReserveReceiptV1 = norito::decode_canonical(&target.value)
            .map_err(|error| format!("Kagemusha V1 receipt is not canonical Norito: {error}"))?;
        if target.key != KagemushaReserveReceiptWitnessV1::expected_key(receipt.operation_id) {
            return Err(
                "Kagemusha V1 receipt witness key does not match its operation id".to_owned(),
            );
        }
        let proof = KagemushaReserveReceiptWitnessV1 {
            key: target.key.clone(),
            receipt,
            siblings: sparse_smt_siblings(&ordinary, target)?,
        };
        if !proof.verify(ordinary_root) {
            return Err(
                "constructed Kagemusha V1 receipt proof does not reconstruct the ordinary-write root"
                    .to_owned(),
            );
        }
        proofs.push(proof);
    }
    if proofs
        .windows(2)
        .any(|pair| pair[0].receipt.operation_id >= pair[1].receipt.operation_id)
    {
        return Err("Kagemusha V1 receipt proofs are not canonically ordered".to_owned());
    }
    Ok((proofs, ordinary_root))
}
/// Construct the exact fixed-key validation-fee policy proof against the ordinary-write SMT.
pub(crate) fn validation_fee_policy_witness_proof_v1(
    witness: &ExecWitness,
) -> Result<(ValidationFeePolicyWitnessProofV1, Hash), String> {
    let target_count = witness
        .writes
        .iter()
        .filter(|entry| entry.key.as_slice() == VALIDATION_FEE_POLICY_WITNESS_KEY_V1)
        .count();
    if target_count != 1 {
        return Err(format!(
            "execution witness contains {target_count} validation-fee synthetic writes; expected exactly one"
        ));
    }
    let mut canonical = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    for entry in &witness.writes {
        canonical.insert(entry.key.clone(), entry.value.clone());
    }
    let ordinary = canonical
        .into_iter()
        .map(|(key, value)| KvPair::new(key, value))
        .collect::<Vec<_>>();
    let ordinary_root = crate::sumeragi::smt::compute_post_state_root(&[], &ordinary);
    let target = ordinary
        .iter()
        .find(|pair| pair.key.as_slice() == VALIDATION_FEE_POLICY_WITNESS_KEY_V1)
        .ok_or_else(|| {
            "validation-fee synthetic write is absent from ordinary writes".to_owned()
        })?;
    let siblings = sparse_smt_siblings(&ordinary, target)?;
    let proof = ValidationFeePolicyWitnessProofV1 {
        key: target.key.clone(),
        value: target.value.clone(),
        siblings,
    };
    if !proof.verify(ordinary_root) {
        return Err(
            "constructed validation-fee witness proof does not reconstruct the ordinary-write root"
                .to_owned(),
        );
    }
    Ok((proof, ordinary_root))
}
/// Construct the exact fixed-key Parliament timed-OVN casting proof against the ordinary-write SMT.
pub(crate) fn parliament_timed_ovn_casting_witness_proof_v1(
    witness: &ExecWitness,
) -> Result<(ParliamentTimedOvnCastingWitnessProofV1, Hash), String> {
    let target_count = witness
        .writes
        .iter()
        .filter(|entry| entry.key.as_slice() == PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1)
        .count();
    if target_count != 1 {
        return Err(format!(
            "execution witness contains {target_count} Parliament timed-OVN casting synthetic writes; expected exactly one"
        ));
    }
    let mut canonical = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    for entry in &witness.writes {
        canonical.insert(entry.key.clone(), entry.value.clone());
    }
    let ordinary = canonical
        .into_iter()
        .map(|(key, value)| KvPair::new(key, value))
        .collect::<Vec<_>>();
    let ordinary_root = crate::sumeragi::smt::compute_post_state_root(&[], &ordinary);
    let target = ordinary
        .iter()
        .find(|pair| pair.key.as_slice() == PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1)
        .ok_or_else(|| {
            "Parliament timed-OVN casting synthetic write is absent from ordinary writes".to_owned()
        })?;
    let siblings = sparse_smt_siblings(&ordinary, target)?;
    let proof = ParliamentTimedOvnCastingWitnessProofV1 {
        key: target.key.clone(),
        value: target.value.clone(),
        siblings,
    };
    if !proof.verify(ordinary_root) {
        return Err(
            "constructed Parliament timed-OVN casting witness proof does not reconstruct the ordinary-write root"
                .to_owned(),
        );
    }
    Ok((proof, ordinary_root))
}
fn sparse_smt_siblings(inputs: &[KvPair], target: &KvPair) -> Result<Vec<Hash>, String> {
    let empty = Hash::new([]);
    let target_path = hash_bytes(&target.key).to_vec();
    let mut raw_paths = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    let mut current = BTreeMap::<Vec<u8>, Hash>::new();
    for pair in inputs {
        let path = hash_bytes(&pair.key).to_vec();
        if let Some(existing_key) = raw_paths.insert(path.clone(), pair.key.clone())
            && existing_key != pair.key
        {
            return Err("ordinary-write SMT contains a key-path hash collision".to_owned());
        }
        current.insert(path, leaf_hash(pair));
    }
    if !current.contains_key(&target_path) {
        return Err("active-receiver target path is absent from ordinary writes".to_owned());
    }
    let mut current_target = target_path;
    let mut siblings = Vec::with_capacity(256);
    let mut current_bits = 256_u16;
    while current_bits > 0 {
        let sibling = sibling_prefix(&current_target, current_bits);
        siblings.push(current.get(&sibling).copied().unwrap_or(empty));
        let mut parents = BTreeSet::new();
        for prefix in current.keys() {
            parents.insert(parent_prefix(prefix, current_bits));
        }
        let mut next = BTreeMap::new();
        for parent in parents {
            let left = child_prefix(&parent, current_bits, false);
            let right = child_prefix(&parent, current_bits, true);
            next.insert(
                parent,
                node_hash(
                    current.get(&left).copied().unwrap_or(empty),
                    current.get(&right).copied().unwrap_or(empty),
                ),
            );
        }
        current_target = parent_prefix(&current_target, current_bits);
        current = next;
        current_bits -= 1;
    }
    if siblings.len() != 256 {
        return Err("ordinary-write SMT proof has the wrong depth".to_owned());
    }
    Ok(siblings)
}
fn hash_bytes(bytes: &[u8]) -> [u8; 32] {
    Hash::new(bytes).into()
}
fn leaf_hash(pair: &KvPair) -> Hash {
    let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
    preimage.push(0);
    preimage.extend_from_slice(&hash_bytes(&pair.key));
    preimage.extend_from_slice(&hash_bytes(&pair.value));
    Hash::new(preimage)
}
fn node_hash(left: Hash, right: Hash) -> Hash {
    let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
    preimage.push(1);
    preimage.extend_from_slice(left.as_ref());
    preimage.extend_from_slice(right.as_ref());
    Hash::new(preimage)
}
fn parent_prefix(prefix: &[u8], len_bits: u16) -> Vec<u8> {
    truncate_prefix(prefix, len_bits - 1)
}
fn sibling_prefix(prefix: &[u8], len_bits: u16) -> Vec<u8> {
    let parent = parent_prefix(prefix, len_bits);
    let bit_index = len_bits - 1;
    let byte_index = usize::from(bit_index / 8);
    let bit_offset = (bit_index % 8) as u8;
    let right = prefix
        .get(byte_index)
        .is_some_and(|byte| byte & (1_u8 << bit_offset) != 0);
    child_prefix(&parent, len_bits, !right)
}
fn child_prefix(parent: &[u8], child_len_bits: u16, right: bool) -> Vec<u8> {
    let mut output = parent.to_vec();
    let bit_index = child_len_bits - 1;
    let byte_index = usize::from(bit_index / 8);
    let bit_offset = (bit_index % 8) as u8;
    if output.len() <= byte_index {
        output.resize(byte_index + 1, 0);
    }
    let mask = 1_u8 << bit_offset;
    if right {
        output[byte_index] |= mask;
    } else {
        output[byte_index] &= !mask;
    }
    mask_tail_bits(&mut output, child_len_bits);
    output
}
fn truncate_prefix(prefix: &[u8], len_bits: u16) -> Vec<u8> {
    if len_bits == 0 {
        return Vec::new();
    }
    let byte_len = usize::from(len_bits.div_ceil(8));
    let mut output = prefix[..prefix.len().min(byte_len)].to_vec();
    output.resize(byte_len, 0);
    mask_tail_bits(&mut output, len_bits);
    output
}
fn mask_tail_bits(bytes: &mut [u8], len_bits: u16) {
    let remainder = (len_bits % 8) as u8;
    if remainder == 0 || bytes.is_empty() {
        return;
    }
    let mask = (1_u16 << remainder) as u8 - 1;
    if let Some(last) = bytes.last_mut() {
        *last &= mask;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::block::consensus::ExecKv;
    use iroha_data_model::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1;
    use iroha_data_model::parliament_casting::ParliamentTimedOvnCastingSnapshotCommitmentV1;
    #[test]
    fn parliament_casting_fixed_write_has_256_siblings_and_rejects_tampering() {
        let snapshot = ParliamentTimedOvnCastingSnapshotCommitmentV1::empty(7);
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: vec![
                ExecKv {
                    key: b"ordinary-a".to_vec(),
                    value: b"one".to_vec(),
                },
                ExecKv {
                    key: PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1.to_vec(),
                    value: norito::to_bytes(&snapshot).expect("encode casting snapshot"),
                },
                ExecKv {
                    key: b"ordinary-b".to_vec(),
                    value: b"two".to_vec(),
                },
            ],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let (proof, root) =
            parliament_timed_ovn_casting_witness_proof_v1(&witness).expect("casting proof");
        assert_eq!(proof.siblings.len(), 256);
        assert_eq!(proof.commitment().expect("snapshot commitment"), snapshot);
        assert!(proof.verify(root));

        let mut tampered = proof.clone();
        tampered.siblings[127] = Hash::new(b"tampered casting sibling");
        assert!(!tampered.verify(root));
        let mut wrong_value = proof;
        wrong_value.value.push(0);
        assert!(!wrong_value.verify(root));
    }

    #[test]
    fn parliament_casting_fixed_write_must_appear_exactly_once() {
        let empty = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        assert!(parliament_timed_ovn_casting_witness_proof_v1(&empty).is_err());

        let snapshot = ParliamentTimedOvnCastingSnapshotCommitmentV1::empty(7);
        let value = norito::to_bytes(&snapshot).expect("encode casting snapshot");
        let duplicate = ExecWitness {
            reads: Vec::new(),
            writes: vec![
                ExecKv {
                    key: PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1.to_vec(),
                    value: value.clone(),
                },
                ExecKv {
                    key: PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1.to_vec(),
                    value,
                },
            ],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        assert!(parliament_timed_ovn_casting_witness_proof_v1(&duplicate).is_err());
    }

    #[test]
    fn kagemusha_receipt_proof_is_exact_and_duplicate_safe() {
        let operation_id = [0x61; 32];
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            Hash::new(b"kagemusha-receipt-proof"),
        ));
        let asset = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            iroha_data_model::DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        );
        let asset_incarnation = iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
            iroha_crypto::Hash::new(b"kagemusha-receipt-proof-incarnation").into(),
        )
        .expect("asset incarnation");
        let receipt = KagemushaReserveReceiptV1 {
            version: KAGEMUSHA_CHAIN_VERSION_V1,
            operation_id,
            kind: iroha_data_model::isi::kagemusha_v1::KagemushaOperationKindV1::TopUp,
            request_digest: [0x62; 32],
            mint_statement_digest: [0x64; 32],
            network_id,
            asset: asset.clone(),
            asset_incarnation,
            scale: 0,
            liability_pool_id: iroha_data_model::kagemusha::kagemusha_liability_pool_id_v1(
                &network_id,
                &asset,
                asset_incarnation,
            )
            .expect("liability pool"),
            amount: 7,
            previous_pool_receipt_digest: [0; 32],
            total_topups: 7,
            total_redemptions: 0,
            transaction_hash: [0x63; 32],
            committed_at_ms: 1,
        };
        let receipt_write = ExecKv {
            key: KagemushaReserveReceiptWitnessV1::expected_key(operation_id),
            value: norito::encode_canonical(&receipt).expect("canonical receipt"),
        };
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: vec![
                ExecKv {
                    key: b"ordinary".to_vec(),
                    value: b"value".to_vec(),
                },
                receipt_write.clone(),
            ],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let (proofs, root) =
            kagemusha_reserve_receipt_witnesses_v1(&witness).expect("receipt proof");
        assert_eq!(proofs.len(), 1);
        assert_eq!(proofs[0].receipt, receipt);
        assert!(proofs[0].verify(root));

        let duplicate = ExecWitness {
            writes: vec![receipt_write.clone(), receipt_write],
            ..ExecWitness::default()
        };
        assert!(kagemusha_reserve_receipt_witnesses_v1(&duplicate).is_err());
    }
}
