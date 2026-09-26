//! Non-authorizing proof of the complete Claim verifier source inventory.
//!
//! This reuses the exact parent/shard/fold scalar verifier graph and its one
//! complete-source challenge. It exposes the source-major point/coefficient
//! cells for the five arithmetic slices, but does not replace the released
//! Claim: child-proof verification, joins, and the terminal relation are still
//! required. The two carrier columns and their reciprocal RLC binding retain
//! the existing cross-Pasta byte authentication.

use super::*;

/// Additional proof-internal source counts after the unchanged 97 external
/// cells and fourteen reciprocal carrier-binding cells.
const INVENTORY_EQ_SOURCE_COUNT: usize = KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1;
const INVENTORY_EP_SOURCE_COUNT: usize = INVENTORY_EQ_SOURCE_COUNT + 1;
const INVENTORY_SEMANTIC_COUNT: usize = INVENTORY_EP_SOURCE_COUNT + 1;

#[derive(Clone, Debug)]
pub(super) struct GlobalInventoryConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    carrier_rlc: KagemushaClaimCarrierRlcConfigV1,
    native_poseidon: PastaNativePoseidonConfigV1,
}

/// Complete source-inventory proof for one parity, without its partial MSMs.
///
/// One proof does not authenticate the opposite carrier by itself: a future
/// root must verify both parity proofs, their four carrier commitments, and
/// their shared two-challenge RLC values before accepting any slice.
#[derive(Clone)]
pub(super) struct KagemushaClaimGlobalInventoryCircuitV1<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    carrier_rlc: KagemushaClaimCarrierRlcMachineV1<F>,
    native_poseidon_jobs: PastaNativePoseidonJobsV1<F>,
}

impl<F: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>> Circuit<F>
    for KagemushaClaimGlobalInventoryCircuitV1<F>
{
    type Config = GlobalInventoryConfigV1<F>;
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;

    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            carrier_rlc: self.carrier_rlc.unknown(),
            native_poseidon_jobs: self.native_poseidon_jobs.clone().unknown(),
        }
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(usable_rows);
        GlobalInventoryConfigV1 {
            carrier_rlc: KagemushaClaimCarrierRlcConfigV1::configure_with_base(meta, Some(&base)),
            native_poseidon: PastaNativePoseidonConfigV1::configure::<F>(
                meta,
                KAGEMUSHA_MINT_HASH_CLAIM_NATIVE_POSEIDON_LANES_V1,
            ),
            base,
        }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("global inventory uses authenticated Base parameters")
    }

    fn synthesize_for_measurement(
        &self,
        config: Self::Config,
        layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let result = self.synthesize(config, layouter);
        self.builder.reset_synthesis_state();
        result
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "Kagemusha Claim global inventory Base"),
        )?;
        let usable_rows = (1_usize << self.builder.config_params.k) - MINIMUM_UNUSABLE_ROWS;
        self.native_poseidon_jobs.synthesize(
            &config.native_poseidon,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            usable_rows,
        )?;
        self.carrier_rlc.synthesize(
            &config.carrier_rlc,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            usable_rows,
        )
    }
}

/// Exact proof instances: `[113, 4090, 4090]` in the same carrier order as
/// the current hybrid Claim. Indices 0..96 remain the external statement;
/// 97..110 retain reciprocal commitments/challenges/evaluations; 111..112
/// state the Eq and Ep complete-source counts respectively.
#[expect(dead_code, reason = "inventory proof is not selected by the release")]
pub(super) struct KagemushaClaimGlobalInventoryProofInputV1<F: KagemushaPoseidonFieldV1> {
    pub(super) circuit: KagemushaClaimGlobalInventoryCircuitV1<F>,
    pub(super) instances: Vec<Vec<F>>,
}

/// Prove the Eq scalar-verifier inventory without authorizing a monetary Claim.
#[expect(dead_code, reason = "inventory proof awaits joined child verification")]
pub(super) fn build_kagemusha_claim_eq_global_inventory_v1(
    eq_carrier_params: &ParamsIPA<EqAffine>,
    ep_carrier_params: &ParamsIPA<EpAffine>,
    eq_shard_params: &ParamsIPA<EqAffine>,
    ep_shard_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintHashClaimPairWitnessV1<'_>,
    audits: &KagemushaMintHashClaimDeferredAuditsV1,
) -> Result<KagemushaClaimGlobalInventoryProofInputV1<Fp>, String> {
    audits.validate_release_inventory_v1()?;
    validate_claim_pair_witness_v1(
        eq_carrier_params,
        ep_carrier_params,
        eq_shard_params,
        ep_shard_params,
        &witness,
    )?;
    let scalar = build_claim_scalar_half_v1::<EqAffine>(
        &super::super::composite::eq_succinct_vk(eq_carrier_params),
        &super::super::composite::eq_succinct_vk(eq_shard_params),
        KagemushaPastaParityV1::Eq,
        witness.previous.map(|state| state.eq),
        witness.previous_metadata,
        &witness.successor,
        witness.metadata,
        &witness.eq_leaf,
        witness.eq,
        Some(audits.carrier_binding),
    )?;
    finish_global_inventory_v1::<EqAffine>(
        scalar,
        KagemushaPastaParityV1::Eq,
        witness.metadata.eq_deferred_audit,
        audits.eq_digest,
        &audits.eq_carrier,
        &audits.ep_carrier,
        audits.ep.batch.source_count(),
    )
}

/// Prove the Ep scalar-verifier inventory without authorizing a monetary Claim.
#[expect(dead_code, reason = "inventory proof awaits joined child verification")]
pub(super) fn build_kagemusha_claim_ep_global_inventory_v1(
    eq_carrier_params: &ParamsIPA<EqAffine>,
    ep_carrier_params: &ParamsIPA<EpAffine>,
    eq_shard_params: &ParamsIPA<EqAffine>,
    ep_shard_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintHashClaimPairWitnessV1<'_>,
    audits: &KagemushaMintHashClaimDeferredAuditsV1,
) -> Result<KagemushaClaimGlobalInventoryProofInputV1<Fq>, String> {
    audits.validate_release_inventory_v1()?;
    validate_claim_pair_witness_v1(
        eq_carrier_params,
        ep_carrier_params,
        eq_shard_params,
        ep_shard_params,
        &witness,
    )?;
    let scalar = build_claim_scalar_half_v1::<EpAffine>(
        &super::super::composite::ep_succinct_vk(ep_carrier_params),
        &super::super::composite::ep_succinct_vk(ep_shard_params),
        KagemushaPastaParityV1::Ep,
        witness.previous.map(|state| state.ep),
        witness.previous_metadata,
        &witness.successor,
        witness.metadata,
        &witness.ep_leaf,
        witness.ep,
        Some(audits.carrier_binding),
    )?;
    finish_global_inventory_v1::<EpAffine>(
        scalar,
        KagemushaPastaParityV1::Ep,
        witness.metadata.ep_deferred_audit,
        audits.ep_digest,
        &audits.ep_carrier,
        &audits.eq_carrier,
        audits.eq.batch.source_count(),
    )
}

fn finish_global_inventory_v1<C>(
    scalar: ClaimScalarHalfV1<C>,
    parity: KagemushaPastaParityV1,
    expected_audit: DigestV1,
    derived_audit: DigestV1,
    expected_own_carrier: &[u128],
    opposite_carrier: &[u128],
    opposite_sources: usize,
) -> Result<KagemushaClaimGlobalInventoryProofInputV1<C::ScalarExt>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1 + ff::WithSmallOrderMulGroup<3>,
{
    if expected_audit != derived_audit {
        return Err("global Claim inventory changed its paired audit".to_owned());
    }
    let ClaimScalarHalfV1 {
        mut builder,
        output,
        common_cells,
        native_poseidon_jobs,
    } = scalar;
    let audit_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_AUDIT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_AUDIT_LO,
    };
    bind_own_audit_v1(&mut builder, audit_offset, &output)?;
    if assigned_digest_bytes_v1(&output.challenge_limbs)? != derived_audit
        || padded_claim_carrier_u128_values_v1(&output)?.as_slice() != expected_own_carrier
    {
        return Err("global Claim inventory differs from its paired carrier".to_owned());
    }
    let own_sources = output.batch.source_count();
    let own_active = output
        .carrier_cells_v1()
        .map_err(|error| format!("global Claim inventory has invalid carrier: {error:?}"))?;
    validate_claim_carrier_active_len_v1(&output, own_active.len())?;
    attach_global_inventory_instances_v1(
        &mut builder,
        parity,
        own_active,
        own_sources,
        opposite_carrier,
        opposite_sources,
        &common_cells,
    )?;
    let semantic = builder.assigned_instances[0].clone();
    let eq_carrier = builder.assigned_instances[1].clone();
    let ep_carrier = builder.assigned_instances[2].clone();
    let carrier_rlc = constrain_claim_carrier_binding_v1(
        &mut builder,
        &semantic[..KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1],
        &eq_carrier,
        &ep_carrier,
    )?;
    super::super::base_packing::finalize_base_params_v1(&mut builder, MINIMUM_UNUSABLE_ROWS)?;
    let usable_rows = (1_usize << KAGEMUSHA_RECURSION_IPA_K_V1) - MINIMUM_UNUSABLE_ROWS;
    carrier_rlc.validate_capacity(usable_rows)?;
    if native_poseidon_jobs.required_rows()? > usable_rows {
        return Err("global Claim inventory Poseidon exceeds the fixed row envelope".to_owned());
    }
    let instances = builder
        .assigned_instances
        .iter()
        .map(|column| column.iter().map(|cell| *cell.value()).collect())
        .collect();
    Ok(KagemushaClaimGlobalInventoryProofInputV1 {
        circuit: KagemushaClaimGlobalInventoryCircuitV1 {
            builder,
            carrier_rlc,
            native_poseidon_jobs,
        },
        instances,
    })
}

/// Constrain the complete source namespace to canonical, proof-visible cells.
///
/// The own active prefix comes only from the authenticated scalar-verifier
/// output, never a host-selected digest. The opposite prefix is a bounded
/// witness: its authority comes from a separately verified opposite-parity
/// inventory proof and the shared four commitments/two-challenge RLC relation.
fn attach_global_inventory_instances_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    parity: KagemushaPastaParityV1,
    own_active: Vec<AssignedValue<F>>,
    own_sources: usize,
    opposite_padded: &[u128],
    opposite_sources: usize,
    common_cells: &[AssignedValue<F>],
) -> Result<(), String> {
    let active_len = |count: usize| {
        count
            .checked_mul(4)
            .and_then(|value| value.checked_add(KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1))
    };
    if builder.assigned_instances.len() != 1
        || builder.assigned_instances[0].len()
            != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || common_cells.len() != KAGEMUSHA_MINT_HASH_CLAIM_BOUND_VALUE_COUNT_V1
        || !(1..=KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1).contains(&own_sources)
        || !(1..=KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1).contains(&opposite_sources)
        || Some(own_active.len()) != active_len(own_sources)
        || opposite_padded.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err("global Claim inventory has invalid public shape".to_owned());
    }
    let opposite_active = active_len(opposite_sources)
        .ok_or_else(|| "global Claim inventory active length overflowed".to_owned())?;
    if opposite_padded[opposite_active..]
        .iter()
        .any(|value| *value != 0)
    {
        return Err("global Claim inventory has nonzero reciprocal padding".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let zero = ctx.load_constant(F::ZERO);
    let own_count = constrain_inventory_source_count_v1(ctx, &range, own_sources, own_sources)?;
    let opposite_count =
        constrain_inventory_source_count_v1(ctx, &range, opposite_sources, opposite_sources)?;
    for (actual, common) in own_active[own_sources * 4..].iter().zip(common_cells) {
        ctx.constrain_equal(actual, common);
    }
    let mut opposite = Vec::with_capacity(opposite_padded.len());
    for (index, value) in opposite_padded.iter().copied().enumerate() {
        if index >= opposite_active {
            opposite.push(zero);
        } else {
            let cell = ctx.load_witness(F::from_u128(value));
            range.range_check(ctx, cell, 128);
            if index >= opposite_sources * 4 {
                ctx.constrain_equal(&cell, &common_cells[index - opposite_sources * 4]);
            }
            opposite.push(cell);
        }
    }
    let (eq_count, ep_count) = match parity {
        KagemushaPastaParityV1::Eq => (own_count, opposite_count),
        KagemushaPastaParityV1::Ep => (opposite_count, own_count),
    };
    builder.assigned_instances[0].extend([eq_count, ep_count]);
    if builder.assigned_instances[0].len() != INVENTORY_SEMANTIC_COUNT {
        return Err("global Claim inventory semantic count drifted".to_owned());
    }
    let [own, opposite] = pad_assigned_claim_carriers_v1(builder, [own_active, opposite])?;
    let [eq, ep] = match parity {
        KagemushaPastaParityV1::Eq => [own, opposite],
        KagemushaPastaParityV1::Ep => [opposite, own],
    };
    builder.assigned_instances.push(eq);
    builder.assigned_instances.push(ep);
    Ok(())
}

/// Bind the public count to the actual verifier-graph source cardinality.
/// A witness and its matching public instance cannot relabel the active tail.
fn constrain_inventory_source_count_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    actual_sources: usize,
    claimed_sources: usize,
) -> Result<AssignedValue<F>, String> {
    if !(1..=KAGEMUSHA_MINT_HASH_CLAIM_MAX_DEFERRED_SOURCES_V1).contains(&actual_sources) {
        return Err("global Claim inventory source count exceeds capacity".to_owned());
    }
    let actual = ctx.load_constant(F::from(
        u64::try_from(actual_sources).map_err(|_| "source count overflowed".to_owned())?,
    ));
    let claimed = ctx.load_witness(F::from(
        u64::try_from(claimed_sources).map_err(|_| "claimed count overflowed".to_owned())?,
    ));
    ctx.constrain_equal(&claimed, &actual);
    range.range_check(ctx, claimed, 10);
    Ok(claimed)
}

#[cfg(test)]
#[path = "mint_hash_claim_global_inventory_tests.rs"]
mod tests;
