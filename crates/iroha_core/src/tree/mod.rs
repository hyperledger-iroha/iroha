//! Transitional interface for [`iroha_tree`].

use std::rc::Rc;

use iroha_tree::{
    changeset, event, node, readset, receptor, state, transitional as tr, FuzzyNodeKey,
    NodeConflict, NodeEntry,
};
use mv::storage::StorageReadOnly;

use crate::{
    smartcontracts::triggers::{
        set::{ExecutableRef, SetReadOnly},
        specialized::LoadedActionTrait,
    },
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

/// TODO when instructions as an executable were replaced with a changeset
#[derive(Debug, Clone, Copy)]
pub struct InvariantViolation;

impl From<Box<NodeConflict<changeset::Write>>> for InvariantViolation {
    fn from(_value: Box<NodeConflict<changeset::Write>>) -> Self {
        unimplemented!("TODO when instructions as an executable were replaced with a changeset")
    }
}

impl state::WorldState for StateTransaction<'_, '_> {
    type InvariantViolation = InvariantViolation;

    fn update_by(
        &mut self,
        _entry: NodeEntry<changeset::Write>,
    ) -> Result<(), Self::InvariantViolation> {
        unimplemented!("TODO when instructions as an executable were replaced with a changeset")
    }

    fn sanitize(
        &self,
        _event_prediction: &event::Event,
    ) -> Result<changeset::ChangeSet, Self::InvariantViolation> {
        // TODO: #4672 Cascade or restrict on delete?
        unimplemented!("TODO when instructions as an executable were replaced with a changeset")
    }

    fn load(&self, readset: &readset::ReadSet) -> state::StateView {
        let mut res = state::StateView::default();
        for (k, _v) in readset.iter() {
            match k {
                FuzzyNodeKey::Trigger(key) => {
                    self.world()
                        .triggers()
                        // FIXME: Other types of triggers are irrelevant as long as this function is used solely for event loop detection.
                        .data_triggers()
                        .iter()
                        .filter(|(id, _)| key.as_ref().map_or(true, |key| **key == **id))
                        .for_each(|(id, action)| {
                            let trigger = state::tr::TriggerV::from(action.repeats);
                            let condition = state::tr::ConditionV::from(receptor::Receptor::from(action.filter.clone()));
                            let executable = match action.executable() {
                                ExecutableRef::Wasm(_hash) => {
                                    let wasm = state::tr::WasmExecutable;
                                    state::tr::ExecutableV::Dynamic(wasm)
                                }
                                ExecutableRef::Instructions(instructions) => {
                                    let changeset = (
                                        action.authority.clone(),
                                        instructions.clone().into_vec(),
                                    )
                                        .try_into()
                                        .expect("instructions that are already registered as an executable should be converted into a changeset");
                                    state::tr::ExecutableV::Static(changeset)
                                }
                            };
                            let trigger_id = id.clone();
                            let condition_id = tr::ConditionId::from(&condition);
                            let executable_id = tr::ExecutableId::from(&executable);

                            for entry in [
                                node!(Trigger, trigger_id.clone(), trigger),
                                node!(
                                    Condition,
                                    condition_id.clone(),
                                    condition
                                ),
                                node!(
                                    Executable,
                                    executable_id.clone(),
                                    executable
                                ),
                                node!(
                                    TriggerCondition,
                                    trigger_id.clone(),
                                    condition_id,
                                    state::tr::UnitV
                                ),
                                node!(
                                    TriggerExecutable,
                                    trigger_id.clone(),
                                    executable_id,
                                    state::tr::UnitV
                                ),
                                node!(
                                    TriggerAdmin,
                                    trigger_id,
                                    action.authority.signatory.clone(),
                                    action.authority.domain.clone(),
                                    state::tr::UnitV
                                ),
                            ] {
                                res.insert(entry);
                            }
                        })
                }
                _ => unimplemented!("no use for now"),
            }
        }
        res
    }
}
