//! Modeling block transitions.
//!
//! Operations on blocks:
//!
//! 1. Static analysis of the block. This is a _fallible_ operation
//! 2. Execution of transactions and time triggers. This is an _infallible_ operation. If there are errors during
//!    transaction execution, they are recorded in the block.
//! 3. Voting
//! 4. Pre-commit signatures check
//! 5. Apply & commit
//!
//! Operations 1 + 2 form a process we call _validation_.
//!
//! Block lifecycle stages:
//!
//! 1. Block is created by the node ([`NewBlock`]). Such blocks are assumed to be valid and do not
//!    require static validation to transform to [`ValidBlock`].
//! 2. Block is received/deserialized from disk (as [`SignedBlock`]). Such blocks require static
//!    validation before execution to transition to [`ValidBlock`].
//! 3. Block is valid ([`ValidBlock`]). It is always created in pair with [`crate::state::StateBlock`]
//!    containing the applied state changes from the block. Transaction errors are written to the
//!    block.
//! 4. Voting block ([`VotingBlock`]). Valid block might not have sufficient signatures to be committed.
//!    Voting block is a wrappper around [`ValidBlock`] and its [`crate::state::StateBlock`] intended to
//!    collect the signatures in order to transition to [`CommittedBlock`]
//! 5. Block is committed ([`CommittedBlock`]). Created from [`ValidBlock`], ensuring the
//!    signatures meet the conditions for commit (e.g. proxy tail has signed the block).
//!
//! ### Scenario: this node creates a block
//!
//! Flow: [`BlockBuilder::new`], [`BlockBuilder::chain`], [`BlockBuilder::sign`],
//! [`NewBlock::validate_and_record_transactions`] (infallible), [`VotingBlock::new`], [`ValidBlock::commit`]
//!
//! ### Scenario: receive a created block
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::validate_keep_voting_block`], [`VotingBlock::new`],
//! [`ValidBlock::commit`]
//!
//! ### Scenario: receive a block via block sync
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::commit_keep_voting_block`]
//!
//! ### Scenario: genesis (init or receive), replay kura blocks
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::validate`], [`ValidBlock::commit`]
//!
//! ### Scenario: plain block execution
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::validate_unchecked`] (infallible),
//! [`ValidBlock::commit_unchecked`] (infallible)
use std::{collections::BTreeSet, time::Duration};

use iroha_crypto::{HashOf, KeyPair, MerkleTree};
use iroha_data_model::{
    block::*, events::prelude::*, peer::PeerId, transaction::SignedTransaction,
};
use thiserror::Error;

pub(crate) use self::event::WithEvents;
pub use self::{chained::Chained, commit::CommittedBlock, new::NewBlock, valid::ValidBlock};
use crate::{
    prelude::*,
    state::State,
    sumeragi::{network_topology::Topology, VotingBlock},
    tx::AcceptTransactionFail,
};

/// Errors occurred on block validation
#[derive(Debug, displaydoc::Display, PartialEq, Eq, Error)]
pub enum BlockValidationError {
    /// Block has committed transactions
    HasCommittedTransactions,
    /// Mismatch between the actual and expected hashes of the previous block. Expected: {expected:?}, actual: {actual:?}
    PrevBlockHashMismatch {
        /// Expected value
        expected: Option<HashOf<BlockHeader>>,
        /// Actual value
        actual: Option<HashOf<BlockHeader>>,
    },
    /// Mismatch between the actual and expected height of the previous block. Expected: {expected}, actual: {actual}
    PrevBlockHeightMismatch {
        /// Expected value
        expected: usize,
        /// Actual value
        actual: usize,
    },
    /// The merkle root does not match the computed one.
    MerkleRootMismatch,
    /// Cannot accept a transaction
    TransactionAccept(#[from] AcceptTransactionFail),
    /// Mismatch between the actual and expected topology. Expected: {expected:?}, actual: {actual:?}
    TopologyMismatch {
        /// Expected value
        expected: Vec<PeerId>,
        /// Actual value
        actual: Vec<PeerId>,
    },
    /// Error during block signatures check
    SignatureVerification(#[from] SignatureVerificationError),
    /// Received view change index is too large
    ViewChangeIndexTooLarge,
    /// Invalid genesis block: {0}
    InvalidGenesis(#[from] InvalidGenesisError),
    /// Block's creation time is earlier than that of the previous block
    BlockInThePast,
    /// Block's creation time is later than the current node local time
    BlockInTheFuture,
    /// Some transaction in the block is created after the block itself
    TransactionInTheFuture,
}

/// Error during signature verification
#[derive(Debug, displaydoc::Display, Clone, Copy, PartialEq, Eq, Error)]
pub enum SignatureVerificationError {
    /// The block doesn't have enough valid signatures to be committed (`votes_count` out of `min_votes_for_commit`)
    NotEnoughSignatures {
        /// Current number of signatures
        votes_count: usize,
        /// Minimal required number of signatures
        min_votes_for_commit: usize,
    },
    /// Block signatory doesn't correspond to any in topology
    UnknownSignatory,
    /// Block signature doesn't correspond to block payload
    UnknownSignature,
    /// The block doesn't have proxy tail signature
    ProxyTailMissing,
    /// The block doesn't have leader signature
    LeaderMissing,
    /// Miscellaneous
    Other,
}

/// Errors occurred on genesis block validation
#[derive(Debug, Copy, Clone, displaydoc::Display, PartialEq, Eq, Error)]
pub enum InvalidGenesisError {
    /// Genesis transaction must be authorized by genesis account
    UnexpectedAuthority,
    /// Genesis transactions must not contain errors
    ContainsErrors,
    /// Genesis transaction must contain instructions
    NotInstructions,
    /// Genesis block must have 1 to 5 transactions (executor upgrade, parameters, ordinary instructions, wasm trigger registrations, initial topology)
    BadTransactionsAmount,
    /// First transaction must contain single `Upgrade` instruction to set executor
    MustUpgrade,
}

/// Builder for blocks
#[derive(Debug, Clone)]
pub struct BlockBuilder<B>(B);

mod pending {
    use iroha_primitives::time::TimeSource;
    use nonzero_ext::nonzero;

    use super::*;

    /// First stage in the life-cycle of a [`Block`].
    /// In the beginning the block is assumed to be verified and to contain only accepted transactions.
    /// Additionally the block must retain events emitted during the execution of on-chain logic during
    /// the previous round, which might then be processed by the trigger system.
    #[derive(Debug, Clone)]
    pub struct Pending {
        /// Collection of transactions which have been accepted.
        transactions: Vec<AcceptedTransaction>,
        time_source: TimeSource,
    }

    impl BlockBuilder<Pending> {
        const TIME_PADDING: Duration = Duration::from_millis(1);

        /// Create [`Self`]
        #[inline]
        pub fn new(transactions: Vec<AcceptedTransaction>) -> Self {
            Self::new_with_time_source(transactions, TimeSource::new_system())
        }

        /// Create with provided [`TimeSource`] to use for block creation time.
        pub fn new_with_time_source(
            transactions: Vec<AcceptedTransaction>,
            time_source: TimeSource,
        ) -> Self {
            // Note that empty block is allowed

            Self(Pending {
                transactions,
                time_source,
            })
        }

        fn make_header(
            &self,
            prev_block: Option<&SignedBlock>,
            view_change_index: usize,
        ) -> BlockHeader {
            let prev_block_time =
                prev_block.map_or(Duration::ZERO, |block| block.header().creation_time());

            let latest_txn_time = self
                .0
                .transactions
                .iter()
                .map(AsRef::as_ref)
                .map(SignedTransaction::creation_time)
                .max()
                // Empty block is allowed
                .unwrap_or(Duration::ZERO);

            let now = self.0.time_source.get_unix_time();

            // NOTE: Lower time bound must always be upheld for a valid block
            // If the clock has drifted too far this block will be rejected
            let creation_time = [now, latest_txn_time, prev_block_time + Self::TIME_PADDING]
                .into_iter()
                .max()
                .unwrap();

            BlockHeader {
                height: prev_block.map(|block| block.header().height).map_or_else(
                    || nonzero!(1_u64),
                    |height| {
                        height
                            .checked_add(1)
                            .expect("INTERNAL BUG: Blockchain height exceeds usize::MAX")
                    },
                ),
                prev_block_hash: prev_block.map(SignedBlock::hash),
                merkle_root: self
                    .0
                    .transactions
                    .iter()
                    .map(AsRef::as_ref)
                    .map(SignedTransaction::hash_as_entrypoint)
                    .collect::<MerkleTree<_>>()
                    .root(),
                result_merkle_root: None,
                creation_time_ms: creation_time
                    .as_millis()
                    .try_into()
                    .expect("Time should fit into u64"),
                view_change_index: view_change_index
                    .try_into()
                    .expect("View change index should fit into u32"),
            }
        }

        /// Chain the block with existing blockchain.
        ///
        /// Upon executing this method current timestamp is stored in the block header.
        pub fn chain(
            self,
            view_change_index: usize,
            latest_block: Option<&SignedBlock>,
        ) -> BlockBuilder<Chained> {
            BlockBuilder(Chained {
                header: self.make_header(latest_block, view_change_index),
                transactions: self.0.transactions,
            })
        }
    }
}

mod chained {
    use iroha_crypto::SignatureOf;
    use new::NewBlock;

    use super::*;

    /// When a `Pending` block is chained with the blockchain it becomes [`Chained`] block.
    #[derive(Debug, Clone)]
    pub struct Chained {
        pub(super) header: BlockHeader,
        pub(super) transactions: Vec<AcceptedTransaction>,
    }

    impl BlockBuilder<Chained> {
        /// Sign this block and get [`NewBlock`].
        pub fn sign(self, private_key: &PrivateKey) -> WithEvents<NewBlock> {
            let signature =
                BlockSignature::new(0, SignatureOf::from_hash(private_key, self.0.header.hash()));

            WithEvents::new(NewBlock {
                signature,
                header: self.0.header,
                transactions: self.0.transactions,
            })
        }
    }
}

mod new {
    use super::*;
    use crate::state::StateBlock;

    /// First stage in the life-cycle of a block.
    ///
    /// Transactions in this block are not categorized.
    #[derive(Debug, Clone)]
    pub struct NewBlock {
        pub(super) signature: BlockSignature,
        pub(super) header: BlockHeader,
        pub(super) transactions: Vec<AcceptedTransaction>,
    }

    impl NewBlock {
        /// Transition to [`ValidBlock`]. Skips static checks and only applies state changes.
        pub fn validate_and_record_transactions(
            self,
            state_block: &mut StateBlock<'_>,
        ) -> WithEvents<ValidBlock> {
            ValidBlock::validate_unchecked(self.into(), state_block)
        }

        /// Block signature
        pub fn signature(&self) -> &BlockSignature {
            &self.signature
        }

        /// Block header
        pub fn header(&self) -> BlockHeader {
            self.header
        }

        /// Block transactions
        pub fn transactions(&self) -> &[AcceptedTransaction] {
            &self.transactions
        }

        #[cfg(test)]
        pub(crate) fn update_header(self, header: BlockHeader, private_key: &PrivateKey) -> Self {
            let signature = BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(private_key, header.hash()),
            );

            Self {
                signature,
                header,
                transactions: self.transactions,
            }
        }
    }

    impl From<NewBlock> for SignedBlock {
        fn from(block: NewBlock) -> Self {
            SignedBlock::presigned(
                block.signature,
                block.header,
                // FIXME: transmute somehow
                block.transactions.into_iter().map(Into::into).collect(),
            )
        }
    }
}

mod valid {
    use std::time::SystemTime;

    use commit::CommittedBlock;
    use iroha_data_model::{
        events::pipeline::PipelineEventBox, isi::InstructionBox, prelude::Executable, ChainId,
    };

    use super::*;
    use crate::{
        smartcontracts::wasm::cache::WasmCache,
        state::{
            storage_transactions::TransactionsReadOnly, StateBlock, StateReadOnlyWithTransactions,
        },
        sumeragi::network_topology::Role,
    };

    /// Block that was validated and accepted
    #[derive(Debug, Clone)]
    #[repr(transparent)]
    pub struct ValidBlock(pub(super) SignedBlock);

    type Error = (Box<SignedBlock>, BlockValidationError);

    impl ValidBlock {
        fn verify_leader_signature(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            use SignatureVerificationError::LeaderMissing;
            let leader_idx = topology.leader_index();

            let signature = block.signatures().next().ok_or(LeaderMissing)?;
            if leader_idx != usize::try_from(signature.index).map_err(|_err| LeaderMissing)? {
                return Err(LeaderMissing);
            }

            signature
                .signature
                .verify_hash(
                    topology.leader().public_key(),
                    block.payload().header.hash(),
                )
                .map_err(|_err| LeaderMissing)?;

            Ok(())
        }

        fn verify_validator_signatures(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            let valid_roles: &[Role] = if topology.view_change_index() >= 1 {
                &[Role::ValidatingPeer, Role::ObservingPeer]
            } else {
                &[Role::ValidatingPeer]
            };

            topology
                .filter_signatures_by_roles(valid_roles, block.signatures())
                .try_for_each(|signature| {
                    use SignatureVerificationError::{UnknownSignatory, UnknownSignature};

                    let signatory =
                        usize::try_from(signature.index).map_err(|_err| UnknownSignatory)?;
                    let signatory: &PeerId =
                        topology.as_ref().get(signatory).ok_or(UnknownSignatory)?;

                    signature
                        .signature
                        .verify_hash(signatory.public_key(), block.payload().header.hash())
                        .map_err(|_err| UnknownSignature)?;

                    Ok(())
                })?;

            Ok(())
        }

        fn verify_no_undefined_signatures(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            if topology
                .filter_signatures_by_roles(&[Role::Undefined], block.signatures())
                .next()
                .is_some()
            {
                return Err(SignatureVerificationError::UnknownSignatory);
            }

            Ok(())
        }

        fn verify_proxy_tail_signature(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            use SignatureVerificationError::ProxyTailMissing;
            let proxy_tail_idx = topology.proxy_tail_index();

            let signature = block
                .signatures()
                .find(|x| {
                    let idx =
                        usize::try_from(x.index).expect("there could not be so many signatures");
                    idx == proxy_tail_idx
                })
                .ok_or(ProxyTailMissing)?;

            signature
                .signature
                .verify_hash(
                    topology.proxy_tail().public_key(),
                    block.payload().header.hash(),
                )
                .map_err(|_err| ProxyTailMissing)?;

            Ok(())
        }
        /// Validate the given block, apply resulting state changes,
        /// and record any transaction errors back into the block.
        pub fn validate(
            mut block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            state_block: &mut StateBlock<'_>,
        ) -> WithEvents<Result<ValidBlock, Error>> {
            if let Err(error) =
                Self::validate_static(&block, topology, expected_chain_id, state_block, false)
            {
                return WithEvents::new(Err((Box::new(block), error)));
            }
            Self::validate_and_record_transactions(&mut block, state_block);
            WithEvents::new(Ok(ValidBlock(block)))
        }

        /// Same as [`Self::validate`] but:
        /// * Block will be validated (statically checked) with read-only state
        /// * If block is valid, voting block will be released,
        ///   and transactions will be validated (executed) with write state
        pub fn validate_keep_voting_block<'state>(
            mut block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            if let Err(error) = Self::validate_static(
                &block,
                topology,
                expected_chain_id,
                &state.view(),
                soft_fork,
            ) {
                return WithEvents::new(Err((Box::new(block), error)));
            }
            // Release block writer before creating new one
            let _ = voting_block.take();
            let mut state_block = if soft_fork {
                state.block_and_revert(block.header())
            } else {
                state.block(block.header())
            };
            Self::validate_and_record_transactions(&mut block, &mut state_block);
            WithEvents::new(Ok((ValidBlock(block), state_block)))
        }

        /// All static checks of the block.
        fn validate_static(
            block: &SignedBlock,
            topology: &Topology,
            chain_id: &ChainId,
            state: &impl StateReadOnlyWithTransactions,
            soft_fork: bool,
        ) -> Result<(), BlockValidationError> {
            let expected_block_height = if soft_fork {
                state.height()
            } else {
                state
                    .height()
                    .checked_add(1)
                    .expect("INTERNAL BUG: Block height exceeds usize::MAX")
            };
            let actual_height = block
                .header()
                .height
                .get()
                .try_into()
                .expect("INTERNAL BUG: Block height exceeds usize::MAX");

            if expected_block_height != actual_height {
                return Err(BlockValidationError::PrevBlockHeightMismatch {
                    expected: expected_block_height,
                    actual: actual_height,
                });
            }

            // TODO: inject TimeSource
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            let max_clock_drift = state.world().parameters().sumeragi.max_clock_drift();
            let block_creation_time = block.header().creation_time();
            if block_creation_time.saturating_sub(now) > max_clock_drift {
                return Err(BlockValidationError::BlockInTheFuture);
            }

            let expected_prev_block_hash = if soft_fork {
                state.prev_block_hash()
            } else {
                state.latest_block_hash()
            };
            let actual_prev_block_hash = block.header().prev_block_hash;

            if expected_prev_block_hash != actual_prev_block_hash {
                return Err(BlockValidationError::PrevBlockHashMismatch {
                    expected: expected_prev_block_hash,
                    actual: actual_prev_block_hash,
                });
            }

            if block.header().is_genesis() {
                check_genesis_block(block)?;
            } else {
                let prev_block_time = if soft_fork {
                    state.prev_block()
                } else {
                    state.latest_block()
                }
                .expect("INTERNAL BUG: Genesis not committed")
                .header()
                .creation_time();

                if block.header().creation_time() <= prev_block_time {
                    return Err(BlockValidationError::BlockInThePast);
                }

                Self::verify_leader_signature(block, topology)?;
                Self::verify_validator_signatures(block, topology)?;
                Self::verify_no_undefined_signatures(block, topology)?;
            }

            let (max_clock_drift, tx_params) = {
                let params = state.world().parameters();
                (params.sumeragi().max_clock_drift(), params.transaction())
            };

            for tx in block.external_transactions() {
                if state
                    .transactions()
                    .get(&tx.hash())
                    // In case of soft-fork transaction is check if it was added at the same height as candidate block
                    .is_some_and(|height| height.get() < expected_block_height)
                {
                    return Err(BlockValidationError::HasCommittedTransactions);
                }

                if tx.creation_time() > block_creation_time {
                    return Err(BlockValidationError::TransactionInTheFuture);
                }

                if block.header().is_genesis() {
                    AcceptedTransaction::validate_genesis(tx, chain_id, max_clock_drift)?;
                } else {
                    AcceptedTransaction::validate(tx, chain_id, max_clock_drift, tx_params)?;
                }
            }

            // TODO: can it be done in a single iteration over block transactions above?
            let expected_merkle_root = block
                .external_transactions()
                .map(SignedTransaction::hash_as_entrypoint)
                .collect::<MerkleTree<_>>()
                .root();
            let actual_merkle_root = block.header().merkle_root();

            if expected_merkle_root != actual_merkle_root {
                return Err(BlockValidationError::MerkleRootMismatch);
            }

            Ok(())
        }

        /// Validate each transaction in the block, apply resulting state changes,
        /// and record results back into the block.
        ///
        /// Must be called with a **block that is _assumed_ to be valid**.
        fn validate_and_record_transactions(
            block: &mut SignedBlock,
            state_block: &mut StateBlock<'_>,
        ) {
            let mut wasm_cache = WasmCache::new();
            let (mut hashes, mut results) = block.external_transactions().cloned().fold(
                (Vec::new(), Vec::new()),
                |mut acc, tx| {
                    // NOTE: function is called with the assumption that the transactions are acceptable
                    // FIXME: cloning is unnecessary; use Cow?
                    let accepted_tx = AcceptedTransaction::new_unchecked(tx.clone());

                    let (hash, result) =
                        state_block.validate_transaction(accepted_tx, &mut wasm_cache);

                    match &result {
                        Err(reason) => {
                            iroha_logger::debug!(
                                tx=%hash,
                                block=%block.hash(),
                                reason=?reason,
                                "Transaction rejected"
                            );
                        }
                        Ok(trigger_sequence) => {
                            iroha_logger::debug!(
                                tx=%hash,
                                block=%block.hash(),
                                trigger_sequence=?trigger_sequence,
                                "Transaction approved"
                            );
                        }
                    }

                    acc.0.push(hash);
                    acc.1.push(result);
                    acc
                },
            );

            let (time_trgs, mut time_trg_hashes, mut time_trg_results) =
                state_block.execute_time_triggers(&block.header());
            hashes.append(&mut time_trg_hashes);
            results.append(&mut time_trg_results);

            block.set_transaction_results(time_trgs, hashes, results);
        }

        /// Like [`Self::validate`], but without the static check part.
        ///
        /// Useful for cases when the block is assumed to be valid:
        ///
        /// - When block is created by the node
        /// - For Explorer, which is not interested in validation and only needs
        ///   state changes
        pub fn validate_unchecked(
            mut block: SignedBlock,
            state_block: &mut StateBlock<'_>,
        ) -> WithEvents<ValidBlock> {
            Self::validate_and_record_transactions(&mut block, state_block);
            WithEvents::new(ValidBlock(block))
        }

        /// Add additional signature for [`Self`]
        ///
        /// # Errors
        ///
        /// If given signature doesn't match block hash
        pub fn add_signature(
            &mut self,
            signature: BlockSignature,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            use SignatureVerificationError::{Other, UnknownSignatory, UnknownSignature};

            let signatory = usize::try_from(signature.index).map_err(|_err| UnknownSignatory)?;
            let signatory = topology.as_ref().get(signatory).ok_or(UnknownSignatory)?;

            assert_ne!(Role::Leader, topology.role(signatory));
            assert_ne!(Role::ProxyTail, topology.role(signatory));
            assert_ne!(Role::Undefined, topology.role(signatory));

            if topology.view_change_index() == 0 {
                assert_ne!(Role::ObservingPeer, topology.role(signatory),);
            }

            signature
                .signature
                .verify_hash(
                    signatory.public_key(),
                    self.as_ref().payload().header.hash(),
                )
                .map_err(|_err| UnknownSignature)?;

            self.0.add_signature(signature).map_err(|_err| Other)
        }

        /// Replace block's signatures. Returns previous block signatures
        ///
        /// # Errors
        ///
        /// - Replacement signatures don't contain the leader signature
        /// - Replacement signatures contain unknown signatories
        /// - Replacement signatures contain incorrect signatures
        /// - Replacement signatures contain duplicate signatures
        pub fn replace_signatures(
            &mut self,
            signatures: BTreeSet<BlockSignature>,
            topology: &Topology,
        ) -> WithEvents<Result<BTreeSet<BlockSignature>, SignatureVerificationError>> {
            let Ok(prev_signatures) = self.0.replace_signatures(signatures) else {
                return WithEvents::new(Err(SignatureVerificationError::Other));
            };

            let result = if let Err(err) = Self::verify_leader_signature(self.as_ref(), topology)
                .and_then(|()| Self::verify_validator_signatures(self.as_ref(), topology))
                .and_then(|()| Self::verify_no_undefined_signatures(self.as_ref(), topology))
            {
                self.0
                    .replace_signatures(prev_signatures)
                    .expect("INTERNAL BUG: invalid signatures in block");
                Err(err)
            } else {
                Ok(prev_signatures)
            };

            WithEvents::new(result)
        }

        /// Transition block to [`CommittedBlock`].
        ///
        /// # Errors
        ///
        /// - Block is not signed by the proxy tail
        /// - Block doesn't have enough signatures
        pub fn commit(
            self,
            topology: &Topology,
        ) -> WithEvents<Result<CommittedBlock, (Box<ValidBlock>, BlockValidationError)>> {
            WithEvents::new(match Self::is_commit(self.as_ref(), topology) {
                Err(err) => Err((Box::new(self), err.into())),
                Ok(()) => Ok(CommittedBlock(self)),
            })
        }

        /// Like [`Self::commit`], but without block signature checks.
        ///
        /// Useful e.g. for Explorer, which assumes all blocks from Iroha are valid, and
        /// only executes them to produce state changes.
        pub fn commit_unchecked(self) -> WithEvents<CommittedBlock> {
            WithEvents::new(CommittedBlock(self))
        }

        /// Validate and commit block if possible.
        ///
        /// The difference from calling [`Self::validate_keep_voting_block`] + [`ValidBlock::commit`]
        /// is that signatures are eagerly checked first.
        #[allow(clippy::too_many_arguments)]
        pub fn commit_keep_voting_block<'state, F: Fn(PipelineEventBox)>(
            block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
            send_events: F,
        ) -> WithEvents<Result<(CommittedBlock, StateBlock<'state>), Error>> {
            if let Err(err) = Self::is_commit(&block, topology) {
                return WithEvents::new(Err((Box::new(block), err.into())));
            }

            WithEvents::new(
                Self::validate_keep_voting_block(
                    block,
                    topology,
                    expected_chain_id,
                    state,
                    voting_block,
                    soft_fork,
                )
                .unpack(send_events)
                .map(|(block, state_block)| (CommittedBlock(block), state_block)),
            )
        }

        /// Check if block satisfy requirements to be committed
        ///
        /// # Errors
        ///
        /// - Block is not signed by the proxy tail
        /// - Block doesn't have enough signatures
        fn is_commit(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            if !block.header().is_genesis() {
                Self::verify_proxy_tail_signature(block, topology)?;

                let votes_count = block.signatures().len();
                if votes_count < topology.min_votes_for_commit() {
                    return Err(SignatureVerificationError::NotEnoughSignatures {
                        votes_count,
                        min_votes_for_commit: topology.min_votes_for_commit(),
                    });
                }
            }

            Ok(())
        }

        /// Add additional signatures for [`Self`].
        pub fn sign(&mut self, key_pair: &KeyPair, topology: &Topology) {
            let signatory_idx = topology
                .position(key_pair.public_key())
                .expect("INTERNAL BUG: Node is not in topology");

            self.0.sign(key_pair.private_key(), signatory_idx);
        }

        #[cfg(test)]
        pub(crate) fn new_dummy(leader_private_key: &PrivateKey) -> Self {
            Self::new_dummy_and_modify_header(leader_private_key, |_| {})
        }

        #[cfg(test)]
        pub(crate) fn new_dummy_and_modify_header(
            leader_private_key: &PrivateKey,
            f: impl FnOnce(&mut BlockHeader),
        ) -> Self {
            let merkle_root = HashOf::from_untyped_unchecked(Hash::prehashed([1; Hash::LENGTH]));
            let mut header = BlockHeader {
                height: nonzero_ext::nonzero!(2_u64),
                prev_block_hash: None,
                merkle_root: Some(merkle_root),
                result_merkle_root: None,
                creation_time_ms: 0,
                view_change_index: 0,
            };
            f(&mut header);
            let unverified_block = BlockBuilder(Chained {
                header,
                transactions: Vec::new(),
            })
            .sign(leader_private_key)
            .unpack(|_| {});

            Self(SignedBlock::presigned(
                unverified_block.signature,
                unverified_block.header,
                unverified_block
                    .transactions
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ))
        }
    }

    impl From<ValidBlock> for SignedBlock {
        fn from(source: ValidBlock) -> Self {
            source.0
        }
    }

    impl AsRef<SignedBlock> for ValidBlock {
        fn as_ref(&self) -> &SignedBlock {
            &self.0
        }
    }

    fn check_genesis_block(block: &SignedBlock) -> Result<(), InvalidGenesisError> {
        assert!(block.signatures().len() == 0);

        // FIXME #5473: check this after dynamic validation
        if block.results().any(|result| result.0.is_err()) {
            return Err(InvalidGenesisError::ContainsErrors);
        }

        let transactions = block.payload().transactions.as_slice();
        if transactions.is_empty() || transactions.len() > 5 {
            return Err(InvalidGenesisError::BadTransactionsAmount);
        }
        for (i, transaction) in transactions.iter().enumerate() {
            if *transaction.authority() != *iroha_genesis::GENESIS_ACCOUNT_ID {
                return Err(InvalidGenesisError::UnexpectedAuthority);
            }
            let Executable::Instructions(isi) = transaction.instructions() else {
                return Err(InvalidGenesisError::NotInstructions);
            };
            if i == 0 {
                let [InstructionBox::Upgrade(_)] = isi.as_ref() else {
                    return Err(InvalidGenesisError::MustUpgrade);
                };
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use iroha_crypto::SignatureOf;

        use super::*;
        use crate::sumeragi::network_topology::test_topology_with_keys;

        #[test]
        fn signature_verification_ok() {
            let key_pairs = core::iter::repeat_with(KeyPair::random)
                .take(7)
                .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let payload = block.0.payload().clone();
            key_pairs
                .iter()
                .enumerate()
                // Include only peers in validator set
                .take(topology.min_votes_for_commit())
                // Skip leader since already singed
                .skip(1)
                .filter(|(i, _)| *i != 4) // Skip proxy tail
                .map(|(i, key_pair)| {
                    BlockSignature::new(
                        i as u64,
                        SignatureOf::from_hash(key_pair.private_key(), payload.header.hash()),
                    )
                })
                .try_for_each(|signature| block.add_signature(signature, &topology))
                .expect("Failed to add signatures");

            block.sign(&key_pairs[4], &topology);

            let _ = block.commit(&topology).unpack(|_| {}).unwrap();
        }

        #[test]
        fn signature_verification_consensus_not_required_ok() {
            let key_pairs = core::iter::repeat_with(KeyPair::random)
                .take(1)
                .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let block = ValidBlock::new_dummy(key_pairs[0].private_key());

            assert!(block.commit(&topology).unpack(|_| {}).is_ok());
        }

        /// Check requirement of having at least $2f + 1$ signatures in $3f + 1$ network
        #[test]
        fn signature_verification_not_enough_signatures() {
            let key_pairs = core::iter::repeat_with(KeyPair::random)
                .take(7)
                .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[4], &topology);

            assert_eq!(
                block.commit(&topology).unpack(|_| {}).unwrap_err().1,
                SignatureVerificationError::NotEnoughSignatures {
                    votes_count: 2,
                    min_votes_for_commit: topology.min_votes_for_commit(),
                }
                .into()
            )
        }

        /// Check requirement of having leader signature
        #[test]
        fn signature_verification_miss_proxy_tail_signature() {
            let key_pairs = core::iter::repeat_with(KeyPair::random)
                .take(7)
                .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let payload = block.0.payload().clone();
            key_pairs
                .iter()
                .enumerate()
                // Include only peers in validator set
                .take(topology.min_votes_for_commit())
                // Skip leader since already singed
                .skip(1)
                .filter(|(i, _)| *i != 4) // Skip proxy tail
                .map(|(i, key_pair)| {
                    BlockSignature::new(
                        i as u64,
                        SignatureOf::from_hash(key_pair.private_key(), payload.header.hash()),
                    )
                })
                .try_for_each(|signature| block.add_signature(signature, &topology))
                .expect("Failed to add signatures");

            assert_eq!(
                block.commit(&topology).unpack(|_| {}).unwrap_err().1,
                SignatureVerificationError::ProxyTailMissing.into()
            )
        }
    }
}

mod commit {
    use super::*;

    /// Represents a block accepted by consensus.
    /// Every [`Self`] will have a different height.
    #[derive(Debug, Clone)]
    pub struct CommittedBlock(pub(super) ValidBlock);

    impl From<CommittedBlock> for ValidBlock {
        fn from(source: CommittedBlock) -> Self {
            ValidBlock(source.0.into())
        }
    }

    impl From<CommittedBlock> for SignedBlock {
        fn from(source: CommittedBlock) -> Self {
            source.0 .0
        }
    }

    impl AsRef<SignedBlock> for CommittedBlock {
        fn as_ref(&self) -> &SignedBlock {
            &self.0 .0
        }
    }

    #[cfg(test)]
    impl AsMut<SignedBlock> for CommittedBlock {
        fn as_mut(&mut self) -> &mut SignedBlock {
            &mut self.0 .0
        }
    }
}

mod event {
    use std::collections::BTreeSet;

    use new::NewBlock;

    use super::*;
    use crate::state::StateBlock;

    pub trait EventProducer {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox>;
    }

    #[derive(Debug)]
    #[must_use]
    pub struct WithEvents<B>(B);

    impl<B> WithEvents<B> {
        pub(super) fn new(source: B) -> Self {
            Self(source)
        }
    }

    impl<B: EventProducer, U> WithEvents<Result<B, (U, BlockValidationError)>> {
        pub fn unpack<F: Fn(PipelineEventBox)>(self, f: F) -> Result<B, (U, BlockValidationError)> {
            match self.0 {
                Ok(ok) => Ok(WithEvents(ok).unpack(f)),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl<'state, B: EventProducer, U>
        WithEvents<Result<(B, StateBlock<'state>), (U, BlockValidationError)>>
    {
        pub fn unpack<F: Fn(PipelineEventBox)>(
            self,
            f: F,
        ) -> Result<(B, StateBlock<'state>), (U, BlockValidationError)> {
            match self.0 {
                Ok((ok, state)) => Ok((WithEvents(ok).unpack(f), state)),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl WithEvents<Result<BTreeSet<BlockSignature>, SignatureVerificationError>> {
        pub fn unpack<F: Fn(PipelineEventBox)>(
            self,
            f: F,
        ) -> Result<BTreeSet<BlockSignature>, SignatureVerificationError> {
            match self.0 {
                Ok(ok) => Ok(ok),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl<B: EventProducer> WithEvents<B> {
        pub fn unpack<F: Fn(PipelineEventBox)>(self, f: F) -> B {
            self.0.produce_events().for_each(f);
            self.0
        }
    }

    impl<B, E: EventProducer> WithEvents<(B, E)> {
        pub(crate) fn unpack<F: Fn(PipelineEventBox)>(self, f: F) -> (B, E) {
            self.0 .1.produce_events().for_each(f);
            self.0
        }
    }

    impl EventProducer for NewBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block_event = BlockEvent {
                header: self.header,
                status: BlockStatus::Created,
            };

            core::iter::once(block_event.into())
        }
    }

    impl EventProducer for ValidBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block_height = self.as_ref().header().height;

            let block = self.as_ref();
            let tx_events = block
                .external_transactions()
                .enumerate()
                .map(move |(idx, tx)| {
                    let status = block.error(idx).map_or_else(
                        || TransactionStatus::Approved,
                        |error| TransactionStatus::Rejected(Box::new(error.clone())),
                    );

                    TransactionEvent {
                        block_height: Some(block_height),
                        hash: tx.hash(),
                        status,
                    }
                });

            let block_event = core::iter::once(BlockEvent {
                header: self.as_ref().header(),
                status: BlockStatus::Approved,
            });

            tx_events
                .map(PipelineEventBox::from)
                .chain(block_event.map(Into::into))
        }
    }

    impl EventProducer for CommittedBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block_event = core::iter::once(BlockEvent {
                header: self.as_ref().header(),
                status: BlockStatus::Committed,
            });

            block_event.map(Into::into)
        }
    }

    impl EventProducer for BlockValidationError {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            // TODO:
            core::iter::empty()
        }
    }

    impl EventProducer for SignatureVerificationError {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            // TODO:
            core::iter::empty()
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::prelude::*;
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use iroha_test_samples::gen_account_in;

    use super::*;
    use crate::{
        kura::Kura, query::store::LiveQueryStore, state::State,
        sumeragi::network_topology::test_topology,
    };

    #[test]
    pub fn committed_and_valid_block_hashes_are_equal() {
        let peer_key_pair = KeyPair::random();
        let peer_id = PeerId::new(peer_key_pair.public_key().clone());
        let topology = Topology::new(vec![peer_id]);
        let valid_block = ValidBlock::new_dummy(peer_key_pair.private_key());
        let committed_block = valid_block
            .clone()
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();

        assert_eq!(valid_block.0.hash(), committed_block.as_ref().hash())
    }

    #[tokio::test]
    async fn should_reject_due_to_repetition() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain_id = "wonderland".parse().expect("Valid");
        let domain = Domain::new(domain_id).build(&alice_id);
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction)
        };
        // Creating an instruction
        let asset_definition_id = "xor#wonderland".parse().expect("Valid");
        let create_asset_definition =
            Register::asset_definition(AssetDefinition::numeric(asset_definition_id));

        // Making two transactions that have the same instruction
        let tx = TransactionBuilder::new(chain_id.clone(), alice_id)
            .with_instructions([create_asset_definition])
            .sign(alice_keypair.private_key());
        let tx =
            AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_limits).expect("Valid");

        // Creating a block of two identical transactions and validating it
        let transactions = vec![tx.clone(), tx];
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state_block.commit();

        // The 1st transaction should be confirmed and the 2nd rejected
        assert_eq!(valid_block.as_ref().errors().next().unwrap().0, 1);
    }

    #[tokio::test]
    async fn tx_order_same_in_validation_and_revalidation() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain_id = "wonderland".parse().expect("Valid");
        let domain = Domain::new(domain_id).build(&alice_id);
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction)
        };
        // Creating an instruction
        let asset_definition_id = "xor#wonderland"
            .parse::<AssetDefinitionId>()
            .expect("Valid");
        let create_asset_definition =
            Register::asset_definition(AssetDefinition::numeric(asset_definition_id.clone()));

        // Making two transactions that have the same instruction
        let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([create_asset_definition])
            .sign(alice_keypair.private_key());
        let tx =
            AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_limits).expect("Valid");

        let fail_mint = Mint::asset_numeric(
            20u32,
            AssetId::new(asset_definition_id.clone(), alice_id.clone()),
        );

        let succeed_mint =
            Mint::asset_numeric(200u32, AssetId::new(asset_definition_id, alice_id.clone()));

        let tx0 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([fail_mint])
            .sign(alice_keypair.private_key());
        let tx0 =
            AcceptedTransaction::accept(tx0, &chain_id, max_clock_drift, tx_limits).expect("Valid");

        let tx2 = TransactionBuilder::new(chain_id.clone(), alice_id)
            .with_instructions([succeed_mint])
            .sign(alice_keypair.private_key());
        let tx2 =
            AcceptedTransaction::accept(tx2, &chain_id, max_clock_drift, tx_limits).expect("Valid");

        // Creating a block of two identical transactions and validating it
        let transactions = vec![tx0, tx, tx2];
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state_block.commit();

        // The 1st transaction should fail and 2nd succeed
        let mut errors = valid_block.as_ref().errors();
        assert_eq!(0, errors.next().unwrap().0);
        assert!(errors.next().is_none());
    }

    #[tokio::test]
    async fn failed_transactions_revert() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain_id = "wonderland".parse().expect("Valid");
        let domain = Domain::new(domain_id).build(&alice_id);
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction)
        };
        let domain_id = "domain".parse().expect("Valid");
        let create_domain = Register::domain(Domain::new(domain_id));
        let asset_definition_id = "coin#domain".parse().expect("Valid");
        let create_asset =
            Register::asset_definition(AssetDefinition::numeric(asset_definition_id));
        let fail_isi = Unregister::domain("dummy".parse().unwrap());
        let tx_fail = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions::<InstructionBox>([create_domain.clone().into(), fail_isi.into()])
            .sign(alice_keypair.private_key());
        let tx_fail = AcceptedTransaction::accept(tx_fail, &chain_id, max_clock_drift, tx_limits)
            .expect("Valid");
        let tx_accept = TransactionBuilder::new(chain_id.clone(), alice_id)
            .with_instructions::<InstructionBox>([create_domain.into(), create_asset.into()])
            .sign(alice_keypair.private_key());
        let tx_accept =
            AcceptedTransaction::accept(tx_accept, &chain_id, max_clock_drift, tx_limits)
                .expect("Valid");

        // Creating a block of where first transaction must fail and second one fully executed
        let transactions = vec![tx_fail, tx_accept];
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state_block.commit();

        let mut errors = valid_block.as_ref().errors();
        // The 1st transaction should be rejected
        assert_eq!(
            0,
            errors.next().unwrap().0,
            "The first transaction should be rejected, as it contains `Fail`."
        );

        // The second transaction should be accepted
        assert!(
            errors.next().is_none(),
            "The second transaction should be accepted."
        );
    }

    #[tokio::test]
    #[ignore = "Should be restored in #5473"]
    async fn genesis_public_key_is_checked() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let genesis_correct_key = KeyPair::random();
        let genesis_wrong_key = KeyPair::random();
        let genesis_correct_account_id = AccountId::new(
            GENESIS_DOMAIN_ID.clone(),
            genesis_correct_key.public_key().clone(),
        );
        let genesis_wrong_account_id = AccountId::new(
            GENESIS_DOMAIN_ID.clone(),
            genesis_wrong_key.public_key().clone(),
        );
        let genesis_domain =
            Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_correct_account_id);
        let genesis_wrong_account =
            Account::new(genesis_wrong_account_id.clone()).build(&genesis_wrong_account_id);
        let world = World::with([genesis_domain], [genesis_wrong_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);

        // Creating an instruction
        let isi = Log::new(
            iroha_data_model::Level::DEBUG,
            "instruction itself doesn't matter here".to_string(),
        );

        // Create genesis transaction
        // Sign with `genesis_wrong_key` as peer which has incorrect genesis key pair
        // Bypass `accept_genesis` check to allow signing with wrong key
        let tx = TransactionBuilder::new(chain_id.clone(), genesis_wrong_account_id.clone())
            .with_instructions([isi])
            .sign(genesis_wrong_key.private_key());
        let tx = AcceptedTransaction::new_unchecked(tx);

        // Create genesis block
        let transactions = vec![tx];
        let topology = test_topology(1);
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(genesis_correct_key.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state_block.commit();

        // Validate genesis block
        // Use correct genesis key and check if transaction is rejected
        let block: SignedBlock = valid_block.into();
        let mut state_block = state.block(block.header());
        let (_, error) = ValidBlock::validate(block, &topology, &chain_id, &mut state_block)
            .unpack(|_| {})
            .unwrap_err();
        state_block.commit();

        // The first transaction should be rejected
        assert_eq!(
            error,
            BlockValidationError::InvalidGenesis(InvalidGenesisError::UnexpectedAuthority)
        )
    }
}
