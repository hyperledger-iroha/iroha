//! Scoped selection reads for a captured completion, separate from the committed MV cell.
//!
//! Every native authorization, handler and IVM host reads the same execution selection through
//! `WorldTransaction`. A completion may therefore run its retained implementation beside a new
//! binding without rewriting State or its undo journal. Applying the transaction always applies
//! only the canonical cell; the temporary context is never committed or serialized.

use iroha_data_model::{
    isi::InstructionBox,
    runtime::{definition::RootSelectionV1, obligation::CompletionCaptureV1},
};
use mv::cell::Transaction as CellTransaction;

mod native;
pub(crate) use native::{native_step, with_native_obligation};

/// One authoritative native operation's original closure and restriction authority.
/// Nested native operations replace this scope temporarily and restore the exact parent.
#[derive(Clone, Debug)]
pub(crate) struct NativeExecution {
    /// Exact authoritative custody capture.
    pub(crate) capture: CompletionCaptureV1,
    /// The operation's own progress or terminal authority; never widened by its caller.
    pub(crate) step: super::obligations::Step,
}

/// Whether the actual executing binding belongs to this original captured call graph.
pub(crate) fn capture_contains_binding(
    transaction: &crate::state::StateTransaction<'_, '_>,
    capture: &CompletionCaptureV1,
    instruction: &InstructionBox,
) -> bool {
    let Some(wire_id) = iroha_data_model::isi::instruction_wire_id(instruction) else {
        return false;
    };
    let bindings = &transaction.world.runtime_selection.get().bindings;
    let Ok(index) = bindings.binary_search_by(|bound| bound.wire_id.as_str().cmp(wire_id)) else {
        return false;
    };
    let bound = &bindings[index];
    capture.implementation_for(
        wire_id,
        &bound.interface,
        &super::change_set::PublishedDefinitions(&transaction.world.runtime_definitions),
    ) == Some(bound.implementation)
}

/// The root's original selection journal with an optional scoped execution context.
pub(crate) struct SelectionTransaction<'block, 'world> {
    canonical: CellTransaction<'block, 'world, RootSelectionV1>,
    executing: Option<RootSelectionV1>,
}

impl<'block, 'world> SelectionTransaction<'block, 'world> {
    /// Wrap the original MV transaction without touching its undo authority.
    pub(crate) fn new(canonical: CellTransaction<'block, 'world, RootSelectionV1>) -> Self {
        Self {
            canonical,
            executing: None,
        }
    }

    /// Read the context currently executing, or the committed root's ordinary context.
    pub(crate) fn get(&self) -> &RootSelectionV1 {
        self.executing
            .as_ref()
            .unwrap_or_else(|| self.canonical.get())
    }

    /// Read the committed journal when constructing a root-wide policy update.
    ///
    /// Execution may see only a captured call graph. A policy write must preserve every
    /// unrelated binding and definition in the canonical selection.
    pub(crate) fn canonical(&self) -> &RootSelectionV1 {
        self.canonical.get()
    }

    /// Install or release the scoped context. The registry brackets one terminal effect.
    pub(crate) fn set_executing(&mut self, selection: Option<RootSelectionV1>) {
        self.executing = selection;
    }

    /// Retain the enclosing execution context so nested scopes restore it exactly.
    pub(crate) fn replace_executing(
        &mut self,
        selection: Option<RootSelectionV1>,
    ) -> Option<RootSelectionV1> {
        core::mem::replace(&mut self.executing, selection)
    }

    /// Apply the canonical journal only; a scoped context grants no State mutation authority.
    pub(crate) fn apply(self) {
        self.canonical.apply();
    }
}

impl<'block, 'world> core::ops::Deref for SelectionTransaction<'block, 'world> {
    type Target = CellTransaction<'block, 'world, RootSelectionV1>;

    fn deref(&self) -> &Self::Target {
        &self.canonical
    }
}

impl core::ops::DerefMut for SelectionTransaction<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.canonical
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_context_never_replaces_the_canonical_cell_or_undo() {
        let original = crate::runtime::release_catalog().unwrap().baseline.clone();
        let mut contextual = original.clone();
        contextual.bindings.clear();
        let cell = mv::cell::Cell::new(original.clone());
        let mut block = cell.block();
        let mut transaction = SelectionTransaction::new(block.transaction());
        transaction.set_executing(Some(contextual.clone()));
        assert_eq!(transaction.get(), &contextual);
        assert_eq!(transaction.canonical(), &original);
        assert_eq!(transaction.get_before_transaction(), &original);
        assert_eq!(transaction.get_before_block(), &original);
        assert!(transaction.touched_value().is_none());
        let enclosing = transaction.replace_executing(Some(original.clone()));
        assert_eq!(enclosing, Some(contextual.clone()));
        assert_eq!(transaction.get(), &original);
        assert_eq!(
            transaction.replace_executing(enclosing),
            Some(original.clone())
        );
        assert_eq!(transaction.get(), &contextual);
        assert_eq!(transaction.replace_executing(None), Some(contextual));
        assert_eq!(transaction.get(), &original);
        transaction.apply();
        block.commit();
        assert_eq!(cell.view().get(), &original);
    }
}
