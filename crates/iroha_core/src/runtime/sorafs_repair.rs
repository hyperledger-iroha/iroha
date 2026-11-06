//! Explicit owner-authorized Taira boundary; not an extension of the predecessor kernel.
//!
//! The single direct transaction retains the complete all-four certificate in signed history.
//! Its transaction overlay selects the separately identified successor before ordinary envelope
//! admission, fees and execution. Any rejection discards that entire overlay. The native
//! handler then applies the one hash-bound initialization under the newly selected definition.
//! No opaque carrier, configuration switch, genesis import or general governance grant exists.

use super::change_set::PublishedDefinitions;
use crate::state::{NativeExecutionTip, StateReadOnly, StateTransaction, StateView, WorldReadOnly};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    NetworkId, ValidationFail,
    isi::sorafs_repair::ApplySorafsRepairV1,
    runtime::{definition::RootSelectionV1, sorafs_repair::SorafsRepairCertificateV1},
    transaction::{Executable, SignedTransaction},
};
use iroha_model_base::chain::ChainId;

/// Owner authorization retained with the deployment evidence, SHA-256 of its exact JSON bytes.
pub const AUTHORIZATION_DIGEST: [u8; 32] = [
    0x73, 0x28, 0xa7, 0x4e, 0xeb, 0x0f, 0x3c, 0xe2, 0x2a, 0xb9, 0x58, 0xd6, 0x43, 0x41, 0x59, 0x50,
    0x7f, 0x40, 0x52, 0xe4, 0xa0, 0x29, 0x14, 0xd0, 0xa0, 0xb7, 0x5b, 0x91, 0x4a, 0x13, 0x85, 0xc1,
];
const TAIRA_NETWORK: [u8; 32] = [
    0x49, 0x78, 0xd0, 0xbd, 0x15, 0xfc, 0x32, 0xb8, 0x34, 0x67, 0x2b, 0x27, 0xbb, 0xd8, 0xa0, 0x36,
    0x53, 0xb1, 0xdf, 0x06, 0x0a, 0x1e, 0xee, 0x7e, 0x93, 0x74, 0xd2, 0x9a, 0xf3, 0x88, 0x6d, 0x91,
];
const TAIRA_CHAIN: &str = "fc56984b-2be7-431d-840e-21514d1883f0";

fn rejected(reason: impl std::fmt::Display) -> ValidationFail {
    ValidationFail::NotPermitted(format!("Taira SoraFS repair rejected: {reason}"))
}

pub(crate) fn sole_repair(executable: &Executable) -> Option<&ApplySorafsRepairV1> {
    let Executable::Instructions(instructions) = executable else {
        return None;
    };
    if instructions.len() != 1 {
        return None;
    }
    instructions[0]
        .as_any()
        .downcast_ref::<ApplySorafsRepairV1>()
}

/// Authenticate the exact parent and permitted successor without changing State.
fn validate_boundary(
    certificate: &SorafsRepairCertificateV1,
    world: &impl WorldReadOnly,
    tip: Option<NativeExecutionTip>,
    chain_id: &ChainId,
    network: NetworkId,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> Result<RootSelectionV1, ValidationFail> {
    if chain_id.to_string() != TAIRA_CHAIN || network.as_bytes() != &TAIRA_NETWORK {
        return Err(rejected(
            "owner authorization is for this exact Taira network only",
        ));
    }
    validate_context(
        certificate,
        world,
        tip,
        chain_id,
        network,
        height,
        now_ms,
        committee,
    )
}

#[allow(clippy::too_many_arguments)]
fn validate_context(
    certificate: &SorafsRepairCertificateV1,
    world: &impl WorldReadOnly,
    tip: Option<NativeExecutionTip>,
    chain_id: &ChainId,
    network: NetworkId,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> Result<RootSelectionV1, ValidationFail> {
    certificate.verify().map_err(rejected)?;
    let plan = &certificate.plan;
    if plan.authorization_digest != AUTHORIZATION_DIGEST
        || &plan.chain_id != chain_id
        || plan.network_id != network
    {
        return Err(rejected("owner authorization, chain or network differs"));
    }
    let tip = tip.ok_or_else(|| rejected("authenticated native parent is absent"))?;
    if tip.root_scope() != iroha_data_model::block::consensus::SumeragiRootScope::Global
        || tip.height() != plan.parent_height
        || tip.core_hash().0 != plan.parent_block_hash
        || tip.result().0 != plan.parent_result_hash
        || tip.height().checked_add(1) != Some(height)
        || plan.change_set.effective_height != height
        || committee != plan.committee
    {
        return Err(rejected(
            "certified parent, height or ordered committee changed",
        ));
    }
    if now_ms == 0 || now_ms < tip.creation_time_ms() || now_ms > plan.expires_at_unix_ms {
        return Err(rejected("certificate is expired or block time is invalid"));
    }
    if plan.payload.providers.len() != 3 {
        return Err(rejected("authorization requires exactly three providers"));
    }
    if crate::smartcontracts::isi::sorafs_bootstrap::read_sorafs_bootstrap_record_v1(world)
        .map_err(rejected)?
        .is_some()
    {
        return Err(rejected("initialization was already consumed"));
    }
    let catalog = super::release_catalog().map_err(rejected)?;
    let published = PublishedDefinitions(world.runtime_definitions());
    let exact = catalog
        .sorafs_repair_activation(
            world.runtime_selection(),
            &published,
            network,
            height,
            plan.change_set.qualification,
        )
        .map_err(rejected)?;
    if exact != plan.change_set {
        return Err(rejected("change set exceeds the exact repair scope"));
    }
    Ok(exact
        .apply(world.runtime_selection(), &published)
        .map_err(rejected)?
        .selection)
}

/// Queue-only admissibility. A valid certificate permits this exact new envelope to reach the
/// execution boundary; ordinary signature, authority, fees and resource checks still apply.
pub(crate) fn queue_admits(view: &StateView<'_>, signed: &SignedTransaction) -> bool {
    let Some(repair) = sole_repair(signed.instructions()) else {
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
        &repair.certificate,
        view.world(),
        Some(tip),
        &view.chain_id,
        view.network_id,
        height,
        tip.creation_time_ms(),
        &committee,
    )
    .is_ok()
}

/// Enter the authorized successor in this signed transaction's rollback-owned overlay.
/// Every non-repair transaction follows the predecessor path unchanged. This must run before
/// envelope admission; it is explicitly the exceptional kernel boundary, not a dispatch bypass.
pub(crate) fn prepare_signed_boundary(
    state: &mut StateTransaction<'_, '_>,
    signed: &SignedTransaction,
) -> Result<(), ValidationFail> {
    let Some(repair) = sole_repair(signed.instructions()) else {
        return Ok(());
    };
    // Once bound, this is ordinary selected dispatch. During the second admission call of
    // the same authorized transaction the original marker remains in the same overlay.
    // A later transaction has no marker and the selected handler refuses it normally.
    if state
        .world
        .runtime_selection()
        .bindings
        .binary_search_by(|binding| binding.wire_id.as_str().cmp(ApplySorafsRepairV1::WIRE_ID))
        .is_ok()
    {
        return Ok(());
    }
    let certificate = &repair.certificate;
    let qualification = (|| {
        if crate::executor::root_scope::execution_root_scope(state)?
            != iroha_data_model::block::consensus::SumeragiRootScope::Global
        {
            return Err(rejected("repair requires the universal execution root"));
        }
        let id = certificate.plan.id().map_err(rejected)?;
        let roster = state
            .threshold_key_lifecycle_frozen_roster_v1()
            .map_err(rejected)?;
        let committee: Vec<_> = roster
            .iter()
            .map(|peer| peer.public_key().clone())
            .collect();
        let successor = validate_boundary(
            certificate,
            &*state.world,
            state.native_execution_tip(),
            &state.chain_id,
            state.network_id,
            state.block_height(),
            state.block_unix_timestamp_ms(),
            &committee,
        )?;
        let definitions = certificate
            .plan
            .change_set
            .publish
            .iter()
            .map(|descriptor| Ok((descriptor.id().map_err(rejected)?, descriptor.clone())))
            .collect::<Result<Vec<_>, ValidationFail>>()?;
        Ok((id, successor, definitions))
    })();
    let (id, successor, definitions) = match qualification {
        Ok(_) if state.is_execution_deferred() => {
            return Err(ValidationFail::InternalError(
                "local SoraFS repair boundary did not complete".into(),
            ));
        }
        Ok(qualified) => qualified,
        // Keep the original local-refusal owner in its existing non-wire latch. The outer
        // execution owner moves it once; no error-string reconstruction or latch reset.
        Err(error) if state.is_execution_deferred() => return Err(error),
        // An invalid proposal is still the predecessor's unbound envelope. Its ordinary
        // admission result must equal that of a release which kept this new payload opaque.
        Err(_) => return Ok(()),
    };
    // All validation and descriptor identities are complete before the first write.
    // These publications and the handler effects share the original rollback scope.
    for (definition_id, descriptor) in definitions {
        state
            .world
            .runtime_definitions
            .insert(definition_id, descriptor);
    }
    *state.world.runtime_selection.get_mut() = successor;
    state.sorafs_repair_entrypoint = Some(id);
    Ok(())
}

/// Test fixtures exercise the same certificate/parent/selection validation on a native synthetic
/// chain. Only the two deployment identity constants differ; no production caller accepts them.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_fixture_boundary(
    certificate: &SorafsRepairCertificateV1,
    world: &impl WorldReadOnly,
    tip: Option<NativeExecutionTip>,
    chain_id: &ChainId,
    network: NetworkId,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> Result<RootSelectionV1, ValidationFail> {
    validate_context(
        certificate,
        world,
        tip,
        chain_id,
        network,
        height,
        now_ms,
        committee,
    )
}

#[cfg(test)]
mod tests;
