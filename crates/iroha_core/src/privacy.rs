//! Canonical signed privacy submissions and transaction-atomic resource budgets.
//!
//! Protocol availability is resolved from the native implementation and optional committed
//! limits in `privacy_state`; governance does not activate or qualify proof relations.
use iroha_data_model::{
    ValidationFail,
    isi::privacy::SubmitPrivacyProofV1,
    privacy::{
        PrivacyConsensusLimitsV1, PrivacyConsensusLimitsValidationError,
        PrivacyTransactionIntentDigestV1,
    },
    transaction::SignedTransaction,
};
use thiserror::Error;
const PRIVACY_SIGNED_SUBMISSION_HASH_DOMAIN_V1: &[u8] = b"iroha.privacy.signed-submission-hash.v1";
/// Hash the complete typed privacy submission authorized by the transaction signature.
///
/// Unlike the transaction-intent digest, this internal one-shot fingerprint includes the final
/// statement digests and proof bytes. It prevents a child contract, trigger, or VM overlay from
/// substituting another submission after admission has validated the signed direct instruction.
pub(crate) fn privacy_signed_submission_hash_v1(
    submission: &SubmitPrivacyProofV1,
) -> Result<iroha_crypto::Hash, norito::Error> {
    let encoded = norito::encode_canonical(submission)?;
    let encoded_len = u64::try_from(encoded.len())
        .expect("Norito output length always fits u64 on supported targets");
    let mut preimage = Vec::with_capacity(
        PRIVACY_SIGNED_SUBMISSION_HASH_DOMAIN_V1.len()
            + core::mem::size_of::<u64>()
            + encoded.len(),
    );
    preimage.extend_from_slice(PRIVACY_SIGNED_SUBMISSION_HASH_DOMAIN_V1);
    preimage.extend_from_slice(&encoded_len.to_le_bytes());
    preimage.extend_from_slice(&encoded);
    Ok(iroha_crypto::Hash::new(preimage))
}
/// Recompute and validate the optional privacy intent in one signed payload.
///
/// Ordinary transactions return `Ok(None)`. Any privacy-bearing payload must contain exactly one
/// direct typed submission, have both derived digests correct, and avoid unsupported opaque V1
/// paths. Mixed-batch contract calls remain in the signed intent; the executor must additionally
/// bind this authorization to the original direct native occurrence before any effect.
pub(crate) fn signed_privacy_transaction_intent_binding_v1(
    transaction: &SignedTransaction,
) -> Result<Option<(PrivacyTransactionIntentDigestV1, iroha_crypto::Hash)>, ValidationFail> {
    let Some((digest, submission)) = transaction
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|error| {
            ValidationFail::NotPermitted(format!(
                "privacy transaction-intent admission failed: {error}"
            ))
        })?
    else {
        return Ok(None);
    };
    let submission_hash = privacy_signed_submission_hash_v1(submission).map_err(|error| {
        ValidationFail::InternalError(format!(
            "failed to encode the signed privacy submission: {error}"
        ))
    })?;
    Ok(Some((digest, submission_hash)))
}
/// Resource budget already committed by privacy actions in one block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrivacyBlockBudgetV1 {
    limits: PrivacyConsensusLimitsV1,
    actions: u32,
    bytes: u64,
}
impl PrivacyBlockBudgetV1 {
    /// Construct an empty block budget.
    ///
    /// # Errors
    ///
    /// Returns an error when `limits` is not a valid first-release profile.
    pub fn new(limits: PrivacyConsensusLimitsV1) -> Result<Self, PrivacyBudgetError> {
        limits
            .validate()
            .map_err(PrivacyBudgetError::InvalidLimits)?;
        Ok(Self {
            limits,
            actions: 0,
            bytes: 0,
        })
    }
    /// Start a transaction-local budget.
    ///
    /// Charges remain local until [`PrivacyTransactionBudgetV1::commit`] is
    /// called.  Dropping the value rolls every reservation back.
    pub fn begin_transaction(&mut self) -> PrivacyTransactionBudgetV1<'_> {
        PrivacyTransactionBudgetV1 {
            block: self,
            actions: 0,
            bytes: 0,
        }
    }
    /// Number of committed privacy actions in this block.
    #[must_use]
    pub const fn actions(&self) -> u32 {
        self.actions
    }
    /// Number of committed canonical privacy bytes in this block.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
    /// Exact chain-wide limits bound to this block budget.
    #[must_use]
    pub const fn limits(&self) -> &PrivacyConsensusLimitsV1 {
        &self.limits
    }
}
impl Default for PrivacyBlockBudgetV1 {
    fn default() -> Self {
        Self::new(PrivacyConsensusLimitsV1::taira_default())
            .expect("the compiled Taira privacy limits are valid")
    }
}
/// Transaction-local, rollback-safe privacy admission budget.
#[must_use = "dropping a privacy transaction budget rolls back its reservations"]
pub struct PrivacyTransactionBudgetV1<'block> {
    block: &'block mut PrivacyBlockBudgetV1,
    actions: u32,
    bytes: u64,
}
impl PrivacyTransactionBudgetV1<'_> {
    /// Reserve one canonically encoded privacy action.
    ///
    /// `action_index` must equal the number of privacy actions already
    /// reserved in this transaction.  This binds statement contexts to their
    /// exact ordering and rejects duplicate or skipped indexes.
    ///
    /// # Errors
    ///
    /// Returns an error without changing either transaction or block counters
    /// if any action, transaction, or block bound would be exceeded.
    pub fn reserve(
        &mut self,
        action_index: u32,
        encoded_action_bytes: u64,
    ) -> Result<(), PrivacyBudgetError> {
        if action_index != self.actions {
            return Err(PrivacyBudgetError::ActionIndexMismatch {
                expected: self.actions,
                actual: action_index,
            });
        }
        if encoded_action_bytes == 0 {
            return Err(PrivacyBudgetError::ZeroActionBytes);
        }
        if encoded_action_bytes > u64::from(self.block.limits.max_action_bytes) {
            return Err(PrivacyBudgetError::ActionTooLarge {
                bytes: encoded_action_bytes,
                max: self.block.limits.max_action_bytes,
            });
        }
        let next_tx_actions = self
            .actions
            .checked_add(1)
            .ok_or(PrivacyBudgetError::ArithmeticOverflow)?;
        if next_tx_actions > self.block.limits.max_actions_per_transaction {
            return Err(PrivacyBudgetError::TransactionActionsExceeded {
                actions: next_tx_actions,
                max: self.block.limits.max_actions_per_transaction,
            });
        }
        let next_block_actions = self
            .block
            .actions
            .checked_add(next_tx_actions)
            .ok_or(PrivacyBudgetError::ArithmeticOverflow)?;
        if next_block_actions > self.block.limits.max_actions_per_block {
            return Err(PrivacyBudgetError::BlockActionsExceeded {
                actions: next_block_actions,
                max: self.block.limits.max_actions_per_block,
            });
        }
        let next_tx_bytes = self
            .bytes
            .checked_add(encoded_action_bytes)
            .ok_or(PrivacyBudgetError::ArithmeticOverflow)?;
        if next_tx_bytes > u64::from(self.block.limits.max_privacy_bytes_per_transaction) {
            return Err(PrivacyBudgetError::TransactionBytesExceeded {
                bytes: next_tx_bytes,
                max: self.block.limits.max_privacy_bytes_per_transaction,
            });
        }
        let next_block_bytes = self
            .block
            .bytes
            .checked_add(next_tx_bytes)
            .ok_or(PrivacyBudgetError::ArithmeticOverflow)?;
        if next_block_bytes > u64::from(self.block.limits.max_privacy_bytes_per_block) {
            return Err(PrivacyBudgetError::BlockBytesExceeded {
                bytes: next_block_bytes,
                max: self.block.limits.max_privacy_bytes_per_block,
            });
        }
        self.actions = next_tx_actions;
        self.bytes = next_tx_bytes;
        Ok(())
    }
    /// Commit every transaction-local reservation into the block budget.
    ///
    /// The method consumes the transaction budget, so a successful reservation
    /// set cannot be committed twice.
    pub fn commit(self) {
        self.block.actions = self
            .block
            .actions
            .checked_add(self.actions)
            .expect("reserve prevalidated the block action count");
        self.block.bytes = self
            .block
            .bytes
            .checked_add(self.bytes)
            .expect("reserve prevalidated the block byte count");
    }
    /// Number of locally reserved privacy actions.
    #[must_use]
    pub const fn actions(&self) -> u32 {
        self.actions
    }
    /// Number of locally reserved canonical privacy bytes.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}
/// Failure reserving privacy resources.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyBudgetError {
    /// The supplied consensus limits are invalid.
    #[error("invalid privacy budget limits: {0}")]
    InvalidLimits(PrivacyConsensusLimitsValidationError),
    /// Action indexes must be contiguous and begin at zero.
    #[error("privacy action index {actual} differs from expected index {expected}")]
    ActionIndexMismatch {
        /// Required next index.
        expected: u32,
        /// Supplied index.
        actual: u32,
    },
    /// A canonical encoded action cannot be empty.
    #[error("privacy action byte length must be non-zero")]
    ZeroActionBytes,
    /// One encoded action exceeds its hard bound.
    #[error("privacy action uses {bytes} bytes, exceeding maximum {max}")]
    ActionTooLarge {
        /// Supplied canonical byte count.
        bytes: u64,
        /// Maximum bytes in one action.
        max: u32,
    },
    /// The transaction contains too many privacy actions.
    #[error("privacy transaction has {actions} actions, exceeding maximum {max}")]
    TransactionActionsExceeded {
        /// Resulting transaction action count.
        actions: u32,
        /// Maximum transaction action count.
        max: u32,
    },
    /// The block contains too many privacy actions.
    #[error("privacy block has {actions} actions, exceeding maximum {max}")]
    BlockActionsExceeded {
        /// Resulting block action count.
        actions: u32,
        /// Maximum block action count.
        max: u32,
    },
    /// Canonical privacy bytes exceed the transaction budget.
    #[error("privacy transaction uses {bytes} bytes, exceeding maximum {max}")]
    TransactionBytesExceeded {
        /// Resulting transaction byte count.
        bytes: u64,
        /// Maximum transaction byte count.
        max: u32,
    },
    /// Canonical privacy bytes exceed the block budget.
    #[error("privacy block uses {bytes} bytes, exceeding maximum {max}")]
    BlockBytesExceeded {
        /// Resulting block byte count.
        bytes: u64,
        /// Maximum block byte count.
        max: u32,
    },
    /// A counter addition overflowed.
    #[error("privacy budget arithmetic overflow")]
    ArithmeticOverflow,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_profiles::{CompiledPrivacyProfileErrorV1, compiled_privacy_profile_v1};
    use iroha_data_model::privacy::PrivacyProtocolIdV1;
    #[test]
    fn signed_submission_hash_ignores_ambient_layout_flags_and_binds_proof_bytes() {
        for sample in iroha_data_model::privacy::privacy_action_role_samples_v1()
            .expect("all canonical privacy roles")
        {
            let mut submission = SubmitPrivacyProofV1::new(sample.envelope);
            let expected = privacy_signed_submission_hash_v1(&submission).expect("canonical hash");
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _layout = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            assert_eq!(
                privacy_signed_submission_hash_v1(&submission).unwrap(),
                expected
            );
            submission.envelope.proof.bytes_mut().bytes.push(0xA5);
            assert_ne!(
                privacy_signed_submission_hash_v1(&submission).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn incomplete_native_security_argument_still_refuses_admission() {
        assert_eq!(
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkX509StarkP256V1),
            Err(
                CompiledPrivacyProfileErrorV1::JointSecurityReductionIncomplete {
                    protocol_id: PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
                }
            )
        );
    }
    #[test]
    fn dropped_transaction_budget_rolls_back_all_charges() {
        let mut block = PrivacyBlockBudgetV1::default();
        {
            let mut transaction = block.begin_transaction();
            transaction.reserve(0, 7).expect("valid reservation");
            assert_eq!(transaction.actions(), 1);
            assert_eq!(transaction.bytes(), 7);
        }
        assert_eq!(block.actions(), 0);
        assert_eq!(block.bytes(), 0);
    }
    #[test]
    fn privacy_block_counts_and_actual_aggregate_bytes_are_independent() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let mut block = PrivacyBlockBudgetV1::new(limits).expect("valid limits");
        let first_bytes = u64::from(limits.max_action_bytes);
        let remaining = u64::from(limits.max_privacy_bytes_per_block) - first_bytes;
        assert!(remaining < first_bytes, "two maximal envelopes cannot fit");
        {
            let mut transaction = block.begin_transaction();
            transaction
                .reserve(0, first_bytes)
                .expect("maximum first action");
            transaction.commit();
        }
        {
            let mut transaction = block.begin_transaction();
            for bytes in [first_bytes, remaining + 1] {
                assert_eq!(
                    transaction.reserve(0, bytes),
                    Err(PrivacyBudgetError::BlockBytesExceeded {
                        bytes: first_bytes + bytes,
                        max: limits.max_privacy_bytes_per_block,
                    })
                );
                assert_eq!(transaction.actions(), 0);
                assert_eq!(transaction.bytes(), 0);
            }
            transaction
                .reserve(0, remaining)
                .expect("exact remaining aggregate bytes");
            transaction.commit();
        }
        assert_eq!(block.actions(), 2);
        assert_eq!(block.bytes(), u64::from(limits.max_privacy_bytes_per_block));
        let mut third = block.begin_transaction();
        assert!(matches!(
            third.reserve(0, 1),
            Err(PrivacyBudgetError::BlockActionsExceeded { .. })
        ));
        drop(third);
        assert_eq!(block.actions(), 2);
    }
    #[test]
    fn budget_rejects_index_skips_zero_length_and_oversize_without_mutation() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let mut block = PrivacyBlockBudgetV1::new(limits).expect("valid limits");
        let mut transaction = block.begin_transaction();
        assert_eq!(
            transaction.reserve(1, 1),
            Err(PrivacyBudgetError::ActionIndexMismatch {
                expected: 0,
                actual: 1,
            })
        );
        assert_eq!(
            transaction.reserve(0, 0),
            Err(PrivacyBudgetError::ZeroActionBytes)
        );
        assert_eq!(
            transaction.reserve(0, u64::from(limits.max_action_bytes) + 1),
            Err(PrivacyBudgetError::ActionTooLarge {
                bytes: u64::from(limits.max_action_bytes) + 1,
                max: limits.max_action_bytes,
            })
        );
        assert_eq!(transaction.actions(), 0);
        assert_eq!(transaction.bytes(), 0);
    }
    #[test]
    fn one_transaction_cannot_smuggle_a_second_privacy_action() {
        let mut block = PrivacyBlockBudgetV1::default();
        let mut transaction = block.begin_transaction();
        transaction.reserve(0, 1).expect("first action");
        assert!(matches!(
            transaction.reserve(1, 1),
            Err(PrivacyBudgetError::TransactionActionsExceeded { .. })
        ));
        assert_eq!(transaction.actions(), 1);
        assert_eq!(transaction.bytes(), 1);
    }
}
