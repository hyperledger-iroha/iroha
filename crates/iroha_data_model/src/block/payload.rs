use std::{collections::BTreeMap, fmt, vec::Vec};

use iroha_crypto::{Hash, HashOf, MerkleProof, MerkleTree};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{SignedBlock, header::BlockHeader};
use crate::{
    consensus::PreviousRosterEvidence,
    da::{
        commitment::{DaCommitmentBundle, DaProofPolicyBundle},
        pin_intent::DaPinIntentBundle,
    },
    fastpq::TransferTranscript,
    transaction::{
        error::TransactionRejectionReason,
        signed::{SignedTransaction, TransactionEntrypoint, TransactionResult},
    },
    trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
};

#[model]
mod model {
    use super::*;
    use crate::{consensus::PreviousRosterEvidence, da::commitment::DaCommitmentBundle};

    /// Core contents of a block.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, IntoSchema, Decode)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) struct BlockPayload {
        /// Essential metadata for a block in the chain.
        pub header: BlockHeader,
        /// External transactions as source of the state, forming the first half of the transaction entrypoints.
        pub transactions: Vec<SignedTransaction>,
        /// Optional DA commitment bundle embedded in this block.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub da_commitments: Option<DaCommitmentBundle>,
        /// Optional DA proof policy bundle embedded in this block.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub da_proof_policies: Option<DaProofPolicyBundle>,
        /// Optional DA pin intent bundle embedded in this block.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub da_pin_intents: Option<DaPinIntentBundle>,
        /// Optional previous-height roster evidence embedded in this block.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub previous_roster_evidence: Option<PreviousRosterEvidence>,
    }

    /// Secondary block state resulting from execution.
    #[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct BlockResult {
        /// Time-triggered entrypoints, forming the second half of the transaction entrypoints.
        pub time_triggers: Vec<TimeTriggerEntrypoint>,
        /// Merkle tree over the transaction entrypoints (external transactions followed by time triggers).
        pub merkle: MerkleTree<TransactionEntrypoint>,
        /// Merkle tree over the transaction results, with indices aligned to the entrypoint Merkle tree.
        pub result_merkle: MerkleTree<TransactionResult>,
        /// Transaction execution results, with indices aligned to the entrypoint Merkle tree.
        pub transaction_results: Vec<TransactionResult>,
        /// FASTPQ transfer transcripts grouped by transaction entrypoint hash.
        pub fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
        /// Completed AXT envelopes recorded while executing the block.
        #[norito(default)]
        pub axt_envelopes: Vec<crate::nexus::AxtEnvelopeRecord>,
        /// Optional AXT policy snapshot used while executing the block.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub axt_policy_snapshot: Option<crate::nexus::AxtPolicySnapshot>,
    }
}

pub use self::model::{BlockPayload, BlockResult};

impl fmt::Display for BlockPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({})", self.header)
    }
}

impl fmt::Display for BlockResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BlockResult")
    }
}

impl SignedBlock {
    /// Return error for the transaction index
    pub fn error(&self, tx: usize) -> Option<&TransactionRejectionReason> {
        self.result
            .as_ref()
            .and_then(|result| result.transaction_results.get(tx))
            .and_then(|result| result.as_ref().err())
    }

    /// Block payload. Used for tests
    #[cfg(feature = "transparent_api")]
    pub fn payload(&self) -> &BlockPayload {
        &self.payload
    }

    /// Signed transactions originating from external sources.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn external_transactions(
        &self,
    ) -> impl ExactSizeIterator<Item = &SignedTransaction> + DoubleEndedIterator {
        ExternalTransactionIterator::new(self)
    }

    /// Block transactions, the underlying vector
    #[inline]
    pub fn transactions_vec(&self) -> &Vec<SignedTransaction> {
        &self.payload.transactions
    }

    /// Optional DA commitment bundle embedded in this block.
    #[inline]
    pub fn da_commitments(&self) -> Option<&DaCommitmentBundle> {
        self.payload.da_commitments.as_ref()
    }

    /// Optional DA proof policy bundle embedded in this block.
    #[inline]
    pub fn da_proof_policies(&self) -> Option<&DaProofPolicyBundle> {
        self.payload.da_proof_policies.as_ref()
    }

    /// Set or clear the DA commitment bundle and update the header hash accordingly.
    pub fn set_da_commitments(&mut self, commitments: Option<DaCommitmentBundle>) {
        let hash = commitments.as_ref().and_then(|bundle| {
            if bundle.is_empty() {
                None
            } else {
                Some(bundle.canonical_hash())
            }
        });
        self.payload.da_commitments = commitments;
        self.payload.header.set_da_commitments_hash(hash);
    }

    /// Set or clear the DA proof policy bundle and update the header hash accordingly.
    pub fn set_da_proof_policies(&mut self, policies: Option<DaProofPolicyBundle>) {
        let hash = policies.as_ref().map(HashOf::new);
        self.payload.da_proof_policies = policies;
        self.payload.header.set_da_proof_policies_hash(hash);
    }

    /// Optional DA pin intent bundle embedded in this block.
    #[inline]
    pub fn da_pin_intents(&self) -> Option<&DaPinIntentBundle> {
        self.payload.da_pin_intents.as_ref()
    }

    /// Set or clear the DA pin intent bundle and update the header hash accordingly.
    pub fn set_da_pin_intents(&mut self, intents: Option<DaPinIntentBundle>) {
        let hash = intents
            .as_ref()
            .and_then(|bundle| bundle.merkle_root().map(HashOf::from_untyped_unchecked));
        self.payload.da_pin_intents = intents;
        self.payload.header.set_da_pin_intents_hash(hash);
    }

    /// Optional previous-height roster evidence embedded in this block.
    #[inline]
    pub fn previous_roster_evidence(&self) -> Option<&PreviousRosterEvidence> {
        self.payload.previous_roster_evidence.as_ref()
    }

    /// Set or clear previous-height roster evidence and update the header hash accordingly.
    pub fn set_previous_roster_evidence(&mut self, evidence: Option<PreviousRosterEvidence>) {
        let hash = evidence.as_ref().map(HashOf::new);
        self.payload.previous_roster_evidence = evidence;
        self.payload.header.set_prev_roster_evidence_hash(hash);
    }

    /// Check whether the block has entrypoints or deterministic artifacts.
    #[inline]
    pub fn is_empty(&self) -> bool {
        if !self.payload.transactions.is_empty() {
            return false;
        }
        if self
            .result
            .as_ref()
            .is_some_and(|result| !result.time_triggers.is_empty())
        {
            return false;
        }
        if self
            .payload
            .da_commitments
            .as_ref()
            .is_some_and(|bundle| !bundle.is_empty())
        {
            return false;
        }
        if self
            .payload
            .da_pin_intents
            .as_ref()
            .is_some_and(|bundle| !bundle.is_empty())
        {
            return false;
        }
        if self.payload.previous_roster_evidence.is_some() {
            return false;
        }
        true
    }

    /// Time-triggered entrypoints in execution order, following external transactions.
    /// Indices offset by the number of the external transactions align with those of the entrypoints.
    #[inline]
    pub fn time_triggers(
        &self,
    ) -> impl ExactSizeIterator<Item = &TimeTriggerEntrypoint> + DoubleEndedIterator {
        self.result_ref().time_triggers.iter()
    }

    /// Hashes of each transaction entrypoint (external and time-triggered) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn entrypoint_hashes(
        &self,
    ) -> impl ExactSizeIterator<Item = HashOf<TransactionEntrypoint>> + DoubleEndedIterator + '_
    {
        self.result_ref().merkle.leaves()
    }

    /// Merkle root over external transactions followed by time-triggered entrypoints.
    #[inline]
    pub fn full_entry_merkle_root(&self) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
        self.result.as_ref().and_then(|result| result.merkle.root())
    }

    /// Merkle proofs for each transaction entrypoint (external and time-triggered) in execution order.
    /// Indices align with those of the entrypoints.
    pub fn entrypoint_proofs(
        &self,
    ) -> impl ExactSizeIterator<Item = MerkleProof<TransactionEntrypoint>> + DoubleEndedIterator + '_
    {
        let n_leaves: u32 = self
            .result_ref()
            .merkle
            .leaves()
            .len()
            .try_into()
            .expect("bug: leaf count exceeded u32::MAX");
        (0..n_leaves).map(|i| {
            self.result_ref()
                .merkle
                .get_proof(i)
                .expect("bug: missing Merkle proof at valid index")
        })
    }

    /// Transaction entrypoints (external and time-triggered) in execution order.
    #[inline]
    pub fn entrypoints_cloned(
        &self,
    ) -> impl ExactSizeIterator<Item = TransactionEntrypoint> + DoubleEndedIterator + '_ {
        EntrypointIterator::new(self)
    }

    /// Hashes of each transaction result (trigger sequence or rejection reason) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn result_hashes(
        &self,
    ) -> impl ExactSizeIterator<Item = HashOf<TransactionResult>> + DoubleEndedIterator + '_ {
        self.result_ref().result_merkle.leaves()
    }

    /// Merkle proofs for each transaction result in execution order.
    /// Indices align with those of the entrypoints.
    pub fn result_proofs(
        &self,
    ) -> impl ExactSizeIterator<Item = MerkleProof<TransactionResult>> + DoubleEndedIterator + '_
    {
        let n_leaves: u32 = self
            .result_ref()
            .result_merkle
            .leaves()
            .len()
            .try_into()
            .expect("bug: leaf count exceeded u32::MAX");
        (0..n_leaves).map(|i| {
            self.result_ref()
                .result_merkle
                .get_proof(i)
                .expect("bug: missing Merkle proof at valid index")
        })
    }

    /// Actual transaction results (trigger sequence or rejection reason) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn results(
        &self,
    ) -> impl ExactSizeIterator<Item = &TransactionResult> + DoubleEndedIterator {
        self.result_ref().transaction_results.iter()
    }

    /// FASTPQ transfer transcripts grouped by transaction entrypoint hash.
    #[inline]
    pub fn fastpq_transcripts(&self) -> &BTreeMap<Hash, Vec<TransferTranscript>> {
        &self.result_ref().fastpq_transcripts
    }

    /// Completed AXT envelopes recorded while executing the block.
    #[inline]
    pub fn axt_envelopes(&self) -> Option<&[crate::nexus::AxtEnvelopeRecord]> {
        self.result
            .as_ref()
            .map(|result| result.axt_envelopes.as_slice())
    }

    /// AXT policy snapshot captured during execution (if any).
    #[inline]
    pub fn axt_policy_snapshot(&self) -> Option<&crate::nexus::AxtPolicySnapshot> {
        self.result
            .as_ref()
            .and_then(|result| result.axt_policy_snapshot.as_ref())
    }

    /// Successful transaction indices and data trigger sequences.
    pub fn successes(&self) -> impl Iterator<Item = (u64, &DataTriggerSequence)> {
        self.results()
            .enumerate()
            .filter_map(|(i, result)| result.as_ref().ok().map(|ok| (i as u64, ok)))
    }

    /// Failed transaction indices and rejection reasons.
    pub fn errors(&self) -> impl Iterator<Item = (u64, &TransactionRejectionReason)> {
        self.results()
            .enumerate()
            .filter_map(|(i, result)| result.as_ref().err().map(|err| (i as u64, err)))
    }
}

struct ExternalTransactionIterator<'a> {
    block: &'a SignedBlock,
    order: Vec<usize>,
    front: usize,
    back: usize,
}

impl<'a> ExternalTransactionIterator<'a> {
    fn new(block: &'a SignedBlock) -> Self {
        let tx_count = block.payload.transactions.len();
        let order: Vec<usize> = (0..tx_count).collect();
        let len = order.len();
        Self {
            block,
            order,
            front: 0,
            back: len,
        }
    }
}

impl<'a> Iterator for ExternalTransactionIterator<'a> {
    type Item = &'a SignedTransaction;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        let idx = self.order[self.front];
        self.front += 1;
        self.block.payload.transactions.get(idx)
    }
}

impl DoubleEndedIterator for ExternalTransactionIterator<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        self.back -= 1;
        let idx = self.order[self.back];
        self.block.payload.transactions.get(idx)
    }
}

impl ExactSizeIterator for ExternalTransactionIterator<'_> {
    fn len(&self) -> usize {
        self.back.saturating_sub(self.front)
    }
}

struct EntrypointIterator {
    entries: Vec<TransactionEntrypoint>,
    front: usize,
    back: usize,
}

impl Iterator for EntrypointIterator {
    type Item = TransactionEntrypoint;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        let idx = self.front;
        self.front += 1;
        self.entries.get(idx).cloned()
    }
}

impl DoubleEndedIterator for EntrypointIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        self.back -= 1;
        self.entries.get(self.back).cloned()
    }
}

impl ExactSizeIterator for EntrypointIterator {
    fn len(&self) -> usize {
        self.back.saturating_sub(self.front)
    }
}

impl EntrypointIterator {
    fn new(block: &SignedBlock) -> Self {
        let entries: Vec<TransactionEntrypoint> = if block.has_results() {
            let mut tx_by_hash: BTreeMap<_, usize> = BTreeMap::new();
            for (idx, tx) in block.payload.transactions.iter().enumerate() {
                tx_by_hash.insert(tx.hash_as_entrypoint(), idx);
            }
            let result = block.result_ref();
            let mut trig_by_hash: BTreeMap<_, usize> = BTreeMap::new();
            for (idx, trig) in result.time_triggers.iter().enumerate() {
                trig_by_hash.insert(trig.hash_as_entrypoint(), idx);
            }
            block
                .entrypoint_hashes()
                .map(|hash| {
                    if let Some(&idx) = tx_by_hash.get(&hash) {
                        TransactionEntrypoint::from(block.payload.transactions[idx].clone())
                    } else if let Some(&idx) = trig_by_hash.get(&hash) {
                        TransactionEntrypoint::from(result.time_triggers[idx].clone())
                    } else {
                        panic!("entrypoint hash missing from block contents");
                    }
                })
                .collect()
        } else {
            block
                .payload
                .transactions
                .iter()
                .cloned()
                .map(TransactionEntrypoint::from)
                .collect()
        };
        let len = entries.len();
        Self {
            entries,
            front: 0,
            back: len,
        }
    }
}
