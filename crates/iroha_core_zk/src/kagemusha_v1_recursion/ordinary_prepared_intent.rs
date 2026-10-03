//! Ordinary outgoing preparation transcript opening, with genuine logical authorization.
//!
//! These inputs are data only. The terminal consumer must verify the separate purpose2
//! ordinary Guard against the exact authorization/S/C columns, every history and the State
//! candidate before treating this SHA opening as authenticated. A carried preparation ID or
//! a Native-looking digest cannot replace those proof equations or the exclusive financial loan.

use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ORDINARY_PREPARED_OUTGOING_DOMAIN_V1,
    KAGEMUSHA_ORDINARY_PREPARED_TRANSITION_DOMAIN_V1,
};

use super::{
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{constant_bytes, digest_limbs_assigned, hash},
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

/// Complete assigned sources, retained from the actual candidate and separate preparation Guard.
/// No source may be assigned afresh from an arbitrary decoded ID after recursive verification.
pub(super) struct KagemushaOrdinaryPreparedIntentSourcesV1<F: KagemushaPoseidonFieldV1> {
    pub(super) operation: AssignedValue<F>,
    pub(super) predecessor_state: [AssignedValue<F>; 2],
    pub(super) successor_state: [AssignedValue<F>; 2],
    pub(super) transition_digest: [AssignedValue<F>; 2],
    pub(super) prepared_transition_binding_digest: [AssignedValue<F>; 2],
    pub(super) projection_semantic_digest: [AssignedValue<F>; 2],
    pub(super) lifecycle_binding_digest: [AssignedValue<F>; 2],
    pub(super) request_digest: [AssignedValue<F>; 2],
    pub(super) artifact_manifest_digest: [AssignedValue<F>; 2],
    pub(super) preparation_guard_digest: [AssignedValue<F>; 2],
    pub(super) reservation_digest: [AssignedValue<F>; 2],
    pub(super) preparation_authorization_digest: [PastaSha256ByteV1<F>; 32],
    pub(super) preparation_approval_purpose: AssignedValue<F>,
    pub(super) stream_lengths: [AssignedValue<F>; 2],
    /// Each length and digest must already be SHA-opened to the actual complete sealed stream.
    pub(super) stream_digests: [[AssignedValue<F>; 2]; 2],
    /// Candidate's original carried ID. Only this complete opening may give it verified meaning.
    pub(super) candidate_preparation_id: [AssignedValue<F>; 2],
}

/// Same fixed SHA graph for all ordinary State operations. Inactive results carry no authority;
/// the actual State operation selects this gate and its public prepared carriers must be zero.
pub(super) fn constrain_ordinary_prepared_intent_if_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    enabled: AssignedValue<F>,
    sources: KagemushaOrdinaryPreparedIntentSourcesV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let gate = range.gate();
    let two = ctx.load_constant(F::from(2));
    range.range_check(ctx, enabled, 1);
    let purpose_delta = gate.sub(ctx, sources.preparation_approval_purpose, two);
    let selected_delta = gate.mul(ctx, purpose_delta, enabled);
    gate.assert_is_const(ctx, &selected_delta, &F::ZERO);
    range.range_check(ctx, sources.operation, 8);
    let send = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(2)));
    let redeem = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(4)));
    let outgoing = gate.or(ctx, send, redeem);
    let missing_outgoing = gate.not(ctx, outgoing);
    let invalid_operation = gate.mul(ctx, missing_outgoing, enabled);
    gate.assert_is_const(ctx, &invalid_operation, &F::ZERO);
    // Request is nonzero exactly for SendSplit; authenticated manifest exactly for RedeemSplit.
    for (digest, present) in [
        (sources.request_digest, send),
        (sources.artifact_manifest_digest, redeem),
    ] {
        let low_zero = gate.is_zero(ctx, digest[0]);
        let high_zero = gate.is_zero(ctx, digest[1]);
        let zero = gate.and(ctx, low_zero, high_zero);
        let actual = gate.not(ctx, zero);
        let selected_presence = gate.mul(ctx, present, enabled);
        ctx.constrain_equal(&actual, &selected_presence);
    }
    let mut bytes = constant_bytes(KAGEMUSHA_ORDINARY_PREPARED_OUTGOING_DOMAIN_V1);
    bytes.extend(constant_bytes(&1_u16.to_le_bytes()));
    bytes.extend(assigned_uint_bytes_v1(ctx, gate, sources.operation, 8));
    for digest in [
        sources.predecessor_state,
        sources.successor_state,
        sources.transition_digest,
        sources.prepared_transition_binding_digest,
        sources.projection_semantic_digest,
        sources.lifecycle_binding_digest,
        sources.request_digest,
        sources.artifact_manifest_digest,
        sources.preparation_guard_digest,
        sources.reservation_digest,
    ] {
        bytes.extend(assigned_digest_bytes_v1(ctx, gate, digest));
    }
    let mut auth_sum = ctx.load_zero();
    for byte in sources.preparation_authorization_digest {
        let actual = byte
            .assigned()
            .ok_or("preparation authorization original SHA byte absent")?;
        range.range_check(ctx, actual, 8);
        auth_sum = gate.add(ctx, auth_sum, actual);
        bytes.push(byte);
    }
    let auth_zero = gate.is_zero(ctx, auth_sum);
    let missing_authorization = gate.mul(ctx, auth_zero, enabled);
    gate.assert_is_const(ctx, &missing_authorization, &F::ZERO);
    for (index, (length, digest)) in sources
        .stream_lengths
        .into_iter()
        .zip(sources.stream_digests)
        .enumerate()
    {
        range.range_check(ctx, length, 64);
        let empty = gate.is_zero(ctx, length);
        let missing_stream = gate.mul(ctx, empty, enabled);
        gate.assert_is_const(ctx, &missing_stream, &F::ZERO);
        let maximum = if index == 0 {
            iroha_data_model::kagemusha::KAGEMUSHA_SEALED_TRANSITION_INPUTS_MAX_BYTES_V1
        } else {
            iroha_data_model::kagemusha::KAGEMUSHA_RECOVERY_SEEDS_MAX_BYTES_V1
        };
        let in_profile = range.is_less_than(
            ctx,
            length,
            QuantumCell::Constant(F::from(u64::from(maximum) + 1)),
            64,
        );
        gate.assert_is_const(ctx, &in_profile, &F::ONE);
        bytes.extend(assigned_uint_bytes_v1(ctx, gate, length, 64));
        bytes.extend(assigned_digest_bytes_v1(ctx, gate, digest));
    }
    let actual = hash(ctx, jobs, bytes)?;
    let actual_limbs = digest_limbs_assigned(ctx, &actual);
    for (actual, expected) in actual_limbs
        .into_iter()
        .zip(sources.candidate_preparation_id)
    {
        let selected_hash = gate.mul(ctx, actual, enabled);
        ctx.constrain_equal(&selected_hash, &expected);
    }
    Ok(actual)
}

/// Same assigned State scope and signed purpose2 W operation ID, not detached host digest data.
pub(super) struct KagemushaOrdinaryPreparedTransitionSourcesV1<F: KagemushaPoseidonFieldV1> {
    pub(super) operation: AssignedValue<F>,
    pub(super) lifecycle_digest: [AssignedValue<F>; 2],
    pub(super) request_digest: [AssignedValue<F>; 2],
    pub(super) predecessor_state: [AssignedValue<F>; 2],
    pub(super) successor_state: [AssignedValue<F>; 2],
    pub(super) amount: AssignedValue<F>,
    pub(super) reservation_digest: [AssignedValue<F>; 2],
    /// Actual original W bytes already copy-bound to the Native operation ID.
    pub(super) preparation_operation_id: [PastaSha256ByteV1<F>; 32],
    pub(super) preparation_purpose: AssignedValue<F>,
    /// Same State field included in the transition SHA, normalized preparation Guard and ID.
    pub(super) state_prepared_transition_binding_digest: [AssignedValue<F>; 2],
}
/// Same fixed SHA graph for all ordinary State operations. Inactive results carry no authority;
/// the actual State operation selects this gate and its public prepared carriers must be zero.
pub(super) fn constrain_ordinary_prepared_transition_if_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    enabled: AssignedValue<F>,
    sources: KagemushaOrdinaryPreparedTransitionSourcesV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let gate = range.gate();
    let two = ctx.load_constant(F::from(2));
    range.range_check(ctx, enabled, 1);
    let purpose_delta = gate.sub(ctx, sources.preparation_purpose, two);
    let selected_delta = gate.mul(ctx, purpose_delta, enabled);
    gate.assert_is_const(ctx, &selected_delta, &F::ZERO);
    range.range_check(ctx, sources.operation, 8);
    let send = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(2)));
    let redeem = gate.is_equal(ctx, sources.operation, QuantumCell::Constant(F::from(4)));
    let outgoing = gate.or(ctx, send, redeem);
    let missing_outgoing = gate.not(ctx, outgoing);
    let invalid_operation = gate.mul(ctx, missing_outgoing, enabled);
    gate.assert_is_const(ctx, &invalid_operation, &F::ZERO);
    let request_low_zero = gate.is_zero(ctx, sources.request_digest[0]);
    let request_high_zero = gate.is_zero(ctx, sources.request_digest[1]);
    let request_zero = gate.and(ctx, request_low_zero, request_high_zero);
    let request_present = gate.not(ctx, request_zero);
    let expected_request = gate.mul(ctx, send, enabled);
    ctx.constrain_equal(&request_present, &expected_request);
    range.range_check(ctx, sources.amount, 128);
    let amount_zero = gate.is_zero(ctx, sources.amount);
    let missing_amount = gate.mul(ctx, amount_zero, enabled);
    gate.assert_is_const(ctx, &missing_amount, &F::ZERO);
    let mut bytes = constant_bytes(KAGEMUSHA_ORDINARY_PREPARED_TRANSITION_DOMAIN_V1);
    bytes.extend(constant_bytes(&1_u16.to_le_bytes()));
    bytes.extend(assigned_uint_bytes_v1(ctx, gate, sources.operation, 8));
    for digest in [
        sources.lifecycle_digest,
        sources.request_digest,
        sources.predecessor_state,
        sources.successor_state,
    ] {
        bytes.extend(assigned_digest_bytes_v1(ctx, gate, digest));
    }
    bytes.extend(assigned_uint_bytes_v1(ctx, gate, sources.amount, 128));
    bytes.extend(assigned_digest_bytes_v1(
        ctx,
        gate,
        sources.reservation_digest,
    ));
    let mut operation_sum = ctx.load_zero();
    for byte in sources.preparation_operation_id {
        let cell = byte
            .assigned()
            .ok_or("original Native preparation operation byte absent")?;
        range.range_check(ctx, cell, 8);
        operation_sum = gate.add(ctx, operation_sum, cell);
        bytes.push(byte);
    }
    let operation_zero = gate.is_zero(ctx, operation_sum);
    let missing_native_operation = gate.mul(ctx, operation_zero, enabled);
    gate.assert_is_const(ctx, &missing_native_operation, &F::ZERO);
    let actual = hash(ctx, jobs, bytes)?;
    let limbs = digest_limbs_assigned(ctx, &actual);
    for (actual, expected) in limbs
        .into_iter()
        .zip(sources.state_prepared_transition_binding_digest)
    {
        let selected_hash = gate.mul(ctx, actual, enabled);
        ctx.constrain_equal(&selected_hash, &expected);
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{kagemusha_v1_poseidon::digest_limbs, pasta_sha256::PastaSha256ConfigV1};
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use iroha_data_model::kagemusha::{
        KagemushaOrdinaryPreparedOutgoingV1, KagemushaOrdinaryPreparedTransitionV1,
    };

    const K: usize = 16;
    const UNUSABLE: usize = 9;
    #[derive(Clone)]
    struct TestCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }
    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for TestCircuit<F> {
        type Config = (BaseConfig<F>, PastaSha256ConfigV1);
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;
        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }
        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }
        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows((1 << K) - UNUSABLE);
            (base, PastaSha256ConfigV1::configure(meta))
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("ordinary prepared transcript has fixed params")
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            self.builder
                .synthesize(config.0, layouter.namespace(|| "ordinary preparation base"))?;
            self.jobs.synthesize(
                &config.1,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1 << K) - UNUSABLE,
            )
        }
    }
    fn values(operation: u8) -> KagemushaOrdinaryPreparedOutgoingV1 {
        KagemushaOrdinaryPreparedOutgoingV1 {
            version: 1,
            operation,
            predecessor_state: [1; 32],
            successor_state: [2; 32],
            transition_digest: [3; 32],
            prepared_transition_binding_digest: [4; 32],
            projection_semantic_digest: [5; 32],
            lifecycle_binding_digest: [6; 32],
            request_digest: if operation == 2 { [7; 32] } else { [0; 32] },
            artifact_manifest_digest: if operation == 4 { [8; 32] } else { [0; 32] },
            preparation_guard_digest: [9; 32],
            reservation_digest: [10; 32],
            preparation_authorization_digest: [11; 32],
            stream_lengths: [17, 19],
            stream_digests: [[12; 32], [13; 32]],
        }
    }
    // Each case keeps the original candidate ID unchanged. Sources are mathematical upstream
    // fixtures; complete sealed-stream, platform, issuer and Native custody are not claimed here.
    fn check<F: KagemushaPoseidonFieldV1>(operation: u8, mutation: Option<usize>) -> bool {
        check_mode::<F>(operation, mutation, true)
    }
    fn check_mode<F: KagemushaPoseidonFieldV1>(
        operation: u8,
        mutation: Option<usize>,
        enabled: bool,
    ) -> bool {
        let original = values(operation);
        let mut actual = original;
        if !enabled {
            actual.request_digest = [0; 32];
            actual.artifact_manifest_digest = [0; 32];
            actual.stream_lengths = [0; 2];
            actual.stream_digests = [[0; 32]; 2];
            actual.reservation_digest = [0; 32];
        }
        match mutation {
            Some(0) => actual.predecessor_state[0] ^= 1,
            Some(1) => actual.successor_state[0] ^= 1,
            Some(2) => actual.transition_digest[0] ^= 1,
            Some(3) => actual.prepared_transition_binding_digest[0] ^= 1,
            Some(4) => actual.projection_semantic_digest[0] ^= 1,
            Some(5) => actual.lifecycle_binding_digest[0] ^= 1,
            Some(6) => actual.request_digest[0] ^= 1,
            Some(7) => actual.artifact_manifest_digest[0] ^= 1,
            Some(8) => actual.preparation_guard_digest[0] ^= 1,
            Some(9) => actual.reservation_digest[0] ^= 1,
            Some(10) => actual.preparation_authorization_digest[0] ^= 1,
            Some(11) => actual.stream_lengths[0] += 1,
            Some(12) => actual.stream_lengths[1] += 1,
            Some(13) => actual.stream_digests[0][0] ^= 1,
            Some(14) => actual.stream_digests[1][0] ^= 1,
            Some(15) => actual.operation = 1,
            Some(17) => actual.stream_lengths[0] = 0,
            Some(18) => actual.stream_lengths[1] = 513,
            Some(19) => actual.preparation_authorization_digest = [0; 32],
            _ => {}
        }
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1)
            .use_instance_columns(0);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let dig = |ctx: &mut Context<F>, bytes: [u8; 32]| {
            digest_limbs::<F>(bytes).map(|value| ctx.load_witness(value))
        };
        let authorization = actual.preparation_authorization_digest.map(|value| {
            let cell = ctx.load_witness(F::from(u64::from(value)));
            PastaSha256ByteV1::range_checked(ctx, &range, cell)
        });
        let sources = KagemushaOrdinaryPreparedIntentSourcesV1 {
            operation: ctx.load_witness(F::from(u64::from(actual.operation))),
            predecessor_state: dig(ctx, actual.predecessor_state),
            successor_state: dig(ctx, actual.successor_state),
            transition_digest: dig(ctx, actual.transition_digest),
            prepared_transition_binding_digest: dig(ctx, actual.prepared_transition_binding_digest),
            projection_semantic_digest: dig(ctx, actual.projection_semantic_digest),
            lifecycle_binding_digest: dig(ctx, actual.lifecycle_binding_digest),
            request_digest: dig(ctx, actual.request_digest),
            artifact_manifest_digest: dig(ctx, actual.artifact_manifest_digest),
            preparation_guard_digest: dig(ctx, actual.preparation_guard_digest),
            reservation_digest: dig(ctx, actual.reservation_digest),
            preparation_authorization_digest: authorization,
            preparation_approval_purpose: ctx.load_witness(F::from(if mutation == Some(16) {
                1
            } else {
                2
            })),
            stream_lengths: actual
                .stream_lengths
                .map(|length| ctx.load_witness(F::from(length))),
            stream_digests: actual.stream_digests.map(|bytes| dig(ctx, bytes)),
            candidate_preparation_id: dig(
                ctx,
                if enabled {
                    original.binding_transcript().digest()
                } else if mutation == Some(20) {
                    [1; 32]
                } else {
                    [0; 32]
                },
            ),
        };
        let mut jobs = PastaSha256JobsV1::default();
        let selected = ctx.load_witness(if enabled { F::ONE } else { F::ZERO });
        constrain_ordinary_prepared_intent_if_v1(ctx, &range, &mut jobs, selected, sources)
            .unwrap();
        assert_eq!(jobs.typed_claim_jobs().unwrap().len(), 1);
        builder.calculate_params(Some(UNUSABLE));
        MockProver::run(K as u32, &TestCircuit { builder, jobs }, vec![])
            .expect("ordinary complete preparation transcript fits")
            .verify()
            .is_ok()
    }
    #[test]
    fn ordinary_prepared_intent_full_sha_matches_native_send_and_redeem_both_fields() {
        for operation in [2, 4] {
            assert!(check::<Fp>(operation, None));
            assert!(check::<Fq>(operation, None));
        }
    }
    #[test]
    fn ordinary_prepared_intent_rejects_substituted_financial_original_length_or_sha() {
        for operation in [2, 4] {
            for changed in 0..20 {
                assert!(
                    !check::<Fp>(operation, Some(changed)),
                    "Fp operation {operation}, field {changed}"
                );
                assert!(
                    !check::<Fq>(operation, Some(changed)),
                    "Fq operation {operation}, field {changed}"
                );
            }
        }
    }
    #[test]
    fn ordinary_prepared_intent_wire_has_distinct_fixed_domain_and_exact_field_order() {
        let value = values(2);
        let bytes = value.binding_transcript().bytes;
        let prefix = KAGEMUSHA_ORDINARY_PREPARED_OUTGOING_DOMAIN_V1.len();
        assert_eq!(
            &bytes[..prefix],
            KAGEMUSHA_ORDINARY_PREPARED_OUTGOING_DOMAIN_V1
        );
        assert_eq!(&bytes[prefix..prefix + 3], &[1, 0, 2]);
        assert_eq!(bytes.len(), prefix + 3 + 11 * 32 + 2 * (8 + 32));
        assert_eq!(
            &bytes[prefix + 3 + 10 * 32..prefix + 3 + 11 * 32],
            &value.preparation_authorization_digest
        );
        assert_eq!(
            &bytes[prefix + 3 + 11 * 32..prefix + 3 + 11 * 32 + 8],
            &17_u64.to_le_bytes()
        );
        assert_ne!(
            KAGEMUSHA_ORDINARY_PREPARED_OUTGOING_DOMAIN_V1,
            b"iroha:kagemusha:v1:outgoing-preparation\0"
        );
    }
    fn transition_values(operation: u8) -> KagemushaOrdinaryPreparedTransitionV1 {
        KagemushaOrdinaryPreparedTransitionV1 {
            version: 1,
            operation,
            lifecycle_digest: [1; 32],
            request_digest: if operation == 2 { [2; 32] } else { [0; 32] },
            predecessor_state: [3; 32],
            successor_state: [4; 32],
            amount: (1_u128 << 70) + 7,
            reservation_digest: [5; 32],
            native_preparation_operation_id: [6; 32],
        }
    }
    fn transition_check<F: KagemushaPoseidonFieldV1>(
        operation: u8,
        mutation: Option<usize>,
    ) -> bool {
        transition_check_mode::<F>(operation, mutation, true)
    }
    fn transition_check_mode<F: KagemushaPoseidonFieldV1>(
        operation: u8,
        mutation: Option<usize>,
        enabled: bool,
    ) -> bool {
        let original = transition_values(operation);
        let mut actual = original;
        if !enabled {
            actual.request_digest = [0; 32];
        }
        match mutation {
            Some(0) => actual.lifecycle_digest[0] ^= 1,
            Some(1) => actual.request_digest[0] ^= 1,
            Some(2) => actual.predecessor_state[0] ^= 1,
            Some(3) => actual.successor_state[0] ^= 1,
            Some(4) => actual.amount += 1,
            Some(5) => actual.reservation_digest[0] ^= 1,
            Some(6) => actual.native_preparation_operation_id[0] ^= 1,
            Some(7) => actual.operation = 1,
            Some(9) => actual.native_preparation_operation_id = [0; 32],
            Some(10) => actual.amount = 0,
            _ => {}
        }
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K)
            .use_lookup_bits(K - 1)
            .use_instance_columns(0);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let dig = |ctx: &mut Context<F>, bytes: [u8; 32]| {
            digest_limbs::<F>(bytes).map(|value| ctx.load_witness(value))
        };
        let id = actual.native_preparation_operation_id.map(|byte| {
            let cell = ctx.load_witness(F::from(u64::from(byte)));
            PastaSha256ByteV1::range_checked(ctx, &range, cell)
        });
        let sources = KagemushaOrdinaryPreparedTransitionSourcesV1 {
            operation: ctx.load_witness(F::from(u64::from(actual.operation))),
            lifecycle_digest: dig(ctx, actual.lifecycle_digest),
            request_digest: dig(ctx, actual.request_digest),
            predecessor_state: dig(ctx, actual.predecessor_state),
            successor_state: dig(ctx, actual.successor_state),
            amount: ctx.load_witness(crate::kagemusha_v1_poseidon::from_u128::<F>(actual.amount)),
            reservation_digest: dig(ctx, actual.reservation_digest),
            preparation_operation_id: id,
            preparation_purpose: ctx.load_witness(F::from(if mutation == Some(8) { 1 } else { 2 })),
            state_prepared_transition_binding_digest: dig(
                ctx,
                if enabled {
                    original.binding_transcript().digest()
                } else if mutation == Some(11) {
                    [1; 32]
                } else {
                    [0; 32]
                },
            ),
        };
        let mut jobs = PastaSha256JobsV1::default();
        let selected = ctx.load_witness(if enabled { F::ONE } else { F::ZERO });
        constrain_ordinary_prepared_transition_if_v1(ctx, &range, &mut jobs, selected, sources)
            .unwrap();
        assert_eq!(jobs.typed_claim_jobs().unwrap().len(), 1);
        builder.calculate_params(Some(UNUSABLE));
        MockProver::run(K as u32, &TestCircuit { builder, jobs }, vec![])
            .expect("ordinary acyclic transition transcript fits")
            .verify()
            .is_ok()
    }
    #[test]
    fn ordinary_prepared_transition_both_fields_native_id_precedes_signed_approval() {
        for operation in [2, 4] {
            assert!(transition_check::<Fp>(operation, None));
            assert!(transition_check::<Fq>(operation, None));
            let original = transition_values(operation);
            let bytes = original.binding_transcript().bytes;
            assert_eq!(
                bytes.len(),
                KAGEMUSHA_ORDINARY_PREPARED_TRANSITION_DOMAIN_V1.len() + 3 + 6 * 32 + 16
            );
            assert_eq!(
                &bytes[bytes.len() - 32..],
                &original.native_preparation_operation_id
            );
        }
    }
    #[test]
    fn ordinary_prepared_transition_rejects_substituted_financial_scope_or_late_purpose() {
        for operation in [2, 4] {
            for changed in 0..11 {
                assert!(
                    !transition_check::<Fp>(operation, Some(changed)),
                    "Fp operation {operation}, field {changed}"
                );
                assert!(
                    !transition_check::<Fq>(operation, Some(changed)),
                    "Fq operation {operation}, field {changed}"
                );
            }
        }
    }

    #[test]
    fn inactive_state_operations_queue_same_full_preparation_transcripts_both_fields() {
        for operation in [0, 1, 3, 5] {
            assert!(check_mode::<Fp>(operation, None, false));
            assert!(check_mode::<Fq>(operation, None, false));
            assert!(transition_check_mode::<Fp>(operation, None, false));
            assert!(transition_check_mode::<Fq>(operation, None, false));
        }
    }
    #[test]
    fn inactive_state_preparation_hashes_cannot_supply_nonzero_public_carriers() {
        for operation in [0, 1, 3, 5] {
            assert!(!check_mode::<Fp>(operation, Some(20), false));
            assert!(!check_mode::<Fq>(operation, Some(20), false));
            assert!(!transition_check_mode::<Fp>(operation, Some(11), false));
            assert!(!transition_check_mode::<Fq>(operation, Some(11), false));
        }
    }
}
