//! Signer-once binding custody and the original State-funded canonical External frame.
//!
//! No failure exposes a preparation, replaceable signed envelope, or renewed deadline. A local
//! refusal retains the exact original graph. TODO: admit serializer-internal scratch and the
//! ordinary preparation/signing graphs at their producers. The allocated-pin variant additionally
//! retains its explicitly prepaid graph; the separate frame charge owns only output backing.

use super::*;
use crate::execution_attempt::{ExecutionAttemptError, ExecutionDeferred};
use iroha_allocation::{ChargedBuffer, ChargedBufferError};
use iroha_data_model::transaction::signed::pin_allocation::AllocatedPinTransactionV1;
use ivm::error::ExecutionDeferral;
use std::{fmt, io};

/// Original binding refusal, separate from a completed native Check rejection.
#[derive(Debug)]
pub enum NativeCheckBindingErrorV1<E> {
    /// The unchanged attempt failed its purpose, signed profile or original lifetime.
    Rejected(E),
    /// Original State admission refusal, retaining its real release owner if present.
    Deferred(ExecutionDeferred),
    /// Exact serializer error and its classification under the surviving caller scope.
    Codec {
        /// Unchanged error returned by the canonical serializer.
        original: norito::Error,
        /// Local scratch/allocator refusal; no allocation release owner is invented.
        local: Option<ExecutionDeferral>,
    },
}
impl<E> NativeCheckBindingErrorV1<E> {
    /// Whether retrying this same signed owner may complete an unfinished local attempt.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(self, Self::Deferred(_) | Self::Codec { local: Some(_), .. })
    }
}
impl<E: From<Error>> From<Error> for NativeCheckBindingErrorV1<E> {
    fn from(error: Error) -> Self {
        Self::Rejected(error.into())
    }
}
impl<E> From<norito::Error> for NativeCheckBindingErrorV1<E> {
    fn from(original: norito::Error) -> Self {
        // Classify while the original caller's scope is still active, retaining the exact error.
        let local = match &original {
            norito::Error::AllocationFailed { .. } => {
                Some(ExecutionDeferral::AllocationUnavailable)
            }
            _ if norito::core::decode_error_matches_active_limits(&original) => {
                Some(ExecutionDeferral::ActiveMemoryCapacity)
            }
            _ => None,
        };
        Self::Codec { original, local }
    }
}
impl<E> From<ChargedBufferError> for NativeCheckBindingErrorV1<E> {
    fn from(error: ChargedBufferError) -> Self {
        match error {
            ChargedBufferError::Admission(original) => Self::Deferred(original.into()),
            ChargedBufferError::Allocator { .. } => {
                Self::Deferred(ExecutionDeferral::AllocationUnavailable.into())
            }
        }
    }
}
impl<E: From<Error>> From<ExecutionAttemptError<Error>> for NativeCheckBindingErrorV1<E> {
    fn from(error: ExecutionAttemptError<Error>) -> Self {
        match error {
            ExecutionAttemptError::Rejected(error) => error.into(),
            ExecutionAttemptError::Deferred(original) => Self::Deferred(original),
        }
    }
}

pub(crate) struct BindingScope<'a> {
    pub(crate) state: &'a State,
    pub(crate) round: &'a mut NativeCheckRoundV1,
    pub(crate) instruction: NativeCustodyCheckRefV1<'a>,
    pub(crate) chain_id: &'a str,
    pub(crate) network_id: [u8; 32],
    pub(crate) authority: &'a AccountId,
    pub(crate) floor: NativeCheckFloorV1,
}

/// Opaque continuation marker; only this owner can retry its already-spent round.
pub(crate) struct SignedCheckAttempt {
    entry: SignedCheckOwner,
    started: bool,
}
/// Immutable signed owner; allocated pin custody retains every graph charge through native
/// finality/current-row verification. Other purpose owners retain their existing moved graphs.
pub(crate) enum SignedCheckOwner {
    Ordinary(TransactionEntrypoint),
    AllocatedPin(AllocatedPinTransactionV1),
}
impl SignedCheckOwner {
    fn entrypoint(&self) -> &TransactionEntrypoint {
        match self {
            Self::Ordinary(entry) => entry,
            Self::AllocatedPin(entry) => entry.entrypoint(),
        }
    }
    pub(crate) fn signed_transaction(&self) -> &SignedTransaction {
        let TransactionEntrypoint::External(signed) = self.entrypoint() else {
            unreachable!("private signed attempt")
        };
        signed
    }
    fn matches_pool(&self, state: &State) -> bool {
        match self {
            Self::Ordinary(_) => true,
            Self::AllocatedPin(entry) => entry.belongs_to(&state.ivm_execution_budget()),
        }
    }
}
impl SignedCheckAttempt {
    #[cfg(test)]
    pub(crate) fn signed_transaction(&self) -> &SignedTransaction {
        self.entry.signed_transaction()
    }
    pub(crate) fn new(signed: SignedTransaction) -> Self {
        Self {
            entry: SignedCheckOwner::Ordinary(TransactionEntrypoint::External(signed)),
            started: false,
        }
    }
    pub(crate) fn from_allocated_pin(signed: AllocatedPinTransactionV1) -> Self {
        Self {
            entry: SignedCheckOwner::AllocatedPin(signed),
            started: false,
        }
    }
}

pub(crate) struct BindingFailure<P, E> {
    pub(crate) prepared: P,
    pub(crate) signed: SignedCheckAttempt,
    pub(crate) error: NativeCheckBindingErrorV1<E>,
}
impl<P, E: fmt::Debug> fmt::Debug for BindingFailure<P, E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BindingFailure")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

/// Exact ASCII chain label; retaining it never clones an uncharged String.
pub(super) struct BoundChainId {
    bytes: [u8; iroha_primitives::chain_id::MAX_CHAIN_ID_BYTES],
    len: usize,
}
impl BoundChainId {
    fn new(value: &str) -> Result<Self, Error> {
        iroha_primitives::chain_id::validate_chain_id(value).map_err(|_| Error::Transaction)?;
        let mut bytes = [0; iroha_primitives::chain_id::MAX_CHAIN_ID_BYTES];
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Ok(Self {
            bytes,
            len: value.len(),
        })
    }
    pub(super) fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub(crate) fn bind_signed_check_v1<P, E: From<Error>>(
    mut prepared: P,
    mut attempt: SignedCheckAttempt,
    scope: for<'a> fn(&'a mut P) -> Result<BindingScope<'a>, Error>,
) -> Result<(P, BoundNativeCheckV1), BindingFailure<P, E>> {
    let result: Result<_, NativeCheckBindingErrorV1<E>> = (|| {
        let BindingScope {
            state,
            round,
            instruction,
            chain_id,
            network_id,
            authority,
            floor,
        } = scope(&mut prepared)?;
        if !attempt.entry.matches_pool(state) {
            return Err(Error::Transaction.into());
        }
        if !attempt.started {
            if round.bound {
                return Err(Error::Invalid.into());
            }
            round.bound = true;
            attempt.started = true;
        }
        round.ensure_live()?;
        floor.validate()?;
        if let NativeCustodyCheckRefV1::MusubiPinOutbox(check) = instruction {
            // Preserve the data-model validator's account -> fields -> instruction order without
            // erasing an unfinished canonical measurement into its payload-free ParseError.
            check_canonical_frame_bound::<_, E>(
                &check.pin_authority,
                iroha_data_model::musubi::MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1,
            )?;
        }
        let (
            purpose,
            challenge,
            check_network,
            minimum_height,
            minimum_block_hash,
            check_context_id,
        ) = instruction.coordinates()?;
        if let NativeCustodyCheckRefV1::MusubiPinOutbox(check) = instruction {
            check_canonical_frame_bound::<_, E>(
                check,
                iroha_data_model::isi::musubi::MUSUBI_PIN_OUTBOX_CHECK_MAX_BYTES_V1,
            )?;
        }
        let chain_id = BoundChainId::new(chain_id)?;
        if round.challenge != Some(challenge)
            || challenge == [0; 32]
            || network_id == [0; 32]
            || check_network != network_id
            || minimum_height != floor.height
            || minimum_block_hash != floor.block_hash
            || check_context_id.is_some_and(|context_id| context_id != floor.context_id)
        {
            return Err(Error::Transaction.into());
        }
        let TransactionEntrypoint::External(signed) = attempt.entry.entrypoint() else {
            return Err(Error::Transaction.into());
        };
        let Executable::Instructions(instructions) = signed.instructions() else {
            return Err(Error::Transaction.into());
        };
        if signed.authority() != authority
            || signed.network_id().map(|network| *network.as_bytes()) != Some(network_id)
            || instructions.len() != 1
            || !instructions
                .first()
                .is_some_and(|candidate| instruction.matches_instruction(candidate))
        {
            return Err(Error::Transaction.into());
        }
        validate_native_signed_profile_v1(signed)?;
        let hash = HashOf::try_new(signed.payload())?;
        iroha_crypto::verify_signature_borrowed(
            &signed.signature().0,
            signed
                .authority()
                .try_signatory()
                .ok_or(Error::Transaction)?,
            hash.as_ref(),
        )
        .map_err(|_| Error::Transaction)?;
        // Fixed canonical flags are independent of the caller's layout. No new decode context
        // replaces surviving ceilings, and no encode/decode copy recreates the signed graph.
        let length = norito::canonical_frame_len(attempt.entry.entrypoint())?;
        if length > FINAL_PROMOTION_NATIVE_TRANSACTION_MAX_BYTES_V1 {
            return Err(Error::Transaction.into());
        }
        norito::core::reserve_decode_allocation(length)?;
        let mut writer = FrameWriter(ChargedBuffer::new(length, &state.ivm_execution_budget())?);
        norito::core::write_canonical_to_writer(attempt.entry.entrypoint(), &mut writer)?;
        if writer.0.as_slice().len() != length {
            return Err(norito::Error::LengthMismatch.into());
        }
        round.ensure_live()?;
        Ok((
            purpose,
            chain_id,
            network_id,
            floor,
            round.started,
            round.max_elapsed,
            challenge,
            writer.0,
        ))
    })();
    match result {
        Ok((
            purpose,
            chain_id,
            network_id,
            floor,
            started,
            max_elapsed,
            challenge,
            entry_bytes,
        )) => Ok((
            prepared,
            BoundNativeCheckV1 {
                purpose,
                chain_id,
                network_id,
                floor,
                started,
                max_elapsed,
                challenge,
                entry: attempt.entry,
                entry_bytes,
            },
        )),
        Err(error) => Err(BindingFailure {
            prepared,
            signed: attempt,
            error,
        }),
    }
}

/// Preserve measurement refusal separately from the unchanged semantic frame ceiling.
fn check_canonical_frame_bound<T: norito::core::NoritoSerialize, E: From<Error>>(
    value: &T,
    maximum: usize,
) -> Result<(), NativeCheckBindingErrorV1<E>> {
    let length = norito::canonical_frame_len(value)?;
    if length == 0 || length > maximum {
        return Err(Error::Transaction.into());
    }
    Ok(())
}

struct FrameWriter(ChargedBuffer<u8>);
impl io::Write for FrameWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.append(bytes)?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
