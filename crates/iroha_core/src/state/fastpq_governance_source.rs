//! Retained governance source allowance before custody changes and on restore.
//!
//! Every retained record occupies the global corpus, including zero/slashed and
//! failed overdue releases. Variable controller identities are measured from the
//! actual record; the supported source profile is not a global controller bound.

use mv::storage::StorageReadOnly;

use super::{
    AccountId, GovernanceLockCustody, GovernanceLocksForReferendum, Hash, StateTransaction,
};
use crate::fastpq::{
    FastpqSourceStatementBuildLimits,
    source_prefix_lengths::entry::measure_fastpq_source_entry_frame_usage,
};
use iroha_data_model::{
    fastpq::{TransferDeltaTranscript, TransferSmtWitness, TransferTranscript},
    parameter::{FastpqSourceLimitsV1, FastpqSourcePolicyV1},
};
use iroha_primitives::{
    bigint::BigInt,
    numeric::{MAX_DECIMAL_SCALE, MAX_MANTISSA_BYTES, Numeric, Quantity},
};

fn limits(value: FastpqSourceLimitsV1) -> Result<FastpqSourceStatementBuildLimits, String> {
    let length = |value: u64| {
        usize::try_from(value)
            .map_err(|_| "FASTPQ mandatory source capacity exceeds host length width".to_owned())
    };
    Ok(FastpqSourceStatementBuildLimits {
        max_executed_entries: value.max_executed_entries,
        max_transcripts: length(u64::from(value.max_transcripts))?,
        max_deltas: length(u64::from(value.max_deltas))?,
        max_input_transcript_bytes: length(value.max_input_transcript_bytes)?,
        max_statement_bytes: length(value.max_statement_bytes)?,
        max_total_statement_bytes: length(value.max_total_statement_bytes)?,
    })
}

/// Measure one future release using the widest positive protocol quantity shape.
/// This is shape-only reservation, not an executable transfer or proof statement.
fn future_release(
    owner: &AccountId,
    custody: &GovernanceLockCustody,
    ceiling: FastpqSourceLimitsV1,
) -> Result<FastpqSourceLimitsV1, String> {
    future_transfer(
        &custody.bond_escrow_account,
        owner,
        &custody.asset_definition_id,
        ceiling,
    )
}

fn future_transfer(
    from: &AccountId,
    to: &AccountId,
    asset: &iroha_data_model::asset::AssetDefinitionId,
    ceiling: FastpqSourceLimitsV1,
) -> Result<FastpqSourceLimitsV1, String> {
    // Reject oversized variable identities before cloning them into a frame.
    let identity_bytes = [
        norito::canonical_frame_len(to),
        norito::canonical_frame_len(from),
        norito::canonical_frame_len(asset),
    ]
    .into_iter()
    .try_fold(0_u64, |sum, length| {
        let length = u64::try_from(length.map_err(|error| error.to_string())?)
            .map_err(|_| "mandatory identity length exceeds u64")?;
        sum.checked_add(length)
            .ok_or_else(|| "mandatory identity lengths overflow".to_owned())
    })?;
    if identity_bytes > ceiling.max_input_transcript_bytes {
        return Err("governance release identities exceed mandatory source allowance".into());
    }
    let mut mantissa = [0xff; MAX_MANTISSA_BYTES];
    *mantissa.last_mut().expect("quantity mantissa is nonempty") = 0x7f;
    let quantity = Quantity::try_from_numeric(
        Numeric::try_new(
            BigInt::from_twos_bytes(&mantissa).map_err(|error| error.to_string())?,
            MAX_DECIMAL_SCALE,
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let hash = Hash::new(b"iroha:fastpq:future-governance-release:shape:v1");
    let transcript = TransferTranscript {
        batch_hash: hash,
        authority_digest: hash,
        poseidon_preimage_digest: Some(hash),
        deltas: vec![TransferDeltaTranscript {
            from_account: from.clone(),
            to_account: to.clone(),
            asset_definition: asset.clone(),
            amount: quantity.clone(),
            from_balance_before: quantity.clone(),
            from_balance_after: quantity.clone(),
            to_balance_before: quantity.clone(),
            to_balance_after: quantity,
            // Runtime release constructors emit empty original paths. Later
            // touched-tree materialization does not rewrite these originals.
            from_smt_witness: TransferSmtWitness::default(),
            to_smt_witness: TransferSmtWitness::default(),
        }],
    };
    let measured = measure_fastpq_source_entry_frame_usage(hash, [&transcript], limits(ceiling)?)
        .map_err(|error| {
        format!("retained governance release exceeds mandatory source allowance: {error}")
    })?;
    Ok(FastpqSourceLimitsV1 {
        max_executed_entries: 1,
        max_transcripts: 1,
        max_deltas: 1,
        max_input_transcript_bytes: measured
            .input_transcript_bytes
            .try_into()
            .map_err(|_| "mandatory source input length exceeds u64")?,
        max_statement_bytes: measured
            .max_statement_bytes
            .try_into()
            .map_err(|_| "mandatory source statement length exceeds u64")?,
        max_total_statement_bytes: measured
            .total_statement_bytes
            .try_into()
            .map_err(|_| "mandatory source statement total exceeds u64")?,
    })
}

/// Combined corpus accounting remains one owner across governance and SNS.
#[derive(Debug)]
pub(super) struct RetainedCorpus {
    profile: FastpqSourcePolicyV1,
    count: u32,
    total: FastpqSourceLimitsV1,
}

impl RetainedCorpus {
    pub(super) fn with_sns(
        mut self,
        storage: &impl StorageReadOnly<iroha_model_base::state_path::StatePath, Vec<u8>>,
        replacement: Option<&iroha_data_model::alias_setup::AliasAutoRenewStateV1>,
    ) -> Result<FastpqSourceLimitsV1, crate::sns::SnsError> {
        if cfg!(all(test, sumeragi_core_mutation = "HC34")) {
            return Ok(FastpqSourceLimitsV1::ZERO);
        }
        let mut sns_total = FastpqSourceLimitsV1::ZERO;
        let ceiling = self
            .profile
            .mandatory
            .reservation()
            .map_err(crate::sns::SnsError::Internal)?;
        crate::sns::visit_retained_auto_renew_obligations(
            storage,
            replacement,
            |owner, collector, asset| {
                self.count = self.count.checked_add(1).ok_or_else(|| {
                    crate::sns::SnsError::Conflict(
                        "retained mandatory obligation count exceeds u32".into(),
                    )
                })?;
                if self.count > self.profile.mandatory.max_retained_obligations {
                    return Err(crate::sns::SnsError::Conflict(
                        "global retained governance/SNS count exceeds mandatory source allowance"
                            .into(),
                    ));
                }
                let shape = future_transfer(
                    owner,
                    collector,
                    asset,
                    self.profile.mandatory.per_obligation,
                )
                .map_err(crate::sns::SnsError::Conflict)?;
                sns_total = sns_total
                    .checked_add_entries(shape)
                    .map_err(crate::sns::SnsError::Conflict)?;
                self.total = self
                    .total
                    .checked_add_entries(shape)
                    .map_err(crate::sns::SnsError::Conflict)?;
                if !self.total.fits_within(ceiling) {
                    return Err(crate::sns::SnsError::Conflict("retained governance/SNS transfers exceed their global mandatory source pool".into()));
                }
                Ok(())
            },
        )?;
        Ok(sns_total)
    }
}

/// Validate a complete authoritative retained corpus and an optional replacement.
/// The cap applies globally across all referenda and expiry heights.
pub(super) fn validate_retained<'a>(
    profile: FastpqSourcePolicyV1,
    groups: impl IntoIterator<Item = (&'a String, &'a GovernanceLocksForReferendum)>,
    replacement: Option<(&str, &AccountId, &GovernanceLockCustody)>,
    rekey: Option<(&AccountId, &AccountId)>,
) -> Result<RetainedCorpus, String> {
    let ceiling = profile.mandatory.reservation()?;
    let mut total = FastpqSourceLimitsV1::ZERO;
    let mut count = 0_u32;
    let mut append = |owner: &AccountId, custody: &GovernanceLockCustody| -> Result<(), String> {
        count = count
            .checked_add(1)
            .ok_or("retained governance lock count exceeds u32")?;
        if count > profile.mandatory.max_retained_obligations {
            return Err(
                "global retained governance lock count exceeds mandatory source allowance".into(),
            );
        }
        let owner = rekey
            .filter(|(old, _)| *old == owner)
            .map_or(owner, |(_, new)| new);
        total = total.checked_add_entries(future_release(
            owner,
            custody,
            profile.mandatory.per_obligation,
        )?)?;
        if !total.fits_within(ceiling) {
            return Err(
                "retained governance releases exceed their global mandatory source pool".into(),
            );
        }
        Ok(())
    };
    for (referendum, locks) in groups {
        if locks.locks.is_empty() {
            return Err("retained governance corpus contains an empty referendum group".into());
        }
        if let Some((old, new)) = rekey {
            if old != new && locks.locks.contains_key(old) && locks.locks.contains_key(new) {
                return Err("governance rekey would merge distinct retained obligations".into());
            }
        }
        for (owner, record) in &locks.locks {
            if &record.owner != owner {
                return Err(format!(
                    "governance lock for referendum {referendum} is stored under an owner different from its record"
                ));
            }
            if replacement.is_some_and(|(id, next, _)| id == referendum && next == owner) {
                continue;
            }
            // Zero amount and failed overdue records are deliberately retained.
            append(owner, &record.custody)?;
        }
    }
    if let Some((_, owner, custody)) = replacement {
        append(owner, custody)?;
    }
    Ok(RetainedCorpus {
        profile,
        count,
        total,
    })
}

impl StateTransaction<'_, '_> {
    /// Admit every retained future charge before changing an SNS configuration.
    pub(crate) fn validate_fastpq_sns_state(
        &self,
        replacement: &iroha_data_model::alias_setup::AliasAutoRenewStateV1,
    ) -> Result<(), crate::sns::SnsError> {
        let current = self.world.parameters.get().block();
        for profile in [self.fastpq_source_policy.0, current.fastpq_source()] {
            let sns = validate_retained(profile, self.world.governance_locks.iter(), None, None)
                .map_err(crate::sns::SnsError::Conflict)?
                .with_sns(&self.world.smart_contract_state, Some(replacement))?;
            if let Some(consumed) = self
                .fastpq_source_quota
                .pending_time_consumed_usage()
                .map_err(crate::sns::SnsError::Conflict)?
            {
                let used = FastpqSourceLimitsV1 {
                    max_executed_entries: consumed.executed_entries.try_into().map_err(|_| {
                        crate::sns::SnsError::Conflict(
                            "consumed mandatory entries exceed u32".into(),
                        )
                    })?,
                    max_transcripts: consumed.transcripts.try_into().map_err(|_| {
                        crate::sns::SnsError::Conflict(
                            "consumed mandatory transcripts exceed u32".into(),
                        )
                    })?,
                    max_deltas: consumed.deltas.try_into().map_err(|_| {
                        crate::sns::SnsError::Conflict(
                            "consumed mandatory deltas exceed u32".into(),
                        )
                    })?,
                    max_input_transcript_bytes: consumed.input_transcript_bytes,
                    max_statement_bytes: consumed.max_statement_bytes,
                    max_total_statement_bytes: consumed.total_statement_bytes,
                };
                if !used
                    .checked_add_entries(sns)
                    .map_err(crate::sns::SnsError::Conflict)?
                    .fits_within(
                        profile
                            .mandatory
                            .reservation()
                            .map_err(crate::sns::SnsError::Conflict)?,
                    )
                {
                    return Err(crate::sns::SnsError::Conflict("consumed mandatory sources and pending SNS Time charges exceed the original carrier allowance".into()));
                }
            }
        }
        Ok(())
    }

    /// Preflight the complete prospective retained corpus before escrow movement.
    pub(crate) fn validate_fastpq_governance_lock(
        &self,
        referendum: &str,
        owner: &AccountId,
        custody: &GovernanceLockCustody,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<String>> {
        let current = self.world.parameters.get().block();
        // Both this block's frozen profile and the installed next-block profile
        // must cover new obligations created during genesis.
        for profile in [self.fastpq_source_policy.0, current.fastpq_source()] {
            validate_retained(
                profile,
                self.world.governance_locks.iter(),
                Some((referendum, owner, custody)),
                None,
            )?
            .with_sns(&self.world.smart_contract_state, None)
            .map_err(sns_attempt_error)?;
        }
        Ok(())
    }

    /// Preflight owner identity growth before rekey changes any State field.
    pub(crate) fn validate_fastpq_governance_rekey(
        &self,
        old: &AccountId,
        new: &AccountId,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<String>> {
        let current = self.world.parameters.get().block();
        for profile in [self.fastpq_source_policy.0, current.fastpq_source()] {
            validate_retained(
                profile,
                self.world.governance_locks.iter(),
                None,
                Some((old, new)),
            )?
            .with_sns(&self.world.smart_contract_state, None)
            .map_err(sns_attempt_error)?;
        }
        Ok(())
    }
}

pub(super) fn sns_attempt_error(
    error: crate::sns::SnsError,
) -> crate::execution_attempt::ExecutionAttemptError<String> {
    match error {
        crate::sns::SnsError::Deferred(reason) => {
            crate::execution_attempt::ExecutionAttemptError::Deferred(reason)
        }
        error => crate::execution_attempt::ExecutionAttemptError::Rejected(error.to_string()),
    }
}

impl super::World {
    /// Validate both retained cuts before startup/restore can publish derived owners.
    pub(super) fn validate_retained_mandatory_sources(
        &self,
    ) -> Result<(), crate::execution_attempt::ExecutionAttemptError<String>> {
        {
            let parameters = self.parameters.view();
            let locks = self.governance_locks.view();
            let records = self.smart_contract_state.view();
            validate_retained(
                parameters.get().block().fastpq_source(),
                locks.iter(),
                None,
                None,
            )?
            .with_sns(&records, None)
            .map_err(sns_attempt_error)?;
        }
        {
            let parameters = self.parameters.block_and_revert();
            let locks = self.governance_locks.block_and_revert();
            let records = self.smart_contract_state.block_and_revert();
            validate_retained(
                parameters.get().block().fastpq_source(),
                locks.iter(),
                None,
                None,
            )?
            .with_sns(&records, None)
            .map_err(sns_attempt_error)?;
        }
        Ok(())
    }
}

impl StateTransaction<'_, '_> {
    pub(crate) fn mandatory_source_instruction_error(
        &mut self,
        error: crate::execution_attempt::ExecutionAttemptError<String>,
    ) -> iroha_data_model::isi::error::InstructionExecutionError {
        let message = match error {
            crate::execution_attempt::ExecutionAttemptError::Deferred(reason) => {
                let _ = self.defer_execution(reason);
                "local mandatory source admission did not complete".into()
            }
            crate::execution_attempt::ExecutionAttemptError::Rejected(message) => message,
        };
        iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(message.into())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod runtime_tests;
