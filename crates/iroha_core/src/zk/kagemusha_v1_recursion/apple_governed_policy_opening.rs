//! Circuit opening of the governed Apple profile and compact credential's app binding.
//!
//! These SHA relations transfer authenticated profile/Guard IDs to the exact RP,
//! app release and app-attestation authority policy. They do not verify the
//! authority's Ed25519 enrollment assertion or admit a monetary transition.
// TODO: Invoke this in both recursive parities with their assigned state/Guard
// cells after the complete signed Core subject and terminal folds are proven.

use halo2_base::{
    AssignedValue, Context,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::builder::BaseCircuitBuilder,
    },
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APPLE_APP_ATTEST_GUARANTEES_V1, KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1,
    KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_FIELD_RANGES_V1, KAGEMUSHA_WIRE_VERSION_V1,
    KagemushaHardwareProfileV1, KagemushaHardwareSelectionSigningLayoutV1,
    kagemusha_app_enrollment_v1::{
        KAGEMUSHA_APP_AUTHORITY_ED25519_KEY_FRAME_BYTES_V1,
        KAGEMUSHA_APP_AUTHORITY_POLICY_DIGEST_PREIMAGE_BYTES_V1,
        KagemushaAppAttestationAuthorityPolicyV1,
    },
    kagemusha_hardware_profile_id_preimage_layout_v1,
};

use crate::zk::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};

use super::super::{guard_bundle, state_relation};

const PROFILE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-profile";
const AUTHORITY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-attestation-authority-policy\0";
const STATIC_BINDING_DOMAIN: &[u8] = b"iroha:kagemusha:v1:app-device-static-binding\0";

/// Exactly the state/Guard cells authenticated by both recursive parities.
#[derive(Clone, Copy)]
pub(super) struct AppleGovernedPolicyCellsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) protocol_version: AssignedValue<F>,
    pub(super) profile_id: [AssignedValue<F>; 2],
    pub(super) guard_profile_id: [AssignedValue<F>; 2],
    pub(super) policy_epoch: AssignedValue<F>,
    pub(super) release_id: [AssignedValue<F>; 2],
    pub(super) key_reference: [AssignedValue<F>; 2],
    pub(super) lane_id: [AssignedValue<F>; 2],
    pub(super) guard_app_binding: [PastaSha256ByteV1<F>; 32],
    pub(super) guard_credential_id: [PastaSha256ByteV1<F>; 32],
}

/// Select the predecessor cells already assigned by Eq/Ep state and Guard relations.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "staged monetary assertion fold remains closed")
)]
pub(super) fn apple_policy_cells_from_state_guard_v1<F: KagemushaPoseidonFieldV1>(
    state: &state_relation::KagemushaAssignedStateRelationV1<F>,
    guard: &guard_bundle::KagemushaAssignedGuardBundleV1<F>,
) -> AppleGovernedPolicyCellsV1<F> {
    AppleGovernedPolicyCellsV1 {
        protocol_version: state.predecessor.protocol_version,
        profile_id: state.predecessor.hardware_profile_id,
        guard_profile_id: guard.hardware_profile_id,
        policy_epoch: state.predecessor.policy_epoch,
        release_id: state.predecessor.release_id,
        key_reference: state.predecessor.key_reference,
        lane_id: state.predecessor.lane_id,
        guard_app_binding: guard.credential_app_policy_binding_digests[0],
        guard_credential_id: guard.credential_issuance_digests[0],
    }
}

/// RP/App ID and app release bytes that must be passed unchanged to assertion SHA.
pub(super) struct AppleGovernedAppBytesV1<F: KagemushaPoseidonFieldV1> {
    pub(super) rp_id_hash: [AssignedValue<F>; 32],
    pub(super) app_release_digest: [AssignedValue<F>; 32],
}

fn uint_le<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
    bits: usize,
) -> Vec<PastaSha256ByteV1<F>> {
    range.range_check(ctx, value, bits);
    PastaSha256BitV1::decompose(ctx, range.gate(), value, bits)
        .chunks_exact(8)
        .map(|part| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), part))
        .collect()
}

fn digest_le<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: [AssignedValue<F>; 2],
) -> Vec<PastaSha256ByteV1<F>> {
    digest
        .into_iter()
        .flat_map(|limb| uint_le(ctx, range, limb, 128))
        .collect()
}

fn bind_digest_to_limbs<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    digest: &[PastaSha256ByteV1<F>; 32],
    limbs: [AssignedValue<F>; 2],
) {
    for (actual, expected) in guard_bundle::digest_limbs_assigned(ctx, digest)
        .into_iter()
        .zip(limbs)
    {
        ctx.constrain_equal(&actual, &expected);
    }
}

fn nonzero_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
) {
    let sum = range.gate().sum(
        ctx,
        bytes.iter().copied().map(PastaSha256ByteV1::quantum_cell),
    );
    let zero = range.gate().is_zero(ctx, sum);
    range.gate().assert_is_const(ctx, &zero, &F::ZERO);
}

/// Prove an Apple profile ID, its exact class/mask/epoch/authority-policy field,
/// and the compact credential's exact static app-device policy binding.
///
/// The profile-ID hash is copy-bound to both predecessor state and Guard. The
/// policy SHA is copy-bound to field 15 of that authenticated profile. The
/// policy RP and app release bytes feed the credential static-binding SHA;
/// its result equals the authenticated Guard credential field. The same RP
/// cells equal the signed 37-byte authenticator header and are returned for
/// `queue_apple_assertion_digest`.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "staged monetary assertion fold remains closed")
)]
#[allow(clippy::too_many_lines)]
pub(super) fn constrain_apple_governed_policy_opening_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    profile: &KagemushaHardwareProfileV1,
    policy: &KagemushaAppAttestationAuthorityPolicyV1,
    authenticated: AppleGovernedPolicyCellsV1<F>,
    authenticator_data: &[AssignedValue<F>; 37],
) -> Result<AppleGovernedAppBytesV1<F>, String> {
    let profile_frame = profile
        .canonical_id_preimage_bytes()
        .map_err(|error| error.to_string())?;
    let policy_opening = policy.canonical_digest_preimage_v1()?;
    if profile_frame.len() != KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1
        || policy_opening.bytes.len() != KAGEMUSHA_APP_AUTHORITY_POLICY_DIGEST_PREIMAGE_BYTES_V1
        || policy_opening.authority_key_frame.len()
            != KAGEMUSHA_APP_AUTHORITY_ED25519_KEY_FRAME_BYTES_V1
        || AUTHORITY_DOMAIN.len() + 8 != policy_opening.authority_key_frame.start
        || policy_opening.maximum_lifetime_ms.end != policy_opening.bytes.len()
    {
        return Err("Apple governed profile/policy transcript width changed".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let profile_bytes = guard_bundle::assign_bytes(ctx, &range, &profile_frame);
    let layout =
        kagemusha_hardware_profile_id_preimage_layout_v1().map_err(|error| error.to_string())?;
    for (byte, fixed) in profile_bytes.iter().zip(layout) {
        if let Some(fixed) = fixed {
            gate.assert_is_const(
                ctx,
                &byte.assigned().expect("profile frame byte assigned"),
                &F::from(u64::from(fixed)),
            );
        }
    }
    let field = |index: usize| {
        &profile_bytes[KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_FIELD_RANGES_V1[index].clone()]
    };
    for (actual, expected) in [
        (
            field(0),
            guard_bundle::constant_bytes(&KAGEMUSHA_WIRE_VERSION_V1.to_le_bytes()),
        ),
        (
            field(1),
            uint_le(ctx, &range, authenticated.protocol_version, 16),
        ),
        (field(3), guard_bundle::constant_bytes(&4_u32.to_le_bytes())),
        (
            field(11),
            guard_bundle::constant_bytes(&KAGEMUSHA_APPLE_APP_ATTEST_GUARANTEES_V1.to_le_bytes()),
        ),
        (
            field(9),
            uint_le(ctx, &range, authenticated.policy_epoch, 64),
        ),
    ] {
        for (left, right) in actual.iter().zip(expected) {
            let left = left.assigned().expect("profile field assigned");
            let right = gate.add(
                ctx,
                right.quantum_cell(),
                halo2_base::QuantumCell::Constant(F::ZERO),
            );
            ctx.constrain_equal(&left, &right);
        }
    }
    let mut profile_message = guard_bundle::constant_bytes(PROFILE_DOMAIN);
    profile_message.push(PastaSha256ByteV1::constant(0));
    profile_message.extend(guard_bundle::constant_bytes(
        &(KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1 as u64).to_le_bytes(),
    ));
    profile_message.extend(profile_bytes.iter().copied());
    let profile_digest = guard_bundle::hash(ctx, jobs, profile_message)?;
    bind_digest_to_limbs(ctx, &profile_digest, authenticated.profile_id);
    bind_digest_to_limbs(ctx, &profile_digest, authenticated.guard_profile_id);

    let policy_bytes = guard_bundle::assign_bytes(ctx, &range, &policy_opening.bytes);
    for (actual, expected) in policy_bytes[..AUTHORITY_DOMAIN.len()]
        .iter()
        .zip(AUTHORITY_DOMAIN)
    {
        gate.assert_is_const(
            ctx,
            &actual.assigned().expect("policy domain byte assigned"),
            &F::from(u64::from(*expected)),
        );
    }
    let key_frame_len = policy_opening.authority_key_frame.len() as u64;
    for (actual, expected) in policy_bytes[AUTHORITY_DOMAIN.len()..AUTHORITY_DOMAIN.len() + 8]
        .iter()
        .zip(key_frame_len.to_le_bytes())
    {
        gate.assert_is_const(
            ctx,
            &actual.assigned().expect("policy key length byte assigned"),
            &F::from(u64::from(expected)),
        );
    }
    for (actual, expected) in policy_bytes[policy_opening.platform_class.clone()]
        .iter()
        .zip(field(3).iter())
    {
        ctx.constrain_equal(
            &actual.assigned().expect("policy class assigned"),
            &expected.assigned().expect("profile class assigned"),
        );
    }
    let class = &policy_bytes[policy_opening.platform_class.clone()];
    gate.assert_is_const(
        ctx,
        &class[0].assigned().expect("policy class assigned"),
        &F::from(4_u64),
    );
    let rp = &policy_bytes[policy_opening.app_signing_identity_digest.clone()];
    let app_release = &policy_bytes[policy_opening.app_release_digest.clone()];
    nonzero_bytes(ctx, &range, rp);
    nonzero_bytes(ctx, &range, app_release);
    nonzero_bytes(
        ctx,
        &range,
        &policy_bytes[policy_opening.maximum_lifetime_ms.clone()],
    );
    for (governed, signed) in rp.iter().zip(&authenticator_data[..32]) {
        range.range_check(ctx, *signed, 8);
        ctx.constrain_equal(&governed.assigned().expect("governed RP assigned"), signed);
    }
    let policy_digest = guard_bundle::hash(ctx, jobs, policy_bytes.clone())?;
    for (actual, expected) in policy_digest.iter().zip(field(15)) {
        ctx.constrain_equal(
            &actual.assigned().expect("policy SHA byte assigned"),
            &expected.assigned().expect("profile policy byte assigned"),
        );
    }
    let mut static_binding = guard_bundle::constant_bytes(STATIC_BINDING_DOMAIN);
    static_binding.extend_from_slice(rp);
    static_binding.extend_from_slice(app_release);
    for digest in [
        authenticated.release_id,
        authenticated.profile_id,
        authenticated.key_reference,
        authenticated.lane_id,
    ] {
        static_binding.extend(digest_le(ctx, &range, digest));
    }
    let bound_app = guard_bundle::hash(ctx, jobs, static_binding)?;
    for (actual, expected) in bound_app.iter().zip(authenticated.guard_app_binding) {
        ctx.constrain_equal(
            &actual
                .assigned()
                .expect("static app binding SHA byte assigned"),
            &expected
                .assigned()
                .expect("Guard app binding byte assigned"),
        );
    }
    Ok(AppleGovernedAppBytesV1 {
        rp_id_hash: rp
            .iter()
            .map(|byte| byte.assigned().expect("governed RP assigned"))
            .collect::<Vec<_>>()
            .try_into()
            .expect("RP width"),
        app_release_digest: app_release
            .iter()
            .map(|byte| byte.assigned().expect("governed app release assigned"))
            .collect::<Vec<_>>()
            .try_into()
            .expect("app release width"),
    })
}

/// Copy-bind signed selection identity to the same governed release, app and
/// credential cells opened by the recursively authenticated state and Guard.
///
/// The governed RP is already copy-equal to `authenticator_data` in the policy
/// opening. Its app release and state release enter the Guard app-binding SHA,
/// whose digest must equal the value in signed `S`. This is a staged relation:
/// the assertion signature, issuer credential and remaining signed Core fields
/// must also share these exact cells before monetary admission.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "staged monetary assertion fold remains closed")
)]
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_apple_governed_signed_identity_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    profile: &KagemushaHardwareProfileV1,
    policy: &KagemushaAppAttestationAuthorityPolicyV1,
    authenticated: AppleGovernedPolicyCellsV1<F>,
    canonical_s: &[AssignedValue<F>; KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES],
    authenticator_data: &[AssignedValue<F>; 37],
) -> Result<AppleGovernedAppBytesV1<F>, String> {
    use KagemushaHardwareSelectionSigningLayoutV1 as S;

    let app = constrain_apple_governed_policy_opening_v1(
        builder,
        jobs,
        profile,
        policy,
        authenticated,
        authenticator_data,
    )?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for (field, digest) in [
        (S::RELEASE_ID, authenticated.release_id),
        (S::HARDWARE_PROFILE_ID, authenticated.profile_id),
    ] {
        let expected = digest_le(ctx, &range, digest);
        if field.len() != expected.len() {
            return Err("Apple signed identity digest width changed".to_owned());
        }
        for (signed, governed) in canonical_s[field].iter().zip(expected) {
            range.range_check(ctx, *signed, 8);
            ctx.constrain_equal(
                signed,
                &governed.assigned().expect("governed digest byte assigned"),
            );
        }
    }
    for (field, digest) in [
        (S::APP_POLICY_DIGEST, authenticated.guard_app_binding),
        (S::CREDENTIAL_ID, authenticated.guard_credential_id),
    ] {
        if field.len() != digest.len() {
            return Err("Apple signed Guard identity width changed".to_owned());
        }
        for (signed, guarded) in canonical_s[field].iter().zip(digest) {
            range.range_check(ctx, *signed, 8);
            ctx.constrain_equal(
                signed,
                &guarded.assigned().expect("Guard identity byte assigned"),
            );
        }
    }
    Ok(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::{kagemusha_v1_poseidon::from_u128, pasta_sha256::PastaSha256ConfigV1};
    use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig};
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::kagemusha::{
        KagemushaDevicePublicKeyV1, KagemushaHardwarePlatformClassV1,
        kagemusha_app_enrollment_v1::KagemushaAppDevicePolicyBindingV1,
        kagemusha_suite_commitment_v1,
    };
    use p256::ecdsa::SigningKey;

    const TEST_K: u32 = 18;
    const UNUSABLE_ROWS: usize = 9;

    #[derive(Clone, Debug)]
    struct Config<F: KagemushaPoseidonFieldV1> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }

    #[derive(Clone)]
    struct PolicyCircuit<F: KagemushaPoseidonFieldV1> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }

    impl<F: KagemushaPoseidonFieldV1> Circuit<F> for PolicyCircuit<F> {
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
            let usable_rows = (1_usize << params.k) - UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable_rows);
            Config {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("policy opening test uses parameterized Base config")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "governed Apple policy Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << TEST_K) - UNUSABLE_ROWS,
            )
        }
    }

    #[derive(Clone, Copy)]
    enum Mutation {
        None,
        AuthorityKey,
        PolicyRp,
        PolicyAppRelease,
        ProfilePolicy,
        ProfileClass,
        ProfileMask,
        GuardAppBinding,
        SignedRp,
        PolicyEpoch,
        SignedRelease,
        SignedProfile,
        SignedAppBinding,
        SignedCredentialId,
        GuardCredentialId,
    }

    fn assigned_digest<F: KagemushaPoseidonFieldV1>(
        ctx: &mut Context<F>,
        digest: [u8; 32],
    ) -> [AssignedValue<F>; 2] {
        std::array::from_fn(|half| {
            let bytes: [u8; 16] = digest[half * 16..half * 16 + 16]
                .try_into()
                .expect("digest limb");
            ctx.load_witness(from_u128(u128::from_le_bytes(bytes)))
        })
    }

    fn fixture() -> (
        KagemushaHardwareProfileV1,
        KagemushaAppAttestationAuthorityPolicyV1,
        [u8; 32],
        [u8; 32],
        [u8; 32],
        [u8; 32],
    ) {
        let authority = KeyPair::from_seed(vec![73; 32], Algorithm::Ed25519);
        let policy = KagemushaAppAttestationAuthorityPolicyV1 {
            authority_key: authority.public_key().clone(),
            platform_class: KagemushaHardwarePlatformClassV1::AppleAppAttest,
            app_signing_identity_digest: [0x31; 32],
            app_release_digest: [0x32; 32],
            maximum_lifetime_ms: 1_000,
        };
        let signing_key = SigningKey::from_bytes((&[7; 32]).into()).expect("governance key");
        let governance_credential_public_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(
            signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        )
        .expect("P-256 governance key");
        let profile = KagemushaHardwareProfileV1 {
            version: 1,
            protocol_version: 1,
            hardware_profile_id: [0; 32],
            provider_id: [1; 32],
            platform_class: KagemushaHardwarePlatformClassV1::AppleAppAttest,
            product_class_digest: [2; 32],
            firmware_policy_digest: [3; 32],
            enrollment_attestation_verifier_digest: [4; 32],
            attestation_trust_roots_digest: [5; 32],
            allowed_suite_commitment: kagemusha_suite_commitment_v1([6; 32]),
            policy_epoch: 7,
            governance_credential_public_key,
            capability_mask: KAGEMUSHA_APPLE_APP_ATTEST_GUARANTEES_V1,
            qualification_report_digest: [8; 32],
            valid_from_ms: 100,
            expires_at_ms: 10_000,
            app_attestation_authority_policy_digest: policy.canonical_digest().unwrap(),
        }
        .seal_hardware_profile_id()
        .expect("governed Apple profile");
        let release = [0x21; 32];
        let key_reference = [0x22; 32];
        let lane = [0x23; 32];
        let app_binding = KagemushaAppDevicePolicyBindingV1 {
            app_signing_identity_digest: policy.app_signing_identity_digest,
            app_release_digest: policy.app_release_digest,
            release_id: release,
            hardware_profile_id: profile.hardware_profile_id,
            device_key_reference: key_reference,
            lane_id: lane,
        }
        .canonical_digest()
        .unwrap();
        (profile, policy, release, key_reference, lane, app_binding)
    }

    fn check<F: KagemushaPoseidonFieldV1>(mutation: Mutation) -> bool {
        let (mut profile, mut policy, release, key_reference, lane, mut app_binding) = fixture();
        let authenticated_profile = profile.hardware_profile_id;
        let signed_app_binding = app_binding;
        let signed_credential_id = [0x24; 32];
        match mutation {
            Mutation::AuthorityKey => {
                policy.authority_key = KeyPair::from_seed(vec![74; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone();
            }
            Mutation::PolicyRp => policy.app_signing_identity_digest[0] ^= 1,
            Mutation::PolicyAppRelease => policy.app_release_digest[0] ^= 1,
            Mutation::ProfilePolicy => profile.app_attestation_authority_policy_digest[0] ^= 1,
            Mutation::ProfileClass => {
                profile.platform_class = KagemushaHardwarePlatformClassV1::AndroidKeyMint;
            }
            Mutation::ProfileMask => profile.capability_mask ^= 1,
            Mutation::GuardAppBinding => app_binding[0] ^= 1,
            _ => {}
        }
        let mut auth = [0_u8; 37];
        auth[..32].copy_from_slice(&[0x31; 32]);
        auth[32] = 0x40;
        auth[36] = 1;
        if matches!(mutation, Mutation::SignedRp) {
            auth[0] ^= 1;
        }
        let mut signed_s = [0_u8; KagemushaHardwareSelectionSigningLayoutV1::TOTAL_BYTES];
        signed_s[KagemushaHardwareSelectionSigningLayoutV1::RELEASE_ID].copy_from_slice(&release);
        signed_s[KagemushaHardwareSelectionSigningLayoutV1::HARDWARE_PROFILE_ID]
            .copy_from_slice(&authenticated_profile);
        signed_s[KagemushaHardwareSelectionSigningLayoutV1::APP_POLICY_DIGEST]
            .copy_from_slice(&signed_app_binding);
        signed_s[KagemushaHardwareSelectionSigningLayoutV1::CREDENTIAL_ID]
            .copy_from_slice(&signed_credential_id);
        match mutation {
            Mutation::SignedRelease => {
                signed_s[KagemushaHardwareSelectionSigningLayoutV1::RELEASE_ID.start] ^= 1;
            }
            Mutation::SignedProfile => {
                signed_s[KagemushaHardwareSelectionSigningLayoutV1::HARDWARE_PROFILE_ID.start] ^= 1;
            }
            Mutation::SignedAppBinding => {
                signed_s[KagemushaHardwareSelectionSigningLayoutV1::APP_POLICY_DIGEST.start] ^= 1;
            }
            Mutation::SignedCredentialId => {
                signed_s[KagemushaHardwareSelectionSigningLayoutV1::CREDENTIAL_ID.start] ^= 1;
            }
            _ => {}
        }
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize);
        let range = builder.range_chip();
        let ctx = builder.main(0);
        let authenticated = AppleGovernedPolicyCellsV1 {
            protocol_version: ctx.load_witness(F::from(1_u64)),
            profile_id: assigned_digest(ctx, authenticated_profile),
            guard_profile_id: assigned_digest(ctx, authenticated_profile),
            policy_epoch: ctx.load_witness(F::from(if matches!(mutation, Mutation::PolicyEpoch) {
                8_u64
            } else {
                7_u64
            })),
            release_id: assigned_digest(ctx, release),
            key_reference: assigned_digest(ctx, key_reference),
            lane_id: assigned_digest(ctx, lane),
            guard_app_binding: guard_bundle::assign_bytes(ctx, &range, &app_binding)
                .try_into()
                .expect("app binding width"),
            guard_credential_id: guard_bundle::assign_bytes(
                ctx,
                &range,
                &if matches!(mutation, Mutation::GuardCredentialId) {
                    [0x25; 32]
                } else {
                    signed_credential_id
                },
            )
            .try_into()
            .expect("credential ID width"),
        };
        let auth = std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(auth[index]))));
        let signed_s =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(signed_s[index]))));
        let mut jobs = PastaSha256JobsV1::default();
        if constrain_apple_governed_signed_identity_v1(
            &mut builder,
            &mut jobs,
            &profile,
            &policy,
            authenticated,
            &signed_s,
            &auth,
        )
        .is_err()
        {
            return false;
        }
        builder.calculate_params(Some(UNUSABLE_ROWS));
        let circuit = PolicyCircuit { builder, jobs };
        MockProver::run(TEST_K, &circuit, vec![])
            .expect("Apple governed policy circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn governed_apple_profile_and_signed_identity_are_constrained_in_both_pasta_fields() {
        assert!(check::<Fp>(Mutation::None));
        assert!(check::<Fq>(Mutation::None));
        for mutation in [
            Mutation::AuthorityKey,
            Mutation::PolicyRp,
            Mutation::PolicyAppRelease,
            Mutation::ProfilePolicy,
            Mutation::ProfileClass,
            Mutation::ProfileMask,
            Mutation::GuardAppBinding,
            Mutation::SignedRp,
            Mutation::PolicyEpoch,
            Mutation::SignedRelease,
            Mutation::SignedProfile,
            Mutation::SignedAppBinding,
            Mutation::SignedCredentialId,
            Mutation::GuardCredentialId,
        ] {
            assert!(!check::<Fp>(mutation));
            assert!(!check::<Fq>(mutation));
        }
    }
}
