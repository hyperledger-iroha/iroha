//! Bounded, rollback-local candidate capture of actual typed quantity effects.
//!
//! This journal is diagnostic preparation for the complete ordinary relation. It
//! cannot export proof/source authority: raw storage mutation coverage and every
//! retained mandatory supply owner must be closed before that boundary is added.
//! Capture failures poison only this candidate, preserving current business
//! execution and existing replay publication. Facts and poison apply together.

use super::*;
use iroha_data_model::fastpq::{
    FastpqExecutionAssetV1, FastpqExecutionBalanceV1, FastpqExecutionEffectContextV1,
    FastpqExecutionEffectKindV1, FastpqExecutionEffectV1, FastpqExecutionEffectsV1,
    FastpqExecutionSupplyChangeV1, FastpqExecutionTransferV1, FastpqSourceExecutionEntryV1,
};

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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "TODO: require_complete is not yet a production consumer"
        )
    )]
    IncompleteCoverage,
}

/// Transaction-owned mutation observation, discarded with its original World overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct QuantityMutationObservation {
    owned: bool,
    unowned: bool,
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

/// Whole candidate facts, never an authenticated source manifest.
#[derive(Debug, Default)]
pub(crate) struct QuantityCandidateArchive {
    entries: BTreeMap<Hash, FastpqExecutionEffectsV1>,
    measured_entries: BTreeMap<Hash, QuantityEntryMeasurement>,
    /// Applied full usage, or pending incremental usage beyond `base_usage`.
    usage: QuantityCandidateUsage,
    /// Original applied aggregate against which this transaction prepared its deltas.
    base_usage: Option<QuantityCandidateUsage>,
    issue: Option<QuantityCaptureIssue>,
}
impl QuantityCandidateArchive {
    fn poison(&mut self, issue: QuantityCaptureIssue) {
        if self.issue.is_none() {
            self.issue = Some(issue);
        }
    }
    /// Candidate export is explicitly refused until the remaining mutation owners join.
    /// TODO: replace this refusal only with complete, independently owned coverage and quotas.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "TODO: export remains closed until complete owner coverage and quota integration"
        )
    )]
    pub(crate) fn require_complete(&self) -> Result<(), QuantityCaptureIssue> {
        Err(self
            .issue
            .unwrap_or(QuantityCaptureIssue::IncompleteCoverage))
    }
    pub(super) fn observe(&mut self, observation: &QuantityMutationObservation) {
        if observation.unowned {
            self.poison(QuantityCaptureIssue::UnownedMutation);
        }
        if observation.owned {
            self.poison(QuantityCaptureIssue::InterruptedScope);
        }
    }
    pub(super) fn apply(&mut self, mut pending: Self) {
        if let Some(issue) = pending.issue {
            self.poison(issue);
        }
        let reconciled = (|| {
            if self.base_usage.is_some()
                || pending.entries.len() != pending.measured_entries.len()
                || (!pending.entries.is_empty() && pending.base_usage != Some(self.usage))
            {
                return None;
            }
            let mut increment = QuantityCandidateUsage::default();
            for (hash, entry) in &pending.entries {
                let measured = pending.measured_entries.get(hash)?;
                let original = self
                    .measured_entries
                    .get(hash)
                    .map(|value| value.full)
                    .unwrap_or_default();
                if measured.baseline != original || measured.full.entries != 1 {
                    return None;
                }
                let applied = self.entries.get(hash);
                if applied.is_some() != self.measured_entries.contains_key(hash)
                    || applied.is_some_and(|existing| existing.context != entry.context)
                    || u64::try_from(applied.map_or(0, |existing| existing.effects.len()))
                        .ok()?
                        .checked_add(u64::try_from(entry.effects.len()).ok()?)?
                        != measured.full.deltas
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
            // World has already applied. Preserve business semantics and refuse this candidate.
            self.poison(QuantityCaptureIssue::InvalidFacts);
            return;
        };
        for (hash, mut entry) in pending.entries {
            let measured = pending
                .measured_entries
                .remove(&hash)
                .expect("reconciled pending entry retains its measured frame");
            match self.entries.get_mut(&hash) {
                Some(existing) => existing.effects.append(&mut entry.effects),
                None => {
                    self.entries.insert(hash, entry);
                }
            }
            self.measured_entries.insert(
                hash,
                QuantityEntryMeasurement {
                    baseline: QuantityCandidateUsage::default(),
                    full: measured.full,
                },
            );
        }
        self.usage = usage;
    }
}

struct ExpectedQuantityState {
    balances: BTreeMap<AssetId, (iroha_data_model::nexus::AxtAssetIncarnationV1, Quantity)>,
    supplies:
        BTreeMap<AssetDefinitionId, (iroha_data_model::nexus::AxtAssetIncarnationV1, Quantity)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PreparedQuantityAccounting {
    parent_before: QuantityCandidateUsage,
    pending_before: QuantityCandidateUsage,
    old_full: QuantityCandidateUsage,
    measurement: QuantityEntryMeasurement,
    pending_after: QuantityCandidateUsage,
}

struct PreparedQuantityCapture {
    accounting: PreparedQuantityAccounting,
    expected: ExpectedQuantityState,
    context: FastpqExecutionEffectContextV1,
    effects: Vec<FastpqExecutionEffectV1>,
}

impl StateTransaction<'_, '_> {
    /// Frozen diagnostic preimage ceiling; inspection grants no invocation owner.
    pub(crate) fn quantity_candidate_preimage_limit(&self) -> u64 {
        let profile = self.fastpq_source_policy.0;
        if self.tx_call_hash.is_some() {
            profile.intrinsic.max_input_transcript_bytes
        } else {
            profile.mandatory.per_obligation.max_input_transcript_bytes
        }
    }

    /// Record an unsupported authenticated owner without changing its business execution.
    pub(crate) fn poison_quantity_candidate_owner(&mut self) {
        self.quantity_candidate_issue(QuantityCaptureIssue::UnsupportedOwner);
    }

    fn quantity_expected_state(
        &self,
        kinds: &[FastpqExecutionEffectKindV1],
    ) -> Result<ExpectedQuantityState, QuantityCaptureIssue> {
        let mut expected = ExpectedQuantityState {
            balances: BTreeMap::new(),
            supplies: BTreeMap::new(),
        };
        for kind in kinds {
            match kind {
                FastpqExecutionEffectKindV1::Transfer(transfer) => {
                    if transfer.source.asset != transfer.destination.asset
                        || !transfer
                            .source_after
                            .checked_add_equals(&transfer.amount, &transfer.source_before)
                        || !transfer
                            .destination_before
                            .checked_add_equals(&transfer.amount, &transfer.destination_after)
                    {
                        return Err(QuantityCaptureIssue::InvalidFacts);
                    }
                    self.advance_quantity_balance(
                        &mut expected,
                        &transfer.source,
                        &transfer.source_before,
                        &transfer.source_after,
                    )?;
                    self.advance_quantity_balance(
                        &mut expected,
                        &transfer.destination,
                        &transfer.destination_before,
                        &transfer.destination_after,
                    )?;
                }
                FastpqExecutionEffectKindV1::Mint(change)
                | FastpqExecutionEffectKindV1::Burn(change) => {
                    let mint = matches!(kind, FastpqExecutionEffectKindV1::Mint(_));
                    let consistent = if mint {
                        !change.amount.is_zero()
                            && change
                                .balance_before
                                .checked_add_equals(&change.amount, &change.balance_after)
                            && change
                                .supply_before
                                .checked_add_equals(&change.amount, &change.supply_after)
                    } else {
                        change
                            .balance_after
                            .checked_add_equals(&change.amount, &change.balance_before)
                            && change
                                .supply_after
                                .checked_add_equals(&change.amount, &change.supply_before)
                    };
                    if !consistent {
                        return Err(QuantityCaptureIssue::InvalidFacts);
                    }
                    self.advance_quantity_balance(
                        &mut expected,
                        &change.balance,
                        &change.balance_before,
                        &change.balance_after,
                    )?;
                    let asset = &change.balance.asset;
                    let before = expected
                        .supplies
                        .get(&asset.definition)
                        .map(|(_, value)| value.clone())
                        .unwrap_or_else(|| {
                            self.world
                                .asset_definitions
                                .get(&asset.definition)
                                .map(|definition| definition.total_quantity().clone())
                                .unwrap_or_else(Quantity::zero)
                        });
                    if before != change.supply_before {
                        return Err(QuantityCaptureIssue::InvalidFacts);
                    }
                    expected.supplies.insert(
                        asset.definition.clone(),
                        (asset.incarnation, change.supply_after.clone()),
                    );
                }
            }
        }
        Ok(expected)
    }

    fn advance_quantity_balance(
        &self,
        expected: &mut ExpectedQuantityState,
        balance: &FastpqExecutionBalanceV1,
        before: &Quantity,
        after: &Quantity,
    ) -> Result<(), QuantityCaptureIssue> {
        let id = AssetId::with_scope(
            balance.asset.definition.clone(),
            balance.account.clone(),
            balance.scope,
        );
        if self.quantity_balance_identity(&id)? != *balance {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let actual = expected
            .balances
            .get(&id)
            .map(|(_, value)| value.clone())
            .unwrap_or_else(|| {
                self.world
                    .assets
                    .get(&id)
                    .map(|value| value.as_ref().clone())
                    .unwrap_or_else(Quantity::zero)
            });
        if actual != *before {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        expected
            .balances
            .insert(id, (balance.asset.incarnation, after.clone()));
        Ok(())
    }

    fn quantity_post_state_matches(&self, expected: &ExpectedQuantityState) -> bool {
        expected.balances.iter().all(|(id, (incarnation, value))| {
            self.world.axt_asset_incarnations.get(id.definition()) == Some(incarnation)
                && self
                    .world
                    .assets
                    .get(id)
                    .map(|actual| actual.as_ref() == value)
                    .unwrap_or_else(|| value.is_zero())
        }) && expected.supplies.iter().all(|(id, (incarnation, value))| {
            self.world.axt_asset_incarnations.get(id) == Some(incarnation)
                && self
                    .world
                    .asset_definitions
                    .get(id)
                    .is_some_and(|definition| definition.total_quantity() == value)
        })
    }

    fn quantity_candidate_issue(&mut self, issue: QuantityCaptureIssue) {
        self.pending_fastpq_quantity_candidate.poison(issue);
    }
    fn quantity_balance_identity(
        &self,
        id: &AssetId,
    ) -> Result<FastpqExecutionBalanceV1, QuantityCaptureIssue> {
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
        Ok(FastpqExecutionBalanceV1 {
            asset: FastpqExecutionAssetV1 {
                definition: id.definition().clone(),
                incarnation,
            },
            account: id.account().clone(),
            scope: *id.scope(),
        })
    }
    fn prepare_quantity_candidate(
        &mut self,
        authority: &AccountId,
        entry_hash: Hash,
        authorization_context: Hash,
        kinds: Vec<FastpqExecutionEffectKindV1>,
    ) -> Result<PreparedQuantityCapture, QuantityCaptureIssue> {
        if self.pending_fastpq_quantity_candidate.issue.is_some()
            || self.block_fastpq_quantity_candidate.issue.is_some()
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let expected = self.quantity_expected_state(&kinds)?;
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
        let ordinal = block
            .map_or(0, |entry| entry.effects.len())
            .checked_add(pending.map_or(0, |entry| entry.effects.len()))
            .ok_or(QuantityCaptureIssue::Capacity)?;
        let total = ordinal
            .checked_add(kinds.len())
            .ok_or(QuantityCaptureIssue::Capacity)?;
        let profile = self.fastpq_source_policy.0;
        let limit = if captured.is_protocol_purpose() {
            profile.mandatory.per_obligation
        } else {
            profile.intrinsic
        };
        if total > limit.max_deltas as usize {
            return Err(QuantityCaptureIssue::Capacity);
        }
        let mut effects = Vec::new();
        effects
            .try_reserve_exact(kinds.len())
            .map_err(|_| QuantityCaptureIssue::Capacity)?;
        for (index, kind) in kinds.into_iter().enumerate() {
            effects.push(FastpqExecutionEffectV1 {
                ordinal: u32::try_from(ordinal + index)
                    .map_err(|_| QuantityCaptureIssue::Capacity)?,
                authority_digest: crate::fastpq::authority_digest(authority),
                authorization_context,
                kind,
            });
        }
        // Count all complete effect frames before cloning the bounded complete tape.
        let mut frames = 0usize;
        for effect in block
            .into_iter()
            .flat_map(|entry| &entry.effects)
            .chain(pending.into_iter().flat_map(|entry| &entry.effects))
            .chain(&effects)
        {
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
        let mut combined = Vec::new();
        combined
            .try_reserve_exact(total)
            .map_err(|_| QuantityCaptureIssue::Capacity)?;
        combined.extend(block.into_iter().flat_map(|entry| &entry.effects).cloned());
        combined.extend(
            pending
                .into_iter()
                .flat_map(|entry| &entry.effects)
                .cloned(),
        );
        combined.extend(effects.iter().cloned());
        let candidate = FastpqExecutionEffectsV1 {
            context,
            effects: combined,
        };
        let bytes = norito::canonical_frame_len(&candidate)
            .map_err(|_| QuantityCaptureIssue::InvalidFacts)?;
        if u64::try_from(bytes).map_err(|_| QuantityCaptureIssue::Capacity)?
            > limit.max_statement_bytes
        {
            return Err(QuantityCaptureIssue::Capacity);
        }
        let parent = &*self.block_fastpq_quantity_candidate;
        let pending_archive = &self.pending_fastpq_quantity_candidate;
        let parent_entry = parent.measured_entries.get(&entry_hash);
        let pending_entry = pending_archive.measured_entries.get(&entry_hash);
        if block.is_some() != parent_entry.is_some()
            || pending.is_some() != pending_entry.is_some()
            || pending_archive
                .base_usage
                .is_some_and(|base| base != parent.usage)
        {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let baseline = parent_entry.map(|entry| entry.full).unwrap_or_default();
        if pending_entry.is_some_and(|entry| entry.baseline != baseline) {
            return Err(QuantityCaptureIssue::InvalidFacts);
        }
        let old_full = pending_entry.map(|entry| entry.full).unwrap_or(baseline);
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
        Ok(PreparedQuantityCapture {
            accounting,
            expected,
            context,
            effects,
        })
    }
    fn apply_with_quantity_candidate<T>(
        &mut self,
        prepared: Result<PreparedQuantityCapture, QuantityCaptureIssue>,
        apply: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let prepared = match prepared {
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
        self.world.quantity_mutation_observation.owned = true;
        let result = apply(self);
        self.world.quantity_mutation_observation.owned = previous_owned;
        if result.is_err() {
            // A caller may catch an error after partial writes. Such a candidate can never export.
            self.quantity_candidate_issue(QuantityCaptureIssue::InterruptedScope);
        }
        if result.is_ok() {
            if let Some(mut prepared) = prepared {
                let hash = prepared.context.entry.entry_hash;
                let current_full = self
                    .pending_fastpq_quantity_candidate
                    .measured_entries
                    .get(&hash)
                    .or_else(|| {
                        self.block_fastpq_quantity_candidate
                            .measured_entries
                            .get(&hash)
                    })
                    .map(|entry| entry.full)
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
                if !self.quantity_post_state_matches(&prepared.expected) {
                    self.quantity_candidate_issue(QuantityCaptureIssue::InvalidFacts);
                    return result;
                }
                let pending = &mut self.pending_fastpq_quantity_candidate;
                let entry =
                    pending
                        .entries
                        .entry(hash)
                        .or_insert_with(|| FastpqExecutionEffectsV1 {
                            context: prepared.context,
                            effects: Vec::new(),
                        });
                entry.effects.append(&mut prepared.effects);
                pending
                    .measured_entries
                    .insert(hash, prepared.accounting.measurement);
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
            let mut kinds = Vec::new();
            kinds
                .try_reserve_exact(legs.len())
                .map_err(|_| QuantityCaptureIssue::Capacity)?;
            for (source, destination, delta) in legs {
                if source.definition() != destination.definition()
                    || source.definition() != &delta.asset_definition
                    || source.account() != &delta.from_account
                    || destination.account() != &delta.to_account
                {
                    return Err(QuantityCaptureIssue::InvalidFacts);
                }
                kinds.push(FastpqExecutionEffectKindV1::Transfer(
                    FastpqExecutionTransferV1 {
                        source: self.quantity_balance_identity(source)?,
                        destination: self.quantity_balance_identity(destination)?,
                        amount: delta.amount.clone(),
                        source_before: delta.from_balance_before.clone(),
                        source_after: delta.from_balance_after.clone(),
                        destination_before: delta.to_balance_before.clone(),
                        destination_after: delta.to_balance_after.clone(),
                    },
                ));
            }
            self.prepare_quantity_candidate(authority, entry_hash, authorization_context, kinds)
        })();
        self.apply_with_quantity_candidate(prepared, apply)
    }
    /// Capture an already-authorized balance/supply mutation from exact live pre-state.
    pub(crate) fn apply_with_quantity_supply_candidate<T>(
        &mut self,
        authority: &AccountId,
        entry_hash: Hash,
        authorization_context: Hash,
        id: &AssetId,
        amount: &Quantity,
        mint: bool,
        apply: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let prepared = (|| {
            let balance = self.quantity_balance_identity(id)?;
            let balance_before = self
                .world
                .assets
                .get(id)
                .map(|value| value.as_ref().clone())
                .unwrap_or_else(Quantity::zero);
            let supply_before = self
                .world
                .asset_definition(id.definition())
                .map_err(|_| QuantityCaptureIssue::InvalidFacts)?
                .total_quantity()
                .clone();
            let balance_after = if mint {
                balance_before.try_add(amount)
            } else {
                balance_before.try_sub(amount)
            }
            .map_err(|_| QuantityCaptureIssue::InvalidFacts)?;
            let supply_after = if mint {
                supply_before.try_add(amount)
            } else {
                supply_before.try_sub(amount)
            }
            .map_err(|_| QuantityCaptureIssue::InvalidFacts)?;
            let change = FastpqExecutionSupplyChangeV1 {
                balance,
                amount: amount.clone(),
                balance_before,
                balance_after,
                supply_before,
                supply_after,
            };
            self.prepare_quantity_candidate(
                authority,
                entry_hash,
                authorization_context,
                vec![if mint {
                    FastpqExecutionEffectKindV1::Mint(change)
                } else {
                    FastpqExecutionEffectKindV1::Burn(change)
                }],
            )
        })();
        self.apply_with_quantity_candidate(prepared, apply)
    }
}

#[cfg(test)]
#[path = "fastpq_quantity_capture_tests.rs"]
mod tests;
