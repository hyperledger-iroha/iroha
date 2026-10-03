//! Bounded, rollback-local candidate capture of actual typed quantity effects.
//!
//! This journal is diagnostic preparation for the complete ordinary relation. It
//! cannot export proof/source authority: raw storage mutation coverage and every
//! retained mandatory supply owner must be closed before that boundary is added.
//! Capture failures poison only this candidate, preserving current business
//! execution and existing replay publication. Facts and poison apply together.

use super::fastpq_quantity_archive::{
    QuantityArchiveMap, QuantityBalanceInput, QuantityKindInput, QuantitySupplyInput, QuantityTape,
    QuantityTransferInput,
};
use super::fastpq_quantity_write_plan::{QuantityWriteKey, QuantityWritePlan};
mod source_census;
use super::*;
use iroha_allocation::ChargedBuffer;
#[cfg(test)]
use iroha_data_model::fastpq::{
    FastpqExecutionAssetV1, FastpqExecutionBalanceV1, FastpqExecutionEffectKindV1,
    FastpqExecutionEffectV1, FastpqExecutionSupplyChangeV1, FastpqExecutionTransferV1,
};
use iroha_data_model::fastpq::{
    FastpqExecutionEffectContextV1, FastpqExecutionEffectsV1, FastpqSourceExecutionEntryV1,
};
use source_census::QuantitySourceCensusState;

/// A finite diagnostic; no attacker-controlled message or unbounded evidence is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuantityCaptureIssue {
    /// No exact retained invocation/purpose owner authorized capture.
    UnsupportedOwner,
    /// A quantity helper changed state outside a supported typed operation scope.
    UnownedMutation,
    /// Definition lifecycle was absent or noncanonical at the actual mutation boundary.
    MissingIncarnation,
    /// Full exact source, identity, scope or checked quantity facts were inconsistent.
    InvalidFacts,
    /// Configured finite candidate capture count/byte bounds were exceeded.
    Capacity,
    /// A callback did not return through its original capture scope.
    InterruptedScope,
    /// Raw storage owners and mandatory supply admission are not completely integrated.
    #[cfg(test)]
    IncompleteCoverage,
}

/// Transaction-owned mutation observation, discarded with its original World overlay.
#[derive(Debug, Default)]
pub(crate) struct QuantityMutationObservation {
    owned: bool,
    unowned: bool,
    plan: Option<QuantityWritePlan<QuantityWriteKey, Quantity>>,
}
impl QuantityMutationObservation {
    /// Mark an actual mutation after fallible prechecks and before the first write.
    pub(crate) fn changed(&mut self) {
        if !self.owned {
            self.unowned = true;
        }
    }
}

/// Exact logical-entry frame accounting, retained with its original rollback owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct QuantityCandidateUsage {
    entries: u64,
    deltas: u64,
    input_bytes: u64,
    statement_bytes: u64,
}
impl QuantityCandidateUsage {
    fn checked_add(self, rhs: Self) -> Option<Self> {
        Some(Self {
            entries: self.entries.checked_add(rhs.entries)?,
            deltas: self.deltas.checked_add(rhs.deltas)?,
            input_bytes: self.input_bytes.checked_add(rhs.input_bytes)?,
            statement_bytes: self.statement_bytes.checked_add(rhs.statement_bytes)?,
        })
    }
    fn checked_sub(self, rhs: Self) -> Option<Self> {
        Some(Self {
            entries: self.entries.checked_sub(rhs.entries)?,
            deltas: self.deltas.checked_sub(rhs.deltas)?,
            input_bytes: self.input_bytes.checked_sub(rhs.input_bytes)?,
            statement_bytes: self.statement_bytes.checked_sub(rhs.statement_bytes)?,
        })
    }
}

/// Pending entries retain the original applied length and their full logical replacement.
/// Applied entries have a zero baseline and retain their complete measured length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct QuantityEntryMeasurement {
    baseline: QuantityCandidateUsage,
    full: QuantityCandidateUsage,
}

/// One immutable full tape and its exact logical accounting share original custody.
#[derive(Debug)]
struct QuantityArchivedEntry {
    tape: QuantityTape,
    measurement: QuantityEntryMeasurement,
}
impl std::ops::Deref for QuantityArchivedEntry {
    type Target = FastpqExecutionEffectsV1;
    fn deref(&self) -> &Self::Target {
        self.tape.wire()
    }
}
impl QuantityArchivedEntry {
    #[cfg(test)]
    fn wire(&self) -> &FastpqExecutionEffectsV1 {
        self.tape.wire()
    }
}

/// Whole candidate facts, never an authenticated source manifest.
#[derive(Default)]
pub(crate) struct QuantityCandidateArchive {
    entries: QuantityArchiveMap<QuantityArchivedEntry>,
    /// Exact pre-admitted map backing for the parent's eventual complete replacement.
    parent_backing: Option<ChargedBuffer<(Hash, QuantityArchivedEntry)>>,
    /// Applied full usage, or pending incremental usage beyond `base_usage`.
    usage: QuantityCandidateUsage,
    /// Original applied aggregate against which this transaction prepared its deltas.
    base_usage: Option<QuantityCandidateUsage>,
    issue: Option<QuantityCaptureIssue>,
    applied_world_transactions: u64,
    source_census: QuantitySourceCensusState,
}
impl std::fmt::Debug for QuantityCandidateArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuantityCandidateArchive")
            .field("entries", &self.entries)
            .field("usage", &self.usage)
            .field("issue", &self.issue)
            .field(
                "applied_world_transactions",
                &self.applied_world_transactions,
            )
            .finish()
    }
}
impl QuantityCandidateArchive {
    fn poison(&mut self, issue: QuantityCaptureIssue) {
        // Retire the exact census and its original credit on every later refusal.
        self.source_census = QuantitySourceCensusState::Failed;
        if self.issue.is_none() {
            self.issue = Some(issue);
        }
    }
    /// Candidate export is explicitly refused until the remaining mutation owners join.
    /// TODO: replace this refusal only with complete, independently owned coverage and quotas.
    #[cfg(test)]
    pub(crate) fn require_complete(&self) -> Result<(), QuantityCaptureIssue> {
        Err(self
            .issue
            .unwrap_or(QuantityCaptureIssue::IncompleteCoverage))
    }
    pub(super) fn observe(&mut self, world: &WorldTransaction<'_, '_>) {
        let observation = &world.quantity_mutation_observation;
        if observation.unowned
            || world.assets.has_raw_write()
            || world.asset_definitions.has_raw_write()
        {
            self.poison(QuantityCaptureIssue::UnownedMutation);
        }
        if observation.owned || observation.plan.is_some() {
            self.poison(QuantityCaptureIssue::InterruptedScope);
        }
    }
    pub(super) fn apply(&mut self, mut pending: Self) {
        if self.source_census.is_preparing_or_sealed()
            || pending.source_census.is_preparing_or_sealed()
        {
            self.poison(QuantityCaptureIssue::InvalidFacts);
        }
        let Some(next) = self.applied_world_transactions.checked_add(1) else {
            self.poison(QuantityCaptureIssue::Capacity);
            return;
        };
        self.applied_world_transactions = next;
        if let Some(issue) = pending.issue {
            self.poison(issue);
        }
        let reconciled = (|| {
            if self.base_usage.is_some()
                || (!pending.entries.is_empty() && pending.base_usage != Some(self.usage))
            {
                return None;
            }
            let mut increment = QuantityCandidateUsage::default();
            for (hash, entry) in pending.entries.iter() {
                let measured = entry.measurement;
                let applied = self.entries.get(hash);
                let original = applied
                    .map(|entry| entry.measurement.full)
                    .unwrap_or_default();
                if measured.baseline != original
                    || measured.full.entries != 1
                    || applied.is_some_and(|existing| existing.context != entry.context)
                    || u64::try_from(entry.effects.len()).ok()? != measured.full.deltas
                {
                    return None;
                }
                increment = increment.checked_add(measured.full.checked_sub(measured.baseline)?)?;
            }
            if increment != pending.usage {
                return None;
            }
            self.usage.checked_add(increment)
        })();
        let Some(usage) = reconciled else {
            self.poison(QuantityCaptureIssue::InvalidFacts);
            return;
        };
        if !pending.entries.is_empty() {
            let Some(backing) = pending.parent_backing.take() else {
                self.poison(QuantityCaptureIssue::InvalidFacts);
                return;
            };
            if backing.capacity() < self.entries.len() + pending.entries.len() {
                self.poison(QuantityCaptureIssue::InvalidFacts);
                return;
            }
            // Measurements become complete parent frames in place; payloads stay immutable.
            // The destination backing was retained before the original callback executed.
            pending.entries.for_each_mut(|entry| {
                entry.measurement.baseline = QuantityCandidateUsage::default()
            });
            self.entries.apply_pending(pending.entries, backing);
        }
        self.usage = usage;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreparedQuantityAccounting {
    parent_before: QuantityCandidateUsage,
    pending_before: QuantityCandidateUsage,
    old_full: QuantityCandidateUsage,
    measurement: QuantityEntryMeasurement,
    pending_after: QuantityCandidateUsage,
}

/// Move-only captured tape and exact write plan, still bound to its original execution owner.
pub(crate) struct PreparedQuantityCapture {
    write_plan: Option<QuantityWritePlan<QuantityWriteKey, Quantity>>,
    accounting: PreparedQuantityAccounting,
    tape: QuantityTape,
    pending_backing: ChargedBuffer<(Hash, QuantityArchivedEntry)>,
    parent_backing: ChargedBuffer<(Hash, QuantityArchivedEntry)>,
}

impl StateBlock<'_> {
    /// Compare the original MV writer lineage with every classified State transaction.
    /// Direct WorldBlock mutation and a raw WorldTransaction apply cannot disappear
    /// merely because the final quantities happen to equal the captured post-state.
    pub(super) fn observe_quantity_block_journals(&mut self) {
        let balances = self.world.assets.write_observation();
        let supplies = self.world.asset_definitions.write_observation();
        let expected = self.fastpq_quantity_candidate.applied_world_transactions;
        if balances.0 || supplies.0 || balances.1 != expected || supplies.1 != expected {
            self.fastpq_quantity_candidate
                .poison(QuantityCaptureIssue::UnownedMutation);
        }
        self.observe_quantity_source_census();
    }
}

impl WorldTransaction<'_, '_> {
    /// Edit only metadata/policy fields; no mutable supply or balance-scope lease escapes.
    pub(crate) fn asset_definition_metadata_mut(
        &mut self,
        id: &AssetDefinitionId,
    ) -> Result<super::fastpq_quantity_storage::AssetDefinitionMetadataMut<'_>, FindError> {
        self.asset_definitions
            .metadata_mut(id)
            .ok_or_else(|| FindError::AssetDefinition(id.clone()))
    }

    /// Apply the prechecked total through one exact operation-owned write port.
    pub(super) fn assign_quantity_supply(&mut self, id: &AssetDefinitionId, value: Quantity) {
        if let Some(plan) = self.quantity_mutation_observation.plan.as_mut() {
            assert!(
                self.asset_definitions.write_supply(id, value, plan),
                "prechecked original definition remains"
            );
        } else {
            self.asset_definitions
                .get_mut(id)
                .expect("prechecked original definition remains")
                .total_quantity = value;
        }
    }

    /// Validate every fallible balance-assignment input before any paired write.
    pub(crate) fn precheck_quantity_balance_assignment(
        &self,
        id: &AssetId,
        value: &Quantity,
    ) -> Result<(), Error> {
        let spec = self.asset_definition(id.definition())?.spec();
        self.account(id.account())?;
        ensure_asset_quantity_value(value, spec)?;
        if let Some(existing) = self.assets.get(id) {
            ensure_asset_quantity_value(existing.as_ref(), spec)?;
        }
        Ok(())
    }

    /// Assign after all paired inputs passed their original checks under this same
    /// exclusive World transaction; no fallible business step follows the first write.
    pub(crate) fn assign_prechecked_quantity_balance(&mut self, id: &AssetId, value: Quantity) {
        if self.assets.get(id).is_none() {
            self.emit_asset_event(AssetEvent::Created(Asset::new(
                id.clone(),
                Quantity::zero(),
            )));
            self.track_asset_holder(id);
        }
        let value = AssetValue::new(value);
        if let Some(plan) = self.quantity_mutation_observation.plan.as_mut() {
            self.assets.write_balance(id.clone(), Some(value), plan);
        } else {
            self.assets.insert(id.clone(), value);
        }
    }

    /// Check and assign one exact balance while preserving its zero-valued creation
    /// event and holder-index behavior. Paired writes precheck both sides first.
    pub(crate) fn assign_quantity_balance_exact(
        &mut self,
        id: &AssetId,
        value: Quantity,
    ) -> Result<(), Error> {
        self.precheck_quantity_balance_assignment(id, &value)?;
        self.assign_prechecked_quantity_balance(id, value);
        Ok(())
    }

    /// Remove the actual balance through its exact port; the caller retains metadata cleanup.
    pub(super) fn remove_quantity_balance(&mut self, id: &AssetId) -> Option<AssetValue> {
        if let Some(plan) = self.quantity_mutation_observation.plan.as_mut() {
            self.assets.write_balance(id.clone(), None, plan)
        } else {
            self.assets.remove(id.clone())
        }
    }
}

impl StateTransaction<'_, '_> {
    /// Frozen diagnostic preimage ceiling; inspection grants no invocation owner.
    pub(crate) fn quantity_candidate_preimage_limit(&self) -> u64 {
        let profile = self.fastpq_source_policy.0;
        if self.tx_call_hash.is_some() || self.fastpq_source_quota.has_native_purpose() {
            profile.intrinsic.max_input_transcript_bytes
        } else {
            profile.mandatory.per_obligation.max_input_transcript_bytes
        }
    }

    /// Record an unsupported authenticated owner without changing its business execution.
    pub(crate) fn poison_quantity_candidate_owner(&mut self) {
        self.quantity_candidate_issue(QuantityCaptureIssue::UnsupportedOwner);
    }

    /// Check arithmetic and exact live lifecycle without allocating a projection map.
    fn quantity_kind_matches_live_lifecycle(&self, kind: QuantityKindInput<'_>) -> bool {
        let live = |balance: QuantityBalanceInput<'_>| {
            self.world
                .asset_definitions
                .get(balance.definition)
                .is_some()
                && self.world.axt_asset_incarnations.get(balance.definition)
                    == Some(&balance.incarnation)
                && balance.incarnation.validate().is_ok()
        };
        match kind {
            QuantityKindInput::Transfer(value) => {
                live(value.source)
                    && live(value.destination)
                    && value.source.definition == value.destination.definition
                    && value.source.incarnation == value.destination.incarnation
                    && value
                        .source_after
                        .checked_add_equals(value.amount, value.source_before)
                    && value
                        .destination_before
                        .checked_add_equals(value.amount, value.destination_after)
                    && (value.source != value.destination
                        || value.source_after == value.destination_before)
            }
            QuantityKindInput::Mint(value) => {
                live(value.balance)
                    && !value.amount.is_zero()
                    && value
                        .balance_before
                        .checked_add_equals(value.amount, value.balance_after)
                    && value
                        .supply_before
                        .checked_add_equals(value.amount, value.supply_after)
            }
            QuantityKindInput::Burn(value) => {
                live(value.balance)
                    && value
                        .balance_after
                        .checked_add_equals(value.amount, value.balance_before)
                    && value
                        .supply_after
                        .checked_add_equals(value.amount, value.supply_before)
            }
        }
    }

    #[cfg(test)]
    fn quantity_facts_match_live_lifecycle(&self, kinds: &[FastpqExecutionEffectKindV1]) -> bool {
        kinds
            .iter()
            .all(|kind| self.quantity_kind_matches_live_lifecycle(kind.into()))
    }

    fn quantity_projection(&self, key: &QuantityWriteKey) -> Option<&Quantity> {
        match key {
            QuantityWriteKey::Balance(id) => self.world.assets.get(id).map(AsRef::as_ref),
            QuantityWriteKey::Supply(id) => self
                .world
                .asset_definitions
                .get(id)
                .map(|definition| definition.total_quantity()),
        }
    }

    fn quantity_pre_state_matches(
        &self,
        plan: &QuantityWritePlan<QuantityWriteKey, Quantity>,
    ) -> bool {
        let zero = Quantity::zero();
        let mut previous: Option<(&QuantityWriteKey, &Quantity)> = None;
        for (key, before, after) in plan.ordered_projections() {
            let expected = match previous {
                Some((previous_key, after)) if previous_key == key => after,
                _ => self.quantity_projection(key).unwrap_or(&zero),
            };
            if expected != before {
                return false;
            }
            previous = Some((key, after));
        }
        true
    }

    fn quantity_post_state_matches(
        &self,
        plan: &QuantityWritePlan<QuantityWriteKey, Quantity>,
    ) -> bool {
        if !plan.lifecycles().iter().all(|(id, incarnation)| {
            self.world.asset_definitions.get(id).is_some()
                && self.world.axt_asset_incarnations.get(id) == Some(incarnation)
        }) {
            return false;
        }
        let zero = Quantity::zero();
        let mut projections = plan.ordered_projections().peekable();
        while let Some((key, _, after)) = projections.next() {
            if projections.peek().is_some_and(|(next, _, _)| *next == key) {
                continue;
            }
            if self.quantity_projection(key).unwrap_or(&zero) != after {
                return false;
            }
        }
        true
    }

    #[cfg(test)]
    fn quantity_expected_state(
        &self,
        kinds: &[FastpqExecutionEffectKindV1],
    ) -> Result<QuantityWritePlan<QuantityWriteKey, Quantity>, QuantityCaptureIssue> {
        if !self.quantity_facts_match_live_lifecycle(kinds) {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let effects = kinds
            .iter()
            .enumerate()
            .map(|(ordinal, kind)| FastpqExecutionEffectV1 {
                ordinal: u32::try_from(ordinal).unwrap(),
                authority_digest: Hash::new([]),
                authorization_context: Hash::new([]),
                kind: kind.clone(),
            })
            .collect::<Vec<_>>();
        let plan = QuantityWritePlan::from_effects(
            &effects,
            kinds.len() * 2,
            self.pipeline_ivm_prepared_cache.execution_budget(),
        )
        .map_err(|_| QuantityCaptureIssue::Capacity)?;
        if !self.quantity_pre_state_matches(&plan) {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        Ok(plan)
    }

    fn quantity_candidate_issue(&mut self, issue: QuantityCaptureIssue) {
        self.pending_fastpq_quantity_candidate.poison(issue);
    }
    fn quantity_balance_input<'a>(
        &self,
        id: &'a AssetId,
    ) -> Result<QuantityBalanceInput<'a>, QuantityCaptureIssue> {
        if self.world.asset_definitions.get(id.definition()).is_none() {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let incarnation = self
            .world
            .axt_asset_incarnations
            .get(id.definition())
            .copied()
            .ok_or(QuantityCaptureIssue::MissingIncarnation)?;
        incarnation
            .validate()
            .map_err(|_| QuantityCaptureIssue::MissingIncarnation)?;
        Ok(QuantityBalanceInput {
            definition: id.definition(),
            incarnation,
            account: id.account(),
            scope: *id.scope(),
        })
    }
    #[cfg(test)]
    fn quantity_balance_identity(
        &self,
        id: &AssetId,
    ) -> Result<FastpqExecutionBalanceV1, QuantityCaptureIssue> {
        let input = self.quantity_balance_input(id)?;
        Ok(FastpqExecutionBalanceV1 {
            asset: FastpqExecutionAssetV1 {
                definition: input.definition.clone(),
                incarnation: input.incarnation,
            },
            account: input.account.clone(),
            scope: input.scope,
        })
    }
    #[cfg(test)]
    fn prepare_quantity_candidate(
        &self,
        authority: &AccountId,
        entry_hash: Hash,
        authorization_context: Hash,
        kinds: Vec<FastpqExecutionEffectKindV1>,
    ) -> Result<PreparedQuantityCapture, QuantityCaptureIssue> {
        self.prepare_quantity_candidate_inputs(
            authority,
            entry_hash,
            authorization_context,
            kinds.iter().map(|kind| Ok(kind.into())),
        )
    }
    fn prepare_quantity_candidate_inputs<'a, Inputs>(
        &self,
        authority: &AccountId,
        entry_hash: Hash,
        authorization_context: Hash,
        kinds: Inputs,
    ) -> Result<PreparedQuantityCapture, QuantityCaptureIssue>
    where
        Inputs:
            Clone + ExactSizeIterator<Item = Result<QuantityKindInput<'a>, QuantityCaptureIssue>>,
    {
        if self.world.assets.has_raw_write() || self.world.asset_definitions.has_raw_write() {
            return Err(QuantityCaptureIssue::UnownedMutation);
        }
        if self.pending_fastpq_quantity_candidate.issue.is_some()
            || self.block_fastpq_quantity_candidate.issue.is_some()
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        for kind in kinds.clone() {
            if !self.quantity_kind_matches_live_lifecycle(kind?) {
                return Err(QuantityCaptureIssue::InvalidFacts);
            }
        }
        let captured = self
            .fastpq_source_context
            .capture_transcript(
                self.tx_call_hash,
                entry_hash,
                self.current_lane_id,
                self.current_dataspace_id,
                *self.committed_fragments,
            )
            .map_err(|_| QuantityCaptureIssue::InvalidFacts)?;
        self.fastpq_source_quota
            .require_existing_quantity_capture_entry(entry_hash, captured.is_protocol_purpose())
            .map_err(|_| QuantityCaptureIssue::UnsupportedOwner)?;
        let context = FastpqExecutionEffectContextV1 {
            source: captured.source(),
            entry: FastpqSourceExecutionEntryV1 {
                entry_hash,
                execution_kind: captured.execution_kind(),
                route: captured.route(),
                dataspace_id: captured.dataspace_id(),
            },
        };
        let block = self
            .block_fastpq_quantity_candidate
            .entries
            .get(&entry_hash);
        let pending = self
            .pending_fastpq_quantity_candidate
            .entries
            .get(&entry_hash);
        if block
            .into_iter()
            .chain(pending)
            .any(|entry| entry.context != context)
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let prefix = pending
            .or(block)
            .map_or(&[][..], |entry| entry.effects.as_slice());
        let ordinal = prefix.len();
        let total = ordinal
            .checked_add(kinds.len())
            .ok_or(QuantityCaptureIssue::Capacity)?;
        let profile = self.fastpq_source_policy.0;
        let limit = if captured.is_protocol_purpose()
            && !self.fastpq_source_quota.is_native_purpose(entry_hash)
        {
            profile.mandatory.per_obligation
        } else {
            profile.intrinsic
        };
        let tape = QuantityTape::prepare_inputs(
            context,
            prefix,
            kinds,
            crate::fastpq::authority_digest(authority),
            authorization_context,
            limit.max_deltas as usize,
            self.pipeline_ivm_prepared_cache.execution_budget(),
        )?;
        let mut frames = 0usize;
        for effect in &tape.effects {
            frames = frames
                .checked_add(
                    norito::canonical_frame_len(effect)
                        .map_err(|_| QuantityCaptureIssue::InvalidFacts)?,
                )
                .ok_or(QuantityCaptureIssue::Capacity)?;
            if u64::try_from(frames).map_err(|_| QuantityCaptureIssue::Capacity)?
                > limit.max_input_transcript_bytes
            {
                return Err(QuantityCaptureIssue::Capacity);
            }
        }
        let bytes = norito::canonical_frame_len(tape.wire())
            .map_err(|_| QuantityCaptureIssue::InvalidFacts)?;
        if u64::try_from(bytes).map_err(|_| QuantityCaptureIssue::Capacity)?
            > limit.max_statement_bytes
        {
            return Err(QuantityCaptureIssue::Capacity);
        }
        let parent = &*self.block_fastpq_quantity_candidate;
        let pending_archive = &self.pending_fastpq_quantity_candidate;
        if pending_archive
            .base_usage
            .is_some_and(|base| base != parent.usage)
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let baseline = block
            .map(|entry| entry.measurement.full)
            .unwrap_or_default();
        if pending.is_some_and(|entry| entry.measurement.baseline != baseline) {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let old_full = pending
            .map(|entry| entry.measurement.full)
            .unwrap_or(baseline);
        let full = QuantityCandidateUsage {
            entries: 1,
            deltas: u64::try_from(total).map_err(|_| QuantityCaptureIssue::Capacity)?,
            input_bytes: u64::try_from(frames).map_err(|_| QuantityCaptureIssue::Capacity)?,
            statement_bytes: u64::try_from(bytes).map_err(|_| QuantityCaptureIssue::Capacity)?,
        };
        let aggregate = parent
            .usage
            .checked_add(pending_archive.usage)
            .and_then(|usage| usage.checked_sub(old_full))
            .and_then(|usage| usage.checked_add(full))
            .ok_or(QuantityCaptureIssue::Capacity)?;
        if aggregate.entries > u64::from(profile.block.max_executed_entries)
            || aggregate.deltas > u64::from(profile.block.max_deltas)
            || aggregate.input_bytes > profile.block.max_input_transcript_bytes
            || aggregate.statement_bytes > profile.block.max_total_statement_bytes
        {
            return Err(QuantityCaptureIssue::Capacity);
        }
        let accounting = PreparedQuantityAccounting {
            parent_before: parent.usage,
            pending_before: pending_archive.usage,
            old_full,
            measurement: QuantityEntryMeasurement { baseline, full },
            pending_after: aggregate
                .checked_sub(parent.usage)
                .ok_or(QuantityCaptureIssue::InvalidFacts)?,
        };
        let pending_capacity = pending_archive
            .entries
            .len()
            .checked_add(1)
            .ok_or(QuantityCaptureIssue::Capacity)?;
        let parent_capacity = parent
            .entries
            .len()
            .checked_add(pending_capacity)
            .ok_or(QuantityCaptureIssue::Capacity)?;
        let pending_backing = QuantityArchiveMap::reserve(
            pending_capacity,
            self.pipeline_ivm_prepared_cache.execution_budget(),
        )?;
        let parent_backing = QuantityArchiveMap::reserve(
            parent_capacity,
            self.pipeline_ivm_prepared_cache.execution_budget(),
        )?;
        let write_plan = QuantityWritePlan::from_effects(
            &tape.effects[ordinal..],
            usize::try_from(limit.max_deltas)
                .ok()
                .and_then(|count| count.checked_mul(2))
                .ok_or(QuantityCaptureIssue::Capacity)?,
            self.pipeline_ivm_prepared_cache.execution_budget(),
        )
        .map_err(|_| QuantityCaptureIssue::Capacity)?;
        if !self.quantity_pre_state_matches(&write_plan) {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        Ok(PreparedQuantityCapture {
            write_plan: Some(write_plan),
            accounting,
            tape,
            pending_backing,
            parent_backing,
        })
    }
    /// Apply an original business owner under its already prepared capture, or retain refusal.
    pub(crate) fn apply_with_quantity_candidate<T>(
        &mut self,
        prepared: Result<PreparedQuantityCapture, QuantityCaptureIssue>,
        apply: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut prepared = match prepared {
            Ok(prepared) => Some(prepared),
            Err(issue) => {
                self.quantity_candidate_issue(issue);
                None
            }
        };
        if self.world.quantity_mutation_observation.owned {
            self.quantity_candidate_issue(QuantityCaptureIssue::InterruptedScope);
        }
        let previous_owned = self.world.quantity_mutation_observation.owned;
        let previous_plan = self.world.quantity_mutation_observation.plan.take();
        if previous_plan.is_some() {
            self.quantity_candidate_issue(QuantityCaptureIssue::InterruptedScope);
        }
        self.world.quantity_mutation_observation.owned = true;
        self.world.quantity_mutation_observation.plan = prepared
            .as_mut()
            .and_then(|prepared| prepared.write_plan.take());
        let result = apply(self);
        let completed_plan = self.world.quantity_mutation_observation.plan.take();
        let post_state_matches = completed_plan
            .as_ref()
            .is_some_and(|plan| self.quantity_post_state_matches(plan));
        self.world.quantity_mutation_observation.plan = previous_plan;
        self.world.quantity_mutation_observation.owned = previous_owned;
        if prepared.is_some() && completed_plan.is_none_or(|plan| plan.finish().is_err()) {
            self.quantity_candidate_issue(QuantityCaptureIssue::InvalidFacts);
        }
        if self.world.assets.has_raw_write() || self.world.asset_definitions.has_raw_write() {
            self.world.quantity_mutation_observation.unowned = true;
            self.quantity_candidate_issue(QuantityCaptureIssue::UnownedMutation);
        }
        if result.is_err() {
            // A caller may catch an error after partial writes. Such a candidate can never export.
            self.quantity_candidate_issue(QuantityCaptureIssue::InterruptedScope);
        }
        if result.is_ok() {
            if let Some(prepared) = prepared {
                let hash = prepared.tape.context.entry.entry_hash;
                let current_full = self
                    .pending_fastpq_quantity_candidate
                    .entries
                    .get(&hash)
                    .or_else(|| self.block_fastpq_quantity_candidate.entries.get(&hash))
                    .map(|entry| entry.measurement.full)
                    .unwrap_or_default();
                if self.pending_fastpq_quantity_candidate.issue.is_some()
                    || self.block_fastpq_quantity_candidate.issue.is_some()
                    || self.pending_fastpq_quantity_candidate.usage
                        != prepared.accounting.pending_before
                    || self.block_fastpq_quantity_candidate.usage
                        != prepared.accounting.parent_before
                    || current_full != prepared.accounting.old_full
                {
                    self.quantity_candidate_issue(QuantityCaptureIssue::InterruptedScope);
                    return result;
                }
                if !post_state_matches
                    || !prepared.tape.effects.iter().all(|effect| {
                        self.quantity_kind_matches_live_lifecycle((&effect.kind).into())
                    })
                {
                    self.quantity_candidate_issue(QuantityCaptureIssue::InvalidFacts);
                    return result;
                }
                let pending = &mut self.pending_fastpq_quantity_candidate;
                pending.entries.grow(prepared.pending_backing);
                pending.entries.insert_reserved(
                    hash,
                    QuantityArchivedEntry {
                        tape: prepared.tape,
                        measurement: prepared.accounting.measurement,
                    },
                );
                pending.parent_backing = Some(prepared.parent_backing);
                pending.usage = prepared.accounting.pending_after;
                pending.base_usage = Some(prepared.accounting.parent_before);
            }
        }
        result
    }
    /// Observe an already-authorized exact transfer; preparation failure cannot alter business execution.
    /// The caller supplies actual resolved storage IDs, never reconstructed legacy transcript keys.
    pub(crate) fn apply_with_quantity_transfer_candidate<T>(
        &mut self,
        authority: &AccountId,
        entry_hash: Hash,
        authorization_context: Hash,
        legs: &[(AssetId, AssetId, TransferDeltaTranscript)],
        apply: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let prepared = (|| {
            let max = self.fastpq_source_policy.0.intrinsic.max_deltas.max(
                self.fastpq_source_policy
                    .0
                    .mandatory
                    .per_obligation
                    .max_deltas,
            ) as usize;
            if legs.len() > max {
                return Err(QuantityCaptureIssue::Capacity);
            }
            let kinds = legs.iter().map(|(source, destination, delta)| {
                if source.definition() != destination.definition()
                    || source.definition() != &delta.asset_definition
                    || source.account() != &delta.from_account
                    || destination.account() != &delta.to_account
                {
                    return Err(QuantityCaptureIssue::InvalidFacts);
                }
                Ok(QuantityKindInput::Transfer(QuantityTransferInput {
                    source: self.quantity_balance_input(source)?,
                    destination: self.quantity_balance_input(destination)?,
                    amount: &delta.amount,
                    source_before: &delta.from_balance_before,
                    source_after: &delta.from_balance_after,
                    destination_before: &delta.to_balance_before,
                    destination_after: &delta.to_balance_after,
                }))
            });
            self.prepare_quantity_candidate_inputs(
                authority,
                entry_hash,
                authorization_context,
                kinds,
            )
        })();
        self.apply_with_quantity_candidate(prepared, apply)
    }
    /// Prepare an exact account-removal burn without changing its original write order.
    /// The existing signed invocation and quota checks still own every captured fact.
    pub(crate) fn prepare_quantity_account_removal_candidate(
        &self,
        authority: &AccountId,
        entry_hash: Hash,
        authorization_context: Hash,
        id: &AssetId,
        amount: &Quantity,
        supply_after: &Quantity,
    ) -> Result<PreparedQuantityCapture, QuantityCaptureIssue> {
        let mut prepared = self.prepare_quantity_supply_candidate(
            authority,
            entry_hash,
            authorization_context,
            id,
            amount,
            false,
            &Quantity::zero(),
            supply_after,
        )?;
        prepared
            .write_plan
            .as_mut()
            .ok_or(QuantityCaptureIssue::InvalidFacts)?
            .order_supply_before_complete_removal(id, amount)
            .map_err(|_| QuantityCaptureIssue::InvalidFacts)?;
        Ok(prepared)
    }

    /// Borrow exact before-state and the values retained by the original supply owner.
    /// No capture-only arithmetic or intermediate owned effect allocation occurs here.
    pub(crate) fn prepare_quantity_supply_candidate(
        &self,
        authority: &AccountId,
        entry_hash: Hash,
        authorization_context: Hash,
        id: &AssetId,
        amount: &Quantity,
        mint: bool,
        balance_after: &Quantity,
        supply_after: &Quantity,
    ) -> Result<PreparedQuantityCapture, QuantityCaptureIssue> {
        let zero = Quantity::zero();
        let balance = self.quantity_balance_input(id)?;
        let definition = self
            .world
            .asset_definition(id.definition())
            .map_err(|_| QuantityCaptureIssue::InvalidFacts)?;
        let change = QuantitySupplyInput {
            balance,
            amount,
            balance_before: self
                .world
                .assets
                .get(id)
                .map(AsRef::as_ref)
                .unwrap_or(&zero),
            balance_after,
            supply_before: definition.total_quantity(),
            supply_after,
        };
        let kind = if mint {
            QuantityKindInput::Mint(change)
        } else {
            QuantityKindInput::Burn(change)
        };
        self.prepare_quantity_candidate_inputs(
            authority,
            entry_hash,
            authorization_context,
            std::iter::once(Ok(kind)),
        )
    }
}

#[cfg(test)]
#[path = "fastpq_quantity_capture_tests.rs"]
mod tests;
