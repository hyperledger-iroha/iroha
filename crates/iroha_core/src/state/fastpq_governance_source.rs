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
    // Reject oversized variable identities before cloning them into a frame.
    let identity_bytes = [
        norito::canonical_frame_len(owner),
        norito::canonical_frame_len(&custody.bond_escrow_account),
        norito::canonical_frame_len(&custody.asset_definition_id),
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
            from_account: custody.bond_escrow_account.clone(),
            to_account: owner.clone(),
            asset_definition: custody.asset_definition_id.clone(),
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

/// Validate a complete authoritative retained corpus and an optional replacement.
/// The cap applies globally across all referenda and expiry heights.
pub(super) fn validate_retained<'a>(
    profile: FastpqSourcePolicyV1,
    groups: impl IntoIterator<Item = (&'a String, &'a GovernanceLocksForReferendum)>,
    replacement: Option<(&str, &AccountId, &GovernanceLockCustody)>,
    rekey: Option<(&AccountId, &AccountId)>,
) -> Result<(), String> {
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
    Ok(())
}

impl StateTransaction<'_, '_> {
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
            )?;
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
            )?;
        }
        Ok(())
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
            validate_retained(
                parameters.get().block().fastpq_source(),
                locks.iter(),
                None,
                None,
            )?;
        }
        {
            let parameters = self.parameters.block_and_revert();
            let locks = self.governance_locks.block_and_revert();
            validate_retained(
                parameters.get().block().fastpq_source(),
                locks.iter(),
                None,
                None,
            )?;
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
