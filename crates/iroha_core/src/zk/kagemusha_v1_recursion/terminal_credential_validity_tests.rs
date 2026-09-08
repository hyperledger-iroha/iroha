//! Exact sender credential openings and historical commit-window constraint regressions.

use halo2_proofs::dev::MockProver;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    kagemusha::{
        KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KagemushaDevicePublicKeyV1,
        KagemushaDeviceSignatureV1, KagemushaHardwarePlatformClassV1,
        kagemusha_device_key_reference_v1, kagemusha_suite_commitment_v1,
    },
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

use super::*;

#[derive(Clone, Debug)]
struct CredentialHashConfig<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
}

#[derive(Clone)]
struct CredentialHashCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}

impl<F: KagemushaPoseidonFieldV1> Circuit<F> for CredentialHashCircuit<F> {
    type Config = CredentialHashConfig<F>;
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
        let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows(usable_rows);
        CredentialHashConfig {
            base,
            sha: PastaSha256ConfigV1::configure(meta),
        }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("credential opening tests use explicit Base parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), PlonkError> {
        let usable_rows = (1_usize << self.builder.config_params.k) - MINIMUM_UNUSABLE_ROWS;
        <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "terminal credential Base"),
        )?;
        self.jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            usable_rows,
        )
    }
}

fn credential_fixture() -> (KagemushaHardwareProfileV1, KagemushaHardwareCredentialV1) {
    let issuer = SigningKey::from_bytes((&[0x31; 32]).into()).expect("diagnostic issuer");
    let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        issuer.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .expect("canonical diagnostic key");
    let profile = KagemushaHardwareProfileV1 {
        version: 1,
        protocol_version: 1,
        hardware_profile_id: [0; 32],
        provider_id: [1; 32],
        platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
        product_class_digest: [2; 32],
        firmware_policy_digest: [3; 32],
        enrollment_attestation_verifier_digest: [4; 32],
        attestation_trust_roots_digest: [5; 32],
        allowed_suite_commitment: kagemusha_suite_commitment_v1([6; 32]),
        policy_epoch: 7,
        governance_credential_public_key: key,
        capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
        qualification_report_digest: [8; 32],
        valid_from_ms: 10,
        expires_at_ms: 1000,
    }
    .seal_hardware_profile_id()
    .expect("canonical diagnostic profile");
    let sign = |message: &[u8]| {
        let signature: Signature = issuer.sign(message);
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("low-S signature")
    };
    let mut credential = KagemushaHardwareCredentialV1 {
        version: 1,
        credential_id: [0; 32],
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"terminal-credential-network"),
        )),
        hardware_profile_id: profile.hardware_profile_id,
        suite_id: [6; 32],
        firmware_policy_digest: profile.firmware_policy_digest,
        policy_epoch: profile.policy_epoch,
        lane_commitment: [9; 32],
        hardware_epoch_id: [10; 32],
        hardware_epoch_generation: 2,
        device_public_key: key,
        device_key_reference: kagemusha_device_key_reference_v1(&key),
        issued_at_ms: 100,
        expires_at_ms: 900,
        governance_signature: sign(b"unsealed diagnostic credential"),
    }
    .seal_credential_id()
    .expect("canonical diagnostic credential");
    credential.governance_signature = sign(
        &credential
            .canonical_signing_bytes()
            .expect("signing preimage"),
    );
    credential
        .validate_against_profile(&profile)
        .expect("valid signed diagnostic fixture");
    (profile, credential)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Substitution {
    None,
    CredentialExpiry,
    CredentialIssued,
    ProfileExpiry,
    ProfileActivation,
    Issuance,
}

fn sender_opening_case<F: KagemushaPoseidonFieldV1>(
    mutation: Substitution,
) -> CredentialHashCircuit<F> {
    let (mut profile, mut credential) = credential_fixture();
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(17)
        .use_lookup_bits(15)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let mut jobs = PastaSha256JobsV1::default();
    // Capture authenticated IDs/context before mutating any canonical preimage fields.
    let sender = TerminalSenderCredentialContextV1 {
        profile: assign_bytes(ctx, &range, &profile.hardware_profile_id),
        network: assign_bytes(ctx, &range, credential.network_id.as_bytes()),
        suite: assign_bytes(ctx, &range, &credential.suite_id),
        lane: assign_bytes(ctx, &range, &credential.lane_commitment),
        epoch: assign_bytes(ctx, &range, &credential.hardware_epoch_id),
        key_reference: assign_bytes(ctx, &range, &credential.device_key_reference),
        protocol_version: ctx.load_witness(F::from(1)),
        policy_epoch: ctx.load_witness(F::from(credential.policy_epoch)),
        generation: ctx.load_witness(F::from(credential.hardware_epoch_generation)),
        issuance: [0, 1].map(|slot| {
            assign_fixed_digest_v1(
                ctx,
                &range,
                if mutation == Substitution::Issuance && slot == 1 {
                    [0x42; 32]
                } else {
                    credential.credential_id
                },
            )
        }),
        device_keys: [0, 1]
            .map(|_| assign_bytes(ctx, &range, credential.device_public_key.as_sec1_bytes())),
    };
    match mutation {
        Substitution::CredentialExpiry => credential.expires_at_ms += 1,
        Substitution::CredentialIssued => credential.issued_at_ms -= 1,
        Substitution::ProfileExpiry => profile.expires_at_ms += 1,
        Substitution::ProfileActivation => profile.valid_from_ms -= 1,
        Substitution::None | Substitution::Issuance => {}
    }
    let enabled = ctx.load_constant(F::ONE);
    let windows = constrain_terminal_sender_credential_v1(
        ctx,
        &range,
        &mut jobs,
        &profile
            .canonical_id_preimage_bytes()
            .expect("canonical profile bytes"),
        &credential
            .canonical_id_preimage_bytes()
            .expect("canonical credential bytes"),
        &sender,
        enabled,
    )
    .expect("actual sender canonical opening constraints");
    // The original time remains valid even if delivery happens later; no receiver wall clock enters.
    let committed = ctx.load_witness(F::from(100));
    let inactive = ctx.load_constant(F::ZERO);
    for window in windows {
        constrain_terminal_commit_window_v1(
            ctx,
            &range,
            enabled,
            inactive,
            committed,
            [inactive; 2],
            window,
        );
    }
    builder.assigned_instances = vec![vec![]];
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    assert_eq!(jobs.capacity_profile().expect("actual SHA jobs").0, 3);
    CredentialHashCircuit { builder, jobs }
}

fn assert_sender_openings<F: KagemushaPoseidonFieldV1>() {
    for mutation in [
        Substitution::None,
        Substitution::CredentialExpiry,
        Substitution::CredentialIssued,
        Substitution::ProfileExpiry,
        Substitution::ProfileActivation,
        Substitution::Issuance,
    ] {
        let circuit = sender_opening_case::<F>(mutation);
        let result = MockProver::run(17, &circuit, vec![vec![]])
            .expect("real Base and SHA synthesis")
            .verify();
        assert_eq!(
            result.is_ok(),
            mutation == Substitution::None,
            "{mutation:?}: {result:?}"
        );
    }
}

#[test]
fn terminal_sender_canonical_openings_reject_detached_lifetimes_eq() {
    assert_sender_openings::<Fp>();
}

#[test]
fn terminal_sender_canonical_openings_reject_detached_lifetimes_ep() {
    assert_sender_openings::<Fq>();
}

fn assert_commit_windows<F: KagemushaPoseidonFieldV1>() {
    // The supplied time/lease is the original authenticated commit evidence. Delivery has no time input.
    for (trusted, time, lease_start, lease_end, expected) in [
        (true, 0, 0, 0, false),
        (true, 99, 0, 0, false),
        (true, 100, 0, 0, true),
        (true, 899, 0, 0, true),
        (true, 900, 0, 0, false),
        (false, 0, 0, 900, false),
        (false, 0, 99, 899, false),
        (false, 0, 100, 900, true),
        (false, 0, 100, 901, false),
        (false, 0, 100, 100, false),
        (false, 0, 899, 900, true),
        (false, 0, 900, 901, false),
    ] {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(12)
            .use_lookup_bits(11)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let trusted_selector = ctx.load_constant(F::from(u64::from(trusted)));
        let lease_selector = ctx.load_constant(F::from(u64::from(!trusted)));
        let committed = assign_fixed_uint_v1(ctx, &range, time, 64).value;
        let lease =
            [lease_start, lease_end].map(|v| assign_fixed_uint_v1(ctx, &range, v, 64).value);
        // Check both authentic windows, with each independently being the tighter interval.
        for window in [[10, 1000], [100, 900]] {
            let validity = window.map(|v| assign_fixed_uint_v1(ctx, &range, v, 64).value);
            constrain_terminal_commit_window_v1(
                ctx,
                &range,
                trusted_selector,
                lease_selector,
                committed,
                lease,
                validity,
            );
        }
        builder.assigned_instances = vec![vec![]];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        let result = MockProver::run(12, &builder, vec![vec![]])
            .expect("window circuit")
            .verify();
        assert_eq!(
            result.is_ok(),
            expected,
            "trusted={trusted} time={time} lease={lease_start}..{lease_end}: {result:?}"
        );
    }
}

#[test]
fn terminal_commit_window_endpoints_and_entire_lease_eq() {
    assert_commit_windows::<Fp>();
}

#[test]
fn terminal_commit_window_endpoints_and_entire_lease_ep() {
    assert_commit_windows::<Fq>();
}

fn assert_guard_credential_binding<F: KagemushaPoseidonFieldV1>() {
    for mutation in [None, Some(6), Some(7), Some(8), Some(9), Some(44)] {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(12)
            .use_lookup_bits(11)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let digests = [[0x21; 32], [0x32; 32]];
        let assigned = digests.map(|digest| assign_fixed_digest_v1(ctx, &range, digest));
        let mut values = vec![F::ZERO; GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1];
        for (offset, digest) in [(6, digests[0]), (8, digests[1])] {
            values[offset..offset + 2]
                .copy_from_slice(&crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(digest));
        }
        if let Some(index) = mutation {
            if index == 44 {
                values.swap(6, 8);
                values.swap(7, 9);
            } else {
                values[index] += F::ONE;
            }
        }
        let column = values
            .iter()
            .map(|value| ctx.load_witness(*value))
            .collect::<Vec<_>>();
        constrain_terminal_guard_credential_digests_v1(ctx, &column, &assigned)
            .expect("Guard44 shape");
        assert!(
            constrain_terminal_guard_credential_digests_v1(ctx, &column[..40], &assigned).is_err()
        );
        builder.assigned_instances = vec![column];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        let result = MockProver::run(12, &builder, vec![values])
            .expect("Guard credential opening circuit")
            .verify();
        assert_eq!(
            result.is_ok(),
            mutation.is_none(),
            "detached Guard digest {mutation:?}: {result:?}"
        );
    }
}

#[test]
fn terminal_guard_credential_digest_substitution_eq() {
    assert_guard_credential_binding::<Fp>();
}

#[test]
fn terminal_guard_credential_digest_substitution_ep() {
    assert_guard_credential_binding::<Fq>();
}
