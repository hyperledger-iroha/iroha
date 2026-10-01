//! Complete outgoing ordinary State preparation openings on the same fixed all-operation graph.
//!
//! The State and full ordinary Guard remain separately required. These data do not create Native
//! financial custody. Inactive operations queue the same four SHA jobs and expose zero carriers.

use super::super::{
    canonical_preimage::stream::KagemushaBoundedByteStreamV1,
    generation::KagemushaOrdinaryRecursivePreparedOpeningV1,
    guard_bundle::{assign_bytes, constant_bytes, digest_limbs_assigned},
    ordinary_guard_data_binding::KagemushaOrdinaryGuardDataBindingV1,
    ordinary_prepared_intent::{
        KagemushaOrdinaryPreparedIntentSourcesV1, KagemushaOrdinaryPreparedTransitionSourcesV1,
        constrain_ordinary_prepared_intent_if_v1, constrain_ordinary_prepared_transition_if_v1,
    },
    state_relation::{
        KagemushaAssignedStateRelationV1, KagemushaStateRelationWitnessV1, public_instance,
    },
};
use super::assigned_uint_bytes_v1;
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::builder::BaseCircuitBuilder,
    },
};

use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_SEALED_RECOVERY_SEEDS_DOMAIN_V1 as RECOVERY_STREAM_DOMAIN,
    KAGEMUSHA_ORDINARY_SEALED_TRANSITION_INPUTS_DOMAIN_V1 as TRANSITION_STREAM_DOMAIN,
};

pub(super) fn constrain_ordinary_state_prepared_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    assigned: &KagemushaAssignedStateRelationV1<F>,
    witness: &KagemushaStateRelationWitnessV1,
    original: &KagemushaOrdinaryGuardDataBindingV1<F>,
    transition_sha: &[PastaSha256ByteV1<F>; 32],
    prepared_carriers: &[AssignedValue<F>],
    opening: Option<KagemushaOrdinaryRecursivePreparedOpeningV1<'_>>,
) -> Result<(), String> {
    if prepared_carriers.len() != 6 {
        return Err("ordinary prepared State carrier width differs".into());
    }
    let outgoing = matches!(
        witness.operation,
        super::super::KagemushaOperationV1::SendSplit
            | super::super::KagemushaOperationV1::RedeemSplit
    );
    if outgoing != opening.is_some() {
        return Err(
            "ordinary exact prepared original must be present only for SendSplit/RedeemSplit"
                .into(),
        );
    }
    if let Some(o) = opening {
        o.record.validate_shape()?;
        let actual_prepared = witness
            .prepared_intent
            .ok_or("outgoing State prepared carriers absent")?;
        let expected_operation =
            if witness.operation == super::super::KagemushaOperationV1::SendSplit {
                2
            } else {
                4
            };
        if o.record.operation != expected_operation
            || o.record.binding_digest()? != actual_prepared.preparation_id
            || o.record.transition_digest
                != witness
                    .public_inputs_v1()?
                    .transition_statement_digest_v1()?
            || o.record.preparation_guard_digest != witness.guard_statement_digest
            || o.record.stream_digests
                != [
                    actual_prepared.sealed_transition_inputs_digest,
                    actual_prepared.sealed_recovery_seeds_digest,
                ]
            || o.record.predecessor_state
                != witness
                    .predecessor
                    .as_ref()
                    .ok_or("outgoing predecessor absent")?
                    .state_commitment
            || o.record.successor_state != witness.successor.state_commitment
            || o.record.prepared_transition_binding_digest
                != witness.prepared_transition_binding_digest
            || o.record.projection_semantic_digest != witness.transport_semantic_digest
            || o.record.lifecycle_binding_digest != witness.lifecycle_binding_digest
            || o.record.stream_digests != [
                iroha_data_model::kagemusha::kagemusha_ordinary_sealed_transition_inputs_digest_v1(o.sealed_transition_inputs)?,
                iroha_data_model::kagemusha::kagemusha_ordinary_sealed_recovery_seeds_digest_v1(o.sealed_recovery_seeds)?,
            ]
            || o.record.stream_lengths
                != [
                    o.sealed_transition_inputs.len() as u64,
                    o.sealed_recovery_seeds.len() as u64,
                ]
        {
            return Err("ordinary Native prepared original differs from actual State".into());
        }
    }
    let transport: [AssignedValue<F>; 2] = builder.assigned_instances[0]
        [public_instance::TRANSPORT_LO..public_instance::TRANSPORT_LO + 2]
        .try_into()
        .map_err(|_| "ordinary State semantic digest absent")?;
    let range = builder.range_chip();
    let gate = range.gate();
    let ctx = builder.main(0);
    let send = gate.is_equal(ctx, assigned.operation, QuantumCell::Constant(F::from(2)));
    let redeem = gate.is_equal(ctx, assigned.operation, QuantumCell::Constant(F::from(4)));
    let enabled = gate.or(ctx, send, redeem);
    let raw_digest = |ctx: &mut Context<F>, bytes: [u8; 32]| {
        digest_limbs::<F>(bytes).map(|v| ctx.load_witness(v))
    };
    let request = raw_digest(ctx, opening.map_or([0; 32], |o| o.record.request_digest));
    let reservation = raw_digest(
        ctx,
        opening.map_or([0; 32], |o| o.record.reservation_digest),
    );
    let manifest = raw_digest(
        ctx,
        opening.map_or([0; 32], |o| o.record.artifact_manifest_digest),
    );
    let inactive = gate.not(ctx, enabled);
    // A reservation only has meaning in the real outgoing branch. Native loans separately
    // retain the actual reservation; inactive parser operands cannot carry monetary claims.
    for limb in reservation {
        let inactive_value = gate.mul(ctx, limb, inactive);
        gate.assert_is_const(ctx, &inactive_value, &F::ZERO);
    }
    let reservation_low_zero = gate.is_zero(ctx, reservation[0]);
    let reservation_high_zero = gate.is_zero(ctx, reservation[1]);
    let reservation_zero = gate.and(ctx, reservation_low_zero, reservation_high_zero);
    let missing_reservation = gate.mul(ctx, reservation_zero, enabled);
    gate.assert_is_const(ctx, &missing_reservation, &F::ZERO);
    constrain_ordinary_prepared_transition_if_v1(
        ctx,
        &range,
        jobs,
        enabled,
        KagemushaOrdinaryPreparedTransitionSourcesV1 {
            operation: assigned.operation,
            lifecycle_digest: assigned.lifecycle_binding_digest,
            request_digest: request,
            predecessor_state: assigned.predecessor_outer,
            successor_state: assigned.successor_outer,
            amount: assigned.amount,
            reservation_digest: reservation,
            preparation_operation_id: original.approval_operation_id,
            preparation_purpose: original.approval_purpose,
            state_prepared_transition_binding_digest: assigned.prepared_transition_binding_digest,
        },
    )?;
    let stream_expected: [[AssignedValue<F>; 2]; 2] = [
        prepared_carriers[2..4]
            .try_into()
            .map_err(|_| "ordinary transition carrier width")?,
        prepared_carriers[4..6]
            .try_into()
            .map_err(|_| "ordinary recovery carrier width")?,
    ];
    let mut lengths = Vec::new();
    for index in 0..2 {
        let (raw, capacity, domain) = if index == 0 {
            (
                opening.map_or(&[][..], |o| o.sealed_transition_inputs),
                2048,
                TRANSITION_STREAM_DOMAIN,
            )
        } else {
            (
                opening.map_or(&[][..], |o| o.sealed_recovery_seeds),
                512,
                RECOVERY_STREAM_DOMAIN,
            )
        };
        let native_expected = raw_digest(
            ctx,
            opening.map_or([0; 32], |o| o.record.stream_digests[index]),
        );
        for (native, carrier) in native_expected.into_iter().zip(stream_expected[index]) {
            ctx.constrain_equal(&native, &carrier);
        }
        lengths.push(constrain_sealed_stream_v1(
            ctx,
            &range,
            jobs,
            raw,
            capacity,
            domain,
            enabled,
            stream_expected[index],
        )?);
    }
    let transition_digest = digest_limbs_assigned(ctx, transition_sha);
    constrain_ordinary_prepared_intent_if_v1(
        ctx,
        &range,
        jobs,
        enabled,
        KagemushaOrdinaryPreparedIntentSourcesV1 {
            operation: assigned.operation,
            predecessor_state: assigned.predecessor_outer,
            successor_state: assigned.successor_outer,
            transition_digest,
            prepared_transition_binding_digest: assigned.prepared_transition_binding_digest,
            projection_semantic_digest: transport,
            lifecycle_binding_digest: assigned.lifecycle_binding_digest,
            request_digest: request,
            artifact_manifest_digest: manifest,
            preparation_guard_digest: assigned.guard_digest,
            reservation_digest: reservation,
            preparation_authorization_digest: original.digests[2],
            preparation_approval_purpose: original.approval_purpose,
            stream_lengths: lengths
                .try_into()
                .map_err(|_| "ordinary sealed-stream count")?,
            stream_digests: stream_expected,
            candidate_preparation_id: prepared_carriers[..2]
                .try_into()
                .map_err(|_| "ordinary preparation ID width")?,
        },
    )?;
    Ok(())
}

fn constrain_sealed_stream_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw: &[u8],
    capacity: usize,
    domain: &[u8],
    enabled: AssignedValue<F>,
    expected: [AssignedValue<F>; 2],
) -> Result<AssignedValue<F>, String> {
    if raw.len() > capacity {
        return Err("ordinary sealed stream exceeds actual fixed profile".into());
    }
    let mut padded = vec![0; capacity];
    padded[..raw.len()].copy_from_slice(raw);
    let bytes = assign_bytes(ctx, range, &padded);
    let length = ctx.load_witness(F::from(raw.len() as u64));
    let payload = KagemushaBoundedByteStreamV1::constrain(ctx, range, bytes, length)?;
    constrain_assigned_sealed_stream_digest_v1(
        ctx, range, jobs, &payload, domain, enabled, expected,
    )?;
    Ok(length)
}

fn constrain_assigned_sealed_stream_digest_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    payload: &KagemushaBoundedByteStreamV1<F>,
    domain: &[u8],
    enabled: AssignedValue<F>,
    expected: [AssignedValue<F>; 2],
) -> Result<(), String> {
    range.range_check(ctx, enabled, 1);
    let length = payload.actual_len();
    let inactive_length = range.gate().mul_not(ctx, enabled, length);
    range
        .gate()
        .assert_is_const(ctx, &inactive_length, &F::ZERO);
    let mut prefix = constant_bytes(domain);
    prefix.extend(assigned_uint_bytes_v1(ctx, range.gate(), length, 64));
    let prefix_capacity = prefix.len();
    let prefix_length = ctx.load_constant(F::from(prefix_capacity as u64));
    let prefix = KagemushaBoundedByteStreamV1::constrain(ctx, range, prefix, prefix_length)?;
    let complete = prefix.concat(ctx, range, payload, prefix_capacity + payload.bytes().len())?;
    let words =
        jobs.digest_bounded_constrained(ctx, range, complete.bytes(), complete.actual_len())?;
    let mut sha = Vec::new();
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for offset in [24, 16, 8, 0] {
            sha.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[offset..offset + 8],
            ));
        }
    }
    let sha: [PastaSha256ByteV1<F>; 32] = sha
        .try_into()
        .map_err(|_| "ordinary sealed original SHA width")?;
    for (actual, expected) in digest_limbs_assigned(ctx, &sha).into_iter().zip(expected) {
        let selected = range.gate().mul(ctx, actual, enabled);
        ctx.constrain_equal(&selected, &expected);
    }
    let empty = range.gate().is_zero(ctx, length);
    let missing = range.gate().mul(ctx, empty, enabled);
    range.gate().assert_is_const(ctx, &missing, &F::ZERO);
    Ok(())
}

#[cfg(test)]
#[path = "ordinary_state_prepared_binding_tests.rs"]
mod tests;
