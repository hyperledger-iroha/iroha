//! Complete World state accumulator: the World state roots of the execution result `R`
//! (`specs/sumeragi.md` §4.1, Appendix E, E51).
//!
//! The accumulator is an incremental homomorphic multiset hash (`LtHash16`) over every canonical
//! World entry that the exhaustive authority registry declares, including the trigger owner's
//! authoritative stores. Derived indexes and local delivery buffers are excluded by the same
//! registry, so the accumulator commits to exactly the independent World authority.
//!
//! - **Element.** A table entry `(k, v)` of field `f`, or the value `v` of cell `f`, maps to
//!   `e = BLAKE3-XOF(derive_key(ELEMENT_CONTEXT), P_f ‖ presence ‖ K ‖ V)` truncated to
//!   [`LANES`] little-endian `u16` lanes. `P_f` hashes the registry identity and kind of `f`,
//!   `K` is the bare Norito value hash of `k` (32 zero bytes and presence 0 for a cell) and `V`
//!   is the field owner's canonical value projection (bare Norito, or the registry's declared
//!   semantic projection that excludes runtime caches).
//! - **Accumulator.** The lane-wise sum of the elements of all entries modulo `2^16`, with the
//!   entry count modulo `2^64`. Adding and removing an element are inverse operations and the
//!   sum is independent of entry order, so the value is a function of the World content alone.
//! - **Root.** `H(ROOT ‖ S ‖ entries ‖ lanes)`, where `H` is the chain hash and `S` binds the
//!   commitment profile and every storage definition the root's selection installs, in identity
//!   order (`specs/live_runtime_upgrades.md` §5.1). A storage definition fixes its field's
//!   identity, kind and key/value schema.
//! - **Installed storage.** A canonical field compiled into this release whose storage
//!   definition the selection does not install is empty, unwritten and outside the commitment,
//!   so a release that adds a field changes no root until a change set installs it. A block
//!   that installs a field absorbs its complete content; its parent root keeps the parent
//!   selection's `S`, its result root takes the successor's.
//!
//! A block updates the accumulator from its complete change set: every entry of every canonical
//! World storage and cell that the block's overlay touched, from the overlay's own undo journal
//! (`before`) and current value (`after`). The block subtracts `e(before)` and adds `e(after)`;
//! unchanged touches cancel and are skipped. The cost is proportional to the change set, never
//! to the size of the World. A cold capture visits every canonical value once (`O(N)`); it seeds
//! genesis and checks the incremental value at startup.
//!
//! Every pass visits exactly the registry's canonical fields: an unclassified, mistyped,
//! repeated or missing field fails the pass, so a new World field cannot silently escape the
//! commitment (the registry itself destructures `WorldData` without `..`). Every canonical field
//! implements one storage definition of the release catalog (`crate::runtime::catalog`).
//!
//! All arithmetic is wrapping integer arithmetic on fixed-width lanes; the element expansion is
//! BLAKE3, whose SIMD back ends are bit-identical to its portable implementation. The lane
//! loops are plain scalar code that the compiler may vectorize; the result is identical on
//! every platform.

use std::{collections::BTreeMap, sync::OnceLock};

use super::{CellField, Hash, StorageField, WorldBlock, WorldProjection, hash_value};
use crate::state::authority_registry::{Canonical, Field, Role, Schema, WORLD_FIELDS};
use iroha_data_model::musubi::{
    ArchiveId, MusubiArchiveAvailabilityV1, MusubiOrderedPackageEntryV1, MusubiPackageSelectorV1,
    MusubiReleaseIdV1, MusubiResolverReleaseRowV1,
};
use iroha_data_model::runtime::definition::RootSelectionV1;
use iroha_data_model::sumeragi_finality::{
    WorldStateElementKindV1, world_state_element_v1, world_state_path_hash_v1,
    world_state_root_from_accumulator_v1,
};
use mv::{Key, Value, storage::StorageReadOnly};
use norito::codec::Encode;

/// Number of 16-bit lanes of the accumulator (`LtHash16`, 2 KiB of state).
pub const LANES: usize = 1024;
const LANE_BYTES: usize = 2 * LANES;
const SCHEMA_START: &[u8] = b"iroha:world-state:schema:start:v1\0";
const SCHEMA_FIELD: &[u8] = b"iroha:world-state:schema:field:v1\0";
const SCHEMA_VALUE: &[u8] = b"iroha:world-state:schema:value:v1\0";
const SCHEMA_NO_KEY: &[u8] = b"iroha:world-state:schema:no-key:v1\0";
const SCHEMA_SELECTION: &[u8] = b"iroha:world-state:schema:selection:v1\0";
const PROFILE: &[u8] = b"iroha:world-state:profile:lthash16:v1\0";
const TABLE: u8 = 0;
const CELL: u8 = 1;

/// Homomorphic multiset hash of the complete canonical World.
///
/// The default value is the empty World (no entries). The root, not the lanes, is what `R`
/// commits; the lanes are the incremental state needed to update it. The lanes live on the
/// heap, so the World overlays that embed this cell (and its per-transaction undo slot) stay
/// small.
#[derive(Clone, PartialEq, Eq)]
pub struct WorldStateAccumulator {
    lanes: Box<[u16; LANES]>,
    entries: u64,
}

impl Default for WorldStateAccumulator {
    fn default() -> Self {
        Self {
            lanes: Box::new([0; LANES]),
            entries: 0,
        }
    }
}

impl core::fmt::Debug for WorldStateAccumulator {
    fn fmt(&self, out: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The root depends on the selection whose schema it commits, so only the count shows.
        out.debug_struct("WorldStateAccumulator")
            .field("entries", &self.entries)
            .finish_non_exhaustive()
    }
}

/// Which values of each touched entry a pass removes and adds.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Add every current value (a cold capture).
    Capture,
    /// Remove each block preimage, add each current value.
    Forward,
    /// Remove each current value, add each block preimage.
    #[cfg(test)]
    Reverse,
}

impl WorldStateAccumulator {
    /// The empty World (no canonical entries).
    #[must_use]
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// Number of canonical entries (modulo `2^64`).
    #[must_use]
    pub(crate) fn entries(&self) -> u64 {
        self.entries
    }

    /// The complete World state root under `selection`: the value `R` binds (§4.1).
    ///
    /// # Errors
    /// This release does not implement the selection's commitment profile.
    pub(crate) fn root(&self, selection: &RootSelectionV1) -> Result<Hash, String> {
        Ok(world_state_root_from_accumulator_v1(
            selection_schema(selection)?,
            self.entries,
            &self.lanes,
        ))
    }

    /// Cold capture of every canonical value of the overlay's current World, in `O(N)`.
    ///
    /// # Errors
    /// A field is unclassified, mistyped, repeated or missing, or a value fails to encode.
    pub(in crate::state) fn capture(world: &WorldBlock<'_>) -> Result<Self, String> {
        Builder::run(Self::empty(), world, Direction::Capture)
    }

    /// Cold capture of the overlay's exact World predecessor (the state before this block).
    ///
    /// # Errors
    /// See [`Self::capture`].
    #[cfg(test)]
    pub(in crate::state) fn capture_predecessor(world: &WorldBlock<'_>) -> Result<Self, String> {
        Builder::run(Self::capture(world)?, world, Direction::Reverse)
    }

    /// Apply the block's complete change set to this accumulator of its predecessor.
    ///
    /// Only touched entries are encoded. The caller guarantees that `self` belongs to the
    /// overlay's exact predecessor; a foreign predecessor yields a root that differs from the
    /// certified one (the multiset hash cannot detect it locally).
    ///
    /// # Errors
    /// See [`Self::capture`]; `self` is left unchanged.
    pub(in crate::state) fn apply_block(&self, world: &WorldBlock<'_>) -> Result<Self, String> {
        Builder::run(self.clone(), world, Direction::Forward)
    }

    /// Add one element: lane-wise wrapping addition.
    fn add(&mut self, element: &[u8; LANE_BYTES]) {
        for (lane, bytes) in self.lanes.iter_mut().zip(element.chunks_exact(2)) {
            *lane = lane.wrapping_add(u16::from_le_bytes([bytes[0], bytes[1]]));
        }
        self.entries = self.entries.wrapping_add(1);
    }

    /// Remove one element: the exact inverse of [`Self::add`].
    fn remove(&mut self, element: &[u8; LANE_BYTES]) {
        for (lane, bytes) in self.lanes.iter_mut().zip(element.chunks_exact(2)) {
            *lane = lane.wrapping_sub(u16::from_le_bytes([bytes[0], bytes[1]]));
        }
        self.entries = self.entries.wrapping_sub(1);
    }
}

/// Canonical bare payload: the fixed-width little-endian entry count and lanes
/// ([`WorldStateAccumulator::PAYLOAD_BYTES`] bytes, independent of layout flags).
impl norito::core::SerializePayload for WorldStateAccumulator {
    fn serialize(&self, out: &mut norito::core::Encoder<'_>) -> Result<(), norito::Error> {
        out.write_all(&self.payload())?;
        Ok(())
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(Self::PAYLOAD_BYTES)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(Self::PAYLOAD_BYTES)
    }
}

impl WorldStateAccumulator {
    /// Length of the canonical payload: the entry count and every lane.
    const PAYLOAD_BYTES: usize = 8 + LANE_BYTES;
    const JSON_HEX: usize = 2 * Self::PAYLOAD_BYTES;

    fn payload(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::PAYLOAD_BYTES);
        bytes.extend_from_slice(&self.entries.to_le_bytes());
        for lane in self.lanes.iter() {
            bytes.extend_from_slice(&lane.to_le_bytes());
        }
        bytes
    }

    fn json_hex(&self) -> String {
        hex::encode(self.payload())
    }

    fn from_json_hex(text: &str) -> Result<Self, String> {
        if text.len() != Self::JSON_HEX {
            return Err(format!(
                "World state accumulator must be {} hex characters, got {}",
                Self::JSON_HEX,
                text.len()
            ));
        }
        let bytes = hex::decode(text).map_err(|error| error.to_string())?;
        let (entries, lanes) = bytes.split_at(8);
        let entries: [u8; 8] = entries
            .try_into()
            .map_err(|_| "World state accumulator entry count is not eight bytes")?;
        let mut value = Self {
            lanes: Box::new([0; LANES]),
            entries: u64::from_le_bytes(entries),
        };
        for (lane, pair) in value.lanes.iter_mut().zip(lanes.chunks_exact(2)) {
            *lane = u16::from_le_bytes([pair[0], pair[1]]);
        }
        Ok(value)
    }
}

/// Snapshot form: one lowercase hex string of the entry count and lanes (little endian).
impl norito::json::JsonSerialize for WorldStateAccumulator {
    fn json_serialize(&self, out: &mut String) {
        out.push('"');
        out.push_str(&self.json_hex());
        out.push('"');
    }

    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        out.push('"')?;
        out.push_str(&self.json_hex())?;
        out.push('"')
    }
}

impl norito::json::JsonDeserialize for WorldStateAccumulator {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let text = <String as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        Self::from_json_hex(&text).map_err(norito::json::Error::Message)
    }
}

/// Registry classification of one projected field.
#[derive(Clone, Copy)]
enum Classified {
    /// Independent authority: its path hash, kind and position among the canonical fields.
    Canonical { path: Hash, kind: u8, slot: usize },
    /// Derived or local: excluded from the accumulator.
    Excluded,
}

/// The registry, indexed by projection name.
///
/// The storage definition each canonical field implements comes from the release catalog at the
/// time of use ([`installed_slots`]), never from this process-wide index: the release, not the
/// compiled registry, decides which definition a field implements (an addition after the baseline
/// publishes a field's storage definition without installing it).
struct FieldIndex {
    by_name: BTreeMap<&'static str, Classified>,
    /// Canonical registry identities in slot order.
    ids: Vec<&'static str>,
    canonical: usize,
    /// Kind and key/value schema fingerprints of every canonical field, by registry identity:
    /// the predecessor root rule of an upgrade checkpoint.
    fingerprints: BTreeMap<&'static str, Fingerprint>,
}

fn field_index() -> &'static Result<FieldIndex, String> {
    static INDEX: OnceLock<Result<FieldIndex, String>> = OnceLock::new();
    INDEX.get_or_init(|| FieldIndex::build(WORLD_FIELDS))
}

impl FieldIndex {
    fn build(fields: &'static [Field]) -> Result<Self, String> {
        let mut flat = Vec::new();
        flatten(fields, &mut flat);
        let mut canonical = BTreeMap::new();
        let mut by_name = BTreeMap::new();
        let mut names = std::collections::BTreeSet::new();
        for field in flat {
            // World fields project under their bare name; owner children keep their id.
            let name = field.id.strip_prefix("world.").unwrap_or(field.id);
            match field.role {
                Role::Canonical(Canonical::Owner(_)) => continue,
                Role::Canonical(Canonical::Table { key, value }) => {
                    canonical.insert(field.id, (TABLE, Some(key), value));
                }
                Role::Canonical(Canonical::Cell(value)) => {
                    canonical.insert(field.id, (CELL, None, value));
                }
                Role::Derived { .. } | Role::Local(_) => {
                    by_name.insert(name, Classified::Excluded);
                }
                Role::History { .. } => {
                    return Err(format!(
                        "historical World field has no state accumulator owner: {}",
                        field.id
                    ));
                }
            }
            if !names.insert(name) {
                return Err(format!("World field registered twice: {}", field.id));
            }
        }
        let mut fingerprints = BTreeMap::new();
        for (slot, (id, (kind, key, value))) in canonical.iter().enumerate() {
            let path = path_hash(id, *kind)?;
            fingerprints.insert(
                *id,
                (
                    *kind,
                    key.map(schema_fingerprint).transpose()?,
                    schema_fingerprint(*value)?,
                ),
            );
            let name = id.strip_prefix("world.").unwrap_or(id);
            by_name.insert(
                name,
                Classified::Canonical {
                    path,
                    kind: *kind,
                    slot,
                },
            );
        }
        Ok(Self {
            by_name,
            ids: canonical.keys().copied().collect(),
            canonical: canonical.len(),
            fingerprints,
        })
    }
}

/// Kind and key/value schema fingerprints of one canonical field (no key for a cell).
type Fingerprint = (u8, Option<Hash>, Hash);

/// Digest of the fixed parameters of this release's commitment profile: the lane count of the
/// `LtHash16` accumulator whose element, path and root rules
/// (`iroha_data_model::sumeragi_finality`) the profile's scheme names.
pub(crate) fn commitment_parameters() -> [u8; 32] {
    Hash::new_from_chunks(&[PROFILE, &(LANES as u64).to_le_bytes()]).into()
}

/// `S` of a selection: its commitment profile, then every storage definition it installs in
/// identity order, then their count.
///
/// # Errors
/// This release does not implement the selection's commitment profile.
pub(crate) fn selection_schema(selection: &RootSelectionV1) -> Result<Hash, String> {
    let catalog = crate::runtime::release_catalog().map_err(|error| error.to_string())?;
    if selection.commitment_profile != catalog.baseline.commitment_profile {
        return Err(format!(
            "commitment profile {} is not implemented by this release",
            selection.commitment_profile
        ));
    }
    let mut schema = Hash::new_from_chunks(&[SCHEMA_SELECTION, &selection.commitment_profile.0]);
    for storage in &selection.installed_storage {
        schema = Hash::new_from_chunks(&[SCHEMA_FIELD, schema.as_ref(), &storage.0]);
    }
    let count = u64::try_from(selection.installed_storage.len())
        .map_err(|_| "installed storage count exceeds u64")?;
    Ok(Hash::new_from_chunks(&[
        SCHEMA_SELECTION,
        schema.as_ref(),
        &count.to_le_bytes(),
    ]))
}

/// Whether each canonical slot's storage is installed by `selection`.
///
/// # Errors
/// A canonical field has no storage definition in the release catalog, or the selection installs
/// storage no canonical field of this release implements.
fn installed_slots(index: &FieldIndex, selection: &RootSelectionV1) -> Result<Vec<bool>, String> {
    let catalog = crate::runtime::release_catalog().map_err(|error| error.to_string())?;
    let installed = index
        .ids
        .iter()
        .map(|id| {
            catalog
                .world_storage
                .get(id)
                .map(|storage| selection.installed_storage.binary_search(storage).is_ok())
                .ok_or_else(|| format!("canonical World field {id} has no storage definition"))
        })
        .collect::<Result<Vec<bool>, String>>()?;
    if installed.iter().filter(|installed| **installed).count() != selection.installed_storage.len()
    {
        return Err("the selection installs storage this release does not implement".into());
    }
    Ok(installed)
}

/// `S`: every canonical field's identity, kind and key/value schema folded in identity order,
/// followed by the field count.
fn schema_digest(fingerprints: &BTreeMap<&'static str, Fingerprint>) -> Result<Hash, String> {
    let mut schema = Hash::new(SCHEMA_START);
    for (id, (kind, key, value)) in fingerprints {
        let path = path_hash(id, *kind)?;
        let key = key.unwrap_or_else(|| Hash::new(SCHEMA_NO_KEY));
        schema = Hash::new_from_chunks(&[
            SCHEMA_FIELD,
            schema.as_ref(),
            path.as_ref(),
            key.as_ref(),
            value.as_ref(),
        ]);
    }
    Ok(Hash::new_from_chunks(&[
        SCHEMA_START,
        schema.as_ref(),
        &u64::try_from(fingerprints.len())
            .map_err(|_| "canonical World field count exceeds u64")?
            .to_le_bytes(),
    ]))
}

/// The accumulator and World root that the predecessor release of an upgrade checkpoint
/// certified for an imported World (`state::upgrade_checkpoint`).
///
/// `current` is a cold capture, under this release's registry, of a World imported from the
/// predecessor's snapshot with the added tables still empty and the runtime fields founded on
/// the baseline. The predecessor's registry held the retired tables instead and none of the
/// founded fields. An import admits only the retired tables' empty values, which add no
/// element; the founded fields' baseline elements are removed again.
///
/// # Errors
/// The registry of this release no longer matches the predecessor profile, or a value fails to
/// encode.
pub(in crate::state) fn predecessor_root(
    current: &WorldStateAccumulator,
) -> Result<(WorldStateAccumulator, Hash), String> {
    use crate::state::upgrade_checkpoint::{
        ADDED_WORLD_FIELDS, FOUNDED_WORLD_FIELDS, retired_world_fields,
    };
    let mut fingerprints = field_index()
        .as_ref()
        .map_err(Clone::clone)?
        .fingerprints
        .clone();
    for added in &ADDED_WORLD_FIELDS {
        if fingerprints.remove(added.id).is_none() {
            return Err(format!(
                "{} is not a canonical World field of this release",
                added.id
            ));
        }
    }
    let mut predecessor = current.clone();
    for (id, _) in &FOUNDED_WORLD_FIELDS {
        if fingerprints.remove(id).is_none() {
            return Err(format!(
                "{id} is not a canonical World field of this release"
            ));
        }
    }
    let catalog = crate::runtime::release_catalog().map_err(|error| error.to_string())?;
    let definitions = path_hash(FOUNDED_WORLD_FIELDS[0].0, TABLE)?;
    for id in &catalog.baseline_definitions {
        let descriptor = catalog
            .definitions
            .get(id)
            .ok_or("the baseline names an unknown definition")?;
        predecessor.remove(&element(
            &definitions,
            Some(&hash_value(id)?),
            &hash_value(descriptor)?,
        ));
    }
    predecessor.remove(&element(
        &path_hash(FOUNDED_WORLD_FIELDS[1].0, CELL)?,
        None,
        &hash_value(&catalog.baseline)?,
    ));
    for retired in retired_world_fields() {
        let kind = if retired.key.is_some() { TABLE } else { CELL };
        let key = retired.key.map(norito_identity_fingerprint).transpose()?;
        let value = norito_identity_fingerprint(retired.value)?;
        if kind == CELL {
            // An empty cell still holds one element, whose value this release can no longer type.
            return Err(format!(
                "{} is a retired cell, which no import admits",
                retired.id
            ));
        }
        if fingerprints
            .insert(retired.id, (kind, key, value))
            .is_some()
        {
            return Err(format!(
                "{} is still a canonical World field of this release",
                retired.id
            ));
        }
    }
    let root = world_state_root_from_accumulator_v1(
        schema_digest(&fingerprints)?,
        predecessor.entries,
        &predecessor.lanes,
    );
    Ok((predecessor, root))
}

/// [`schema_fingerprint`] of a Norito schema whose nominal identity was `identity`.
fn norito_identity_fingerprint(identity: &str) -> Result<Hash, String> {
    let layout = crate::state::authority_registry::V1_LAYOUT;
    let length = u64::try_from(identity.len()).map_err(|_| "World schema identity exceeds u64")?;
    Ok(Hash::new_from_chunks(&[
        SCHEMA_VALUE,
        &[0, layout.major, layout.minor, layout.flags],
        &length.to_le_bytes(),
        identity.as_bytes(),
    ]))
}

fn flatten(fields: &'static [Field], out: &mut Vec<&'static Field>) {
    for field in fields {
        out.push(field);
        if let Role::Canonical(Canonical::Owner(children)) = field.role {
            flatten(children, out);
        }
    }
}

fn path_hash(id: &str, kind: u8) -> Result<Hash, String> {
    world_state_path_hash_v1(id, element_kind(kind)?).map_err(|error| error.to_string())
}

fn element_kind(kind: u8) -> Result<WorldStateElementKindV1, String> {
    match kind {
        TABLE => Ok(WorldStateElementKindV1::Table),
        CELL => Ok(WorldStateElementKindV1::Cell),
        _ => Err("World field kind is invalid".into()),
    }
}

fn schema_fingerprint(schema: Schema) -> Result<Hash, String> {
    let (kind, identity, layout) = match schema {
        Schema::Norito {
            nominal_name,
            layout,
        } => (0_u8, nominal_name(), layout),
        Schema::Semantic {
            identity, layout, ..
        } => (1_u8, std::borrow::Cow::Borrowed(identity), layout),
        Schema::Required { identity, .. } => {
            return Err(format!(
                "World state accumulator cannot commit unresolved schema {identity}"
            ));
        }
    };
    let length = u64::try_from(identity.len()).map_err(|_| "World schema identity exceeds u64")?;
    Ok(Hash::new_from_chunks(&[
        SCHEMA_VALUE,
        &[kind, layout.major, layout.minor, layout.flags],
        &length.to_le_bytes(),
        identity.as_bytes(),
    ]))
}

/// Expand one entry into its lanes.
fn element(path: &Hash, key: Option<&Hash>, value: &Hash) -> [u8; LANE_BYTES] {
    world_state_element_v1(path, key, value)
}

/// How a pass treats one canonical field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Installation {
    /// Not installed: empty, unwritten and outside the commitment.
    Absent,
    /// Installed by this block: its complete current content enters the commitment.
    Fresh,
    /// Installed before this block: only its changes enter.
    Installed,
}

/// One projection pass over the World overlay.
struct Builder<'a> {
    index: &'a FieldIndex,
    accumulator: WorldStateAccumulator,
    direction: Direction,
    visited: Vec<bool>,
    /// How the pass treats each canonical slot.
    installation: Vec<Installation>,
    snapshot: Option<world_state_snapshot::SnapshotCollector<'a>>,
    snapshot_field: Option<(usize, u8)>,
}

impl Builder<'_> {
    fn run(
        accumulator: WorldStateAccumulator,
        world: &WorldBlock<'_>,
        direction: Direction,
    ) -> Result<WorldStateAccumulator, String> {
        let index = field_index().as_ref().map_err(Clone::clone)?;
        let mut builder = Builder {
            index,
            accumulator,
            direction,
            visited: vec![false; index.canonical],
            installation: installation(index, world, direction)?,
            snapshot: None,
            snapshot_field: None,
        };
        world
            .project_world(&mut builder)
            .map_err(|error| error.to_string())?;
        builder.finish()
    }

    /// The accumulator, once every canonical field was visited exactly once.
    fn finish(self) -> Result<WorldStateAccumulator, String> {
        match self.visited.iter().position(|visited| !visited) {
            Some(missing) => Err(format!(
                "World state accumulator did not visit canonical field {}",
                self.index.ids[missing]
            )),
            None => Ok(self.accumulator),
        }
    }

    /// The field's path hash and installation, or `None` when the registry excludes it.
    fn field(&mut self, name: &str, kind: u8) -> Result<Option<(Hash, Installation)>, String> {
        match self.index.by_name.get(name) {
            None => Err(format!("unclassified World state field: {name}")),
            Some(Classified::Excluded) => Ok(None),
            Some(Classified::Canonical {
                path,
                kind: declared,
                slot,
            }) => {
                if *declared != kind {
                    return Err(format!("World state field kind mismatch: {name}"));
                }
                let visited = &mut self.visited[*slot];
                if *visited {
                    return Err(format!("World state field visited twice: {name}"));
                }
                *visited = true;
                if self.snapshot.is_some() {
                    self.snapshot_field = Some((*slot, kind));
                }
                Ok(Some((*path, self.installation[*slot])))
            }
        }
    }

    fn change(
        &mut self,
        path: &Hash,
        key: Option<&Hash>,
        before: Option<Hash>,
        after: Option<Hash>,
    ) -> Result<(), world_state_snapshot::WorldStateSnapshotError> {
        if before == after {
            return Ok(());
        }
        let (removed, added) = match self.direction {
            #[cfg(test)]
            Direction::Reverse => (after, before),
            Direction::Capture | Direction::Forward => (before, after),
        };
        if let Some(value) = removed {
            self.accumulator.remove(&element(path, key, &value));
        }
        if let Some(value) = added {
            if let Some(snapshot) = self.snapshot.as_mut() {
                let (slot, kind) = self
                    .snapshot_field
                    .ok_or("World snapshot has no canonical field owner")?;
                snapshot.push(
                    self.index.ids[slot],
                    element_kind(kind)?,
                    key.copied(),
                    value,
                )?;
            }
            self.accumulator.add(&element(path, key, &value));
        }
        Ok(())
    }
}

/// How a pass over `world` treats each canonical slot: a capture follows the current selection;
/// a block pass also compares it with the selection before the block.
///
/// # Errors
/// A selection installs storage this release does not implement, or the block retires storage,
/// which only a migration cutover may do and this release does not implement.
fn installation(
    index: &FieldIndex,
    world: &WorldBlock<'_>,
    direction: Direction,
) -> Result<Vec<Installation>, String> {
    let successor = installed_slots(index, world.runtime_selection.get())?;
    let parent = match direction {
        Direction::Capture => successor.clone(),
        _ => installed_slots(index, world.runtime_selection.get_before_block())?,
    };
    successor
        .iter()
        .zip(&parent)
        .zip(&index.ids)
        .map(|((successor, parent), id)| match (*parent, *successor) {
            (false, false) => Ok(Installation::Absent),
            (false, true) => Ok(Installation::Fresh),
            (true, true) => Ok(Installation::Installed),
            (true, false) => Err(format!(
                "World field {id} is retired without a migration cutover"
            )),
        })
        .collect()
}

impl WorldProjection for Builder<'_> {
    type Error = world_state_snapshot::WorldStateSnapshotError;

    /// Block passes follow the execution seal's and publication surface's net-delta passes over
    /// the same trigger stores, which already ran the full contract-row check.
    fn validates_trigger_contract_rows(&self) -> bool {
        self.direction == Direction::Capture
    }

    fn append_musubi_archive_availability(
        &mut self,
        storage: &StorageField<'_, ArchiveId, MusubiArchiveAvailabilityV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_archive_availability", storage, |row| {
            let anchor = crate::state::authority_registry::world::musubi_availability_policy::MusubiAvailabilityAuthorityV1::from_record(row);
            hash_value(&anchor)
        })
    }

    fn append_musubi_resolver_index(
        &mut self,
        storage: &StorageField<'_, MusubiReleaseIdV1, MusubiResolverReleaseRowV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_resolver_index", storage, |row| {
            let authority = crate::state::authority_registry::world::musubi_universal_policy::MusubiResolverAuthorityV1::from_record(row);
            hash_value(&authority)
        })
    }

    fn append_musubi_public_directory(
        &mut self,
        storage: &StorageField<'_, MusubiPackageSelectorV1, MusubiOrderedPackageEntryV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_public_directory", storage, |row| {
            let authority = crate::state::authority_registry::world::musubi_universal_policy::MusubiDirectoryAuthorityV1::from_record(row);
            hash_value(&authority)
        })
    }

    fn append_storage_with<K: Key + Encode, V: Value, M: mv::storage::StorageMode<K, V>>(
        &mut self,
        name: &'static str,
        storage: &StorageField<'_, K, V, M>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error> {
        let Some((path, installation)) = self.field(name, TABLE)? else {
            return Ok(());
        };
        match (installation, self.direction) {
            (Installation::Absent, Direction::Capture) => {
                if storage.iter().next().is_some() {
                    return Err(
                        format!("World field {name} holds entries but is not installed").into(),
                    );
                }
            }
            (Installation::Absent, _) => {
                if storage.touched_entries().next().is_some() {
                    return Err(
                        format!("World field {name} was written but is not installed").into(),
                    );
                }
            }
            (Installation::Fresh, _) | (Installation::Installed, Direction::Capture) => {
                for (key, value) in storage.iter() {
                    let key = hash_value(key)?;
                    self.change(&path, Some(&key), None, Some(encode(value)?))?;
                }
            }
            (Installation::Installed, _) => {
                for entry in storage.touched_entries() {
                    let before = entry.before.map(&encode).transpose()?;
                    let after = entry.after.map(&encode).transpose()?;
                    if before != after {
                        let key = hash_value(entry.key)?;
                        self.change(&path, Some(&key), before, after)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn append_cell_with<V: Value, C: Send + Sync + 'static>(
        &mut self,
        name: &'static str,
        cell: &CellField<'_, V, C>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error> {
        let Some((path, installation)) = self.field(name, CELL)? else {
            return Ok(());
        };
        match (installation, self.direction) {
            (Installation::Absent, Direction::Capture) => {}
            (Installation::Absent, _) => {
                if cell.touched_value().is_some() {
                    return Err(
                        format!("World field {name} was written but is not installed").into(),
                    );
                }
            }
            (Installation::Fresh, _) | (Installation::Installed, Direction::Capture) => {
                self.change(&path, None, None, Some(encode(cell.get())?))?;
            }
            (Installation::Installed, _) => {
                if let Some(value) = cell.touched_value() {
                    self.change(
                        &path,
                        None,
                        Some(encode(value.before)?),
                        Some(encode(value.after)?),
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl WorldBlock<'_> {
    /// The accumulators before and after this overlay's changes so far. Genesis starts from the
    /// empty World and absorbs everything the World holds, including state seeded before its
    /// execution; every later block starts from the stored accumulator of its predecessor.
    ///
    /// # Errors
    /// See [`WorldStateAccumulator::capture`].
    pub(in crate::state) fn state_transition(
        &self,
        genesis: bool,
    ) -> Result<(WorldStateAccumulator, WorldStateAccumulator), String> {
        if genesis {
            return Ok((
                WorldStateAccumulator::empty(),
                WorldStateAccumulator::capture(self)?,
            ));
        }
        let parent = self.state_accumulator.get_before_block().clone();
        let post = parent.apply_block(self)?;
        Ok((parent, post))
    }

    /// Store the accumulator of the completed overlay, after every deterministic tail write of
    /// the block: the parent World state root of the next block. Repeating it is idempotent (the
    /// accumulator is derived state and never part of its own change set).
    ///
    /// # Errors
    /// See [`WorldStateAccumulator::capture`]; the stored accumulator is left unchanged.
    pub(in crate::state) fn advance_state_accumulator(
        &mut self,
        genesis: bool,
    ) -> Result<(), String> {
        let (_, post) = self.state_transition(genesis)?;
        *self.state_accumulator.get_mut() = post;
        Ok(())
    }
}

impl crate::state::StateBlock<'_> {
    /// The complete-World part of this block's execution result (`specs/sumeragi.md` §4.1,
    /// Appendix E, E51): the World state roots before and after the execution so far and the
    /// commitment of the events it emitted. The executor binds it in `R` once execution is
    /// sealed; later apply-time writes enter the next block's parent root.
    ///
    /// # Errors
    /// A World value or an event cannot be encoded, or the World inventory is incomplete.
    pub(crate) fn world_state_transition(
        &self,
    ) -> Result<crate::sumeragi::commitment::WorldStateTransition, String> {
        let (parent, post) = self.world.state_transition(self._curr_block.is_genesis())?;
        Ok(crate::sumeragi::commitment::WorldStateTransition {
            parent_world_state_root: parent
                .root(self.world.runtime_selection.get_before_block())?,
            world_state_root: post.root(self.world.runtime_selection.get())?,
            event_commitment: crate::sumeragi::commitment::event_commitment(
                &self.world.external_event_buf,
            )
            .map_err(|error| error.to_string())?,
        })
    }
}

/// A cold World-root check retains local acquisition refusal separately from invalid content.
#[derive(Debug, thiserror::Error)]
pub enum WorldStateVerificationError {
    /// The complete original World overlay could not be acquired.
    /// Busy and capacity errors retain their exact original release observations.
    #[error("World root verification acquisition: {0}")]
    Acquisition(#[source] mv::storage::AdmittedStorageError),
    /// A canonical value could not be encoded or the authority inventory was invalid.
    #[error("World root verification capture: {0}")]
    Capture(String),
    /// The stored accumulator does not describe the complete committed World.
    #[error(
        "stored World state root {stored_root} ({stored_entries} entries) differs from the complete World's {captured_root} ({captured_entries} entries)"
    )]
    Mismatch {
        /// Root asserted by the stored accumulator.
        stored_root: Hash,
        /// Number of entries asserted by the stored accumulator.
        stored_entries: u64,
        /// Root independently captured from the complete committed World.
        captured_root: Hash,
        /// Number of entries independently captured from the complete committed World.
        captured_entries: u64,
    },
}

impl crate::state::State {
    /// Check the stored World state accumulator against a cold capture of the complete
    /// committed World, in `O(N)` (startup after replay). Returns the verified World state root.
    ///
    /// # Errors
    /// The stored accumulator differs from the World it claims to commit, the World cannot be
    /// acquired or a value cannot be encoded.
    pub(crate) fn verify_world_state_accumulator(
        &self,
    ) -> Result<Hash, WorldStateVerificationError> {
        // The value every reader of the committed World sees.
        let view = self.view();
        let stored = view.world.state_accumulator.get().clone();
        drop(view);
        self.world
            .verify_captured_accumulator(&stored, &self.ivm_execution_budget())
    }
}

impl crate::state::World {
    /// Check the stored World state accumulator against a cold capture of this complete World,
    /// in `O(N)`; a restored World is checked before any State is built on it. Returns the
    /// verified World state root.
    ///
    /// # Errors
    /// The stored accumulator differs from the World it claims to commit, the World cannot be
    /// acquired or a value cannot be encoded.
    pub(crate) fn verify_state_accumulator(
        &self,
        execution_budget: &iroha_allocation::AllocationBudget,
    ) -> Result<Hash, WorldStateVerificationError> {
        let stored = self.state_accumulator.view().get().clone();
        self.verify_captured_accumulator(&stored, execution_budget)
    }

    /// Take over a World imported at an upgrade checkpoint (`state::upgrade_checkpoint`).
    ///
    /// The decoded accumulator Cell is the predecessor's. A cold capture of this World under
    /// this release's registry must equal it once the predecessor's retired fields are added back
    /// ([`predecessor_root`]), which proves every canonical entry was imported exactly, and the
    /// predecessor's root of it must equal `certified`, the checkpoint block's certified World
    /// root. The Cell then holds the capture under this release's registry, from which the next
    /// block chains. Returns that root.
    ///
    /// # Errors
    /// The import differs from the predecessor's World, the root differs from `certified`, the
    /// World cannot be acquired or a value cannot be encoded.
    pub(crate) fn adopt_checkpoint_accumulator(
        &mut self,
        execution_budget: &iroha_allocation::AllocationBudget,
        certified: Hash,
    ) -> Result<Hash, WorldStateVerificationError> {
        let stored = self.state_accumulator.view().get().clone();
        let block = self
            .try_block(execution_budget)
            .map_err(WorldStateVerificationError::Acquisition)?;
        let captured =
            WorldStateAccumulator::capture(&block).map_err(WorldStateVerificationError::Capture)?;
        drop(block);
        let (predecessor, root) =
            predecessor_root(&captured).map_err(WorldStateVerificationError::Capture)?;
        if predecessor != stored || root != certified {
            return Err(WorldStateVerificationError::Mismatch {
                stored_root: certified,
                stored_entries: stored.entries(),
                captured_root: root,
                captured_entries: predecessor.entries(),
            });
        }
        let current = captured
            .root(self.runtime_selection.view().get())
            .map_err(WorldStateVerificationError::Capture)?;
        self.state_accumulator = mv::cell::Cell::new(captured);
        Ok(current)
    }

    fn verify_captured_accumulator(
        &self,
        stored: &WorldStateAccumulator,
        execution_budget: &iroha_allocation::AllocationBudget,
    ) -> Result<Hash, WorldStateVerificationError> {
        let block = self.try_block(execution_budget).map_err(|error| {
            if cfg!(all(test, sumeragi_core_mutation = "HC82")) {
                WorldStateVerificationError::Capture(error.to_string())
            } else {
                WorldStateVerificationError::Acquisition(error)
            }
        })?;
        let captured =
            WorldStateAccumulator::capture(&block).map_err(WorldStateVerificationError::Capture)?;
        let selection = block.runtime_selection.get();
        if *stored != captured {
            return Err(WorldStateVerificationError::Mismatch {
                stored_root: stored
                    .root(selection)
                    .map_err(WorldStateVerificationError::Capture)?,
                stored_entries: stored.entries(),
                captured_root: captured
                    .root(selection)
                    .map_err(WorldStateVerificationError::Capture)?,
                captured_entries: captured.entries(),
            });
        }
        captured
            .root(selection)
            .map_err(WorldStateVerificationError::Capture)
    }
}

#[cfg(test)]
impl WorldStateAccumulator {
    /// The root under the baseline selection, which every World a test founds keeps unless
    /// the test changes it.
    pub(crate) fn baseline_root(&self) -> Result<Hash, String> {
        self.root(&baseline_selection())
    }
}

/// The selection every World of this release is founded on.
#[cfg(test)]
pub(crate) fn baseline_selection() -> RootSelectionV1 {
    crate::runtime::release_catalog()
        .expect("release catalog")
        .baseline
        .clone()
}

#[cfg(test)]
#[path = "world_state_accumulator_tests.rs"]
mod tests;

// Inventory of every State/World field and the root it currently affects
// (`specs/state_table_inventory.json`), with the per-field mutation evidence.
#[cfg(test)]
#[path = "state_table_inventory_tests.rs"]
mod inventory_tests;

#[path = "world_state_snapshot.rs"]
mod world_state_snapshot;
pub use world_state_snapshot::{
    NativeResourceNamesSnapshotV1, NativeZkX509StateV1, ProviderAdmissionSnapshotOriginalsV1,
    WorldStateSnapshotError,
};

#[path = "world_state_cut.rs"]
pub mod world_state_cut;
