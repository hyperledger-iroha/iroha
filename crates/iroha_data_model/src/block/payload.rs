use super::{SignedBlock, execution_context::BlockExecutionContextBundle, header::BlockHeader};
use crate::{
    consensus::{NposConsensusEffects, PreviousRosterEvidence},
    da::{
        commitment::{DaCommitmentBundle, DaProofPolicyBundle},
        pin_intent::DaPinIntentBundle,
    },
    events::{data::prelude::AssetBatchTransferOutcome, trigger_completed::TriggerCompletedEvent},
    fastpq::TransferTranscript,
    transaction::{
        error::TransactionRejectionReason,
        signed::{SignedTransaction, TransactionEntrypoint, TransactionResult},
    },
    trigger::{DataTriggerSequence, TimeTriggerEntrypoint},
};
use iroha_crypto::{Hash, HashOf, MerkleError, MerkleProof, MerkleTree, MerkleTreeCommitment};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{cmp::Ordering, collections::BTreeMap, fmt, vec::Vec};
#[model]
mod model {
    use super::*;
    use crate::{
        consensus::{NposConsensusEffects, PreviousRosterEvidence},
        da::commitment::DaCommitmentBundle,
    };
    /// Core contents of a block.
    #[derive(Debug, Clone, Encode, IntoSchema, Decode)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[allow(clippy::redundant_pub_crate)]
    pub(crate) struct BlockPayload {
        /// Essential metadata for a block in the chain.
        pub header: BlockHeader,
        /// Legacy in-memory cache of signed external transactions.
        ///
        /// New V1 wire stores external transaction entrypoints once in
        /// [`Self::external_entrypoints`]; this vector is kept for constructors
        /// and older in-process call sites only.
        #[norito(skip)]
        pub transactions: Vec<SignedTransaction>,
        /// External transaction entrypoints in consensus order.
        ///
        /// Older blocks omit this field and reconstruct the order from the legacy
        /// signed-transaction payload vector.
        #[norito(default)]
        #[norito(skip_serializing_if = "Vec::is_empty")]
        pub external_entrypoints: Vec<TransactionEntrypoint>,
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
        /// Deterministic `NPoS` effects embedded in this block.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub npos_consensus_effects: Option<NposConsensusEffects>,
        /// Durable execution context for external entrypoints.
        ///
        /// New committed blocks include this context so replay does not need to
        /// re-derive route-dependent execution inputs from the current WSV.
        #[norito(default)]
        #[norito(skip_serializing_if = "Option::is_none")]
        pub execution_context: Option<BlockExecutionContextBundle>,
    }
    /// Secondary block state resulting from execution.
    #[derive(Debug, Clone, Default, Decode, Encode, IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct BlockResult {
        /// Legacy in-memory copy of external transaction entrypoints.
        ///
        /// New V1 wire stores these on [`BlockPayload`] only.
        #[norito(skip)]
        pub external_entrypoints: Vec<TransactionEntrypoint>,
        /// Time-triggered entrypoints, forming the second half of the transaction entrypoints.
        pub time_triggers: Vec<TimeTriggerEntrypoint>,
        /// Merkle tree over the transaction entrypoints (external transactions followed by time triggers).
        pub merkle: MerkleTree<TransactionEntrypoint>,
        /// Merkle tree over the transaction results, with indices aligned to the entrypoint Merkle tree.
        pub result_merkle: MerkleTree<TransactionResult>,
        /// Transaction execution results, with indices aligned to the entrypoint Merkle tree.
        pub transaction_results: Vec<TransactionResult>,
        /// Number of successful execution fragments committed while executing this block.
        ///
        /// This includes external transactions, time triggers, and deterministic internal
        /// fragments folded into a block execution result.
        #[norito(default)]
        pub committed_fragment_count: u64,
        /// FASTPQ transfer transcripts grouped by transaction entrypoint hash.
        pub fastpq_transcripts: BTreeMap<Hash, Vec<TransferTranscript>>,
        /// Completed AXT envelopes recorded while executing the block.
        #[norito(default)]
        pub axt_envelopes: Vec<crate::nexus::AxtEnvelopeRecord>,
        /// Trigger completion events recorded while executing the block.
        #[norito(default)]
        pub trigger_completions: Vec<TriggerCompletedEvent>,
        /// Canonical AXT policy snapshot used while executing the block.
        pub axt_policy_snapshot: crate::nexus::AxtPolicySnapshot,
        /// Canonically ordered post-execution lane effects authenticated by the global CommitQC.
        ///
        /// This required V1 field stays last so its absence cannot alias a defaulted extension.
        pub lane_finality_statements: Vec<crate::nexus::LaneFinalityStatement>,
    }
}
pub use self::model::{BlockPayload, BlockResult};
impl BlockPayload {
    /// Hydrate the legacy signed-transaction cache from explicit external entrypoints.
    ///
    /// New block wire stores external transaction entrypoints in `external_entrypoints`; the
    /// `transactions` vector is skipped by Norito and exists only for legacy in-process callers.
    /// Call this after decoding payload bytes only when that legacy cache is explicitly needed.
    pub fn hydrate_legacy_transaction_cache_from_entrypoints(&mut self) -> usize {
        if !self.transactions.is_empty() || self.external_entrypoints.is_empty() {
            return self.transactions.len();
        }
        self.transactions = self
            .external_entrypoints
            .iter()
            .filter_map(|entrypoint| match entrypoint {
                TransactionEntrypoint::External(tx) => Some(tx.clone()),
                TransactionEntrypoint::SealedReveal(reveal) => {
                    Some(reveal.signed_transaction().clone())
                }
                TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => None,
            })
            .collect();
        self.transactions.len()
    }
}
impl PartialEq for BlockPayload {
    fn eq(&self, other: &Self) -> bool {
        self.header == other.header
            && self.external_entrypoints == other.external_entrypoints
            && self.execution_context == other.execution_context
            && self.da_commitments == other.da_commitments
            && self.da_proof_policies == other.da_proof_policies
            && self.da_pin_intents == other.da_pin_intents
            && self.previous_roster_evidence == other.previous_roster_evidence
            && self.npos_consensus_effects == other.npos_consensus_effects
    }
}
impl Eq for BlockPayload {}
impl PartialOrd for BlockPayload {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for BlockPayload {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_npos_effects_hash = self.npos_consensus_effects.as_ref().map(HashOf::new);
        let other_npos_effects_hash = other.npos_consensus_effects.as_ref().map(HashOf::new);
        let self_execution_context_hash = self.execution_context.as_ref().map(HashOf::new);
        let other_execution_context_hash = other.execution_context.as_ref().map(HashOf::new);
        (
            &self.header,
            &self.external_entrypoints,
            &self_execution_context_hash,
            &self.da_commitments,
            &self.da_proof_policies,
            &self.da_pin_intents,
            &self.previous_roster_evidence,
            &self_npos_effects_hash,
        )
            .cmp(&(
                &other.header,
                &other.external_entrypoints,
                &other_execution_context_hash,
                &other.da_commitments,
                &other.da_proof_policies,
                &other.da_pin_intents,
                &other.previous_roster_evidence,
                &other_npos_effects_hash,
            ))
    }
}
impl PartialEq for BlockResult {
    fn eq(&self, other: &Self) -> bool {
        self.time_triggers == other.time_triggers
            && self.merkle == other.merkle
            && self.result_merkle == other.result_merkle
            && self.transaction_results == other.transaction_results
            && self.committed_fragment_count == other.committed_fragment_count
            && self.fastpq_transcripts == other.fastpq_transcripts
            && self.axt_envelopes == other.axt_envelopes
            && self.lane_finality_statements == other.lane_finality_statements
            && self.trigger_completions == other.trigger_completions
            && self.axt_policy_snapshot == other.axt_policy_snapshot
    }
}
impl Eq for BlockResult {}
impl PartialOrd for BlockResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for BlockResult {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            &self.time_triggers,
            &self.merkle,
            &self.result_merkle,
            &self.transaction_results,
            &self.committed_fragment_count,
            &self.fastpq_transcripts,
            &self.axt_envelopes,
            &self.lane_finality_statements,
            &self.trigger_completions,
            &self.axt_policy_snapshot,
        )
            .cmp(&(
                &other.time_triggers,
                &other.merkle,
                &other.result_merkle,
                &other.transaction_results,
                &other.committed_fragment_count,
                &other.fastpq_transcripts,
                &other.axt_envelopes,
                &other.lane_finality_statements,
                &other.trigger_completions,
                &other.axt_policy_snapshot,
            ))
    }
}
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
    /// Borrow external entrypoints in execution order when the block stores them explicitly.
    ///
    /// Older in-memory blocks may only carry the legacy signed-transaction vector; callers should
    /// fall back to [`Self::external_transactions`] in that case.
    #[inline]
    pub fn external_entrypoints_slice(&self) -> Option<&[TransactionEntrypoint]> {
        if self.payload.external_entrypoints.is_empty() {
            self.result.as_ref().and_then(|result| {
                (!result.external_entrypoints.is_empty())
                    .then_some(result.external_entrypoints.as_slice())
            })
        } else {
            Some(self.payload.external_entrypoints.as_slice())
        }
    }
    /// Number of external entrypoints (signed or authority-free) recorded in the block.
    #[inline]
    pub fn external_entrypoint_count(&self) -> usize {
        self.external_entrypoints_slice().map_or(
            self.payload.transactions.len(),
            <[TransactionEntrypoint]>::len,
        )
    }
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
    /// External entrypoints in execution order.
    #[inline]
    pub fn external_entrypoints_cloned(
        &self,
    ) -> impl ExactSizeIterator<Item = TransactionEntrypoint> + DoubleEndedIterator + '_ {
        ExternalEntrypointIterator::new(self)
    }
    /// Borrow one signed external transaction and return its canonical entrypoint hash.
    ///
    /// Authority-free commitments are not signed transactions and return `None`. The lookup is
    /// constant-space and does not clone either the transaction or the complete entrypoint list.
    #[inline]
    pub fn external_signed_transaction_at(
        &self,
        index: usize,
    ) -> Option<(HashOf<TransactionEntrypoint>, &SignedTransaction)> {
        self.external_entrypoints_slice().map_or_else(
            || {
                let transaction = self.payload.transactions.get(index)?;
                Some((transaction.hash_as_entrypoint(), transaction))
            },
            |entries| {
                let entrypoint = entries.get(index)?;
                let hash = entrypoint.hash();
                let transaction = match entrypoint {
                    TransactionEntrypoint::External(transaction) => transaction,
                    TransactionEntrypoint::SealedReveal(reveal) => reveal.signed_transaction(),
                    TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => {
                        return None;
                    }
                };
                Some((hash, transaction))
            },
        )
    }
    /// Borrow one signed external transaction by canonical entrypoint index.
    ///
    /// Unlike [`Self::external_signed_transaction_at`], this avoids hashing the entrypoint and is
    /// useful to continue streaming an entrypoint after its hash has already been retained.
    #[inline]
    pub fn external_signed_transaction_ref_at(&self, index: usize) -> Option<&SignedTransaction> {
        self.external_entrypoints_slice().map_or_else(
            || self.payload.transactions.get(index),
            |entries| match entries.get(index)? {
                TransactionEntrypoint::External(transaction) => Some(transaction),
                TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
                TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => None,
            },
        )
    }
    /// Block transactions, the underlying vector
    #[inline]
    pub fn transactions_vec(&self) -> &Vec<SignedTransaction> {
        &self.payload.transactions
    }
    /// Durable execution context embedded in this block, if any.
    #[inline]
    pub fn execution_context(&self) -> Option<&BlockExecutionContextBundle> {
        self.payload.execution_context.as_ref()
    }
    /// Set or clear durable execution context and update the header hash accordingly.
    pub fn set_execution_context(&mut self, context: Option<BlockExecutionContextBundle>) {
        let context = context.filter(|bundle| !bundle.is_empty());
        let hash = context.as_ref().map(HashOf::new);
        self.payload.execution_context = context;
        self.payload.header.set_execution_context_hash(hash);
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
        let commitments = commitments.filter(|bundle| !bundle.is_empty());
        let hash = commitments
            .as_ref()
            .and_then(DaCommitmentBundle::merkle_commitment);
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
        let intents = intents.filter(|bundle| !bundle.is_empty());
        let hash = intents
            .as_ref()
            .and_then(DaPinIntentBundle::merkle_commitment);
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
    /// Deterministic `NPoS` effects embedded in this block.
    #[inline]
    pub fn npos_consensus_effects(&self) -> Option<&NposConsensusEffects> {
        self.payload.npos_consensus_effects.as_ref()
    }
    /// Set or clear deterministic `NPoS` effects and update the header hash accordingly.
    pub fn set_npos_consensus_effects(&mut self, effects: Option<NposConsensusEffects>) {
        let effects = effects.filter(|bundle| !bundle.is_empty());
        let hash = effects.as_ref().map(HashOf::new);
        self.payload.npos_consensus_effects = effects;
        self.payload.header.set_npos_effects_hash(hash);
    }
    /// Set or clear the SCCP commitment root finalized in this block.
    pub fn set_sccp_commitment_root(&mut self, root: Option<[u8; 32]>) {
        self.payload.header.set_sccp_commitment_root(root);
    }
    /// Replace the ordered external entrypoints and update Merkle material accordingly.
    pub fn set_external_entrypoints(&mut self, entrypoints: Vec<TransactionEntrypoint>) {
        let merkle = entrypoints
            .iter()
            .map(TransactionEntrypoint::hash)
            .collect::<MerkleTree<TransactionEntrypoint>>();
        self.payload.header.merkle_root = merkle.root();
        if let Some(result) = self.result.as_mut() {
            result.external_entrypoints.clear();
            result.merkle = entrypoints
                .iter()
                .map(TransactionEntrypoint::hash)
                .chain(
                    result
                        .time_triggers
                        .iter()
                        .cloned()
                        .map(TransactionEntrypoint::from)
                        .map(|entrypoint| entrypoint.hash()),
                )
                .collect();
        }
        self.payload.external_entrypoints = entrypoints;
    }
    /// Check whether the block has entrypoints or deterministic artifacts.
    #[inline]
    pub fn is_empty(&self) -> bool {
        if self.external_entrypoint_count() != 0 {
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
        if self
            .payload
            .npos_consensus_effects
            .as_ref()
            .is_some_and(|bundle| !bundle.is_empty())
        {
            return false;
        }
        if self
            .payload
            .execution_context
            .as_ref()
            .is_some_and(|context| !context.is_empty())
        {
            return false;
        }
        if self.payload.header.sccp_commitment_root().is_some() {
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
        self.entrypoints_cloned()
            .map(|entrypoint| entrypoint.hash())
    }
    /// Merkle root over external transactions followed by time-triggered entrypoints.
    #[inline]
    pub fn full_entry_merkle_root(&self) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
        self.result.as_ref().and_then(|result| result.merkle.root())
    }
    /// Root and exact leaf count over external and time-triggered entrypoints.
    #[inline]
    pub fn full_entry_merkle_commitment(
        &self,
    ) -> Option<MerkleTreeCommitment<TransactionEntrypoint>> {
        self.result
            .as_ref()
            .and_then(|result| result.merkle.commitment())
    }
    /// Validate the retained entrypoint Merkle cache against this block's entries.
    ///
    /// The validation walks the retained tree in place and does not rebuild an
    /// entry-sized node vector.
    pub fn validate_entrypoint_merkle_cache(&self) -> Result<(), MerkleError> {
        let result = self.result.as_ref().ok_or_else(|| {
            MerkleError::InvalidLayout("block transaction results are missing".to_owned())
        })?;
        result.merkle.validate_leaves(self.entrypoint_hashes())
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
            .leaf_count()
            .try_into()
            .expect("bug: leaf count exceeded u32::MAX");
        (0..n_leaves).map(|i| {
            self.result_ref()
                .merkle
                .get_proof(i)
                .expect("bug: missing Merkle proof at valid index")
        })
    }
    /// Return the retained Merkle proof for one canonical entrypoint index.
    #[inline]
    pub fn entrypoint_proof(&self, index: u32) -> Option<MerkleProof<TransactionEntrypoint>> {
        self.result.as_ref()?.merkle.get_proof(index)
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
        self.result_ref()
            .transaction_results
            .iter()
            .map(TransactionResult::hash)
    }
    /// Root and exact leaf count over transaction execution results.
    #[inline]
    pub fn result_merkle_commitment(&self) -> Option<MerkleTreeCommitment<TransactionResult>> {
        self.result
            .as_ref()
            .and_then(|result| result.result_merkle.commitment())
    }
    /// Validate the retained result Merkle cache against this block's results.
    ///
    /// The validation walks the retained tree in place and does not rebuild a
    /// result-sized node vector.
    pub fn validate_result_merkle_cache(&self) -> Result<(), MerkleError> {
        let result = self.result.as_ref().ok_or_else(|| {
            MerkleError::InvalidLayout("block transaction results are missing".to_owned())
        })?;
        result.result_merkle.validate_leaves(self.result_hashes())
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
            .leaf_count()
            .try_into()
            .expect("bug: leaf count exceeded u32::MAX");
        (0..n_leaves).map(|i| {
            self.result_ref()
                .result_merkle
                .get_proof(i)
                .expect("bug: missing Merkle proof at valid index")
        })
    }
    /// Return the retained Merkle proof for one canonical result index.
    #[inline]
    pub fn result_proof(&self, index: u32) -> Option<MerkleProof<TransactionResult>> {
        self.result.as_ref()?.result_merkle.get_proof(index)
    }
    /// Actual transaction results (trigger sequence or rejection reason) in execution order.
    /// Indices align with those of the entrypoints.
    #[inline]
    pub fn results(
        &self,
    ) -> impl ExactSizeIterator<Item = &TransactionResult> + DoubleEndedIterator {
        self.result_ref().transaction_results.iter()
    }
    /// Transaction entrypoints paired with their execution results.
    ///
    /// The returned index is the canonical entrypoint/result index in the block.
    #[inline]
    pub fn entrypoint_results(
        &self,
    ) -> impl Iterator<Item = (usize, TransactionEntrypoint, &TransactionResult)> + '_ {
        self.entrypoints_cloned()
            .zip(self.results())
            .enumerate()
            .map(|(index, (entrypoint, result))| (index, entrypoint, result))
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
    /// Trigger completion events recorded while executing the block.
    #[inline]
    pub fn trigger_completions(&self) -> Option<&[TriggerCompletedEvent]> {
        self.result
            .as_ref()
            .map(|result| result.trigger_completions.as_slice())
    }
    /// Durable independent-batch outcomes for one transaction entrypoint.
    #[inline]
    pub fn batch_transfer_outcomes_for(
        &self,
        entrypoint_hash: &HashOf<TransactionEntrypoint>,
    ) -> &[AssetBatchTransferOutcome] {
        self.entrypoint_hashes()
            .zip(self.results())
            .find_map(|(hash, result)| {
                (hash == *entrypoint_hash).then(|| result.batch_transfer_outcomes())
            })
            .unwrap_or(&[])
    }
    /// AXT policy snapshot captured during execution, when results are present.
    #[inline]
    pub fn axt_policy_snapshot(&self) -> Option<&crate::nexus::AxtPolicySnapshot> {
        self.result
            .as_ref()
            .map(|result| &result.axt_policy_snapshot)
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
#[derive(Clone, Copy)]
enum ExternalTransactionSource<'a> {
    Legacy(&'a [SignedTransaction]),
    Entrypoints(&'a [TransactionEntrypoint]),
}
struct ExternalTransactionIterator<'a> {
    source: ExternalTransactionSource<'a>,
    front: usize,
    back: usize,
    remaining: usize,
}
impl<'a> ExternalTransactionIterator<'a> {
    fn new(block: &'a SignedBlock) -> Self {
        let (source, len, remaining) = block.external_entrypoints_slice().map_or_else(
            || {
                let transactions = block.payload.transactions.as_slice();
                (
                    ExternalTransactionSource::Legacy(transactions),
                    transactions.len(),
                    transactions.len(),
                )
            },
            |entries| {
                let remaining = entries
                    .iter()
                    .filter(|entry| {
                        matches!(
                            entry,
                            TransactionEntrypoint::External(_)
                                | TransactionEntrypoint::SealedReveal(_)
                        )
                    })
                    .count();
                (
                    ExternalTransactionSource::Entrypoints(entries),
                    entries.len(),
                    remaining,
                )
            },
        );
        Self {
            source,
            front: 0,
            back: len,
            remaining,
        }
    }
    fn transaction_at(&self, index: usize) -> Option<&'a SignedTransaction> {
        match self.source {
            ExternalTransactionSource::Legacy(transactions) => transactions.get(index),
            ExternalTransactionSource::Entrypoints(entries) => match entries.get(index)? {
                TransactionEntrypoint::External(transaction) => Some(transaction),
                TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
                TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => None,
            },
        }
    }
}
impl<'a> Iterator for ExternalTransactionIterator<'a> {
    type Item = &'a SignedTransaction;
    fn next(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            let idx = self.front;
            self.front += 1;
            if let Some(transaction) = self.transaction_at(idx) {
                self.remaining -= 1;
                return Some(transaction);
            }
        }
        None
    }
}
impl DoubleEndedIterator for ExternalTransactionIterator<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            self.back -= 1;
            if let Some(transaction) = self.transaction_at(self.back) {
                self.remaining -= 1;
                return Some(transaction);
            }
        }
        None
    }
}
impl ExactSizeIterator for ExternalTransactionIterator<'_> {
    fn len(&self) -> usize {
        self.remaining
    }
}
#[derive(Clone, Copy)]
enum ExternalEntrypointSource<'a> {
    Legacy(&'a [SignedTransaction]),
    Entrypoints(&'a [TransactionEntrypoint]),
}
struct ExternalEntrypointIterator<'a> {
    source: ExternalEntrypointSource<'a>,
    front: usize,
    back: usize,
}
impl<'a> ExternalEntrypointIterator<'a> {
    fn new(block: &'a SignedBlock) -> Self {
        let (source, len) = block.external_entrypoints_slice().map_or_else(
            || {
                let transactions = block.payload.transactions.as_slice();
                (
                    ExternalEntrypointSource::Legacy(transactions),
                    transactions.len(),
                )
            },
            |entries| {
                (
                    ExternalEntrypointSource::Entrypoints(entries),
                    entries.len(),
                )
            },
        );
        Self {
            source,
            front: 0,
            back: len,
        }
    }
    fn entrypoint_at(&self, index: usize) -> Option<TransactionEntrypoint> {
        match self.source {
            ExternalEntrypointSource::Legacy(transactions) => transactions
                .get(index)
                .cloned()
                .map(TransactionEntrypoint::from),
            ExternalEntrypointSource::Entrypoints(entries) => entries.get(index).cloned(),
        }
    }
}
impl Iterator for ExternalEntrypointIterator<'_> {
    type Item = TransactionEntrypoint;
    fn next(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        let idx = self.front;
        self.front += 1;
        self.entrypoint_at(idx)
    }
}
impl DoubleEndedIterator for ExternalEntrypointIterator<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        self.back -= 1;
        self.entrypoint_at(self.back)
    }
}
impl ExactSizeIterator for ExternalEntrypointIterator<'_> {
    fn len(&self) -> usize {
        self.back.saturating_sub(self.front)
    }
}
struct EntrypointIterator<'a> {
    external: ExternalEntrypointIterator<'a>,
    time_triggers: Option<std::slice::Iter<'a, TimeTriggerEntrypoint>>,
}
impl Iterator for EntrypointIterator<'_> {
    type Item = TransactionEntrypoint;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entrypoint) = self.external.next() {
            return Some(entrypoint);
        }
        self.time_triggers
            .as_mut()?
            .next()
            .cloned()
            .map(TransactionEntrypoint::from)
    }
}
impl DoubleEndedIterator for EntrypointIterator<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if let Some(entrypoint) = self
            .time_triggers
            .as_mut()
            .and_then(|time_triggers| time_triggers.next_back())
        {
            return Some(TransactionEntrypoint::from(entrypoint.clone()));
        }
        self.external.next_back()
    }
}
impl ExactSizeIterator for EntrypointIterator<'_> {
    fn len(&self) -> usize {
        self.external.len()
            + self
                .time_triggers
                .as_ref()
                .map_or(0, ExactSizeIterator::len)
    }
}
impl<'a> EntrypointIterator<'a> {
    fn new(block: &'a SignedBlock) -> Self {
        Self {
            external: ExternalEntrypointIterator::new(block),
            time_triggers: block
                .result
                .as_ref()
                .map(|result| result.time_triggers.iter()),
        }
    }
}
