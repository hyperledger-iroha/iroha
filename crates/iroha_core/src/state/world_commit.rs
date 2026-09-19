//! Move-only preparation of the exact World overlay published by State commit.
//!
//! Late DA writes and lifecycle cleanup finish before the overlay is captured by
//! tiered persistence or consumed by publication. A rebuildable DA cache never
//! decides which authoritative World records exist. The prepared owner exposes
//! no mutable World access, so its baseline projection and its commit see the
//! same values. Existing State authorization and transaction gates still own
//! permission to publish; this object cannot authorize a carrier by itself.
//!
//! TODO: compose non-World membership/runtime/finality values, authenticated
//! predecessor restoration and aggregate resource admission into this owner
//! before it can publish a complete State commitment.

use super::world_projection::WorldStateBaseline;
use super::*;

/// One finalized World overlay and its derived DA cache publication records.
pub(in crate::state) struct PreparedWorldCommit<'state> {
    state: &'state State,
    world: WorldBlock<'state>,
    effects: PreparedWorldEffects,
}

/// Deferred cache records from the exact prepared World; no publication authority.
/// The consuming carrier owner retains these alongside its immutable World.
pub(in crate::state) struct PreparedWorldEffects {
    da_pins: Vec<DaPinIntentWithLocation>,
}

impl<'state> PreparedWorldCommit<'state> {
    /// Finish late deterministic writes before any live World/cache publication.
    pub(in crate::state) fn prepare(
        state: &'state State,
        mut world: WorldBlock<'state>,
        block_height: u64,
        nexus: &iroha_config::parameters::actual::Nexus,
        activation_heights: &BTreeMap<LaneId, u64>,
        pending_pins: Option<&PendingDaPinIntentBundle>,
        pending_lifecycle: Option<&PendingAutoscaleLaneLifecycle>,
    ) -> Result<Self, String> {
        let effects = Self::prepare_overlay(
            &mut world,
            block_height,
            nexus,
            activation_heights,
            pending_pins,
            pending_lifecycle,
        )?;
        Ok(Self {
            state,
            world,
            effects,
        })
    }

    /// Finish the same deterministic tail while the consuming carrier still
    /// owns every State journal. Its returned effects are retained by that
    /// read-only owner; they cannot publish or make an unprepared World valid.
    pub(in crate::state) fn prepare_overlay(
        world: &mut WorldBlock<'state>,
        block_height: u64,
        nexus: &iroha_config::parameters::actual::Nexus,
        activation_heights: &BTreeMap<LaneId, u64>,
        pending_pins: Option<&PendingDaPinIntentBundle>,
        pending_lifecycle: Option<&PendingAutoscaleLaneLifecycle>,
    ) -> Result<PreparedWorldEffects, String> {
        if pending_pins.is_some_and(|pending| pending.block_height != block_height) {
            return Err("prepared DA pin bundle belongs to a different block height".into());
        }
        let da_pins = match pending_pins {
            Some(pending) => {
                Self::prepare_pins(world, nexus, activation_heights, pending, pending_lifecycle)?
            }
            None => Vec::new(),
        };
        if let Some(pending) = pending_pins {
            for (key, value) in &pending.quota_writes {
                world
                    .smart_contract_state
                    .insert(key.clone(), value.clone());
            }
        }
        for record in &da_pins {
            Self::apply_pin(world, record);
        }
        if let Some(pending) = pending_lifecycle {
            State::prune_lane_lifecycle_world_block_state_for_lanes(
                world,
                &pending.catalog_update.lanes_to_reset,
            );
            Self::prune_emergency_validators(world, &pending.catalog_update);
        }
        Ok(PreparedWorldEffects { da_pins })
    }

    /// Read the completed overlay; no subsequent caller mutation is possible.
    pub(in crate::state) fn world(&self) -> &WorldBlock<'state> {
        &self.world
    }

    /// Derive the new private World baseline from this exact predecessor version.
    /// The complete State lifecycle must still establish predecessor identity.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "TODO: connect retained journals to the consuming State publisher"
        )
    )]
    pub(in crate::state) fn baseline_after(
        &self,
        parent: &WorldStateBaseline,
    ) -> Result<WorldStateBaseline, String> {
        parent.apply_block(&self.world)
    }

    /// Consume the same prepared World after State's publication gates succeed.
    pub(in crate::state) fn commit(self) {
        self.world.commit();
        self.state.publish_prepared_da_pins(self.effects.da_pins);
    }

    fn prepare_pins(
        world: &WorldBlock<'_>,
        nexus: &iroha_config::parameters::actual::Nexus,
        activation_heights: &BTreeMap<LaneId, u64>,
        pending: &PendingDaPinIntentBundle,
        lifecycle: Option<&PendingAutoscaleLaneLifecycle>,
    ) -> Result<Vec<DaPinIntentWithLocation>, String> {
        let effective_nexus = if let Some(lifecycle) = lifecycle {
            let mut updated = nexus.clone();
            updated.lane_catalog = lifecycle.catalog_update.updated_catalog.clone();
            updated.lane_config = lifecycle.catalog_update.updated_lane_config.clone();
            updated.dataspace_catalog = lifecycle.catalog_update.updated_dataspace_catalog.clone();
            std::borrow::Cow::Owned(updated)
        } else {
            std::borrow::Cow::Borrowed(nexus)
        };
        // Preserve positions in the authenticated canonical bundle even when a
        // later validation/reset filter removes an earlier element.
        let bundle =
            iroha_data_model::da::pin_intent::DaPinIntentBundle::new(pending.intents.clone());
        let positions = State::pin_intent_bundle_positions(&bundle.intents);
        let (intents, rejected) = crate::da::sanitize_pin_intents_against_nexus_at_height(
            bundle.intents,
            &effective_nexus,
            pending.block_height,
            |account| world.accounts.get(account).is_some(),
        );
        for reason in rejected {
            warn!(
                height = pending.block_height,
                ?reason,
                "dropping invalid DA pin intent during World preparation"
            );
        }
        // Use the applying State's captured incarnation boundary. Local DA
        // cursor journals may be stale or ahead and cannot veto a World write.
        let mut records = Vec::new();
        for intent in intents {
            let reset_by_transition = lifecycle.is_some_and(|pending_lifecycle| {
                pending_lifecycle
                    .catalog_update
                    .lanes_to_reset
                    .contains(&intent.lane_id)
            });
            if reset_by_transition
                || activation_heights
                    .get(&intent.lane_id)
                    .is_some_and(|height| pending.block_height <= *height)
            {
                continue;
            }
            let ticket = intent.storage_ticket;
            let manifest = intent.manifest_hash;
            let lane_epoch = (intent.lane_id, intent.epoch, intent.sequence);
            // The sanitizer already rejects duplicate identities within this bundle.
            // Only authoritative predecessor/overlay indexes decide remaining collisions.
            if world.da_pin_intents_by_ticket.get(&ticket).is_some()
                || world.da_pin_intents_by_manifest.get(&manifest).is_some()
                || world
                    .da_pin_intents_by_lane_epoch
                    .get(&lane_epoch)
                    .is_some()
            {
                continue;
            }
            let position = positions
                .get(&intent)
                .copied()
                .ok_or("prepared DA pin lost its canonical bundle position")?;
            let index_in_bundle = crate::da::da_bundle_location_index(position)
                .ok_or("prepared DA pin bundle location exceeds its wire range")?;
            let record = DaPinIntentWithLocation {
                intent,
                location: DaCommitmentLocation {
                    block_height: pending.block_height,
                    index_in_bundle,
                },
            };
            // Locations are unique within the one canonical bundle. Existing
            // records belong to authenticated predecessor heights; replacement
            // starts from MV undo before preparing this height's bundle.
            records.push(record);
        }
        Ok(records)
    }

    fn apply_pin(world: &mut WorldBlock<'_>, record: &DaPinIntentWithLocation) {
        let intent = &record.intent;
        let ticket = intent.storage_ticket;
        if let Some(alias) = &intent.alias {
            world.da_pin_intents_by_alias.insert(alias.clone(), ticket);
        }
        world
            .da_pin_intents_by_ticket
            .insert(ticket, record.clone());
        world
            .da_pin_intents_by_manifest
            .insert(intent.manifest_hash, ticket);
        world
            .da_pin_intents_by_lane_epoch
            .insert((intent.lane_id, intent.epoch, intent.sequence), ticket);
    }

    pub(in crate::state) fn prune_emergency_validators(
        world: &mut WorldBlock<'_>,
        update: &LaneLifecycleCatalogUpdate,
    ) {
        let active: BTreeSet<_> = update
            .updated_catalog
            .lanes()
            .iter()
            .map(|lane| lane.id)
            .collect();
        let stale: Vec<_> = world
            .lane_relay_emergency_validators
            .iter()
            .filter_map(|(lane, _)| {
                (update.lanes_to_reset.contains(lane) || !active.contains(lane)).then_some(*lane)
            })
            .collect();
        for lane in stale {
            world.lane_relay_emergency_validators.remove(lane);
        }
    }
}

impl StateBlock<'_> {
    /// Project the four persisted pin indexes through the same pure admission plan as commit.
    pub(crate) fn json_serialize_committed_da_pin_indexes(&self) -> [Option<String>; 4] {
        if self.pending_da_pin_intents.is_none() && self.pending_autoscale_lifecycle.is_none() {
            return Default::default();
        }
        let records = self
            .pending_da_pin_intents
            .as_ref()
            .map(|pending| {
                PreparedWorldCommit::prepare_pins(
                    &self.world,
                    &self.nexus,
                    &self.lane_incarnation_activation_heights,
                    pending,
                    self.pending_autoscale_lifecycle.as_ref(),
                )
                .expect("validated pin bundle must retain representable canonical locations")
            })
            .unwrap_or_default();
        let mut tickets = BTreeMap::new();
        let mut aliases = BTreeMap::new();
        let mut manifests = BTreeMap::new();
        let mut lane_epochs = BTreeMap::new();
        if let Some(lifecycle) = &self.pending_autoscale_lifecycle {
            let stale = State::da_pin_intent_index_prune_keys_for_lanes(
                &self.world.da_pin_intents_by_ticket,
                &self.world.da_pin_intents_by_alias,
                &self.world.da_pin_intents_by_manifest,
                &self.world.da_pin_intents_by_lane_epoch,
                &lifecycle.catalog_update.lanes_to_reset,
            );
            tickets.extend(stale.tickets.into_iter().map(|key| (key, None)));
            aliases.extend(stale.aliases.into_iter().map(|key| (key, None)));
            manifests.extend(stale.manifests.into_iter().map(|key| (key, None)));
            lane_epochs.extend(stale.lane_epochs.into_iter().map(|key| (key, None)));
        }
        for record in records {
            let intent = &record.intent;
            let ticket = intent.storage_ticket;
            if let Some(alias) = &intent.alias {
                aliases.insert(alias.clone(), Some(ticket));
            }
            manifests.insert(intent.manifest_hash, Some(ticket));
            lane_epochs.insert(
                (intent.lane_id, intent.epoch, intent.sequence),
                Some(ticket),
            );
            tickets.insert(ticket, Some(record));
        }
        macro_rules! project {
            ($store:ident, $changes:ident) => {{
                if $changes.is_empty() {
                    None
                } else {
                    let mut out = String::new();
                    mv::json::json_serialize_storage_block_with_changes(
                        &self.world.$store,
                        &$changes,
                        &mut out,
                    );
                    Some(out)
                }
            }};
        }
        [
            project!(da_pin_intents_by_ticket, tickets),
            project!(da_pin_intents_by_alias, aliases),
            project!(da_pin_intents_by_manifest, manifests),
            project!(da_pin_intents_by_lane_epoch, lane_epochs),
        ]
    }
}

impl State {
    /// Reconstruct the derived pin cache, preserving explicitly unbound aliases.
    pub(in crate::state) fn da_pin_cache_from_world(&self) -> DaPinStore {
        let by_ticket = self.world.da_pin_intents_by_ticket.view();
        let mut canonical: Vec<_> = by_ticket.iter().map(|(_, value)| value.clone()).collect();
        canonical
            .sort_by_key(|entry| (entry.location.block_height, entry.location.index_in_bundle));
        let mut cache = DaPinStore::from_intents(&canonical);
        let aliases = self.world.da_pin_intents_by_alias.view();
        cache.replace_alias_bindings(
            aliases
                .iter()
                .map(|(alias, ticket)| (alias.clone(), *ticket))
                .collect(),
        );
        cache
    }

    /// Materialize a derived cache only after its authoritative World is visible.
    /// A cache collision triggers reconstruction; it can never veto a World write.
    fn publish_prepared_da_pins(&self, records: Vec<DaPinIntentWithLocation>) {
        if records.is_empty() {
            return;
        }
        let mut cache = self.da_pin_intents.write();
        for record in records {
            if !cache.insert_with_location(record) {
                // Recovery from a stale/ahead cache is deliberately a cold path.
                // Ordinary publication only inserts its bounded prepared records.
                // TODO: admit cold reconstruction memory with the complete State
                // resource owner before qualifying restart/publication budgets.
                *cache = self.da_pin_cache_from_world();
                break;
            }
        }
    }
}

#[cfg(test)]
#[path = "world_commit_tests.rs"]
mod tests;
