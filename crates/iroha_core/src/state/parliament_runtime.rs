//! One-use original signed source of the anonymous runtime proposal creator.
//!
//! A nested instruction, callback, reveal or user executor cannot manufacture this source.

use super::{StateReadOnly, StateTransaction};
use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    account::AccountId,
    governance::anonymous_ballot::CreateAnonymousRuntimeUpgradeAttemptV1,
    isi::InstructionBox,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};

/// Only the direct external executor can arm this coordinate; it is neither clonable nor wire data.
pub(super) struct DirectRuntimeProposal {
    signed_hash: HashOf<SignedTransaction>,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    input_index: u64,
    height: u64,
    payload: Hash,
}

/// Original request binding or its unchanged local refusal, consumed once.
pub(super) type DirectRuntimeCapture =
    Result<DirectRuntimeProposal, ExecutionAttemptError<&'static str>>;

fn payload(
    instruction: &CreateAnonymousRuntimeUpgradeAttemptV1,
) -> Result<Hash, ExecutionAttemptError<&'static str>> {
    norito::core::classify_decode_attempt(|| {
        let mut encoded = Ok(());
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(b"iroha:parliament:runtime-creator:v1\0")?;
            encoded = norito::core::write_canonical_to_writer(instruction, writer);
            Ok(())
        })
        .map_err(norito::Error::Io)?;
        encoded?;
        Ok(hash)
    })
    .map_err(|original| {
        canonical_decode_attempt_error(original, |_| "runtime creator payload cannot be encoded")
    })
}

impl StateTransaction<'_, '_> {
    /// Arm exactly the sole creator in the original direct signed instruction vector.
    pub(crate) fn arm_direct_parliament_runtime(
        &mut self,
        signed: &SignedTransaction,
        authority: &AccountId,
        instruction: &InstructionBox,
        instruction_index: usize,
        direct: bool,
    ) {
        self.direct_parliament_runtime = None;
        if !direct
            || self.execution_callback_active()
            || signed.authority() != authority
            || signed.network_id() != Some(self.network_id())
            || instruction
                .as_any()
                .downcast_ref::<CreateAnonymousRuntimeUpgradeAttemptV1>()
                .is_none()
        {
            return;
        }
        let Some(input_index) = self.current_entrypoint_index else {
            return;
        };
        let Executable::Instructions(originals) = signed.instructions() else {
            return;
        };
        if instruction_index != 0 || originals.len() != 1 || originals.first() != Some(instruction)
        {
            return;
        }
        let entrypoint_hash =
            match norito::core::classify_decode_attempt(|| signed.try_hash_as_entrypoint()) {
                Ok(hash) => hash,
                Err(original) => {
                    self.direct_parliament_runtime = Some(Err(canonical_decode_attempt_error(
                        original,
                        |_| "runtime creator original signed intent cannot be encoded",
                    )));
                    return;
                }
            };
        let signed_hash = HashOf::from_untyped_unchecked(Hash::from(entrypoint_hash));
        if self.current_tx_hash != Some(signed_hash)
            || self.tx_call_hash != Some(Hash::from(entrypoint_hash))
            || self.current_network_entrypoint_hash != Some(entrypoint_hash)
        {
            return;
        }
        let Some(creator) = instruction
            .as_any()
            .downcast_ref::<CreateAnonymousRuntimeUpgradeAttemptV1>()
        else {
            return;
        };
        let payload = match payload(creator) {
            Ok(payload) => payload,
            Err(original) => {
                self.direct_parliament_runtime = Some(Err(original));
                return;
            }
        };
        self.direct_parliament_runtime = Some(Ok(DirectRuntimeProposal {
            signed_hash,
            entrypoint_hash,
            input_index,
            height: self.block_height(),
            payload,
        }));
    }

    /// Consume the original before any fallible admission or State mutation.
    pub(crate) fn consume_direct_parliament_runtime(
        &mut self,
        instruction: &CreateAnonymousRuntimeUpgradeAttemptV1,
    ) -> Result<(), ExecutionAttemptError<&'static str>> {
        let original =
            self.direct_parliament_runtime
                .take()
                .ok_or(ExecutionAttemptError::Rejected(
                    "runtime proposal requires its sole original signed creator",
                ))??;
        if self.execution_callback_active()
            || self.current_tx_hash != Some(original.signed_hash)
            || self.tx_call_hash != Some(Hash::from(original.entrypoint_hash))
            || self.current_network_entrypoint_hash != Some(original.entrypoint_hash)
            || self.current_entrypoint_index != Some(original.input_index)
            || self.block_height() != original.height
            || payload(instruction)? != original.payload
        {
            return Err(ExecutionAttemptError::Rejected(
                "runtime proposal original source changed",
            ));
        }
        Ok(())
    }
}
