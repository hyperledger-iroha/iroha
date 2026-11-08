//! Explicit owner-authorized Taira activation of Time and Ironwood; not a general grant.
//!
//! The certificate takes the place of Parliament's approval and of nothing else. One direct
//! transaction carrying it records the exact compiled change set as `Scheduled` through the path
//! Parliament enactment uses. Every validator still attests, and `change_set::activate_due` still
//! applies the change set at the start of its effective height. The retained certificate then
//! authorizes exactly one Ironwood pool, through the ordinary bootstrap instruction, once
//! (`specs/runtime/taira_time_ironwood_v1.md`).

use iroha_crypto::PublicKey;
use iroha_data_model::{
    NetworkId, ValidationFail,
    isi::{
        error::InstructionExecutionError, runtime_upgrade::AttestRuntimeSupportV1,
        taira_activation::ScheduleTairaTimeIronwoodV1,
    },
    privacy::PrivacyIronwoodPoolBootstrapV1,
    runtime::{
        lifecycle::{ChangeSetStatusV1, RuntimeUpgradeActionV1},
        taira_activation::TairaActivationCertificateV1,
    },
    transaction::{Executable, SignedTransaction},
};
use iroha_model_base::{chain::ChainId, state_path::StatePath};
use mv::storage::StorageReadOnly;

use super::{
    change_set::PublishedDefinitions,
    sorafs_repair::{TAIRA_CHAIN, TAIRA_NETWORK},
};
use crate::{
    smartcontracts::Execute,
    state::{StateReadOnly, StateTransaction, StateView, WorldReadOnly},
};

/// Owner authorization addendum retained with the deployment evidence, SHA-256 of its exact JSON
/// bytes. It names the digest of the authorization it amends.
pub const AUTHORIZATION_DIGEST: [u8; 32] = [
    0xb6, 0xc4, 0x2a, 0x28, 0x3b, 0x1d, 0xb2, 0x97, 0x99, 0x4a, 0xa1, 0xdd, 0xab, 0xd0, 0x76, 0xaa,
    0xb4, 0x5f, 0xdc, 0x8f, 0xec, 0x4c, 0x26, 0x8d, 0xd2, 0x81, 0xad, 0x04, 0xf5, 0xfd, 0x47, 0x71,
];

fn rejected(reason: impl std::fmt::Display) -> ValidationFail {
    ValidationFail::NotPermitted(format!("Taira activation rejected: {reason}"))
}

fn certificate_path() -> StatePath {
    "runtime/taira_time_ironwood/certificate"
        .parse()
        .expect("fixed activation receipt path")
}

/// The retained certificate, if this chain scheduled the activation.
///
/// # Errors
/// A retained receipt is not an exact canonical, fully signed certificate.
pub(crate) fn retained_certificate(
    world: &impl WorldReadOnly,
) -> Result<Option<TairaActivationCertificateV1>, String> {
    world
        .smart_contract_state()
        .get(&certificate_path())
        .map(|bytes| TairaActivationCertificateV1::from_bytes(bytes).map_err(|e| e.to_string()))
        .transpose()
}

pub(crate) fn sole_schedule(executable: &Executable) -> Option<&ScheduleTairaTimeIronwoodV1> {
    let Executable::Instructions(instructions) = executable else {
        return None;
    };
    if instructions.len() != 1 {
        return None;
    }
    instructions[0]
        .as_any()
        .downcast_ref::<ScheduleTairaTimeIronwoodV1>()
}

/// Authenticate the exact deployment, roster, selection and change set without changing State.
fn validate_boundary(
    certificate: &TairaActivationCertificateV1,
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    network: NetworkId,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> Result<[u8; 32], ValidationFail> {
    if !is_taira(chain_id, network) {
        return Err(rejected(
            "owner authorization is for this exact Taira network only",
        ));
    }
    validate_context(
        certificate,
        world,
        chain_id,
        network,
        height,
        now_ms,
        committee,
    )
}

fn is_taira(chain_id: &ChainId, network: NetworkId) -> bool {
    #[cfg(test)]
    if fixture_deployment::accepts(chain_id, network) {
        return true;
    }
    chain_id.to_string() == TAIRA_CHAIN && network.as_bytes() == &TAIRA_NETWORK
}

/// Test seam for signed execution on a native synthetic chain. It changes only the two deployment
/// identity constants; no production caller reaches it.
#[cfg(test)]
pub(crate) mod fixture_deployment {
    use std::{collections::BTreeSet, sync::Mutex};

    use super::{ChainId, NetworkId};

    static ACCEPTED: Mutex<BTreeSet<(String, [u8; 32])>> = Mutex::new(BTreeSet::new());

    pub(crate) fn allow(chain_id: &ChainId, network: NetworkId) {
        ACCEPTED
            .lock()
            .expect("fixture deployments")
            .insert((chain_id.to_string(), *network.as_bytes()));
    }

    pub(super) fn accepts(chain_id: &ChainId, network: NetworkId) -> bool {
        ACCEPTED
            .lock()
            .expect("fixture deployments")
            .contains(&(chain_id.to_string(), *network.as_bytes()))
    }
}

fn validate_context(
    certificate: &TairaActivationCertificateV1,
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    network: NetworkId,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> Result<[u8; 32], ValidationFail> {
    certificate.verify().map_err(rejected)?;
    let plan = &certificate.plan;
    if plan.authorization_digest != AUTHORIZATION_DIGEST
        || &plan.chain_id != chain_id
        || plan.network_id != network
    {
        return Err(rejected("owner authorization, chain or network differs"));
    }
    if committee != plan.committee {
        return Err(rejected("the ordered scheduled committee changed"));
    }
    if now_ms == 0 || now_ms > plan.expires_at_unix_ms {
        return Err(rejected("certificate is expired or block time is invalid"));
    }
    if plan.change_set.effective_height < height.saturating_add(plan.notice.blocks) {
        return Err(rejected("the change set takes effect before its notice"));
    }
    // A lapsed attempt selected nothing, so a new certificate may replace its receipt. Every other
    // recorded outcome ends this authorization.
    if let Some(retained) = retained_certificate(world).map_err(rejected)? {
        let id = retained.plan.change_set.id().map_err(rejected)?;
        let lapsed = world
            .runtime_change_sets()
            .get(&id)
            .is_some_and(|record| matches!(record.status, ChangeSetStatusV1::Lapsed(_)));
        if !lapsed {
            return Err(rejected("the activation was already scheduled"));
        }
    }
    let published = PublishedDefinitions(world.runtime_definitions());
    let exact = super::ironwood::taira_time_ironwood_change_set(
        network,
        plan.change_set.effective_height,
        plan.change_set.qualification,
        world.runtime_selection(),
        &published,
    )
    .map_err(rejected)?;
    if exact != plan.change_set {
        return Err(rejected("change set exceeds the exact authorized scope"));
    }
    plan.id().map_err(rejected)
}

/// Queue-only admissibility. A valid certificate permits this exact new envelope to reach the
/// execution boundary; ordinary signature, authority, fees and resource checks still apply.
pub(crate) fn queue_admits(view: &StateView<'_>, signed: &SignedTransaction) -> bool {
    let Some(schedule) = sole_schedule(signed.instructions()) else {
        return false;
    };
    let Some(tip) = view.native_execution_tip() else {
        return false;
    };
    let Some(height) = tip.height().checked_add(1) else {
        return false;
    };
    let Ok(roster) = crate::state::threshold_key_lifecycle_roster_v1(view.world(), height) else {
        return false;
    };
    let committee: Vec<_> = roster
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect();
    validate_boundary(
        &schedule.certificate,
        view.world(),
        &view.chain_id,
        view.network_id,
        height,
        tip.creation_time_ms(),
        &committee,
    )
    .is_ok()
}

/// Admit the exact certified envelope in this signed transaction's rollback-owned overlay.
/// Every other transaction follows its ordinary path unchanged. An invalid certificate stays the
/// unbound envelope it is on a release that kept this payload opaque. This must run before
/// envelope admission; it is explicitly the exceptional kernel boundary, not a dispatch bypass.
pub(crate) fn prepare_signed_boundary(
    state: &mut StateTransaction<'_, '_>,
    signed: &SignedTransaction,
) -> Result<(), ValidationFail> {
    let Some(schedule) = sole_schedule(signed.instructions()) else {
        return Ok(());
    };
    // The second admission call of the same transaction finds the marker in the same overlay.
    if state.taira_activation_entrypoint.is_some() {
        return Ok(());
    }
    let qualification = (|| {
        if crate::executor::root_scope::execution_root_scope(state)?
            != iroha_data_model::block::consensus::SumeragiRootScope::Global
        {
            return Err(rejected("activation requires the universal execution root"));
        }
        let roster = state
            .threshold_key_lifecycle_frozen_roster_v1()
            .map_err(rejected)?;
        let committee: Vec<_> = roster
            .iter()
            .map(|peer| peer.public_key().clone())
            .collect();
        validate_boundary(
            &schedule.certificate,
            &*state.world,
            &state.chain_id,
            state.network_id,
            state.block_height(),
            state.block_unix_timestamp_ms(),
            &committee,
        )
    })();
    match qualification {
        Ok(_) if state.is_execution_deferred() => Err(ValidationFail::InternalError(
            "local Taira activation boundary did not complete".into(),
        )),
        Ok(id) => {
            state.taira_activation_entrypoint = Some(id);
            Ok(())
        }
        // Keep the original local-refusal owner in its existing non-wire latch.
        Err(error) if state.is_execution_deferred() => Err(error),
        Err(_) => Ok(()),
    }
}

/// Whether the certified boundary admitted `instruction` in this transaction, in place of the
/// binding no selection gives it.
pub(crate) fn admitted_by_boundary(
    state: &StateTransaction<'_, '_>,
    instruction: &iroha_data_model::isi::InstructionBox,
) -> bool {
    state.taira_activation_entrypoint.is_some()
        && instruction
            .as_any()
            .downcast_ref::<ScheduleTairaTimeIronwoodV1>()
            .is_some()
}

impl Execute for ScheduleTairaTimeIronwoodV1 {
    fn execute(
        self,
        authority: &iroha_data_model::account::AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let invalid = |reason: &str| {
            InstructionExecutionError::InvariantViolation(
                format!("Taira activation rejected: {reason}").into(),
            )
        };
        let plan = &self.certificate.plan;
        let id = plan.id().map_err(|error| invalid(&error.to_string()))?;
        if state.taira_activation_entrypoint != Some(id) {
            return Err(invalid("schedule requires the exact certified boundary"));
        }
        let now_ms = super::change_set::block_ms(state);
        super::change_set::enact(
            state,
            &iroha_data_model::governance::types::RuntimeUpgradeProposal {
                proposal_operator: authority.clone(),
                action: RuntimeUpgradeActionV1::Schedule(plan.change_set.clone()),
                notice: plan.notice,
                proposed_ms: now_ms,
            },
        )?;
        // Retain the complete certificate as the once-only authority receipt, independently of
        // the signed transaction's membership in history.
        let frame = self
            .certificate
            .to_bytes()
            .map_err(|error| invalid(&error.to_string()))?;
        state
            .world
            .smart_contract_state
            .insert(certificate_path(), frame);
        Ok(())
    }
}

/// Whether `signed` does nothing but attest the change set this chain's retained certificate
/// scheduled.
///
/// The universal root's runtime-upgrade lane policy asks for a Parliament module, and this
/// deployment's consensus configuration never named one. The certificate takes Parliament's place
/// for this one change set, and nothing else: the attestation is still verified against the
/// committee at activation. Without a retained certificate every release refuses the same way.
pub(crate) fn attests_certified_change_set(
    state: &StateTransaction<'_, '_>,
    signed: &SignedTransaction,
) -> bool {
    let Executable::Instructions(instructions) = signed.instructions() else {
        return false;
    };
    if instructions.len() != 1 {
        return false;
    }
    let Some(attest) = instructions[0]
        .as_any()
        .downcast_ref::<AttestRuntimeSupportV1>()
    else {
        return false;
    };
    let Ok(Some(certificate)) = retained_certificate(&*state.world) else {
        return false;
    };
    certificate
        .plan
        .change_set
        .id()
        .is_ok_and(|id| id == attest.attestation.attestation.change_set)
}

/// Whether the retained certificate authorizes bootstrapping exactly `pool` now: the certified
/// change set activated and its successor is still selected. Anything else leaves the ordinary
/// permission check in force. A second bootstrap of the pool refuses as already initialized.
pub(crate) fn certified_pool_bootstrap(
    state: &StateTransaction<'_, '_>,
    pool: &PrivacyIronwoodPoolBootstrapV1,
) -> bool {
    let Ok(Some(certificate)) = retained_certificate(&*state.world) else {
        return false;
    };
    let plan = certificate.plan;
    if plan.pool != *pool || plan.network_id != state.network_id || plan.chain_id != state.chain_id
    {
        return false;
    }
    let Ok(id) = plan.change_set.id() else {
        return false;
    };
    let activated = state
        .world
        .runtime_change_sets
        .get(&id)
        .is_some_and(|record| matches!(record.status, ChangeSetStatusV1::Activated(_)));
    activated
        && state
            .world
            .runtime_selection
            .get()
            .id()
            .is_ok_and(|selection| selection == plan.change_set.successor)
}

/// Test fixtures exercise the same certificate and selection validation on a native synthetic
/// chain. Only the two deployment identity constants differ; no production caller accepts them.
#[cfg(test)]
pub(crate) fn validate_fixture_boundary(
    certificate: &TairaActivationCertificateV1,
    world: &impl WorldReadOnly,
    chain_id: &ChainId,
    network: NetworkId,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> Result<[u8; 32], ValidationFail> {
    validate_context(
        certificate,
        world,
        chain_id,
        network,
        height,
        now_ms,
        committee,
    )
}

#[cfg(test)]
mod tests;
