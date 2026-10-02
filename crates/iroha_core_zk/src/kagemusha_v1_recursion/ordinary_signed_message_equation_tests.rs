//! Real mathematical P256/SHA original-equation tests over a distinct receiver-sized message.
//! Public fixture signing keys create no installed release, Native custody or receiver authority.
use super::*;
use crate::{
    kagemusha_v1_recursion::base_packing::finalize_base_params_v1,
    pasta_sha256::PastaSha256ConfigV1,
};
use ff::{Field as _, PrimeField};
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
use halo2_ecc::{
    ecc::EcPoint,
    fields::{FieldChip as _, fp::FpChip},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::{
        pasta::{Fp, Fq},
        secp256r1::{Fp as P256Base, Fq as P256Scalar},
    },
    plonk::{Circuit, ConstraintSystem, Error},
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};
#[derive(Clone)]
struct MessageCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
#[derive(Clone, Debug)]
struct Config<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for MessageCircuit<F> {
    type Config = Config<F>;
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
        unreachable!("test requires explicit Base params")
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let usable = (1usize << params.k) - 9;
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(usable);
        Config {
            base,
            sha: PastaSha256ConfigV1::configure(meta),
        }
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.builder
            .synthesize(config.base, layouter.namespace(|| "distinct message Base"))?;
        self.jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1usize << self.builder.config_params.k) - 9,
        )
    }
}
fn field_be<T: PrimeField>(bytes: &[u8]) -> T {
    let mut repr = T::Repr::default();
    for (a, b) in repr.as_mut().iter_mut().zip(bytes.iter().rev()) {
        *a = *b
    }
    Option::<T>::from(T::from_repr(repr)).unwrap()
}
#[derive(Clone, Copy)]
enum Mutation {
    None,
    Message,
    OriginalDer,
    EnrolledKey,
    CounterFloor,
}
fn build<F: KagemushaPoseidonFieldV1>(apple: bool, mutation: Mutation) -> MessageCircuit<F> {
    let signing = SigningKey::from_bytes((&[7u8; 32]).into()).unwrap();
    let public = signing.verifying_key().to_encoded_point(false);
    let key_raw = public.as_bytes();
    let original_message = (0..448)
        .map(|i| (i as u8).wrapping_mul(13))
        .collect::<Vec<_>>();
    let mut auth = [2u8; 37];
    auth[32] = 0x40;
    auth[33..].copy_from_slice(&17u32.to_be_bytes());
    let signature_message = if apple {
        let mut b = auth.to_vec();
        b.extend(Sha256::digest(&original_message));
        Sha256::digest(b).to_vec()
    } else {
        original_message.clone()
    };
    let signature: Signature = signing.sign(&signature_message);
    let mut der = signature.to_der().as_bytes().to_vec();
    let r = field_be::<P256Scalar>(&signature.r().to_bytes());
    let s = field_be::<P256Scalar>(&signature.s().to_bytes());
    let digest: [u8; 32] = Sha256::digest(&signature_message).into();
    let z = digest.iter().fold(P256Scalar::ZERO, |a, b| {
        a * P256Scalar::from(256) + P256Scalar::from(u64::from(*b))
    });
    let mut little = digest;
    little.reverse();
    let quotient = u64::from(Option::<P256Scalar>::from(P256Scalar::from_repr(little)).is_none());
    let mut message = original_message;
    if matches!(mutation, Mutation::Message) {
        message[390] ^= 1
    }
    if matches!(mutation, Mutation::OriginalDer) {
        let last = der.len() - 1;
        der[last] ^= 1
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(16)
        .use_lookup_bits(15)
        .use_instance_columns(1);
    let mut jobs = PastaSha256JobsV1::default();
    let range = builder.range_chip();
    let base = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let scalar = FpChip::<F, P256Scalar>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let ctx = builder.main(0);
    let key = EcPoint::new(
        base.load_private(ctx, field_be(&key_raw[1..33])),
        base.load_private(ctx, field_be(&key_raw[33..])),
    );
    let mut enrolled = key_raw.to_vec();
    if matches!(mutation, Mutation::EnrolledKey) {
        enrolled[1] ^= 1
    }
    let enrolled = core::array::from_fn(|i| ctx.load_witness(F::from(u64::from(enrolled[i]))));
    let r = scalar.load_private(ctx, r);
    let s = scalar.load_private(ctx, s);
    let z = scalar.load_private(ctx, z);
    let cells = OrdinaryPlatformSignatureCellsV1 {
        signature_public_key: &key,
        enrolled_public_key: &key,
        enrolled_public_key_sec1: &enrolled,
        r: &r,
        s: &s,
        z: &z,
        digest_reduction_quotient: ctx.load_witness(F::from(quotient)),
    };
    let message = message
        .iter()
        .map(|b| {
            let b = ctx.load_witness(F::from(u64::from(*b)));
            PastaSha256ByteV1::range_checked(ctx, &range, b)
        })
        .collect::<Vec<_>>();
    if apple {
        let auth_cells = auth.map(|b| ctx.load_witness(F::from(u64::from(b))));
        let rp = core::array::from_fn(|i| ctx.load_constant(F::from(u64::from(auth[i]))));
        let release = [ctx.load_constant(F::ZERO); 32];
        let floor = ctx.load_witness(F::from(if matches!(mutation, Mutation::CounterFloor) {
            17
        } else {
            16
        }));
        let counter = ctx.load_witness(F::from(17));
        let message = message
            .into_iter()
            .map(|b| b.assigned().unwrap())
            .collect::<Vec<_>>();
        let mut assertion = vec![0xa2, 0x69];
        assertion.extend(b"signature");
        assertion.extend([0x58, der.len() as u8]);
        assertion.extend(der);
        assertion.push(0x71);
        assertion.extend(b"authenticatorData");
        assertion.extend([0x58, 37]);
        assertion.extend(auth);
        constrain_original_apple_signed_message_stream_v1(
            &mut builder,
            &mut jobs,
            &assertion,
            &message,
            None,
            &auth_cells,
            &rp,
            [0; 32],
            &release,
            floor,
            counter,
            &cells,
        )
        .unwrap();
    } else {
        constrain_original_android_signed_message_stream_v1(
            &mut builder,
            &mut jobs,
            &der,
            &message,
            &cells,
        )
        .unwrap();
    }
    builder.assigned_instances = vec![vec![]];
    finalize_base_params_v1(&mut builder, 9).unwrap();
    jobs.validate_capacity((1usize << 16) - 9).unwrap();
    MessageCircuit { builder, jobs }
}
fn check<F: KagemushaPoseidonFieldV1>() {
    for apple in [false, true] {
        let circuit = build::<F>(apple, Mutation::None);
        MockProver::run(16, &circuit, vec![vec![]])
            .unwrap()
            .assert_satisfied();
        drop(circuit);
        for mutation in [
            Mutation::Message,
            Mutation::OriginalDer,
            Mutation::EnrolledKey,
        ] {
            let circuit = build::<F>(apple, mutation);
            assert!(
                MockProver::run(16, &circuit, vec![vec![]])
                    .unwrap()
                    .verify()
                    .is_err()
            );
        }
        if apple {
            let circuit = build::<F>(true, Mutation::CounterFloor);
            assert!(
                MockProver::run(16, &circuit, vec![vec![]])
                    .unwrap()
                    .verify()
                    .is_err()
            );
        }
    }
}
#[test]
fn distinct_receiver_message_original_platform_equations_and_mutations_both_fields() {
    std::thread::Builder::new()
        .name("ordinary receiver message equations".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            check::<Fp>();
            check::<Fq>();
        })
        .unwrap()
        .join()
        .unwrap();
}
