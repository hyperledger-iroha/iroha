//! Trigger matching against authenticated producer events under either execution profile.
use crate::smartcontracts::isi::triggers::{
    trigger_is_enabled, trigger_was_registered_before_block,
};
use iroha_data_model::{
    events::{
        EventBox, EventFilter,
        pipeline::{PipelineEventBox, PipelineEventFilterBox},
        time::TimeEvent,
    },
    ivm_c10::{EventFilterBoxC10V1, PipelineEventBoxC10V1, StoredC10TriggerV1},
    trigger::action::Repeats,
};

/// The source producer retains the event's actual result family until all predicates were evaluated.
#[derive(Clone, Copy)]
pub(crate) enum PipelineSource<'a> {
    /// Event whose transaction result belongs to the installed baseline.
    Baseline(&'a PipelineEventBox),
    /// Event whose transaction result belongs to C10.
    C10(&'a PipelineEventBoxC10V1),
}

impl PipelineSource<'_> {
    pub(super) fn matches_baseline(self, filter: &PipelineEventFilterBox) -> bool {
        match self {
            Self::Baseline(event) => filter.matches(event),
            Self::C10(event) => event.matches_baseline_filter(filter),
        }
    }
    pub(super) fn matches_c10(self, row: &StoredC10TriggerV1, height: u64) -> bool {
        let action = &row.trigger().action;
        let EventFilterBoxC10V1::Pipeline(filter) = action.filter() else {
            return false;
        };
        trigger_is_enabled(action.metadata())
            && !action.repeats().is_depleted()
            && trigger_was_registered_before_block(action.metadata(), height)
            && match self {
                Self::Baseline(event) => filter.matches_baseline(event),
                Self::C10(event) => filter.matches(event),
            }
    }
}

/// Borrow only the source categories that feed callback arguments or scheduled payout authorization.
/// Pipeline callbacks take no guest event arguments under either profile.
#[derive(Clone, Copy)]
pub(super) enum CallbackEvent<'a> {
    Event(&'a EventBox),
    Pipeline(PipelineSource<'a>),
}

impl CallbackEvent<'_> {
    pub(super) fn scheduled_time(self) -> bool {
        match self {
            Self::Event(event) => matches!(event, EventBox::Time(_)),
            Self::Pipeline(PipelineSource::Baseline(_) | PipelineSource::C10(_)) => false,
        }
    }
}

/// Data matches require a live, enabled action and a predicate of the same event family.
pub(super) fn data_matches(
    row: &StoredC10TriggerV1,
    event: &iroha_data_model::events::data::DataEvent,
) -> bool {
    let action = &row.trigger().action;
    matches!(action.filter(), EventFilterBoxC10V1::Data(filter) if filter.matches(event))
        && trigger_is_enabled(action.metadata())
        && !action.repeats().is_depleted()
}

fn data_active(row: &StoredC10TriggerV1) -> bool {
    let action = &row.trigger().action;
    matches!(action.filter(), EventFilterBoxC10V1::Data(_))
        && trigger_is_enabled(action.metadata())
        && !action.repeats().is_depleted()
}

/// Earliest funded occurrence over the caller's exact current or pre-block rows.
pub(crate) fn scheduled_progress_at_ms<'a>(
    baseline: Option<u64>,
    rows: impl Iterator<Item = &'a StoredC10TriggerV1>,
    since_ms: u64,
    next_height: u64,
) -> Option<u64> {
    baseline
        .into_iter()
        .chain(rows.filter_map(|row| {
            let action = &row.trigger().action;
            let EventFilterBoxC10V1::Time(filter) = action.filter() else {
                return None;
            };
            crate::smartcontracts::isi::triggers::set::scheduled_action_deadline(
                filter,
                action.metadata(),
                action.repeats(),
                row.retry_state().map(|retry| retry.next_time_ms),
                since_ms,
                next_height,
            )
        }))
        .min()
}

pub(super) fn time_rows(
    world: &impl super::WorldReadOnly,
) -> impl Iterator<Item = (&TriggerId, &StoredC10TriggerV1)> {
    world
        .c10_trigger_candidates()
        .iter()
        .filter(|(_, kind)| **kind == iroha_data_model::events::TriggeringEventType::Time)
        .filter_map(|(id, _)| Some((id, world.c10_triggers().get(id)?)))
}
pub(super) fn clock_progress_required(
    world: &impl super::WorldReadOnly,
    height: u64,
    parent: std::time::Duration,
) -> bool {
    time_rows(world).any(|(id, row)| {
        let action = &row.trigger().action;
        let EventFilterBoxC10V1::Time(filter) = action.filter() else {
            return false;
        };
        crate::runtime::deferred_consent::trigger_ready(world, id, height)
            && super::time_action_requires_clock_progress(
                filter,
                action.metadata(),
                action.repeats(),
                row.retry_state().is_some(),
                parent,
            )
    })
}

/// Number of eligible Time invocations in the actual interval. A pending retry owns a single
/// occurrence and suppresses the ordinary schedule, including when it is not due yet.
pub(super) fn time_occurrences(
    row: &StoredC10TriggerV1,
    event: &TimeEvent,
    height: u64,
    now: u64,
) -> u32 {
    let action = &row.trigger().action;
    let EventFilterBoxC10V1::Time(filter) = action.filter() else {
        return 0;
    };
    if !trigger_is_enabled(action.metadata())
        || action.repeats().is_depleted()
        || !trigger_was_registered_before_block(action.metadata(), height)
    {
        return 0;
    }
    if let Some(retry) = row.retry_state() {
        return u32::from(now >= retry.next_time_ms);
    }
    let count = filter.count_matches(event);
    match action.repeats() {
        Repeats::Indefinitely => count,
        Repeats::Exactly(limit) => count.min(limit),
    }
}

use super::{StateTransaction, StorageReadOnly as _};
use crate::smartcontracts::isi::triggers::set::lifecycle_journal::{
    LifecycleChange, LifecycleSnapshot,
};
use iroha_data_model::trigger::TriggerId;

impl StateTransaction<'_, '_> {
    pub(crate) fn c10_trigger_snapshot(&mut self, id: &TriggerId) -> Option<LifecycleSnapshot> {
        let world = &mut *self.world;
        let row = world.c10_triggers.get(id)?;
        world.triggers.lifecycle_c10_snapshot(row)
    }
    pub(crate) fn c10_trigger_inserted(&mut self, id: &TriggerId) {
        self.world.triggers.register_c10_incarnation(id);
        if let Some(after) = self.c10_trigger_snapshot(id) {
            self.world
                .triggers
                .lifecycle_record(id, LifecycleChange::Insert(after));
        }
    }
    pub(crate) fn c10_trigger_removed(
        &mut self,
        id: &TriggerId,
        before: Option<LifecycleSnapshot>,
    ) {
        self.world.triggers.remove_c10_incarnation(id);
        if let Some(before) = before {
            self.world
                .triggers
                .lifecycle_record(id, LifecycleChange::Remove(before));
        }
    }
    pub(super) fn consume_c10_repeat(&mut self, id: &TriggerId) -> bool {
        let Some(stored) = self.world.c10_triggers.get(id) else {
            return false;
        };
        match stored.trigger().action.repeats() {
            Repeats::Indefinitely => false,
            Repeats::Exactly(0 | 1) => {
                crate::smartcontracts::isi::triggers::isi::remove_c10_trigger(self, id)
            }
            Repeats::Exactly(n) => {
                self.update_c10_trigger(id, |stored| stored.set_repeats(Repeats::Exactly(n - 1)));
                false
            }
        }
    }

    pub(super) fn consume_time_progress(&mut self, id: &TriggerId) {
        use crate::smartcontracts::isi::triggers::progress;
        if let Some(stored) = self.world.c10_triggers.get(id) {
            if progress::remaining(stored.trigger().action.metadata()) != 0 {
                self.update_c10_trigger(id, |row| progress::consume(row.metadata_mut()));
            }
        } else {
            self.world.triggers.consume_time_progress(id);
        }
    }
    pub(super) fn clear_time_retry(&mut self, id: &TriggerId) {
        if let Some(stored) = self.world.c10_triggers.get(id) {
            if stored.retry_state().is_some() {
                self.update_c10_trigger(id, |row| {
                    row.set_retry_state(None)
                        .expect("absent retry always valid")
                });
            }
        } else {
            self.world.triggers.set_time_trigger_retry_state(id, None);
        }
    }
    pub(crate) fn update_c10_trigger<R>(
        &mut self,
        id: &TriggerId,
        update: impl FnOnce(&mut StoredC10TriggerV1) -> R,
    ) -> Option<R> {
        let before = self.c10_trigger_snapshot(id);
        let row = self.world.c10_triggers.get_mut(id)?;
        let active_before = data_active(row);
        let result = update(row);
        if !active_before && data_active(row) {
            self.world.triggers.data_trigger_reenabled(id);
        }
        if let (Some(before), Some(after)) = (before, self.c10_trigger_snapshot(id)) {
            self.world
                .triggers
                .lifecycle_record(id, LifecycleChange::Update(before, after));
        }
        Some(result)
    }
}

/// Reconstruct the authenticated C10 source without discarding a local codec refusal.
pub(super) fn identity(
    row: &StoredC10TriggerV1,
    height: u64,
) -> Result<
    iroha_data_model::block::execution_output::TriggerUseV1,
    crate::execution_attempt::ExecutionAttemptError<String>,
> {
    use crate::{
        execution_attempt::ExecutionAttemptError,
        smartcontracts::isi::triggers::set::lifecycle_journal::parse_registration_height,
    };
    let stored_height = row
        .trigger()
        .action
        .metadata()
        .get(crate::smartcontracts::isi::triggers::TRIGGER_REGISTERED_BLOCK_HEIGHT_METADATA_KEY)
        .and_then(|json| parse_registration_height(json.get()))
        .filter(|registered| *registered < height)
        .ok_or("C10 trigger registration is not strictly before its applying height")?;
    let action_hash =
        norito::core::classify_decode_attempt(|| row.action_hash()).map_err(|error| match error
            .into_local_refusal()
        {
            Ok(original) => ExecutionAttemptError::Deferred(original.into()),
            Err(error) => ExecutionAttemptError::Rejected(error.error().to_string()),
        })?;
    Ok(iroha_data_model::block::execution_output::TriggerUseV1 {
        trigger_id: row.trigger().id.clone(),
        registered_at_height: stored_height,
        action_hash,
    })
}

/// Verify the callback's content and ABI hashes against its own registered manifest family.
pub(super) fn validate_callback_manifest(
    world: &impl super::WorldReadOnly,
    identity: &crate::smartcontracts::code::BoundContractIdentity,
    code_hash: iroha_crypto::Hash,
    abi_hash: iroha_crypto::Hash,
) -> Result<(), iroha_data_model::ValidationFail> {
    use crate::smartcontracts::code::registered_manifest::RegisteredManifest;
    use iroha_data_model::{ValidationFail, smart_contract::ContractArtifactId};
    let key = ContractArtifactId::for_address(&identity.contract_address, identity.code_hash)
        .map_err(|error| ValidationFail::NotPermitted(error.to_string()))?;
    let manifest = RegisteredManifest::get(world, &key).ok_or_else(|| {
        ValidationFail::NotPermitted(format!(
            "contract instance `{}` has no manifest",
            identity.contract_address
        ))
    })?;
    crate::smartcontracts::ivm::validate_manifest_hash_fields(
        manifest.hashes(),
        code_hash,
        abi_hash,
    )
    .map_err(ValidationFail::IvmAdmission)
}

/// Build one bounded schedule across the two stores, ordering retries first and then each
/// category by ID. Guard the count before cloning IDs, independent of elapsed wall time.
pub(super) fn fill_time_schedule(
    world: &super::WorldBlock<'_>,
    event: &TimeEvent,
    height: u64,
    now: u64,
    maximum: usize,
    rows: &mut Vec<TriggerId>,
) {
    use crate::smartcontracts::isi::triggers::set::{SetReadOnly, time_trigger_action_is_due};
    use iroha_data_model::events::TriggeringEventType;
    for retry in [true, false] {
        if rows.len() >= maximum {
            break;
        }
        let mut baseline = world
            .triggers
            .time_triggers()
            .iter()
            .filter_map(|(id, action)| {
                if action.retry_state.is_some() != retry
                    || !time_trigger_action_is_due(action, event, height, now)
                {
                    return None;
                }
                let n = if retry {
                    1
                } else {
                    let count = action.filter.count_matches(event);
                    match action.repeats {
                        Repeats::Indefinitely => count,
                        Repeats::Exactly(max) => count.min(max),
                    }
                };
                (n != 0).then_some((id, n))
            })
            .peekable();
        let mut c10 = world
            .c10_trigger_candidates
            .iter()
            .filter_map(|(id, kind)| {
                if *kind != TriggeringEventType::Time {
                    return None;
                }
                let stored = world.c10_triggers.get(id)?;
                if stored.retry_state().is_some() != retry {
                    return None;
                }
                let n = time_occurrences(stored, event, height, now);
                (n != 0).then_some((id, n))
            })
            .peekable();
        while rows.len() < maximum {
            let next = match (baseline.peek(), c10.peek()) {
                (Some((lhs, _)), Some((rhs, _))) if lhs <= rhs => baseline.next(),
                (_, Some(_)) => c10.next(),
                (Some(_), None) => baseline.next(),
                (None, None) => break,
            };
            let (id, n) = next.expect("peeked owner");
            rows.extend(
                std::iter::repeat_with(|| id.clone()).take((n as usize).min(maximum - rows.len())),
            );
        }
    }
}

/// Exercise both registered callback program representations against the certified C10 instance.
#[cfg(test)]
pub(crate) fn verify_deployed_callbacks(
    chain: &mut crate::sumeragi::test_chain::CertifiedTestChain,
    owner: &iroha_crypto::KeyPair,
    invocation: iroha_data_model::transaction::executable::ContractInvocation,
) {
    use super::WorldReadOnly;
    use iroha_data_model::{
        block::execution_output::ExecutionOutputV1,
        events::execute_trigger::ExecuteTriggerEventFilter,
        isi::ExecuteTrigger,
        ivm_c10::{ActionC10V1, RegisterTriggerC10V1, TriggerC10V1},
        smart_contract::ContractArtifactId,
        transaction::{Executable, executable::IvmBytecode},
    };
    use iroha_model_base::metadata::Metadata;
    use iroha_primitives::json::Json;
    let authority = chain.genesis_account().clone();
    let key = ContractArtifactId::for_address(
        &invocation.contract_address,
        invocation.expected_code_hash,
    )
    .unwrap();
    let bytes = chain
        .state()
        .view()
        .world()
        .contract_code()
        .get(&key)
        .unwrap()
        .clone();
    let mut invalid = bytes.clone();
    let layout = ivm::IvmProfileId::C10_ABI_V1.engine().header_layout();
    invalid[layout.budget_offset..layout.abi_version_offset].copy_from_slice(&19u64.to_le_bytes());
    let invalid_id: TriggerId = "c10_invalid_program".parse().unwrap();
    let invalid_registration = RegisterTriggerC10V1 {
        object: TriggerC10V1 {
            id: invalid_id.clone(),
            action: ActionC10V1::new(
                Executable::Ivm(IvmBytecode::from_compiled(invalid)),
                Repeats::Exactly(1),
                authority.clone(),
                EventFilterBoxC10V1::ExecuteTrigger(
                    ExecuteTriggerEventFilter::new().for_trigger(invalid_id.clone()),
                ),
            )
            .unwrap(),
        },
    };
    let refused_at = chain.committed(chain.height()).block_time_ms() + 1_000;
    let tx = chain.sign(owner, [invalid_registration.into()], refused_at);
    assert_eq!(chain.commit_at(refused_at, vec![tx]), [false]);
    let tip = chain.committed(chain.height());
    let (_, failed) = tip.block().network_execution_output_at(0).unwrap();
    use iroha_data_model::ivm_c10::{
        IvmAdmissionErrorC10V1, TransactionRejectionReasonC10V1, ValidationFailC10V1,
    };
    assert!(
        matches!(failed, ExecutionOutputV1::C10(output) if matches!(&output.result.0,
        Err(TransactionRejectionReasonC10V1::Validation(ValidationFailC10V1::IvmAdmission(
            IvmAdmissionErrorC10V1::MaxCyclesWithoutZk(19)))))),
        "{failed:?}"
    );
    assert!(
        chain
            .state()
            .view()
            .world()
            .c10_triggers()
            .get(&invalid_id)
            .is_none()
    );
    let mut metadata = Metadata::default();
    metadata.insert(
        "contract_address".parse().unwrap(),
        Json::from(invocation.contract_address.as_str()),
    );
    metadata.insert(
        "contract_entrypoint".parse().unwrap(),
        Json::from(invocation.entrypoint.as_str()),
    );
    let ids: [TriggerId; 2] = [
        "c10_direct_callback".parse().unwrap(),
        "c10_prepared_callback".parse().unwrap(),
    ];
    let bodies = [
        (Executable::ContractCall(invocation), Metadata::default()),
        (Executable::Ivm(IvmBytecode::from_compiled(bytes)), metadata),
    ];
    let registered_at = chain.committed(chain.height()).block_time_ms() + 1_000;
    let registrations = ids
        .iter()
        .cloned()
        .zip(bodies)
        .map(|(id, (executable, metadata))| {
            let action = ActionC10V1::new(
                executable,
                Repeats::Exactly(2),
                authority.clone(),
                EventFilterBoxC10V1::ExecuteTrigger(
                    ExecuteTriggerEventFilter::new().for_trigger(id.clone()),
                ),
            )
            .unwrap()
            .with_metadata(metadata);
            RegisterTriggerC10V1 {
                object: TriggerC10V1 { id, action },
            }
            .into()
        });
    let tx = chain.sign(owner, registrations, registered_at);
    let outcomes = chain.commit_at(registered_at, vec![tx]);
    assert_eq!(
        outcomes,
        [true],
        "{:?}",
        chain.committed(chain.height()).block().execution_outputs()
    );
    let executed_at = registered_at + 1_000;
    let execute = ids.iter().cloned().map(|id| ExecuteTrigger::new(id).into());
    let tx = chain.sign(owner, execute, executed_at);
    let outcomes = chain.commit_at(executed_at, vec![tx]);
    let tip = chain.committed(chain.height());
    let (_, row) = tip.block().network_execution_output_at(0).unwrap();
    assert_eq!(outcomes, [true], "{row:?}");
    let ExecutionOutputV1::C10(row) = row else {
        panic!("expected certified C10 callback result: {row:?}");
    };
    assert_eq!(
        row.completions
            .iter()
            .map(|r| &r.trigger_id)
            .collect::<Vec<_>>(),
        ids.iter().collect::<Vec<_>>()
    );
    for id in &ids {
        let view = chain.state().view();
        assert_eq!(
            view.world()
                .c10_triggers()
                .get(id)
                .unwrap()
                .trigger()
                .action
                .repeats(),
            Repeats::Exactly(1)
        );
    }
}
