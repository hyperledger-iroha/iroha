//! Canonical compact credential-ID opening for the staged Apple monetary fold.
//!
//! The 409-byte Norito frame is assembled from assigned Guard/profile cells, including
//! its constrained CRC64-XZ, then SHA-bound to the provider-authenticated Guard ID.
//! This does not verify issuer signature, app enrollment, profile lifetime or assertion.
// TODO: Wire the authenticated firmware/profile lifetime and exact issuer credential
// signature into both recursive parities before monetary admission.

use halo2_base::{
    AssignedValue, Context,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1,
    KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_FIELD_RANGES_V1, KAGEMUSHA_WIRE_VERSION_V1,
    kagemusha_hardware_credential_id_preimage_layout_v1,
};

use crate::zk::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256ByteV1, PastaSha256JobsV1},
};

use super::{
    super::{
        canonical_preimage::assemble_canonical_preimage_v1,
        guard_bundle::{constant_bytes, hash},
    },
    assigned_digest_bytes_v1, assigned_uint_bytes_v1,
};

const CREDENTIAL_ID_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-credential-id";

/// Assigned semantic fields of the provider-authenticated predecessor credential.
///
/// `firmware_policy_digest` must come from the governed profile opening. The two
/// lifetime cells must be range/validity checked against that same profile before
/// this ID opening can authorize a monetary transition.
pub(super) struct AppleCompactCredentialIdCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) network_id: [AssignedValue<F>; 2],
    pub(super) hardware_profile_id: [AssignedValue<F>; 2],
    pub(super) suite_id: [AssignedValue<F>; 2],
    pub(super) firmware_policy_digest: [PastaSha256ByteV1<F>; 32],
    pub(super) policy_epoch: AssignedValue<F>,
    pub(super) lane_id: [AssignedValue<F>; 2],
    pub(super) epoch_id: [AssignedValue<F>; 2],
    pub(super) epoch_generation: AssignedValue<F>,
    pub(super) device_public_key: Vec<PastaSha256ByteV1<F>>,
    pub(super) key_reference: [AssignedValue<F>; 2],
    pub(super) issued_at_ms: AssignedValue<F>,
    pub(super) expires_at_ms: AssignedValue<F>,
    pub(super) app_policy_binding_digest: [PastaSha256ByteV1<F>; 32],
    pub(super) guard_issuance_digest: [PastaSha256ByteV1<F>; 32],
}

/// Prove the canonical compact credential frame hashes to the exact Guard ID.
///
/// All fourteen semantic fields use already assigned cells; only fixed codec
/// framing comes from the model layout. The resulting digest is copy-equal to
/// the provider-authenticated predecessor issuance field in both Pasta parities.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "staged Apple assertion monetary fold remains closed"
    )
)]
pub(super) fn constrain_apple_compact_credential_id_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    fields: &AppleCompactCredentialIdCellsV1<F>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if fields.device_public_key.len() != 65 {
        return Err("Apple compact credential requires uncompressed SEC1".to_owned());
    }
    let gate = range.gate();
    gate.assert_is_const(
        ctx,
        &fields.device_public_key[0]
            .assigned()
            .expect("Guard SEC1 prefix assigned"),
        &F::from(4_u64),
    );
    range.range_check(ctx, fields.issued_at_ms, 64);
    range.range_check(ctx, fields.expires_at_ms, 64);
    let valid_window = range.is_less_than(ctx, fields.issued_at_ms, fields.expires_at_ms, 64);
    gate.assert_is_const(ctx, &valid_window, &F::ONE);
    let values: [Vec<PastaSha256ByteV1<F>>; 14] = [
        constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()),
        assigned_digest_bytes_v1(ctx, gate, fields.network_id),
        assigned_digest_bytes_v1(ctx, gate, fields.hardware_profile_id),
        assigned_digest_bytes_v1(ctx, gate, fields.suite_id),
        fields.firmware_policy_digest.to_vec(),
        assigned_uint_bytes_v1(ctx, gate, fields.policy_epoch, 64),
        assigned_digest_bytes_v1(ctx, gate, fields.lane_id),
        assigned_digest_bytes_v1(ctx, gate, fields.epoch_id),
        assigned_uint_bytes_v1(ctx, gate, fields.epoch_generation, 64),
        fields.device_public_key.clone(),
        assigned_digest_bytes_v1(ctx, gate, fields.key_reference),
        assigned_uint_bytes_v1(ctx, gate, fields.issued_at_ms, 64),
        assigned_uint_bytes_v1(ctx, gate, fields.expires_at_ms, 64),
        fields.app_policy_binding_digest.to_vec(),
    ];
    let layout =
        kagemusha_hardware_credential_id_preimage_layout_v1().map_err(|error| error.to_string())?;
    let sources = values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let frame = assemble_canonical_preimage_v1(
        ctx,
        range,
        &layout,
        &KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_FIELD_RANGES_V1,
        &sources,
    )?;
    if frame.len() != KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1 {
        return Err("Apple compact credential frame width changed".to_owned());
    }
    let digest = hash(
        ctx,
        jobs,
        [
            constant_bytes(CREDENTIAL_ID_DOMAIN),
            constant_bytes(&[0]),
            constant_bytes(&(frame.len() as u64).to_le_bytes()),
            frame,
        ]
        .concat(),
    )?;
    for (actual, expected) in digest.iter().zip(fields.guard_issuance_digest) {
        ctx.constrain_equal(
            &actual
                .assigned()
                .expect("derived compact credential ID byte"),
            &expected
                .assigned()
                .expect("Guard compact credential ID byte"),
        );
    }
    Ok(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::{
        kagemusha_v1_poseidon::digest_limbs, kagemusha_v1_recursion::guard_bundle::assign_bytes,
        pasta_sha256::PastaSha256ConfigV1,
    };
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        kagemusha::{
            KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1, KagemushaHardwareCredentialV1,
            kagemusha_device_key_reference_v1,
        },
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

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
            let usable = (1_usize << params.k) - UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable);
            TestConfig {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }
        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("compact Apple credential test uses parameterized Base")
        }
        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "Apple compact credential Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << self.builder.config_params.k) - UNUSABLE_ROWS,
            )
        }
    }

    fn credential() -> KagemushaHardwareCredentialV1 {
        let signing = SigningKey::from_bytes((&[0x31; 32]).into()).expect("fixture signer");
        let device_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("fixture device key");
        let signature: Signature = signing.sign(b"fixture credential");
        let signature = signature.normalize_s().unwrap_or(signature);
        let signature = KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("fixture signature");
        KagemushaHardwareCredentialV1 {
            version: 1,
            credential_id: [0; 32],
            network_id: NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"apple-compact-credential-test",
                )),
            ),
            hardware_profile_id: [0x11; 32],
            suite_id: [0x12; 32],
            firmware_policy_digest: [0x13; 32],
            policy_epoch: 7,
            lane_commitment: [0x14; 32],
            hardware_epoch_id: [0x15; 32],
            hardware_epoch_generation: 2,
            device_public_key: device_key,
            device_key_reference: kagemusha_device_key_reference_v1(&device_key),
            issued_at_ms: 100,
            expires_at_ms: 900,
            app_policy_binding_digest: [0x16; 32],
            governance_signature: signature,
        }
        .seal_credential_id()
        .expect("canonical compact credential")
    }

    fn check<F: KagemushaPoseidonFieldV1>(mutation: u8) -> bool {
        let credential = credential();
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(K as usize)
            .use_lookup_bits((K - 1) as usize);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let assign_digest = |ctx: &mut Context<F>, digest: [u8; 32]| {
            digest_limbs::<F>(digest).map(|value| ctx.load_witness(value))
        };
        let mut network = *credential.network_id.as_bytes();
        let mut expected_id = credential.credential_id;
        let mut key = credential.device_public_key.as_sec1_bytes().to_vec();
        if mutation == 1 {
            expected_id[0] ^= 1;
        } else if mutation == 2 {
            network[0] ^= 1;
        } else if mutation == 3 {
            key[64] ^= 1;
        }
        let fields = AppleCompactCredentialIdCellsV1 {
            network_id: assign_digest(ctx, network),
            hardware_profile_id: assign_digest(ctx, credential.hardware_profile_id),
            suite_id: assign_digest(ctx, credential.suite_id),
            firmware_policy_digest: assign_bytes(ctx, &range, &credential.firmware_policy_digest)
                .try_into()
                .expect("firmware width"),
            policy_epoch: ctx.load_witness(F::from(credential.policy_epoch)),
            lane_id: assign_digest(ctx, credential.lane_commitment),
            epoch_id: assign_digest(ctx, credential.hardware_epoch_id),
            epoch_generation: ctx.load_witness(F::from(credential.hardware_epoch_generation)),
            device_public_key: assign_bytes(ctx, &range, &key),
            key_reference: assign_digest(ctx, credential.device_key_reference),
            issued_at_ms: ctx.load_witness(F::from(credential.issued_at_ms)),
            expires_at_ms: ctx.load_witness(F::from(credential.expires_at_ms)),
            app_policy_binding_digest: assign_bytes(
                ctx,
                &range,
                &credential.app_policy_binding_digest,
            )
            .try_into()
            .expect("app binding width"),
            guard_issuance_digest: assign_bytes(ctx, &range, &expected_id)
                .try_into()
                .expect("Guard ID width"),
        };
        let mut jobs = PastaSha256JobsV1::default();
        constrain_apple_compact_credential_id_v1(ctx, &range, &mut jobs, &fields)
            .expect("fixed credential frame");
        assert_eq!(jobs.compression_blocks().expect("fixed credential SHA"), 8);
        builder.calculate_params(Some(UNUSABLE_ROWS));
        MockProver::run(K, &TestCircuit { builder, jobs }, vec![])
            .expect("canonical Apple credential circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn canonical_credential_id_opening_rejects_mutated_guard_id_network_and_key_in_both_fields() {
        for mutation in 0..=3 {
            assert_eq!(check::<Fp>(mutation), mutation == 0);
            assert_eq!(check::<Fq>(mutation), mutation == 0);
        }
    }
}
