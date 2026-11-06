//! Host-owned lifecycle origins. These scopes allocate nothing while capture is disabled.

use super::*;
use iroha_data_model::block::execution_output::{
    TriggerLifecycleOriginV1 as Origin, TriggerLifecyclePolicyV1 as Policy,
};

impl StateTransaction<'_, '_> {
    /// Restore the enclosing origin on errors and unwinding as well as success.
    pub(crate) fn with_trigger_lifecycle_origin<R>(
        &mut self,
        origin: Origin,
        run: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let Some(previous) = self.world.triggers.lifecycle_origin() else {
            return run(self);
        };
        self.world.triggers.set_lifecycle_origin(origin);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(self)));
        self.world.triggers.set_lifecycle_origin(previous);
        match result {
            Ok(value) => value,
            Err(panic) => {
                self.world
                    .triggers
                    .refuse_lifecycle("lifecycle execution scope unwound");
                std::panic::resume_unwind(panic)
            }
        }
    }

    /// Derived execution preserves its actual callback but drops signed-input provenance.
    pub(crate) fn derived_trigger_lifecycle_origin(&self) -> Origin {
        let callback_index = match self.world.triggers.lifecycle_origin() {
            Some(Origin::Derived { callback_index }) => callback_index,
            Some(Origin::InvocationPolicy { callback_index, .. }) => Some(callback_index),
            _ => None,
        };
        Origin::Derived { callback_index }
    }

    /// Classify one exact original signed item, including uncompressed Batch positions.
    pub(crate) fn direct_trigger_lifecycle_origin(
        &self,
        transaction: &SignedTransaction,
        instruction: &InstructionBox,
        index: usize,
        direct_body: bool,
    ) -> Origin {
        let derived = self.derived_trigger_lifecycle_origin();
        if !self.world.triggers.lifecycle_enabled()
            || !direct_body
            || self.execution_callback_active()
        {
            return derived;
        }
        let submitted = match transaction.instructions() {
            Executable::Instructions(items) => items.get(index),
            Executable::Batch(items) => items.get(index).and_then(|item| match item {
                iroha_data_model::transaction::ExecutableBatchItem::Instruction(i) => Some(i),
                _ => None,
            }),
            _ => None,
        };
        let entry_hash = transaction.hash_as_entrypoint();
        // Original-body dispatch borrows this exact slot. InstructionBox equality
        // would allocate two encodings and could label a synthesized copy direct.
        if !submitted.is_some_and(|submitted| core::ptr::eq(submitted, instruction))
            || self.current_network_entrypoint_hash != Some(entry_hash)
            || self.tx_call_hash != Some(Hash::from(entry_hash))
            || self.current_tx_hash != Some(transaction.hash())
            || self.current_entrypoint_index.is_none()
        {
            return derived;
        }
        u32::try_from(index).map_or(derived, |item_index| Origin::DirectNetworkInstruction {
            item_index,
        })
    }

    pub(crate) fn with_trigger_lifecycle_policy<R>(
        &mut self,
        callback_index: Option<u32>,
        policy: Policy,
        run: impl FnOnce(&mut Self) -> R,
    ) -> R {
        match callback_index {
            Some(callback_index) => self.with_trigger_lifecycle_origin(
                Origin::InvocationPolicy {
                    callback_index,
                    policy,
                },
                run,
            ),
            None => run(self),
        }
    }
}

#[cfg(test)]
#[path = "trigger_lifecycle_tests.rs"]
mod tests;
