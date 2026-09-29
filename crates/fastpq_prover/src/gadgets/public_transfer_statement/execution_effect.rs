//! Strict complete-entry quantity effects over the shared two-update SMT relation.
//!
//! Transfer consumes balance/balance ports; mint and burn consume balance/supply
//! ports. Arithmetic, exact quantities, typed keys and original chronology are
//! checked here before generic SMT leaf bindings are constructed. Source facts
//! and final public inputs require independent authenticated expectations. Local
//! materialization establishes no finality, authorization or admission result.
//! TODO: replace the ordinary transfer-only source capture and artifact format
//! coherently before enabling this relation in any production proof dispatcher.

use std::collections::{BTreeMap, HashMap};

use iroha_crypto::Hash;
use iroha_data_model::fastpq::{
    FastpqExecutionAssetV1, FastpqExecutionEffectKindV1, FastpqExecutionEffectStatementV1,
    FastpqExecutionEffectsV1, FastpqExecutionQuantityKeyV1, FastpqOperationKind,
    FastpqPublicInputs, FastpqQuantityUnits, FastpqSourceRouteV1, FastpqStateTransition,
    execution_quantity_key_v1,
};
use iroha_primitives::numeric::Quantity;

use super::{
    PublicKeyAllocation, PublicPreparationWork, PublicTransferLimits, TransferValue, allocate_path,
    check_limit, checked_u32, digest_limbs, encode_quantity_units_v1, invariant,
    materialize::{
        CheckedUpdatePair, CheckedUpdateRow, CheckedUpdateTable, DerivedTransferSmtWitnesses,
        TransferSmtBuildLimits, derive_two_update_smt,
    },
    require_marked,
};
use crate::gadgets::compact_smt_air::{PublicStatement, PublicUpdate};
use crate::{PublicInputs, Result};

const KEY_DOMAIN: &[u8] = b"fastpq:execution-effects:v1:key|";
const ORDERING_DOMAIN: &[u8] = b"fastpq:execution-effects:v1:ordering|";
const FACTS_DOMAIN: &[u8] = b"fastpq:execution-effects:v1:source|";
const STATEMENT_DOMAIN: &[u8] = b"fastpq:execution-effects:v1:statement|";

/// Explicit complete-entry public preparation limits; no private path work is included.
#[allow(
    clippy::struct_field_names,
    reason = "every field is an inclusive maximum, named like the crate's other `*Limits` policies"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionEffectLimits {
    /// Maximum original typed effects in one logical execution entry.
    pub max_effects: usize,
    /// Maximum participant rows, exactly twice the effect count.
    pub max_rows: usize,
    /// Maximum complete canonical statement frame bytes.
    pub max_public_bytes: usize,
    /// Maximum distinct complete tagged quantity keys.
    pub max_unique_keys: usize,
    /// Maximum occupied-interval lookups for deterministic path allocation.
    pub max_allocation_steps: usize,
}
impl Default for ExecutionEffectLimits {
    fn default() -> Self {
        let limits = PublicTransferLimits::default();
        Self {
            max_effects: limits.max_deltas,
            max_rows: limits.max_rows,
            max_public_bytes: limits.max_public_bytes,
            max_unique_keys: limits.max_unique_keys,
            max_allocation_steps: limits.max_allocation_steps,
        }
    }
}

/// Independent expectations obtained from authenticated execution/source state.
/// Copying these fields from the offered statement does not authenticate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionEffectExpectations {
    /// Exact complete original effect tape commitment.
    pub effects_digest: Hash,
    /// Exact complete canonical statement commitment, including all context.
    pub statement_digest: Hash,
    /// Exact expected public inputs, including externally authorized roots.
    pub public_inputs: FastpqPublicInputs,
}

/// One checked participant row; roles retain typed effect semantics instead of aliases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionEffectRow {
    /// Original contiguous effect ordinal, independent of key sorting.
    pub effect_ordinal: u32,
    /// Port zero then one: source/destination or balance/supply respectively.
    pub leg: usize,
    /// Complete typed-key allocation index.
    pub key_index: usize,
    /// Exact common asset/incarnation scale chosen from every original quantity.
    pub scale: u32,
    /// Complete normalized pre-value.
    pub before: FastpqQuantityUnits,
    /// Complete normalized post-value.
    pub after: FastpqQuantityUnits,
    /// Original normalized operation amount, retained even for zero effects.
    pub amount: FastpqQuantityUnits,
    /// Exact leaf hashes and collision-resolved path consumed by the common AIR.
    pub update: PublicUpdate,
}

/// Private-field public table after complete statement/expectation and semantic checks.
#[derive(Debug)]
pub struct PreparedExecutionEffects {
    public_inputs: PublicInputs,
    keys: Vec<PublicKeyAllocation>,
    rows: Vec<ExecutionEffectRow>,
    pairs: Vec<[usize; 2]>,
    ordering_hash: Hash,
    work: PublicPreparationWork,
}
impl PreparedExecutionEffects {
    /// Participant bindings in the canonical key/operation row order.
    #[must_use]
    pub fn rows(&self) -> &[ExecutionEffectRow] {
        &self.rows
    }
    /// Complete sorted typed quantity keys and paths.
    #[must_use]
    pub fn keys(&self) -> &[PublicKeyAllocation] {
        &self.keys
    }
    /// Exact public preparation work; no SMT node hashes are included.
    #[must_use]
    pub const fn work(&self) -> PublicPreparationWork {
        self.work
    }
    /// Bind each chronological effect to independently committed intermediate roots.
    /// # Errors
    /// Rejects an incorrect root count or noncanonical hash marker.
    pub fn compact_statements(&self, intermediate: &[[u8; 32]]) -> Result<Vec<PublicStatement>> {
        if intermediate.len() != self.pairs.len().saturating_sub(1) {
            return Err(invariant(
                "execution effect intermediate-root count mismatch",
            ));
        }
        for root in intermediate {
            require_marked(root)?;
        }
        let mut current = self.public_inputs.old_root;
        self.pairs
            .iter()
            .enumerate()
            .map(|(index, ports)| {
                let next = intermediate
                    .get(index)
                    .copied()
                    .unwrap_or(self.public_inputs.new_root);
                let result = PublicStatement {
                    updates: [self.rows[ports[0]].update, self.rows[ports[1]].update],
                    old_root: digest_limbs(current),
                    new_root: digest_limbs(next),
                };
                current = next;
                Ok(result)
            })
            .collect()
    }
    /// Materialize private paths and require exact independently expected old/new roots.
    /// # Errors
    /// Rejects limits, malformed internal ports or roots inconsistent with the public table.
    pub fn build_smt_witnesses(
        &self,
        limits: TransferSmtBuildLimits,
    ) -> Result<DerivedTransferSmtWitnesses> {
        let built = derive_two_update_smt(self, limits)?;
        if built.roots() != (self.public_inputs.old_root, self.public_inputs.new_root) {
            return Err(invariant(
                "execution effect derived roots differ from public inputs",
            ));
        }
        Ok(built)
    }
}
impl CheckedUpdateTable for PreparedExecutionEffects {
    fn public_inputs(&self) -> PublicInputs {
        self.public_inputs
    }
    fn keys(&self) -> &[PublicKeyAllocation] {
        &self.keys
    }
    fn row_count(&self) -> usize {
        self.rows.len()
    }
    fn pair_count(&self) -> usize {
        self.pairs.len()
    }
    fn row(&self, index: usize) -> Option<CheckedUpdateRow> {
        self.rows.get(index).map(|row| CheckedUpdateRow {
            key_index: row.key_index,
            occurrence: [0, row.effect_ordinal, row.effect_ordinal],
            leg: row.leg,
            update: row.update,
        })
    }
    fn pair(&self, index: usize) -> Option<CheckedUpdatePair> {
        let ordinal = u32::try_from(index).ok()?;
        let ports = *self.pairs.get(index)?;
        Some(CheckedUpdatePair {
            occurrence: [0, ordinal, ordinal],
            row_indices: ports,
            updates: [
                self.rows.get(ports[0])?.update,
                self.rows.get(ports[1])?.update,
            ],
        })
    }
}

/// Producer-owned local statement and paths; these do not certify execution or finality.
#[derive(Debug)]
pub struct ExecutionEffectMaterialization {
    /// Complete canonical statement retaining every original fact and occurrence.
    pub statement: FastpqExecutionEffectStatementV1,
    /// Exact private paths from the existing two-update SMT constructor.
    pub witnesses: DerivedTransferSmtWitnesses,
}

/// Prepare the complete offered statement against independent expectations without SMT work.
/// # Errors
/// Rejects all count/byte/allocation bounds, commitment/input disagreement, malformed
/// lifecycle/context, arithmetic, missing/reordered effects, gaps and row mismatches.
pub fn prepare_execution_effect_statement(
    statement: &FastpqExecutionEffectStatementV1,
    expected: ExecutionEffectExpectations,
    limits: ExecutionEffectLimits,
) -> Result<PreparedExecutionEffects> {
    preflight(&statement.effects, Some(&statement.transitions), limits)?;
    let frame = bounded_frame(statement, limits.max_public_bytes)?;
    if Hash::new_from_chunks(&[STATEMENT_DOMAIN, &frame]) != expected.statement_digest
        || statement.public_inputs != expected.public_inputs
    {
        return Err(invariant(
            "execution effect independent statement expectation mismatch",
        ));
    }
    check_effects_digest(&statement.effects, expected.effects_digest, limits)?;
    let (canonical, mut prepared) =
        prepare_facts(&statement.effects, statement.public_inputs, limits)?;
    if canonical != statement.transitions
        || prepared.ordering_hash.as_ref() != &statement.ordering_hash
    {
        return Err(invariant(
            "execution effect canonical row/ordering mismatch",
        ));
    }
    prepared.work.public_bytes = frame.len();
    Ok(prepared)
}

/// Derive local touched-quantity roots from an independently expected complete effect tape.
///
/// All non-root public inputs remain exactly as supplied. No gap is filled and no
/// authorization or lifecycle token is generated. Returned roots concern only the
/// complete typed keys touched by this entry, not the global world state.
/// # Errors
/// Rejects count/byte/tree limits, facts commitment mismatch or any strict semantic failure.
pub fn materialize_execution_effect_statement(
    effects: &FastpqExecutionEffectsV1,
    expected_effects_digest: Hash,
    mut public_inputs: FastpqPublicInputs,
    limits: ExecutionEffectLimits,
    tree_limits: TransferSmtBuildLimits,
) -> Result<ExecutionEffectMaterialization> {
    preflight(effects, None, limits)?;
    check_limit(
        "max_execution_effect_tree_updates",
        effects
            .effects
            .len()
            .checked_mul(2)
            .ok_or_else(|| invariant("execution effect row count overflows"))?,
        tree_limits.max_updates,
    )?;
    check_effects_digest(effects, expected_effects_digest, limits)?;
    let mut scratch = public_inputs;
    if !effects.effects.is_empty() {
        let placeholder: [u8; 32] = Hash::new(b"execution effect local root placeholder").into();
        scratch.old_root = placeholder;
        scratch.new_root = placeholder;
    }
    let (transitions, prepared) = prepare_facts(effects, scratch, limits)?;
    let witnesses = derive_two_update_smt(&prepared, tree_limits)?;
    (public_inputs.old_root, public_inputs.new_root) = witnesses.roots();
    let statement = FastpqExecutionEffectStatementV1 {
        public_inputs,
        ordering_hash: prepared.ordering_hash.into(),
        transitions,
        effects: effects.clone(),
    };
    // Bound the complete final nominal frame before exposing a candidate to a caller.
    bounded_frame(&statement, limits.max_public_bytes)?;
    Ok(ExecutionEffectMaterialization {
        statement,
        witnesses,
    })
}

fn bounded_frame<T: norito::NoritoSerialize>(value: &T, max: usize) -> Result<Vec<u8>> {
    let _canonical = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    norito::core::to_bytes_bounded(value, max).map_err(|error| {
        invariant(&format!(
            "execution effect bounded encoding failed: {error}"
        ))
    })
}
fn check_effects_digest(
    effects: &FastpqExecutionEffectsV1,
    expected: Hash,
    limits: ExecutionEffectLimits,
) -> Result<()> {
    let bytes = bounded_frame(effects, limits.max_public_bytes)?;
    if Hash::new_from_chunks(&[FACTS_DOMAIN, &bytes]) != expected {
        return Err(invariant(
            "execution effect independent facts expectation mismatch",
        ));
    }
    Ok(())
}
fn preflight(
    effects: &FastpqExecutionEffectsV1,
    transitions: Option<&[FastpqStateTransition]>,
    limits: ExecutionEffectLimits,
) -> Result<()> {
    check_limit(
        "max_execution_effects",
        effects.effects.len(),
        limits.max_effects,
    )?;
    let count = effects
        .effects
        .len()
        .checked_mul(2)
        .ok_or_else(|| invariant("execution effect row count overflows"))?;
    checked_u32(count)?;
    check_limit("max_execution_effect_rows", count, limits.max_rows)?;
    if let Some(rows) = transitions {
        if rows.len() != count {
            return Err(invariant("execution effect row count mismatch"));
        }
        let mut bytes = 0usize;
        for row in rows {
            for value in [&row.key, &row.pre_value, &row.post_value] {
                bytes = bytes
                    .checked_add(value.len())
                    .ok_or_else(|| invariant("execution effect bytes overflow"))?;
                check_limit("max_execution_effect_bytes", bytes, limits.max_public_bytes)?;
            }
        }
    }
    // Stream exact field lengths before any complete tape/frame copy or normalized tables.
    let mut bytes = norito::canonical_frame_len(&effects.context)?;
    for effect in &effects.effects {
        bytes = bytes
            .checked_add(norito::canonical_frame_len(effect)?)
            .ok_or_else(|| invariant("execution effect bytes overflow"))?;
        check_limit("max_execution_effect_bytes", bytes, limits.max_public_bytes)?;
    }
    Ok(())
}

struct NormalizedRow {
    transition: FastpqStateTransition,
    ordinal: u32,
    leg: usize,
    scale: u32,
    before: FastpqQuantityUnits,
    after: FastpqQuantityUnits,
    amount: FastpqQuantityUnits,
}
fn quantities(kind: &FastpqExecutionEffectKindV1) -> (&FastpqExecutionAssetV1, [&Quantity; 5]) {
    match kind {
        FastpqExecutionEffectKindV1::Transfer(t) => (
            &t.source.asset,
            [
                &t.amount,
                &t.source_before,
                &t.source_after,
                &t.destination_before,
                &t.destination_after,
            ],
        ),
        FastpqExecutionEffectKindV1::Mint(t) | FastpqExecutionEffectKindV1::Burn(t) => (
            &t.balance.asset,
            [
                &t.amount,
                &t.balance_before,
                &t.balance_after,
                &t.supply_before,
                &t.supply_after,
            ],
        ),
    }
}
fn operation_rank(operation: FastpqOperationKind) -> u8 {
    match operation {
        FastpqOperationKind::Transfer => 0,
        FastpqOperationKind::Mint => 1,
        FastpqOperationKind::Burn => 2,
        _ => u8::MAX,
    }
}
fn inputs(value: FastpqPublicInputs) -> PublicInputs {
    PublicInputs {
        dsid: value.dsid,
        slot: value.slot,
        old_root: value.old_root,
        new_root: value.new_root,
        perm_root: value.perm_root,
        tx_set_hash: value.tx_set_hash,
    }
}

fn prepare_facts(
    effects: &FastpqExecutionEffectsV1,
    public_inputs: FastpqPublicInputs,
    limits: ExecutionEffectLimits,
) -> Result<(Vec<FastpqStateTransition>, PreparedExecutionEffects)> {
    preflight(effects, None, limits)?;
    check_context(effects, &public_inputs)?;
    let scales = asset_scales(effects)?;
    let (mut normalized, unique_keys) = normalize_effects(effects, &scales, limits)?;
    // Stable sorting retains original same-key/same-operation occurrence order.
    // Ports record chronology explicitly even when operation sorting moves mint/burn rows.
    normalized.sort_by(|a, b| {
        (&a.transition.key, operation_rank(a.transition.operation))
            .cmp(&(&b.transition.key, operation_rank(b.transition.operation)))
    });
    let (keys, allocation_steps) = allocate_keys(&normalized, unique_keys, limits)?;
    let unique_count = keys.len();
    let mut rows = Vec::with_capacity(normalized.len());
    let mut transitions = Vec::with_capacity(normalized.len());
    let mut pairs = vec![[usize::MAX; 2]; effects.effects.len()];
    let mut key_index = 0;
    for row in normalized {
        while keys[key_index].key != row.transition.key {
            key_index += 1;
        }
        let key = &keys[key_index];
        let update = PublicUpdate {
            old_leaf: digest_limbs(FastpqQuantityUnits::leaf(&key.key_hash, row.before)?),
            new_leaf: digest_limbs(FastpqQuantityUnits::leaf(&key.key_hash, row.after)?),
            path: key.path,
        };
        pairs[row.ordinal as usize][row.leg] = rows.len();
        transitions.push(row.transition);
        rows.push(ExecutionEffectRow {
            effect_ordinal: row.ordinal,
            leg: row.leg,
            key_index,
            scale: row.scale,
            before: row.before,
            after: row.after,
            amount: row.amount,
            update,
        });
    }
    let ordering_bytes = bounded_frame(&transitions, limits.max_public_bytes)?;
    let ordering_hash = Hash::new_from_chunks(&[ORDERING_DOMAIN, &ordering_bytes]);
    let hashes = rows
        .len()
        .checked_mul(2)
        .ok_or_else(|| invariant("execution effect hash work overflows"))?;
    Ok((
        transitions,
        PreparedExecutionEffects {
            public_inputs: inputs(public_inputs),
            keys,
            rows,
            pairs,
            ordering_hash,
            work: PublicPreparationWork {
                public_bytes: 0,
                key_hashes: unique_count,
                value_hashes: hashes,
                leaf_hashes: hashes,
                allocation_steps,
            },
        },
    ))
}

/// Check marked roots, source height, lane incarnation and empty-entry root equality.
fn check_context(
    effects: &FastpqExecutionEffectsV1,
    public_inputs: &FastpqPublicInputs,
) -> Result<()> {
    require_marked(&public_inputs.old_root)?;
    require_marked(&public_inputs.new_root)?;
    if effects.context.source.height == 0 {
        return Err(invariant("execution effect source height is zero"));
    }
    if let FastpqSourceRouteV1::Lane(lane) = effects.context.entry.route {
        require_marked(lane.lane_incarnation.as_ref())?;
    }
    if effects.effects.is_empty() && public_inputs.old_root != public_inputs.new_root {
        return Err(invariant(
            "empty execution effect statement changes its root",
        ));
    }
    Ok(())
}

/// Common scale per asset incarnation: the maximum over every original quantity.
fn asset_scales(
    effects: &FastpqExecutionEffectsV1,
) -> Result<BTreeMap<FastpqExecutionAssetV1, u32>> {
    let mut scales = BTreeMap::<FastpqExecutionAssetV1, u32>::new();
    for effect in &effects.effects {
        let (asset, values) = quantities(&effect.kind);
        asset
            .incarnation
            .validate()
            .map_err(|_| invariant("execution effect asset incarnation is invalid"))?;
        let scale = scales.entry(asset.clone()).or_default();
        for value in values {
            *scale = (*scale).max(value.scale());
        }
    }
    Ok(scales)
}

/// Normalize every effect into its two chained participant rows, in original order.
///
/// Also returns the number of distinct complete quantity keys.
fn normalize_effects(
    effects: &FastpqExecutionEffectsV1,
    scales: &BTreeMap<FastpqExecutionAssetV1, u32>,
    limits: ExecutionEffectLimits,
) -> Result<(Vec<NormalizedRow>, usize)> {
    let mut normalized = Vec::with_capacity(effects.effects.len() * 2);
    let mut last = HashMap::<Vec<u8>, FastpqQuantityUnits>::new();
    for (index, effect) in effects.effects.iter().enumerate() {
        if effect.ordinal != checked_u32(index)? {
            return Err(invariant("execution effect ordinals are not contiguous"));
        }
        let (asset, values) = quantities(&effect.kind);
        let scale = scales[asset];
        let [
            amount,
            first_before,
            first_after,
            second_before,
            second_after,
        ] = values.map(|q| {
            FastpqQuantityUnits::from_quantity(q, scale)
                .ok_or_else(|| invariant("execution effect normalization failed"))
        });
        let (amount, first_before, first_after, second_before, second_after) = (
            amount?,
            first_before?,
            first_after?,
            second_before?,
            second_after?,
        );
        let (keys, operation) = effect_ports(
            &effect.kind,
            amount,
            &[first_before, first_after, second_before, second_after],
        )?;
        for (leg, (key, before, after)) in keys
            .into_iter()
            .zip([(first_before, first_after), (second_before, second_after)])
            .map(|(key, (before, after))| (key, before, after))
            .enumerate()
        {
            let key = execution_quantity_key_v1(&key)?;
            if last.get(&key).is_some_and(|previous| *previous != before) {
                return Err(invariant(
                    "execution effect repeated-key quantities do not chain",
                ));
            }
            if !last.contains_key(&key) {
                check_limit(
                    "max_execution_effect_keys",
                    last.len() + 1,
                    limits.max_unique_keys,
                )?;
            }
            last.insert(key.clone(), after);
            normalized.push(NormalizedRow {
                transition: FastpqStateTransition {
                    key,
                    pre_value: encode_quantity_units_v1(&before)?,
                    post_value: encode_quantity_units_v1(&after)?,
                    operation,
                },
                ordinal: effect.ordinal,
                leg,
                scale,
                before,
                after,
                amount,
            });
        }
    }
    Ok((normalized, last.len()))
}

/// Check one effect's typed arithmetic and return its two port keys and operation.
///
/// Transfers use source/destination balance ports; mint and burn use balance and
/// supply ports, where supply never falls below the balance.
fn effect_ports(
    kind: &FastpqExecutionEffectKindV1,
    amount: FastpqQuantityUnits,
    port_values: &[FastpqQuantityUnits; 4],
) -> Result<([FastpqExecutionQuantityKeyV1; 2], FastpqOperationKind)> {
    let [first_before, first_after, second_before, second_after] = *port_values;
    match kind {
        FastpqExecutionEffectKindV1::Transfer(t) => {
            if t.source.asset != t.destination.asset {
                return Err(invariant(
                    "execution effect transfer crosses asset incarnations",
                ));
            }
            if first_before.checked_sub(&amount) != Some(first_after)
                || second_before.checked_add(&amount) != Some(second_after)
            {
                return Err(invariant("execution effect transfer arithmetic mismatch"));
            }
            Ok((
                [
                    FastpqExecutionQuantityKeyV1::Balance(t.source.clone()),
                    FastpqExecutionQuantityKeyV1::Balance(t.destination.clone()),
                ],
                FastpqOperationKind::Transfer,
            ))
        }
        FastpqExecutionEffectKindV1::Mint(t) | FastpqExecutionEffectKindV1::Burn(t) => {
            let mint = matches!(kind, FastpqExecutionEffectKindV1::Mint(_));
            if mint && t.amount.is_zero() {
                return Err(invariant("execution effect mint amount is zero"));
            }
            let arithmetic = if mint {
                first_before.checked_add(&amount) == Some(first_after)
                    && second_before.checked_add(&amount) == Some(second_after)
            } else {
                first_before.checked_sub(&amount) == Some(first_after)
                    && second_before.checked_sub(&amount) == Some(second_after)
            };
            if !arithmetic
                || second_before.checked_cmp(&first_before) == Some(std::cmp::Ordering::Less)
                || second_after.checked_cmp(&first_after) == Some(std::cmp::Ordering::Less)
            {
                return Err(invariant(
                    "execution effect balance/supply arithmetic mismatch",
                ));
            }
            Ok((
                [
                    FastpqExecutionQuantityKeyV1::Balance(t.balance.clone()),
                    FastpqExecutionQuantityKeyV1::Supply(t.balance.asset.clone()),
                ],
                if mint {
                    FastpqOperationKind::Mint
                } else {
                    FastpqOperationKind::Burn
                },
            ))
        }
    }
}

/// Hash and allocate one collision-resolved path per distinct sorted key.
///
/// Returns the allocations in key order and the occupied-interval lookups used.
fn allocate_keys(
    normalized: &[NormalizedRow],
    unique_keys: usize,
    limits: ExecutionEffectLimits,
) -> Result<(Vec<PublicKeyAllocation>, usize)> {
    let mut keys: Vec<PublicKeyAllocation> = Vec::with_capacity(unique_keys);
    for row in normalized {
        if keys.last().is_none_or(|key| key.key != row.transition.key) {
            keys.push(PublicKeyAllocation {
                key: row.transition.key.clone(),
                key_hash: [0; 32],
                path: 0,
            });
        }
    }
    let unique_count = keys.len();
    let mut occupied = BTreeMap::new();
    let mut allocation_steps = 0;
    for key in &mut keys {
        key.key_hash = Hash::new_from_chunks(&[KEY_DOMAIN, &key.key]).into();
        let base = u32::from_le_bytes(key.key_hash[..4].try_into().expect("four key-hash bytes"));
        key.path = allocate_path(
            &mut occupied,
            base,
            unique_count,
            &mut allocation_steps,
            limits.max_allocation_steps,
        )?;
    }
    Ok((keys, allocation_steps))
}

#[cfg(test)]
mod tests;
