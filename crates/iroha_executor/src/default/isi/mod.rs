use iroha_executor_data_model::isi::multisig::MultisigInstructionBox;
use iroha_smart_contract::data_model::{isi::CustomInstruction, prelude::InstructionBox};

use super::*;
use crate::prelude::{Execute, Visit};

/// Dispatches a custom instruction through the default executor pipeline.
pub fn visit_custom_instruction<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    instruction: &CustomInstruction,
) {
    if let Ok(multisig) = MultisigInstructionBox::try_from(instruction.payload()) {
        multisig::visit_instruction(multisig, executor);
        return;
    }

    deny!(executor, "unexpected custom instruction");
}

/// Trait implemented by instructions that can be visited and executed sequentially.
pub trait VisitExecute {
    /// Visits and executes the instruction within the provided executor.
    fn visit_execute<V: Execute + Visit + ?Sized>(self, executor: &mut V)
    where
        Self: Sized,
    {
        let init_authority = executor.context().authority.clone();
        self.visit(executor);
        if executor.verdict().is_ok()
            && let Err(err) = self.execute(executor)
        {
            executor.deny(err);
        }
        executor.context_mut().authority = init_authority;
    }

    /// Performs the visitor portion of instruction handling.
    fn visit<V: Execute + Visit + ?Sized>(&self, executor: &mut V) {
        let name = core::any::type_name::<Self>();
        executor.deny(ValidationFail::NotPermitted(format!(
            "{name} does not implement visit()"
        )));
    }

    /// Executes the instruction after visiting it.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationFail`] when the instruction cannot be executed under the current context.
    fn execute<V: Execute + Visit + ?Sized>(self, _executor: &mut V) -> Result<(), ValidationFail>
    where
        Self: Sized,
    {
        let name = core::any::type_name::<Self>();
        Err(ValidationFail::NotPermitted(format!(
            "{name} does not implement execute()"
        )))
    }
}

/// Validate and execute instructions in sequence without returning back to the visit root,
/// checking the sanity of the executor verdict
#[allow(unused_macros)]
macro_rules! visit_seq {
    ($executor:ident.$visit:ident($instruction:expr)) => {
        $executor.$visit($instruction);
        if $executor.verdict().is_err() {
            return $executor.verdict().clone();
        }
    };
}

impl VisitExecute for InstructionBox {
    fn visit_execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) {
        if let Ok(multisig) = MultisigInstructionBox::try_from(&self) {
            multisig::visit_instruction(multisig, executor);
            return;
        }

        if let Some(custom) = (*self).as_any().downcast_ref::<CustomInstruction>() {
            visit_custom_instruction(executor, custom);
            return;
        }

        deny!(executor, "unexpected instruction type");
    }
}

mod multisig;

pub(super) fn is_reserved_multisig_metadata_key(key: &iroha_smart_contract::data_model::name::Name) -> bool {
    multisig::is_reserved_multisig_metadata_key(key)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId, block::BlockHeader, executor, prelude::ValidationFail,
    };

    use super::*;
    use crate::prelude::Context;

    #[derive(Debug, Clone)]
    struct DummyInstruction;

    impl VisitExecute for DummyInstruction {}

    #[derive(Debug)]
    struct DummyExecutor {
        host: iroha_smart_contract::Iroha,
        ctx: Context,
        verdict: executor::Result<(), ValidationFail>,
    }

    impl DummyExecutor {
        fn new() -> Self {
            let authority_keypair = KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519);
            let authority = AccountId::new(authority_keypair.public_key().clone());
            let header =
                BlockHeader::new(NonZeroU64::new(1).expect("nonzero"), None, None, None, 0, 0);
            Self {
                host: iroha_smart_contract::Iroha,
                ctx: Context {
                    authority,
                    curr_block: header,
                },
                verdict: Ok(()),
            }
        }
    }

    impl Execute for DummyExecutor {
        fn host(&self) -> &iroha_smart_contract::Iroha {
            &self.host
        }

        fn context(&self) -> &Context {
            &self.ctx
        }

        fn context_mut(&mut self) -> &mut Context {
            &mut self.ctx
        }

        fn verdict(&self) -> &executor::Result<(), ValidationFail> {
            &self.verdict
        }

        fn deny(&mut self, reason: ValidationFail) {
            self.verdict = Err(reason);
        }
    }

    impl Visit for DummyExecutor {}

    #[test]
    fn default_visit_execute_sets_denial() {
        let mut executor = DummyExecutor::new();
        DummyInstruction.visit_execute(&mut executor);
        let verdict = executor.verdict().as_ref().expect_err("expected denial");
        if let ValidationFail::NotPermitted(msg) = verdict {
            assert!(
                msg.contains("DummyInstruction"),
                "denial message should mention instruction type"
            );
        } else {
            panic!("expected NotPermitted verdict, got {verdict:?}");
        }
    }
}
