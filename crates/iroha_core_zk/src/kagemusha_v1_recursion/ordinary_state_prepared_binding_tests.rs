//! Complete sealed-stream SHA openings and inactive-carrier refusal in both Pasta fields.
//!
//! These pure mathematical fixtures grant no Native approval or financial custody.

use super::*;
use crate::pasta_sha256::{PastaSha256ConfigV1, PastaSha256PlanMessageV1};
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use sha2::{Digest as _, Sha256};

const K: usize = 17;
const UNUSABLE: usize = 9;
#[derive(Clone)]
struct StreamCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for StreamCircuit<F> {
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
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows((1 << K) - UNUSABLE);
        (base, PastaSha256ConfigV1::configure(meta))
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("fixed sealed profile")
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.builder
            .synthesize(config.0, layouter.namespace(|| "sealed stream Base"))?;
        self.jobs.synthesize(
            &config.1,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1 << K) - UNUSABLE,
        )
    }
}
#[derive(Clone, Copy)]
enum Mutation {
    Byte,
    Length,
    Tail,
    Digest,
    InactiveCarrier,
    InactiveLength,
}

fn circuit<F: KagemushaPoseidonFieldV1>(
    capacity: usize,
    domain: &[u8],
    length: usize,
    enabled: bool,
    mutation: Option<Mutation>,
) -> StreamCircuit<F> {
    assert!(length <= capacity);
    let mut raw = vec![0; capacity];
    for (i, byte) in raw[..length].iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(37).wrapping_add(11);
    }
    let mut native = domain.to_vec();
    native.extend((length as u64).to_le_bytes());
    native.extend(&raw[..length]);
    let expected: [u8; 32] = if enabled {
        Sha256::digest(&native).into()
    } else if matches!(mutation, Some(Mutation::InactiveCarrier)) {
        [1; 32]
    } else {
        [0; 32]
    };
    let mut actual_length = length;
    match mutation {
        Some(Mutation::Byte) => raw[length - 1] ^= 1,
        Some(Mutation::Length) => {
            raw[length - 1] = 0;
            actual_length -= 1;
        }
        Some(Mutation::Tail) => raw[length] = 1,
        Some(Mutation::InactiveLength) => {
            actual_length = 1;
            raw[0] = 0;
        }
        _ => {}
    }
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K)
        .use_lookup_bits(16)
        .use_instance_columns(0);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let padded = assign_bytes(ctx, &range, &raw);
    let actual_length = ctx.load_witness(F::from(actual_length as u64));
    let stream =
        KagemushaBoundedByteStreamV1::constrain(ctx, &range, padded, actual_length).unwrap();
    let selected = ctx.load_witness(if enabled { F::ONE } else { F::ZERO });
    let mut expected = digest_limbs::<F>(expected);
    if matches!(mutation, Some(Mutation::Digest)) {
        expected[0] += F::ONE;
    }
    let expected = expected.map(|value| ctx.load_witness(value));
    let mut jobs = PastaSha256JobsV1::default();
    constrain_assigned_sealed_stream_digest_v1(
        ctx, &range, &mut jobs, &stream, domain, selected, expected,
    )
    .unwrap();
    assert_eq!(jobs.typed_claim_jobs().unwrap().len(), 1);
    if mutation.is_none() {
        match jobs.canonical_plan_messages().unwrap().as_slice() {
            [
                PastaSha256PlanMessageV1::Bounded {
                    logical_message,
                    capacity: actual_capacity,
                    ..
                },
            ] => {
                assert_eq!(logical_message, &native);
                assert_eq!(*actual_capacity, domain.len() + 8 + capacity);
            }
            _ => panic!("sealed stream must retain its complete bounded SHA geometry"),
        }
    }
    builder.calculate_params(Some(UNUSABLE));
    StreamCircuit { builder, jobs }
}
fn check<F: KagemushaPoseidonFieldV1>(
    capacity: usize,
    domain: &[u8],
    length: usize,
    enabled: bool,
    mutation: Option<Mutation>,
) -> bool {
    MockProver::run(
        K as u32,
        &circuit::<F>(capacity, domain, length, enabled, mutation),
        vec![],
    )
    .expect("complete fixed sealed-stream profile fits")
    .verify()
    .is_ok()
}
#[test]
fn complete_original_sealed_stream_sha_matches_native_both_fields_and_profiles() {
    for (capacity, domain) in [
        (2048, TRANSITION_STREAM_DOMAIN),
        (512, RECOVERY_STREAM_DOMAIN),
    ] {
        assert!(check::<Fp>(capacity, domain, capacity, true, None));
        assert!(check::<Fq>(capacity, domain, capacity, true, None));
    }
}
#[test]
fn sealed_stream_rejects_changed_original_length_tail_or_digest_both_fields() {
    for mutation in [
        Mutation::Byte,
        Mutation::Length,
        Mutation::Tail,
        Mutation::Digest,
    ] {
        assert!(!check::<Fp>(
            2048,
            TRANSITION_STREAM_DOMAIN,
            55,
            true,
            Some(mutation)
        ));
        assert!(!check::<Fq>(
            2048,
            TRANSITION_STREAM_DOMAIN,
            55,
            true,
            Some(mutation)
        ));
    }
}
#[test]
fn inactive_sealed_stream_keeps_same_sha_job_and_cannot_carry_nonzero_claims() {
    for (capacity, domain) in [
        (2048, TRANSITION_STREAM_DOMAIN),
        (512, RECOVERY_STREAM_DOMAIN),
    ] {
        assert!(check::<Fp>(capacity, domain, 0, false, None));
        assert!(check::<Fq>(capacity, domain, 0, false, None));
        for mutation in [Mutation::InactiveCarrier, Mutation::InactiveLength] {
            assert!(!check::<Fp>(capacity, domain, 0, false, Some(mutation)));
            assert!(!check::<Fq>(capacity, domain, 0, false, Some(mutation)));
        }
        assert!(!check::<Fp>(capacity, domain, 0, true, None));
        assert!(!check::<Fq>(capacity, domain, 0, true, None));
    }
}
#[test]
fn sealed_stream_exact_logical_messages_and_fixed_columns_cross_sha_boundaries() {
    for (capacity, domain) in [
        (2048, TRANSITION_STREAM_DOMAIN),
        (512, RECOVERY_STREAM_DOMAIN),
    ] {
        let initial = circuit::<Fp>(capacity, domain, 1, true, None);
        for length in [1, 7, 15, 16, 55, 56, 63, 64, 65, capacity] {
            let actual = circuit::<Fp>(capacity, domain, length, true, None);
            assert_eq!(
                actual.builder.config_params.num_advice_per_phase,
                initial.builder.config_params.num_advice_per_phase
            );
            assert_eq!(
                actual.builder.config_params.num_lookup_advice_per_phase,
                initial.builder.config_params.num_lookup_advice_per_phase
            );
            assert_eq!(
                actual.builder.config_params.num_fixed,
                initial.builder.config_params.num_fixed
            );
            assert_eq!(
                actual.jobs.compression_blocks().unwrap(),
                initial.jobs.compression_blocks().unwrap()
            );
            assert_eq!(
                actual
                    .builder
                    .core()
                    .copy_manager
                    .lock()
                    .unwrap()
                    .advice_equalities,
                initial
                    .builder
                    .core()
                    .copy_manager
                    .lock()
                    .unwrap()
                    .advice_equalities
            );
        }
    }
}
