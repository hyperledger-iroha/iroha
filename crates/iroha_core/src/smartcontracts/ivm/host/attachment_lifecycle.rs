//! Actual deployed host frames borrow the executor's original attachment owner.

use super::*;
use crate::executor::attachment_custody::{ContractCustody, CustodyError, Invocation};

const _: () = assert!(MAX_NESTED_CONTRACT_CALL_DEPTH == iroha_data_model::transaction::signed_attachments::ACTION_INVOCATION_MAX_CALL_PATH_DEPTH_V1);

impl<'run, 'tx, QS: Default + QueryStateAccess> CoreHostImpl<'run, 'tx, QS> {
    /// Install the root minted from the actual original signed call object.
    pub(crate) fn bind_attachment_custody(
        &mut self,
        mut owner: ContractCustody<'run, 'tx>,
    ) -> Result<(), CustodyError> {
        if self.attachment_custody.is_some() || self.nested_contract_call_depth != 0 {
            self.abort_attachment_attempt();
            // Dropping the refused new owner poisons its original attempt too.
            return Err(CustodyError::WrongInvocation(0));
        }
        match (
            &self.current_contract_runtime_context,
            &self.current_entrypoint_authorization,
        ) {
            (Some(context), Some(authorization)) if self.authority == authorization.authority => {
                owner.require_root_authorization(context, authorization)?;
            }
            _ => return Err(CustodyError::WrongAuthority(0)),
        }
        self.attachment_custody = Some(owner);
        Ok(())
    }

    pub(super) fn enter_attachment_child(&mut self) -> Result<(), ivm::VMError> {
        if let Some(owner) = &mut self.attachment_custody {
            owner
                .enter_child()
                .map_err(|_| ivm::VMError::PermissionDenied)?;
        }
        Ok(())
    }
    pub(super) fn finish_attachment_child(&mut self, rollback: bool) -> Result<(), ivm::VMError> {
        let authority = if self.attachment_custody.is_some() {
            self.with_attachment_invocation::<_, CustodyError>(|_| Ok(()))
                .map_err(|_| ivm::VMError::PermissionDenied)
        } else {
            Ok(())
        };
        // Always pop the actual entered child, even when its authority check poisoned it.
        let exit = self.attachment_custody.as_mut().map_or(Ok(()), |owner| {
            owner
                .exit_child(rollback)
                .map_err(|_| ivm::VMError::PermissionDenied)
        });
        authority.and(exit)
    }
    pub(super) fn abort_attachment_attempt(&mut self) {
        if let Some(owner) = &mut self.attachment_custody {
            owner.abort();
        }
    }
    pub(super) fn rollback_attachment_root(&mut self) -> Result<(), ivm::VMError> {
        if let Some(owner) = &mut self.attachment_custody {
            owner
                .require_rollback_unchanged()
                .map_err(|_| ivm::VMError::PermissionDenied)?;
        }
        Ok(())
    }
    pub(super) fn finish_attachment_root(&mut self) -> Result<(), ValidationFail> {
        if self.attachment_custody.is_some() {
            // Revalidate the restored root's actual subject before releasing any artifacts.
            self.with_attachment_invocation::<_, CustodyError>(|_| Ok(()))
                .map_err(|e| ValidationFail::NotPermitted(e.to_string()))?;
        }
        if let Some(owner) = self.attachment_custody.take() {
            owner
                .finish()
                .map_err(|e| ValidationFail::NotPermitted(e.to_string()))?;
        }
        Ok(())
    }

    /// The current immutable authorization supplies the subject, not a signed
    /// declaration or guest account. This does not itself enable any operation.
    /// X.509 binds its normalized request and native effect to this invocation.
    /// TODO(I.3): complete X509's joint security construction and genuine signed execution
    /// before enabling admission; other operations and independent replay roots still
    /// need their typed consumers.
    pub(super) fn with_attachment_invocation<T, E: From<CustodyError>>(
        &mut self,
        visit: impl FnOnce(&mut Invocation<'_, '_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let Some(context) = self.current_contract_runtime_context.as_ref() else {
            self.abort_attachment_attempt();
            return Err(CustodyError::WrongAuthority(0).into());
        };
        let Some(authorization) = self.current_entrypoint_authorization.as_ref() else {
            self.abort_attachment_attempt();
            return Err(CustodyError::WrongAuthority(0).into());
        };
        if context.contract_subject != context.contract_address.subject_id()
            || context.contract_address != authorization.contract_address
            || context.entrypoint != authorization.entrypoint
            || self.authority != authorization.authority
        {
            self.abort_attachment_attempt();
            return Err(CustodyError::WrongAuthority(0).into());
        }
        let Some(owner) = self.attachment_custody.as_mut() else {
            return Err(CustodyError::ForeignOwner.into());
        };
        owner.with_invocation(&context.contract_subject, visit)
    }
}
