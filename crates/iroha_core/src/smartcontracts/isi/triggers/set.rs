//! Trigger logic. Instead of defining a Trigger as an entity, we provide a collection of triggers
//! as the smallest unit, which is an idea borrowed from lisp hooks.
//!
//! The point of the idea is to create an ordering (or hash function) which maps the event filter
//! and the event that triggers it to the same approximate location in the hierarchy, thus using
//! Binary search trees (common lisp) or hash tables (racket) to quickly trigger hooks.
use super::{
    data_trigger_global_permission_grantee, data_trigger_scope_authorization_is_well_formed,
    replace_data_trigger_global_permission_grantee, trigger_is_enabled,
    trigger_was_registered_before_block,
};
use crate::smartcontracts::isi::triggers::specialized::{
    LoadedAction, LoadedActionTrait, SpecializedAction, SpecializedTrigger, TimeTriggerRetryState,
};
use core::cmp::min;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    events::EventFilter,
    isi::error::{InstructionExecutionError, MathError},
    prelude::*,
    query::error::FindError,
    transaction::{
        IvmBytecode,
        executable::{ContractInvocation, ExecutableBatchItem},
    },
    trigger::action::EnsureTriggerAuthority,
};
use iroha_logger::prelude::*;
use iroha_primitives::const_vec::ConstVec;
use ivm::VMError;
use mv::storage::{
    Block as StorageBlock, Storage, StorageReadOnly, Transaction as StorageTransaction,
    View as StorageView,
};
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json;
#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize as JsonSerializeTrait};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroU64,
};
use thiserror::Error;
/// Error type for [`Set`] operations.
#[derive(Debug, Error, displaydoc::Display)]
pub enum Error {
    /// Failed to preload IVM trigger
    Preload(#[from] VMError),
    /// Data trigger is missing canonical v1 scope-authorization metadata
    InvalidDataScopeAuthorization,
    /// Data trigger capacity exceeded: maximum 4096
    DataTriggerCapacity,
    /// Data trigger authority `{0}` capacity exceeded: maximum 64
    DataTriggerAuthorityCapacity(AccountId),
}
/// Result type for [`Set`] operations.
pub type Result<T, E = Error> = core::result::Result<T, E>;
/// Revalidate a time-trigger action immediately before invocation.
pub(crate) fn time_trigger_action_is_due(
    action: &LoadedAction<TimeEventFilter>,
    event: &TimeEvent,
    current_block_height: u64,
    current_block_time_ms: u64,
) -> bool {
    if !trigger_is_enabled(action.metadata())
        || action.repeats.is_depleted()
        || !trigger_was_registered_before_block(action.metadata(), current_block_height)
    {
        return false;
    }
    action.retry_state.map_or_else(
        || action.filter.count_matches(event) > 0,
        |retry_state| current_block_time_ms >= retry_state.next_retry_at_ms,
    )
}
/// Revalidate a pipeline-trigger action immediately before invocation.
pub(crate) fn pipeline_trigger_action_matches(
    action: &LoadedAction<PipelineEventFilterBox>,
    event: &PipelineEventBox,
    current_block_height: u64,
) -> bool {
    trigger_is_enabled(action.metadata())
        && !action.repeats.is_depleted()
        && trigger_was_registered_before_block(action.metadata(), current_block_height)
        && action.filter.matches(event)
}
/// Revalidate a data-trigger action against its captured event.
pub(crate) fn data_trigger_action_matches(
    action: &LoadedAction<DataEventFilter>,
    event: &iroha_data_model::events::data::DataEvent,
) -> bool {
    trigger_is_enabled(action.metadata())
        && !action.repeats.is_depleted()
        && action.filter.matches(event)
}
struct BorrowedEnumVariant<'a, T> {
    discriminant: u32,
    value: &'a dyn norito::core::NoritoSerialize,
    marker: core::marker::PhantomData<T>,
}
impl<'a, T> BorrowedEnumVariant<'a, T> {
    fn new(discriminant: u32, value: &'a dyn norito::core::NoritoSerialize) -> Self {
        Self {
            discriminant,
            value,
            marker: core::marker::PhantomData,
        }
    }
}
impl<T: norito::core::NoritoSerialize> norito::core::NoritoSerialize
    for BorrowedEnumVariant<'_, T>
{
    fn schema_hash() -> [u8; 16] {
        T::schema_hash()
    }
    fn serialize(
        &self,
        writer: &mut norito::core::Encoder<'_>,
    ) -> core::result::Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(&self.discriminant, writer)?;
        let mut scratch = norito::core::DeriveSmallBuf::new();
        norito::core::write_len_prefixed(writer, self.value, &mut scratch)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        let discriminant = self.discriminant.encoded_len_exact()?;
        let value = self.value.encoded_len_exact()?;
        discriminant
            .checked_add(norito::core::len_prefix_len(value))?
            .checked_add(value)
    }
}
/// [`IvmBytecode`]s keyed by contract hash.
/// Stored together with usage counts so triggers sharing the same blob can be deduplicated.
type TriggerContractStore = Storage<HashOf<IvmBytecode>, IvmBytecodeEntry>;
type TriggerContractStoreBlock<'set> = StorageBlock<'set, HashOf<IvmBytecode>, IvmBytecodeEntry>;
type TriggerContractStoreTransaction<'block, 'set> =
    StorageTransaction<'block, 'set, HashOf<IvmBytecode>, IvmBytecodeEntry>;
type TriggerContractStoreView<'set> = StorageView<'set, HashOf<IvmBytecode>, IvmBytecodeEntry>;
type ActiveTriggerIdStore = Storage<TriggerId, ()>;
type ActiveTriggerIdStoreBlock<'set> = StorageBlock<'set, TriggerId, ()>;
type ActiveTriggerIdStoreTransaction<'block, 'set> =
    StorageTransaction<'block, 'set, TriggerId, ()>;
type ActiveTriggerIdStoreView<'set> = StorageView<'set, TriggerId, ()>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DataTriggerFamily {
    Peer,
    Domain,
    Account,
    Asset,
    AssetDefinition,
    Nft,
    Rwa,
    Trigger,
    Role,
    Configuration,
    Executor,
    Proof,
    VerifyingKey,
    RuntimeUpgrade,
    SmartContract,
    Soradns,
    Sorafs,
    Musubi,
    SpaceDirectory,
    Escrow,
    Oracle,
    Social,
    Bridge,
    Governance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum DataTriggerSubjectKind {
    Domain,
    Account,
    Asset,
    AssetDefinitionForAsset,
    AssetDefinition,
    AssetTransferSource,
    AssetTransferDestination,
    Nft,
    Rwa,
    Trigger,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum DataTriggerIndexKey {
    Any,
    Family(DataTriggerFamily),
    Subject(DataTriggerSubjectKind, Vec<u8>),
}

#[derive(Clone, Debug, Default)]
struct DataTriggerIndex {
    postings: BTreeMap<DataTriggerIndexKey, BTreeSet<TriggerId>>,
}

impl DataTriggerIndex {
    fn from_triggers(
        triggers: &impl StorageReadOnly<TriggerId, LoadedAction<DataEventFilter>>,
    ) -> Self {
        let mut index = Self::default();
        for (id, action) in triggers.iter() {
            index.insert(id, &action.filter);
        }
        index
    }

    fn insert(&mut self, id: &TriggerId, filter: &DataEventFilter) {
        for key in data_trigger_filter_index_keys(filter) {
            self.postings.entry(key).or_default().insert(id.clone());
        }
    }

    fn remove(&mut self, id: &TriggerId) {
        self.postings.retain(|_, ids| {
            ids.remove(id);
            !ids.is_empty()
        });
    }

    fn candidates(&self, event: &DataEvent) -> Vec<TriggerId> {
        let mut candidates = BTreeSet::new();
        for key in data_event_index_keys(event) {
            if let Some(ids) = self.postings.get(&key) {
                candidates.extend(ids.iter().cloned());
            }
        }
        candidates.into_iter().collect()
    }

    /// Return a deterministic upper bound on candidate IDs visited for `event`.
    ///
    /// A trigger may occur in more than one posting list, so this deliberately
    /// counts duplicates. Charging this bound before constructing the deduped
    /// candidate vector prevents an installed trigger population from causing
    /// unmetered allocation or tree work in another transaction.
    fn candidate_scan_work(&self, event: &DataEvent) -> usize {
        data_event_index_keys(event)
            .iter()
            .filter_map(|key| self.postings.get(key))
            .fold(0_usize, |work, ids| work.saturating_add(ids.len()))
    }
}

/// Constant-size watermark freezing data-trigger eligibility for an event batch.
///
/// Trigger registration and inactive-to-active transitions receive a monotonic
/// transaction-local generation. A callback can therefore mutate the live index
/// without requiring a deep copy of all 4,096 trigger filters for every DFS frame.
#[derive(Clone, Copy, Debug)]
pub(crate) struct DataTriggerMatchSnapshot {
    generation_watermark: u64,
}

fn validate_data_trigger_capacities<'a>(
    triggers: impl IntoIterator<Item = (&'a TriggerId, &'a LoadedAction<DataEventFilter>)>,
) -> core::result::Result<(), String> {
    let mut total = 0_usize;
    let mut per_authority = BTreeMap::<&AccountId, usize>::new();
    for (_, action) in triggers {
        total = total.saturating_add(1);
        if total > super::isi::MAX_DATA_TRIGGERS_TOTAL {
            return Err(format!(
                "data trigger capacity exceeds first-release maximum {}",
                super::isi::MAX_DATA_TRIGGERS_TOTAL
            ));
        }
        let count = per_authority.entry(&action.authority).or_default();
        *count = count.saturating_add(1);
        if *count > super::isi::MAX_DATA_TRIGGERS_PER_AUTHORITY {
            return Err(format!(
                "data trigger authority `{}` exceeds first-release maximum {}",
                action.authority,
                super::isi::MAX_DATA_TRIGGERS_PER_AUTHORITY
            ));
        }
    }
    Ok(())
}

fn data_trigger_subject_key<T: Encode>(
    kind: DataTriggerSubjectKind,
    subject: &T,
) -> DataTriggerIndexKey {
    DataTriggerIndexKey::Subject(kind, subject.encode())
}

fn data_trigger_filter_index_keys(filter: &DataEventFilter) -> Vec<DataTriggerIndexKey> {
    let family = |family| vec![DataTriggerIndexKey::Family(family)];
    match filter {
        DataEventFilter::Any => vec![DataTriggerIndexKey::Any],
        DataEventFilter::Peer(_) => family(DataTriggerFamily::Peer),
        DataEventFilter::Domain(filter) => filter.id_matcher().as_ref().map_or_else(
            || family(DataTriggerFamily::Domain),
            |id| vec![data_trigger_subject_key(DataTriggerSubjectKind::Domain, id)],
        ),
        DataEventFilter::Account(filter) => filter.id_matcher().as_ref().map_or_else(
            || family(DataTriggerFamily::Account),
            |id| {
                vec![data_trigger_subject_key(
                    DataTriggerSubjectKind::Account,
                    id,
                )]
            },
        ),
        DataEventFilter::Asset(filter) => {
            let mut keys = Vec::new();
            if let Some(id) = filter.id_matcher() {
                keys.push(data_trigger_subject_key(DataTriggerSubjectKind::Asset, id));
            }
            if let Some(id) = filter.asset_definition_matcher() {
                keys.push(data_trigger_subject_key(
                    DataTriggerSubjectKind::AssetDefinitionForAsset,
                    id,
                ));
            }
            if let Some(id) = filter.transfer_source_account_matcher() {
                keys.push(data_trigger_subject_key(
                    DataTriggerSubjectKind::AssetTransferSource,
                    id,
                ));
            }
            if let Some(id) = filter.transfer_destination_account_matcher() {
                keys.push(data_trigger_subject_key(
                    DataTriggerSubjectKind::AssetTransferDestination,
                    id,
                ));
            }
            if keys.is_empty() {
                keys.push(DataTriggerIndexKey::Family(DataTriggerFamily::Asset));
            }
            keys
        }
        DataEventFilter::AssetDefinition(filter) => filter.id_matcher().as_ref().map_or_else(
            || family(DataTriggerFamily::AssetDefinition),
            |id| {
                vec![data_trigger_subject_key(
                    DataTriggerSubjectKind::AssetDefinition,
                    id,
                )]
            },
        ),
        DataEventFilter::Nft(filter) => filter.id_matcher().as_ref().map_or_else(
            || family(DataTriggerFamily::Nft),
            |id| vec![data_trigger_subject_key(DataTriggerSubjectKind::Nft, id)],
        ),
        DataEventFilter::Rwa(filter) => filter.id_matcher().as_ref().map_or_else(
            || family(DataTriggerFamily::Rwa),
            |id| vec![data_trigger_subject_key(DataTriggerSubjectKind::Rwa, id)],
        ),
        DataEventFilter::Trigger(filter) => filter.id_matcher().as_ref().map_or_else(
            || family(DataTriggerFamily::Trigger),
            |id| {
                vec![data_trigger_subject_key(
                    DataTriggerSubjectKind::Trigger,
                    id,
                )]
            },
        ),
        DataEventFilter::Role(_) => family(DataTriggerFamily::Role),
        DataEventFilter::Configuration(_) => family(DataTriggerFamily::Configuration),
        DataEventFilter::Executor(_) => family(DataTriggerFamily::Executor),
        DataEventFilter::Proof(_) => family(DataTriggerFamily::Proof),
        DataEventFilter::VerifyingKey(_) => family(DataTriggerFamily::VerifyingKey),
        DataEventFilter::RuntimeUpgrade(_) => family(DataTriggerFamily::RuntimeUpgrade),
        DataEventFilter::Soradns(_) => family(DataTriggerFamily::Soradns),
        DataEventFilter::Sorafs(_) => family(DataTriggerFamily::Sorafs),
        DataEventFilter::Musubi(_) => family(DataTriggerFamily::Musubi),
        DataEventFilter::SpaceDirectory(_) => family(DataTriggerFamily::SpaceDirectory),
        DataEventFilter::Escrow(_) => family(DataTriggerFamily::Escrow),
        DataEventFilter::Oracle(_) => family(DataTriggerFamily::Oracle),
        DataEventFilter::Social(_) => family(DataTriggerFamily::Social),
        DataEventFilter::Bridge(_) => family(DataTriggerFamily::Bridge),
        DataEventFilter::Governance(_) => family(DataTriggerFamily::Governance),
    }
}

fn add_asset_event_index_keys(keys: &mut BTreeSet<DataTriggerIndexKey>, event: &AssetEvent) {
    keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Asset));
    let asset_id = event.origin();
    keys.insert(data_trigger_subject_key(
        DataTriggerSubjectKind::Asset,
        asset_id,
    ));
    keys.insert(data_trigger_subject_key(
        DataTriggerSubjectKind::AssetDefinitionForAsset,
        asset_id.definition(),
    ));
    if let AssetEvent::Transferred(transfer) = event {
        keys.insert(data_trigger_subject_key(
            DataTriggerSubjectKind::AssetTransferSource,
            transfer.source().account(),
        ));
        keys.insert(data_trigger_subject_key(
            DataTriggerSubjectKind::AssetTransferDestination,
            transfer.destination().account(),
        ));
    }
}

fn data_event_index_keys(event: &DataEvent) -> BTreeSet<DataTriggerIndexKey> {
    let mut keys = BTreeSet::from([DataTriggerIndexKey::Any]);
    match event {
        DataEvent::Peer(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Peer));
        }
        DataEvent::Domain(event) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Domain));
            keys.insert(data_trigger_subject_key(
                DataTriggerSubjectKind::Domain,
                event.origin(),
            ));
            match event {
                DomainEvent::Account(scoped) => {
                    keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Account));
                    keys.insert(data_trigger_subject_key(
                        DataTriggerSubjectKind::Account,
                        scoped.event.origin(),
                    ));
                }
                DomainEvent::Asset(scoped) => {
                    add_asset_event_index_keys(&mut keys, &scoped.event);
                }
                DomainEvent::AssetDefinition(scoped) => {
                    keys.insert(DataTriggerIndexKey::Family(
                        DataTriggerFamily::AssetDefinition,
                    ));
                    keys.insert(data_trigger_subject_key(
                        DataTriggerSubjectKind::AssetDefinition,
                        scoped.event.origin(),
                    ));
                }
                DomainEvent::Nft(event) => {
                    keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Nft));
                    keys.insert(data_trigger_subject_key(
                        DataTriggerSubjectKind::Nft,
                        event.origin(),
                    ));
                }
                DomainEvent::Rwa(event) => {
                    keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Rwa));
                    keys.insert(data_trigger_subject_key(
                        DataTriggerSubjectKind::Rwa,
                        event.origin(),
                    ));
                }
                _ => {}
            }
        }
        DataEvent::Account(event) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Account));
            keys.insert(data_trigger_subject_key(
                DataTriggerSubjectKind::Account,
                event.origin(),
            ));
        }
        DataEvent::Asset(event) => add_asset_event_index_keys(&mut keys, event),
        DataEvent::AssetDefinition(event) => {
            keys.insert(DataTriggerIndexKey::Family(
                DataTriggerFamily::AssetDefinition,
            ));
            keys.insert(data_trigger_subject_key(
                DataTriggerSubjectKind::AssetDefinition,
                event.origin(),
            ));
        }
        DataEvent::Trigger(event) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Trigger));
            keys.insert(data_trigger_subject_key(
                DataTriggerSubjectKind::Trigger,
                event.origin(),
            ));
        }
        DataEvent::Role(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Role));
        }
        DataEvent::Configuration(_) => {
            keys.insert(DataTriggerIndexKey::Family(
                DataTriggerFamily::Configuration,
            ));
        }
        DataEvent::Executor(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Executor));
        }
        DataEvent::Proof(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Proof));
        }
        DataEvent::VerifyingKey(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::VerifyingKey));
        }
        DataEvent::RuntimeUpgrade(_) => {
            keys.insert(DataTriggerIndexKey::Family(
                DataTriggerFamily::RuntimeUpgrade,
            ));
        }
        DataEvent::SmartContract(_) => {
            keys.insert(DataTriggerIndexKey::Family(
                DataTriggerFamily::SmartContract,
            ));
        }
        DataEvent::Soradns(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Soradns));
        }
        DataEvent::Sorafs(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Sorafs));
        }
        DataEvent::Musubi(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Musubi));
        }
        DataEvent::SpaceDirectory(_) => {
            keys.insert(DataTriggerIndexKey::Family(
                DataTriggerFamily::SpaceDirectory,
            ));
        }
        DataEvent::Escrow(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Escrow));
        }
        DataEvent::Oracle(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Oracle));
        }
        DataEvent::Social(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Social));
        }
        DataEvent::Bridge(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Bridge));
        }
        DataEvent::Governance(_) => {
            keys.insert(DataTriggerIndexKey::Family(DataTriggerFamily::Governance));
        }
    }
    keys
}

#[cfg(test)]
mod data_trigger_index_tests {
    use super::*;
    use iroha_crypto::KeyPair;

    fn account_id() -> AccountId {
        AccountId::new(
            KeyPair::try_random()
                .expect("data-trigger index fixture key generation should succeed")
                .public_key()
                .clone(),
        )
    }

    #[test]
    fn exact_account_and_domain_postings_bound_nested_event_candidates() {
        let alice = account_id();
        let bob = account_id();
        let domain = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let any_id: TriggerId = "a_any".parse().expect("valid trigger id");
        let account_family_id: TriggerId = "b_account_family".parse().expect("valid trigger id");
        let alice_id: TriggerId = "c_alice".parse().expect("valid trigger id");
        let domain_id: TriggerId = "d_domain".parse().expect("valid trigger id");
        let bob_id: TriggerId = "e_bob".parse().expect("valid trigger id");

        let mut index = DataTriggerIndex::default();
        index.insert(&any_id, &DataEventFilter::Any);
        index.insert(
            &account_family_id,
            &DataEventFilter::Account(AccountEventFilter::new()),
        );
        index.insert(
            &alice_id,
            &DataEventFilter::Account(AccountEventFilter::new().for_account(alice.clone())),
        );
        index.insert(
            &domain_id,
            &DataEventFilter::Domain(DomainEventFilter::new().for_domain(domain.clone())),
        );
        index.insert(
            &bob_id,
            &DataEventFilter::Account(AccountEventFilter::new().for_account(bob)),
        );

        let event = DataEvent::account_in_domain(AccountEvent::Deleted(alice), domain);
        assert_eq!(
            index.candidates(&event),
            vec![
                any_id.clone(),
                account_family_id.clone(),
                alice_id.clone(),
                domain_id.clone(),
            ]
        );

        index.remove(&alice_id);
        assert_eq!(
            index.candidates(&event),
            vec![any_id, account_family_id, domain_id]
        );
    }

    #[test]
    fn asset_postings_deduplicate_multi_constraint_candidates() {
        let source = account_id();
        let destination = account_id();
        let unrelated = account_id();
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("assets", "universal").expect("valid domain"),
            "rose".parse().expect("valid asset name"),
        );
        let source_asset = AssetId::new(definition.clone(), source.clone());
        let destination_asset = AssetId::new(definition.clone(), destination.clone());
        let matching_id: TriggerId = "asset_match".parse().expect("valid trigger id");
        let unrelated_id: TriggerId = "asset_unrelated".parse().expect("valid trigger id");
        let definition_event_id: TriggerId = "definition_event".parse().expect("valid trigger id");

        let mut index = DataTriggerIndex::default();
        index.insert(
            &matching_id,
            &DataEventFilter::Asset(
                AssetEventFilter::new()
                    .for_asset(source_asset.clone())
                    .for_asset_definition(definition.clone())
                    .for_transfer_source_account(source)
                    .for_transfer_destination_account(destination),
            ),
        );
        index.insert(
            &unrelated_id,
            &DataEventFilter::Asset(AssetEventFilter::new().for_transfer_source_account(unrelated)),
        );
        index.insert(
            &definition_event_id,
            &DataEventFilter::AssetDefinition(
                AssetDefinitionEventFilter::new().for_asset_definition(definition),
            ),
        );

        let event = DataEvent::Asset(AssetEvent::Transferred(AssetTransferred {
            source: source_asset,
            destination: destination_asset,
            amount: 1_u32.into(),
        }));
        assert_eq!(index.candidates(&event), vec![matching_id]);
    }
}
/// Specialized structure that maps event filters to Triggers.
// NB: `Set` has custom `Serialize` and `DeserializeSeed` implementations
// which need to be manually updated when changing the struct
#[derive(Default)]
pub struct Set {
    /// Triggers using [`DataEventFilter`]
    data_triggers: Storage<TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilterBox`]
    pipeline_triggers: Storage<TriggerId, LoadedAction<PipelineEventFilterBox>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: Storage<TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers: Storage<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: Storage<TriggerId, TriggeringEventType>,
    /// Active data trigger ids.
    active_data_trigger_ids: ActiveTriggerIdStore,
    /// Active pipeline trigger ids.
    active_pipeline_trigger_ids: ActiveTriggerIdStore,
    /// Active time trigger ids.
    active_time_trigger_ids: ActiveTriggerIdStore,
    /// Active by-call trigger ids.
    active_by_call_trigger_ids: ActiveTriggerIdStore,
    /// [`IvmBytecode`]s map by contract blob hash. This map serves multiple purposes:
    /// 1. Querying original contract blob of trigger
    /// 2. Deduplicating triggers with the same contract blob
    contracts: TriggerContractStore,
}
impl Set {
    fn action_is_active<F>(action: &LoadedAction<F>) -> bool {
        !action.repeats.is_depleted() && trigger_is_enabled(&action.metadata)
    }
    fn collect_active_ids<F: mv::Value>(
        triggers: &Storage<TriggerId, LoadedAction<F>>,
    ) -> ActiveTriggerIdStore {
        triggers
            .view()
            .iter()
            .filter(|(_, action)| Self::action_is_active(action))
            .map(|(id, _)| (id.clone(), ()))
            .collect()
    }
}
impl json::JsonDeserialize for Set {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut visitor = json::MapVisitor::new(parser)?;
        let mut data_triggers: Option<Storage<TriggerId, LoadedAction<DataEventFilter>>> = None;
        let mut pipeline_triggers: Option<
            Storage<TriggerId, LoadedAction<PipelineEventFilterBox>>,
        > = None;
        let mut time_triggers: Option<Storage<TriggerId, LoadedAction<TimeEventFilter>>> = None;
        let mut by_call_triggers: Option<
            Storage<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
        > = None;
        let mut ids: Option<Storage<TriggerId, TriggeringEventType>> = None;
        let mut contracts: Option<Storage<HashOf<IvmBytecode>, IvmBytecodeEntry>> = None;
        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "data_triggers" => {
                    data_triggers = Some(visitor.parse_value()?);
                }
                "pipeline_triggers" => {
                    pipeline_triggers = Some(visitor.parse_value()?);
                }
                "time_triggers" => {
                    time_triggers = Some(visitor.parse_value()?);
                }
                "by_call_triggers" => {
                    by_call_triggers = Some(visitor.parse_value()?);
                }
                "ids" => {
                    ids = Some(visitor.parse_value()?);
                }
                "contracts" => {
                    contracts = Some(visitor.parse_value()?);
                }
                other => {
                    visitor.skip_value()?;
                    trace!(%other, "ignoring unknown trigger set field");
                }
            }
        }
        visitor.finish()?;
        let data_triggers =
            data_triggers.ok_or_else(|| json::MapVisitor::missing_field("data_triggers"))?;
        let pipeline_triggers = pipeline_triggers
            .ok_or_else(|| json::MapVisitor::missing_field("pipeline_triggers"))?;
        let time_triggers =
            time_triggers.ok_or_else(|| json::MapVisitor::missing_field("time_triggers"))?;
        let by_call_triggers =
            by_call_triggers.ok_or_else(|| json::MapVisitor::missing_field("by_call_triggers"))?;
        let ids = ids.ok_or_else(|| json::MapVisitor::missing_field("ids"))?;
        let contracts = contracts.ok_or_else(|| json::MapVisitor::missing_field("contracts"))?;
        let incompatible_data_trigger = {
            let view = data_triggers.view();
            view.iter()
                .find(|(_, action)| {
                    !data_trigger_scope_authorization_is_well_formed(action.metadata())
                })
                .map(|(id, _)| id.clone())
        };
        if let Some(id) = incompatible_data_trigger {
            return Err(json::Error::InvalidField {
                field: "data_triggers".into(),
                message: format!(
                    "incompatible data trigger `{id}`: missing or malformed v1 scope authorization metadata; regenerate the first-release snapshot"
                ),
            });
        }
        let capacity_error = {
            let view = data_triggers.view();
            validate_data_trigger_capacities(view.iter()).err()
        };
        if let Some(message) = capacity_error {
            return Err(json::Error::InvalidField {
                field: "data_triggers".into(),
                message: format!(
                    "incompatible first-release data-trigger snapshot: {message}; regenerate the snapshot"
                ),
            });
        }
        let active_data_trigger_ids = Self::collect_active_ids(&data_triggers);
        let active_pipeline_trigger_ids = Self::collect_active_ids(&pipeline_triggers);
        let active_time_trigger_ids = Self::collect_active_ids(&time_triggers);
        let active_by_call_trigger_ids = Self::collect_active_ids(&by_call_triggers);
        Ok(Self {
            data_triggers,
            pipeline_triggers,
            time_triggers,
            by_call_triggers,
            ids,
            active_data_trigger_ids,
            active_pipeline_trigger_ids,
            active_time_trigger_ids,
            active_by_call_trigger_ids,
            contracts,
        })
    }
}
#[cfg(feature = "json")]
impl FastJsonWrite for Set {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut write = |name: &str, serialize: &dyn Fn(&mut String)| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
            norito::json::write_json_string(name, out);
            out.push(':');
            serialize(out);
        };
        write("data_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.data_triggers, out);
        });
        write("pipeline_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.pipeline_triggers, out);
        });
        write("time_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.time_triggers, out);
        });
        write("by_call_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.by_call_triggers, out);
        });
        write("ids", &|out| {
            JsonSerializeTrait::json_serialize(&self.ids, out)
        });
        write("contracts", &|out| {
            JsonSerializeTrait::json_serialize(&self.contracts, out)
        });
        out.push('}');
    }
}
/// Trigger set for block's aggregated changes
pub struct SetBlock<'set> {
    /// Triggers using [`DataEventFilter`]
    data_triggers: StorageBlock<'set, TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilterBox`]
    pipeline_triggers: StorageBlock<'set, TriggerId, LoadedAction<PipelineEventFilterBox>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: StorageBlock<'set, TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers: StorageBlock<'set, TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: StorageBlock<'set, TriggerId, TriggeringEventType>,
    /// Active data trigger ids.
    active_data_trigger_ids: ActiveTriggerIdStoreBlock<'set>,
    /// Active pipeline trigger ids.
    active_pipeline_trigger_ids: ActiveTriggerIdStoreBlock<'set>,
    /// Active time trigger ids.
    active_time_trigger_ids: ActiveTriggerIdStoreBlock<'set>,
    /// Active by-call trigger ids.
    active_by_call_trigger_ids: ActiveTriggerIdStoreBlock<'set>,
    /// Original [`IvmBytecode`]s by [`TriggerId`] for querying purposes.
    contracts: TriggerContractStoreBlock<'set>,
}
#[cfg(feature = "json")]
impl FastJsonWrite for SetBlock<'_> {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut write = |name: &str, serialize: &dyn Fn(&mut String)| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
            norito::json::write_json_string(name, out);
            out.push(':');
            serialize(out);
        };
        write("data_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.data_triggers, out);
        });
        write("pipeline_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.pipeline_triggers, out);
        });
        write("time_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.time_triggers, out);
        });
        write("by_call_triggers", &|out| {
            JsonSerializeTrait::json_serialize(&self.by_call_triggers, out);
        });
        write("ids", &|out| {
            JsonSerializeTrait::json_serialize(&self.ids, out)
        });
        write("contracts", &|out| {
            JsonSerializeTrait::json_serialize(&self.contracts, out)
        });
        out.push('}');
    }
}
fn append_delta_component(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    out.extend_from_slice(bytes);
}
fn append_trigger_storage_delta<K, V>(
    out: &mut Vec<u8>,
    name: &'static str,
    storage: &StorageBlock<'_, K, V>,
    encode_value: impl Fn(&V) -> Vec<u8>,
) where
    K: mv::Key + Encode,
    V: mv::Value,
{
    if !storage.is_dirty() {
        return;
    }
    append_delta_component(out, name.as_bytes());
    out.extend_from_slice(
        &u64::try_from(storage.revert_map().len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for key in storage.revert_map().keys() {
        append_delta_component(out, &key.encode());
        match storage.get(key) {
            Some(value) => {
                out.push(1);
                append_delta_component(out, &encode_value(value));
            }
            None => out.push(0),
        }
    }
}
impl SetBlock<'_> {
    /// Append every staged trigger-store mutation to a canonical merge write set.
    pub(crate) fn append_merge_execution_write_set(&self, out: &mut Vec<u8>) {
        append_trigger_storage_delta(out, "triggers.data", &self.data_triggers, |value| {
            LoadedActionDto::from(value).encode()
        });
        append_trigger_storage_delta(out, "triggers.pipeline", &self.pipeline_triggers, |value| {
            LoadedActionDto::from(value).encode()
        });
        append_trigger_storage_delta(out, "triggers.time", &self.time_triggers, |value| {
            LoadedActionDto::from(value).encode()
        });
        append_trigger_storage_delta(out, "triggers.by_call", &self.by_call_triggers, |value| {
            LoadedActionDto::from(value).encode()
        });
        append_trigger_storage_delta(out, "triggers.ids", &self.ids, Encode::encode);
        append_trigger_storage_delta(
            out,
            "triggers.active_data",
            &self.active_data_trigger_ids,
            Encode::encode,
        );
        append_trigger_storage_delta(
            out,
            "triggers.active_pipeline",
            &self.active_pipeline_trigger_ids,
            Encode::encode,
        );
        append_trigger_storage_delta(
            out,
            "triggers.active_time",
            &self.active_time_trigger_ids,
            Encode::encode,
        );
        append_trigger_storage_delta(
            out,
            "triggers.active_by_call",
            &self.active_by_call_trigger_ids,
            Encode::encode,
        );
        append_trigger_storage_delta(out, "triggers.contracts", &self.contracts, |value| {
            IvmBytecodeEntryDto::from(value).encode()
        });
    }
}
#[cfg(test)]
mod merge_write_set_tests {
    use super::*;
    #[test]
    fn encoder_mentions_every_trigger_block_store() {
        let source = include_str!("set.rs");
        let struct_start = source
            .find("pub struct SetBlock<'set> {")
            .expect("SetBlock declaration must remain discoverable");
        let struct_tail = &source[struct_start..];
        let struct_end = struct_tail
            .find("\n}\n\nfn append_delta_component")
            .expect("SetBlock declaration terminator must remain discoverable");
        let struct_body = &struct_tail[..struct_end];
        let encoder_start = source
            .find("pub(crate) fn append_merge_execution_write_set")
            .expect("trigger merge write-set encoder must exist");
        let encoder_tail = &source[encoder_start..];
        let encoder_end = encoder_tail
            .find("\n    }\n}")
            .expect("trigger merge write-set encoder terminator must remain discoverable");
        let encoder = &encoder_tail[..encoder_end];
        for line in struct_body.lines() {
            if !line.starts_with("    ") || line.starts_with("        ") {
                continue;
            }
            let Some((field, _)) = line.trim().split_once(':') else {
                continue;
            };
            if !field
                .chars()
                .all(|character| character == '_' || character.is_ascii_alphanumeric())
            {
                continue;
            }
            assert!(
                encoder.contains(field),
                "trigger SetBlock field `{field}` is absent from the merge write-set encoder"
            );
        }
    }
    #[test]
    fn encoder_distinguishes_equal_trigger_kinds_under_different_ids() {
        let set = Set::default();
        let mut first = set.block();
        first.ids.insert(
            "merge_trigger_a".parse().expect("valid trigger id"),
            TriggeringEventType::Time,
        );
        let mut first_bytes = Vec::new();
        first.append_merge_execution_write_set(&mut first_bytes);
        drop(first);
        let mut second = set.block();
        second.ids.insert(
            "merge_trigger_b".parse().expect("valid trigger id"),
            TriggeringEventType::Time,
        );
        let mut second_bytes = Vec::new();
        second.append_merge_execution_write_set(&mut second_bytes);
        assert_ne!(first_bytes, second_bytes);
    }
}
/// Trigger set for transaction's aggregated changes
pub struct SetTransaction<'block, 'set> {
    /// Last transaction-local trigger lifecycle generation allocated.
    next_registration_generation: u64,
    /// Per-ID registration generation within this transaction overlay.
    ///
    /// This is intentionally ephemeral: it distinguishes an ID that was
    /// removed and re-registered after a match was materialized without
    /// changing the persisted trigger wire format.
    registration_generations: BTreeMap<TriggerId, u64>,
    /// Generation at which each data trigger most recently became eligible.
    ///
    /// Inherited active triggers implicitly use generation zero. An explicitly
    /// inactive trigger uses `u64::MAX` until an inactive-to-active transition.
    data_trigger_eligibility_generations: BTreeMap<TriggerId, u64>,
    /// Deterministic, transaction-local postings for bounded data-event matching.
    data_trigger_index: DataTriggerIndex,
    /// Triggers using [`DataEventFilter`]
    data_triggers: StorageTransaction<'block, 'set, TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilterBox`]
    pipeline_triggers:
        StorageTransaction<'block, 'set, TriggerId, LoadedAction<PipelineEventFilterBox>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: StorageTransaction<'block, 'set, TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers:
        StorageTransaction<'block, 'set, TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: StorageTransaction<'block, 'set, TriggerId, TriggeringEventType>,
    /// Active data trigger ids.
    active_data_trigger_ids: ActiveTriggerIdStoreTransaction<'block, 'set>,
    /// Active pipeline trigger ids.
    active_pipeline_trigger_ids: ActiveTriggerIdStoreTransaction<'block, 'set>,
    /// Active time trigger ids.
    active_time_trigger_ids: ActiveTriggerIdStoreTransaction<'block, 'set>,
    /// Active by-call trigger ids.
    active_by_call_trigger_ids: ActiveTriggerIdStoreTransaction<'block, 'set>,
    /// Original [`IvmBytecode`]s by [`TriggerId`] for querying purposes.
    contracts: TriggerContractStoreTransaction<'block, 'set>,
}
/// Consistent point in time view of the [`Set`]
pub struct SetView<'set> {
    /// Triggers using [`DataEventFilter`]
    data_triggers: StorageView<'set, TriggerId, LoadedAction<DataEventFilter>>,
    /// Triggers using [`PipelineEventFilterBox`]
    pipeline_triggers: StorageView<'set, TriggerId, LoadedAction<PipelineEventFilterBox>>,
    /// Triggers using [`TimeEventFilter`]
    time_triggers: StorageView<'set, TriggerId, LoadedAction<TimeEventFilter>>,
    /// Triggers using [`ExecuteTriggerEventFilter`]
    by_call_triggers: StorageView<'set, TriggerId, LoadedAction<ExecuteTriggerEventFilter>>,
    /// Trigger ids with type of events they process
    ids: StorageView<'set, TriggerId, TriggeringEventType>,
    /// Active data trigger ids.
    active_data_trigger_ids: ActiveTriggerIdStoreView<'set>,
    /// Active pipeline trigger ids.
    active_pipeline_trigger_ids: ActiveTriggerIdStoreView<'set>,
    /// Active time trigger ids.
    active_time_trigger_ids: ActiveTriggerIdStoreView<'set>,
    /// Active by-call trigger ids.
    active_by_call_trigger_ids: ActiveTriggerIdStoreView<'set>,
    /// Original [`IvmBytecode`]s by [`TriggerId`] for querying purposes.
    contracts: TriggerContractStoreView<'set>,
}
/// Entry in smart-contracts map
#[cfg_attr(feature = "json", derive(norito::derive::FastJsonWrite))]
#[derive(Debug, Clone)]
pub struct IvmBytecodeEntry {
    /// Original smart contract binary blob
    original_contract: IvmBytecode,
    /// Canonical complete-deployable hash retained for prepared-contract lookup.
    code_hash: Hash,
    /// Number of times this contract is used
    count: NonZeroU64,
}
impl json::JsonDeserialize for IvmBytecodeEntry {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut visitor = json::MapVisitor::new(parser)?;
        let mut original_contract: Option<IvmBytecode> = None;
        let mut code_hash: Option<Hash> = None;
        let mut count: Option<u64> = None;
        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "original_contract" => {
                    original_contract = Some(visitor.parse_value()?);
                }
                "code_hash" => {
                    code_hash = Some(visitor.parse_value()?);
                }
                "count" => {
                    count = Some(visitor.parse_value()?);
                }
                other => {
                    visitor.skip_value()?;
                    iroha_logger::warn!(
                        "unknown field `{other}` while decoding IVM bytecode entry; skipping"
                    );
                }
            }
        }
        visitor.finish()?;
        let original_contract = original_contract
            .ok_or_else(|| json::MapVisitor::missing_field("original_contract"))?;
        let raw_count = count.ok_or_else(|| json::MapVisitor::missing_field("count"))?;
        let count = NonZeroU64::new(raw_count).ok_or_else(|| json::Error::InvalidField {
            field: "count".into(),
            message: "must be non-zero".into(),
        })?;
        let code_hash = code_hash.ok_or_else(|| json::MapVisitor::missing_field("code_hash"))?;
        if ivm::contract_code_hash(original_contract.as_ref()) != code_hash {
            return Err(json::Error::InvalidField {
                field: "code_hash".into(),
                message: "must match the complete deployable artifact".into(),
            });
        }
        Ok(Self {
            original_contract,
            code_hash,
            count,
        })
    }
}
// Norito DTOs for Set serialization
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
enum ExecutableRefDto {
    Ivm(HashOf<IvmBytecode>),
    ContractCall(ContractInvocation),
    Instructions(ConstVec<InstructionBox>),
    Batch(ConstVec<ExecutableBatchItem>),
}
impl From<&ExecutableRef> for ExecutableRefDto {
    fn from(r: &ExecutableRef) -> Self {
        match r {
            ExecutableRef::Ivm(h) => ExecutableRefDto::Ivm(*h),
            ExecutableRef::ContractCall(invocation) => {
                ExecutableRefDto::ContractCall(invocation.clone())
            }
            ExecutableRef::Instructions(v) => ExecutableRefDto::Instructions(v.clone()),
            ExecutableRef::Batch(items) => ExecutableRefDto::Batch(items.clone()),
        }
    }
}
impl TryFrom<ExecutableRefDto> for ExecutableRef {
    type Error = String;
    fn try_from(dto: ExecutableRefDto) -> Result<Self, Self::Error> {
        Ok(match dto {
            ExecutableRefDto::Ivm(h) => ExecutableRef::Ivm(h),
            ExecutableRefDto::ContractCall(invocation) => ExecutableRef::ContractCall(invocation),
            ExecutableRefDto::Instructions(v) => ExecutableRef::Instructions(v),
            ExecutableRefDto::Batch(items) => ExecutableRef::Batch(items),
        })
    }
}
#[derive(Encode, Decode)]
struct IvmBytecodeEntryDto {
    original_contract: IvmBytecode,
    code_hash: Hash,
    count: u64,
}
impl From<&IvmBytecodeEntry> for IvmBytecodeEntryDto {
    fn from(e: &IvmBytecodeEntry) -> Self {
        IvmBytecodeEntryDto {
            original_contract: e.original_contract.clone(),
            code_hash: e.code_hash,
            count: e.count.get(),
        }
    }
}
impl TryFrom<IvmBytecodeEntryDto> for IvmBytecodeEntry {
    type Error = String;
    fn try_from(dto: IvmBytecodeEntryDto) -> Result<Self, Self::Error> {
        let nz = NonZeroU64::new(dto.count).ok_or_else(|| "count must be non-zero".to_string())?;
        if ivm::contract_code_hash(dto.original_contract.as_ref()) != dto.code_hash {
            return Err(
                "trigger contract code hash does not match its deployable artifact".to_owned(),
            );
        }
        Ok(IvmBytecodeEntry {
            original_contract: dto.original_contract,
            code_hash: dto.code_hash,
            count: nz,
        })
    }
}
#[derive(Encode, Decode, Clone)]
struct LoadedActionDto<F> {
    executable: ExecutableRefDto,
    repeats: Repeats,
    authority: AccountId,
    filter: F,
    retry_policy: Option<TimeTriggerRetryPolicy>,
    retry_state: Option<TimeTriggerRetryState>,
    metadata: Metadata,
}
impl<F: Clone> From<&LoadedAction<F>> for LoadedActionDto<F> {
    fn from(a: &LoadedAction<F>) -> Self {
        LoadedActionDto {
            executable: ExecutableRefDto::from(&a.executable),
            repeats: a.repeats,
            authority: a.authority.clone(),
            filter: a.filter.clone(),
            retry_policy: a.retry_policy,
            retry_state: a.retry_state,
            metadata: a.metadata.clone(),
        }
    }
}
impl<F> TryFrom<LoadedActionDto<F>> for LoadedAction<F> {
    type Error = String;
    fn try_from(dto: LoadedActionDto<F>) -> Result<Self, Self::Error> {
        Ok(LoadedAction {
            executable: ExecutableRef::try_from(dto.executable)?,
            repeats: dto.repeats,
            authority: dto.authority,
            filter: dto.filter,
            retry_policy: dto.retry_policy,
            retry_state: dto.retry_state,
            metadata: dto.metadata,
        })
    }
}
/// Read-only accessor trait for trigger sets.
pub trait SetReadOnly {
    /// Data triggers map.
    fn data_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<DataEventFilter>>;
    /// Pipeline event triggers map.
    fn pipeline_triggers(
        &self,
    ) -> &impl StorageReadOnly<TriggerId, LoadedAction<PipelineEventFilterBox>>;
    /// Time triggers map.
    fn time_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<TimeEventFilter>>;
    /// Execute-by-call triggers map.
    fn by_call_triggers(
        &self,
    ) -> &impl StorageReadOnly<TriggerId, LoadedAction<ExecuteTriggerEventFilter>>;
    /// Trigger type registry.
    fn ids(&self) -> &impl StorageReadOnly<TriggerId, TriggeringEventType>;
    /// Active data trigger id registry.
    fn active_data_trigger_ids(&self) -> &impl StorageReadOnly<TriggerId, ()>;
    /// Active pipeline trigger id registry.
    fn active_pipeline_trigger_ids(&self) -> &impl StorageReadOnly<TriggerId, ()>;
    /// Active time trigger id registry.
    fn active_time_trigger_ids(&self) -> &impl StorageReadOnly<TriggerId, ()>;
    /// Active by-call trigger id registry.
    fn active_by_call_trigger_ids(&self) -> &impl StorageReadOnly<TriggerId, ()>;
    /// Mapping from code hash to bytecode entry.
    fn contracts(&self) -> &impl StorageReadOnly<HashOf<IvmBytecode>, IvmBytecodeEntry>;
    /// Get original [`IvmBytecode`] for [`TriggerId`]. Returns `None` if there's no [`Trigger`]
    /// with specified `id` that has IVM executable
    #[inline]
    fn get_original_contract(&self, hash: &HashOf<IvmBytecode>) -> Option<&IvmBytecode> {
        self.contracts()
            .get(hash)
            .map(|entry| &entry.original_contract)
    }
    /// Borrow original trigger bytecode together with its validated deployable hash.
    #[inline]
    fn get_original_contract_with_code_hash(
        &self,
        hash: &HashOf<IvmBytecode>,
    ) -> Option<(&IvmBytecode, Hash)> {
        self.contracts()
            .get(hash)
            .map(|entry| (&entry.original_contract, entry.code_hash))
    }
    /// Convert [`LoadedAction`] to original [`Action`] by retrieving original
    /// [`IvmBytecode`] if applicable.
    ///
    /// Returns `None` when the original bytecode is missing or stored action
    /// invariants no longer validate.
    fn get_original_action<F>(&self, action: LoadedAction<F>) -> Option<Action>
    where
        F: Clone + EnsureTriggerAuthority + Into<EventFilterBox>,
    {
        let LoadedAction {
            executable,
            repeats,
            authority,
            filter,
            retry_policy,
            retry_state: _,
            metadata,
        } = action;
        let original_executable = match executable {
            ExecutableRef::Ivm(ref blob_hash) => {
                let Some(original_contract) = self.get_original_contract(blob_hash).cloned() else {
                    warn!(
                        ?blob_hash,
                        "missing original trigger bytecode; skipping trigger action"
                    );
                    return None;
                };
                Executable::Ivm(original_contract)
            }
            ExecutableRef::ContractCall(invocation) => Executable::ContractCall(invocation),
            ExecutableRef::Instructions(isi) => Executable::Instructions(isi),
            ExecutableRef::Batch(items) => Executable::Batch(items),
        };
        let mut specialized =
            match SpecializedAction::new(original_executable, repeats, authority, filter) {
                Ok(action) => action,
                Err(error) => {
                    warn!(%error, "stored trigger action violates its authority invariant");
                    return None;
                }
            };
        specialized.retry_policy = retry_policy;
        specialized.metadata = metadata;
        match Action::try_from(specialized) {
            Ok(action) => Some(action),
            Err(error) => {
                warn!(%error, "stored trigger action violates its validation invariants");
                None
            }
        }
    }
    /// Get all contained trigger ids without a particular order
    #[inline]
    fn ids_iter(&self) -> impl Iterator<Item = &TriggerId> {
        self.ids().iter().map(|(trigger_id, _)| trigger_id)
    }
    /// Iterate active trigger ids from id-only registries maintained on writes.
    fn active_trigger_ids_iter(&self) -> impl Iterator<Item = &TriggerId> {
        self.active_data_trigger_ids()
            .iter()
            .map(|(id, _)| id)
            .chain(self.active_pipeline_trigger_ids().iter().map(|(id, _)| id))
            .chain(self.active_time_trigger_ids().iter().map(|(id, _)| id))
            .chain(self.active_by_call_trigger_ids().iter().map(|(id, _)| id))
    }
    /// Iterate triggers directly from the typed trigger stores.
    fn triggers_iter(&self) -> impl Iterator<Item = Trigger> + '_ {
        self.data_triggers()
            .iter()
            .filter_map(move |(id, action)| {
                self.get_original_action(action.clone())
                    .map(|action| Trigger::new(id.clone(), action))
            })
            .chain(
                self.pipeline_triggers()
                    .iter()
                    .filter_map(move |(id, action)| {
                        self.get_original_action(action.clone())
                            .map(|action| Trigger::new(id.clone(), action))
                    }),
            )
            .chain(self.time_triggers().iter().filter_map(move |(id, action)| {
                self.get_original_action(action.clone())
                    .map(|action| Trigger::new(id.clone(), action))
            }))
            .chain(
                self.by_call_triggers()
                    .iter()
                    .filter_map(move |(id, action)| {
                        self.get_original_action(action.clone())
                            .map(|action| Trigger::new(id.clone(), action))
                    }),
            )
    }
    /// Project one stored trigger through the active bounded singular-query corridor.
    fn bounded_trigger<F>(
        &self,
        id: &TriggerId,
        event_type: TriggeringEventType,
        action: &LoadedAction<F>,
    ) -> core::result::Result<Option<Trigger>, iroha_data_model::query::error::QueryExecutionFail>
    where
        F: norito::core::NoritoSerialize,
    {
        let (executable_discriminant, executable): (u32, &dyn norito::core::NoritoSerialize) =
            match &action.executable {
                ExecutableRef::Instructions(instructions) => (0, instructions),
                ExecutableRef::ContractCall(invocation) => (1, invocation),
                ExecutableRef::Ivm(blob_hash) => {
                    let Some(contract) = self.get_original_contract(blob_hash) else {
                        warn!(
                            ?blob_hash,
                            "missing original trigger bytecode; skipping trigger action"
                        );
                        return Ok(None);
                    };
                    (2, contract)
                }
                ExecutableRef::Batch(items) => (4, items),
            };
        let executable =
            BorrowedEnumVariant::<Executable>::new(executable_discriminant, executable);
        let filter_discriminant = match event_type {
            TriggeringEventType::Pipeline => 0,
            TriggeringEventType::Data => 1,
            TriggeringEventType::Time => 2,
            TriggeringEventType::ExecuteTrigger => 3,
        };
        let filter =
            BorrowedEnumVariant::<EventFilterBox>::new(filter_discriminant, &action.filter);
        let retry_policy = crate::smartcontracts::isi::query::BorrowedSingularOption::new(
            action.retry_policy.as_ref(),
        );
        let action = crate::smartcontracts::isi::query::BorrowedSingularStruct::<Action, 6>::new([
            &executable,
            &action.repeats,
            &action.authority,
            &filter,
            &retry_policy,
            &action.metadata,
        ]);
        crate::smartcontracts::isi::query::own_singular_query_struct::<Trigger, 2>(
            [id, &action],
            || unreachable!("bounded trigger projection requires active singular limits"),
        )
        .map(Some)
    }
    /// Resolve one trigger without cloning its variable-size action before the
    /// active singular-query corridor has admitted it.
    fn trigger_by_id_bounded(
        &self,
        id: &TriggerId,
    ) -> core::result::Result<Option<Trigger>, iroha_data_model::query::error::QueryExecutionFail>
    {
        if !crate::smartcontracts::isi::query::singular_query_limits_active() {
            return Ok(self.trigger_by_id(id));
        }
        let Some(event_type) = self.ids().get(id).copied() else {
            return Ok(None);
        };
        let action = match event_type {
            TriggeringEventType::Data => self
                .data_triggers()
                .get(id)
                .map(|action| self.bounded_trigger(id, event_type, action))
                .transpose()?
                .flatten(),
            TriggeringEventType::Pipeline => self
                .pipeline_triggers()
                .get(id)
                .map(|action| self.bounded_trigger(id, event_type, action))
                .transpose()?
                .flatten(),
            TriggeringEventType::Time => self
                .time_triggers()
                .get(id)
                .map(|action| self.bounded_trigger(id, event_type, action))
                .transpose()?
                .flatten(),
            TriggeringEventType::ExecuteTrigger => self
                .by_call_triggers()
                .get(id)
                .map(|action| self.bounded_trigger(id, event_type, action))
                .transpose()?
                .flatten(),
        };
        if action.is_none() {
            warn!(
                trigger_id = %id,
                ?event_type,
                "trigger id missing from typed map while resolving trigger"
            );
        }
        Ok(action)
    }
    /// Resolve one trigger by identifier.
    fn trigger_by_id(&self, id: &TriggerId) -> Option<Trigger> {
        let event_type = self.ids().get(id).copied()?;
        let trigger = match event_type {
            TriggeringEventType::Data => self
                .data_triggers()
                .get(id)
                .cloned()
                .and_then(|action| self.get_original_action(action))
                .map(|action| Trigger::new(id.clone(), action)),
            TriggeringEventType::Pipeline => self
                .pipeline_triggers()
                .get(id)
                .cloned()
                .and_then(|action| self.get_original_action(action))
                .map(|action| Trigger::new(id.clone(), action)),
            TriggeringEventType::Time => self
                .time_triggers()
                .get(id)
                .cloned()
                .and_then(|action| self.get_original_action(action))
                .map(|action| Trigger::new(id.clone(), action)),
            TriggeringEventType::ExecuteTrigger => self
                .by_call_triggers()
                .get(id)
                .cloned()
                .and_then(|action| self.get_original_action(action))
                .map(|action| Trigger::new(id.clone(), action)),
        };
        if trigger.is_none() {
            warn!(
                trigger_id = %id,
                ?event_type,
                "trigger id missing from typed map while resolving trigger"
            );
        }
        trigger
    }
    /// Returns a bounded iterator of trigger ids matching a given time event.
    ///
    /// Retry attempts are selected first. Scheduled matches beyond `max_invocations` are discarded
    /// with the elapsed interval and are not carried into a later block.
    fn match_time_event(
        &self,
        event: TimeEvent,
        current_block_height: u64,
        current_block_time_ms: u64,
        max_invocations: usize,
    ) -> impl Iterator<Item = TriggerId> + '_ {
        // Retry invocations retain their existing priority over ordinary
        // schedule matches, but both categories share one consensus resource
        // cap so an elapsed interval can never allocate an unbounded clone list.
        let mut due_retries = Vec::with_capacity(max_invocations.min(self.time_triggers().len()));
        for (id, action) in self.time_triggers().iter() {
            if due_retries.len() == max_invocations {
                break;
            }
            if !time_trigger_action_is_due(
                action,
                &event,
                current_block_height,
                current_block_time_ms,
            ) {
                continue;
            }
            if action.retry_state.is_some() {
                due_retries.push(id.clone());
            }
        }
        let scheduled_capacity = max_invocations.saturating_sub(due_retries.len());
        let mut scheduled = Vec::with_capacity(scheduled_capacity);
        for (id, action) in self.time_triggers().iter() {
            if scheduled.len() == scheduled_capacity {
                break;
            }
            if !time_trigger_action_is_due(
                action,
                &event,
                current_block_height,
                current_block_time_ms,
            ) || action.retry_state.is_some()
            {
                continue;
            }
            let mut count = action.filter.count_matches(&event);
            if let Repeats::Exactly(repeats) = action.repeats {
                count = min(repeats, count);
            }
            let count = usize::try_from(count)
                .unwrap_or(usize::MAX)
                .min(scheduled_capacity - scheduled.len());
            for _ in 0..count {
                scheduled.push(id.clone());
            }
        }
        due_retries.into_iter().chain(scheduled)
    }
    /// Returns trigger ids matching a deterministic pipeline event.
    ///
    /// Triggers registered at `current_block_height` are excluded so a pipeline
    /// event cannot execute a trigger created by the same block.
    fn match_pipeline_event<'a>(
        &'a self,
        event: &'a PipelineEventBox,
        current_block_height: u64,
    ) -> impl Iterator<Item = TriggerId> + 'a {
        self.pipeline_triggers()
            .iter()
            .filter(move |(_, action)| {
                pipeline_trigger_action_matches(action, event, current_block_height)
            })
            .map(|(id, _)| id.clone())
    }
    /// Get [`ExecutableRef`] for given [`TriggerId`]. Returns `None` if `id` is not in the set.
    fn get_executable(&self, id: &TriggerId) -> Option<&ExecutableRef> {
        let event_type = self.ids().get(id)?;
        let executable = match event_type {
            TriggeringEventType::Data => {
                self.data_triggers().get(id).map(|entry| &entry.executable)
            }
            TriggeringEventType::Pipeline => self
                .pipeline_triggers()
                .get(id)
                .map(|entry| &entry.executable),
            TriggeringEventType::Time => {
                self.time_triggers().get(id).map(|entry| &entry.executable)
            }
            TriggeringEventType::ExecuteTrigger => self
                .by_call_triggers()
                .get(id)
                .map(|entry| &entry.executable),
        };
        if executable.is_none() {
            warn!(
                trigger_id = %id,
                ?event_type,
                "trigger id missing from typed map while resolving executable"
            );
        }
        executable
    }
    /// Apply `f` to triggers whose action satisfies the predicate.
    ///
    /// Return an empty list if [`Set`] doesn't contain any such triggers.
    fn inspect_by_action<'a, P, F, R>(&'a self, filter: P, f: F) -> impl Iterator<Item = R> + 'a
    where
        P: Fn(&dyn LoadedActionTrait) -> bool + 'a,
        F: Fn(&TriggerId, &dyn LoadedActionTrait) -> R + 'a,
    {
        self.ids()
            .iter()
            .filter_map(move |(id, event_type)| match event_type {
                TriggeringEventType::Data => {
                    let Some(action) = self.data_triggers().get(id) else {
                        warn!(
                            trigger_id = %id,
                            ?event_type,
                            "trigger id missing from typed map while iterating triggers"
                        );
                        return None;
                    };
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::Pipeline => {
                    let Some(action) = self.pipeline_triggers().get(id) else {
                        warn!(
                            trigger_id = %id,
                            ?event_type,
                            "trigger id missing from typed map while iterating triggers"
                        );
                        return None;
                    };
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::Time => {
                    let Some(action) = self.time_triggers().get(id) else {
                        warn!(
                            trigger_id = %id,
                            ?event_type,
                            "trigger id missing from typed map while iterating triggers"
                        );
                        return None;
                    };
                    filter(action).then(|| f(id, action))
                }
                TriggeringEventType::ExecuteTrigger => {
                    let Some(action) = self.by_call_triggers().get(id) else {
                        warn!(
                            trigger_id = %id,
                            ?event_type,
                            "trigger id missing from typed map while iterating triggers"
                        );
                        return None;
                    };
                    filter(action).then(|| f(id, action))
                }
            })
    }
    /// Apply `f` to the trigger identified by `id`.
    ///
    /// Return [`None`] if [`Set`] doesn't contain the trigger with the given `id`.
    fn inspect_by_id<F, R>(&self, id: &TriggerId, f: F) -> Option<R>
    where
        F: Fn(&dyn LoadedActionTrait) -> R,
    {
        let event_type = self.ids().get(id).copied()?;
        let result = match event_type {
            TriggeringEventType::Data => self.data_triggers().get(id).map(|entry| f(entry)),
            TriggeringEventType::Pipeline => self.pipeline_triggers().get(id).map(|entry| f(entry)),
            TriggeringEventType::Time => self.time_triggers().get(id).map(|entry| f(entry)),
            TriggeringEventType::ExecuteTrigger => {
                self.by_call_triggers().get(id).map(|entry| f(entry))
            }
        };
        if result.is_none() {
            warn!(
                trigger_id = %id,
                ?event_type,
                "trigger id missing from typed map while inspecting trigger"
            );
        }
        result
    }
}
macro_rules! impl_set_ro {
    ($($ident:ty),*) => {$(
        impl SetReadOnly for $ident {
            fn data_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<DataEventFilter>> {
                &self.data_triggers
            }
            fn pipeline_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<PipelineEventFilterBox>> {
                &self.pipeline_triggers
            }
            fn time_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<TimeEventFilter>> {
                &self.time_triggers
            }
            fn by_call_triggers(&self) -> &impl StorageReadOnly<TriggerId, LoadedAction<ExecuteTriggerEventFilter>> {
                &self.by_call_triggers
            }
            fn ids(&self) -> &impl StorageReadOnly<TriggerId, TriggeringEventType> {
                &self.ids
            }
            fn active_data_trigger_ids(&self) -> &impl StorageReadOnly<TriggerId, ()> {
                &self.active_data_trigger_ids
            }
            fn active_pipeline_trigger_ids(&self) -> &impl StorageReadOnly<TriggerId, ()> {
                &self.active_pipeline_trigger_ids
            }
            fn active_time_trigger_ids(&self) -> &impl StorageReadOnly<TriggerId, ()> {
                &self.active_time_trigger_ids
            }
            fn active_by_call_trigger_ids(&self) -> &impl StorageReadOnly<TriggerId, ()> {
                &self.active_by_call_trigger_ids
            }
            fn contracts(&self) -> &impl StorageReadOnly<HashOf<IvmBytecode>, IvmBytecodeEntry> {
                &self.contracts
            }
        }
    )*};
}
impl_set_ro! {
    SetBlock<'_>, SetTransaction<'_, '_>, SetView<'_>
}
impl Set {
    /// Create struct to apply block's changes
    pub fn block(&self) -> SetBlock<'_> {
        SetBlock {
            data_triggers: self.data_triggers.block(),
            pipeline_triggers: self.pipeline_triggers.block(),
            time_triggers: self.time_triggers.block(),
            by_call_triggers: self.by_call_triggers.block(),
            ids: self.ids.block(),
            active_data_trigger_ids: self.active_data_trigger_ids.block(),
            active_pipeline_trigger_ids: self.active_pipeline_trigger_ids.block(),
            active_time_trigger_ids: self.active_time_trigger_ids.block(),
            active_by_call_trigger_ids: self.active_by_call_trigger_ids.block(),
            contracts: self.contracts.block(),
        }
    }
    /// Create struct to apply block's changes while reverting changes made in the latest block
    pub fn block_and_revert(&self) -> SetBlock<'_> {
        SetBlock {
            data_triggers: self.data_triggers.block_and_revert(),
            pipeline_triggers: self.pipeline_triggers.block_and_revert(),
            time_triggers: self.time_triggers.block_and_revert(),
            by_call_triggers: self.by_call_triggers.block_and_revert(),
            ids: self.ids.block_and_revert(),
            active_data_trigger_ids: self.active_data_trigger_ids.block_and_revert(),
            active_pipeline_trigger_ids: self.active_pipeline_trigger_ids.block_and_revert(),
            active_time_trigger_ids: self.active_time_trigger_ids.block_and_revert(),
            active_by_call_trigger_ids: self.active_by_call_trigger_ids.block_and_revert(),
            contracts: self.contracts.block_and_revert(),
        }
    }
    /// Create point in time view of the [`Set`]
    pub fn view(&self) -> SetView<'_> {
        SetView {
            data_triggers: self.data_triggers.view(),
            pipeline_triggers: self.pipeline_triggers.view(),
            time_triggers: self.time_triggers.view(),
            by_call_triggers: self.by_call_triggers.view(),
            ids: self.ids.view(),
            active_data_trigger_ids: self.active_data_trigger_ids.view(),
            active_pipeline_trigger_ids: self.active_pipeline_trigger_ids.view(),
            active_time_trigger_ids: self.active_time_trigger_ids.view(),
            active_by_call_trigger_ids: self.active_by_call_trigger_ids.view(),
            contracts: self.contracts.view(),
        }
    }
    /// Test-only helper to drop a trigger bytecode entry and commit the change.
    #[cfg(test)]
    pub(crate) fn remove_contract_for_test(&mut self, hash: HashOf<IvmBytecode>) -> bool {
        let mut block = self.block();
        let mut tx = block.transaction();
        let removed = tx.contracts.remove(hash).is_some();
        tx.apply();
        block.commit();
        removed
    }
}
impl<'set> SetBlock<'set> {
    /// Create struct to apply transaction's changes
    pub fn transaction(&mut self) -> SetTransaction<'_, 'set> {
        let data_trigger_index = DataTriggerIndex::from_triggers(&self.data_triggers);
        SetTransaction {
            next_registration_generation: 0,
            registration_generations: BTreeMap::new(),
            data_trigger_eligibility_generations: BTreeMap::new(),
            data_trigger_index,
            data_triggers: self.data_triggers.transaction(),
            pipeline_triggers: self.pipeline_triggers.transaction(),
            time_triggers: self.time_triggers.transaction(),
            by_call_triggers: self.by_call_triggers.transaction(),
            ids: self.ids.transaction(),
            active_data_trigger_ids: self.active_data_trigger_ids.transaction(),
            active_pipeline_trigger_ids: self.active_pipeline_trigger_ids.transaction(),
            active_time_trigger_ids: self.active_time_trigger_ids.transaction(),
            active_by_call_trigger_ids: self.active_by_call_trigger_ids.transaction(),
            contracts: self.contracts.transaction(),
        }
    }
    /// Commit block's changes
    pub fn commit(self) {
        // NOTE: commit in reverse order
        self.contracts.commit();
        self.active_by_call_trigger_ids.commit();
        self.active_time_trigger_ids.commit();
        self.active_pipeline_trigger_ids.commit();
        self.active_data_trigger_ids.commit();
        self.ids.commit();
        self.by_call_triggers.commit();
        self.time_triggers.commit();
        self.pipeline_triggers.commit();
        self.data_triggers.commit();
    }
    /// Returns a bounded iterator of trigger ids matching a given time event.
    pub fn match_time_event(
        &self,
        event: TimeEvent,
        current_block_height: u64,
        current_block_time_ms: u64,
        max_invocations: usize,
    ) -> impl Iterator<Item = TriggerId> + '_ {
        <Self as SetReadOnly>::match_time_event(
            self,
            event,
            current_block_height,
            current_block_time_ms,
            max_invocations,
        )
    }
    /// Returns trigger ids matching a deterministic pipeline event outside their registration block.
    pub fn match_pipeline_event<'a>(
        &'a self,
        event: &'a PipelineEventBox,
        current_block_height: u64,
    ) -> impl Iterator<Item = TriggerId> + 'a {
        <Self as SetReadOnly>::match_pipeline_event(self, event, current_block_height)
    }
}
trait TriggeringEventFilter: EventFilter {}
impl TriggeringEventFilter for DataEventFilter {}
impl TriggeringEventFilter for PipelineEventFilterBox {}
impl TriggeringEventFilter for TimeEventFilter {}
impl TriggeringEventFilter for ExecuteTriggerEventFilter {}
impl<'block, 'set> SetTransaction<'block, 'set> {
    /// Current transaction-local registration generation for `id`.
    ///
    /// IDs inherited from the parent block have generation zero. Every
    /// successful registration in this overlay advances the generation.
    pub(crate) fn registration_generation(&self, id: &TriggerId) -> u64 {
        self.registration_generations.get(id).copied().unwrap_or(0)
    }
    fn advance_registration_generation(&mut self) -> u64 {
        self.next_registration_generation = self
            .next_registration_generation
            .checked_add(1)
            .expect("trigger lifecycle generation must not overflow within one transaction");
        self.next_registration_generation
    }
    /// Freeze data-trigger eligibility for one buffered event batch.
    pub(crate) fn data_trigger_match_snapshot(&self) -> DataTriggerMatchSnapshot {
        DataTriggerMatchSnapshot {
            generation_watermark: self.next_registration_generation,
        }
    }
    /// Return the pre-allocation work charge for indexed candidates of `event`.
    pub(crate) fn data_trigger_candidate_scan_work(&self, event: &DataEvent) -> usize {
        self.data_trigger_index.candidate_scan_work(event)
    }
    /// Return canonically ordered candidates from the live deterministic index.
    pub(crate) fn data_trigger_candidates(&self, event: &DataEvent) -> Vec<TriggerId> {
        self.data_trigger_index.candidates(event)
    }
    /// Match a live candidate only if it was continuously eligible at capture.
    pub(crate) fn data_trigger_matching_generation(
        &self,
        snapshot: DataTriggerMatchSnapshot,
        id: &TriggerId,
        event: &DataEvent,
    ) -> Option<u64> {
        let generation = self.registration_generation(id);
        let eligible_since = self
            .data_trigger_eligibility_generations
            .get(id)
            .copied()
            .unwrap_or(0);
        if generation > snapshot.generation_watermark
            || eligible_since > snapshot.generation_watermark
        {
            return None;
        }
        let action = self.data_triggers.get(id)?;
        (trigger_is_enabled(action.metadata())
            && !action.repeats.is_depleted()
            && action.filter.matches(event))
        .then_some(generation)
    }
    fn set_active_id(
        active_ids: &mut ActiveTriggerIdStoreTransaction<'block, 'set>,
        id: &TriggerId,
        active: bool,
    ) {
        if active {
            active_ids.insert(id.clone(), ());
        } else {
            active_ids.remove(id.clone());
        }
    }
    fn set_active_id_by_event_type(
        &mut self,
        event_type: TriggeringEventType,
        id: &TriggerId,
        active: bool,
    ) {
        match event_type {
            TriggeringEventType::Data => {
                Self::set_active_id(&mut self.active_data_trigger_ids, id, active);
            }
            TriggeringEventType::Pipeline => {
                Self::set_active_id(&mut self.active_pipeline_trigger_ids, id, active);
            }
            TriggeringEventType::Time => {
                Self::set_active_id(&mut self.active_time_trigger_ids, id, active);
            }
            TriggeringEventType::ExecuteTrigger => {
                Self::set_active_id(&mut self.active_by_call_trigger_ids, id, active);
            }
        }
    }
    /// Apply transaction's changes
    pub fn apply(self) {
        // NOTE: apply in reverse order
        self.contracts.apply();
        self.active_by_call_trigger_ids.apply();
        self.active_time_trigger_ids.apply();
        self.active_pipeline_trigger_ids.apply();
        self.active_data_trigger_ids.apply();
        self.ids.apply();
        self.by_call_triggers.apply();
        self.time_triggers.apply();
        self.pipeline_triggers.apply();
        self.data_triggers.apply();
    }
    /// Return global-trigger capability grants whose target must follow a rekey.
    ///
    /// Each pair contains the current grantee and the grantee after the account
    /// migration. The caller still has to verify that the old direct capability
    /// is live; a revoked capability must never be resurrected by rekeying.
    pub(crate) fn global_data_trigger_permission_rekeys(
        &self,
        old: &AccountId,
        new: &AccountId,
    ) -> Result<BTreeSet<(AccountId, AccountId)>> {
        self.ensure_account_id_rekey_capacity(old, new)?;
        if old == new {
            return Ok(BTreeSet::new());
        }
        let mut rekeys = BTreeSet::new();
        for (_, action) in self.data_triggers.iter() {
            let grantee = data_trigger_global_permission_grantee(action.metadata())
                .map_err(|()| Error::InvalidDataScopeAuthorization)?;
            if action.authority != *old {
                continue;
            }
            if let Some(grantee) = grantee {
                let migrated = if grantee == *old {
                    new.clone()
                } else {
                    grantee.clone()
                };
                rekeys.insert((grantee, migrated));
            }
        }
        Ok(rekeys)
    }

    fn ensure_account_id_rekey_capacity(&self, old: &AccountId, new: &AccountId) -> Result<()> {
        if old == new {
            return Ok(());
        }
        let merged_authority_count = self
            .data_triggers
            .iter()
            .filter(|(_, action)| action.authority == *old || action.authority == *new)
            .count();
        if merged_authority_count > super::isi::MAX_DATA_TRIGGERS_PER_AUTHORITY {
            return Err(Error::DataTriggerAuthorityCapacity(new.clone()));
        }
        Ok(())
    }

    /// Replace occurrences of `old` with `new` in trigger authorities, filters,
    /// and captured global-capability provenance.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidDataScopeAuthorization`] before mutation when a
    /// persisted data trigger lacks the canonical v1 authorization record, or
    /// [`Error::DataTriggerAuthorityCapacity`] when merging the two authorities
    /// would exceed the first-release per-authority trigger cap.
    pub fn replace_account_id(&mut self, old: &AccountId, new: &AccountId) -> Result<()> {
        if old == new {
            return Ok(());
        }
        self.ensure_account_id_rekey_capacity(old, new)?;
        if self
            .data_triggers
            .iter()
            .any(|(_, action)| !data_trigger_scope_authorization_is_well_formed(action.metadata()))
        {
            return Err(Error::InvalidDataScopeAuthorization);
        }
        let trigger_ids: Vec<_> = self.ids.iter().map(|(id, _)| id.clone()).collect();
        for trigger_id in trigger_ids {
            let Some(event_type) = self.ids.get(&trigger_id).copied() else {
                continue;
            };
            let mut replacement_filter = None;
            let mut replacement_filter_is_active = false;
            let updated = match event_type {
                TriggeringEventType::Data => {
                    let updated = self.data_triggers.get_mut(&trigger_id).map(|action| {
                        let mut updated = replace_trigger_authority(action, old, new);
                        if action.filter.replace_account_id(old, new) {
                            replacement_filter = Some(action.filter.clone());
                            replacement_filter_is_active = Set::action_is_active(action);
                            updated = true;
                        }
                        updated |= replace_data_trigger_global_permission_grantee(
                            &mut action.metadata,
                            old,
                            new,
                        )
                        .expect("data-trigger authorization was preflighted");
                        updated
                    });
                    if let Some(filter) = replacement_filter {
                        self.data_trigger_index.remove(&trigger_id);
                        self.data_trigger_index.insert(&trigger_id, &filter);
                        if replacement_filter_is_active {
                            let generation = self.advance_registration_generation();
                            self.data_trigger_eligibility_generations
                                .insert(trigger_id.clone(), generation);
                        }
                    }
                    updated
                }
                TriggeringEventType::Pipeline => self
                    .pipeline_triggers
                    .get_mut(&trigger_id)
                    .map(|action| replace_trigger_authority(action, old, new)),
                TriggeringEventType::Time => self
                    .time_triggers
                    .get_mut(&trigger_id)
                    .map(|action| replace_trigger_authority(action, old, new)),
                TriggeringEventType::ExecuteTrigger => self
                    .by_call_triggers
                    .get_mut(&trigger_id)
                    .map(|action| replace_by_call_authority(action, old, new)),
            };
            if updated.is_none() {
                warn!(
                    trigger_id = %trigger_id,
                    ?event_type,
                    "`Set` ids referenced a missing trigger while rekeying"
                );
            }
        }
        Ok(())
    }
    /// Add trigger with [`DataEventFilter`]
    ///
    /// Return `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Returns [`Err`] when scope authorization is malformed or a global/per-authority
    /// registration cap has already been reached.
    #[inline]
    pub fn add_data_trigger(
        &mut self,
        trigger: SpecializedTrigger<DataEventFilter>,
    ) -> Result<bool> {
        if self.ids.get(&trigger.id).is_some() {
            return Ok(false);
        }
        if !data_trigger_scope_authorization_is_well_formed(&trigger.action.metadata) {
            return Err(Error::InvalidDataScopeAuthorization);
        }
        if self.data_triggers.len() >= super::isi::MAX_DATA_TRIGGERS_TOTAL {
            return Err(Error::DataTriggerCapacity);
        }
        let authority = &trigger.action.authority;
        if self
            .data_triggers
            .iter()
            .filter(|(_, action)| &action.authority == authority)
            .count()
            >= super::isi::MAX_DATA_TRIGGERS_PER_AUTHORITY
        {
            return Err(Error::DataTriggerAuthorityCapacity(authority.clone()));
        }
        let trigger_id = trigger.id.clone();
        let filter = trigger.action.filter.clone();
        let added = self.add_to(trigger, TriggeringEventType::Data, |me| {
            &mut me.data_triggers
        });
        if added {
            self.data_trigger_index.insert(&trigger_id, &filter);
        }
        Ok(added)
    }
    /// Add trigger with [`PipelineEventFilterBox`]
    ///
    /// Return `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    #[inline]
    pub fn add_pipeline_trigger(
        &mut self,
        trigger: SpecializedTrigger<PipelineEventFilterBox>,
    ) -> Result<bool> {
        Ok(self.add_to(trigger, TriggeringEventType::Pipeline, |me| {
            &mut me.pipeline_triggers
        }))
    }
    /// Add trigger with [`TimeEventFilter`]
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    #[inline]
    pub fn add_time_trigger(
        &mut self,
        trigger: SpecializedTrigger<TimeEventFilter>,
    ) -> Result<bool> {
        Ok(self.add_to(trigger, TriggeringEventType::Time, |me| {
            &mut me.time_triggers
        }))
    }
    /// Add trigger with [`ExecuteTriggerEventFilter`]
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    #[inline]
    pub fn add_by_call_trigger(
        &mut self,
        trigger: SpecializedTrigger<ExecuteTriggerEventFilter>,
    ) -> Result<bool> {
        Ok(
            self.add_to(trigger, TriggeringEventType::ExecuteTrigger, |me| {
                &mut me.by_call_triggers
            }),
        )
    }
    /// Add generic trigger to generic collection
    ///
    /// Returns `false` if a trigger with given id already exists
    ///
    /// # Errors
    ///
    /// Return [`Err`] if failed to preload IVM trigger
    fn add_to<F: TriggeringEventFilter + mv::Value>(
        &mut self,
        trigger: SpecializedTrigger<F>,
        event_type: TriggeringEventType,
        map: impl FnOnce(&mut Self) -> &mut StorageTransaction<'block, 'set, TriggerId, LoadedAction<F>>,
    ) -> bool {
        let SpecializedTrigger {
            id: trigger_id,
            action:
                SpecializedAction {
                    executable,
                    repeats,
                    authority,
                    filter,
                    retry_policy,
                    metadata,
                },
        } = trigger;
        if self.ids.get(&trigger_id).is_some() {
            return false;
        }
        let active = !repeats.is_depleted() && trigger_is_enabled(&metadata);
        let loaded_executable = match executable {
            Executable::Ivm(bytes) => {
                let hash = HashOf::new(&bytes);
                if let Some(IvmBytecodeEntry { count, .. }) = self.contracts.get_mut(&hash) {
                    let updated = count.get().strict_add(1);
                    *count = NonZeroU64::new(updated).expect(
                        "There is no way someone could register 2^64 amount of same triggers",
                    );
                } else {
                    let code_hash = ivm::contract_code_hash(bytes.as_ref());
                    self.contracts.insert(
                        hash,
                        IvmBytecodeEntry {
                            original_contract: bytes,
                            code_hash,
                            count: NonZeroU64::MIN,
                        },
                    );
                }
                ExecutableRef::Ivm(hash)
            }
            Executable::IvmProved(proved) => {
                // Triggers do not carry proof attachments; treat proved IVM executables as plain
                // bytecode and execute them via the standard IVM trigger machinery.
                let bytes = proved.bytecode;
                let hash = HashOf::new(&bytes);
                if let Some(IvmBytecodeEntry { count, .. }) = self.contracts.get_mut(&hash) {
                    let updated = count.get().strict_add(1);
                    *count = NonZeroU64::new(updated).expect(
                        "There is no way someone could register 2^64 amount of same triggers",
                    );
                } else {
                    let code_hash = ivm::contract_code_hash(bytes.as_ref());
                    self.contracts.insert(
                        hash,
                        IvmBytecodeEntry {
                            original_contract: bytes,
                            code_hash,
                            count: NonZeroU64::MIN,
                        },
                    );
                }
                ExecutableRef::Ivm(hash)
            }
            Executable::ContractCall(invocation) => ExecutableRef::ContractCall(invocation),
            Executable::Instructions(instructions) => ExecutableRef::Instructions(instructions),
            Executable::Batch(items) => ExecutableRef::Batch(items),
        };
        map(self).insert(
            trigger_id.clone(),
            LoadedAction {
                executable: loaded_executable,
                repeats,
                authority,
                filter,
                retry_policy,
                retry_state: None,
                metadata,
            },
        );
        self.ids.insert(trigger_id.clone(), event_type);
        self.set_active_id_by_event_type(event_type, &trigger_id, active);
        let generation = self.advance_registration_generation();
        self.registration_generations
            .insert(trigger_id.clone(), generation);
        if event_type == TriggeringEventType::Data {
            self.data_trigger_eligibility_generations
                .insert(trigger_id, if active { generation } else { u64::MAX });
        }
        true
    }
    /// Apply `f` to the trigger identified by `id`.
    ///
    /// Return [`None`] if [`Set`] doesn't contain the trigger with the given `id`.
    pub fn inspect_by_id_mut<F, R>(&mut self, id: &TriggerId, f: F) -> Option<R>
    where
        F: Fn(&mut dyn LoadedActionTrait) -> R,
    {
        let event_type = self.ids.get(id).copied()?;
        let mut prior_active = None;
        let mut active = None;
        let result = match event_type {
            TriggeringEventType::Data => self.data_triggers.get_mut(id).map(|entry| {
                prior_active = Some(Set::action_is_active(entry));
                let result = f(entry);
                active = Some(Set::action_is_active(entry));
                result
            }),
            TriggeringEventType::Pipeline => self.pipeline_triggers.get_mut(id).map(|entry| {
                let result = f(entry);
                active = Some(Set::action_is_active(entry));
                result
            }),
            TriggeringEventType::Time => self.time_triggers.get_mut(id).map(|entry| {
                let result = f(entry);
                active = Some(Set::action_is_active(entry));
                result
            }),
            TriggeringEventType::ExecuteTrigger => self.by_call_triggers.get_mut(id).map(|entry| {
                let result = f(entry);
                active = Some(Set::action_is_active(entry));
                result
            }),
        };
        if let Some(active) = active {
            self.set_active_id_by_event_type(event_type, id, active);
            if event_type == TriggeringEventType::Data && prior_active == Some(false) && active {
                let generation = self.advance_registration_generation();
                self.data_trigger_eligibility_generations
                    .insert(id.clone(), generation);
            }
        }
        if result.is_none() {
            warn!(
                trigger_id = %id,
                ?event_type,
                "trigger id missing from typed map while mutating trigger"
            );
        }
        result
    }
    /// Remove a trigger from the [`Set`].
    ///
    /// Return `false` if [`Set`] doesn't contain the trigger with the given `id`.
    /// Logs and continues if the internal storage is inconsistent.
    pub fn remove(&mut self, id: &TriggerId) -> bool {
        let Some(event_type) = self.ids.remove(id.clone()) else {
            return false;
        };
        self.registration_generations.remove(id);
        self.data_trigger_eligibility_generations.remove(id);
        self.set_active_id_by_event_type(event_type, id, false);
        let removed = match event_type {
            TriggeringEventType::Data => {
                self.data_trigger_index.remove(id);
                Self::remove_from(&mut self.contracts, &mut self.data_triggers, id.clone())
            }
            TriggeringEventType::Pipeline => {
                Self::remove_from(&mut self.contracts, &mut self.pipeline_triggers, id.clone())
            }
            TriggeringEventType::Time => {
                Self::remove_from(&mut self.contracts, &mut self.time_triggers, id.clone())
            }
            TriggeringEventType::ExecuteTrigger => {
                Self::remove_from(&mut self.contracts, &mut self.by_call_triggers, id.clone())
            }
        };
        if !removed {
            warn!(
                trigger_id = %id,
                ?event_type,
                "`Set` ids referenced a missing trigger while removing"
            );
        }
        true
    }
    /// Modify repetitions of the hook identified by [`TriggerId`].
    ///
    /// # Errors
    ///
    /// - If a trigger with the given id is not found.
    /// - If updating the current trigger `repeats` causes an overflow. Indefinitely
    ///   repeating triggers and triggers set for exact time always cause an overflow.
    pub fn mod_repeats(
        &mut self,
        id: &TriggerId,
        f: impl Fn(u32) -> Result<u32, RepeatsOverflowError>,
    ) -> Result<(), ModRepeatsError> {
        self.inspect_by_id_mut(id, |action| match action.repeats() {
            Repeats::Exactly(repeats) => {
                let new_repeats = f(*repeats)?;
                action.set_repeats(Repeats::Exactly(new_repeats));
                Ok(())
            }
            _ => Err(ModRepeatsError::RepeatsOverflow(RepeatsOverflowError)),
        })
        .ok_or_else(|| ModRepeatsError::NotFound(id.clone()))
        // .flatten() -- unstable
        .and_then(std::convert::identity)
    }
    /// Remove trigger from `triggers` and decrease the counter of the original [`IvmBytecode`].
    ///
    /// Note that this function doesn't remove the trigger from [`Set::ids`].
    ///
    /// Returns `true` if trigger was removed and `false` otherwise.
    fn remove_from<F: mv::Value + EventFilter>(
        contracts: &mut TriggerContractStoreTransaction<'block, 'set>,
        triggers: &mut StorageTransaction<'block, 'set, TriggerId, LoadedAction<F>>,
        trigger_id: TriggerId,
    ) -> bool {
        triggers
            .remove(trigger_id)
            .map(|loaded_action| {
                if let Some(blob_hash) = loaded_action.extract_blob_hash() {
                    Self::remove_original_trigger(contracts, blob_hash);
                }
            })
            .is_some()
    }
    /// Decrease the counter of the original [`IvmBytecode`] by `blob_hash` or remove it if the
    /// counter reaches zero. Logs and skips removal if the bytecode entry is missing.
    fn remove_original_trigger(
        contracts: &mut TriggerContractStoreTransaction,
        blob_hash: HashOf<IvmBytecode>,
    ) {
        #[allow(clippy::option_if_let_else)] // More readable this way
        match contracts.get_mut(&blob_hash) {
            Some(entry) => {
                let count = &mut entry.count;
                if let Some(new_count) = NonZeroU64::new(count.get() - 1) {
                    *count = new_count;
                } else {
                    contracts.remove(blob_hash);
                }
            }
            None => {
                warn!(
                    ?blob_hash,
                    "`Set` contracts missing entry for trigger bytecode; skipping removal"
                );
            }
        }
    }
    /// Decrease `action`s for provided triggers and remove those whose counter reached zero.
    pub fn decrease_repeats<'a>(
        &'a mut self,
        triggers: impl Iterator<Item = &'a TriggerId>,
    ) -> Vec<TriggerId> {
        for id in triggers {
            // Ignoring error if trigger has not `Repeats::Exact(_)` but something else
            let _mod_repeats_res = self.mod_repeats(id, |n| Ok(n.saturating_sub(1)));
        }
        let mut removed = Vec::new();
        let Self {
            registration_generations,
            data_trigger_eligibility_generations,
            data_trigger_index,
            data_triggers,
            pipeline_triggers,
            time_triggers,
            by_call_triggers,
            ids,
            active_data_trigger_ids,
            active_pipeline_trigger_ids,
            active_time_trigger_ids,
            active_by_call_trigger_ids,
            contracts,
            ..
        } = self;
        Self::remove_zeros(
            &mut removed,
            ids,
            active_data_trigger_ids,
            contracts,
            data_triggers,
        );
        Self::remove_zeros(
            &mut removed,
            ids,
            active_pipeline_trigger_ids,
            contracts,
            pipeline_triggers,
        );
        Self::remove_zeros(
            &mut removed,
            ids,
            active_time_trigger_ids,
            contracts,
            time_triggers,
        );
        Self::remove_zeros(
            &mut removed,
            ids,
            active_by_call_trigger_ids,
            contracts,
            by_call_triggers,
        );
        for id in &removed {
            registration_generations.remove(id);
            data_trigger_eligibility_generations.remove(id);
            data_trigger_index.remove(id);
        }
        removed
    }
    /// Update internal retry runtime state for a time trigger.
    pub fn set_time_trigger_retry_state(
        &mut self,
        id: &TriggerId,
        retry_state: Option<TimeTriggerRetryState>,
    ) -> bool {
        self.time_triggers
            .get_mut(id)
            .map(|action| {
                action.retry_state = retry_state;
            })
            .is_some()
    }
    /// Remove actions with zero execution count from `triggers`
    fn remove_zeros<F: mv::Value + EventFilter>(
        removed: &mut Vec<TriggerId>,
        ids: &mut StorageTransaction<'block, 'set, TriggerId, TriggeringEventType>,
        active_ids: &mut ActiveTriggerIdStoreTransaction<'block, 'set>,
        contracts: &mut TriggerContractStoreTransaction<'block, 'set>,
        triggers: &mut StorageTransaction<'block, 'set, TriggerId, LoadedAction<F>>,
    ) {
        let mut to_remove: Vec<TriggerId> = triggers
            .iter()
            .filter(|(_, action)| action.repeats.is_depleted())
            .map(|(id, _)| id.clone())
            .collect();
        for id in &to_remove {
            let removed_id = ids.remove(id.clone()).is_some();
            active_ids.remove(id.clone());
            let removed_trigger = Self::remove_from(contracts, triggers, id.clone());
            if !removed_id || !removed_trigger {
                warn!(
                    trigger_id = %id,
                    removed_id,
                    removed_trigger,
                    "`Set` trigger collections out of sync while removing depleted trigger"
                );
            }
        }
        removed.append(&mut to_remove);
    }
}
fn replace_trigger_authority<F>(
    action: &mut LoadedAction<F>,
    old: &AccountId,
    new: &AccountId,
) -> bool {
    if action.authority == *old {
        action.authority = new.clone();
        true
    } else {
        false
    }
}
fn replace_by_call_authority(
    action: &mut LoadedAction<ExecuteTriggerEventFilter>,
    old: &AccountId,
    new: &AccountId,
) -> bool {
    let mut updated = replace_trigger_authority(action, old, new);
    let update_filter = match action.filter.authority() {
        Some(authority) => authority == old,
        None => action.authority == *new,
    };
    if update_filter {
        action.filter = action.filter.clone().under_authority(new.clone());
        updated = true;
    }
    updated
}
/// Same as [`Executable`], but instead of [`Ivm`](iroha_data_model::transaction::Executable::Ivm)
/// contains hash of the IVM blob Hash of the bytecode used by the trigger
#[derive(Clone)]
pub enum ExecutableRef {
    /// Loaded IVM
    Ivm(HashOf<IvmBytecode>),
    /// By-reference deployed contract invocation.
    ContractCall(ContractInvocation),
    /// Vector of ISI
    Instructions(ConstVec<InstructionBox>),
    /// Ordered batch of ISIs and deployed contract invocations.
    Batch(ConstVec<ExecutableBatchItem>),
}
impl core::fmt::Debug for ExecutableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ivm(hash) => f.debug_tuple("Ivm").field(hash).finish(),
            Self::ContractCall(invocation) => {
                f.debug_tuple("ContractCall").field(invocation).finish()
            }
            Self::Instructions(instructions) => {
                f.debug_tuple("Instructions").field(instructions).finish()
            }
            Self::Batch(items) => f.debug_tuple("Batch").field(items).finish(),
        }
    }
}
#[cfg(feature = "json")]
impl FastJsonWrite for ExecutableRef {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        match self {
            ExecutableRef::Ivm(hash) => {
                norito::json::write_json_string("Ivm", out);
                out.push(':');
                JsonSerializeTrait::json_serialize(hash, out);
            }
            ExecutableRef::ContractCall(invocation) => {
                norito::json::write_json_string("ContractCall", out);
                out.push(':');
                JsonSerializeTrait::json_serialize(invocation, out);
            }
            ExecutableRef::Instructions(instrs) => {
                norito::json::write_json_string("Instructions", out);
                out.push(':');
                JsonSerializeTrait::json_serialize(instrs, out);
            }
            ExecutableRef::Batch(items) => {
                norito::json::write_json_string("Batch", out);
                out.push(':');
                JsonSerializeTrait::json_serialize(items, out);
            }
        }
        out.push('}');
    }
}
#[cfg(feature = "json")]
impl json::JsonDeserialize for ExecutableRef {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        let map = match value {
            norito::json::Value::Object(map) => map,
            other => {
                return Err(json::Error::InvalidField {
                    field: "ExecutableRef".to_owned(),
                    message: format!("expected JSON object with single key, found {other:?}."),
                });
            }
        };
        if map.len() != 1 {
            return Err(json::Error::InvalidField {
                field: "ExecutableRef".to_owned(),
                message: "expected exactly one variant field".to_owned(),
            });
        }
        let (key, inner) = map.into_iter().next().expect("checked map length");
        match key.as_str() {
            "Ivm" => json::from_value(inner).map(ExecutableRef::Ivm),
            "ContractCall" => json::from_value(inner).map(ExecutableRef::ContractCall),
            "Instructions" => json::from_value(inner).map(ExecutableRef::Instructions),
            "Batch" => json::from_value(inner).map(ExecutableRef::Batch),
            other => Err(json::Error::unknown_field(other)),
        }
    }
}
#[cfg(all(test, feature = "json"))]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::triggers::TRIGGER_ENABLED_METADATA_KEY;
    use core::{num::NonZeroU64, time::Duration};
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        block::BlockHeader,
        events::{
            execute_trigger::ExecuteTriggerEventFilter,
            pipeline::{
                BlockEvent, BlockEventFilter, BlockStatus, PipelineEventBox, PipelineEventFilterBox,
            },
            time::Schedule,
        },
        metadata::Metadata,
        prelude::{
            AccountId, Executable, ExecutionTime, InstructionBox, Level, Log, TimeEvent,
            TimeEventFilter, TimeInterval, TriggerId,
        },
    };
    use iroha_primitives::{const_vec::ConstVec, json::Json};
    fn sample_hash() -> HashOf<IvmBytecode> {
        let bytecode = IvmBytecode::from_compiled(vec![0x01, 0x02, 0x03]);
        HashOf::new(&bytecode)
    }
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("trigger-set JSON fixture key generation should succeed")
    }
    fn sample_authority() -> AccountId {
        AccountId::new(checked_keypair().public_key().clone())
    }
    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }
    #[test]
    fn inspect_by_id_skips_missing_entry() {
        let mut set = Set::default();
        let trigger_id: TriggerId = "missing_trigger".parse().expect("valid trigger id");
        set.ids
            .insert(trigger_id.clone(), TriggeringEventType::Time);
        let view = set.view();
        let found = SetReadOnly::inspect_by_id(&view, &trigger_id, |_| ());
        assert!(found.is_none(), "missing trigger should return None");
    }
    #[test]
    fn inspect_by_id_mut_skips_missing_entry() {
        let set = Set::default();
        let trigger_id: TriggerId = "missing_trigger_mut".parse().expect("valid trigger id");
        let mut block = set.block();
        let mut tx = block.transaction();
        tx.ids.insert(trigger_id.clone(), TriggeringEventType::Time);
        let found = tx.inspect_by_id_mut(&trigger_id, |_| ());
        assert!(found.is_none(), "missing trigger should return None");
    }
    #[test]
    fn active_trigger_id_index_tracks_mutations_and_removal() {
        let set = Set::default();
        let trigger_id: TriggerId = "call_active_index".parse().expect("valid trigger id");
        let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
        let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
        let action = SpecializedAction::new(
            executable,
            Repeats::Exactly(1),
            sample_authority(),
            ExecuteTriggerEventFilter::new(),
        )
        .expect("test trigger action satisfies its authority invariant");
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            assert!(
                tx.add_by_call_trigger(SpecializedTrigger::new(trigger_id.clone(), action))
                    .expect("add trigger")
            );
            tx.apply();
            block.commit();
        }
        let view = set.view();
        let mut active_ids = view.active_trigger_ids_iter().cloned();
        assert_eq!(active_ids.next(), Some(trigger_id.clone()));
        assert_eq!(active_ids.next(), None);
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            tx.inspect_by_id_mut(&trigger_id, |action| {
                action.set_repeats(Repeats::Exactly(0));
            })
            .expect("trigger present");
            assert!(
                tx.active_trigger_ids_iter().all(|id| id != &trigger_id),
                "depleted trigger should be removed from the active-id index"
            );
            tx.apply();
            block.commit();
        }
        assert!(
            set.view()
                .active_trigger_ids_iter()
                .all(|id| id != &trigger_id),
            "committed view should keep depleted trigger out of the active-id index"
        );
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            tx.inspect_by_id_mut(&trigger_id, |action| {
                action.set_repeats(Repeats::Exactly(1));
            })
            .expect("trigger present");
            assert!(
                tx.active_trigger_ids_iter().any(|id| id == &trigger_id),
                "reactivated trigger should be restored to the active-id index"
            );
            assert!(tx.remove(&trigger_id), "trigger should be removed");
            assert!(
                tx.active_trigger_ids_iter().all(|id| id != &trigger_id),
                "removed trigger should be dropped from the active-id index"
            );
            tx.apply();
            block.commit();
        }
    }
    #[test]
    fn executable_ref_json_roundtrip_ivm() {
        let original = ExecutableRef::Ivm(sample_hash());
        let json = norito::json::to_json(&original).expect("serialize ExecutableRef::Ivm");
        let reparsed: ExecutableRef =
            norito::json::from_json(&json).expect("deserialize ExecutableRef::Ivm");
        match reparsed {
            ExecutableRef::Ivm(hash) => assert_eq!(hash, sample_hash()),
            other => panic!("expected Ivm variant, got {other:?}"),
        }
    }
    #[test]
    fn executable_ref_json_roundtrip_instructions() {
        let instruction = InstructionBox::from(Log::new(Level::INFO, "roundtrip".to_owned()));
        let instructions = ConstVec::from(vec![instruction]);
        let original = ExecutableRef::Instructions(instructions.clone());
        let json = norito::json::to_json(&original).expect("serialize ExecutableRef::Instructions");
        let reparsed: ExecutableRef =
            norito::json::from_json(&json).expect("deserialize ExecutableRef::Instructions");
        match reparsed {
            ExecutableRef::Instructions(restored) => assert_eq!(restored, instructions),
            other => panic!("expected Instructions variant, got {other:?}"),
        }
    }
    #[test]
    fn executable_ref_json_roundtrip_mixed_batch() {
        let authority = sample_authority();
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
            &authority,
            7,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("derive contract address");
        let items = ConstVec::from(vec![
            ExecutableBatchItem::Instruction(InstructionBox::from(Log::new(
                Level::INFO,
                "before call".to_owned(),
            ))),
            ExecutableBatchItem::ContractCall(ContractInvocation {
                contract_address,
                expected_code_hash: iroha_crypto::Hash::new(b"trigger-batch"),
                entrypoint: "main".to_owned(),
                arguments: None,
            }),
        ]);
        let original = ExecutableRef::Batch(items.clone());
        let json = norito::json::to_json(&original).expect("serialize ExecutableRef::Batch");
        let reparsed: ExecutableRef =
            norito::json::from_json(&json).expect("deserialize ExecutableRef::Batch");
        match reparsed {
            ExecutableRef::Batch(restored) => assert_eq!(restored, items),
            other => panic!("expected Batch variant, got {other:?}"),
        }
    }
    #[test]
    fn executable_ref_dto_decode_from_slice_roundtrip() {
        let instruction = InstructionBox::from(Log::new(Level::INFO, "dto-roundtrip".to_owned()));
        let dto =
            ExecutableRefDto::Instructions(ConstVec::from(vec![instruction.clone(), instruction]));
        let bytes =
            norito::to_bytes(&dto).expect("serialize ExecutableRefDto::Instructions variant");
        let decoded: ExecutableRefDto = norito::decode_from_bytes(&bytes).expect("decode dto");
        assert_eq!(decoded, dto);
    }
    #[test]
    fn replace_account_id_updates_trigger_authority_and_filter() {
        let set = Set::default();
        let old = sample_authority();
        let new = sample_authority();
        let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
        let executable = Executable::Instructions(ConstVec::from(vec![instruction.clone()]));
        let time_trigger_id: TriggerId = "time_trigger_rekey".parse().expect("valid id");
        let call_trigger_id: TriggerId = "call_trigger_rekey".parse().expect("valid id");
        let owned_data_trigger_id: TriggerId =
            "owned_data_trigger_rekey".parse().expect("valid id");
        let global_data_trigger_id: TriggerId =
            "global_data_trigger_rekey".parse().expect("valid id");
        let time_action = SpecializedAction::new(
            executable.clone(),
            Repeats::Exactly(1),
            old.clone(),
            TimeEventFilter(ExecutionTime::PreCommit),
        )
        .expect("test time-trigger action satisfies its authority invariant");
        let call_action = SpecializedAction::new(
            executable,
            Repeats::Exactly(1),
            old.clone(),
            ExecuteTriggerEventFilter::new(),
        )
        .expect("test by-call action satisfies its authority invariant");
        let mut owned_data_action = SpecializedAction::new(
            Executable::Instructions(ConstVec::new_empty()),
            Repeats::Indefinitely,
            old.clone(),
            DataEventFilter::Account(AccountEventFilter::new().for_account(old.clone())),
        )
        .expect("test owned data-trigger action satisfies its authority invariant");
        owned_data_action.metadata =
            crate::smartcontracts::isi::triggers::owned_data_trigger_scope_metadata_for_testing();
        let mut global_data_action = SpecializedAction::new(
            Executable::Instructions(ConstVec::new_empty()),
            Repeats::Indefinitely,
            old.clone(),
            DataEventFilter::Any,
        )
        .expect("test global data-trigger action satisfies its authority invariant");
        global_data_action.metadata =
            crate::smartcontracts::isi::triggers::global_data_trigger_scope_metadata_for_testing(
                &old,
            );
        let mut block = set.block();
        let mut tx = block.transaction();
        tx.add_time_trigger(SpecializedTrigger::new(
            time_trigger_id.clone(),
            time_action,
        ))
        .expect("add time trigger");
        tx.add_by_call_trigger(SpecializedTrigger::new(
            call_trigger_id.clone(),
            call_action,
        ))
        .expect("add call trigger");
        tx.add_data_trigger(SpecializedTrigger::new(
            owned_data_trigger_id.clone(),
            owned_data_action,
        ))
        .expect("add owned data trigger");
        tx.add_data_trigger(SpecializedTrigger::new(
            global_data_trigger_id.clone(),
            global_data_action,
        ))
        .expect("add global data trigger");
        assert_eq!(
            tx.global_data_trigger_permission_rekeys(&old, &new)
                .expect("canonical provenance"),
            BTreeSet::from([(old.clone(), new.clone())])
        );
        tx.replace_account_id(&old, &new)
            .expect("canonical trigger state rekeys");
        tx.apply();
        block.commit();
        let view = set.view();
        let time_action = view
            .time_triggers()
            .get(&time_trigger_id)
            .expect("time trigger present");
        assert_eq!(time_action.authority, new);
        let call_action = view
            .by_call_triggers()
            .get(&call_trigger_id)
            .expect("call trigger present");
        assert_eq!(call_action.authority, new);
        assert_eq!(
            call_action.filter.authority(),
            Some(&new),
            "by-call filter authority should be updated"
        );
        let owned_data_action = view
            .data_triggers()
            .get(&owned_data_trigger_id)
            .expect("owned data trigger present");
        assert_eq!(owned_data_action.authority, new);
        let DataEventFilter::Account(filter) = &owned_data_action.filter else {
            panic!("owned data trigger remains an account filter")
        };
        assert_eq!(filter.id_matcher().as_ref(), Some(&new));
        let global_data_action = view
            .data_triggers()
            .get(&global_data_trigger_id)
            .expect("global data trigger present");
        assert_eq!(global_data_action.authority, new);
        assert_eq!(
            data_trigger_global_permission_grantee(global_data_action.metadata())
                .expect("canonical global provenance"),
            Some(new)
        );
    }

    #[test]
    fn replace_account_id_rejects_merged_data_trigger_authority_over_cap_without_mutation() {
        let set = Set::default();
        let old = sample_authority();
        let new = sample_authority();
        let action = |authority: AccountId| {
            let mut action = SpecializedAction::new(
                Executable::Instructions(ConstVec::new_empty()),
                Repeats::Indefinitely,
                authority.clone(),
                DataEventFilter::Any,
            )
            .expect("valid data-trigger action");
            action.metadata = crate::smartcontracts::isi::triggers::global_data_trigger_scope_metadata_for_testing(
                &authority,
            );
            action
        };
        let mut block = set.block();
        let mut tx = block.transaction();
        for index in 0..super::super::isi::MAX_DATA_TRIGGERS_PER_AUTHORITY {
            let id = format!("new_authority_{index}")
                .parse()
                .expect("valid trigger id");
            tx.add_data_trigger(SpecializedTrigger::new(id, action(new.clone())))
                .expect("new authority remains within its cap");
        }
        let old_trigger_id: TriggerId = "old_authority_extra".parse().expect("valid trigger id");
        tx.add_data_trigger(SpecializedTrigger::new(
            old_trigger_id.clone(),
            action(old.clone()),
        ))
        .expect("old authority remains within its cap");

        let error = tx
            .global_data_trigger_permission_rekeys(&old, &new)
            .expect_err("multisig preflight must reject an over-cap authority merge");
        assert!(
            matches!(error, Error::DataTriggerAuthorityCapacity(ref account) if account == &new)
        );
        let error = tx
            .replace_account_id(&old, &new)
            .expect_err("direct trigger rekey must reject an over-cap authority merge");
        assert!(
            matches!(error, Error::DataTriggerAuthorityCapacity(ref account) if account == &new)
        );
        assert_eq!(
            tx.data_triggers
                .get(&old_trigger_id)
                .expect("old trigger remains present")
                .authority,
            old,
            "failed preflight must not mutate trigger state"
        );
    }

    #[test]
    fn rekeyed_data_filter_does_not_inherit_a_pre_rekey_event() {
        let set = Set::default();
        let old = sample_authority();
        let new = sample_authority();
        let trigger_id: TriggerId = "rekeyed_filter_watermark"
            .parse()
            .expect("valid trigger id");
        let event = DataEvent::Account(AccountEvent::Deleted(new.clone()));
        let mut action = SpecializedAction::new(
            Executable::Instructions(ConstVec::new_empty()),
            Repeats::Indefinitely,
            old.clone(),
            DataEventFilter::Account(AccountEventFilter::new().for_account(old.clone())),
        )
        .expect("valid owned data-trigger action");
        action.metadata =
            crate::smartcontracts::isi::triggers::owned_data_trigger_scope_metadata_for_testing();
        let mut block = set.block();
        let mut tx = block.transaction();
        tx.add_data_trigger(SpecializedTrigger::new(trigger_id.clone(), action))
            .expect("add owned data trigger");
        let pre_rekey_snapshot = tx.data_trigger_match_snapshot();
        assert!(
            !tx.data_trigger_candidates(&event).contains(&trigger_id),
            "the event must not match the old account selector"
        );

        tx.replace_account_id(&old, &new)
            .expect("canonical trigger state rekeys");
        assert!(
            tx.data_trigger_candidates(&event).contains(&trigger_id),
            "the live index must follow the rekeyed selector"
        );
        assert!(
            tx.data_trigger_matching_generation(pre_rekey_snapshot, &trigger_id, &event)
                .is_none(),
            "a rewritten selector must not inherit an event captured before the rekey"
        );
        let post_rekey_snapshot = tx.data_trigger_match_snapshot();
        assert!(
            tx.data_trigger_matching_generation(post_rekey_snapshot, &trigger_id, &event)
                .is_some(),
            "the rekeyed selector must remain eligible for later events"
        );
    }

    #[test]
    fn data_trigger_watermark_excludes_later_registration_and_reactivation() {
        let set = Set::default();
        let authority = sample_authority();
        let event = DataEvent::Account(AccountEvent::Deleted(authority.clone()));
        let mut block = set.block();
        let mut tx = block.transaction();
        let active_id: TriggerId = "watermark_active".parse().expect("valid id");
        let later_id: TriggerId = "watermark_later".parse().expect("valid id");
        let reactivated_id: TriggerId = "watermark_reactivated".parse().expect("valid id");
        let action = |metadata| {
            let mut action = SpecializedAction::new(
                Executable::Instructions(ConstVec::new_empty()),
                Repeats::Indefinitely,
                authority.clone(),
                DataEventFilter::Any,
            )
            .expect("valid data-trigger action");
            action.metadata = metadata;
            action
        };
        tx.add_data_trigger(SpecializedTrigger::new(
            active_id.clone(),
            action(
                crate::smartcontracts::isi::triggers::global_data_trigger_scope_metadata_for_testing(
                    &authority,
                ),
            ),
        ))
        .expect("add active trigger");
        let mut disabled_metadata =
            crate::smartcontracts::isi::triggers::global_data_trigger_scope_metadata_for_testing(
                &authority,
            );
        disabled_metadata.insert(
            TRIGGER_ENABLED_METADATA_KEY.parse().expect("valid key"),
            Json::from(false),
        );
        tx.add_data_trigger(SpecializedTrigger::new(
            reactivated_id.clone(),
            action(disabled_metadata),
        ))
        .expect("add disabled trigger");
        let snapshot = tx.data_trigger_match_snapshot();
        tx.add_data_trigger(SpecializedTrigger::new(
            later_id.clone(),
            action(
                crate::smartcontracts::isi::triggers::global_data_trigger_scope_metadata_for_testing(
                    &authority,
                ),
            ),
        ))
        .expect("add later trigger");
        tx.inspect_by_id_mut(&reactivated_id, |action| {
            action.metadata_mut().insert(
                TRIGGER_ENABLED_METADATA_KEY.parse().expect("valid key"),
                Json::from(true),
            );
        })
        .expect("reactivate trigger");

        assert!(
            tx.data_trigger_matching_generation(snapshot, &active_id, &event)
                .is_some()
        );
        assert!(
            tx.data_trigger_matching_generation(snapshot, &later_id, &event)
                .is_none(),
            "a later registration must not inherit a captured event"
        );
        assert!(
            tx.data_trigger_matching_generation(snapshot, &reactivated_id, &event)
                .is_none(),
            "an inactive trigger reactivated later must not inherit a captured event"
        );
        let later_snapshot = tx.data_trigger_match_snapshot();
        assert!(
            tx.data_trigger_matching_generation(later_snapshot, &later_id, &event)
                .is_some()
        );
        assert!(
            tx.data_trigger_matching_generation(later_snapshot, &reactivated_id, &event)
                .is_some()
        );
    }
    #[test]
    fn match_time_event_skips_recently_registered_triggers() {
        let set = Set::default();
        {
            let mut block = set.block();
            {
                let mut tx = block.transaction();
                let trigger_id: TriggerId = "time_trigger".parse().expect("valid id");
                let authority = sample_authority();
                let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
                let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
                let mut action = SpecializedAction::new(
                    executable,
                    Repeats::Exactly(1),
                    authority,
                    TimeEventFilter(ExecutionTime::PreCommit),
                )
                .expect("test time-trigger action satisfies its authority invariant");
                let mut metadata = Metadata::default();
                metadata.insert(
                    "__registered_block_height".parse().expect("valid name"),
                    Json::from(42_u64),
                );
                metadata.insert(
                    "__registered_at_ms".parse().expect("valid name"),
                    Json::from(1_234_u64),
                );
                action.metadata = metadata;
                let trigger = SpecializedTrigger::new(trigger_id, action);
                tx.add_time_trigger(trigger)
                    .expect("time trigger should be added");
                tx.apply();
            }
            block.commit();
        }
        let block_view = set.block();
        let interval_current =
            TimeInterval::new_since_to(Duration::from_millis(0), Duration::from_millis(1_234));
        let time_event = TimeEvent {
            interval: interval_current,
        };
        assert!(
            block_view
                .match_time_event(time_event, 42, 1_234, 16)
                .next()
                .is_none(),
            "trigger registered in current block must be skipped"
        );
        let matches_same_time = block_view
            .match_time_event(time_event, 99, 1_234, 16)
            .count();
        assert_eq!(
            matches_same_time, 1,
            "trigger should match when height differs even if timestamp matches"
        );
        let interval_later =
            TimeInterval::new_since_to(Duration::from_millis(1_234), Duration::from_millis(2_000));
        let later_event = TimeEvent {
            interval: interval_later,
        };
        let matches_later = block_view
            .match_time_event(later_event, 99, 2_000, 16)
            .count();
        assert_eq!(
            matches_later, 1,
            "trigger should appear for subsequent blocks"
        );
    }
    #[test]
    fn match_pipeline_event_skips_recently_registered_triggers() {
        let set = Set::default();
        let trigger_id: TriggerId = "pipeline_trigger".parse().expect("valid id");
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
            let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
            let mut action = SpecializedAction::new(
                executable,
                Repeats::Exactly(1),
                sample_authority(),
                PipelineEventFilterBox::from(
                    BlockEventFilter::new().for_status(BlockStatus::Approved),
                ),
            )
            .expect("test pipeline-trigger action satisfies its authority invariant");
            action.metadata.insert(
                "__registered_block_height".parse().expect("valid name"),
                Json::from(42_u64),
            );
            tx.add_pipeline_trigger(SpecializedTrigger::new(trigger_id.clone(), action))
                .expect("pipeline trigger should be added");
            tx.apply();
            block.commit();
        }
        let event = PipelineEventBox::from(BlockEvent {
            header: BlockHeader::new(
                NonZeroU64::new(42).expect("nonzero height"),
                None,
                None,
                None,
                0,
                0,
            ),
            status: BlockStatus::Approved,
        });
        let block_view = set.block();
        assert!(
            block_view.match_pipeline_event(&event, 42).next().is_none(),
            "pipeline trigger registered in the current block must be skipped"
        );
        assert_eq!(
            block_view
                .match_pipeline_event(&event, 43)
                .collect::<Vec<_>>(),
            vec![trigger_id],
            "pipeline trigger should match in a subsequent block"
        );
    }
    #[test]
    fn match_time_event_caps_periodic_materialisation() {
        let set = Set::default();
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            let trigger_id: TriggerId = "bounded_periodic_trigger".parse().expect("valid id");
            let authority = sample_authority();
            let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
            let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
            let mut action = SpecializedAction::new(
                executable,
                Repeats::Indefinitely,
                authority,
                TimeEventFilter(ExecutionTime::Schedule(Schedule {
                    start_ms: 0,
                    period_ms: Some(1),
                })),
            )
            .expect("test scheduled trigger action satisfies its authority invariant");
            action.metadata.insert(
                "__registered_block_height".parse().expect("valid name"),
                Json::from(1_u64),
            );
            tx.add_time_trigger(SpecializedTrigger::new(trigger_id, action))
                .expect("add periodic trigger");
            tx.apply();
            block.commit();
        }
        let event = TimeEvent {
            interval: TimeInterval {
                since_ms: 0,
                length_ms: u64::MAX,
            },
        };
        let matches = set
            .view()
            .match_time_event(event, 2, u64::MAX, 3)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 3);
    }
    #[test]
    fn match_time_event_skips_disabled_triggers() {
        let set = Set::default();
        {
            let mut block = set.block();
            {
                let mut tx = block.transaction();
                let trigger_id: TriggerId = "time_trigger_disabled".parse().expect("valid id");
                let authority = sample_authority();
                let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
                let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
                let mut action = SpecializedAction::new(
                    executable,
                    Repeats::Exactly(1),
                    authority,
                    TimeEventFilter(ExecutionTime::PreCommit),
                )
                .expect("test time-trigger action satisfies its authority invariant");
                let mut metadata = Metadata::default();
                metadata.insert(
                    "__registered_block_height".parse().expect("valid name"),
                    Json::from(42_u64),
                );
                metadata.insert(
                    "__registered_at_ms".parse().expect("valid name"),
                    Json::from(1_234_u64),
                );
                metadata.insert(
                    TRIGGER_ENABLED_METADATA_KEY.parse().expect("valid name"),
                    Json::from(false),
                );
                action.metadata = metadata;
                let trigger = SpecializedTrigger::new(trigger_id, action);
                tx.add_time_trigger(trigger)
                    .expect("time trigger should be added");
                tx.apply();
            }
            block.commit();
        }
        let block_view = set.block();
        let interval =
            TimeInterval::new_since_to(Duration::from_millis(0), Duration::from_millis(1_234));
        let time_event = TimeEvent { interval };
        assert!(
            block_view
                .match_time_event(time_event, 99, 1_234, 16)
                .next()
                .is_none(),
            "disabled trigger must be skipped"
        );
    }
    #[test]
    fn match_time_event_requires_registration_metadata() {
        let set = Set::default();
        {
            let mut block = set.block();
            {
                let mut tx = block.transaction();
                let trigger_id: TriggerId = "time_trigger_missing_meta".parse().expect("valid id");
                let authority = sample_authority();
                let instruction = InstructionBox::from(Log::new(Level::INFO, "noop".to_owned()));
                let executable = Executable::Instructions(ConstVec::from(vec![instruction]));
                let action = SpecializedAction::new(
                    executable,
                    Repeats::Exactly(1),
                    authority,
                    TimeEventFilter(ExecutionTime::PreCommit),
                )
                .expect("test time-trigger action satisfies its authority invariant");
                let trigger = SpecializedTrigger::new(trigger_id, action);
                tx.add_time_trigger(trigger)
                    .expect("time trigger should be added");
                tx.apply();
            }
            block.commit();
        }
        let block_view = set.block();
        let interval =
            TimeInterval::new_since_to(Duration::from_millis(0), Duration::from_millis(1_234));
        let time_event = TimeEvent { interval };
        assert!(
            block_view
                .match_time_event(time_event, 99, 1_234, 16)
                .next()
                .is_none(),
            "trigger missing registration metadata must be skipped"
        );
    }
}
/// [`SetTransaction::mod_repeats()`] error
#[derive(Debug, Clone, thiserror::Error, displaydoc::Display)]
pub enum ModRepeatsError {
    /// Trigger with id = `{0}` not found
    NotFound(TriggerId),
    /// Trigger repeats count overflow error
    RepeatsOverflow(#[from] RepeatsOverflowError),
}
/// Trigger repeats count overflow
#[derive(Debug, Copy, Clone, thiserror::Error, displaydoc::Display)]
pub struct RepeatsOverflowError;
impl From<ModRepeatsError> for InstructionExecutionError {
    fn from(err: ModRepeatsError) -> Self {
        match err {
            ModRepeatsError::NotFound(not_found_id) => FindError::Trigger(not_found_id).into(),
            ModRepeatsError::RepeatsOverflow(_) => MathError::Overflow.into(),
        }
    }
}
// --- Norito DTO for Set (Phase 1 scaffolding) ---
/// Norito-encoded Data Transfer Object for serializing/deserializing the `Set` of triggers and
/// associated entries. Used in scaffolding paths where a compact binary representation is required.
#[derive(Encode, Decode)]
pub struct SetDto {
    data: Vec<(TriggerId, LoadedActionDto<DataEventFilter>)>,
    pipeline: Vec<(TriggerId, LoadedActionDto<PipelineEventFilterBox>)>,
    time: Vec<(TriggerId, LoadedActionDto<TimeEventFilter>)>,
    by_call: Vec<(TriggerId, LoadedActionDto<ExecuteTriggerEventFilter>)>,
    ids: Vec<(TriggerId, TriggeringEventType)>,
    contracts: Vec<(HashOf<IvmBytecode>, IvmBytecodeEntryDto)>,
}
impl SetDto {
    /// Encode this DTO into Norito bytes.
    ///
    /// # Errors
    /// Returns an error if Norito encoding fails.
    pub fn encode(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(self)
    }
    /// Decode a DTO from Norito bytes.
    ///
    /// # Errors
    /// Returns an error if Norito decoding fails.
    pub fn decode(bytes: &[u8]) -> Result<Self, norito::core::Error> {
        norito::decode_from_bytes(bytes)
    }
}
impl From<&Set> for SetDto {
    fn from(set: &Set) -> Self {
        // Use a read-only view to iterate storages
        let view = SetView {
            data_triggers: set.data_triggers.view(),
            pipeline_triggers: set.pipeline_triggers.view(),
            time_triggers: set.time_triggers.view(),
            by_call_triggers: set.by_call_triggers.view(),
            ids: set.ids.view(),
            active_data_trigger_ids: set.active_data_trigger_ids.view(),
            active_pipeline_trigger_ids: set.active_pipeline_trigger_ids.view(),
            active_time_trigger_ids: set.active_time_trigger_ids.view(),
            active_by_call_trigger_ids: set.active_by_call_trigger_ids.view(),
            contracts: set.contracts.view(),
        };
        let data: Vec<(TriggerId, LoadedActionDto<DataEventFilter>)> = view
            .data_triggers
            .iter()
            .map(|(k, v)| (k.clone(), LoadedActionDto::from(v)))
            .collect();
        let pipeline: Vec<(TriggerId, LoadedActionDto<PipelineEventFilterBox>)> = view
            .pipeline_triggers
            .iter()
            .map(|(k, v)| (k.clone(), LoadedActionDto::from(v)))
            .collect();
        let time: Vec<(TriggerId, LoadedActionDto<TimeEventFilter>)> = view
            .time_triggers
            .iter()
            .map(|(k, v)| (k.clone(), LoadedActionDto::from(v)))
            .collect();
        let by_call: Vec<(TriggerId, LoadedActionDto<ExecuteTriggerEventFilter>)> = view
            .by_call_triggers
            .iter()
            .map(|(k, v)| (k.clone(), LoadedActionDto::from(v)))
            .collect();
        let ids = view.ids.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let contracts: Vec<(HashOf<IvmBytecode>, IvmBytecodeEntryDto)> = view
            .contracts
            .iter()
            .map(|(k, v)| (*k, IvmBytecodeEntryDto::from(v)))
            .collect();
        SetDto {
            data,
            pipeline,
            time,
            by_call,
            ids,
            contracts,
        }
    }
}
fn load_trigger_entries<F>(
    raw: Vec<(TriggerId, LoadedActionDto<F>)>,
    event_type: TriggeringEventType,
    contracts: &BTreeMap<HashOf<IvmBytecode>, IvmBytecodeEntry>,
    ids: &mut BTreeMap<TriggerId, TriggeringEventType>,
    duplicate_ids: &mut Vec<TriggerId>,
    missing_contracts: &mut Vec<TriggerId>,
) -> Result<Vec<(TriggerId, LoadedAction<F>)>, String> {
    let mut entries = Vec::with_capacity(raw.len());
    for (id, dto) in raw {
        if ids.contains_key(&id) {
            duplicate_ids.push(id);
            continue;
        }
        let action = LoadedAction::try_from(dto)?;
        if let Some(blob_hash) = action.extract_blob_hash() {
            if !contracts.contains_key(&blob_hash) {
                missing_contracts.push(id);
                continue;
            }
        }
        ids.insert(id.clone(), event_type);
        entries.push((id, action));
    }
    Ok(entries)
}
impl TryFrom<SetDto> for Set {
    type Error = String;
    #[allow(clippy::too_many_lines)]
    fn try_from(dto: SetDto) -> Result<Self, Self::Error> {
        let SetDto {
            data,
            pipeline,
            time,
            by_call,
            ids: ids_raw,
            contracts,
        } = dto;
        let mut contracts_map = BTreeMap::new();
        let mut duplicate_contracts = 0usize;
        for (hash, entry) in contracts {
            let entry = IvmBytecodeEntry::try_from(entry)?;
            if HashOf::new(&entry.original_contract) != hash {
                return Err(
                    "trigger contract lookup hash does not match its original bytecode".to_owned(),
                );
            }
            if contracts_map.insert(hash, entry).is_some() {
                duplicate_contracts = duplicate_contracts.saturating_add(1);
            }
        }
        let mut ids = BTreeMap::new();
        let mut duplicate_ids = Vec::new();
        let mut missing_contracts = Vec::new();
        let data = load_trigger_entries(
            data,
            TriggeringEventType::Data,
            &contracts_map,
            &mut ids,
            &mut duplicate_ids,
            &mut missing_contracts,
        )?;
        if let Some((id, _)) = data
            .iter()
            .find(|(_, action)| !data_trigger_scope_authorization_is_well_formed(action.metadata()))
        {
            return Err(format!(
                "incompatible data trigger `{id}`: missing or malformed v1 scope authorization metadata; regenerate the first-release snapshot"
            ));
        }
        validate_data_trigger_capacities(data.iter().map(|(id, action)| (id, action))).map_err(
            |message| {
                format!(
                    "incompatible first-release data-trigger snapshot: {message}; regenerate the snapshot"
                )
            },
        )?;
        let pipeline = load_trigger_entries(
            pipeline,
            TriggeringEventType::Pipeline,
            &contracts_map,
            &mut ids,
            &mut duplicate_ids,
            &mut missing_contracts,
        )?;
        let time = load_trigger_entries(
            time,
            TriggeringEventType::Time,
            &contracts_map,
            &mut ids,
            &mut duplicate_ids,
            &mut missing_contracts,
        )?;
        let by_call = load_trigger_entries(
            by_call,
            TriggeringEventType::ExecuteTrigger,
            &contracts_map,
            &mut ids,
            &mut duplicate_ids,
            &mut missing_contracts,
        )?;
        let mut orphaned_ids = 0usize;
        let mut mismatched_ids = 0usize;
        for (id, event_type) in ids_raw {
            match ids.get(&id) {
                Some(actual) if actual == &event_type => {}
                Some(_) => mismatched_ids = mismatched_ids.saturating_add(1),
                None => orphaned_ids = orphaned_ids.saturating_add(1),
            }
        }
        if !duplicate_ids.is_empty() {
            warn!(
                count = duplicate_ids.len(),
                "dropping duplicate trigger ids while repairing trigger storage"
            );
        }
        if !missing_contracts.is_empty() {
            warn!(
                count = missing_contracts.len(),
                "dropping triggers referencing missing IVM bytecode"
            );
        }
        if orphaned_ids > 0 || mismatched_ids > 0 {
            warn!(
                orphaned_ids,
                mismatched_ids, "trigger id registry out of sync; rebuilding from typed triggers"
            );
        }
        if duplicate_contracts > 0 {
            warn!(
                count = duplicate_contracts,
                "duplicate trigger bytecode entries found; keeping latest"
            );
        }
        let mut contract_counts: BTreeMap<HashOf<IvmBytecode>, u64> = BTreeMap::new();
        for (_, action) in &data {
            if let Some(blob_hash) = action.extract_blob_hash() {
                let count = contract_counts.entry(blob_hash).or_insert(0);
                *count = count.saturating_add(1);
            }
        }
        for (_, action) in &pipeline {
            if let Some(blob_hash) = action.extract_blob_hash() {
                let count = contract_counts.entry(blob_hash).or_insert(0);
                *count = count.saturating_add(1);
            }
        }
        for (_, action) in &time {
            if let Some(blob_hash) = action.extract_blob_hash() {
                let count = contract_counts.entry(blob_hash).or_insert(0);
                *count = count.saturating_add(1);
            }
        }
        for (_, action) in &by_call {
            if let Some(blob_hash) = action.extract_blob_hash() {
                let count = contract_counts.entry(blob_hash).or_insert(0);
                *count = count.saturating_add(1);
            }
        }
        let mut repaired_contracts = BTreeMap::new();
        let mut dropped_contracts = 0usize;
        let mut fixed_counts = 0usize;
        for (hash, mut entry) in contracts_map {
            let Some(count) = contract_counts.get(&hash) else {
                dropped_contracts = dropped_contracts.saturating_add(1);
                continue;
            };
            let Some(new_count) = NonZeroU64::new(*count) else {
                warn!(
                    ?hash,
                    count, "invalid trigger bytecode reference count; dropping entry"
                );
                dropped_contracts = dropped_contracts.saturating_add(1);
                continue;
            };
            if entry.count.get() != new_count.get() {
                fixed_counts = fixed_counts.saturating_add(1);
                entry.count = new_count;
            }
            repaired_contracts.insert(hash, entry);
        }
        if dropped_contracts > 0 {
            warn!(
                count = dropped_contracts,
                "dropping unused trigger bytecode entries"
            );
        }
        if fixed_counts > 0 {
            warn!(
                count = fixed_counts,
                "repairing trigger bytecode reference counts"
            );
        }
        let set = Set::default();
        // Use a block + transaction to mutate storages safely
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            for (k, v) in data {
                if Set::action_is_active(&v) {
                    tx.active_data_trigger_ids.insert(k.clone(), ());
                }
                tx.data_triggers.insert(k, v);
            }
            for (k, v) in pipeline {
                if Set::action_is_active(&v) {
                    tx.active_pipeline_trigger_ids.insert(k.clone(), ());
                }
                tx.pipeline_triggers.insert(k, v);
            }
            for (k, v) in time {
                if Set::action_is_active(&v) {
                    tx.active_time_trigger_ids.insert(k.clone(), ());
                }
                tx.time_triggers.insert(k, v);
            }
            for (k, v) in by_call {
                if Set::action_is_active(&v) {
                    tx.active_by_call_trigger_ids.insert(k.clone(), ());
                }
                tx.by_call_triggers.insert(k, v);
            }
            for (k, v) in ids {
                tx.ids.insert(k, v);
            }
            for (k, v) in repaired_contracts {
                tx.contracts.insert(k, v);
            }
            tx.apply();
            block.commit();
        }
        Ok(set)
    }
}
#[cfg(test)]
mod dto_tests {
    use super::*;
    use crate::smartcontracts::isi::triggers::global_data_trigger_scope_metadata_for_testing;
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{
        events::pipeline,
        events::time::Schedule,
        prelude as dm,
        prelude::{BlockStatus, ExecutionTime, IvmBytecode, Log, SetKeyValue},
    };
    use iroha_primitives::const_vec::ConstVec;
    use mv::storage::StorageReadOnly;
    use norito::json;
    use std::collections::BTreeMap;
    use std::num::NonZeroU64;
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("trigger-set DTO fixture key generation should succeed")
    }
    fn checked_account_id() -> dm::AccountId {
        dm::AccountId::new(checked_keypair().public_key().clone())
    }
    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }
    fn sample_set() -> Set {
        let authority = checked_account_id();
        let set = Set::default();
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            // Data trigger with Instruction executable
            let data_id: dm::TriggerId = "data1".parse().unwrap();
            let data_filter = dm::DataEventFilter::Any;
            let instr =
                dm::InstructionBox::from(dm::Log::new(dm::Level::INFO, "hello".to_string()));
            let exec = dm::Executable::Instructions(ConstVec::from(vec![instr]));
            let mut action = SpecializedAction::new(
                exec,
                dm::Repeats::Exactly(1),
                authority.clone(),
                data_filter,
            )
            .expect("test data-trigger action satisfies its authority invariant");
            action.metadata = global_data_trigger_scope_metadata_for_testing(&authority);
            let trig = SpecializedTrigger::new(data_id, action);
            tx.add_data_trigger(trig).expect("add data trigger");
            // Pipeline trigger with BlockEventFilter variant
            let pipe_id: dm::TriggerId = "pipe1".parse().unwrap();
            let block_filter = pipeline::BlockEventFilter {
                height: Some(NonZeroU64::new(5).unwrap()),
                status: Some(BlockStatus::Committed),
            };
            let pipe_filter: dm::PipelineEventFilterBox = block_filter.into();
            let key: dm::Name = "k1".parse().unwrap();
            let val = dm::Json::new("v1");
            let set_kv = SetKeyValue::account(authority.clone(), key, val);
            let log = Log::new(dm::Level::INFO, "pipeline".to_string());
            let exec2 = dm::Executable::Instructions(ConstVec::from(vec![
                dm::InstructionBox::from(log),
                dm::InstructionBox::from(set_kv),
            ]));
            let action2 = SpecializedAction::new(
                exec2,
                dm::Repeats::Exactly(3),
                authority.clone(),
                pipe_filter,
            )
            .expect("test pipeline-trigger action satisfies its authority invariant");
            let trig2 = SpecializedTrigger::new(pipe_id, action2);
            tx.add_pipeline_trigger(trig2)
                .expect("add pipeline trigger");
            // Scheduled time trigger with retry policy and empty executable
            let time_id: dm::TriggerId = "time1".parse().unwrap();
            let time_filter = dm::TimeEventFilter(ExecutionTime::Schedule(Schedule::starting_at(
                std::time::Duration::from_secs(1),
            )));
            let exec3 =
                dm::Executable::Instructions(ConstVec::from(Vec::<dm::InstructionBox>::new()));
            let mut action3 = SpecializedAction::new(
                exec3,
                dm::Repeats::Exactly(1),
                authority.clone(),
                time_filter,
            )
            .expect("test scheduled trigger action satisfies its authority invariant");
            action3.retry_policy = Some(dm::TimeTriggerRetryPolicy {
                max_retries: std::num::NonZeroU32::new(2).expect("nonzero"),
                retry_after_ms: std::num::NonZeroU64::new(750).expect("nonzero"),
            });
            let trig3 = SpecializedTrigger::new(time_id, action3);
            tx.add_time_trigger(trig3).expect("add time trigger");
            // Execute-by-call trigger with IVM executable
            let call_id: dm::TriggerId = "call1".parse().unwrap();
            let call_filter = dm::ExecuteTriggerEventFilter::new();
            let ivm_code = IvmBytecode::from_compiled(vec![0xAA, 0xBB]);
            let exec4 = dm::Executable::Ivm(ivm_code);
            let action4 =
                SpecializedAction::new(exec4, dm::Repeats::Exactly(1), authority, call_filter)
                    .expect("test by-call action satisfies its authority invariant");
            let trig4 = SpecializedTrigger::new(call_id, action4);
            tx.add_by_call_trigger(trig4).expect("add by-call trigger");
            tx.apply();
            block.commit();
        }
        set
    }
    fn active_trigger_ids(set: &Set) -> Vec<dm::TriggerId> {
        let mut ids: Vec<_> = set.view().active_trigger_ids_iter().cloned().collect();
        ids.sort();
        ids
    }
    fn assert_loaded_action_dto_equivalent<F>(left: &LoadedActionDto<F>, right: &LoadedActionDto<F>)
    where
        F: PartialEq + core::fmt::Debug,
    {
        assert_eq!(left.executable, right.executable);
        assert_eq!(left.repeats, right.repeats);
        assert_eq!(left.authority.subject_id(), right.authority.subject_id());
        assert_eq!(left.filter, right.filter);
        assert_eq!(left.retry_policy, right.retry_policy);
        assert_eq!(left.retry_state, right.retry_state);
        assert_eq!(left.metadata, right.metadata);
    }
    fn assert_trigger_entries_equivalent<F>(
        left: &[(TriggerId, LoadedActionDto<F>)],
        right: &[(TriggerId, LoadedActionDto<F>)],
    ) where
        F: Clone + PartialEq + core::fmt::Debug,
    {
        let left_map: BTreeMap<_, _> = left.iter().cloned().collect();
        let right_map: BTreeMap<_, _> = right.iter().cloned().collect();
        assert_eq!(left_map.len(), right_map.len());
        for (id, left_action) in &left_map {
            let right_action = right_map
                .get(id)
                .expect("decoded trigger set must preserve trigger ids");
            assert_loaded_action_dto_equivalent(left_action, right_action);
        }
    }
    fn assert_contract_entries_equivalent(
        left: &[(HashOf<IvmBytecode>, IvmBytecodeEntryDto)],
        right: &[(HashOf<IvmBytecode>, IvmBytecodeEntryDto)],
    ) {
        assert_eq!(left.len(), right.len());
        for (hash, left_entry) in left {
            let right_entry = right
                .iter()
                .find_map(|(right_hash, right_entry)| (right_hash == hash).then_some(right_entry))
                .expect("decoded trigger set must preserve contract hashes");
            assert_eq!(left_entry.original_contract, right_entry.original_contract);
            assert_eq!(left_entry.code_hash, right_entry.code_hash);
            assert_eq!(left_entry.count, right_entry.count);
        }
    }
    #[test]
    fn empty_set_roundtrip_dto() {
        let set = Set::default();
        let dto = SetDto::from(&set);
        let bytes = dto.encode().expect("encode dto");
        let dto2 = SetDto::decode(&bytes).expect("decode dto");
        let set2 = Set::try_from(dto2).expect("dto to set");
        let dto3 = SetDto::from(&set2);
        assert_eq!(dto3.data.len(), 0);
        assert_eq!(dto3.pipeline.len(), 0);
        assert_eq!(dto3.time.len(), 0);
        assert_eq!(dto3.by_call.len(), 0);
        assert_eq!(dto3.ids.len(), 0);
        assert_eq!(dto3.contracts.len(), 0);
    }
    #[test]
    fn non_empty_set_roundtrip_dto() {
        let set = sample_set();
        let dto = SetDto::from(&set);
        assert_eq!(dto.data.len(), 1);
        assert_eq!(dto.pipeline.len(), 1);
        assert_eq!(dto.time.len(), 1);
        assert_eq!(dto.by_call.len(), 1);
        assert_eq!(dto.ids.len(), 4);
        assert_eq!(dto.contracts.len(), 1);
        let bytes = dto.encode().expect("encode dto");
        let dto2 = SetDto::decode(&bytes).expect("decode dto");
        // Reconstruct Set (full reconstruction path)
        let set2 = Set::try_from(dto2).expect("dto to set");
        let dto3 = SetDto::from(&set2);
        assert_eq!(dto3.data.len(), 1);
        assert_eq!(dto3.pipeline.len(), 1);
        assert_eq!(dto3.time.len(), 1);
        assert_eq!(dto3.by_call.len(), 1);
        assert_eq!(dto3.ids.len(), 4);
        assert_eq!(dto3.contracts.len(), 1);
    }
    #[test]
    fn data_trigger_snapshot_without_v1_scope_authorization_fails_fast() {
        let mut dto = SetDto::from(&sample_set());
        dto.data
            .first_mut()
            .expect("sample set has one data trigger")
            .1
            .metadata = dm::Metadata::default();
        let error = match Set::try_from(dto) {
            Ok(_) => panic!("legacy data-trigger snapshot must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.contains("missing or malformed v1 scope authorization metadata")
                && error.contains("regenerate the first-release snapshot"),
            "unexpected compatibility error: {error}"
        );
    }
    #[test]
    fn data_trigger_snapshot_with_legacy_genesis_global_scope_fails_fast() {
        let mut dto = SetDto::from(&sample_set());
        let legacy_metadata =
            crate::smartcontracts::isi::triggers::legacy_genesis_global_scope_metadata_for_testing(
            );
        dto.data
            .first_mut()
            .expect("sample set has one data trigger")
            .1
            .metadata = legacy_metadata;
        let error = match Set::try_from(dto) {
            Ok(_) => panic!("legacy genesis-global trigger snapshot must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.contains("missing or malformed v1 scope authorization metadata")
                && error.contains("regenerate the first-release snapshot"),
            "unexpected legacy genesis-global compatibility error: {error}"
        );
    }
    #[test]
    fn data_trigger_snapshot_above_authority_cap_fails_fast() {
        let mut dto = SetDto::from(&sample_set());
        let action = dto
            .data
            .first()
            .expect("sample set has one data trigger")
            .1
            .clone();
        dto.data = (0..=super::super::isi::MAX_DATA_TRIGGERS_PER_AUTHORITY)
            .map(|index| {
                (
                    format!("over_capacity_{index}")
                        .parse()
                        .expect("valid trigger id"),
                    action.clone(),
                )
            })
            .collect();
        let error = match Set::try_from(dto) {
            Ok(_) => panic!("over-capacity data-trigger snapshot must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.contains("exceeds first-release maximum 64")
                && error.contains("regenerate the snapshot"),
            "unexpected compatibility error: {error}"
        );
    }
    #[test]
    fn set_roundtrips_rebuild_active_trigger_ids() {
        let authority = checked_account_id();
        let active_id: dm::TriggerId = "active_roundtrip".parse().unwrap();
        let depleted_id: dm::TriggerId = "depleted_roundtrip".parse().unwrap();
        let set = Set::default();
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            let empty_exec =
                dm::Executable::Instructions(ConstVec::from(Vec::<dm::InstructionBox>::new()));
            let active_action = SpecializedAction::new(
                empty_exec.clone(),
                dm::Repeats::Exactly(1),
                authority.clone(),
                dm::ExecuteTriggerEventFilter::new(),
            )
            .expect("test by-call action satisfies its authority invariant");
            let depleted_action = SpecializedAction::new(
                empty_exec,
                dm::Repeats::Exactly(0),
                authority,
                dm::TimeEventFilter(dm::ExecutionTime::PreCommit),
            )
            .expect("test time-trigger action satisfies its authority invariant");
            tx.add_by_call_trigger(SpecializedTrigger::new(active_id.clone(), active_action))
                .expect("add active by-call trigger");
            tx.add_time_trigger(SpecializedTrigger::new(
                depleted_id.clone(),
                depleted_action,
            ))
            .expect("add depleted time trigger");
            tx.apply();
            block.commit();
        }
        assert_eq!(active_trigger_ids(&set), vec![active_id.clone()]);
        let dto_bytes = SetDto::from(&set).encode().expect("encode set dto");
        let dto_restored = Set::try_from(SetDto::decode(&dto_bytes).expect("decode set dto"))
            .expect("restore dto");
        assert_eq!(active_trigger_ids(&dto_restored), vec![active_id.clone()]);
        let json_repr = json::to_json(&set).expect("serialize set json");
        let json_restored: Set = json::from_json(&json_repr).expect("restore set json");
        assert_eq!(active_trigger_ids(&json_restored), vec![active_id]);
    }
    #[test]
    fn set_dto_repairs_inconsistent_storage() {
        let authority = checked_account_id();
        let missing_code = IvmBytecode::from_compiled(vec![0x01]);
        let missing_hash = HashOf::new(&missing_code);
        let valid_code = IvmBytecode::from_compiled(vec![0xAA]);
        let valid_hash = HashOf::new(&valid_code);
        let valid_code_hash = ivm::contract_code_hash(valid_code.as_ref());
        let extra_code = IvmBytecode::from_compiled(vec![0xBB]);
        let extra_hash = HashOf::new(&extra_code);
        let extra_code_hash = ivm::contract_code_hash(extra_code.as_ref());
        let data_id: dm::TriggerId = "data_missing".parse().unwrap();
        let call_id: dm::TriggerId = "call_valid".parse().unwrap();
        let orphan_id: dm::TriggerId = "orphan".parse().unwrap();
        let data_action = LoadedActionDto {
            executable: ExecutableRefDto::Ivm(missing_hash),
            repeats: dm::Repeats::Exactly(1),
            authority: authority.clone(),
            filter: dm::DataEventFilter::Any,
            retry_policy: None,
            retry_state: None,
            metadata: dm::Metadata::default(),
        };
        let call_action = LoadedActionDto {
            executable: ExecutableRefDto::Ivm(valid_hash),
            repeats: dm::Repeats::Exactly(1),
            authority,
            filter: dm::ExecuteTriggerEventFilter::new(),
            retry_policy: None,
            retry_state: None,
            metadata: dm::Metadata::default(),
        };
        let dto = SetDto {
            data: vec![(data_id.clone(), data_action)],
            pipeline: Vec::new(),
            time: Vec::new(),
            by_call: vec![(call_id.clone(), call_action)],
            ids: vec![
                (data_id.clone(), TriggeringEventType::Pipeline),
                (call_id.clone(), TriggeringEventType::ExecuteTrigger),
                (orphan_id.clone(), TriggeringEventType::Data),
            ],
            contracts: vec![
                (
                    valid_hash,
                    IvmBytecodeEntryDto {
                        original_contract: valid_code,
                        code_hash: valid_code_hash,
                        count: 9,
                    },
                ),
                (
                    extra_hash,
                    IvmBytecodeEntryDto {
                        original_contract: extra_code,
                        code_hash: extra_code_hash,
                        count: 1,
                    },
                ),
            ],
        };
        let set = Set::try_from(dto).expect("dto to set");
        let view = set.view();
        assert!(view.data_triggers().get(&data_id).is_none());
        assert!(view.by_call_triggers().get(&call_id).is_some());
        assert!(view.ids().get(&data_id).is_none());
        assert_eq!(
            view.ids().get(&call_id),
            Some(&TriggeringEventType::ExecuteTrigger)
        );
        assert!(view.ids().get(&orphan_id).is_none());
        let entry = view
            .contracts()
            .get(&valid_hash)
            .expect("valid contract should remain");
        assert_eq!(entry.count.get(), 1);
        assert!(view.contracts().get(&extra_hash).is_none());
    }
    #[test]
    fn set_json_roundtrip_matches_dto() {
        let set = sample_set();
        let json_repr = json::to_json(&set).expect("serialize set to json");
        let decoded: Set = json::from_json(&json_repr).expect("deserialize set from json");
        let original = SetDto::from(&set);
        let decoded_dto = SetDto::from(&decoded);
        assert_trigger_entries_equivalent(&original.data, &decoded_dto.data);
        assert_trigger_entries_equivalent(&original.pipeline, &decoded_dto.pipeline);
        assert_trigger_entries_equivalent(&original.time, &decoded_dto.time);
        assert_trigger_entries_equivalent(&original.by_call, &decoded_dto.by_call);
        assert_eq!(original.ids, decoded_dto.ids);
        assert_contract_entries_equivalent(&original.contracts, &decoded_dto.contracts);
    }
    #[test]
    fn time_trigger_retry_state_roundtrip_dto() {
        let authority = checked_account_id();
        let trigger_id: dm::TriggerId = "retry_time".parse().unwrap();
        let retry_policy = dm::TimeTriggerRetryPolicy {
            max_retries: std::num::NonZeroU32::new(3).expect("nonzero"),
            retry_after_ms: std::num::NonZeroU64::new(500).expect("nonzero"),
        };
        let set = Set::default();
        {
            let mut block = set.block();
            let mut tx = block.transaction();
            let mut action = SpecializedAction::new(
                dm::Executable::Instructions(ConstVec::from(Vec::<dm::InstructionBox>::new())),
                dm::Repeats::Exactly(1),
                authority,
                dm::TimeEventFilter(dm::ExecutionTime::Schedule(Schedule::starting_at(
                    std::time::Duration::from_millis(5),
                ))),
            )
            .expect("test scheduled trigger action satisfies its authority invariant");
            action.retry_policy = Some(retry_policy);
            tx.add_time_trigger(SpecializedTrigger::new(trigger_id.clone(), action))
                .expect("add time trigger");
            assert!(tx.set_time_trigger_retry_state(
                &trigger_id,
                Some(TimeTriggerRetryState {
                    retries_used: 1,
                    next_retry_at_ms: 42,
                }),
            ));
            tx.apply();
            block.commit();
        }
        let dto = SetDto::from(&set);
        let bytes = dto.encode().expect("encode dto");
        let decoded = SetDto::decode(&bytes).expect("decode dto");
        let restored = Set::try_from(decoded).expect("restore set");
        let restored_dto = SetDto::from(&restored);
        assert_trigger_entries_equivalent(&dto.time, &restored_dto.time);
        let (_, restored_action) = restored_dto
            .time
            .iter()
            .find(|(id, _)| id == &trigger_id)
            .expect("time trigger should survive roundtrip");
        assert_eq!(restored_action.retry_policy, Some(retry_policy));
        assert_eq!(
            restored_action.retry_state,
            Some(TimeTriggerRetryState {
                retries_used: 1,
                next_retry_at_ms: 42,
            })
        );
    }
    #[test]
    fn ivm_entry_rejects_zero_count() {
        let contract = IvmBytecode::from_compiled(vec![1, 2, 3, 4]);
        let encoded_contract = json::to_json(&contract).expect("encode contract");
        let candidate = format!("{{\"original_contract\":{encoded_contract},\"count\":0}}");
        let err = json::from_json::<IvmBytecodeEntry>(&candidate)
            .expect_err("zero count must produce error");
        match err {
            json::Error::InvalidField { field, .. } => assert_eq!(field, "count"),
            other => panic!("unexpected error {other}"),
        }
    }
    #[test]
    fn ivm_entry_rejects_mismatched_deployable_hash() {
        let contract = IvmBytecode::from_compiled(vec![1, 2, 3, 4]);
        let entry = IvmBytecodeEntryDto {
            original_contract: contract,
            code_hash: Hash::new(b"forged-trigger-contract"),
            count: 1,
        };
        let error = IvmBytecodeEntry::try_from(entry)
            .expect_err("trigger deployable hash must be authenticated by its bytes");
        assert!(error.contains("code hash"), "unexpected error: {error}");
    }
}
