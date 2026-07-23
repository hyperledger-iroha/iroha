//! Witness-side proof construction for active-receiver snapshots.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;
use iroha_data_model::offline::{
    KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1, KagemushaActiveReceiverWitnessProofV1,
};
use iroha_data_model::validation_fee::{
    VALIDATION_FEE_POLICY_WITNESS_KEY_V1, ValidationFeePolicyWitnessProofV1,
};

use crate::sumeragi::{
    consensus::ExecWitness,
    smt::{KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG, KvPair},
};

/// Construct the exact fixed-key proof against the ordinary-write SMT.
pub(crate) fn active_receiver_witness_proof_v1(
    witness: &ExecWitness,
) -> Result<(KagemushaActiveReceiverWitnessProofV1, Hash), String> {
    let target_count = witness
        .writes
        .iter()
        .filter(|entry| entry.key.as_slice() == KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1)
        .count();
    if target_count != 1 {
        return Err(format!(
            "execution witness contains {target_count} active-receiver synthetic writes; expected exactly one"
        ));
    }

    let mut canonical = BTreeMap::<Vec<u8>, Vec<u8>>::new();
    for entry in &witness.writes {
        canonical.insert(entry.key.clone(), entry.value.clone());
    }
    let ordinary = canonical
        .into_iter()
        .filter(|(key, _)| key.first() != Some(&KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG))
        .map(|(key, value)| KvPair::new(key, value))
        .collect::<Vec<_>>();
    let ordinary_root = crate::sumeragi::smt::compute_post_state_root(&[], &ordinary);
    let target = ordinary
        .iter()
        .find(|pair| pair.key.as_slice() == KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1)
        .ok_or_else(|| {
            "active-receiver synthetic write is absent from ordinary writes".to_owned()
        })?;
    let siblings = sparse_smt_siblings(&ordinary, target)?;
    let proof = KagemushaActiveReceiverWitnessProofV1 {
        key: target.key.clone(),
        value: target.value.clone(),
        siblings,
    };
    if !proof.verify(ordinary_root) {
        return Err("constructed active-receiver witness proof does not reconstruct the ordinary-write root".to_owned());
    }
    Ok((proof, ordinary_root))
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
        .filter(|(key, _)| key.first() != Some(&KAGEMUSHA_V4_TOPUP_ANCHOR_WITNESS_KEY_TAG))
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
    use crate::sumeragi::consensus::ExecKv;
    use iroha_data_model::offline::{
        KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_VERSION_V1, KagemushaActiveReceiverSnapshotCommitmentV1,
        KagemushaActiveReceiverSnapshotStatusV1,
    };

    fn snapshot_value() -> Vec<u8> {
        norito::to_bytes(&KagemushaActiveReceiverSnapshotCommitmentV1 {
            version: KAGEMUSHA_ACTIVE_RECEIVER_SNAPSHOT_VERSION_V1,
            evaluated_height: 7,
            evaluated_at_ms: 1_000,
            status: KagemushaActiveReceiverSnapshotStatusV1::Available(Hash::new(b"policy")),
            leaf_count: 0,
            tree_root: Hash::new(b"empty receiver tree"),
        })
        .expect("encode snapshot")
    }

    #[test]
    fn fixed_write_proof_matches_consensus_ordinary_root_and_rejects_tampering() {
        let witness = ExecWitness {
            reads: Vec::new(),
            writes: vec![
                ExecKv {
                    key: b"ordinary-a".to_vec(),
                    value: b"one".to_vec(),
                },
                ExecKv {
                    key: KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1.to_vec(),
                    value: snapshot_value(),
                },
                ExecKv {
                    key: b"ordinary-b".to_vec(),
                    value: b"two".to_vec(),
                },
            ],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        let (proof, root) = active_receiver_witness_proof_v1(&witness).expect("proof");
        assert_eq!(proof.siblings.len(), 256);
        assert!(proof.verify(root));
        let mut tampered = proof.clone();
        tampered.siblings[127] = Hash::new(b"tampered sibling");
        assert!(!tampered.verify(root));
    }

    #[test]
    fn fixed_write_must_appear_exactly_once() {
        let empty = ExecWitness {
            reads: Vec::new(),
            writes: Vec::new(),
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        assert!(active_receiver_witness_proof_v1(&empty).is_err());
        let duplicate = ExecWitness {
            reads: Vec::new(),
            writes: vec![
                ExecKv {
                    key: KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1.to_vec(),
                    value: snapshot_value(),
                },
                ExecKv {
                    key: KAGEMUSHA_ACTIVE_RECEIVER_WITNESS_KEY_V1.to_vec(),
                    value: snapshot_value(),
                },
            ],
            fastpq_transcripts: Vec::new(),
            fastpq_batches: Vec::new(),
        };
        assert!(active_receiver_witness_proof_v1(&duplicate).is_err());
    }
}
