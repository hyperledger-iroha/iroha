//! Resolve compact credential sources against the original authenticated execution graph.
//!
//! This is a read boundary, not a second registration path. The caller first acquires an
//! [`AuthenticatedExecutionBlock`] from the native history reader, retaining its original
//! decoder and finality custody. Missing history or a local read refusal stays with that
//! reader; neither can become an invalid-snapshot verdict here. Resolution borrows the full
//! proof from the exact successful signed input rather than substituting its digest.
//!
//! TODO(design §12.3): integrate this read with bounded history retrieval after restart and
//! DA sidecarring. Structural snapshot restoration does not acquire local history.

use iroha_data_model::{
    governance::{
        anonymous_ballot::{
            BallotBodyContextV1, ParliamentCredentialRecordV1, RegisterParliamentCredentialV1,
        },
        types::AssignmentId,
    },
    transaction::{Executable, TransactionEntrypoint},
};

use crate::sumeragi::certified_chain::AuthenticatedExecutionBlock;

/// A compact credential record does not name its claimed successful registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CredentialHistoryErrorV1 {
    /// The authenticated block or canonical input coordinate differs from the locator.
    #[error("credential source does not name this authenticated input")]
    Source,
    /// The located input is not a direct signed input with exactly one registration.
    #[error("credential source is not one direct signed registration")]
    Instruction,
    /// The exact input has no successful authenticated Network output.
    #[error("credential source registration was not applied")]
    NotApplied,
    /// Network, attempt, election, signer-derived assignment, commitment or authorization
    /// differs from the credential's expected context.
    #[error("credential source registration differs from its stored credential")]
    Binding,
}

/// Borrow the full registration carried by `record`'s successful authenticated input.
///
/// `context`, `assignment` and `record` must come from the consumer's authoritative State.
/// The returned proof remains borrowed from `original`; resolving it does not reverify the
/// registration relation, authorize a new registration, or grant ballot qualification. A
/// proof consumer verifies the actual returned proof under the complete body context.
///
/// The input index is the complete network projection, including authority-free and sealed
/// inputs. A signed-transaction iterator's position is never used as this coordinate.
///
/// # Errors
/// [`CredentialHistoryErrorV1`] if any original source, successful outcome or binding check
/// fails. A native-history read error must be handled before this function is called.
pub fn resolve_credential_registration_v1<'a>(
    original: &'a AuthenticatedExecutionBlock,
    context: &BallotBodyContextV1,
    assignment: AssignmentId,
    record: &ParliamentCredentialRecordV1,
) -> Result<&'a RegisterParliamentCredentialV1, CredentialHistoryErrorV1> {
    use CredentialHistoryErrorV1 as Error;

    let source = record.source;
    if source.height == 0
        || source.height != original.committed().height()
        || source.instruction_index != 0
    {
        return Err(Error::Source);
    }
    let index = u32::try_from(source.input_index).map_err(|_| Error::Source)?;
    let index_usize = usize::try_from(index).map_err(|_| Error::Source)?;
    let block = original.block();
    let input = block
        .network_entrypoint_at(index_usize)
        .ok_or(Error::Source)?;
    if input.hash() != source.entrypoint_hash {
        return Err(Error::Source);
    }
    let TransactionEntrypoint::External(signed) = input else {
        return Err(Error::Instruction);
    };
    let (_, output) = block.network_output_at(index).ok_or(Error::NotApplied)?;
    if output.result.is_err() {
        return Err(Error::NotApplied);
    }
    let Executable::Instructions(instructions) = signed.instructions() else {
        return Err(Error::Instruction);
    };
    let [instruction] = instructions.as_ref() else {
        return Err(Error::Instruction);
    };
    let registration = instruction
        .as_any()
        .downcast_ref::<RegisterParliamentCredentialV1>()
        .ok_or(Error::Instruction)?;
    if signed.network_id() != Some(&context.network_id)
        || registration.governance_attempt_id != context.governance_attempt_id
        || registration.election_attempt_id != context.election_attempt_id
        || AssignmentId::derive_v1(context.election_attempt_id, signed.authority()) != assignment
        || registration.registration.commitment != record.commitment
        || registration.registration.authorization != record.authorization
    {
        return Err(Error::Binding);
    }
    Ok(registration)
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::assert_registration_history;
