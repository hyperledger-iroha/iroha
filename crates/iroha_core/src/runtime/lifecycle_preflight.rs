//! Read-only sizing of a supplied committed parameter/trigger snapshot.
//!
//! This report neither activates a selection nor grants release support. Callers
//! must obtain parameters and trigger registrations from one authenticated cut.
//! An optional rekey destination measures the production storage mutation and
//! journal in disposable state. It does not authorize activation or a transaction.

use iroha_data_model::{
    account::AccountId,
    events::EventFilterBox,
    parameter::Parameters,
    runtime::definition::SelectionId,
    sumeragi_finality::lifecycle_runtime::{LifecycleRuntimeProofStepV1, LifecycleRuntimeProofV1},
    transaction::Executable,
    trigger::{Trigger, TriggerId},
};

use crate::smartcontracts::isi::triggers::{
    set::{
        Set,
        lifecycle_journal::{
            LifecycleJournalFailure, LifecycleJournalLimits, lifecycle_block_work_limits,
        },
    },
    specialized::{SpecializedTrigger, TimeTriggerRetryState},
};

/// One of the largest canonical persistent Actions in the supplied registry.
#[derive(Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize)]
pub struct LifecycleActionSize {
    /// Registered trigger identity.
    pub trigger_id: TriggerId,
    /// Exact canonical Action frame length, including original IVM bytecode.
    pub action_frame_bytes: usize,
}

/// Exact source inventory; these are source bytes, not estimated hashing work.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, norito::derive::JsonSerialize)]
pub struct LifecycleSourceInventory {
    /// Number of registered trigger entries.
    pub triggers: usize,
    /// Original bytecode bytes, deduplicated by canonical blob digest.
    pub distinct_artifact_bytes: u64,
    /// Exact canonical inline Executable frame bytes, counted once per entry.
    pub inline_executable_bytes: u64,
}

/// Why a disposable rekey did not complete its capture.
#[derive(Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize)]
pub struct LifecycleRekeyFailure {
    /// `setup`, `rekey`, `output_limit`, `allocation`, or `local_invariant`.
    pub kind: String,
    /// Production error, preserved without converting it to a fit estimate.
    pub reason: String,
}

/// Actual production capture counters for one fresh disposable block/root.
///
/// Every Time entry has a synthetic maximum-width nonzero retry state because
/// Torii's Action export omits persisted retry state. This is a conservative
/// capture-work envelope for this destination, not an exact current-state replay
/// or a promise that a complete transaction/output row will be admitted.
#[derive(Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize)]
pub struct LifecycleRekeyMeasurement {
    /// Nonrefunded byte visits charged by the production journal.
    pub byte_visits: u64,
    /// Nonrefunded codec/index operations charged by the production journal.
    pub operations: u64,
    /// Successfully retained effects, including effects on other authorities' entries.
    pub effects: usize,
    /// Successfully retained Action commitment frame bytes.
    pub frame_bytes: usize,
    /// Retained unavailable source states; these cannot verify a trigger's Action.
    pub unavailable_states: usize,
    /// Capture or storage failure; counters remain partial when this is present.
    pub failure: Option<LifecycleRekeyFailure>,
}

/// Owned source inventory and an optional measurement of the complete registry rekey.
#[derive(Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize)]
pub struct LifecycleAuthorityWork {
    /// An authority appearing in a supplied Action; reference-only accounts are
    /// not enumerated. The global inventory also covers rekeys from such accounts.
    pub authority: AccountId,
    /// Exact source inventory of entries owned by this authority.
    pub owned_sources: LifecycleSourceInventory,
    /// Absent without an explicit destination: capture fit is then unknown.
    /// When present, uses all entries, including foreign account references and
    /// destination-owned authorityless ByCall filters, via `replace_account_id`.
    pub rekey: Option<LifecycleRekeyMeasurement>,
}

/// Diagnostic sizing only; original compact commitments never embed these Actions.
#[derive(Debug, Clone, PartialEq, Eq, norito::derive::JsonSerialize)]
pub struct LifecyclePreflightReport {
    /// Exact release-support failure for the supplied committed output/source policies.
    pub output_support_error: Option<String>,
    /// Number of supplied registered triggers examined.
    pub registered_triggers: usize,
    /// All supplied sources, an inventory upper bound for any single rekey's
    /// affected entries, including reference-only accounts. Not a work estimate.
    pub all_sources: LifecycleSourceInventory,
    /// Explicit destination for disposable production rekey measurements.
    /// Absent means capture fit is unknown, regardless of source sizes.
    pub rekey_destination: Option<AccountId>,
    /// Number of Time entries given the documented synthetic retry envelope.
    pub retry_envelope_entries: usize,
    /// Per-authority rekey source inventory, sorted by authority.
    pub authority_work: Vec<LifecycleAuthorityWork>,
    /// Shared nonrefunded byte-visit allowance for the entire block.
    pub block_byte_visit_budget: u64,
    /// Shared nonrefunded codec/index operation allowance for the entire block.
    pub block_operation_budget: u64,
    /// Maximum byte visits admitted by a root before earlier block work.
    pub root_byte_visit_budget: u64,
    /// Maximum codec/index operations per root before earlier block work.
    pub root_operation_budget: u64,
    /// Aggregate retained commitment frame-byte allowance per root.
    pub root_frame_byte_budget: usize,
    /// Maximum effects per root.
    pub root_effect_budget: usize,
    /// Up to twenty largest Actions, descending by size and then ascending by ID.
    pub largest_actions: Vec<LifecycleActionSize>,
    /// Number of consecutive certified cuts the caller plans to prove.
    pub witness_cuts: usize,
    /// Estimated JSON bytes for uniformly distributed ordinary-write keys.
    /// Includes shared selection descriptors and two extra nonempty sibling levels.
    pub estimated_witness_bytes: usize,
    /// Conservative JSON bound assuming all 256 sibling levels are nonempty.
    pub worst_case_witness_bytes: usize,
    /// Current SDK JSON admission cap, independent of the number of World entries.
    pub sdk_witness_max_bytes: usize,
}

/// Inspect committed parameters and original trigger registrations without writing State.
///
/// `ordinary_writes_per_block` estimates writes in each *block*, not World size.
/// The estimate assumes uniform hashed keys; adversarial or unusually deep paths
/// can require the reported worst case. A real proof must still pass SDK bounds.
/// Supply every registered trigger from one cut; the helper cannot authenticate
/// an incomplete caller-supplied iterator. It reports only the twenty largest IDs.
/// The CLI also requires canonical roundtrip identity for every supplied Trigger
/// frame before calling this helper. Neither check can detect an upstream exporter
/// that already normalized a stored field or omitted an entry; exact, complete
/// source exports from one authenticated cut remain a caller prerequisite.
///
/// Without `rekey_destination`, capture fit is unknown: inventory bytes do not
/// predict mutable-field capture work. With a destination, each listed authority
/// is rekeyed over the entire supplied registry in a separate disposable fresh
/// block using the production mutation/journal. Immutable executable bindings are
/// prepared before capture, as at registration/restore; their source inventory is
/// reported separately, without guessed CPU/hash multipliers. No replay root is
/// applied. Time retry state is absent from Torii Actions, so every disposable Time
/// entry receives `Some { retries_used: u32::MAX, next_retry_at_ms: u64::MAX }`.
/// This is the maximum structurally admissible retry encoding, conservatively
/// including entries whose policy could never produce it. Measurements cover this
/// envelope and this destination, not arbitrary future metadata, prior roots in a
/// block, gas, fee, transaction validation, final-row overhead or output admission.
///
/// # Errors
/// Zero/more than 4096 witness cuts, arithmetic overflow, or an Action/descriptor
/// whose canonical size cannot be measured. A requested replay also rejects a
/// snapshot that cannot be restored through production insertion (including
/// duplicate IDs, malformed scope authorization or an invalid IVM artifact).
/// Policy incompatibility stays visible alongside action sizes in the report.
pub fn lifecycle_preflight<'a>(
    parameters: &Parameters,
    registered_triggers: impl IntoIterator<Item = &'a Trigger>,
    witness_cuts: usize,
    ordinary_writes_per_block: usize,
    rekey_destination: Option<&AccountId>,
) -> Result<LifecyclePreflightReport, String> {
    if !(1..=4096).contains(&witness_cuts) {
        return Err("lifecycle witness requires 1..=4096 consecutive cuts".into());
    }
    let block = parameters.block();
    let output_support_error = super::lifecycle_support::selected_output_support(
        &super::release_catalog()
            .map_err(|error| error.to_string())?
            .lifecycle,
        block.execution_output(),
        block.fastpq_source(),
    )
    .err();
    let registered_triggers: Vec<_> = registered_triggers.into_iter().collect();
    let mut largest_actions = Vec::<LifecycleActionSize>::new();
    let mut all_sources = LifecycleSourceInventory::default();
    let mut all_artifacts = std::collections::BTreeSet::new();
    let mut authorities = std::collections::BTreeMap::<
        AccountId,
        (
            LifecycleAuthorityWork,
            std::collections::BTreeSet<iroha_crypto::Hash>,
        ),
    >::new();
    let limits = LifecycleJournalLimits::selected(block.execution_output().max_output_bytes);
    for trigger in &registered_triggers {
        let size = norito::canonical_frame_len(trigger.action()).map_err(|e| e.to_string())?;
        let (work, seen) = authorities
            .entry(trigger.action().authority().clone())
            .or_insert_with(|| {
                (
                    LifecycleAuthorityWork {
                        authority: trigger.action().authority().clone(),
                        owned_sources: LifecycleSourceInventory::default(),
                        rekey: None,
                    },
                    Default::default(),
                )
            });
        work.owned_sources.triggers = work
            .owned_sources
            .triggers
            .checked_add(1)
            .ok_or("trigger count overflow")?;
        all_sources.triggers = all_sources
            .triggers
            .checked_add(1)
            .ok_or("trigger count overflow")?;
        match trigger.action().executable() {
            Executable::Ivm(code) => {
                let hash: iroha_crypto::Hash = iroha_crypto::HashOf::new(code).into();
                let bytes =
                    u64::try_from(code.as_ref().len()).map_err(|_| "artifact length overflow")?;
                if seen.insert(hash) {
                    work.owned_sources.distinct_artifact_bytes = work
                        .owned_sources
                        .distinct_artifact_bytes
                        .checked_add(bytes)
                        .ok_or("artifact inventory overflow")?;
                }
                if all_artifacts.insert(hash) {
                    all_sources.distinct_artifact_bytes = all_sources
                        .distinct_artifact_bytes
                        .checked_add(bytes)
                        .ok_or("artifact inventory overflow")?;
                }
            }
            _ => {
                let bytes = u64::try_from(
                    norito::canonical_frame_len(trigger.action().executable())
                        .map_err(|e| e.to_string())?,
                )
                .map_err(|_| "inline length overflow")?;
                work.owned_sources.inline_executable_bytes = work
                    .owned_sources
                    .inline_executable_bytes
                    .checked_add(bytes)
                    .ok_or("inline inventory overflow")?;
                all_sources.inline_executable_bytes = all_sources
                    .inline_executable_bytes
                    .checked_add(bytes)
                    .ok_or("inline inventory overflow")?;
            }
        }
        let index = largest_actions.partition_point(|entry| {
            entry.action_frame_bytes > size
                || (entry.action_frame_bytes == size && &entry.trigger_id <= trigger.id())
        });
        if index < 20 {
            largest_actions.insert(
                index,
                LifecycleActionSize {
                    trigger_id: trigger.id().clone(),
                    action_frame_bytes: size,
                },
            );
            largest_actions.truncate(20);
        }
    }
    let retry_envelope_entries = if let Some(destination) = rekey_destination {
        let set = disposable_rekey_store(&registered_triggers)?;
        for (work, _) in authorities.values_mut() {
            work.rekey = Some(measure_rekey(&set, &work.authority, destination, limits));
        }
        registered_triggers
            .iter()
            .filter(|trigger| matches!(trigger.action().filter(), EventFilterBox::Time(_)))
            .count()
    } else {
        0
    };
    let (block_byte_visit_budget, block_operation_budget) = lifecycle_block_work_limits();
    // Use maximum decimal height width. Hash/selection JSON lengths are fixed.
    let branch_bytes = |levels: usize| -> Result<usize, String> {
        let proof = LifecycleRuntimeProofV1 {
            height: u64::MAX,
            execution_selection: SelectionId([0x11; 32]),
            post_selection: SelectionId([0x11; 32]),
            siblings: (0..levels)
                .map(|level| LifecycleRuntimeProofStepV1 {
                    level: (255 - level) as u8,
                    hash: iroha_crypto::Hash::prehashed([0x55; 32]),
                })
                .rev()
                .collect(),
        };
        norito::json::to_json(&proof)
            .map(|value| value.len())
            .map_err(|e| e.to_string())
    };
    let catalog = super::release_catalog().map_err(|e| e.to_string())?;
    let selection = &catalog.lifecycle;
    let services = &catalog.definitions[&selection.services_profile];
    let implementation = &catalog.definitions[&selection.bindings[0].implementation];
    let shared = norito::json::to_json(&norito::json!({
        "selection": selection, "services": services, "implementation": implementation,
        "runtime_proof": null, "continuation_proofs": [],
    }))
    .map_err(|e| e.to_string())?
    .len();
    let size = |levels| -> Result<usize, String> {
        branch_bytes(levels)?
            .checked_add(1)
            .and_then(|value| value.checked_mul(witness_cuts))
            .and_then(|value| value.checked_add(shared))
            .ok_or_else(|| "lifecycle witness estimate overflow".into())
    };
    let estimated_levels =
        (usize::BITS - ordinary_writes_per_block.saturating_sub(1).leading_zeros()) as usize + 2;
    Ok(LifecyclePreflightReport {
        output_support_error,
        registered_triggers: all_sources.triggers,
        all_sources,
        rekey_destination: rekey_destination.cloned(),
        retry_envelope_entries,
        authority_work: authorities.into_values().map(|(work, _)| work).collect(),
        block_byte_visit_budget,
        block_operation_budget,
        root_byte_visit_budget: limits.encode.max_byte_visits as u64,
        root_operation_budget: limits.encode.max_operations as u64,
        root_frame_byte_budget: limits.total_frame_bytes,
        root_effect_budget: limits.effects,
        largest_actions,
        witness_cuts,
        estimated_witness_bytes: size(estimated_levels.min(256))?,
        worst_case_witness_bytes: size(256)?,
        sdk_witness_max_bytes: 32 * 1024 * 1024,
    })
}

/// Restore a disposable registry through production insertion paths, without
/// lifecycle capture. The exported inputs and any live State remain untouched.
fn disposable_rekey_store(triggers: &[&Trigger]) -> Result<Set, String> {
    let set = Set::default();
    let mut block = set.block();
    let mut transaction = block.transaction();
    for trigger in triggers {
        macro_rules! add {
            ($method:ident) => {
                transaction
                    .$method(
                        SpecializedTrigger::try_from((*trigger).clone()).map_err(str::to_owned)?,
                    )
                    .map_err(|error| {
                        format!(
                            "cannot restore trigger {} for preflight: {error}",
                            trigger.id()
                        )
                    })?
            };
        }
        let inserted = match trigger.action().filter() {
            EventFilterBox::Data(_) => add!(add_data_trigger),
            EventFilterBox::Pipeline(_) => add!(add_pipeline_trigger),
            EventFilterBox::Time(_) => add!(add_time_trigger),
            EventFilterBox::ExecuteTrigger(_) => add!(add_by_call_trigger),
            EventFilterBox::TriggerCompleted(_) => {
                return Err(format!(
                    "unsupported trigger filter in preflight snapshot: {}",
                    trigger.id()
                ));
            }
        };
        if !inserted {
            return Err(format!(
                "duplicate trigger ID in preflight snapshot: {}",
                trigger.id()
            ));
        }
        if matches!(trigger.action().filter(), EventFilterBox::Time(_)) {
            transaction.set_time_trigger_retry_state(
                trigger.id(),
                Some(TimeTriggerRetryState {
                    retries_used: u32::MAX,
                    next_retry_at_ms: u64::MAX,
                }),
            );
        }
    }
    transaction.apply();
    block.commit();
    Ok(set)
}

/// Run the real rekey against an overlay and drop it, including on refusal.
fn measure_rekey(
    set: &Set,
    authority: &AccountId,
    destination: &AccountId,
    limits: LifecycleJournalLimits,
) -> LifecycleRekeyMeasurement {
    let mut measurement = LifecycleRekeyMeasurement {
        byte_visits: 0,
        operations: 0,
        effects: 0,
        frame_bytes: 0,
        unavailable_states: 0,
        failure: None,
    };
    let mut block = set.block();
    let mut transaction = block.transaction();
    // Private bounded diagnostic pool, matching production-envelope tests:
    // no shared node pool or live State admission. Exhaustion remains an
    // allocation refusal in the report, never an inferred fit result.
    if let Err(error) = transaction.enable_selected_lifecycle_journal(
        iroha_allocation::AllocationBudget::new(32 * 1024 * 1024),
        limits,
        u64::MAX,
    ) {
        measurement.failure = Some(LifecycleRekeyFailure {
            kind: "setup".into(),
            reason: error.into(),
        });
        return measurement;
    }
    let rekey_error = transaction.replace_account_id(authority, destination).err();
    let metrics = transaction
        .lifecycle_capture_metrics()
        .expect("enabled diagnostic journal");
    measurement.byte_visits = metrics.byte_visits;
    measurement.operations = metrics.operations;
    measurement.effects = metrics.effects;
    measurement.frame_bytes = metrics.frame_bytes;
    measurement.unavailable_states = metrics.unavailable_states;
    measurement.failure = transaction
        .lifecycle_failure()
        .map(|error| LifecycleRekeyFailure {
            kind: match error {
                LifecycleJournalFailure::OutputLimit(_) => "output_limit",
                LifecycleJournalFailure::Allocation(_) => "allocation",
                LifecycleJournalFailure::Refused(_) => "local_invariant",
            }
            .into(),
            reason: error.reason().into(),
        })
        .or_else(|| {
            rekey_error.map(|error| LifecycleRekeyFailure {
                kind: "rekey".into(),
                reason: error.to_string(),
            })
        });
    measurement
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        events::time::{ExecutionTime, TimeEventFilter},
        transaction::{Executable, IvmBytecode},
        trigger::action::{Action, Repeats},
    };

    #[test]
    fn lifecycle_preflight_measures_original_actions_and_reports_the_real_policy_check() {
        let key = KeyPair::from_seed(vec![0x59; 32], Algorithm::Ed25519);
        let authority = AccountId::new(key.public_key().clone());
        let triggers: Vec<_> = (1..=24)
            .map(|index| {
                Trigger::new(
                    format!("sized_{index:02}").parse().unwrap(),
                    Action::new(
                        Executable::Ivm(IvmBytecode::from_compiled(vec![
                            0x55;
                            if index == 24 {
                                4 * 1024 * 1024
                            } else {
                                index * 1024
                            }
                        ])),
                        Repeats::Exactly(1),
                        authority.clone(),
                        TimeEventFilter::new(ExecutionTime::PreCommit),
                    )
                    .unwrap(),
                )
            })
            .collect();
        let parameters = Parameters::default();
        let report = lifecycle_preflight(&parameters, &triggers, 4096, 10_000, None).unwrap();
        println!(
            "lifecycle preflight: cuts={}, estimated_witness_bytes={}, worst_case_witness_bytes={}, sdk_max_bytes={}",
            report.witness_cuts,
            report.estimated_witness_bytes,
            report.worst_case_witness_bytes,
            report.sdk_witness_max_bytes
        );
        assert_eq!(
            report.output_support_error,
            super::super::lifecycle_support::lifecycle_output_support(
                parameters.block().execution_output(),
                parameters.block().fastpq_source()
            )
            .err()
        );
        assert_eq!(report.registered_triggers, 24);
        assert_eq!(report.largest_actions.len(), 20);
        assert!(report.largest_actions[0].action_frame_bytes > 4 * 1024 * 1024);
        assert!(report.output_support_error.is_none());
        let mut incompatible = parameters.clone();
        let mut policy = parameters.block().execution_output();
        policy.max_output_bytes = 1;
        incompatible.set_parameter(iroha_data_model::parameter::Parameter::Block(
            iroha_data_model::parameter::BlockParameter::ExecutionOutput(policy),
        ));
        assert!(
            lifecycle_preflight(&incompatible, &triggers, 1, 1, None)
                .unwrap()
                .output_support_error
                .is_some()
        );
        for (row, trigger) in report.largest_actions.iter().zip(triggers.iter().rev()) {
            assert_eq!(&row.trigger_id, trigger.id());
            assert_eq!(
                row.action_frame_bytes,
                norito::encode_canonical(trigger.action()).unwrap().len()
            );
        }
        assert!(report.estimated_witness_bytes > 4 * 1024 * 1024);
        assert!(report.estimated_witness_bytes < 8 * 1024 * 1024);
        assert!(report.worst_case_witness_bytes > report.sdk_witness_max_bytes);
        assert!(lifecycle_preflight(&parameters, &triggers, 0, 10_000, None).is_err());
        assert!(lifecycle_preflight(&parameters, &triggers, 4097, 10_000, None).is_err());
        let empty = lifecycle_preflight(&parameters, [], 1, 0, None).unwrap();
        assert_eq!(empty.registered_triggers, 0);
        assert!(empty.largest_actions.is_empty());
        assert!(empty.estimated_witness_bytes < empty.worst_case_witness_bytes);
    }
    #[test]
    fn lifecycle_preflight_groups_authorities_and_deduplicates_only_artifacts() {
        let key = KeyPair::from_seed(vec![0x59; 32], Algorithm::Ed25519);
        let authority = AccountId::new(key.public_key().clone());
        let code = IvmBytecode::from_compiled(vec![0x71; 4096]);
        let action = Action::new(
            Executable::Ivm(code),
            Repeats::Exactly(1),
            authority.clone(),
            TimeEventFilter::new(ExecutionTime::PreCommit),
        )
        .unwrap();
        let triggers = vec![
            Trigger::new("preflight_one".parse().unwrap(), action.clone()),
            Trigger::new("preflight_two".parse().unwrap(), action),
        ];
        let parameters = Parameters::default();
        let original = norito::json::to_json(&triggers).unwrap();
        let exported: Vec<Trigger> = norito::json::from_json(&original).unwrap();
        let parameters_json = norito::json::to_json(&parameters).unwrap();
        let parameters: Parameters = norito::json::from_json(&parameters_json).unwrap();
        let report = lifecycle_preflight(&parameters, &exported, 10, 100, None).unwrap();
        assert_eq!(report.authority_work.len(), 1);
        let work = &report.authority_work[0];
        assert_eq!(work.authority, authority);
        assert_eq!(work.owned_sources.triggers, 2);
        assert_eq!(work.owned_sources.distinct_artifact_bytes, 4096);
        assert_eq!(work.owned_sources.inline_executable_bytes, 0);
        assert_eq!(report.all_sources, work.owned_sources);
        assert!(work.rekey.is_none());
        assert!(report.rekey_destination.is_none());
        assert_eq!(report.retry_envelope_entries, 0);
        assert_eq!(original, norito::json::to_json(&triggers).unwrap());
        let json: norito::json::Value =
            norito::json::from_json(&norito::json::to_json(&report).unwrap()).unwrap();
        assert_eq!(json["root_effect_budget"].as_u64(), Some(4096));
    }

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    fn inline_action(authority: &AccountId, filter: impl Into<EventFilterBox>) -> Action {
        use iroha_data_model::prelude::*;
        let mut action = Action::new(
            Executable::Instructions(
                vec![InstructionBox::from(Log::new(
                    Level::INFO,
                    "preflight".into(),
                ))]
                .into(),
            ),
            Repeats::Indefinitely,
            authority.clone(),
            filter.into(),
        )
        .unwrap();
        action.metadata.insert(
            "__registered_block_height".parse().unwrap(),
            iroha_primitives::json::Json::from(1_u64),
        );
        action
    }

    #[test]
    fn lifecycle_preflight_rekeys_foreign_references_and_destination_by_call_without_applying() {
        use crate::smartcontracts::isi::triggers::{
            global_data_trigger_scope_metadata_for_testing,
            owned_data_trigger_scope_metadata_for_testing,
        };
        use iroha_data_model::prelude::*;
        let old = account(1);
        let destination = account(2);
        let foreign = account(3);
        let time = Trigger::new(
            "owned_time".parse().unwrap(),
            inline_action(&old, TimeEventFilter::new(ExecutionTime::PreCommit)),
        );
        let mut filter_reference = inline_action(
            &foreign,
            DataEventFilter::Account(AccountEventFilter::new().for_account(old.clone())),
        );
        for (key, value) in owned_data_trigger_scope_metadata_for_testing().iter() {
            filter_reference.metadata.insert(key.clone(), value.clone());
        }
        let mut metadata_reference = inline_action(&foreign, DataEventFilter::Any);
        for (key, value) in global_data_trigger_scope_metadata_for_testing(&old).iter() {
            metadata_reference
                .metadata
                .insert(key.clone(), value.clone());
        }
        let mut destination_by_call = inline_action(&destination, ExecuteTriggerEventFilter::new());
        // Persisted authorityless filters are handled by the real rekey seam;
        // avoid Action::new's registration-time normalization in this fixture.
        destination_by_call.filter = ExecuteTriggerEventFilter::new().into();
        let triggers = vec![
            time,
            Trigger::new("foreign_filter".parse().unwrap(), filter_reference),
            Trigger::new("foreign_metadata".parse().unwrap(), metadata_reference),
            Trigger::new("destination_by_call".parse().unwrap(), destination_by_call),
        ];
        let specialized: SpecializedTrigger<ExecuteTriggerEventFilter> =
            SpecializedTrigger::try_from(triggers[3].clone()).unwrap();
        assert!(
            specialized.action.filter.authority().is_none(),
            "the production specialized conversion must preserve the supplied filter"
        );
        let before = norito::json::to_json(&triggers).unwrap();
        let parameters = Parameters::default();
        let report = lifecycle_preflight(&parameters, &triggers, 1, 1, Some(&destination)).unwrap();
        let owned = report
            .authority_work
            .iter()
            .find(|entry| entry.authority == old)
            .unwrap();
        assert_eq!(owned.owned_sources.triggers, 1);
        let measurement = owned.rekey.as_ref().unwrap();
        assert_eq!(
            measurement.effects, 4,
            "the complete registry participates in the production rekey"
        );
        assert_eq!(
            measurement.frame_bytes,
            8 * iroha_data_model::block::execution_output::LIFECYCLE_HASH_FRAME_BYTES
        );
        assert!(measurement.byte_visits > 0);
        assert!(measurement.operations > 0);
        assert_eq!(measurement.unavailable_states, 0);
        assert_eq!(measurement.failure, None);
        assert_eq!(report.retry_envelope_entries, 1);
        let same_authority = report
            .authority_work
            .iter()
            .find(|entry| entry.authority == destination)
            .unwrap()
            .rekey
            .as_ref()
            .unwrap();
        assert_eq!(same_authority.effects, 0);
        assert_eq!(same_authority.byte_visits, 0);
        assert_eq!(same_authority.operations, 0);
        assert_eq!(before, norito::json::to_json(&triggers).unwrap());
        assert_eq!(
            report,
            lifecycle_preflight(&parameters, &triggers, 1, 1, Some(&destination)).unwrap(),
            "every authority receives a fresh disposable root; prior diagnostic roots are never applied"
        );
    }

    #[test]
    fn lifecycle_preflight_retry_envelope_is_present_and_bounds_none_work() {
        use crate::smartcontracts::isi::triggers::set::SetReadOnly;
        use mv::storage::StorageReadOnly;
        let old = account(4);
        let destination = account(5);
        let trigger = Trigger::new(
            "retry_envelope".parse().unwrap(),
            inline_action(&old, TimeEventFilter::new(ExecutionTime::PreCommit)),
        );
        let original = norito::json::to_json(&trigger).unwrap();
        let set = disposable_rekey_store(&[&trigger]).unwrap();
        let view = set.view();
        let retry = view
            .time_triggers()
            .get(trigger.id())
            .unwrap()
            .retry_state
            .unwrap();
        assert_eq!(retry.retries_used, u32::MAX);
        assert_eq!(retry.next_retry_at_ms, u64::MAX);
        drop(view);
        let limits = LifecycleJournalLimits::selected(1_048_576);
        let some = measure_rekey(&set, &old, &destination, limits);
        assert_eq!(some.effects, 1);
        assert_eq!(some.failure, None);
        let mut block = set.block();
        let mut transaction = block.transaction();
        assert!(transaction.set_time_trigger_retry_state(trigger.id(), None));
        transaction.apply();
        block.commit();
        let none = measure_rekey(&set, &old, &destination, limits);
        assert_eq!(none.effects, some.effects);
        assert_eq!(none.failure, None);
        assert!(
            some.byte_visits > none.byte_visits,
            "Some retry state must be measured, not assumed away"
        );
        assert!(some.operations >= none.operations);
        assert_eq!(original, norito::json::to_json(&trigger).unwrap());
    }

    #[test]
    fn lifecycle_preflight_reports_production_cap_failure_and_unavailable_sources() {
        let old = account(6);
        let destination = account(7);
        let action = inline_action(&old, TimeEventFilter::new(ExecutionTime::PreCommit));
        let trigger = Trigger::new("limited".parse().unwrap(), action.clone());
        let mut parameters = Parameters::default();
        let mut policy = parameters.block().execution_output();
        policy.max_output_bytes = 1;
        parameters.set_parameter(iroha_data_model::parameter::Parameter::Block(
            iroha_data_model::parameter::BlockParameter::ExecutionOutput(policy),
        ));
        let report =
            lifecycle_preflight(&parameters, [&trigger], 1, 1, Some(&destination)).unwrap();
        let measurement = report.authority_work[0].rekey.as_ref().unwrap();
        assert_eq!(report.root_frame_byte_budget, 1);
        assert!(report.output_support_error.is_some());
        assert_eq!(measurement.failure.as_ref().unwrap().kind, "output_limit");
        assert!(measurement.byte_visits > 0);
        assert_eq!(measurement.effects, 0);
        assert_eq!(measurement.frame_bytes, 0);
        let set = disposable_rekey_store(&[&trigger]).unwrap();
        let mut limits = LifecycleJournalLimits::selected(1_048_576);
        limits.encode.max_byte_visits = 1;
        let limited = measure_rekey(&set, &old, &destination, limits);
        assert_eq!(limited.failure.as_ref().unwrap().kind, "output_limit");
        assert!(limited.byte_visits <= 1);
        let mut unavailable = action;
        unavailable.metadata.remove("__registered_block_height");
        let trigger = Trigger::new("unavailable".parse().unwrap(), unavailable);
        let report =
            lifecycle_preflight(&Parameters::default(), [&trigger], 1, 1, Some(&destination))
                .unwrap();
        let measurement = report.authority_work[0].rekey.as_ref().unwrap();
        assert_eq!(measurement.failure, None);
        assert_eq!(
            measurement.unavailable_states, 2,
            "successful capture is not proof of an available trigger source"
        );
    }
}
