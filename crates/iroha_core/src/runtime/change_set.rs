//! Governed change sets: admission, validator attestation and activation
//! (`specs/live_runtime_upgrades.md` §6).
//!
//! SORA Parliament approves a change set for the universal root. Approval schedules it at its
//! effective height with the notice policy frozen at proposal time. Validators then attest, each
//! with its own key, that their release executes the exact successor selection. At the start of
//! the effective height the change set activates only if every member of the committee attested
//! for that committee, its notice elapsed in committed time and it still applies to the current
//! selection; otherwise it lapses and never applies. Both outcomes are deterministic functions of
//! committed State and the block header, so every node reaches the same selection.

use std::collections::BTreeMap;

use iroha_crypto::PublicKey;
use iroha_data_model::{
    isi::error::InstructionExecutionError as Error,
    parameter::{
        runtime_provenance_policy::RuntimeProvenancePolicyV1,
        runtime_upgrade_policy::RuntimeUpgradePolicyV1,
    },
    runtime::{
        definition::{DefinitionDescriptorV1, DefinitionId, DefinitionLookup, RootSelectionV1},
        lifecycle::{
            ChangeSetEndorsementV1, ChangeSetId, ChangeSetRecordV1, ChangeSetStatusV1, ChangeSetV1,
            RootScopeV1, RuntimeNoticeV1, RuntimeUpgradeActionV1, SelectionTransitionV1,
            SignedSupportAttestationV1, check_change_set_provenance, committee_digest,
            committee_supports,
        },
    },
};
use mv::storage::StorageReadOnly;

/// The definitions a World has published, as a [`DefinitionLookup`].
pub(crate) struct PublishedDefinitions<'a, S>(pub(crate) &'a S);

impl<S> DefinitionLookup for PublishedDefinitions<'_, S>
where
    S: StorageReadOnly<DefinitionId, DefinitionDescriptorV1>,
{
    fn definition(&self, id: &DefinitionId) -> Option<&DefinitionDescriptorV1> {
        self.0.get(id)
    }
}

/// Whether `change_set` restricts or removes anything, which takes the longer disable notice.
#[must_use]
pub(crate) fn needs_disable_notice(change_set: &ChangeSetV1) -> bool {
    change_set.transitions.iter().any(|transition| {
        matches!(
            transition,
            SelectionTransitionV1::Restrict { .. }
                | SelectionTransitionV1::Deactivate(_)
                | SelectionTransitionV1::Unbind(_)
                | SelectionTransitionV1::RetireStorage(_)
        )
    })
}

/// The notice `change_set` must give under `policy`.
#[must_use]
pub(crate) fn notice_for(
    policy: &RuntimeUpgradePolicyV1,
    change_set: &ChangeSetV1,
) -> RuntimeNoticeV1 {
    if needs_disable_notice(change_set) {
        policy.disable
    } else {
        policy.activation
    }
}

/// The selection a change set effective at `height` will meet: `current` advanced by every
/// scheduled change set due before `height`, in effective-height then identity order.
///
/// # Errors
/// A scheduled change set no longer applies to the selection it would meet.
pub(crate) fn projected_selection<S>(
    current: &RootSelectionV1,
    scheduled: &BTreeMap<ChangeSetId, ChangeSetRecordV1>,
    height: u64,
    published: &PublishedDefinitions<'_, S>,
) -> Result<
    (
        RootSelectionV1,
        BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    ),
    Error,
>
where
    S: StorageReadOnly<DefinitionId, DefinitionDescriptorV1>,
{
    let mut due: Vec<(&ChangeSetId, &ChangeSetRecordV1)> = scheduled
        .iter()
        .filter(|(_, record)| {
            record.status == ChangeSetStatusV1::Scheduled
                && record.change_set.effective_height < height
        })
        .collect();
    due.sort_by_key(|(id, record)| (record.change_set.effective_height, **id));
    let mut selection = current.clone();
    let mut added = BTreeMap::new();
    for (id, record) in due {
        let lookup = Staged {
            published,
            added: &added,
        };
        let applied = record
            .change_set
            .apply(&selection, &lookup)
            .map_err(|error| {
                Error::InvariantViolation(
                    format!("scheduled change set {id} no longer applies: {error}").into(),
                )
            })?;
        added.extend(applied.published);
        selection = applied.selection;
    }
    Ok((selection, added))
}

/// Published definitions plus those scheduled change sets will publish first.
struct Staged<'a, 'p, S> {
    published: &'a PublishedDefinitions<'p, S>,
    added: &'a BTreeMap<DefinitionId, DefinitionDescriptorV1>,
}

impl<S> DefinitionLookup for Staged<'_, '_, S>
where
    S: StorageReadOnly<DefinitionId, DefinitionDescriptorV1>,
{
    fn definition(&self, id: &DefinitionId) -> Option<&DefinitionDescriptorV1> {
        self.added.get(id).or_else(|| self.published.definition(id))
    }
}

/// Check a proposed or approved change set (§6): it targets this network's universal root,
/// gives at least `notice.blocks` of notice from `height`, and applies to the selection it will
/// meet, producing its declared closed successor.
///
/// A release does not need to execute the successor to admit it: nodes store and authenticate
/// definitions they cannot execute, and only activation needs every validator's support.
///
/// # Errors
/// The first rule the change set violates.
pub(crate) fn admit<S>(
    change_set: &ChangeSetV1,
    network: &iroha_data_model::NetworkId,
    height: u64,
    notice: RuntimeNoticeV1,
    current: &RootSelectionV1,
    scheduled: &BTreeMap<ChangeSetId, ChangeSetRecordV1>,
    published: &PublishedDefinitions<'_, S>,
) -> Result<ChangeSetId, Error>
where
    S: StorageReadOnly<DefinitionId, DefinitionDescriptorV1>,
{
    if change_set.network != *network {
        return Err(Error::InvariantViolation(
            "the change set belongs to another network".into(),
        ));
    }
    if change_set.root != RootScopeV1::Universal {
        // TODO(live_runtime_upgrades.md §11): independently rooted data spaces own their own
        // selections; this release implements only the universal root.
        return Err(Error::InvariantViolation(
            "this release implements change sets of the universal root only".into(),
        ));
    }
    if change_set.effective_height < height.saturating_add(notice.blocks) {
        return Err(Error::InvariantViolation(
            format!(
                "the change set takes effect before its notice of {} blocks",
                notice.blocks
            )
            .into(),
        ));
    }
    let id = change_set
        .id()
        .map_err(|error| Error::Conversion(error.to_string()))?;
    successor(change_set, id, current, scheduled, published)?;
    Ok(id)
}

/// The successor selection `change_set` produces: applied to the selection it will meet,
/// `current` advanced by every scheduled change set due before its effective height.
///
/// # Errors
/// A scheduled change set or `change_set` itself no longer applies.
pub(crate) fn successor<S>(
    change_set: &ChangeSetV1,
    id: ChangeSetId,
    current: &RootSelectionV1,
    scheduled: &BTreeMap<ChangeSetId, ChangeSetRecordV1>,
    published: &PublishedDefinitions<'_, S>,
) -> Result<RootSelectionV1, Error>
where
    S: StorageReadOnly<DefinitionId, DefinitionDescriptorV1>,
{
    let (meets, staged) =
        projected_selection(current, scheduled, change_set.effective_height, published)?;
    let lookup = Staged {
        published,
        added: &staged,
    };
    change_set
        .apply(&meets, &lookup)
        .map(|applied| applied.selection)
        .map_err(|error| Error::InvariantViolation(format!("change set {id}: {error}").into()))
}

/// Check one validator's attestation of a scheduled change set (§6, "Attest").
///
/// # Errors
/// The record is not scheduled, the attestation names another change set, selection, network
/// or root, its validator is not a current committee member, it is bound to another committee,
/// it no longer counts at the effective height, or its signature does not verify.
pub(crate) fn check_attestation(
    record: &ChangeSetRecordV1,
    id: ChangeSetId,
    signed: &SignedSupportAttestationV1,
    committee: &[PublicKey],
) -> Result<(), Error> {
    let invalid = |reason: &str| Error::InvariantViolation(format!("attestation: {reason}").into());
    if record.status != ChangeSetStatusV1::Scheduled {
        return Err(invalid("the change set is not scheduled"));
    }
    let attestation = &signed.attestation;
    let change_set = &record.change_set;
    if attestation.change_set != id
        || attestation.predecessor != change_set.predecessor
        || attestation.successor != change_set.successor
        || attestation.network != change_set.network
        || attestation.root != change_set.root
    {
        return Err(invalid("it names another change set or selection"));
    }
    if !committee.contains(&attestation.validator) {
        return Err(invalid("its validator is not in the current committee"));
    }
    let digest = committee_digest(committee).map_err(|error| invalid(&error.to_string()))?;
    if attestation.committee != digest {
        return Err(invalid("it is bound to another committee"));
    }
    if !attestation.counts_for(id, change_set.effective_height, digest) {
        return Err(invalid("it does not count at the effective height"));
    }
    signed
        .verify()
        .map_err(|_| invalid("the signature does not verify"))
}

/// Record `signed`, replacing an earlier attestation of the same validator; attestations stay
/// sorted by validator key.
pub(crate) fn record_attestation(
    record: &mut ChangeSetRecordV1,
    signed: SignedSupportAttestationV1,
) {
    let validator = signed.attestation.validator.clone();
    match record
        .attestations
        .binary_search_by(|existing| existing.attestation.validator.cmp(&validator))
    {
        Ok(index) => record.attestations[index] = signed,
        Err(index) => record.attestations.insert(index, signed),
    }
}

/// Whether a scheduled change set activates at `height`: every committee member attested for
/// this committee, and the frozen notice elapsed by the block's committed time `now_ms`.
#[must_use]
pub(crate) fn ready(
    record: &ChangeSetRecordV1,
    id: ChangeSetId,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> bool {
    record.status == ChangeSetStatusV1::Scheduled
        && record.change_set.effective_height == height
        && record.notice_elapsed(now_ms)
        && committee_supports(committee, &record.attestations, id, height)
}

/// Activate or lapse every change set due at the start of `height` (§6, "Scheduling").
///
/// Change sets due at one height apply in identity order, each to the selection the previous
/// one left. A change set that is not ready or no longer applies lapses and never applies; a
/// scheduled change set whose height already passed lapses too. Every outcome is a function of
/// committed State, the block's committed time `now_ms` and the committee.
///
/// # Errors
/// This release cannot execute the resulting selection. The node stops before executing the
/// block instead of judging it (§6, "Stop the unsupported").
pub(crate) fn activate_due(
    wtx: &mut crate::state::WorldTransaction<'_, '_>,
    height: u64,
    now_ms: u64,
    committee: &[PublicKey],
) -> Result<(), crate::state::StateStorageAdmissionError> {
    use iroha_data_model::events::data::runtime_upgrade::{
        RuntimeUpgradeActivated, RuntimeUpgradeEvent, RuntimeUpgradeLapsed,
    };
    let mut due: Vec<(u64, ChangeSetId)> = wtx
        .runtime_change_sets
        .iter()
        .filter(|(_, record)| {
            record.status == ChangeSetStatusV1::Scheduled
                && record.change_set.effective_height <= height
        })
        .map(|(id, record)| (record.change_set.effective_height, *id))
        .collect();
    due.sort_unstable();
    let holding = super::containment::holding(wtx);
    for (_, id) in due {
        let Some(mut record) = wtx.runtime_change_sets.get(&id).cloned() else {
            continue;
        };
        let applied = if ready(&record, id, height, now_ms, committee) {
            let current = wtx.runtime_selection.get().clone();
            record
                .change_set
                .apply(&current, &PublishedDefinitions(&wtx.runtime_definitions))
                .ok()
        } else {
            None
        };
        let event = if let Some(applied) = applied {
            for (definition, descriptor) in applied.published {
                wtx.runtime_definitions.insert(definition, descriptor);
            }
            *wtx.runtime_selection.get_mut() = applied.selection;
            record.status = ChangeSetStatusV1::Activated(height);
            RuntimeUpgradeEvent::Activated(RuntimeUpgradeActivated {
                id,
                height,
                successor: record.change_set.successor,
            })
        } else {
            record.status = ChangeSetStatusV1::Lapsed(height);
            RuntimeUpgradeEvent::Lapsed(RuntimeUpgradeLapsed { id, height })
        };
        wtx.runtime_change_sets.insert(id, record);
        wtx.emit_events(Some(event));
    }
    super::containment::resolve_taken_over(wtx, height, &holding);
    let catalog = crate::runtime::release_catalog().map_err(|error| {
        crate::state::StateStorageAdmissionError::UnsupportedRuntime {
            height,
            reason: error.to_string(),
        }
    })?;
    let selection = wtx.runtime_selection.get();
    if let Some(definition) = catalog.unsupported(selection) {
        let named = selection.id().map_or_else(
            |error| format!("whose identity cannot be encoded ({error})"),
            |id| id.to_string(),
        );
        return Err(
            crate::state::StateStorageAdmissionError::UnsupportedRuntime {
                height,
                reason: format!(
                    "selection {named} relies on definition {definition}, which this release does \
                     not implement"
                ),
            },
        );
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;

/// The committed notice policy of the universal root.
///
/// # Errors
/// The policy is not committed or is malformed; there is no default (§6, "Notice").
pub(crate) fn committed_policy(
    parameters: &iroha_data_model::parameter::Parameters,
) -> Result<RuntimeUpgradePolicyV1, Error> {
    let custom = parameters
        .custom()
        .get(RuntimeUpgradePolicyV1::parameter_id())
        .ok_or_else(|| {
            Error::InvariantViolation(
                "the runtime upgrade policy is not committed; no change set is admitted".into(),
            )
        })?;
    RuntimeUpgradePolicyV1::from_custom_parameter(custom)
        .map_err(|error| Error::InvariantViolation(error.to_string().into()))
}

/// The committed provenance policy of the universal root, if one is committed (§6, "Who
/// governs").
///
/// # Errors
/// The committed policy is malformed; it never falls back to no requirement.
pub(crate) fn committed_provenance(
    parameters: &iroha_data_model::parameter::Parameters,
) -> Result<Option<RuntimeProvenancePolicyV1>, Error> {
    parameters
        .custom()
        .get(RuntimeProvenancePolicyV1::parameter_id())
        .map(|custom| {
            RuntimeProvenancePolicyV1::from_custom_parameter(custom)
                .map_err(|error| Error::InvariantViolation(error.to_string().into()))
        })
        .transpose()
}

/// Check the endorsements a runtime-upgrade proposal carries against the committed provenance
/// policy (§6, "Who governs"). A proposal to schedule a change set needs valid endorsements of its
/// identity from `threshold` distinct trusted release signers, and carries at most one
/// endorsement per trusted signer. Without a committed policy, and for a cancellation, which
/// publishes nothing, a proposal carries no endorsements.
///
/// # Errors
/// The policy is malformed, the proposal carries endorsements nothing judges or more than the
/// policy trusts signers, the change set has no identity, or the endorsements fall short.
pub(crate) fn check_provenance(
    action: &RuntimeUpgradeActionV1,
    provenance: &[ChangeSetEndorsementV1],
    parameters: &iroha_data_model::parameter::Parameters,
) -> Result<(), Error> {
    let (RuntimeUpgradeActionV1::Schedule(change_set), Some(policy)) =
        (action, committed_provenance(parameters)?)
    else {
        return if provenance.is_empty() {
            Ok(())
        } else {
            Err(Error::InvariantViolation(
                "endorsements accompany only a change set a committed provenance policy judges"
                    .into(),
            ))
        };
    };
    if provenance.len() > policy.signers.len() {
        return Err(Error::InvariantViolation(
            format!(
                "{} endorsements exceed the {} trusted release signers",
                provenance.len(),
                policy.signers.len()
            )
            .into(),
        ));
    }
    let id = change_set
        .id()
        .map_err(|error| Error::Conversion(error.to_string()))?;
    let threshold = usize::try_from(policy.threshold)
        .map_err(|_| Error::InvariantViolation("the provenance threshold exceeds usize".into()))?;
    check_change_set_provenance(id, &policy.signers, threshold, provenance).map_err(|shortfall| {
        Error::InvariantViolation(format!("change set {id} provenance: {shortfall}").into())
    })
}

/// Committed time of the executing block, in milliseconds.
pub(super) fn block_ms(state_transaction: &crate::state::StateTransaction<'_, '_>) -> u64 {
    u64::try_from(state_transaction._curr_block.creation_time().as_millis()).unwrap_or(u64::MAX)
}

/// Scheduled change sets of the World, by identity.
fn scheduled_records(
    state_transaction: &crate::state::StateTransaction<'_, '_>,
) -> BTreeMap<ChangeSetId, ChangeSetRecordV1> {
    state_transaction
        .world
        .runtime_change_sets
        .iter()
        .filter(|(_, record)| record.status == ChangeSetStatusV1::Scheduled)
        .map(|(id, record)| (*id, record.clone()))
        .collect()
}

/// Admit a runtime-upgrade proposal to SORA Parliament: the notice the committed policy requires,
/// frozen for the proposal, and the committed time of admission. A change set to schedule needs
/// the endorsements the committed provenance policy requires.
///
/// # Errors
/// The policy is not committed, a change set to schedule fails admission or its provenance
/// falls short, or a change set to cancel is not scheduled.
pub(crate) fn admit_proposal(
    action: &RuntimeUpgradeActionV1,
    provenance: &[ChangeSetEndorsementV1],
    state_transaction: &crate::state::StateTransaction<'_, '_>,
) -> Result<(RuntimeNoticeV1, u64), Error> {
    let policy = committed_policy(crate::state::WorldReadOnly::parameters(
        &state_transaction.world,
    ))?;
    check_provenance(
        action,
        provenance,
        crate::state::WorldReadOnly::parameters(&state_transaction.world),
    )?;
    let now_ms = block_ms(state_transaction);
    match action {
        RuntimeUpgradeActionV1::Schedule(change_set) => {
            let notice = notice_for(&policy, change_set);
            admit(
                change_set,
                &state_transaction.network_id,
                state_transaction._curr_block.height().get(),
                notice,
                state_transaction.world.runtime_selection.get(),
                &scheduled_records(state_transaction),
                &PublishedDefinitions(&state_transaction.world.runtime_definitions),
            )?;
            Ok((notice, now_ms))
        }
        RuntimeUpgradeActionV1::Cancel(id) => {
            scheduled(state_transaction, id)?;
            Ok((policy.activation, now_ms))
        }
    }
}

fn scheduled(
    state_transaction: &crate::state::StateTransaction<'_, '_>,
    id: &ChangeSetId,
) -> Result<ChangeSetRecordV1, Error> {
    match state_transaction.world.runtime_change_sets.get(id) {
        Some(record) if record.status == ChangeSetStatusV1::Scheduled => Ok(record.clone()),
        _ => Err(Error::InvariantViolation(
            format!("change set {id} is not scheduled").into(),
        )),
    }
}

/// Enact a runtime-upgrade proposal SORA Parliament approved (§6, "Activate"): schedule its
/// change set with the notice frozen at proposal time, or cancel a scheduled one.
///
/// # Errors
/// The change set no longer applies to the selection it will meet, its effective height is not
/// after the approving block, it is already recorded, or the change set to cancel is not
/// scheduled.
pub(crate) fn enact(
    state_transaction: &mut crate::state::StateTransaction<'_, '_>,
    proposal: &iroha_data_model::governance::types::RuntimeUpgradeProposal,
) -> Result<(), Error> {
    use iroha_data_model::events::data::runtime_upgrade::{
        RuntimeUpgradeCanceled, RuntimeUpgradeEvent, RuntimeUpgradeScheduled,
    };
    let height = state_transaction._curr_block.height().get();
    let now_ms = block_ms(state_transaction);
    let event = match &proposal.action {
        RuntimeUpgradeActionV1::Schedule(change_set) => {
            // The block floor counted from the proposal; approval only needs a later height.
            let id = admit(
                change_set,
                &state_transaction.network_id,
                height,
                RuntimeNoticeV1 {
                    blocks: 1,
                    ..proposal.notice
                },
                state_transaction.world.runtime_selection.get(),
                &scheduled_records(state_transaction),
                &PublishedDefinitions(&state_transaction.world.runtime_definitions),
            )?;
            if state_transaction
                .world
                .runtime_change_sets
                .get(&id)
                .is_some()
            {
                return Err(Error::InvariantViolation(
                    format!("change set {id} is already recorded").into(),
                ));
            }
            state_transaction.world.runtime_change_sets.insert(
                id,
                ChangeSetRecordV1 {
                    change_set: change_set.clone(),
                    status: ChangeSetStatusV1::Scheduled,
                    proposed_ms: proposal.proposed_ms,
                    approved_height: height,
                    approved_ms: now_ms,
                    notice: proposal.notice,
                    attestations: Vec::new(),
                },
            );
            super::containment::record_review(&mut state_transaction.world, id, change_set, height);
            RuntimeUpgradeEvent::Scheduled(RuntimeUpgradeScheduled {
                id,
                effective_height: change_set.effective_height,
            })
        }
        RuntimeUpgradeActionV1::Cancel(id) => {
            let mut record = scheduled(state_transaction, id)?;
            record.status = ChangeSetStatusV1::Cancelled(height);
            state_transaction
                .world
                .runtime_change_sets
                .insert(*id, record);
            RuntimeUpgradeEvent::Canceled(RuntimeUpgradeCanceled { id: *id, height })
        }
    };
    state_transaction.world.emit_events(Some(event));
    Ok(())
}

/// Record one validator's support for a scheduled change set (§6, "Attest").
///
/// # Errors
/// See [`check_attestation`].
pub(crate) fn attest(
    state_transaction: &mut crate::state::StateTransaction<'_, '_>,
    signed: SignedSupportAttestationV1,
) -> Result<(), Error> {
    use iroha_data_model::events::data::runtime_upgrade::{
        RuntimeUpgradeAttested, RuntimeUpgradeEvent,
    };
    let id = signed.attestation.change_set;
    let mut record = scheduled(state_transaction, &id)?;
    let committee: Vec<PublicKey> = state_transaction
        .commit_topology()
        .get()
        .iter()
        .map(|peer| peer.public_key().clone())
        .collect();
    check_attestation(&record, id, &signed, &committee)?;
    let validator = signed.attestation.validator.clone();
    record_attestation(&mut record, signed);
    state_transaction
        .world
        .runtime_change_sets
        .insert(id, record);
    state_transaction
        .world
        .emit_events(Some(RuntimeUpgradeEvent::Attested(
            RuntimeUpgradeAttested { id, validator },
        )));
    Ok(())
}
