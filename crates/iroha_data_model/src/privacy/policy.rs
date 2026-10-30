//! Consensus privacy limits and governed future-block update validation.

use super::*;

impl PrivacyConsensusLimitsV1 {
    /// Return the approved first-release Taira profile.
    #[must_use]
    pub const fn taira_default() -> Self {
        Self {
            max_actions_per_transaction: TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1,
            max_actions_per_block: TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1,
            max_proof_bytes_per_action: TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
            max_action_bytes: TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
            max_privacy_bytes_per_transaction: TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1,
            max_privacy_bytes_per_block: TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1,
            max_statement_and_encrypted_output_bytes_per_transaction:
                TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1,
            max_nullifiers_per_action: TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1,
            max_commitments_per_action: TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1,
            retained_root_count: TAIRA_PRIVACY_RETAINED_ROOT_COUNT_V1,
        }
    }
    /// Validate non-zero, hard-ceiling, and cross-field ordering invariants.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyConsensusLimitsValidationError`] for the first invalid
    /// field or relationship in deterministic field order.
    pub fn validate(&self) -> Result<(), PrivacyConsensusLimitsValidationError> {
        let fields = [
            (
                PrivacyLimitFieldV1::ActionsPerTransaction,
                self.max_actions_per_transaction,
                TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::ActionsPerBlock,
                self.max_actions_per_block,
                TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1,
            ),
            (
                PrivacyLimitFieldV1::ProofBytesPerAction,
                self.max_proof_bytes_per_action,
                TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::ActionBytes,
                self.max_action_bytes,
                TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
                self.max_privacy_bytes_per_transaction,
                TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerBlock,
                self.max_privacy_bytes_per_block,
                TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1,
            ),
            (
                PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
                self.max_statement_and_encrypted_output_bytes_per_transaction,
                TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::NullifiersPerAction,
                self.max_nullifiers_per_action,
                TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::CommitmentsPerAction,
                self.max_commitments_per_action,
                TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::RetainedRootCount,
                self.retained_root_count,
                TAIRA_PRIVACY_RETAINED_ROOT_COUNT_V1,
            ),
        ];
        for (field, value, hard_max) in fields {
            if value == 0 {
                return Err(PrivacyConsensusLimitsValidationError::Zero { field });
            }
            if value > hard_max {
                return Err(PrivacyConsensusLimitsValidationError::ExceedsHardMaximum {
                    field,
                    value,
                    hard_max,
                });
            }
        }
        validate_limit_order(
            PrivacyLimitFieldV1::ActionsPerTransaction,
            self.max_actions_per_transaction,
            PrivacyLimitFieldV1::ActionsPerBlock,
            self.max_actions_per_block,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::ProofBytesPerAction,
            self.max_proof_bytes_per_action,
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
            PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
            self.max_privacy_bytes_per_transaction,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
            self.max_privacy_bytes_per_transaction,
            PrivacyLimitFieldV1::PrivacyBytesPerBlock,
            self.max_privacy_bytes_per_block,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
            self.max_statement_and_encrypted_output_bytes_per_transaction,
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
        )?;
        Ok(())
    }
    /// Validate `next` as an ordinary update within canonical hard bounds of this policy.
    ///
    /// # Errors
    ///
    /// Rejects invalid profiles and a no-op update; authorized changes may increase or decrease limits.
    pub fn validate_update_to(
        &self,
        next: &Self,
    ) -> Result<(), PrivacyConsensusLimitsUpdateErrorV1> {
        self.validate()
            .map_err(PrivacyConsensusLimitsUpdateErrorV1::InvalidCurrent)?;
        next.validate()
            .map_err(PrivacyConsensusLimitsUpdateErrorV1::InvalidNext)?;
        if self == next {
            return Err(PrivacyConsensusLimitsUpdateErrorV1::NoChange);
        }
        Ok(())
    }
}

impl Default for PrivacyConsensusLimitsV1 {
    fn default() -> Self {
        Self::taira_default()
    }
}

fn validate_limit_order(
    smaller_field: PrivacyLimitFieldV1,
    smaller_value: u32,
    larger_field: PrivacyLimitFieldV1,
    larger_value: u32,
) -> Result<(), PrivacyConsensusLimitsValidationError> {
    if smaller_value > larger_value {
        return Err(PrivacyConsensusLimitsValidationError::InconsistentOrder {
            smaller_field,
            smaller_value,
            larger_field,
            larger_value,
        });
    }
    Ok(())
}

impl PrivacyConsensusPolicyUpdateV1 {
    /// Validate schedule timing and canonical resource bounds.
    ///
    /// # Errors
    ///
    /// Rejects zero/overflowing heights, a non-future effective height, an invalid
    /// successor or a no-op.
    pub fn validate_against(
        &self,
        current_limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyPolicyValidationErrorV1> {
        validate_privacy_policy_schedule_heights_v1(
            self.scheduled_at_height,
            self.effective_at_height,
        )?;
        current_limits
            .validate_update_to(&self.next_limits)
            .map_err(PrivacyPolicyValidationErrorV1::ConsensusUpdate)
    }
}

impl PrivacyConsensusPolicyV1 {
    /// Construct the first-release Taira policy with no pending change.
    #[must_use]
    pub const fn taira_default() -> Self {
        Self {
            current_limits: PrivacyConsensusLimitsV1::taira_default(),
            pending_update: None,
        }
    }
    /// Validate the complete persisted policy independent of chain height.
    ///
    /// # Errors
    ///
    /// Rejects invalid current limits or a malformed pending update.
    pub fn validate(&self) -> Result<(), PrivacyPolicyValidationErrorV1> {
        self.current_limits
            .validate()
            .map_err(PrivacyPolicyValidationErrorV1::InvalidCurrentLimits)?;
        if let Some(pending) = self.pending_update {
            pending.validate_against(&self.current_limits)?;
        }
        Ok(())
    }
    /// Validate a restored policy against the latest committed block height.
    ///
    /// A pending transition at height `E` is valid in a snapshot committed at
    /// `E - 1`, and invalid in a snapshot already committed at `E`.
    ///
    /// # Errors
    ///
    /// Rejects an intrinsically invalid policy or a missed/due transition.
    pub fn validate_at_committed_height(
        &self,
        committed_height: u64,
    ) -> Result<(), PrivacyPolicyValidationErrorV1> {
        self.validate()?;
        if let Some(pending) = self.pending_update {
            if pending.scheduled_at_height > committed_height {
                return Err(
                    PrivacyPolicyValidationErrorV1::PendingScheduledAfterCommitted {
                        scheduled_at_height: pending.scheduled_at_height,
                        committed_height,
                    },
                );
            }
            if pending.effective_at_height <= committed_height {
                return Err(PrivacyPolicyValidationErrorV1::PendingNotFuture {
                    effective_at_height: pending.effective_at_height,
                    committed_height,
                });
            }
        }
        Ok(())
    }
    /// Root-retention cap enforced while admitting new roots.
    ///
    /// While a update is pending, new histories must already satisfy the
    /// lower limit so the effective-height transition is deterministic.
    #[must_use]
    pub const fn admission_retained_root_count(&self) -> u32 {
        match self.pending_update {
            Some(pending)
                if pending.next_limits.retained_root_count
                    < self.current_limits.retained_root_count =>
            {
                pending.next_limits.retained_root_count
            }
            _ => self.current_limits.retained_root_count,
        }
    }
}

impl Default for PrivacyConsensusPolicyV1 {
    fn default() -> Self {
        Self::taira_default()
    }
}

fn validate_privacy_policy_schedule_heights_v1(
    scheduled_at_height: u64,
    effective_at_height: u64,
) -> Result<(), PrivacyPolicyValidationErrorV1> {
    if scheduled_at_height == 0 {
        return Err(PrivacyPolicyValidationErrorV1::ZeroScheduledHeight);
    }
    let next_height = scheduled_at_height
        .checked_add(1)
        .ok_or(PrivacyPolicyValidationErrorV1::HeightOverflow)?;
    if effective_at_height < next_height {
        return Err(PrivacyPolicyValidationErrorV1::EffectiveNotLater {
            scheduled_at_height,
            effective_at_height,
        });
    }
    Ok(())
}

impl PrivacyProtocolLimitsUpdateV1 {
    /// Validate schedule timing, protocol identity and canonical hard bounds.
    ///
    /// # Errors
    ///
    /// Rejects invalid schedule heights, a protocol mismatch, invalid limits or a no-op.
    pub fn validate_against(
        &self,
        current_limits: &PrivacyProtocolLimitsV1,
    ) -> Result<(), PrivacyProtocolLimitsUpdateValidationErrorV1> {
        validate_privacy_policy_schedule_heights_v1(
            self.scheduled_at_height,
            self.effective_at_height,
        )
        .map_err(PrivacyProtocolLimitsUpdateValidationErrorV1::Schedule)?;
        current_limits
            .validate()
            .map_err(PrivacyProtocolLimitsUpdateValidationErrorV1::Limits)?;
        self.next_limits
            .validate()
            .map_err(PrivacyProtocolLimitsUpdateValidationErrorV1::Limits)?;
        if self.next_limits.protocol_id() != current_limits.protocol_id() {
            return Err(PrivacyProtocolLimitsUpdateValidationErrorV1::Limits(
                PrivacyProtocolLimitsValidationError::ProtocolMismatch {
                    actual: self.next_limits.protocol_id(),
                    ceiling: current_limits.protocol_id(),
                },
            ));
        }
        if self.next_limits == *current_limits {
            return Err(PrivacyProtocolLimitsUpdateValidationErrorV1::NoChange);
        }
        Ok(())
    }
}

impl PrivacyProtocolPolicyV1 {
    /// Build an ordinary policy at canonical limits with no scheduled update.
    /// Persisting this value is optional and does not make an implementation available.
    #[must_use]
    pub const fn canonical_default_v1(protocol_id: PrivacyProtocolIdV1) -> Self {
        Self {
            protocol_id,
            current_limits: PrivacyProtocolLimitsV1::canonical_default_v1(protocol_id),
            pending_update: None,
        }
    }

    /// Validate the protocol identity, hard bounds and optional successor.
    /// # Errors
    /// Rejects wrong protocol tags, invalid limits, invalid schedules and no-op updates.
    pub fn validate(&self) -> Result<(), PrivacyProtocolPolicyValidationErrorV1> {
        if self.current_limits.protocol_id() != self.protocol_id {
            return Err(PrivacyProtocolPolicyValidationErrorV1::ProtocolMismatch {
                protocol_id: self.protocol_id,
                limits_protocol: self.current_limits.protocol_id(),
            });
        }
        self.current_limits
            .validate()
            .map_err(PrivacyProtocolPolicyValidationErrorV1::Limits)?;
        if let Some(pending) = self.pending_update {
            pending
                .validate_against(&self.current_limits)
                .map_err(PrivacyProtocolPolicyValidationErrorV1::PendingUpdate)?;
        }
        Ok(())
    }
    /// Validate a committed policy, including exact future-block scheduling.
    /// # Errors
    /// Rejects a malformed policy, future claimed admission or an unapplied due update.
    pub fn validate_at_committed_height(
        &self,
        committed_height: u64,
    ) -> Result<(), PrivacyProtocolPolicyValidationErrorV1> {
        self.validate()?;
        if let Some(pending) = self.pending_update {
            if pending.scheduled_at_height > committed_height {
                return Err(PrivacyProtocolPolicyValidationErrorV1::Schedule(
                    PrivacyPolicyValidationErrorV1::PendingScheduledAfterCommitted {
                        scheduled_at_height: pending.scheduled_at_height,
                        committed_height,
                    },
                ));
            }
            if pending.effective_at_height <= committed_height {
                return Err(PrivacyProtocolPolicyValidationErrorV1::Schedule(
                    PrivacyPolicyValidationErrorV1::PendingNotFuture {
                        effective_at_height: pending.effective_at_height,
                        committed_height,
                    },
                ));
            }
        }
        Ok(())
    }
}
