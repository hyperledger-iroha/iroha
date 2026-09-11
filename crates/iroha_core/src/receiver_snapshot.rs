//! Witness-side proof construction for consensus-authenticated synthetic writes.
use crate::sumeragi::smt::KvPair;
use iroha_crypto::Hash;
use iroha_data_model::block::consensus::ExecWitness;
use iroha_data_model::execution_witness::KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1;
use iroha_data_model::isi::kagemusha_v1::{
    KagemushaReserveReceiptV1, KagemushaReserveReceiptWitnessV1,
};
use iroha_data_model::parliament_casting::{
    PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1, ParliamentTimedOvnCastingWitnessProofV1,
};
use iroha_data_model::validation_fee::{
    VALIDATION_FEE_POLICY_WITNESS_KEY_V1, ValidationFeePolicyWitnessProofV1,
};
use std::collections::{BTreeMap, BTreeSet};

/// Caller-supplied bounds for constructing one local FASTPQ source opening or archive.
#[derive(Debug, Clone, Copy)]
pub struct FastpqSourceOpeningBuildLimits {
    /// Maximum ordinary writes inspected before cloning any witness key or value.
    pub max_ordinary_writes: usize,
    /// Maximum aggregate raw key/value bytes across those ordinary writes.
    pub max_ordinary_write_bytes: usize,
    /// Maximum complete executed-entry count.
    pub max_executed_entries: u32,
    /// Maximum recorded transfer-operation statements across all entries.
    pub max_statements: u32,
    /// Manifest decoding budget, preserving any stricter enclosing Norito scope.
    pub manifest_decode: norito::DecodeLimits,
}

/// Complete local source archive with one shared ordinary-write path.
///
/// The leaves remain borrowed from the caller's exact archive. Neither this local
/// result nor its computed root authenticates execution ownership or finality.
pub(crate) struct FastpqOrdinarySourceArchive<'leaves> {
    /// Canonical manifest matching every supplied leaf and the fixed witness write.
    pub(crate) manifest: iroha_data_model::fastpq::FastpqOrdinarySourceStatementManifestV1,
    /// Complete validated leaf sequence, without an additional owned copy.
    pub(crate) leaves: &'leaves [iroha_data_model::fastpq::FastpqOrdinarySourceStatementLeafV1],
    /// Exactly 256 siblings for the one fixed D7 ordinary-write key.
    pub(crate) manifest_siblings: Vec<Hash>,
    /// Computed ordinary-write root, requiring independent finality authentication.
    pub(crate) ordinary_root: Hash,
}

/// Bounded manifest and complete-leaf validation before ordinary tree allocation.
///
/// Separating preparation from [`Self::build`] lets a leaf-opening caller reject
/// absent positions before cloning ordinary writes, preserving its existing order.
pub(crate) struct PreparedFastpqOrdinarySourceArchive<'witness, 'leaves> {
    witness: &'witness ExecWitness,
    target: &'witness iroha_data_model::block::consensus::ExecKv,
    manifest: iroha_data_model::fastpq::FastpqOrdinarySourceStatementManifestV1,
    leaves: &'leaves [iroha_data_model::fastpq::FastpqOrdinarySourceStatementLeafV1],
    limits: FastpqSourceOpeningBuildLimits,
}

/// Validate the exact fixed manifest and complete borrowed source leaf archive.
///
/// The caller must supply the independently obtained complete execution-entry inventory,
/// including entries without transfers. It is never reconstructed from the leaves or the
/// advertised manifest count. Preparation binds every entry and every leaf to that inventory.
/// This accepts a canonical empty manifest without inventing a leaf opening.
/// Counts and raw ordinary-write bytes are bounded before cloning any key/value.
/// The returned preparation does not establish execution ownership or finality.
///
/// # Errors
/// Rejects exceeded bounds, malformed or duplicate reserved keys, noncanonical
/// manifests, and inconsistent source context, complete leaf order or counts.
pub(crate) fn prepare_fastpq_ordinary_source_archive_v1<'witness, 'leaves>(
    witness: &'witness ExecWitness,
    source: iroha_data_model::fastpq::FastpqSourceStatementContextV1,
    expected_entries: &[iroha_data_model::fastpq::FastpqSourceExecutionEntryV1],
    leaves: &'leaves [iroha_data_model::fastpq::FastpqOrdinarySourceStatementLeafV1],
    limits: FastpqSourceOpeningBuildLimits,
) -> Result<PreparedFastpqOrdinarySourceArchive<'witness, 'leaves>, String> {
    use iroha_data_model::{
        execution_witness::FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1,
        fastpq::{
            FASTPQ_SOURCE_STATEMENT_MANIFEST_MAX_BYTES_V1, FastpqOrdinarySourceStatementManifestV1,
            build_fastpq_ordinary_source_statement_manifest_v1,
        },
    };
    if !u32::try_from(expected_entries.len())
        .is_ok_and(|count| count <= limits.max_executed_entries)
        || witness.writes.len() > limits.max_ordinary_writes
        || !u32::try_from(leaves.len()).is_ok_and(|count| count <= limits.max_statements)
    {
        return Err("FASTPQ source opening exceeds its write or entry count cap".to_owned());
    }
    let mut total_bytes = 0_usize;
    let mut manifest_write = None;
    for write in &witness.writes {
        total_bytes = total_bytes
            .checked_add(write.key.len())
            .and_then(|total| total.checked_add(write.value.len()))
            .filter(|total| *total <= limits.max_ordinary_write_bytes)
            .ok_or_else(|| {
                "FASTPQ source opening exceeds its ordinary-write byte cap".to_owned()
            })?;
        if write.key.first() == FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1.first() {
            if write.key != FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1
                || manifest_write.is_some()
            {
                return Err(
                    "FASTPQ source witness contains a malformed or duplicate reserved key"
                        .to_owned(),
                );
            }
            manifest_write = Some(write);
        }
    }
    let target =
        manifest_write.ok_or_else(|| "FASTPQ source manifest write is absent".to_owned())?;
    if target.value.len() > FASTPQ_SOURCE_STATEMENT_MANIFEST_MAX_BYTES_V1 {
        return Err("FASTPQ source manifest exceeds its canonical frame cap".to_owned());
    }
    let manifest: FastpqOrdinarySourceStatementManifestV1 =
        norito::decode_canonical_with_limits(&target.value, limits.manifest_decode).map_err(
            |error| format!("FASTPQ source manifest is not bounded canonical Norito: {error}"),
        )?;
    let rebuilt = build_fastpq_ordinary_source_statement_manifest_v1(
        source,
        expected_entries,
        leaves,
        limits.max_executed_entries,
        limits.max_statements,
    )
    .ok_or_else(|| "FASTPQ source leaf archive has invalid context, order or count".to_owned())?;
    if manifest != rebuilt {
        return Err("FASTPQ source manifest differs from its complete leaf archive".to_owned());
    }
    Ok(PreparedFastpqOrdinarySourceArchive {
        witness,
        target,
        manifest,
        leaves,
        limits,
    })
}

impl<'leaves> PreparedFastpqOrdinarySourceArchive<'_, 'leaves> {
    /// Construct the one shared ordinary-write path from the bounded witness.
    ///
    /// Other keys retain the ordinary tree's existing last-write-wins projection.
    /// Empty source archives still prove the actual manifest write. The returned
    /// leaves borrow only the original archive, so the witness can be released.
    ///
    /// # Errors
    /// Rejects a sparse-tree construction failure or a path that does not
    /// reconstruct the manifest under the computed ordinary-write root.
    pub(crate) fn build(self) -> Result<FastpqOrdinarySourceArchive<'leaves>, String> {
        use iroha_data_model::fastpq::verify_fastpq_ordinary_source_statement_manifest_write_v1;

        let canonical = self
            .witness
            .writes
            .iter()
            .map(|write| (write.key.clone(), write.value.clone()))
            .collect::<BTreeMap<_, _>>();
        let ordinary = canonical
            .into_iter()
            .map(|(key, value)| KvPair::new(key, value))
            .collect::<Vec<_>>();
        let ordinary_root = crate::sumeragi::smt::compute_post_state_root(&[], &ordinary);
        let target = KvPair::new(self.target.key.clone(), self.target.value.clone());
        let manifest_siblings = sparse_smt_siblings(&ordinary, &target)?;
        if !verify_fastpq_ordinary_source_statement_manifest_write_v1(
            &self.manifest,
            self.manifest.source,
            &manifest_siblings,
            ordinary_root,
            self.limits.max_executed_entries,
            self.limits.max_statements,
        ) {
            return Err(
                "constructed FASTPQ source opening does not reconstruct the ordinary-write root"
                    .to_owned(),
            );
        }
        Ok(FastpqOrdinarySourceArchive {
            manifest: self.manifest,
            leaves: self.leaves,
            manifest_siblings,
            ordinary_root,
        })
    }
}

/// Construct a bounded complete transport archive with one shared manifest path.
///
/// The existing preparation validates raw write/count/byte limits, the canonical
/// D7 manifest and every supplied leaf before ordinary witness keys/values are
/// copied. One shared archive build derives and checks its ordinary-write path;
/// it is not repeated for individual leaves. The caller's already-owned leaf
/// allocation is moved into the payload without a second copy, including when
/// the leaf sequence is empty. The witness is only borrowed and is not retained.
///
/// The caller supplies the complete independently obtained execution-entry inventory,
/// including entries with no transfer leaves. It is checked against the manifest's
/// source-entry commitment and every leaf before the witness tree is allocated.
/// The payload and returned computed ordinary-write root establish consistency
/// with the supplied witness/source/entries only. They do not authenticate finality,
/// validator-owned execution completeness, durable retention or spend authority.
/// These construction limits do not select a production profile or bound encoded
/// transport bytes; callers must separately bound canonical encoding/decoding.
/// TODO: integrate the durable source owner and independently authenticated
/// finality admission after the D7 policy and execution-accounting gates are met.
///
/// # Errors
/// Rejects every preparation/build error unchanged, or a manifest path whose
/// depth cannot be represented by the transport's exact 256-sibling array.
pub fn fastpq_ordinary_source_statement_archive_v1(
    witness: &ExecWitness,
    source: iroha_data_model::fastpq::FastpqSourceStatementContextV1,
    expected_entries: &[iroha_data_model::fastpq::FastpqSourceExecutionEntryV1],
    leaves: Vec<iroha_data_model::fastpq::FastpqOrdinarySourceStatementLeafV1>,
    limits: FastpqSourceOpeningBuildLimits,
) -> Result<
    (
        iroha_data_model::fastpq::FastpqOrdinarySourceStatementArchiveV1,
        Hash,
    ),
    String,
> {
    use iroha_data_model::fastpq::{
        FASTPQ_ORDINARY_SOURCE_STATEMENT_ARCHIVE_VERSION_V1, FastpqOrdinarySourceStatementArchiveV1,
    };

    let (manifest, manifest_siblings, ordinary_root) = {
        let archive = prepare_fastpq_ordinary_source_archive_v1(
            witness,
            source,
            expected_entries,
            &leaves,
            limits,
        )?
        .build()?;
        (
            archive.manifest,
            archive.manifest_siblings,
            archive.ordinary_root,
        )
    };
    let manifest_siblings = manifest_siblings.try_into().map_err(|_| {
        "FASTPQ source archive manifest path does not have exactly 256 siblings".to_owned()
    })?;
    Ok((
        FastpqOrdinarySourceStatementArchiveV1 {
            version: FASTPQ_ORDINARY_SOURCE_STATEMENT_ARCHIVE_VERSION_V1,
            manifest,
            leaves,
            manifest_siblings,
        },
        ordinary_root,
    ))
}

/// Construct one bounded opening from an exact local witness and complete leaf archive.
///
/// The reserved FASTPQ key family must contain exactly one canonical fixed-key
/// manifest. Its source-entry commitment must match the caller's independently obtained
/// complete inventory, including entries without transfers, and its root/count must match
/// every supplied ordered leaf before an opening is returned. Other ordinary writes use
/// the existing consensus last-write-wins projection and sparse-tree proof constructor.
///
/// Both the returned opening and computed root remain untrusted. This function
/// neither authenticates the witness nor attests that its manifest was derived by
/// validator execution. A consumer still needs independently authenticated finality.
///
/// # Errors
/// Rejects exceeded bounds, malformed/duplicate reserved keys, noncanonical
/// manifests, inconsistent source/leaf archives, absent positions and proof errors.
pub fn fastpq_ordinary_source_statement_opening_v1(
    witness: &ExecWitness,
    source: iroha_data_model::fastpq::FastpqSourceStatementContextV1,
    expected_entries: &[iroha_data_model::fastpq::FastpqSourceExecutionEntryV1],
    leaves: &[iroha_data_model::fastpq::FastpqOrdinarySourceStatementLeafV1],
    statement_index: u32,
    limits: FastpqSourceOpeningBuildLimits,
) -> Result<
    (
        iroha_data_model::fastpq::FastpqOrdinarySourceStatementOpeningV1,
        Hash,
    ),
    String,
> {
    use iroha_crypto::MerkleTree;
    use iroha_data_model::fastpq::{
        FastpqOrdinarySourceStatementOpeningV1, fastpq_ordinary_source_statement_leaf_hash_v1,
        verify_fastpq_ordinary_source_statement_membership_v1,
    };
    let prepared = prepare_fastpq_ordinary_source_archive_v1(
        witness,
        source,
        expected_entries,
        leaves,
        limits,
    )?;
    let leaf_index = usize::try_from(statement_index)
        .map_err(|_| "FASTPQ source statement index is not representable".to_owned())?;
    prepared
        .leaves
        .get(leaf_index)
        .ok_or_else(|| "FASTPQ source statement index is absent".to_owned())?;
    let hashes = leaves
        .iter()
        .map(|leaf| {
            fastpq_ordinary_source_statement_leaf_hash_v1(leaf)
                .ok_or_else(|| "FASTPQ source leaf exceeds its canonical encoding bound".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let tree: MerkleTree<_> = hashes.into_iter().collect();
    let membership = tree
        .get_proof(statement_index)
        .ok_or_else(|| "FASTPQ source statement membership is absent".to_owned())?;
    let archive = prepared.build()?;
    let leaf = archive.leaves[leaf_index];
    let opening = FastpqOrdinarySourceStatementOpeningV1 {
        manifest: archive.manifest,
        leaf,
        membership,
        manifest_siblings: archive.manifest_siblings,
    };
    // The common archive builder already verified the manifest's ordinary-write
    // inclusion. Keep the per-leaf membership check without hashing that path twice.
    if !verify_fastpq_ordinary_source_statement_membership_v1(
        &opening.leaf,
        &leaf,
        &opening.manifest,
        &opening.membership,
        limits.max_executed_entries,
        limits.max_statements,
    ) {
        return Err(
            "constructed FASTPQ source opening does not reconstruct the ordinary-write root"
                .to_owned(),
        );
    }
    Ok((opening, archive.ordinary_root))
}

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
        // Select the entire reserved family so malformed key lengths fail validation below.
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
#[path = "receiver_snapshot/fastpq_source_archive_tests.rs"]
mod fastpq_source_archive_tests;

#[cfg(test)]
#[path = "receiver_snapshot/fastpq_source_archive_bridge_tests.rs"]
mod fastpq_source_archive_bridge_tests;

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

    fn sample_receipt(operation_id: [u8; 32]) -> KagemushaReserveReceiptV1 {
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            iroha_data_model::block::BlockHeader,
        >::from_untyped_unchecked(
            Hash::new(b"kagemusha-receipt-proof"),
        ));
        let asset = iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        );
        let asset_incarnation = iroha_data_model::nexus::AxtAssetIncarnationV1::try_from_bytes(
            iroha_crypto::Hash::new(b"kagemusha-receipt-proof-incarnation").into(),
        )
        .expect("asset incarnation");
        KagemushaReserveReceiptV1 {
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
        }
    }

    #[test]
    fn kagemusha_receipt_proof_is_exact_and_duplicate_safe() {
        let operation_id = [0x61; 32];
        let receipt = sample_receipt(operation_id);
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
                ExecKv {
                    key: PARLIAMENT_TIMED_OVN_CASTING_WITNESS_KEY_V1.to_vec(),
                    value: b"separate d5-prefixed synthetic namespace".to_vec(),
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

    #[test]
    fn captured_block_synthetic_write_families_share_one_authenticated_root() {
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
            sumeragi::witness as recorder,
        };
        use iroha_data_model::{asset::AssetId, block::BlockHeader};
        use iroha_primitives::numeric::Quantity;

        let _guard = recorder::exec_witness_guard();
        let receipts = [sample_receipt([0x61; 32]), sample_receipt([0x62; 32])];
        for include_receipts in [false, true] {
            let state = State::new(
                World::default(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let header = BlockHeader::new(
                std::num::NonZeroU64::new(1).expect("height"),
                None,
                None,
                None,
                0,
                0,
            );
            let mut state_block = state.block(header);
            recorder::start_block();
            recorder::record_write_asset(
                &AssetId::new(
                    receipts[0].asset.clone(),
                    iroha_test_samples::ALICE_ID.clone(),
                ),
                &Quantity::from(42_u32),
            );
            if include_receipts {
                // Reverse insertion order also exercises the canonical operation-id order.
                for receipt in receipts.iter().rev() {
                    recorder::record_write_kagemusha_reserve_receipt_v1(receipt)
                        .expect("record canonical receipt");
                }
            }
            // Direct fixture execution has no external or time entrypoint wires.
            let tx_set_hash: [u8; 32] =
                iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(std::iter::empty::<
                    &iroha_data_model::transaction::TransactionEntrypoint,
                >())
                .unwrap()
                .into();
            state_block.set_fastpq_tx_set_hash(tx_set_hash);
            state_block
                .finalize_fastpq_source_inventory(&[], &[], &[])
                .unwrap();
            assert_eq!(
                state_block
                    .fastpq_source_inventory()
                    .unwrap()
                    .unwrap()
                    .tx_set_hash(),
                tx_set_hash
            );
            state_block.capture_exec_witness().unwrap();
            let witness = state_block
                .take_exec_witness()
                .expect("actual captured witness");
            let encoded = norito::encode_canonical(&witness).expect("encode witness");
            let decoded: ExecWitness = norito::decode_canonical(&encoded).expect("decode witness");
            assert_eq!(decoded, witness);
            assert_eq!(witness.writes.len(), if include_receipts { 5 } else { 3 });
            let (fee, fee_root) =
                validation_fee_policy_witness_proof_v1(&decoded).expect("actual fee proof");
            let (casting, casting_root) = parliament_timed_ovn_casting_witness_proof_v1(&decoded)
                .expect("actual casting proof");
            let (proofs, receipt_root) = kagemusha_reserve_receipt_witnesses_v1(&decoded)
                .expect("casting writes must not be decoded as receipts");
            assert_eq!(fee_root, casting_root);
            assert_eq!(fee_root, receipt_root);
            assert!(fee.verify(fee_root));
            assert!(casting.verify(fee_root));
            assert_eq!(proofs.len(), if include_receipts { 2 } else { 0 });
            for (proof, receipt) in proofs.iter().zip(&receipts) {
                assert_eq!(&proof.receipt, receipt);
                assert!(proof.verify(fee_root));
                let encoded = norito::encode_canonical(proof).expect("encode receipt proof");
                let decoded: KagemushaReserveReceiptWitnessV1 =
                    norito::decode_canonical(&encoded).expect("decode receipt proof");
                assert_eq!(&decoded, proof);
                let mut wrong_namespace = decoded;
                wrong_namespace.key[0] = 0xD5;
                assert!(!wrong_namespace.verify(fee_root));
            }
        }
    }

    #[test]
    fn malformed_receipt_family_writes_are_never_filtered_as_absence() {
        let receipt = sample_receipt([0x61; 32]);
        let key = KagemushaReserveReceiptWitnessV1::expected_key(receipt.operation_id);
        let value = norito::encode_canonical(&receipt).expect("canonical receipt");
        let mut wrong_operation = key.clone();
        wrong_operation[1] ^= 1;
        for write in [
            ExecKv {
                key: vec![KAGEMUSHA_RESERVE_RECEIPT_WITNESS_KEY_TAG_V1],
                value: value.clone(),
            },
            ExecKv {
                key: [key.as_slice(), &[0]].concat(),
                value: value.clone(),
            },
            ExecKv {
                key: wrong_operation,
                value,
            },
            ExecKv {
                key,
                value: b"malformed receipt".to_vec(),
            },
        ] {
            let witness = ExecWitness {
                writes: vec![write],
                ..ExecWitness::default()
            };
            assert!(kagemusha_reserve_receipt_witnesses_v1(&witness).is_err());
        }
    }
}
