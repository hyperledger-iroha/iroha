//! One bounded call surface ensuring raw CBOR, canonical DER, SHA and ECDSA share cells.
//!
//! This staged helper remains unused by the monetary fold. It binds both signed
//! secure indices to the passed state cells, but does not by itself authenticate
//! the remaining Core subject, governed RP, credential, or release policy.
// TODO: Wire this exact helper into both recursive parities only after Core/Guard,
// provider-policy and terminal commitment links are fully circuit-derived.

use halo2_base::{AssignedValue, gates::circuit::builder::BaseCircuitBuilder};
use halo2_ecc::{bigint::ProperCrtUint, ecc::EcPoint, fields::fp::FpChip};
use halo2_proofs::halo2curves::secp256r1::Fp as P256Base;
use iroha_data_model::kagemusha::KagemushaHardwareSelectionSigningLayoutV1;

use crate::zk::{
    kagemusha_p256_curve_gadget::{
        P256_LIMB_BITS, P256_NUM_LIMBS, app_attest_der_gadget::constrain_p256_canonical_der_v1,
        assert_apple_assertion_ecdsa,
    },
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::PastaSha256JobsV1,
};

use super::{
    app_attest_assertion_cbor::constrain_original_apple_assertion_37_v1,
    constrain_apple_signed_secure_index_v1,
};

/// Derive DER from `r,s`, prove original CBOR equality, then verify the Apple
/// assertion with those same `r,s` and authenticator/S cells. The signed
/// before/after indices and signed Apple counter are copy-bound to the passed
/// state indices. No host-parsed DER pair or detached counter can be substituted
/// between the relations. Production instantiation must use `N = 256`; smaller
/// windows are only useful for bounded equation tests and reject most signatures.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_original_apple_assertion_ecdsa_37_v1<
    F: KagemushaPoseidonFieldV1,
    const N: usize,
>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    original_raw_assertion: &[u8],
    canonical_s: &[AssignedValue<F>; KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES],
    authenticator_data: &[AssignedValue<F>; 37],
    governed_rp_id_hash: &[AssignedValue<F>; 32],
    previous_secure_index: AssignedValue<F>,
    next_secure_index: AssignedValue<F>,
    signature_public_key: &EcPoint<F, ProperCrtUint<F>>,
    enrolled_public_key: &EcPoint<F, ProperCrtUint<F>>,
    enrolled_public_key_sec1: &[AssignedValue<F>; 65],
    r: &ProperCrtUint<F>,
    s: &ProperCrtUint<F>,
    z: &ProperCrtUint<F>,
    digest_reduction_quotient: AssignedValue<F>,
) -> Result<(), String> {
    constrain_apple_signed_secure_index_v1(
        builder,
        previous_secure_index,
        next_secure_index,
        canonical_s,
        authenticator_data,
    );
    let range = builder.range_chip();
    let chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
    let der = constrain_p256_canonical_der_v1(&chip, builder.main(0), r, s);
    constrain_original_apple_assertion_37_v1(
        builder,
        original_raw_assertion,
        authenticator_data,
        &der,
    )?;
    let expected_flags = builder.main(0).load_constant(F::from(0x40_u64));
    assert_apple_assertion_ecdsa::<
        F,
        { KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES },
        N,
        37,
        false,
    >(
        &chip,
        builder.main(0),
        jobs,
        canonical_s,
        authenticator_data,
        governed_rp_id_hash,
        expected_flags,
        previous_secure_index,
        next_secure_index,
        signature_public_key,
        enrolled_public_key,
        enrolled_public_key_sec1,
        r,
        s,
        z,
        digest_reduction_quotient,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::pasta_sha256::PastaSha256ConfigV1;
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
    use halo2_ecc::fields::FieldChip as _;
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::{
            CurveAffine as _, CurveAffineExt as _,
            ff::{Field as _, PrimeField as _},
            group::{Curve as _, prime::PrimeCurveAffine as _},
            pasta::{Fp, Fq},
            secp256r1::{Fq as P256Scalar, Secp256r1Affine},
        },
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use sha2::{Digest as _, Sha256};

    const K: u32 = 18;
    const UNUSABLE_ROWS: usize = 9;
    const S_LEN: usize = KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES;

    #[derive(Clone, Debug)]
    struct Config<F: KagemushaPoseidonFieldV1> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }

    #[derive(Clone)]
    struct AssertionCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for AssertionCircuit<F> {
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

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows((1_usize << K) - UNUSABLE_ROWS);
            Config {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("Apple original-assertion test uses parameterized Base")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "Apple original assertion Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << K) - UNUSABLE_ROWS,
            )
        }
    }

    fn canonical_positive_integer(value: P256Scalar) -> Vec<u8> {
        let mut bytes = value.to_repr().to_vec();
        bytes.reverse();
        let first_nonzero = bytes.iter().position(|byte| *byte != 0).unwrap_or(31);
        let mut bytes = bytes[first_nonzero..].to_vec();
        if bytes[0] & 0x80 != 0 {
            bytes.insert(0, 0);
        }
        bytes
    }

    fn check<F: KagemushaPoseidonFieldV1>(mutation: u8) -> bool {
        use KagemushaHardwareSelectionSigningLayoutV1 as S;

        let rp: [u8; 32] = Sha256::digest(b"TEAM.bundle").into();
        let mut auth = [0_u8; 37];
        auth[..32].copy_from_slice(&rp);
        auth[32] = 0x40;
        auth[36] = 1;
        let mut signed_s = [0_u8; S_LEN];
        signed_s[S::DOMAIN].copy_from_slice(S::DOMAIN_BYTES);
        signed_s[S::BODY_LENGTH].copy_from_slice(&(S::BODY_BYTES as u64).to_le_bytes());
        signed_s[S::SECURE_INDEX_AFTER].copy_from_slice(&1_u128.to_le_bytes());

        // A real SHA-derived digest with r=s=z gives u1=u2=1. Choose an
        // on-curve digest x and set the enrolled key to R-G, so the complete
        // P-256 equation is satisfied without a host-supplied signature oracle.
        let (z, q) = (0_u8..=u8::MAX)
            .find_map(|tweak| {
                signed_s[S::TRANSITION_STATEMENT_DIGEST.start] = tweak;
                let client_hash: [u8; 32] = Sha256::digest(signed_s).into();
                let mut nonce_preimage = auth.to_vec();
                nonce_preimage.extend_from_slice(&client_hash);
                let nonce: [u8; 32] = Sha256::digest(nonce_preimage).into();
                let mut digest: [u8; 32] = Sha256::digest(nonce).into();
                digest.reverse();
                let z = Option::<P256Scalar>::from(P256Scalar::from_repr(digest))?;
                if z == P256Scalar::ZERO {
                    return None;
                }
                let x = Option::<P256Base>::from(P256Base::from_repr(digest))?;
                let rhs = x * x * x - P256Base::from(3_u64) * x + Secp256r1Affine::b();
                let y = Option::<P256Base>::from(rhs.sqrt())?;
                let point = Option::<Secp256r1Affine>::from(Secp256r1Affine::from_xy(x, y))?;
                let q = (point.to_curve() - Secp256r1Affine::generator().to_curve()).to_affine();
                (!bool::from(q.is_identity())).then_some((z, q))
            })
            .expect("some bounded Apple assertion digest is a P-256 x-coordinate");

        let integer = canonical_positive_integer(z);
        let mut der = vec![
            0x30,
            (4 + 2 * integer.len()) as u8,
            0x02,
            integer.len() as u8,
        ];
        der.extend_from_slice(&integer);
        der.extend([0x02, integer.len() as u8]);
        der.extend_from_slice(&integer);
        let mut raw = Vec::from([0xa2, 0x71]);
        raw.extend_from_slice(b"authenticatorData");
        raw.extend([0x58, 37]);
        raw.extend_from_slice(&auth);
        raw.push(0x69);
        raw.extend_from_slice(b"signature");
        raw.extend([0x58, der.len() as u8]);
        raw.extend_from_slice(&der);

        let (qx, qy) = q.into_coordinates();
        let mut x_be = qx.to_repr();
        let mut y_be = qy.to_repr();
        x_be.reverse();
        y_be.reverse();
        let mut sec1 = [0_u8; 65];
        sec1[0] = 4;
        sec1[1..33].copy_from_slice(&x_be);
        sec1[33..].copy_from_slice(&y_be);
        if mutation == 3 {
            *raw.last_mut().expect("DER byte") ^= 1;
        }

        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits((K - 1) as usize);
        let range = builder.range_chip();
        let base_chip = FpChip::<F, P256Base>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
        let scalar_chip = FpChip::<F, P256Scalar>::new(&range, P256_LIMB_BITS, P256_NUM_LIMBS);
        let ctx = builder.main(0);
        let signed_s =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(signed_s[index]))));
        let auth = std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(auth[index]))));
        let governed_rp =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(rp[index]))));
        let sec1 = std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(sec1[index]))));
        let key = EcPoint::new(
            base_chip.load_private(ctx, qx),
            base_chip.load_private(ctx, qy),
        );
        let r = scalar_chip.load_private(ctx, z);
        let s = scalar_chip.load_private(ctx, z);
        let digest = scalar_chip.load_private(ctx, z);
        let before = ctx.load_witness(F::from(u64::from(mutation == 1)));
        let after = ctx.load_witness(F::from(if mutation == 2 { 2 } else { 1 }));
        let quotient = ctx.load_witness(F::ZERO);
        let mut jobs = PastaSha256JobsV1::default();
        constrain_original_apple_assertion_ecdsa_37_v1::<F, 2>(
            &mut builder,
            &mut jobs,
            &raw,
            &signed_s,
            &auth,
            &governed_rp,
            before,
            after,
            &key,
            &key,
            &sec1,
            &r,
            &s,
            &digest,
            quotient,
        )
        .expect("fixed original Apple assertion is structurally accepted");
        builder.calculate_params(Some(UNUSABLE_ROWS));
        MockProver::run(K, &AssertionCircuit { builder, jobs }, vec![])
            .expect("complete original Apple assertion circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn original_signed_assertion_binds_both_state_indices_in_both_pasta_fields() {
        // Full positive proofs in each parity; distribute the three independent
        // negative relations to keep this complete SHA/ECDSA fixture bounded.
        assert!(check::<Fp>(0));
        assert!(check::<Fq>(0));
        assert!(!check::<Fp>(1));
        assert!(!check::<Fq>(2));
        assert!(!check::<Fp>(3));
    }
}
