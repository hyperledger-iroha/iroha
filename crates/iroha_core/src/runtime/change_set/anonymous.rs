//! Admission and enactment of the separately selected anonymous Parliament runtime corridor.
//!
//! The installed proposal handlers keep their original wrappers. This owner retains canonical
//! codec refusals and uses the same transition rules with checked content identities.

use super::*;
use crate::{
    execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error},
    state::{StateTransaction, WorldReadOnly},
};
use iroha_data_model::{
    governance::types::{GovernanceExpectedHeadV1, RuntimeUpgradeProposal},
    runtime::lifecycle::{CheckedChangeSetErrorV1, checked_change_set_id_v1},
};
use iroha_primitives::json::Json;

type Attempt<T> = Result<T, ExecutionAttemptError<Error>>;

/// This corridor never borrows the administrative authority of an independent child root.
pub(crate) fn require_global(state: &StateTransaction<'_, '_>) -> Attempt<()> {
    let scope = crate::sumeragi::lanes::routing::read_routing_root_scope(&*state.world)
        .map_err(ExecutionAttemptError::Deferred)?;
    if scope != Some(iroha_data_model::block::consensus::SumeragiRootScope::Global) {
        return Err(invalid(
            "SORA Parliament runtime admission requires the authenticated global root",
        )
        .into());
    }
    Ok(())
}

fn invalid(reason: impl core::fmt::Display) -> Error {
    Error::InvariantViolation(reason.to_string().into())
}

fn codec<T>(operation: impl FnOnce() -> Result<T, norito::Error>) -> Attempt<T> {
    norito::core::classify_decode_attempt(operation)
        .map_err(|original| canonical_decode_attempt_error(original, invalid))
}

/// Decode exactly one bounded canonical policy while retaining its original codec refusal.
pub(crate) fn policy<
    T: norito::json::JsonDeserialize + norito::json::JsonSerialize + norito::FastJsonWrite,
>(
    parameter: &iroha_data_model::parameter::CustomParameter,
    max: usize,
) -> Attempt<T> {
    if parameter.payload().get().len() > max {
        return Err(invalid("committed runtime policy exceeds its byte bound").into());
    }
    let value: T = codec(|| {
        norito::json::from_str(parameter.payload().get())
            .map_err(norito::json::Error::into_core_error)
    })?;
    let canonical = codec(|| Json::try_new(&value))?;
    if &canonical != parameter.payload() {
        return Err(invalid("committed runtime policy is not canonical").into());
    }
    Ok(value)
}

fn id(change: &ChangeSetV1) -> Attempt<ChangeSetId> {
    checked_change_set_id_v1(change)
        .map_err(|original| canonical_decode_attempt_error(original, invalid))
}

fn apply_checked(
    change: &ChangeSetV1,
    predecessor: &RootSelectionV1,
    lookup: &impl DefinitionLookup,
) -> Attempt<AppliedChangeSet> {
    change
        .apply_checked_v1(predecessor, lookup)
        .map_err(|error| match error {
            CheckedChangeSetErrorV1::Rejected(reason) => {
                ExecutionAttemptError::Rejected(invalid(reason))
            }
            CheckedChangeSetErrorV1::Codec(original) => {
                canonical_decode_attempt_error(original, invalid)
            }
        })
}

fn transition(
    change: &ChangeSetV1,
    current: &RootSelectionV1,
    previous: &RootSelectionV1,
    applied: &AppliedChangeSet,
    published: &impl DefinitionLookup,
) -> Attempt<()> {
    struct Added<'a, L> {
        base: &'a L,
        added: &'a BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    }
    impl<L: DefinitionLookup> DefinitionLookup for Added<'_, L> {
        fn definition(&self, id: &DefinitionId) -> Option<&DefinitionDescriptorV1> {
            self.added.get(id).or_else(|| self.base.definition(id))
        }
    }
    crate::runtime::catalog::ivm_c10::validate_transition(previous, &applied.selection)
        .map_err(invalid)?;
    crate::runtime::keyed_state::checked::validate(
        previous,
        &applied.selection,
        &Added {
            base: published,
            added: &applied.published,
        },
    )
    .map_err(|error| match error {
        ExecutionAttemptError::Deferred(original) => ExecutionAttemptError::Deferred(original),
        ExecutionAttemptError::Rejected(reason) => ExecutionAttemptError::Rejected(invalid(reason)),
    })?;
    check_transition_support_after_keyed(change, current, previous, applied, published)?;
    Ok(())
}

/// The same universal-root, notice and successor rules as the installed admission owner,
/// with borrowed timeline records and original canonical writer refusals.
fn admit(
    change: &ChangeSetV1,
    notice: RuntimeNoticeV1,
    state: &StateTransaction<'_, '_>,
) -> Attempt<ChangeSetId> {
    if change.network != state.network_id || change.root != RootScopeV1::Universal {
        return Err(invalid("runtime proposal names another network or non-universal root").into());
    }
    if change.effective_height
        < state
            ._curr_block
            .height()
            .get()
            .saturating_add(notice.blocks)
    {
        return Err(invalid("runtime change set takes effect before its required notice").into());
    }
    let identity = id(change)?;
    let due = |record: &ChangeSetRecordV1| {
        record.status == ChangeSetStatusV1::Scheduled
            && record.change_set.effective_height < change.effective_height
    };
    let count = state
        .world
        .runtime_change_sets
        .iter()
        .filter(|(_, record)| due(record))
        .count();
    let mut records = codec(|| {
        let bytes = count
            .checked_mul(core::mem::size_of::<(&ChangeSetId, &ChangeSetRecordV1)>())
            .ok_or(norito::Error::LengthMismatch)?;
        norito::core::reserve_decode_allocation(bytes)?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(count)
            .map_err(|_| norito::Error::AllocationFailed {
                bytes: u64::try_from(bytes).unwrap_or(u64::MAX),
            })?;
        Ok(records)
    })?;
    records.extend(
        state
            .world
            .runtime_change_sets
            .iter()
            .filter(|(_, record)| due(record)),
    );
    records.sort_unstable_by_key(|(id, record)| (record.change_set.effective_height, **id));
    let current = state.world.runtime_selection.canonical();
    let mut meets = current.clone();
    let mut staged = BTreeMap::new();
    let published = PublishedDefinitions(&state.world.runtime_definitions);
    for (_, record) in records {
        let lookup = Staged {
            published: &published,
            added: &staged,
        };
        let applied = apply_checked(&record.change_set, &meets, &lookup)?;
        transition(&record.change_set, current, &meets, &applied, &lookup)?;
        staged.extend(applied.published);
        meets = applied.selection;
    }
    let lookup = Staged {
        published: &published,
        added: &staged,
    };
    let applied = apply_checked(change, &meets, &lookup)?;
    transition(change, current, &meets, &applied, &lookup)?;
    if crate::runtime::ironwood::enabled_with_definitions(&meets, &lookup) {
        let target = Staged {
            published: &published,
            added: &staged,
        };
        struct Added<'a, L> {
            base: L,
            added: &'a BTreeMap<DefinitionId, DefinitionDescriptorV1>,
        }
        impl<L: DefinitionLookup> DefinitionLookup for Added<'_, L> {
            fn definition(&self, id: &DefinitionId) -> Option<&DefinitionDescriptorV1> {
                self.added.get(id).or_else(|| self.base.definition(id))
            }
        }
        crate::runtime::ironwood::validate_admitted_successor(
            &applied.selection,
            &Added {
                base: target,
                added: &applied.published,
            },
        )
        .map_err(invalid)?;
    }
    Ok(identity)
}

fn check_provenance(
    change: ChangeSetId,
    policy: &RuntimeProvenancePolicyV1,
    endorsements: &[ChangeSetEndorsementV1],
) -> Attempt<()> {
    use iroha_data_model::runtime::lifecycle::CHANGE_SET_ENDORSEMENT_DOMAIN;
    let mut payload = [0; CHANGE_SET_ENDORSEMENT_DOMAIN.len() + 32];
    payload[..CHANGE_SET_ENDORSEMENT_DOMAIN.len()].copy_from_slice(CHANGE_SET_ENDORSEMENT_DOMAIN);
    payload[CHANGE_SET_ENDORSEMENT_DOMAIN.len()..].copy_from_slice(&change.0);
    let mut have = 0;
    for signer in &policy.signers {
        for endorsement in endorsements
            .iter()
            .filter(|entry| entry.change_set == change && &entry.signer == signer)
        {
            match endorsement.signature.verify(signer, &payload) {
                Ok(()) => {
                    have += 1;
                    break;
                }
                Err(iroha_crypto::Error::UnsupportedImplementation(original)) => {
                    return Err(ExecutionAttemptError::Deferred(
                        ivm::ExecutionDeferral::CryptoImplementationUnavailable(original).into(),
                    ));
                }
                Err(_) => {}
            }
        }
    }
    if have < policy.threshold as usize {
        return Err(
            invalid(iroha_data_model::runtime::lifecycle::ProvenanceShortfall {
                have,
                need: policy.threshold as usize,
            })
            .into(),
        );
    }
    Ok(())
}

/// Derive and freeze the original notice/time; release endorsements are always checked.
pub(crate) fn admit_proposal(
    action: &RuntimeUpgradeActionV1,
    provenance: &[ChangeSetEndorsementV1],
    state: &StateTransaction<'_, '_>,
) -> Attempt<(RuntimeNoticeV1, u64)> {
    require_global(state)?;
    let custom = state.world.parameters().custom();
    let notice: RuntimeUpgradePolicyV1 = policy(
        custom
            .get(RuntimeUpgradePolicyV1::parameter_id())
            .ok_or_else(|| invalid("the runtime upgrade policy is not committed"))?,
        RuntimeUpgradePolicyV1::MAX_PAYLOAD_BYTES,
    )?;
    notice.validate().map_err(invalid)?;
    let provenance_policy: Option<RuntimeProvenancePolicyV1> = custom
        .get(RuntimeProvenancePolicyV1::parameter_id())
        .map(|value| policy(value, RuntimeProvenancePolicyV1::MAX_PAYLOAD_BYTES))
        .transpose()?;
    if let Some(policy) = &provenance_policy {
        policy.validate().map_err(invalid)?;
    }
    match (action, provenance_policy) {
        (RuntimeUpgradeActionV1::Schedule(change), Some(policy)) => {
            if provenance.len() > policy.signers.len() {
                return Err(invalid("too many release endorsements").into());
            }
            check_provenance(id(change)?, &policy, provenance)?;
        }
        _ if !provenance.is_empty() => {
            return Err(invalid(
                "endorsements require a scheduled change and committed provenance policy",
            )
            .into());
        }
        _ => {}
    }
    let required = match action {
        RuntimeUpgradeActionV1::Schedule(change) => {
            let required = notice_for(&notice, change);
            admit(change, required, state)?;
            required
        }
        RuntimeUpgradeActionV1::Cancel(change) => {
            scheduled(state, change)?;
            notice.activation
        }
    };
    Ok((required, block_ms(state)))
}

/// Exact original subject/head, avoiding the installed action wrapper's erased ID error.
pub(crate) fn expected_head(
    proposal: &RuntimeUpgradeProposal,
    state: &StateTransaction<'_, '_>,
) -> Attempt<GovernanceExpectedHeadV1> {
    use iroha_data_model::governance::types::{
        GovernanceExpectedHeadAbsentV1, GovernanceExpectedHeadPresentV1,
        checked_runtime_upgrade_subject_id_v1,
    };
    let subject_id = checked_runtime_upgrade_subject_id_v1(&proposal.action)
        .map_err(|original| canonical_decode_attempt_error(original, invalid))?;
    let change = match &proposal.action {
        RuntimeUpgradeActionV1::Schedule(change) => id(change)?,
        RuntimeUpgradeActionV1::Cancel(id) => *id,
    };
    let Some(record) = state.world.runtime_change_sets.get(&change) else {
        return Ok(GovernanceExpectedHeadV1::Absent(
            GovernanceExpectedHeadAbsentV1 { subject_id },
        ));
    };
    let head_root = iroha_data_model::governance::checked_fingerprint(
        iroha_data_model::governance::PARLIAMENT_EXPECTED_HEAD_ROOT_V1,
        record,
    )
    .map_err(|original| canonical_decode_attempt_error(original, invalid))?;
    if head_root == [0; 32] {
        return Err(invalid("runtime subject head is inert").into());
    }
    Ok(GovernanceExpectedHeadV1::Present(
        GovernanceExpectedHeadPresentV1 {
            subject_id,
            version: 1,
            head_root,
        },
    ))
}

/// Apply only a certificate-authorized runtime effect; the caller owns atomic rollback.
pub(crate) fn enact(
    state: &mut StateTransaction<'_, '_>,
    proposal: &RuntimeUpgradeProposal,
) -> Attempt<()> {
    use iroha_data_model::events::data::runtime_upgrade::{
        RuntimeUpgradeCanceled, RuntimeUpgradeEvent, RuntimeUpgradeScheduled,
    };
    require_global(state)?;
    let height = state._curr_block.height().get();
    let now_ms = block_ms(state);
    let event = match &proposal.action {
        RuntimeUpgradeActionV1::Schedule(change) => {
            let id = admit(
                change,
                RuntimeNoticeV1 {
                    blocks: 1,
                    ..proposal.notice
                },
                state,
            )?;
            if state.world.runtime_change_sets.get(&id).is_some() {
                return Err(invalid("change set already recorded").into());
            }
            state.world.runtime_change_sets.insert(
                id,
                ChangeSetRecordV1 {
                    change_set: change.clone(),
                    status: ChangeSetStatusV1::Scheduled,
                    proposed_ms: proposal.proposed_ms,
                    approved_height: height,
                    approved_ms: now_ms,
                    notice: proposal.notice,
                    attestations: Vec::new(),
                },
            );
            crate::runtime::containment::record_review(&mut state.world, id, change, height);
            RuntimeUpgradeEvent::Scheduled(RuntimeUpgradeScheduled {
                id,
                effective_height: change.effective_height,
            })
        }
        RuntimeUpgradeActionV1::Cancel(id) => {
            let mut record = scheduled(state, id)?;
            record.status = ChangeSetStatusV1::Cancelled(height);
            state.world.runtime_change_sets.insert(*id, record);
            RuntimeUpgradeEvent::Canceled(RuntimeUpgradeCanceled { id: *id, height })
        }
    };
    state.world.emit_events(core::iter::once(event));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_policy_refusal_retains_original_scope_after_the_scope_retires() {
        let parameter = RuntimeUpgradePolicyV1::default()
            .to_custom_parameter()
            .unwrap();
        let limits =
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
        let result = norito::core::with_decode_limits_scope(limits, || {
            policy::<RuntimeUpgradePolicyV1>(&parameter, RuntimeUpgradePolicyV1::MAX_PAYLOAD_BYTES)
        });
        let Err(ExecutionAttemptError::Deferred(original)) = result else {
            panic!("original policy allocation refusal")
        };
        let cause = original
            .decode_refusal()
            .expect("retained original codec scope")
            .clone();
        assert_eq!(
            cause.kind(),
            norito::core::DecodeLocalRefusalKind::EnclosingLimit
        );
        let vm = original.into_vm_error();
        assert_eq!(
            crate::execution_attempt::ExecutionDeferred::try_from_vm_error(vm)
                .unwrap()
                .decode_refusal(),
            Some(&cause)
        );
    }

    #[test]
    fn typed_provenance_preserves_the_installed_distinct_signer_verdicts() {
        use iroha_crypto::{Algorithm, KeyPair};
        let signer = KeyPair::from_seed(vec![0x75; 32], Algorithm::Ed25519);
        let change = ChangeSetId([9; 32]);
        let valid =
            ChangeSetEndorsementV1::sign(change, signer.public_key().clone(), signer.private_key())
                .unwrap();
        let policy = RuntimeProvenancePolicyV1 {
            signers: vec![signer.public_key().clone()],
            threshold: 1,
        };
        for endorsements in [Vec::new(), vec![valid.clone()], vec![valid.clone(), valid]] {
            assert_eq!(
                check_provenance(change, &policy, &endorsements).is_ok(),
                check_change_set_provenance(change, &policy.signers, 1, &endorsements).is_ok()
            );
        }
        let wrong = ChangeSetEndorsementV1::sign(
            ChangeSetId([8; 32]),
            signer.public_key().clone(),
            signer.private_key(),
        )
        .unwrap();
        assert!(check_provenance(change, &policy, &[wrong]).is_err());
    }
}
