//! Fixed runtime frames borrowing the executor's one original signed owner.

use super::*;
use iroha_data_model::transaction::{
    Executable, ExecutableBatchItem, executable::ContractInvocation,
};

#[derive(Clone, Copy, Default)]
struct Frame {
    generation: u64,
    next_child: u32,
    issued: u16,
    consumed: u16,
    completed: u16,
}

/// One deployed root and its actual child frames; never a guest-provided scope.
/// Dropping an unfinished root poisons the same transaction attempt, including unwind.
pub struct ContractCustody<'owner, 'tx> {
    owner: &'owner Owner<'tx>,
    attempt: &'owner mut Attempt,
    call: &'tx ContractInvocation,
    root_index: u32,
    path: [u32; ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1],
    frames: [Frame; ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1 + 1],
    depth: usize,
    finished: bool,
}

impl<'tx> AttachmentCustody<'tx> {
    /// Mint a root only for the original call object at its signed executable position.
    /// Equal re-encodings or cloned calls do not establish original-source custody.
    pub(crate) fn contract_host(
        &mut self,
        root_index: u32,
        call: &ContractInvocation,
    ) -> Result<ContractCustody<'_, 'tx>, CustodyError> {
        self.attempt.ensure_active()?;
        let original = match self.owner.source.instructions() {
            Executable::ContractCall(original) if root_index == 0 => Some(original),
            Executable::Batch(items) => usize::try_from(root_index)
                .ok()
                .and_then(|index| items.get(index))
                .and_then(|item| match item {
                    ExecutableBatchItem::ContractCall(original) => Some(original),
                    ExecutableBatchItem::Instruction(_) => None,
                }),
            _ => None,
        };
        let Some(original) = original.filter(|original| core::ptr::eq(*original, call)) else {
            return self.attempt.refuse(CustodyError::ForeignOwner);
        };
        let generation = self.attempt.next_generation()?;
        let mut frames = [Frame::default(); ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1 + 1];
        frames[0] = Frame {
            generation,
            next_child: 0,
            issued: self.attempt.issued,
            consumed: self.attempt.consumed,
            completed: self.attempt.completed,
        };
        Ok(ContractCustody {
            owner: &self.owner,
            attempt: &mut self.attempt,
            call: original,
            root_index,
            path: [0; ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1],
            frames,
            depth: 0,
            finished: false,
        })
    }
}

impl ContractCustody<'_, '_> {
    /// The selected executable must still be the exact original call object.
    pub(crate) fn require_original_call(
        &mut self,
        call: &ContractInvocation,
    ) -> Result<(), CustodyError> {
        self.attempt.ensure_active()?;
        if !core::ptr::eq(self.call, call) {
            return self.attempt.refuse(CustodyError::ForeignOwner);
        }
        Ok(())
    }

    /// Match the host's resolved root authorization to its original signed call.
    pub(crate) fn require_root_authorization(
        &mut self,
        context: &crate::executor::ContractRuntimeExecutionContext,
        authorization: &crate::executor::ContractEntrypointAuthorizationSnapshot,
    ) -> Result<(), CustodyError> {
        self.attempt.ensure_active()?;
        if !authorization.is_root()
            || context.contract_address != self.call.contract_address
            || context.entrypoint != self.call.entrypoint
            || context.contract_subject != self.call.contract_address.subject_id()
            || authorization.contract_address != self.call.contract_address
            || authorization.entrypoint != self.call.entrypoint
            || authorization.code_hash != self.call.expected_code_hash
            || authorization.authority != *self.owner.source.authority()
        {
            return self
                .attempt
                .refuse(CustodyError::WrongInvocation(self.root_index));
        }
        Ok(())
    }

    fn binding(&self) -> InvocationBinding {
        InvocationBinding::Contract {
            root_index: self.root_index,
            path: self.path,
            depth: self.depth,
        }
    }

    fn check_exit(&mut self) -> Result<(), CustodyError> {
        self.attempt.ensure_active()?;
        if let Some(attachments) = self.owner.source.attachments() {
            for action in attachments.actions() {
                if self.binding().matches(&action.invocation)
                    && self.attempt.completed & bit(action.action_index) == 0
                {
                    return self
                        .attempt
                        .refuse(CustodyError::UnconsumedAction(action.action_index));
                }
            }
        }
        Ok(())
    }

    /// Enter a child only after the actual host authenticated its dispatch and arguments.
    pub(crate) fn enter_child(&mut self) -> Result<(), CustodyError> {
        self.attempt.ensure_active()?;
        if self.depth >= ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1 {
            return self.attempt.refuse(CustodyError::InvalidChild);
        }
        let Some(next) = self.frames[self.depth].next_child.checked_add(1) else {
            return self.attempt.refuse(CustodyError::GenerationExhausted);
        };
        let generation = self.attempt.next_generation()?;
        self.path[self.depth] = self.frames[self.depth].next_child;
        self.frames[self.depth].next_child = next;
        self.depth += 1;
        self.frames[self.depth] = Frame {
            generation,
            next_child: 0,
            issued: self.attempt.issued,
            consumed: self.attempt.consumed,
            completed: self.attempt.completed,
        };
        Ok(())
    }

    /// A recoverable Result or view rollback cannot erase declaration consumption.
    pub(crate) fn require_rollback_unchanged(&mut self) -> Result<(), CustodyError> {
        self.attempt.ensure_active()?;
        let start = self.frames[self.depth];
        if (
            self.attempt.issued,
            self.attempt.consumed,
            self.attempt.completed,
        ) != (start.issued, start.consumed, start.completed)
        {
            return self.attempt.refuse(CustodyError::RolledBack);
        }
        Ok(())
    }

    /// Close the actual child before restoring its caller's business context.
    pub(crate) fn exit_child(&mut self, rollback: bool) -> Result<(), CustodyError> {
        if self.depth == 0 {
            return self.attempt.refuse(CustodyError::InvalidChild);
        }
        let result = self.check_exit().and_then(|()| {
            if rollback {
                self.require_rollback_unchanged()
            } else {
                Ok(())
            }
        });
        // The ordinal counter stays outside rollback; a second child cannot reuse [0].
        self.depth -= 1;
        result
    }

    /// Borrow the current runtime frame for one native operation. The actual host
    /// supplies its authorized callee subject; no declaration supplies authority.
    /// TODO(I.3): connect only a complete typed native consumer to this private seam.
    pub(crate) fn with_invocation<T, E: From<CustodyError>>(
        &mut self,
        subject: &AccountId,
        visit: impl FnOnce(&mut Invocation<'_, '_>) -> Result<T, E>,
    ) -> Result<T, E> {
        self.attempt.ensure_active()?;
        let binding = self.binding();
        if let Some(attachments) = self.owner.source.attachments() {
            for action in attachments.actions() {
                if binding.matches(&action.invocation)
                    && !matches!(&action.authority,
                        ActionAuthorityBindingV1::ContractSubject(actual) if actual == subject)
                {
                    return self
                        .attempt
                        .refuse(CustodyError::WrongAuthority(action.action_index))
                        .map_err(E::from);
                }
            }
        }
        let mut invocation = Invocation {
            owner: self.owner,
            attempt: self.attempt,
            binding,
            actor: subject,
            generation: self.frames[self.depth].generation,
            #[cfg(test)]
            next_child: 0,
        };
        let mut guard = InvocationGuard {
            frame: &mut invocation,
            completed: false,
        };
        let value = visit(guard.frame)?;
        guard.completed = true;
        Ok(value)
    }

    /// Poison the original attempt when actual child execution fails.
    pub(crate) fn abort(&mut self) {
        let _ = self.attempt.refuse::<()>(CustodyError::FailedAttempt);
    }

    /// Finish the root only after ordinary host result and output validation.
    pub(crate) fn finish(mut self) -> Result<(), CustodyError> {
        if self.depth != 0 {
            return self.attempt.refuse(CustodyError::InvalidChild);
        }
        self.check_exit()?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for ContractCustody<'_, '_> {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.attempt.refuse::<()>(CustodyError::FailedAttempt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        smart_contract::ContractAddress,
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use iroha_model_base::topology::DataSpaceId;
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};

    fn original() -> SignedTransaction {
        let network = NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"host-custody-boundary"),
        ));
        let call = ContractInvocation {
            contract_address: ContractAddress::derive(
                &network,
                &ALICE_ID,
                1,
                DataSpaceId::UNIVERSAL,
            )
            .unwrap(),
            expected_code_hash: Hash::new(b"fixed host boundary fixture"),
            entrypoint: "main".into(),
            arguments: None,
        };
        TransactionBuilder::new(
            network,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::ContractCall(call))
        .sign(ALICE_KEYPAIR.private_key())
    }
    fn call(source: &SignedTransaction) -> &ContractInvocation {
        let Executable::ContractCall(call) = source.instructions() else {
            unreachable!()
        };
        call
    }

    #[test]
    fn actual_host_frames_admit_depth_32_and_refuse_33_without_growing() {
        for overflow in [false, true] {
            let source = original();
            let mut owner = AttachmentCustody::new(&source).unwrap();
            let mut root = owner.contract_host(0, call(&source)).unwrap();
            for depth in 1..=ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1 {
                root.enter_child().unwrap();
                assert_eq!(root.depth, depth);
                assert_eq!(
                    root.frames.len(),
                    ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1 + 1
                );
            }
            if overflow {
                assert_eq!(root.enter_child(), Err(CustodyError::InvalidChild));
                assert_eq!(root.depth, ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1);
                drop(root);
                assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
            } else {
                for _ in 0..ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1 {
                    root.exit_child(false).unwrap();
                }
                root.finish().unwrap();
                owner.finish().unwrap();
            }
        }
    }

    #[test]
    fn actual_host_child_ordinal_exhaustion_never_wraps_or_enters() {
        let source = original();
        let mut owner = AttachmentCustody::new(&source).unwrap();
        let mut root = owner.contract_host(0, call(&source)).unwrap();
        root.frames[0].next_child = u32::MAX;
        assert_eq!(root.enter_child(), Err(CustodyError::GenerationExhausted));
        assert_eq!(root.frames[0].next_child, u32::MAX);
        assert_eq!(root.depth, 0);
        drop(root);
        assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    }

    #[test]
    fn actual_host_root_and_child_generation_exhaustion_poison_the_original_attempt() {
        let source = original();
        let mut owner = AttachmentCustody::new(&source).unwrap();
        owner.attempt.generation = u64::MAX;
        assert!(matches!(
            owner.contract_host(0, call(&source)),
            Err(CustodyError::GenerationExhausted)
        ));
        assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
        let mut owner = AttachmentCustody::new(&source).unwrap();
        let mut root = owner.contract_host(0, call(&source)).unwrap();
        root.attempt.generation = u64::MAX;
        assert_eq!(root.enter_child(), Err(CustodyError::GenerationExhausted));
        assert_eq!(root.attempt.generation, u64::MAX);
        assert_eq!(root.frames[0].next_child, 0);
        assert_eq!(root.depth, 0);
        drop(root);
        assert_eq!(owner.finish(), Err(CustodyError::FailedAttempt));
    }
}
