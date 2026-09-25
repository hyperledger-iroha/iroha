//! Staged circuit binding of Android one-use key heads to an exact signed index.
//!
//! This is a necessary part of the KeyMint monetary relation, not a monetary
//! verifier. The live recursive circuit still fixes one-use heads to zero.
//! TODO: Bind this relation to the full signed Core subject, KeyMint signature,
//! attestation certificate/app identity/one-use properties, and both live folds.

use halo2_base::{
    AssignedValue,
    QuantumCell::Constant,
    gates::{GateInstructions as _, RangeInstructions as _, circuit::builder::BaseCircuitBuilder},
    utils::power_of_two,
};
use iroha_data_model::kagemusha::KagemushaHardwareSelectionSigningLayoutV1;

use crate::zk::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

use super::super::guard_bundle::{constant_bytes, digest_limbs_assigned, hash};

const DEVICE_KEY_REFERENCE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:device-key-reference";

/// Prove that adjacent State heads commit to the consumed and prepared SEC1
/// keys and that the signed Core frame advances the exact authenticated index.
///
/// The keys and signed frame are witness bytes here. A future caller must also
/// prove their KeyMint provenance, the signature over this frame, and all other
/// independently authenticated frame fields before enabling ordinary apps.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "KeyMint monetary fold remains closed")
)]
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_keymint_one_use_head_stage_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    signed_selection: &[AssignedValue<F>; KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES],
    consumed_sec1: &[AssignedValue<F>; 65],
    prepared_sec1: &[AssignedValue<F>; 65],
    predecessor_head: [AssignedValue<F>; 2],
    successor_head: [AssignedValue<F>; 2],
    predecessor_index: AssignedValue<F>,
    successor_index: AssignedValue<F>,
) -> Result<(), String> {
    use KagemushaHardwareSelectionSigningLayoutV1 as S;

    let range = builder.range_chip();
    let gate = range.gate();
    let ctx = builder.main(0);
    for signed in signed_selection {
        range.range_check(ctx, *signed, 8);
    }
    for (actual, expected) in signed_selection[S::DOMAIN].iter().zip(S::DOMAIN_BYTES) {
        gate.assert_is_const(ctx, actual, &F::from(u64::from(*expected)));
    }
    for (actual, expected) in signed_selection[S::BODY_LENGTH]
        .iter()
        .zip((S::BODY_BYTES as u64).to_le_bytes())
    {
        gate.assert_is_const(ctx, actual, &F::from(u64::from(expected)));
    }
    for (actual, expected) in signed_selection[S::VERSION]
        .iter()
        .zip(iroha_data_model::kagemusha::KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes())
    {
        gate.assert_is_const(ctx, actual, &F::from(u64::from(expected)));
    }
    range.range_check(ctx, predecessor_index, 128);
    range.range_check(ctx, successor_index, 128);
    let compose_index = |ctx: &mut halo2_base::Context<F>, bytes: &[AssignedValue<F>]| {
        gate.inner_product(
            ctx,
            bytes.iter().copied(),
            (0..16).map(|index| Constant(power_of_two::<F>(8 * index))),
        )
    };
    let signed_before = compose_index(ctx, &signed_selection[S::SECURE_INDEX_BEFORE]);
    let signed_after = compose_index(ctx, &signed_selection[S::SECURE_INDEX_AFTER]);
    ctx.constrain_equal(&signed_before, &predecessor_index);
    ctx.constrain_equal(&signed_after, &successor_index);
    let exact_next = gate.inc(ctx, predecessor_index);
    ctx.constrain_equal(&exact_next, &successor_index);

    for (sec1, head) in [
        (consumed_sec1, predecessor_head),
        (prepared_sec1, successor_head),
    ] {
        gate.assert_is_const(ctx, &sec1[0], &F::from(4_u64));
        let key = sec1
            .iter()
            .copied()
            .map(|byte| PastaSha256ByteV1::range_checked(ctx, &range, byte))
            .collect::<Vec<_>>();
        let reference = hash(
            ctx,
            jobs,
            constant_bytes(DEVICE_KEY_REFERENCE_DOMAIN)
                .into_iter()
                .chain([PastaSha256ByteV1::constant(0)])
                .chain(key)
                .collect(),
        )?;
        for (derived, authenticated) in digest_limbs_assigned(ctx, &reference).into_iter().zip(head)
        {
            ctx.constrain_equal(&derived, &authenticated);
        }
    }
    let same_low = gate.is_equal(ctx, predecessor_head[0], successor_head[0]);
    let same_high = gate.is_equal(ctx, predecessor_head[1], successor_head[1]);
    let same = gate.and(ctx, same_low, same_high);
    gate.assert_is_const(ctx, &same, &F::ZERO);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::{kagemusha_v1_poseidon::digest_limbs, pasta_sha256::PastaSha256ConfigV1};
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaDevicePublicKeyV1, kagemusha_device_key_reference_v1,
    };
    use p256::ecdsa::SigningKey;

    const K: u32 = 17;
    const UNUSABLE_ROWS: usize = 9;

    #[derive(Clone, Debug)]
    struct TestConfig<F: KagemushaPoseidonFieldV1> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }

    #[derive(Clone)]
    struct TestCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for TestCircuit<F> {
        type Config = TestConfig<F>;
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
            TestConfig {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("KeyMint one-use head test uses parameterized Base")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "KeyMint one-use head Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << K) - UNUSABLE_ROWS,
            )
        }
    }

    fn key(seed: u8) -> (KagemushaDevicePublicKeyV1, [u8; 65]) {
        let signer = SigningKey::from_bytes((&[seed; 32]).into()).expect("fixture signer");
        let sec1: [u8; 65] = signer
            .verifying_key()
            .to_encoded_point(false)
            .as_bytes()
            .try_into()
            .expect("uncompressed SEC1");
        let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(&sec1).expect("fixture key");
        (key, sec1)
    }

    fn check<F: KagemushaPoseidonFieldV1>(mutation: u8) -> bool {
        use KagemushaHardwareSelectionSigningLayoutV1 as S;

        let (consumed, mut consumed_sec1) = key(3);
        let (prepared, mut prepared_sec1) = key(4);
        let mut predecessor_head = kagemusha_device_key_reference_v1(&consumed);
        let mut successor_head = kagemusha_device_key_reference_v1(&prepared);
        let mut signed = [0_u8; S::TOTAL_BYTES];
        signed[S::DOMAIN].copy_from_slice(S::DOMAIN_BYTES);
        signed[S::BODY_LENGTH].copy_from_slice(&(S::BODY_BYTES as u64).to_le_bytes());
        signed[S::VERSION].copy_from_slice(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes());
        signed[S::SECURE_INDEX_BEFORE].copy_from_slice(&41_u128.to_le_bytes());
        signed[S::SECURE_INDEX_AFTER].copy_from_slice(&42_u128.to_le_bytes());
        match mutation {
            1 => consumed_sec1[7] ^= 1,
            2 => predecessor_head[0] ^= 1,
            3 => successor_head[31] ^= 1,
            4 => signed[S::SECURE_INDEX_AFTER.start] ^= 1,
            5 => signed[S::DOMAIN.start] ^= 1,
            6 => signed[S::VERSION.start] ^= 1,
            7 => signed[S::SECURE_INDEX_BEFORE.start] ^= 1,
            8 => {
                prepared_sec1 = consumed_sec1;
                successor_head = predecessor_head;
            }
            9 => successor_head = predecessor_head,
            _ => {}
        }

        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits((K - 1) as usize)
            .use_instance_columns(1);
        let ctx = builder.main(0);
        let signed =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(signed[index]))));
        let consumed_sec1 =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(consumed_sec1[index]))));
        let prepared_sec1 =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(prepared_sec1[index]))));
        let predecessor_head =
            digest_limbs::<F>(predecessor_head).map(|limb| ctx.load_witness(limb));
        let successor_head = digest_limbs::<F>(successor_head).map(|limb| ctx.load_witness(limb));
        let predecessor_index = ctx.load_witness(F::from(41_u64));
        let successor_index = ctx.load_witness(F::from(42_u64));
        let mut jobs = PastaSha256JobsV1::default();
        constrain_keymint_one_use_head_stage_v1(
            &mut builder,
            &mut jobs,
            &signed,
            &consumed_sec1,
            &prepared_sec1,
            predecessor_head,
            successor_head,
            predecessor_index,
            successor_index,
        )
        .expect("fixed KeyMint stage structurally accepted");
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(UNUSABLE_ROWS));
        MockProver::run(K, &TestCircuit { builder, jobs }, vec![Vec::new()])
            .expect("KeyMint one-use head circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn keymint_heads_and_signed_indices_are_bound_in_both_pasta_fields() {
        assert!(check::<Fp>(0));
        assert!(check::<Fq>(0));
        for mutation in 1..=9 {
            assert!(!check::<Fp>(mutation), "Fp mutation {mutation}");
            assert!(!check::<Fq>(mutation), "Fq mutation {mutation}");
        }
    }
}
