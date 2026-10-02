//! Actual whole-original SHA kernel tests, separate from full recursive Redeem qualification.
use super::*;
use crate::pasta_sha256::PastaSha256ConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use sha2::{Digest as _, Sha256};
const K: u32 = 16;
#[derive(Clone)]
struct WholeOriginalCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for WholeOriginalCircuit<F> {
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
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("explicit Redeem original Base params")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, p: Self::Params) -> Self::Config {
        let mut b = BaseConfig::configure(meta, p);
        b.set_usable_rows((1 << K) - 9);
        (b, PastaSha256ConfigV1::configure(meta))
    }
    fn synthesize(&self, c: Self::Config, mut l: impl Layouter<F>) -> Result<(), Error> {
        self.builder
            .synthesize(c.0, l.namespace(|| "Redeem whole original Base"))?;
        self.jobs.synthesize(
            &c.1,
            &mut l,
            &self.builder.core().copy_manager,
            (1 << K) - 9,
        )
    }
}
fn check<F: KagemushaPoseidonFieldV1>(domain: &[u8], mutation: usize, active: bool) -> bool {
    let original = if active {
        vec![0x21, 0x32, 0x43, 0x54, 0x65, 0x76, 0x87]
    } else {
        vec![]
    };
    let mut full = domain.to_vec();
    full.extend_from_slice(&(original.len() as u64).to_le_bytes());
    full.extend_from_slice(&original);
    let expected: [u8; 32] = if active {
        Sha256::digest(full).into()
    } else {
        [0; 32]
    };
    let mut raw = original;
    if mutation == 1 && active {
        raw[0] ^= 1;
    }
    if mutation == 2 && active {
        raw[6] ^= 1;
    }
    if mutation == 3 {
        raw.push(0x98);
    }
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K as usize)
        .use_lookup_bits(15)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let enabled = ctx.load_witness(F::from(u64::from(active)));
    let stream = raw_original(ctx, &range, &raw, 16, enabled).unwrap();
    let mut jobs = PastaSha256JobsV1::default();
    let derived = whole_original_digest(ctx, &range, &mut jobs, domain, &stream).unwrap();
    let derived = selected(ctx, &range, enabled, derived);
    let expected = assign_bytes(ctx, &range, &expected).try_into().unwrap();
    equal(ctx, &range, derived, expected);
    super::super::base_packing::finalize_base_params_v1(&mut builder, 9).unwrap();
    jobs.validate_capacity((1 << K) - 9).unwrap();
    let c = WholeOriginalCircuit { builder, jobs };
    MockProver::run(K, &c, vec![vec![]])
        .unwrap()
        .verify()
        .is_ok()
}
fn all<F: KagemushaPoseidonFieldV1>() {
    for domain in [
        b"iroha:kagemusha:v1:manifest\0".as_slice(),
        b"iroha:kagemusha:v1:app-approval-account\0".as_slice(),
    ] {
        for mutation in 0..4 {
            assert_eq!(check::<F>(domain, mutation, true), mutation == 0);
        }
        assert!(check::<F>(domain, 0, false));
        assert!(!check::<F>(domain, 3, false));
    }
}
#[test]
fn whole_manifest_and_account_maintain_raw_domain_length_and_every_original_byte_in_both_fields() {
    std::thread::Builder::new()
        .name("Redeem-whole-original-kernel".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            all::<Fp>();
            all::<Fq>();
        })
        .unwrap()
        .join()
        .unwrap();
}
#[test]
fn full_original_capacity_refuses_oversized_data_before_witness_allocation() {
    let mut b = BaseCircuitBuilder::<Fp>::new(false)
        .use_k(K as usize)
        .use_lookup_bits(15);
    let range = b.range_chip();
    let ctx = b.main(0);
    let e = ctx.load_constant(Fp::from(1));
    assert!(raw_original(ctx, &range, &vec![1; ACCOUNT_MAX + 1], ACCOUNT_MAX, e).is_err());
    assert!(
        raw_original(
            ctx,
            &range,
            &vec![1; KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1 + 1],
            KAGEMUSHA_RELEASE_MANIFEST_MAX_BYTES_V1,
            e
        )
        .is_err()
    );
}
