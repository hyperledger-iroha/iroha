//! Named host-profile restrictions at the actual execution height.
//!
//! Prepared code and cache residency identify a profile; only the execution root authorizes
//! using it. A captured obligation lends its original profile only inside its native scope.

use iroha_data_model::{
    ValidationFail,
    isi::error::InstructionExecutionError,
    runtime::{
        definition::{DefinitionDescriptorV1, DefinitionId, RestrictionV1, RootSelectionV1},
        lifecycle::RuntimeRejectionV1,
        obligation::CompletionCaptureV1,
    },
};
use mv::storage::StorageReadOnly as _;

use crate::{
    runtime::obligations::Step,
    state::{StateTransaction, WorldReadOnly},
};

/// Check new work or a read-only invocation at its authoritative height.
/// Baseline ABI V1 retains its original admission rules.
pub(crate) fn ensure_at_height(
    world: &impl WorldReadOnly,
    policy: ivm::SyscallPolicy,
    height: u64,
) -> Result<(), ValidationFail> {
    ensure(world, policy, height, |_| None)
}

/// Check one mutable invocation using only its actual scoped obligation custody.
/// Deferred bodies cannot borrow an enclosing completion's restriction exemption.
pub(crate) fn ensure_in_transaction(
    transaction: &StateTransaction<'_, '_>,
    policy: ivm::SyscallPolicy,
) -> Result<(), ValidationFail> {
    ensure(
        transaction.world(),
        policy,
        transaction.block_height(),
        |profile| captured_step(transaction, profile),
    )
}

/// Admit newly captured named profiles before registration or funding creates an obligation.
///
/// A broad capture currently retains every selected host profile. Consequently a closed named
/// profile also refuses an otherwise unrelated new obligation retaining it. This conservative
/// rule prevents a new registration from manufacturing a completion exemption after closure;
/// captures that do not retain this named profile and the baseline profile stay unchanged.
/// The caller retains the original typed restriction rejection without formatting it away.
pub(crate) fn ensure_new_capture(
    world: &impl WorldReadOnly,
    capture: &CompletionCaptureV1,
    height: u64,
) -> Result<(), InstructionExecutionError> {
    let hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1RuntimeLifecycle);
    for id in &capture.host_profiles {
        if matches!(world.runtime_definitions().get(id),
            Some(DefinitionDescriptorV1::HostProfile(profile))
                if profile.abi_version == 1 && profile.abi_hash == hash)
        {
            check_restriction(world.runtime_selection(), *id, height, None)?;
        }
    }
    Ok(())
}

fn captured_step(transaction: &StateTransaction<'_, '_>, profile: DefinitionId) -> Option<Step> {
    if !transaction.deferred_expectations.is_empty() {
        return None;
    }
    // Native custody is authoritative even when registered consent also surrounds it.
    if let Some(native) = &transaction.native_execution {
        return native
            .capture
            .host_profiles
            .binary_search(&profile)
            .is_ok()
            .then_some(native.step);
    }
    transaction
        .completing_obligation
        .as_ref()
        .and_then(|capture| {
            capture
                .host_profiles
                .binary_search(&profile)
                .is_ok()
                .then_some(Step::Complete)
        })
}

fn ensure(
    world: &impl WorldReadOnly,
    policy: ivm::SyscallPolicy,
    height: u64,
    step: impl Fn(DefinitionId) -> Option<Step>,
) -> Result<(), ValidationFail> {
    if policy == ivm::SyscallPolicy::AbiV1 {
        return Ok(());
    }
    let hash = ivm::syscalls::compute_abi_hash(policy);
    let selection = world.runtime_selection();
    let mut selected = false;
    for id in &selection.active {
        if !matches!(world.runtime_definitions().get(id),
            Some(DefinitionDescriptorV1::HostProfile(profile))
                if profile.abi_version == 1 && profile.abi_hash == hash)
        {
            continue;
        }
        selected = true;
        check_restriction(selection, *id, height, step(*id))
            .map_err(ValidationFail::InstructionFailed)?;
    }
    if !selected {
        return Err(ValidationFail::NotPermitted(
            "contract host profile is not selected by this execution root".into(),
        ));
    }
    Ok(())
}

fn check_restriction(
    selection: &RootSelectionV1,
    profile: DefinitionId,
    height: u64,
    step: Option<Step>,
) -> Result<(), InstructionExecutionError> {
    let Ok(index) = selection
        .restrictions
        .binary_search_by(|entry| entry.definition.cmp(&profile))
    else {
        return Ok(());
    };
    let restriction = selection.restrictions[index].restriction;
    let effective = match restriction {
        RestrictionV1::Deprecated(_) => false,
        RestrictionV1::AdmissionClosed(from)
        | RestrictionV1::CompletionOnly(from)
        | RestrictionV1::Disabled(from) => from <= height,
    };
    if effective && !step.is_some_and(|step| step.runs_under(restriction)) {
        return Err(RuntimeRejectionV1::DefinitionRestricted {
            definition: profile,
            restriction,
        }
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::{
        block::BlockHeader,
        isi::{InstructionBox, Log},
        level::Level,
        runtime::{definition::RestrictionEntryV1, obligation::CompletionCaptureV1},
    };

    const NAMED: ivm::SyscallPolicy = ivm::SyscallPolicy::AbiV1RuntimeLifecycle;

    fn state() -> crate::state::State {
        let state = crate::state::State::new_for_testing(
            crate::state::World::new(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        );
        for module in [
            crate::runtime::catalog::additions::RUNTIME_LIFECYCLE.module,
            crate::runtime::catalog::additions::CONTRACT_LIFECYCLE.module,
        ] {
            crate::runtime::activate_additions_for_testing(&state.world, module).unwrap();
        }
        state
    }

    fn profile(world: &impl WorldReadOnly) -> DefinitionId {
        let hash = ivm::syscalls::compute_abi_hash(NAMED);
        *world
            .runtime_selection()
            .active
            .iter()
            .find(|id| {
                matches!(world.runtime_definitions().get(id),
                Some(DefinitionDescriptorV1::HostProfile(profile)) if profile.abi_hash == hash)
            })
            .unwrap()
    }

    fn restrict(transaction: &mut StateTransaction<'_, '_>, kind: RestrictionV1) -> DefinitionId {
        let id = profile(transaction.world());
        transaction.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
            definition: id,
            restriction: kind,
        }];
        id
    }

    #[test]
    fn named_restrictions_use_the_effective_height_and_leave_baseline_unchanged() {
        let state = state();
        let header = BlockHeader::new(std::num::NonZeroU64::new(5).unwrap(), None, None, 1_000, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        for restriction in [
            RestrictionV1::AdmissionClosed(5),
            RestrictionV1::CompletionOnly(5),
            RestrictionV1::Disabled(5),
        ] {
            restrict(&mut transaction, restriction);
            assert!(ensure_at_height(transaction.world(), NAMED, 4).is_ok());
            assert!(ensure_at_height(transaction.world(), NAMED, 5).is_err());
            assert!(ensure_in_transaction(&transaction, NAMED).is_err());
            assert!(ensure_at_height(transaction.world(), ivm::SyscallPolicy::AbiV1, 5).is_ok());
        }
        restrict(&mut transaction, RestrictionV1::Deprecated(5));
        assert!(ensure_at_height(transaction.world(), NAMED, 5).is_ok());
        assert!(ensure_at_height(&crate::state::World::new().view(), NAMED, 5).is_err());
    }

    #[test]
    fn new_captures_never_manufacture_exemptions_after_named_profile_closure() {
        let state = state();
        let header = BlockHeader::new(std::num::NonZeroU64::new(5).unwrap(), None, None, 1_000, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let instruction: InstructionBox =
            Log::new(Level::INFO, "new profile capture".to_owned()).into();
        let capture = CompletionCaptureV1::capture(
            transaction.world.runtime_selection.canonical(),
            &crate::runtime::change_set::PublishedDefinitions(
                &transaction.world.runtime_definitions,
            ),
            &[iroha_data_model::isi::instruction_wire_id(&instruction)
                .unwrap()
                .to_owned()],
            5,
        )
        .unwrap();
        for restriction in [
            RestrictionV1::AdmissionClosed(5),
            RestrictionV1::CompletionOnly(5),
            RestrictionV1::Disabled(5),
        ] {
            let id = restrict(&mut transaction, restriction);
            assert!(ensure_new_capture(transaction.world(), &capture, 4).is_ok());
            assert!(ensure_new_capture(transaction.world(), &capture, 5).is_err());
            let mut unrelated = capture.clone();
            unrelated.host_profiles.retain(|profile| *profile != id);
            assert!(ensure_new_capture(transaction.world(), &unrelated, 5).is_ok());
        }
        restrict(&mut transaction, RestrictionV1::Deprecated(5));
        assert!(ensure_new_capture(transaction.world(), &capture, 5).is_ok());
        let named = profile(transaction.world());
        let baseline = *capture
            .host_profiles
            .iter()
            .find(|id| **id != named)
            .unwrap();
        transaction.world.runtime_selection.get_mut().restrictions = vec![RestrictionEntryV1 {
            definition: baseline,
            restriction: RestrictionV1::Disabled(5),
        }];
        assert!(ensure_new_capture(transaction.world(), &capture, 5).is_ok());
    }

    #[test]
    fn only_original_profile_capture_and_native_step_can_cross_closed_admission() {
        let state = state();
        let header = BlockHeader::new(std::num::NonZeroU64::new(5).unwrap(), None, None, 1_000, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        let instruction: InstructionBox =
            Log::new(Level::INFO, "capture profile".to_owned()).into();
        let capture = CompletionCaptureV1::capture(
            transaction.world.runtime_selection.canonical(),
            &crate::runtime::change_set::PublishedDefinitions(
                &transaction.world.runtime_definitions,
            ),
            &[iroha_data_model::isi::instruction_wire_id(&instruction)
                .unwrap()
                .to_owned()],
            5,
        )
        .unwrap();
        let id = restrict(&mut transaction, RestrictionV1::AdmissionClosed(5));
        assert!(capture.host_profiles.contains(&id));
        transaction.completing_obligation = Some(capture.clone());
        assert!(ensure_in_transaction(&transaction, NAMED).is_ok());
        let mut foreign = capture.clone();
        foreign.host_profiles.retain(|profile| *profile != id);
        transaction.completing_obligation = Some(foreign);
        assert!(ensure_in_transaction(&transaction, NAMED).is_err());
        transaction.completing_obligation = Some(capture.clone());
        transaction.deferred_expectations.push(None);
        assert!(ensure_in_transaction(&transaction, NAMED).is_err());
        transaction.deferred_expectations.clear();
        transaction.native_execution = Some(crate::runtime::execution::NativeExecution {
            capture: capture.clone(),
            step: Step::Continue,
        });
        assert!(ensure_in_transaction(&transaction, NAMED).is_ok());
        restrict(&mut transaction, RestrictionV1::CompletionOnly(5));
        assert!(ensure_in_transaction(&transaction, NAMED).is_err());
        transaction.native_execution.as_mut().unwrap().step = Step::Complete;
        assert!(ensure_in_transaction(&transaction, NAMED).is_ok());
        restrict(&mut transaction, RestrictionV1::Disabled(5));
        assert!(ensure_in_transaction(&transaction, NAMED).is_err());
    }
}
