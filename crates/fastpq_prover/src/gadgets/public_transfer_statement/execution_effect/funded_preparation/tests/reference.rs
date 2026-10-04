//! Original map-based preparation, retained only as a full-byte parity oracle.
use super::super::super::super::{TransferValue, allocate_path, encode_quantity_units_v1};
use super::super::super::*;
use iroha_data_model::fastpq::{FastpqExecutionQuantityKeyV1, execution_quantity_key_v1};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(super) struct OriginalPrepared {
    pub(super) keys: Vec<PublicKeyAllocation>,
    pub(super) rows: Vec<ExecutionEffectRow>,
    pub(super) pairs: Vec<[usize; 2]>,
    pub(super) ordering_hash: Hash,
    pub(super) work: PublicPreparationWork,
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
pub(super) fn prepare_facts(
    effects: &FastpqExecutionEffectsV1,
    public_inputs: FastpqPublicInputs,
    limits: ExecutionEffectLimits,
) -> Result<(Vec<FastpqStateTransition>, OriginalPrepared)> {
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
    let (ordering_hash, _) =
        bounded_canonical_digest(ORDERING_DOMAIN, &transitions, limits.max_public_bytes)?;
    let hashes = rows
        .len()
        .checked_mul(2)
        .ok_or_else(|| invariant("execution effect hash work overflows"))?;
    Ok((
        transitions,
        OriginalPrepared {
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
        for value in values.into_iter().flatten() {
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
    let mut retired = BTreeSet::<FastpqExecutionAssetV1>::new();
    let mut nonzero_balances = BTreeMap::<FastpqExecutionAssetV1, usize>::new();
    for (index, effect) in effects.effects.iter().enumerate() {
        if effect.ordinal != checked_u32(index)? {
            return Err(invariant("execution effect ordinals are not contiguous"));
        }
        let (asset, values) = quantities(&effect.kind);
        let scale = scales[asset];
        // Closed retirement has no caller-supplied numeric presence fields. Supply
        // is exactly zero at the asset scale; lifecycle presence uses its own fixed
        // Boolean scale, never a supply/balance key or decimal quantity substitute.
        let [
            amount,
            first_before,
            first_after,
            second_before,
            second_after,
        ] = if let Some(values) = values {
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
            [
                amount?,
                first_before?,
                first_after?,
                second_before?,
                second_after?,
            ]
        } else {
            let zero = FastpqQuantityUnits::from_quantity(&Quantity::zero(), scale)
                .ok_or_else(|| invariant("retirement supply scale is invalid"))?;
            let absent = FastpqQuantityUnits::from_quantity(&Quantity::zero(), 0)
                .ok_or_else(|| invariant("retirement lifecycle zero is invalid"))?;
            let present = FastpqQuantityUnits::from_quantity(&Quantity::from(1_u32), 0)
                .ok_or_else(|| invariant("retirement lifecycle one is invalid"))?;
            [zero, zero, zero, present, absent]
        };
        // Original chronology is checked independently of key/operation sorting.
        // A lifecycle end cannot be duplicated or followed by any use of that same
        // incarnation, even if a later operation supplies coherent zero quantities.
        if retired.contains(asset) {
            return Err(invariant("execution effect uses a retired incarnation"));
        }
        if matches!(&effect.kind, FastpqExecutionEffectKindV1::Retire(_)) {
            if nonzero_balances.get(asset).copied().unwrap_or(0) != 0 {
                return Err(invariant(
                    "execution effect retires a nonzero observed balance",
                ));
            }
            retired.insert(asset.clone());
        }

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
            let balance_asset = match &key {
                FastpqExecutionQuantityKeyV1::Balance(balance) => Some(&balance.asset),
                _ => None,
            };
            let encoded_key = execution_quantity_key_v1(&key)?;
            if let Some(asset) = balance_asset {
                let count = nonzero_balances.entry(asset.clone()).or_default();
                if last
                    .get(&encoded_key)
                    .is_some_and(|previous| previous.limbs().iter().any(|limb| *limb != 0))
                {
                    *count = count
                        .checked_sub(1)
                        .ok_or_else(|| invariant("execution balance census underflow"))?;
                }
                if after.limbs().iter().any(|limb| *limb != 0) {
                    *count = count
                        .checked_add(1)
                        .ok_or_else(|| invariant("execution balance census overflow"))?;
                }
            }
            let key = encoded_key;
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
                scale: before.scale(),
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
        FastpqExecutionEffectKindV1::Retire(asset) => {
            let zero = Quantity::zero();
            if first_before.to_quantity() != Some(zero.clone())
                || first_after != first_before
                || second_before.to_quantity() != Some(Quantity::from(1_u32))
                || second_after.to_quantity() != Some(zero)
                || second_before.scale() != 0
                || second_after.scale() != 0
            {
                return Err(invariant(
                    "execution effect retirement presence/supply mismatch",
                ));
            }
            Ok((
                [
                    FastpqExecutionQuantityKeyV1::Supply(asset.clone()),
                    FastpqExecutionQuantityKeyV1::Lifecycle(asset.clone()),
                ],
                // The shared transition carrier already reserves MetaSet for semantics
                // authenticated by its outer statement. Here the closed Retire variant
                // determines BOTH exact keys/values; no arbitrary metadata row is accepted.
                // Ordinary/AXT dispatch remains unchanged and does not accept this candidate.
                FastpqOperationKind::MetaSet,
            ))
        }
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
