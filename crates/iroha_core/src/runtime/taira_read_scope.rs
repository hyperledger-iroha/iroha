//! Single-purpose four-seat scheduling of the scoped-read permission after Taira's keyed cutover.

use iroha_crypto::PublicKey;
use iroha_data_model::{
    NetworkId, ValidationFail,
    isi::{error::InstructionExecutionError, taira_read_scope::ScheduleTairaReadScopeV1},
    runtime::{
        definition::{DefinitionLookup, RootSelectionV1},
        lifecycle::{ChangeSetId, ChangeSetStatusV1, ChangeSetV1, RuntimeUpgradeActionV1},
        taira_read_scope::{KEYED_PREDECESSOR, SCOPED_SUCCESSOR, TairaReadScopeCertificateV1},
    },
};
use iroha_model_base::{chain::ChainId, state_path::StatePath};
use mv::storage::StorageReadOnly;

use super::{
    change_set::PublishedDefinitions, state_scope_permission, taira_activation::AdmittedSchedule,
};
use crate::{
    smartcontracts::Execute,
    state::{StateTransaction, WorldReadOnly},
};

fn certificate_path() -> StatePath {
    "runtime/taira_read_scope/certificate"
        .parse()
        .expect("fixed certificate path")
}

fn certificate(world: &impl WorldReadOnly) -> Result<Option<TairaReadScopeCertificateV1>, String> {
    world
        .smart_contract_state()
        .get(&certificate_path())
        .map(|bytes| TairaReadScopeCertificateV1::from_bytes(bytes).map_err(|e| e.to_string()))
        .transpose()
}

fn expected_change_set(
    current: &RootSelectionV1,
    published: &impl DefinitionLookup,
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, String> {
    if current.id().map_err(|e| e.to_string())? != KEYED_PREDECESSOR {
        return Err(
            "requires the exact selection from Taira's certified keyed-State cutover".into(),
        );
    }
    if height < 2 || qualification == [0; 32] {
        return Err("a qualified successor height is required".into());
    }
    let change =
        state_scope_permission::activation(current, published, network, height, qualification)
            .map_err(|e| e.to_string())?;
    if change.publish.as_slice() != [state_scope_permission::descriptor()]
        || change.successor != SCOPED_SUCCESSOR
    {
        return Err(
            "the newly published scoped-read permission must produce its exact successor".into(),
        );
    }
    Ok(change)
}

/// Draft the sole change set this boundary accepts, from the pinned predecessor exported by the
/// operator. The predecessor's immutable dependencies are resolved with the release catalog.
/// Core independently resolves the same dependencies from committed State when executing.
///
/// # Errors
/// The pinned selection is not release R's keyed successor or the change does not apply.
pub fn draft(
    current: &RootSelectionV1,
    network: NetworkId,
    height: u64,
    qualification: [u8; 32],
) -> Result<ChangeSetV1, String> {
    use iroha_data_model::runtime::definition::{DefinitionDescriptorV1, DefinitionId};
    let permission = state_scope_permission::descriptor()
        .id()
        .map_err(|e| e.to_string())?;
    struct Predecessor<'a> {
        catalog: &'a super::ReleaseCatalog,
        unpublished: DefinitionId,
    }
    impl DefinitionLookup for Predecessor<'_> {
        fn definition(&self, id: &DefinitionId) -> Option<&DefinitionDescriptorV1> {
            if *id == self.unpublished {
                return None;
            }
            self.catalog.definitions.get(id)
        }
    }
    let catalog = super::release_catalog().map_err(|e| e.to_string())?;
    expected_change_set(
        current,
        &Predecessor {
            catalog,
            unpublished: permission,
        },
        network,
        height,
        qualification,
    )
}

pub(super) fn validate_context(
    received: &TairaReadScopeCertificateV1,
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    network: NetworkId,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> Result<[u8; 32], ValidationFail> {
    let reject = |reason: String| {
        ValidationFail::NotPermitted(format!("Taira read-scope activation: {reason}"))
    };
    received.verify().map_err(|e| reject(e.to_string()))?;
    let plan = &received.plan;
    if &plan.chain_id != chain_id || plan.network_id != network || committee != plan.committee {
        return Err(reject(
            "deployment or ordered validator roster differs".into(),
        ));
    }
    if now_ms == 0
        || now_ms > plan.expires_at_unix_ms
        || plan
            .change_set
            .effective_height
            .checked_sub(height)
            .is_none_or(|distance| distance < plan.notice.blocks)
    {
        return Err(reject(
            "certificate expired or insufficient scheduling notice".into(),
        ));
    }
    let exact = expected_change_set(
        world.runtime_selection(),
        &PublishedDefinitions(world.runtime_definitions()),
        network,
        plan.change_set.effective_height,
        plan.change_set.qualification,
    )
    .map_err(&reject)?;
    if plan.change_set != exact {
        return Err(reject(
            "change set differs from the sole authorized permission activation".into(),
        ));
    }
    if let Some(previous) = certificate(world).map_err(&reject)? {
        let id = previous
            .plan
            .change_set
            .id()
            .map_err(|e| reject(e.to_string()))?;
        if !world
            .runtime_change_sets()
            .get(&id)
            .is_some_and(|record| matches!(record.status, ChangeSetStatusV1::Lapsed(_)))
        {
            return Err(reject("already scheduled".into()));
        }
    }
    plan.id().map_err(|e| reject(e.to_string()))
}

pub(super) fn attests_retained(world: &impl WorldReadOnly, target: ChangeSetId) -> bool {
    certificate(world)
        .ok()
        .flatten()
        .is_some_and(|certificate| {
            certificate
                .plan
                .change_set
                .id()
                .is_ok_and(|id| id == target)
        })
}

impl Execute for ScheduleTairaReadScopeV1 {
    fn execute(
        self,
        authority: &iroha_data_model::account::AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let reject = |reason: String| {
            InstructionExecutionError::InvariantViolation(
                format!("Taira scoped-read schedule: {reason}").into(),
            )
        };
        let plan = &self.certificate.plan;
        let id = plan.id().map_err(|e| reject(e.to_string()))?;
        if state.taira_activation_entrypoint != Some(AdmittedSchedule::ReadScope(id)) {
            return Err(reject("requires the exact certified transaction".into()));
        }
        let now = super::change_set::block_ms(state);
        super::change_set::enact(
            state,
            &iroha_data_model::governance::types::RuntimeUpgradeProposal {
                proposal_operator: authority.clone(),
                action: RuntimeUpgradeActionV1::Schedule(plan.change_set.clone()),
                notice: plan.notice,
                proposed_ms: now,
            },
        )?;
        let bytes = self
            .certificate
            .to_bytes()
            .map_err(|e| reject(e.to_string()))?;
        state
            .world
            .smart_contract_state
            .insert(certificate_path(), bytes);
        Ok(())
    }
}
