//! Child runtime foundation selected solely from the original signed-genesis recipe.
//!
//! Ordinary parameter writes never enter this module. Source validation and candidate construction
//! finish before the combined genesis installer mutates its pristine unpublished overlay.

use std::collections::BTreeMap;

use iroha_data_model::{
    NetworkId,
    block::{BlockHeader, SignedBlock, consensus::SumeragiRootScope},
    isi::SetParameter,
    parameter::{CustomParameter, Parameter},
    runtime::{
        child_admin::{
            ChildAdminErrorV1, ChildRootIdentityV1, ChildRuntimeAdminStateV1,
            foundation::ChildRuntimeFoundationV1,
        },
        definition::{
            DefinitionDescriptorV1, DefinitionId, RootSelectionV1, SELECTION_ID_DOMAIN, SelectionId,
        },
    },
    transaction::Executable,
};

use super::{AuthenticatedGenesisOutputSource, BlockValidationError, InvalidGenesisError};
use crate::state::{State, StateBlock};

/// Private selection of the exact original signed recipe, retained through genesis execution.
#[derive(Debug)]
pub(crate) struct GenesisChildRuntime {
    header: BlockHeader,
    coordinate: (usize, usize),
    parameter: CustomParameter,
    recipe: ChildRuntimeFoundationV1,
    scope: SumeragiRootScope,
}

/// Candidate prepared before the first pristine-overlay write. It exposes no public constructor.
pub(super) struct PreparedChildRuntime {
    selection: RootSelectionV1,
    definitions: BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    authority: ChildRuntimeAdminStateV1,
}

fn invalid(message: impl Into<String>) -> BlockValidationError {
    InvalidGenesisError::CryptoPolicy(format!("child runtime foundation: {}", message.into()))
        .into()
}

fn decoding(error: norito::core::DecodeAttemptError) -> BlockValidationError {
    use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
    match canonical_decode_attempt_error(error, |error| invalid(error.to_string())) {
        ExecutionAttemptError::Rejected(error) => error,
        ExecutionAttemptError::Deferred(original) => {
            BlockValidationError::ExecutionDeferred(original)
        }
    }
}

fn child_admin_error(error: ChildAdminErrorV1) -> BlockValidationError {
    match error {
        ChildAdminErrorV1::Resource(original) => decoding(original),
        ChildAdminErrorV1::CryptoUnavailable(original) => BlockValidationError::ExecutionDeferred(
            ivm::ExecutionDeferral::CryptoImplementationUnavailable(original).into(),
        ),
        ChildAdminErrorV1::Json(original) => {
            use crate::execution_attempt::{ExecutionAttemptError, json_decode_attempt_error};
            match json_decode_attempt_error(original, |error| invalid(error.to_string())) {
                ExecutionAttemptError::Rejected(error) => error,
                ExecutionAttemptError::Deferred(original) => {
                    BlockValidationError::ExecutionDeferred(original)
                }
            }
        }
        error => invalid(error.to_string()),
    }
}

fn signed_scope(block: &SignedBlock) -> Result<SumeragiRootScope, BlockValidationError> {
    // Keep the typed signed-body reader's original JSON refusal inside its classifier.
    // The convenience iroha_genesis wrapper erases this distinction into eyre text.
    let metadata = norito::core::classify_decode_attempt(|| {
        iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(block).map_err(
            |error| match error {
                iroha_data_model::sumeragi_finality::GenesisReadError::Json(original) => {
                    original.into_core_error()
                }
                iroha_data_model::sumeragi_finality::GenesisReadError::Invalid(reason) => {
                    norito::Error::Message(reason)
                }
            },
        )
    })
    .map_err(decoding)?;
    Ok(metadata.sumeragi_context.root_scope)
}

fn selection_identity(selection: &RootSelectionV1) -> Result<SelectionId, BlockValidationError> {
    // Use the frozen selection preimage/domain while preserving the original local refusal;
    // RootSelectionV1::id's public SelectionError::Encoding stores only a diagnostic string.
    norito::core::classify_decode_attempt(|| {
        let mut encoded = Ok(());
        let hash = iroha_crypto::Hash::new_from_writer(|writer| {
            writer.write_all(SELECTION_ID_DOMAIN)?;
            encoded = norito::core::write_canonical_to_writer(selection, writer);
            Ok(())
        })
        .map_err(norito::Error::Io)?;
        encoded?;
        Ok(SelectionId(hash.into()))
    })
    .map_err(decoding)
}

impl AuthenticatedGenesisOutputSource {
    /// Select an optional new-child recipe after configured-root signature authentication.
    pub(super) fn initial_child_runtime(
        &self,
        block: &SignedBlock,
    ) -> Result<Option<GenesisChildRuntime>, BlockValidationError> {
        self.account_for(block).map_err(invalid)?;
        let mut selected = None;
        for (tx_index, transaction) in block.external_transactions().enumerate() {
            let Executable::Instructions(instructions) = transaction.instructions() else {
                continue;
            };
            for (isi_index, instruction) in instructions.iter().enumerate() {
                let Some(set) = instruction.as_any().downcast_ref::<SetParameter>() else {
                    continue;
                };
                let Parameter::Custom(custom) = set.inner() else {
                    continue;
                };
                if custom.id() != ChildRuntimeFoundationV1::parameter_id() {
                    continue;
                }
                if selected.is_some() {
                    return Err(invalid("repeated original recipe"));
                }
                selected = Some(((tx_index, isi_index), custom));
            }
        }
        let Some((coordinate, parameter)) = selected else {
            return Ok(None);
        };
        let recipe = ChildRuntimeFoundationV1::from_custom_parameter(parameter)
            .map_err(child_admin_error)?;
        let scope = signed_scope(block)?;
        if !matches!(scope, SumeragiRootScope::Dataspace { .. }) {
            return Err(invalid(
                "a global genesis cannot select child administrator authority",
            ));
        }
        Ok(Some(GenesisChildRuntime {
            header: block.header(),
            coordinate,
            parameter: parameter.clone(),
            recipe,
            scope,
        }))
    }
}

impl GenesisChildRuntime {
    /// Admit only the unchanged parameter in its original authenticated genesis transaction.
    /// The installed authority and scoped transaction capability must still share this source.
    pub(crate) fn require_parameter(
        &self,
        state: &crate::state::StateTransaction<'_, '_>,
        parameter: &CustomParameter,
    ) -> Result<(), String> {
        if state._curr_block != self.header
            || !state.block_hashes.is_empty()
            || state.current_entrypoint_index != u64::try_from(self.coordinate.0).ok()
            || parameter != &self.parameter
            || state
                .genesis_execution_scope
                .as_ref()
                .and_then(|scope| scope.for_transaction(state))
                != Some(self.scope)
        {
            return Err("child foundation parameter lost its original signed transaction".into());
        }
        let Some(authority) = state.world.runtime_child_admin_state.get().as_ref() else {
            return Err("child foundation parameter has no installed initial authority".into());
        };
        let instance = self
            .scope
            .instance_id(
                &crate::sumeragi::crypto::BlsCrypto::new(),
                state.network_id,
                state.chain_id.as_str(),
            )
            .map_err(|error| error.to_string())?
            .0;
        if authority.root.network != state.network_id
            || authority.root.network != NetworkId::from_genesis_hash(self.header.hash())
            || authority.root.scope() != self.scope
            || authority.root.instance != instance
            || authority.policy != self.recipe.policy
            || authority.next_nonce != 0
        {
            return Err("child foundation parameter differs from its installed authority".into());
        }
        Ok(())
    }

    fn root(&self, state: &StateBlock<'_>) -> Result<ChildRootIdentityV1, BlockValidationError> {
        let SumeragiRootScope::Dataspace {
            parent_network_id,
            dataspace_id,
        } = self.scope
        else {
            return Err(invalid("foundation source has no child scope"));
        };
        let instance = self
            .scope
            .instance_id(
                &crate::sumeragi::crypto::BlsCrypto::new(),
                state.network_id,
                state.chain_id.as_str(),
            )
            .map_err(|error| invalid(error.to_string()))?
            .0;
        let root = ChildRootIdentityV1 {
            network: state.network_id,
            parent_network: parent_network_id,
            dataspace: dataspace_id,
            instance,
        };
        root.validate().map_err(child_admin_error)?;
        Ok(root)
    }

    /// Revalidate original source and pristine owner before preparing any selected State changes.
    pub(super) fn prepare_installation(
        &self,
        block: &SignedBlock,
        state: &StateBlock<'_>,
        original: &State,
        generation: u64,
    ) -> Result<PreparedChildRuntime, BlockValidationError> {
        block.validate_proposal_commitments().map_err(invalid)?;
        state
            .require_original_execution_recorder()
            .map_err(invalid)?;
        state
            .validate_native_pristine_control_owner(original, generation, &self.header)
            .map_err(invalid)?;
        if block.header() != self.header
            || state._curr_block != self.header
            || !state.block_hashes.is_empty()
            || state.network_id != NetworkId::from_genesis_hash(block.hash())
            || state.world.runtime_child_admin_state.get().is_some()
        {
            return Err(invalid(
                "source, pristine child identity or empty authority changed",
            ));
        }
        let source = block
            .external_transactions()
            .nth(self.coordinate.0)
            .and_then(|tx| match tx.instructions() {
                Executable::Instructions(instructions) => instructions.get(self.coordinate.1),
                _ => None,
            })
            .and_then(|instruction| instruction.as_any().downcast_ref::<SetParameter>());
        if !matches!(source.map(SetParameter::inner), Some(Parameter::Custom(custom)) if custom == &self.parameter)
        {
            return Err(invalid("original signed recipe changed"));
        }
        let catalog =
            crate::runtime::release_catalog().map_err(|error| invalid(error.to_string()))?;
        if *state.world.runtime_selection.get() != catalog.baseline {
            return Err(invalid(
                "foundation cannot replace an already selected runtime",
            ));
        }
        let (selection, definitions) = catalog
            .child_foundation()
            .map_err(|error| invalid(error.to_string()))?;
        if selection_identity(&selection)? != self.recipe.selection {
            return Err(invalid(
                "signed foundation selection differs from the compiled recipe",
            ));
        }
        let authority = ChildRuntimeAdminStateV1 {
            root: self.root(state)?,
            policy: self.recipe.policy.clone(),
            next_nonce: 0,
        };
        authority.validate().map_err(child_admin_error)?;
        Ok(PreparedChildRuntime {
            selection,
            definitions,
            authority,
        })
    }

    /// Genesis instructions may not replace the signed foundation or mutate its initial authority.
    pub(crate) fn require_unchanged(
        &self,
        state: &StateBlock<'_>,
    ) -> Result<(), BlockValidationError> {
        if state._curr_block != self.header
            || selection_identity(state.world.runtime_selection.get())? != self.recipe.selection
            || state.world.runtime_child_admin_state.get().as_ref()
                != Some(&ChildRuntimeAdminStateV1 {
                    root: self.root(state)?,
                    policy: self.recipe.policy.clone(),
                    next_nonce: 0,
                })
            || state
                .world
                .parameters
                .get()
                .custom()
                .get(ChildRuntimeFoundationV1::parameter_id())
                != Some(&self.parameter)
        {
            return Err(invalid(
                "executed genesis changed its signed child foundation",
            ));
        }
        Ok(())
    }
}

impl PreparedChildRuntime {
    /// Publish only the already authenticated and fully checked pristine candidate.
    pub(super) fn install(self, state: &mut StateBlock<'_>) {
        for (id, descriptor) in self.definitions {
            state.world.runtime_definitions.insert(id, descriptor);
        }
        *state.world.runtime_selection.get_mut() = self.selection;
        *state.world.runtime_child_admin_state.get_mut() = Some(self.authority);
    }
}

#[cfg(test)]
mod tests;
