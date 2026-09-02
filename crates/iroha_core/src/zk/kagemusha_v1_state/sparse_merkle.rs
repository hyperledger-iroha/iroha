//! Exact sparse-Merkle index for consumed Kagemusha credit identities.

use std::collections::BTreeMap;

use halo2_proofs::halo2curves::pasta::{Fp, Fq};
use iroha_data_model::kagemusha::KagemushaPastaStateCommitmentV1;

use super::{
    ConsumedCreditInsertWitnessV1, ConsumedCreditRecordV1, CreditIdV1, DigestV1,
    KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1, KagemushaStateErrorV1,
};
use crate::zk::kagemusha_v1_poseidon::{
    KAGEMUSHA_REPLAY_EMPTY_DOMAIN_V1, KAGEMUSHA_REPLAY_LEAF_DOMAIN_V1,
    KAGEMUSHA_REPLAY_NODE_DOMAIN_V1, decode, digest_limbs, encode, hash,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NodePosition {
    depth: u16,
    prefix: DigestV1,
}

/// Opaque, prehashed replay-tree mutation assembled against one exact starting root.
///
/// The successor paths are deliberately retained outside the public proof witness. Safe Rust
/// callers can inspect the witnesses needed by the circuit, but cannot substitute the prehashed
/// nodes installed after transition authorization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PreparedConsumedCreditBatchV1 {
    starting_root: KagemushaPastaStateCommitmentV1,
    final_root: KagemushaPastaStateCommitmentV1,
    inserts: Vec<PreparedConsumedCreditInsertV1>,
    final_overlay: BTreeMap<NodePosition, Option<KagemushaPastaStateCommitmentV1>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreparedConsumedCreditInsertV1 {
    credit_id: CreditIdV1,
    envelope_digest: DigestV1,
    witness: ConsumedCreditInsertWitnessV1,
    successor_path:
        [KagemushaPastaStateCommitmentV1; KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1 + 1],
}

impl PreparedConsumedCreditBatchV1 {
    pub(super) fn len(&self) -> usize {
        self.inserts.len()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.inserts.is_empty()
    }

    pub(super) fn starting_root(&self) -> KagemushaPastaStateCommitmentV1 {
        self.starting_root
    }

    pub(super) fn final_root(&self) -> KagemushaPastaStateCommitmentV1 {
        self.final_root
    }

    pub(super) fn witness(&self, index: usize) -> Option<&ConsumedCreditInsertWitnessV1> {
        self.inserts.get(index).map(|insert| &insert.witness)
    }
}

/// Host-owned exact dictionary and its incrementally maintained sparse-Merkle tree.
///
/// The dictionary deliberately retains the full credit-id to envelope-digest mapping. The
/// constant-size root is committed by the private balance state; the dictionary is recovery
/// material and is never placed in a peer payment. This avoids finite replay windows and
/// probabilistic false positives.
#[derive(Clone, Debug)]
pub(super) struct ExactConsumedCreditIndex {
    records: BTreeMap<CreditIdV1, DigestV1>,
    nodes: BTreeMap<NodePosition, KagemushaPastaStateCommitmentV1>,
    empty_at_depth: [KagemushaPastaStateCommitmentV1; 257],
}

impl ExactConsumedCreditIndex {
    pub(super) fn empty() -> Self {
        let mut empty_at_depth = [KagemushaPastaStateCommitmentV1::ZERO; 257];
        empty_at_depth[256] = hash_empty_leaf();
        for depth in (0..256).rev() {
            empty_at_depth[depth] =
                hash_node_unchecked(&empty_at_depth[depth + 1], &empty_at_depth[depth + 1]);
        }
        Self {
            records: BTreeMap::new(),
            nodes: BTreeMap::new(),
            empty_at_depth,
        }
    }

    pub(super) fn from_records(
        records: &[ConsumedCreditRecordV1],
    ) -> Result<Self, KagemushaStateErrorV1> {
        let mut index = Self::empty();
        let mut previous = None;
        for record in records {
            if record.credit_id.is_zero() || record.envelope_digest == [0; 32] {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            if previous.is_some_and(|prior| prior >= record.credit_id) {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            index.insert(record.credit_id, record.envelope_digest)?;
            previous = Some(record.credit_id);
        }
        Ok(index)
    }

    pub(super) fn root(&self) -> KagemushaPastaStateCommitmentV1 {
        self.nodes
            .get(&NodePosition {
                depth: 0,
                prefix: [0; 32],
            })
            .copied()
            .unwrap_or(self.empty_at_depth[0])
    }

    pub(super) fn len(&self) -> usize {
        self.records.len()
    }

    pub(super) fn get(&self, credit_id: CreditIdV1) -> Option<DigestV1> {
        self.records.get(&credit_id).copied()
    }

    pub(super) fn records(&self) -> Vec<ConsumedCreditRecordV1> {
        self.records
            .iter()
            .map(|(credit_id, envelope_digest)| ConsumedCreditRecordV1 {
                credit_id: *credit_id,
                envelope_digest: *envelope_digest,
            })
            .collect()
    }

    pub(super) fn preview_insert_witness(
        &self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
    ) -> Result<ConsumedCreditInsertWitnessV1, KagemushaStateErrorV1> {
        if let Some(existing) = self.get(credit_id) {
            return if existing == envelope_digest {
                Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id))
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(credit_id))
            };
        }
        if credit_id.is_zero() || envelope_digest == [0; 32] {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        let siblings_root_to_leaf = self.siblings_root_to_leaf(credit_id);
        Ok(ConsumedCreditInsertWitnessV1 {
            credit_id,
            envelope_digest,
            predecessor_root: self.root(),
            successor_root: path_root_from_siblings(
                credit_id,
                hash_present_leaf(credit_id, envelope_digest),
                &siblings_root_to_leaf,
            )?,
            siblings_root_to_leaf,
        })
    }

    pub(super) fn insert(
        &mut self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let prepared = self.prepare_batch_inserts(&[(credit_id, envelope_digest)])?;
        self.install_prepared_batch(prepared)
    }

    pub(super) fn insert_with_witness(
        &mut self,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
        witness: &ConsumedCreditInsertWitnessV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if let Some(existing) = self.get(credit_id) {
            return if existing == envelope_digest {
                Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id))
            } else {
                Err(KagemushaStateErrorV1::CreditConflict(credit_id))
            };
        }
        if credit_id.is_zero()
            || envelope_digest == [0; 32]
            || witness.credit_id != credit_id
            || witness.envelope_digest != envelope_digest
            || witness.predecessor_root != self.root()
            || witness.successor_root.is_zero()
            || witness.successor_root == witness.predecessor_root
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }

        let siblings_root_to_leaf = self.siblings_root_to_leaf(credit_id);
        if witness.siblings_root_to_leaf != siblings_root_to_leaf {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        let successor_path = path_hashes_from_siblings(
            credit_id,
            hash_present_leaf(credit_id, envelope_digest),
            &siblings_root_to_leaf,
        )?;
        if successor_path[0] != witness.successor_root {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }

        let mut final_overlay = BTreeMap::new();
        apply_successor_path_to_overlay(
            &mut final_overlay,
            &self.empty_at_depth,
            credit_id,
            &successor_path,
        );
        self.install_prepared_batch(PreparedConsumedCreditBatchV1 {
            starting_root: witness.predecessor_root,
            final_root: witness.successor_root,
            inserts: vec![PreparedConsumedCreditInsertV1 {
                credit_id,
                envelope_digest,
                witness: witness.clone(),
                successor_path,
            }],
            final_overlay,
        })
    }

    /// Prepare one or more sequential inserts over a small node overlay without cloning the
    /// retained replay dictionary or its base tree.
    pub(super) fn prepare_batch_inserts(
        &self,
        inserts: &[(CreditIdV1, DigestV1)],
    ) -> Result<PreparedConsumedCreditBatchV1, KagemushaStateErrorV1> {
        if inserts.is_empty() {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        let starting_root = self.root();
        let mut predecessor_root = starting_root;
        let mut overlay = BTreeMap::new();
        let mut batch_records = BTreeMap::new();
        let mut prepared = Vec::with_capacity(inserts.len());

        for &(credit_id, envelope_digest) in inserts {
            if let Some(existing) = self
                .get(credit_id)
                .or_else(|| batch_records.get(&credit_id).copied())
            {
                return if existing == envelope_digest {
                    Err(KagemushaStateErrorV1::CreditAlreadyConsumed(credit_id))
                } else {
                    Err(KagemushaStateErrorV1::CreditConflict(credit_id))
                };
            }
            if credit_id.is_zero() || envelope_digest == [0; 32] {
                return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
            }

            let siblings_root_to_leaf =
                self.siblings_root_to_leaf_with_overlay(credit_id, &overlay);
            let successor_path = path_hashes_from_siblings(
                credit_id,
                hash_present_leaf(credit_id, envelope_digest),
                &siblings_root_to_leaf,
            )?;
            let successor_root = successor_path[0];
            if successor_root.is_zero() || successor_root == predecessor_root {
                return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
            }
            let witness = ConsumedCreditInsertWitnessV1 {
                credit_id,
                envelope_digest,
                predecessor_root,
                successor_root,
                siblings_root_to_leaf,
            };
            apply_successor_path_to_overlay(
                &mut overlay,
                &self.empty_at_depth,
                credit_id,
                &successor_path,
            );
            batch_records.insert(credit_id, envelope_digest);
            prepared.push(PreparedConsumedCreditInsertV1 {
                credit_id,
                envelope_digest,
                witness,
                successor_path,
            });
            predecessor_root = successor_root;
        }

        Ok(PreparedConsumedCreditBatchV1 {
            starting_root,
            final_root: predecessor_root,
            inserts: prepared,
            final_overlay: overlay,
        })
    }

    /// Atomically install one opaque batch after cheap stale-root/path binding checks.
    ///
    /// All hashes were computed by [`Self::prepare_batch_inserts`] before hardware authorization.
    /// This commit pass validates exact local siblings, root chaining, identities, and the private
    /// prehashed overlay before mutating either retained map.
    pub(super) fn install_prepared_batch(
        &mut self,
        prepared: PreparedConsumedCreditBatchV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if prepared.is_empty()
            || prepared.starting_root != self.root()
            || prepared.final_root.is_zero()
            || prepared.final_root == prepared.starting_root
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }

        let mut validation_overlay = BTreeMap::new();
        let mut predecessor_root = prepared.starting_root;
        let mut batch_records = BTreeMap::new();
        for insert in &prepared.inserts {
            if let Some(existing) = self
                .get(insert.credit_id)
                .or_else(|| batch_records.get(&insert.credit_id).copied())
            {
                return if existing == insert.envelope_digest {
                    Err(KagemushaStateErrorV1::CreditAlreadyConsumed(
                        insert.credit_id,
                    ))
                } else {
                    Err(KagemushaStateErrorV1::CreditConflict(insert.credit_id))
                };
            }
            let expected_siblings =
                self.siblings_root_to_leaf_with_overlay(insert.credit_id, &validation_overlay);
            if insert.credit_id.is_zero()
                || insert.envelope_digest == [0; 32]
                || insert.witness.credit_id != insert.credit_id
                || insert.witness.envelope_digest != insert.envelope_digest
                || insert.witness.predecessor_root != predecessor_root
                || insert.witness.siblings_root_to_leaf != expected_siblings
                || insert.successor_path[0] != insert.witness.successor_root
                || insert.successor_path[KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1]
                    != hash_present_leaf(insert.credit_id, insert.envelope_digest)
                || insert.witness.successor_root.is_zero()
                || insert.witness.successor_root == predecessor_root
            {
                return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
            }
            apply_successor_path_to_overlay(
                &mut validation_overlay,
                &self.empty_at_depth,
                insert.credit_id,
                &insert.successor_path,
            );
            batch_records.insert(insert.credit_id, insert.envelope_digest);
            predecessor_root = insert.witness.successor_root;
        }
        if predecessor_root != prepared.final_root || validation_overlay != prepared.final_overlay {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }

        for (position, value) in prepared.final_overlay {
            if let Some(value) = value {
                self.nodes.insert(position, value);
            } else {
                self.nodes.remove(&position);
            }
        }
        self.records.extend(batch_records);
        debug_assert_eq!(self.root(), prepared.final_root);
        Ok(())
    }

    fn siblings_root_to_leaf(
        &self,
        credit_id: CreditIdV1,
    ) -> [KagemushaPastaStateCommitmentV1; KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1] {
        core::array::from_fn(|parent_depth| {
            let child_depth = parent_depth + 1;
            self.nodes
                .get(&NodePosition {
                    depth: u16::try_from(child_depth).expect("sparse-Merkle depth is at most 256"),
                    prefix: sibling_prefix(&credit_id.0, parent_depth),
                })
                .copied()
                .unwrap_or(self.empty_at_depth[child_depth])
        })
    }

    fn siblings_root_to_leaf_with_overlay(
        &self,
        credit_id: CreditIdV1,
        overlay: &BTreeMap<NodePosition, Option<KagemushaPastaStateCommitmentV1>>,
    ) -> [KagemushaPastaStateCommitmentV1; KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1] {
        core::array::from_fn(|parent_depth| {
            let child_depth = parent_depth + 1;
            let position = NodePosition {
                depth: u16::try_from(child_depth).expect("sparse-Merkle depth is at most 256"),
                prefix: sibling_prefix(&credit_id.0, parent_depth),
            };
            match overlay.get(&position) {
                Some(Some(value)) => *value,
                Some(None) => self.empty_at_depth[child_depth],
                None => self
                    .nodes
                    .get(&position)
                    .copied()
                    .unwrap_or(self.empty_at_depth[child_depth]),
            }
        })
    }
}

fn apply_successor_path_to_overlay(
    overlay: &mut BTreeMap<NodePosition, Option<KagemushaPastaStateCommitmentV1>>,
    empty_at_depth: &[KagemushaPastaStateCommitmentV1; 257],
    credit_id: CreditIdV1,
    successor_path: &[KagemushaPastaStateCommitmentV1;
         KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1 + 1],
) {
    for (depth, &path_hash) in successor_path.iter().enumerate() {
        let position = NodePosition {
            depth: u16::try_from(depth).expect("sparse-Merkle depth is at most 256"),
            prefix: prefix_at_depth(&credit_id.0, depth),
        };
        overlay.insert(
            position,
            (path_hash != empty_at_depth[depth]).then_some(path_hash),
        );
    }
}

impl ConsumedCreditInsertWitnessV1 {
    /// Return the protocol-fixed canonical empty-leaf digest.
    #[must_use]
    pub fn canonical_empty_leaf_digest() -> KagemushaPastaStateCommitmentV1 {
        hash_empty_leaf()
    }

    /// Return the canonical present-leaf digest bound to this credit and envelope.
    #[must_use]
    pub fn canonical_present_leaf_digest(&self) -> KagemushaPastaStateCommitmentV1 {
        hash_present_leaf(self.credit_id, self.envelope_digest)
    }

    /// Verify the fixed empty/present leaf relations and both exact sparse-Merkle roots.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness`] when any identity,
    /// path bit, sibling, leaf, or root relation is invalid.
    pub fn verify(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.credit_id.is_zero()
            || self.envelope_digest == [0; 32]
            || self.predecessor_root.is_zero()
            || self.successor_root.is_zero()
            || self.predecessor_root == self.successor_root
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        let predecessor_root = path_root_from_siblings(
            self.credit_id,
            hash_empty_leaf(),
            &self.siblings_root_to_leaf,
        )?;
        let successor_root = path_root_from_siblings(
            self.credit_id,
            self.canonical_present_leaf_digest(),
            &self.siblings_root_to_leaf,
        )?;
        if predecessor_root != self.predecessor_root || successor_root != self.successor_root {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        Ok(())
    }

    /// Verify this witness against the exact transition inputs and roots.
    ///
    /// # Errors
    ///
    /// Rejects any substitution of the credit ID, envelope digest, predecessor root, successor
    /// root, path, or the protocol-fixed leaf hash relations.
    pub fn verify_binding(
        &self,
        predecessor_root: KagemushaPastaStateCommitmentV1,
        credit_id: CreditIdV1,
        envelope_digest: DigestV1,
        successor_root: KagemushaPastaStateCommitmentV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.predecessor_root != predecessor_root
            || self.credit_id != credit_id
            || self.envelope_digest != envelope_digest
            || self.successor_root != successor_root
        {
            return Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness);
        }
        self.verify()
    }
}

fn hash_empty_leaf() -> KagemushaPastaStateCommitmentV1 {
    KagemushaPastaStateCommitmentV1 {
        eq: encode(hash::<Fp>(KAGEMUSHA_REPLAY_EMPTY_DOMAIN_V1, &[])),
        ep: encode(hash::<Fq>(KAGEMUSHA_REPLAY_EMPTY_DOMAIN_V1, &[])),
    }
}

fn hash_present_leaf(
    credit_id: CreditIdV1,
    envelope_digest: DigestV1,
) -> KagemushaPastaStateCommitmentV1 {
    let eq_credit = digest_limbs::<Fp>(credit_id.0);
    let eq_envelope = digest_limbs::<Fp>(envelope_digest);
    let ep_credit = digest_limbs::<Fq>(credit_id.0);
    let ep_envelope = digest_limbs::<Fq>(envelope_digest);
    KagemushaPastaStateCommitmentV1 {
        eq: encode(hash(
            KAGEMUSHA_REPLAY_LEAF_DOMAIN_V1,
            &[eq_credit[0], eq_credit[1], eq_envelope[0], eq_envelope[1]],
        )),
        ep: encode(hash(
            KAGEMUSHA_REPLAY_LEAF_DOMAIN_V1,
            &[ep_credit[0], ep_credit[1], ep_envelope[0], ep_envelope[1]],
        )),
    }
}

fn path_root_from_siblings(
    credit_id: CreditIdV1,
    leaf: KagemushaPastaStateCommitmentV1,
    siblings_root_to_leaf: &[KagemushaPastaStateCommitmentV1;
         KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1],
) -> Result<KagemushaPastaStateCommitmentV1, KagemushaStateErrorV1> {
    Ok(path_hashes_from_siblings(credit_id, leaf, siblings_root_to_leaf)?[0])
}

fn path_hashes_from_siblings(
    credit_id: CreditIdV1,
    leaf: KagemushaPastaStateCommitmentV1,
    siblings_root_to_leaf: &[KagemushaPastaStateCommitmentV1;
         KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1],
) -> Result<
    [KagemushaPastaStateCommitmentV1; KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1 + 1],
    KagemushaStateErrorV1,
> {
    let key = credit_id.0;
    let mut path =
        [KagemushaPastaStateCommitmentV1::ZERO; KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1 + 1];
    path[KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1] = leaf;
    for parent_depth in (0..KAGEMUSHA_CONSUMED_CREDIT_TREE_DEPTH_V1).rev() {
        let sibling = siblings_root_to_leaf[parent_depth];
        let path_hash = path[parent_depth + 1];
        let (left, right) = if key_bit(&key, parent_depth) {
            (sibling, path_hash)
        } else {
            (path_hash, sibling)
        };
        path[parent_depth] = hash_node(&left, &right)?;
    }
    Ok(path)
}

fn hash_node(
    left: &KagemushaPastaStateCommitmentV1,
    right: &KagemushaPastaStateCommitmentV1,
) -> Result<KagemushaPastaStateCommitmentV1, KagemushaStateErrorV1> {
    let left_eq =
        decode::<Fp>(left.eq).ok_or(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness)?;
    let right_eq = decode::<Fp>(right.eq)
        .ok_or(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness)?;
    let left_ep =
        decode::<Fq>(left.ep).ok_or(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness)?;
    let right_ep = decode::<Fq>(right.ep)
        .ok_or(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness)?;
    Ok(KagemushaPastaStateCommitmentV1 {
        eq: encode(hash(
            KAGEMUSHA_REPLAY_NODE_DOMAIN_V1,
            &[left_eq, right_eq],
        )),
        ep: encode(hash(
            KAGEMUSHA_REPLAY_NODE_DOMAIN_V1,
            &[left_ep, right_ep],
        )),
    })
}

fn hash_node_unchecked(
    left: &KagemushaPastaStateCommitmentV1,
    right: &KagemushaPastaStateCommitmentV1,
) -> KagemushaPastaStateCommitmentV1 {
    hash_node(left, right).expect("internally generated Poseidon nodes are canonical")
}

fn key_bit(key: &DigestV1, depth: usize) -> bool {
    let byte = key[depth / 8];
    let shift = 7 - (depth % 8);
    ((byte >> shift) & 1) == 1
}

fn prefix_at_depth(key: &DigestV1, depth: usize) -> DigestV1 {
    let mut prefix = [0_u8; 32];
    let full_bytes = depth / 8;
    prefix[..full_bytes].copy_from_slice(&key[..full_bytes]);
    let remaining_bits = depth % 8;
    if remaining_bits != 0 {
        let mask = u8::MAX << (8 - remaining_bits);
        prefix[full_bytes] = key[full_bytes] & mask;
    }
    prefix
}

fn sibling_prefix(key: &DigestV1, parent_depth: usize) -> DigestV1 {
    let mut sibling_key = *key;
    let byte_index = parent_depth / 8;
    let shift = 7 - (parent_depth % 8);
    sibling_key[byte_index] ^= 1 << shift;
    prefix_at_depth(&sibling_key, parent_depth + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credit(tag: u8) -> CreditIdV1 {
        let mut digest = [0_u8; 32];
        digest[0] = tag;
        digest[31] = tag.wrapping_mul(17);
        CreditIdV1(digest)
    }

    fn envelope(tag: u8) -> DigestV1 {
        let mut digest = [tag; 32];
        digest[0] ^= 0x5A;
        digest
    }

    #[test]
    fn overlay_batch_matches_sequential_exact_inserts() {
        let records = [
            (credit(0x11), envelope(0xA1)),
            (credit(0x80), envelope(0xA2)),
            (credit(0xC3), envelope(0xA3)),
        ];
        let mut batch = ExactConsumedCreditIndex::empty();
        let prepared = batch
            .prepare_batch_inserts(&records)
            .expect("prepare overlay batch");
        for index in 1..prepared.len() {
            assert_eq!(
                prepared.witness(index - 1).expect("prior").successor_root,
                prepared.witness(index).expect("next").predecessor_root
            );
        }
        batch
            .install_prepared_batch(prepared)
            .expect("install overlay batch");

        let mut sequential = ExactConsumedCreditIndex::empty();
        for &(credit_id, envelope_digest) in &records {
            let witness = sequential
                .preview_insert_witness(credit_id, envelope_digest)
                .expect("preview sequential insert");
            sequential
                .insert_with_witness(credit_id, envelope_digest, &witness)
                .expect("install sequential insert");
        }

        assert_eq!(batch.root(), sequential.root());
        assert_eq!(batch.records(), sequential.records());
    }

    #[test]
    fn prepared_batch_rejects_tampered_sibling_without_mutation() {
        let inserts = [
            (credit(0x21), envelope(0xB1)),
            (credit(0xE2), envelope(0xB2)),
        ];
        let mut index = ExactConsumedCreditIndex::empty();
        let starting_root = index.root();
        let mut prepared = index
            .prepare_batch_inserts(&inserts)
            .expect("prepare overlay batch");
        prepared.inserts[1].witness.siblings_root_to_leaf[0] =
            KagemushaPastaStateCommitmentV1::ZERO;

        assert_eq!(
            index.install_prepared_batch(prepared),
            Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness)
        );
        assert_eq!(index.root(), starting_root);
        assert!(index.records().is_empty());
    }

    #[test]
    fn prepared_batch_rejects_stale_starting_root_atomically() {
        let pending = [(credit(0x31), envelope(0xC1))];
        let mut index = ExactConsumedCreditIndex::empty();
        let prepared = index
            .prepare_batch_inserts(&pending)
            .expect("prepare pending batch");
        index
            .insert(credit(0x72), envelope(0xC2))
            .expect("advance exact index");
        let advanced_root = index.root();
        let advanced_records = index.records();

        assert_eq!(
            index.install_prepared_batch(prepared),
            Err(KagemushaStateErrorV1::InvalidConsumedCreditInsertWitness)
        );
        assert_eq!(index.root(), advanced_root);
        assert_eq!(index.records(), advanced_records);
    }
}
