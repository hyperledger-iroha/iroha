//! Cold complete element publication at one original native applied cut.
//!
//! Normal block execution retains only the incremental accumulator. This
//! on-demand path borrows target values from the same locked World overlay and
//! funds every retained snapshot entry with the original finite operation pool.

use super::*;
use crate::{
    state::{State, StateReadOnly, StateView, is_stable_state_view_generation},
    sumeragi::certified_chain::CommittedBlock,
};
use iroha_allocation::{AllocationBudget, AllocationCharge, ChargedBuffer};
use iroha_data_model::{
    asset::{AssetDefinition, AssetDefinitionId},
    kagemusha::KagemushaGovernedVerifierRegistryV1,
    nexus::AxtAssetIncarnationV1,
    sumeragi_finality::{
        MAX_WORLD_STATE_SNAPSHOT_BYTES_V1, MAX_WORLD_STATE_SNAPSHOT_ENTRIES_V1,
        WorldStateSnapshotEntryV1, WorldStateSnapshotV1,
    },
};
use std::alloc::Layout;

/// Private move-only snapshot owner; payload fields drop before their exact charges.
pub(super) struct SnapshotCollector<'a> {
    entries: Vec<WorldStateSnapshotEntryV1>,
    charges: ChargedBuffer<AllocationCharge>,
    budget: &'a AllocationBudget,
    expected: usize,
}

impl<'a> SnapshotCollector<'a> {
    fn new(budget: &'a AllocationBudget, expected: usize, fields: usize) -> Result<Self, String> {
        if expected > MAX_WORLD_STATE_SNAPSHOT_ENTRIES_V1 {
            return Err("World snapshot exceeds its complete-entry bound".into());
        }
        let mut charges = ChargedBuffer::new(
            expected
                .checked_add(3)
                .ok_or("World snapshot allocation ledger overflows")?,
            budget,
        )
        .map_err(|error| error.to_string())?;
        let layouts = [
            Layout::array::<WorldStateSnapshotEntryV1>(expected)
                .map_err(|error| error.to_string())?,
            Layout::array::<bool>(fields).map_err(|error| error.to_string())?,
            Layout::new::<[u16; LANES]>(),
        ];
        let mut reservation = budget
            .try_reserve_layouts(layouts.iter().copied())
            .map_err(|error| error.to_string())?;
        for layout in layouts {
            charges.push_reserved(
                reservation
                    .try_split(layout)
                    .map_err(|error| error.to_string())?,
            );
        }
        Ok(Self {
            entries: Vec::with_capacity(expected),
            charges,
            budget,
            expected,
        })
    }

    pub(super) fn push(
        &mut self,
        id: &str,
        kind: WorldStateElementKindV1,
        key_hash: Option<Hash>,
        value_hash: Hash,
    ) -> Result<(), String> {
        if self.entries.len() >= self.expected {
            return Err("World snapshot differs from the original stored entry count".into());
        }
        // String::from(str) owns one exact byte allocation; admit it before copying.
        let layout = Layout::array::<u8>(id.len()).map_err(|error| error.to_string())?;
        let charge = self
            .budget
            .try_reserve(layout)
            .map_err(|error| error.to_string())?
            .try_split(layout)
            .map_err(|error| error.to_string())?;
        let field_id = id.to_owned();
        self.charges.push_reserved(charge);
        self.entries.push(WorldStateSnapshotEntryV1 {
            field_id,
            kind,
            key_hash,
            value_hash,
        });
        Ok(())
    }

    fn finish(mut self, schema_hash: Hash) -> Result<CapturedSnapshot, String> {
        if self.entries.len() != self.expected {
            return Err("World snapshot omits original canonical entries".into());
        }
        self.entries.sort_unstable_by(|a, b| {
            (&a.field_id, a.kind, a.key_hash).cmp(&(&b.field_id, b.kind, b.key_hash))
        });
        if self.entries.windows(2).any(|rows| {
            (&rows[0].field_id, rows[0].kind, rows[0].key_hash)
                == (&rows[1].field_id, rows[1].kind, rows[1].key_hash)
        }) {
            return Err("World snapshot repeats a canonical element identity".into());
        }
        let snapshot = WorldStateSnapshotV1 {
            schema_hash,
            entries: self.entries,
        };
        if norito::canonical_frame_len(&snapshot).map_err(|error| error.to_string())?
            > MAX_WORLD_STATE_SNAPSHOT_BYTES_V1
        {
            return Err("World snapshot exceeds its original canonical byte bound".into());
        }
        Ok(CapturedSnapshot {
            snapshot,
            _charges: self.charges,
        })
    }
}

struct CapturedSnapshot {
    snapshot: WorldStateSnapshotV1,
    _charges: ChargedBuffer<AllocationCharge>,
}

fn capture(
    world: &WorldBlock<'_>,
    expected: &WorldStateAccumulator,
    budget: &AllocationBudget,
) -> Result<CapturedSnapshot, String> {
    let index = field_index().as_ref().map_err(Clone::clone)?;
    let count = usize::try_from(expected.entries()).map_err(|error| error.to_string())?;
    let snapshot = SnapshotCollector::new(budget, count, index.canonical)?;
    let mut builder = Builder {
        index,
        accumulator: WorldStateAccumulator::empty(),
        direction: Direction::Capture,
        visited: vec![false; index.canonical],
        snapshot: Some(snapshot),
        snapshot_field: None,
    };
    world.project_world(&mut builder)?;
    if builder.visited.iter().any(|visited| !visited) {
        return Err("World snapshot omits a canonical registry field".into());
    }
    if builder.accumulator != *expected {
        return Err("World snapshot does not match the complete stored World accumulator".into());
    }
    builder
        .snapshot
        .take()
        .ok_or("World snapshot collector is absent")?
        .finish(index.schema)
}

fn require_cut(view: &StateView<'_>, tip: &CommittedBlock) -> Result<(), String> {
    let native = view
        .native_execution_tip()
        .ok_or("World snapshot has no original native execution tip")?;
    if tip.height() < 2
        || tip.header().is_none()
        || u64::try_from(view.height()).ok() != Some(tip.height())
        || view.latest_block_hash() != Some(tip.block_hash())
        || native.height() != tip.height()
        || native.iroha_hash() != tip.block_hash()
        || native.core_hash() != tip.core_hash()
        || native.result() != tip.result()
        || native.creation_time_ms() != tip.block_time_ms()
        || tip.commitment().height != tip.height()
        || view.world.state_accumulator.get().root()? != tip.commitment().execution.world_state_root
    {
        return Err(
            "World snapshot certified execution differs from the current applied cut".into(),
        );
    }
    Ok(())
}

impl State {
    /// Publish every canonical World element and borrowed exact target originals on demand.
    ///
    /// `tip` must come from the retained native certified chain. This method checks
    /// its original header/result against State's opaque execution owner before and
    /// after capture; no supplied bare root grants authority. The callback may only
    /// produce a data response, which its independent consumer still authenticates
    /// under an installed finality root. It must not publish side effects.
    ///
    /// # Errors
    /// Busy or changed publication generation, foreign/retired native tip, missing
    /// exact targets, inconsistent accumulator, finite allocation or wire bounds.
    pub fn with_native_world_state_snapshot_v1<T>(
        &self,
        tip: &CommittedBlock,
        asset_id: &AssetDefinitionId,
        budget: &AllocationBudget,
        consume: impl FnOnce(
            &WorldStateSnapshotV1,
            &AssetDefinition,
            &AxtAssetIncarnationV1,
            &KagemushaGovernedVerifierRegistryV1,
        ) -> Result<T, String>,
    ) -> Result<T, String> {
        let generation = self.state_view_generation();
        if generation % 2 != 0 {
            return Err("World snapshot publication is busy".into());
        }
        if tip.commitment().schedule.current.network_id != *self.network_id_ref() {
            return Err("World snapshot native tip belongs to another network".into());
        }
        {
            let view = self
                .try_view_once()
                .map_err(|error| error.to_string())?
                .ok_or("World snapshot publication is busy or changed")?;
            require_cut(&view, tip)?;
        }
        let result = {
            // Acquire only storage overlays, under the caller's original pool.
            // State::block would also initialize height-bound execution state.
            let world = self
                .world
                .try_block(budget)
                .map_err(|error| error.to_string())?;
            let expected = world.state_accumulator.get();
            if expected.root()? != tip.commitment().execution.world_state_root {
                return Err("World snapshot acquired another applied World".into());
            }
            let definition = world
                .asset_definitions
                .get(asset_id)
                .ok_or("World snapshot exact asset definition is absent")?;
            let incarnation = world
                .axt_asset_incarnations
                .get(asset_id)
                .ok_or("World snapshot exact asset incarnation is absent")?;
            let captured = capture(&world, expected, budget)?;
            consume(
                &captured.snapshot,
                definition,
                incarnation,
                world.kagemusha_verifier_registry.get(),
            )
        };
        let view = self
            .try_view_once()
            .map_err(|error| error.to_string())?
            .ok_or("World snapshot publication is busy or changed")?;
        require_cut(&view, tip)?;
        if !is_stable_state_view_generation(generation, self.state_view_generation()) {
            return Err("World snapshot publication generation changed".into());
        }
        result
    }
}

#[cfg(test)]
#[path = "world_state_snapshot_tests.rs"]
mod tests;
