//! Release-pinned recipient authorization for reserve-backed mint credits.
//!
//! The reserve must not trust a host-side credential check.  Each parity recursively verifies the
//! provider-issued platform credential, proves possession of the device-only authority, opens the
//! two fresh mint commitments, and binds the exact encrypted-credit bytes.  X25519 and
//! XChaCha20-Poly1305 stay in the qualified non-forking hardware service; the circuit authenticates
//! that service's one-use authorization over the exact key, opening and ciphertext transcript.

use ff::PrimeField as _;
use halo2_base::{
    AssignedValue, Context,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
};
use iroha_data_model::offline::{
    OFFLINE_CASH_ASSET_SCALE_MAX_V1, OFFLINE_CASH_HALO2_K_V1,
    OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1, OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1,
    OfflineCashCreditOpeningV1, OfflineCashEncryptedCreditEnvelopeV1,
    OfflineCashHardwareCredentialV1, OfflineCashHardwareProfileV1,
    OfflineCashMintAuthorizationStatementV1, offline_cash_asset_identity_digest_v1,
    offline_cash_ciphertext_digest_v1, offline_cash_mint_credit_opening_commitment_v1,
    offline_cash_recipient_credential_commitment_v1,
};
use sha2::{Digest as _, Sha256};
use snark_verifier::{pcs::ipa::IpaSuccinctVerifyingKey, verifier::plonk::PlonkProtocol};

use super::{
    DigestV1, OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1, OfflineCashEpAccumulatorV1,
    OfflineCashEqAccumulatorV1, OfflineCashPastaParityV1,
    commit_wrapper::{
        COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1, constrain_enabled_hardware_profile_membership_v1,
    },
    deferred_parent::{
        DeferredAccumulator, accumulator_limb_count, bind_accumulator_limbs,
        constrain_reciprocal_tagged_audit_v1, deferred_field_chips_v1, deferred_loader_v1,
        finalize_tagged_deferred_audit_v1, verify_ordinary_proof_v1,
    },
    guard_bundle::{
        AssignedCredentialV1, OfflineCashPlatformCredentialStatementV1, assert_digest_nonzero,
        assign_bytes, assign_credential_statement_v1, bind_equal_digest, constant_bytes,
        device_authority_commitment_v1, digest_limbs_assigned, hash,
    },
};
use crate::zk::{
    offline_cash_v1_poseidon::{OfflineCashPoseidonFieldV1, digest_limbs, from_u128},
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256ConfigV1, PastaSha256JobsV1},
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const DEVICE_AUTHORITY_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:device-proof-authority";
const HARDWARE_CREDENTIAL_ID_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:hardware-credential-id";
const HARDWARE_PROFILE_ID_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:hardware-profile";
const SUITE_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:suite-commitment";
const RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:recipient-credential-commitment";
const MINT_CREDIT_OPENING_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:mint-credit-opening-commitment";
const CIPHERTEXT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:ciphertext";
const ACCOUNT_IDENTITY_DIGEST_DOMAIN_V1: &[u8] = b"iroha:offline-cash:v1:account-identity";
const HARDWARE_AUTHORIZATION_DOMAIN_V1: &[u8] =
    b"iroha:offline-cash:v1:mint-hardware-authorization";
const MINT_AUTHORIZATION_CREDENTIAL_EQUATION_TAG_V1: u32 = 5;

const RECIPIENT_COMMITMENT_PREIMAGE_BYTES_V1: usize = 96;
const CREDIT_COMMITMENT_PREIMAGE_BYTES_V1: usize = 246;
const HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1: usize = 323;
const HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1: usize = 323;

/// Non-history public cells in one mint-authorization parity.
pub(crate) const MINT_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1: usize = 50;
/// Public cells in one mint-authorization parity, including the credential accumulator.
pub(crate) const MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1: usize =
    MINT_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1 + 34;

/// Public-instance offsets shared by both mint-authorization parities.
pub(crate) mod public_instance {
    pub(crate) const VERSION: usize = 0;
    pub(crate) const SEMANTIC_LO: usize = 1;
    pub(crate) const OPERATION_LO: usize = 3;
    pub(crate) const RELEASE_LO: usize = 5;
    pub(crate) const SUITE_LO: usize = 7;
    pub(crate) const VK_LO: usize = 9;
    pub(crate) const MANIFEST_LO: usize = 11;
    pub(crate) const NETWORK_LO: usize = 13;
    pub(crate) const ASSET_LO: usize = 15;
    pub(crate) const INCARNATION_LO: usize = 17;
    pub(crate) const SCALE: usize = 19;
    pub(crate) const POOL_LO: usize = 20;
    pub(crate) const AMOUNT: usize = 22;
    pub(crate) const PAYER_LO: usize = 23;
    pub(crate) const RECIPIENT_LO: usize = 25;
    pub(crate) const CREDENTIAL_LO: usize = 27;
    pub(crate) const PROFILE_LO: usize = 29;
    pub(crate) const POLICY_EPOCH: usize = 31;
    pub(crate) const RECIPIENT_COMMITMENT_LO: usize = 32;
    pub(crate) const CREDIT_COMMITMENT_LO: usize = 34;
    pub(crate) const RECIPIENT_KEY_LO: usize = 36;
    pub(crate) const ISSUANCE_LO: usize = 38;
    pub(crate) const CREDIT_ID_LO: usize = 40;
    pub(crate) const CIPHERTEXT_LO: usize = 42;
    pub(crate) const HARDWARE_AUTHORIZATION_LO: usize = 44;
    pub(crate) const EQ_AUDIT_LO: usize = 46;
    pub(crate) const EP_AUDIT_LO: usize = 48;
    pub(crate) const HISTORY_START: usize = 50;
}

/// Exact private recipient material consumed by both authorization parities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCashMintAuthorizationRelationWitnessV1 {
    /// Public statement which will be carried by the top-up request.
    pub statement: OfflineCashMintAuthorizationStatementV1,
    /// Exact enabled profile resolved from the authenticated release.
    pub hardware_profile: OfflineCashHardwareProfileV1,
    /// Compact credential issued to the recipient device.
    pub hardware_credential: OfflineCashHardwareCredentialV1,
    /// Provider-proof statement recursively authenticated by this relation.
    pub platform_credential: OfflineCashPlatformCredentialStatementV1,
    /// Device-only authority retained by the qualified hardware service.
    pub device_authority_secret: DigestV1,
    /// Fixed private credit opening protected by the encrypted envelope.
    pub credit_opening: OfflineCashCreditOpeningV1,
    /// Opaque non-exportable key-handle opening maintained by hardware.
    pub recipient_key_handle_opening: DigestV1,
    /// Fresh hardware authorization nonce preventing transcript reuse.
    pub hardware_authorization_nonce: DigestV1,
    /// Exact canonical encrypted-credit envelope bytes.
    pub encrypted_credit: Vec<u8>,
}

impl OfflineCashMintAuthorizationRelationWitnessV1 {
    /// Validate all non-authoritative shape and exact public/private bindings.
    pub fn validate_shape(&self) -> Result<(), String> {
        self.statement
            .validate_shape()
            .map_err(|error| format!("invalid mint authorization statement: {error}"))?;
        self.hardware_profile
            .validate()
            .map_err(|error| format!("invalid mint hardware profile: {error}"))?;
        self.hardware_credential
            .validate_against_profile(&self.hardware_profile)
            .map_err(|error| format!("invalid mint hardware credential: {error}"))?;
        self.platform_credential.validate()?;
        self.credit_opening
            .validate_shape_against(self.statement.credit_id, self.statement.context.amount)
            .map_err(|error| format!("invalid mint credit opening: {error}"))?;
        OfflineCashEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            self.statement.context.recipient_one_time_key,
        )
        .map_err(|error| format!("invalid mint encrypted-credit envelope: {error}"))?;

        let context = &self.statement.context;
        let credential = &self.hardware_credential;
        let platform = &self.platform_credential;
        let asset_id = offline_cash_asset_identity_digest_v1(&context.asset)
            .map_err(|error| error.to_string())?;
        let generation = u64::try_from(platform.hardware_epoch_generation)
            .map_err(|_| "platform credential hardware generation exceeds u64".to_owned())?;
        if self.device_authority_secret == [0; 32]
            || self.recipient_key_handle_opening == [0; 32]
            || self.hardware_authorization_nonce == [0; 32]
            || device_authority_commitment_v1(self.device_authority_secret)
                != platform.device_authority_commitment
            || context.release_id != platform.release_id
            || context.suite_id != platform.suite_id
            || context.network_id.as_bytes() != &platform.network_id
            || asset_id != platform.asset_id
            || context.asset_incarnation != platform.asset_incarnation
            || context.scale != platform.asset_scale
            || context.liability_pool_id != platform.liability_pool_id
            || context.hardware_profile_id != platform.hardware_profile_id
            || context.policy_epoch != platform.policy_epoch
            || context.hardware_profile_id != self.hardware_profile.hardware_profile_id
            || context.hardware_credential_id != credential.credential_id
            || credential.network_id != context.network_id
            || credential.hardware_profile_id != context.hardware_profile_id
            || credential.suite_id != context.suite_id
            || credential.policy_epoch != context.policy_epoch
            || credential.lane_commitment != platform.lane_id
            || credential.hardware_epoch_id != platform.hardware_epoch_id
            || credential.hardware_epoch_generation != generation
            || credential.device_key_reference != platform.key_reference
            || credential.device_public_key != platform.device_public_key
            || credential.credential_id != platform.credential_issuance_digest
            || context.recipient_credential_commitment
                != offline_cash_recipient_credential_commitment_v1(
                    context.operation_id,
                    context.hardware_credential_id,
                    self.credit_opening.recipient_binding_opening,
                )
                .map_err(|error| error.to_string())?
            || context.credit_commitment
                != offline_cash_mint_credit_opening_commitment_v1(
                    &context.network_id,
                    &context.asset,
                    context.asset_incarnation,
                    context.scale,
                    context.liability_pool_id,
                    context.amount,
                    &context.recipient,
                    context.recipient_one_time_key,
                    self.credit_opening.credit_commitment_opening,
                )
                .map_err(|error| error.to_string())?
            || self.statement.ciphertext_digest
                != offline_cash_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err("mint authorization public/private relation mismatch".to_owned());
        }
        if canonical_hardware_credential_id_v1(credential) != credential.credential_id {
            return Err("hardware credential canonical layout drift".to_owned());
        }
        if canonical_hardware_profile_id_v1(&self.hardware_profile)
            != self.hardware_profile.hardware_profile_id
        {
            return Err("hardware profile canonical layout drift".to_owned());
        }
        Ok(())
    }

    /// Derive the one-use hardware authorization exposed by both proof parities.
    pub fn hardware_authorization_digest(&self) -> Result<DigestV1, String> {
        self.validate_shape()?;
        let semantic = self
            .statement
            .canonical_digest()
            .map_err(|error| error.to_string())?;
        let opening = self
            .credit_opening
            .canonical_bytes()
            .map_err(|error| error.to_string())?;
        Ok(hardware_authorization_digest_v1(
            semantic,
            self.platform_credential.canonical_digest(),
            self.statement.context.recipient_credential_commitment,
            self.statement.context.credit_commitment,
            self.statement.context.recipient_one_time_key,
            self.statement.ciphertext_digest,
            Sha256::digest(opening).into(),
            self.recipient_key_handle_opening,
            self.hardware_authorization_nonce,
            self.device_authority_secret,
        ))
    }
}

/// Complete paired credential-proof inputs for one mint authorization.
pub(crate) struct OfflineCashMintAuthorizationRecursiveWitnessV1<'a> {
    pub(crate) relation: OfflineCashMintAuthorizationRelationWitnessV1,
    pub(crate) enabled_hardware_profiles: [DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
    pub(crate) eq_credential_protocol: &'a PlonkProtocol<EqAffine>,
    pub(crate) eq_credential_proof: &'a [u8],
    pub(crate) eq_credential_history: &'a OfflineCashEqAccumulatorV1,
    pub(crate) ep_credential_protocol: &'a PlonkProtocol<EpAffine>,
    pub(crate) ep_credential_proof: &'a [u8],
    pub(crate) ep_credential_history: &'a OfflineCashEpAccumulatorV1,
    pub(crate) eq_deferred_audit: DigestV1,
    pub(crate) ep_deferred_audit: DigestV1,
}

/// Base, SHA-256 and reciprocal dense-MSM configuration for one authorization parity.
#[derive(Clone, Debug)]
pub(crate) struct OfflineCashMintAuthorizationCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp mint-authorization circuit.
#[derive(Clone)]
pub(crate) struct OfflineCashMintAuthorizationEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
    sha_jobs: PastaSha256JobsV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq mint-authorization circuit.
#[derive(Clone)]
pub(crate) struct OfflineCashMintAuthorizationEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
    sha_jobs: PastaSha256JobsV1<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

macro_rules! impl_mint_authorization_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = OfflineCashMintAuthorizationCircuitConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                    sha_jobs: self.sha_jobs.unknown(),
                    dense_jobs: self.dense_jobs.unknown(),
                }
            }

            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable_rows);
                OfflineCashMintAuthorizationCircuitConfigV1 {
                    base,
                    sha: PastaSha256ConfigV1::configure(meta),
                    dense: PastaDenseMsmConfigV1::configure::<$opposite>(meta),
                }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let usable_rows = (1_usize << self.builder.config_params.k) - MINIMUM_UNUSABLE_ROWS;
                <BaseCircuitBuilder<$field> as Circuit<$field>>::synthesize(
                    &self.builder,
                    config.base,
                    layouter.namespace(|| concat!($label, " Base")),
                )?;
                self.sha_jobs.synthesize(
                    &config.sha,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    usable_rows,
                )?;
                self.dense_jobs.synthesize(
                    &config.dense,
                    &mut layouter,
                    &self.builder.core().copy_manager,
                    self.builder.witness_gen_only(),
                    usable_rows,
                )
            }
        }
    };
}

impl_mint_authorization_circuit!(
    OfflineCashMintAuthorizationEqCircuitV1,
    Fp,
    EpAffine,
    "Offline Cash Eq mint authorization"
);
impl_mint_authorization_circuit!(
    OfflineCashMintAuthorizationEpCircuitV1,
    Fq,
    EqAffine,
    "Offline Cash Ep mint authorization"
);

/// Build the release-pinned, mutually audited mint-authorization pair.
pub(crate) fn build_offline_cash_mint_authorization_pair_v1(
    eq_svk: &IpaSuccinctVerifyingKey<EqAffine>,
    ep_svk: &IpaSuccinctVerifyingKey<EpAffine>,
    witness: OfflineCashMintAuthorizationRecursiveWitnessV1<'_>,
) -> Result<
    (
        OfflineCashMintAuthorizationEqCircuitV1,
        OfflineCashMintAuthorizationEpCircuitV1,
        DigestV1,
        DigestV1,
    ),
    String,
> {
    witness.relation.validate_shape()?;
    validate_enabled_profiles(&witness.enabled_hardware_profiles)?;
    validate_audits(witness.eq_deferred_audit, witness.ep_deferred_audit)?;
    let hardware_authorization = witness.relation.hardware_authorization_digest()?;
    let (mut eq_builder, eq_sha, eq_output) = build_scalar_half_v1::<EqAffine>(
        eq_svk,
        OfflineCashPastaParityV1::Eq,
        &witness.relation,
        &witness.enabled_hardware_profiles,
        witness.eq_credential_protocol,
        witness.eq_credential_proof,
        witness.eq_credential_history.as_bytes(),
        hardware_authorization,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
    )?;
    let (mut ep_builder, ep_sha, ep_output) = build_scalar_half_v1::<EpAffine>(
        ep_svk,
        OfflineCashPastaParityV1::Ep,
        &witness.relation,
        &witness.enabled_hardware_profiles,
        witness.ep_credential_protocol,
        witness.ep_credential_proof,
        witness.ep_credential_history.as_bytes(),
        hardware_authorization,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
    )?;

    let eq_expected_ep = audit_cells(&eq_builder, public_instance::EP_AUDIT_LO)?;
    let mut eq_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_tagged_audit_v1::<EpAffine>(
        &mut eq_builder,
        &ep_output.audit,
        &ep_output.equation_selectors,
        &eq_expected_ep,
        MINT_AUTHORIZATION_CREDENTIAL_EQUATION_TAG_V1,
        &mut eq_dense,
    )?;
    let ep_expected_eq = audit_cells(&ep_builder, public_instance::EQ_AUDIT_LO)?;
    let mut ep_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_tagged_audit_v1::<EqAffine>(
        &mut ep_builder,
        &eq_output.audit,
        &eq_output.equation_selectors,
        &ep_expected_eq,
        MINT_AUTHORIZATION_CREDENTIAL_EQUATION_TAG_V1,
        &mut ep_dense,
    )?;

    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << OFFLINE_CASH_HALO2_K_V1) - MINIMUM_UNUSABLE_ROWS;
    eq_sha.validate_capacity(usable_rows)?;
    ep_sha.validate_capacity(usable_rows)?;
    eq_dense.validate_capacity(usable_rows)?;
    ep_dense.validate_capacity(usable_rows)?;
    let eq_audit = super::composite::assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    let ep_audit = super::composite::assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    Ok((
        OfflineCashMintAuthorizationEqCircuitV1 {
            builder: eq_builder,
            sha_jobs: eq_sha,
            dense_jobs: eq_dense,
        },
        OfflineCashMintAuthorizationEpCircuitV1 {
            builder: ep_builder,
            sha_jobs: ep_sha,
            dense_jobs: ep_dense,
        },
        eq_audit,
        ep_audit,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_scalar_half_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: OfflineCashPastaParityV1,
    relation: &OfflineCashMintAuthorizationRelationWitnessV1,
    enabled_profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
    credential_protocol: &PlonkProtocol<C>,
    credential_proof: &[u8],
    credential_history: &[u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
    hardware_authorization: DigestV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        PastaSha256JobsV1<C::ScalarExt>,
        super::deferred_parent::OfflineCashDeferredParentOutputV1<C>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: OfflineCashPoseidonFieldV1,
{
    if credential_protocol.num_instance != [2]
        || credential_proof.is_empty()
        || credential_proof.len() > OFFLINE_CASH_PARITY_PROOF_MAX_BYTES_V1
    {
        return Err("mint-authorization credential proof has wrong fixed shape".to_owned());
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(usize::try_from(OFFLINE_CASH_HALO2_K_V1).expect("k fits usize"))
        .use_lookup_bits(
            usize::try_from(OFFLINE_CASH_HALO2_K_V1 - 1).expect("lookup bits fit usize"),
        )
        .use_instance_columns(1);
    let mut sha_jobs = PastaSha256JobsV1::default();
    let assigned = constrain_relation_v1(
        &mut builder,
        &mut sha_jobs,
        relation,
        enabled_profiles,
        hardware_authorization,
    )?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let eq_audit_bytes = assign_digest(ctx, &range, eq_audit);
    let ep_audit_bytes = assign_digest(ctx, &range, ep_audit);
    let history_instances = credential_history
        .chunks_exact(16)
        .map(|chunk| {
            let value = C::ScalarExt::from_u128(u128::from_le_bytes(
                chunk.try_into().expect("history limb width"),
            ));
            let assigned = ctx.load_witness(value);
            range.range_check(ctx, assigned, 128);
            assigned
        })
        .collect::<Vec<_>>();
    if history_instances.len() != accumulator_limb_count() {
        return Err("mint-authorization credential history has wrong shape".to_owned());
    }
    builder.assigned_instances = vec![
        assigned
            .public_prefix
            .into_iter()
            .chain(digest_limbs_assigned(ctx, &eq_audit_bytes))
            .chain(digest_limbs_assigned(ctx, &ep_audit_bytes))
            .chain(history_instances.iter().copied())
            .collect(),
    ];

    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let loaded_protocol = credential_protocol.loaded(&loader);
    let credential_instances = assigned
        .platform_credential_digest
        .into_iter()
        .map(|value| loader.scalar_from_assigned(value))
        .collect::<Vec<_>>();
    let credential_accumulator: DeferredAccumulator<'_, C> = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &loaded_protocol,
        &[credential_instances],
        credential_proof,
    )
    .map_err(|error| format!("mint-authorization credential verifier failed: {error:?}"))?;
    bind_accumulator_limbs(&loader, &credential_accumulator, &history_instances)
        .map_err(|error| format!("mint-authorization credential history failed: {error:?}"))?;
    let output = finalize_tagged_deferred_audit_v1(
        &mut builder,
        loader,
        MINT_AUTHORIZATION_CREDENTIAL_EQUATION_TAG_V1,
    )
    .map_err(|error| format!("mint-authorization credential audit failed: {error:?}"))?;
    let expected_offset = match parity {
        OfflineCashPastaParityV1::Eq => public_instance::EQ_AUDIT_LO,
        OfflineCashPastaParityV1::Ep => public_instance::EP_AUDIT_LO,
    };
    let expected = audit_cells(&builder, expected_offset)?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok((builder, sha_jobs, output))
}

struct AssignedAuthorizationV1<F: OfflineCashPoseidonFieldV1> {
    public_prefix: Vec<AssignedValue<F>>,
    platform_credential_digest: [AssignedValue<F>; 2],
}

fn constrain_relation_v1<F: OfflineCashPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    witness: &OfflineCashMintAuthorizationRelationWitnessV1,
    enabled_profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
    hardware_authorization: DigestV1,
) -> Result<AssignedAuthorizationV1<F>, String> {
    witness.validate_shape()?;
    let context = &witness.statement.context;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();

    let version = assign_uint_le(ctx, &range, u128::from(witness.statement.version), 16);
    gate.assert_is_const(ctx, &version.value, &F::ONE);
    let semantic = assign_digest(
        ctx,
        &range,
        witness
            .statement
            .canonical_digest()
            .map_err(|error| error.to_string())?,
    );
    let operation = assign_digest(ctx, &range, context.operation_id);
    let release = assign_digest(ctx, &range, context.release_id);
    let suite = assign_digest(ctx, &range, context.suite_id);
    let vk = assign_digest(ctx, &range, context.vk_digest);
    let manifest = assign_digest(ctx, &range, context.artifact_manifest_digest);
    let network = assign_digest(ctx, &range, *context.network_id.as_bytes());
    let asset = assign_digest(
        ctx,
        &range,
        offline_cash_asset_identity_digest_v1(&context.asset).map_err(|error| error.to_string())?,
    );
    let incarnation = assign_digest(ctx, &range, *context.asset_incarnation.as_bytes());
    let scale = assign_uint_le(ctx, &range, u128::from(context.scale), 32);
    let scale_ok = range.is_less_than_safe(
        ctx,
        scale.value,
        u64::from(OFFLINE_CASH_ASSET_SCALE_MAX_V1) + 1,
    );
    gate.assert_is_const(ctx, &scale_ok, &F::ONE);
    let pool = assign_digest(ctx, &range, context.liability_pool_id);
    let amount = assign_uint_le(ctx, &range, context.amount, 128);
    assert_nonzero(ctx, &range, amount.value);
    let payer = assign_digest(ctx, &range, account_identity_digest_v1(&context.payer)?);
    let recipient = assign_digest(ctx, &range, account_identity_digest_v1(&context.recipient)?);
    let credential_id = assign_digest(ctx, &range, context.hardware_credential_id);
    let profile_id = assign_digest(ctx, &range, context.hardware_profile_id);
    let policy_epoch = assign_uint_le(ctx, &range, u128::from(context.policy_epoch), 64);
    assert_nonzero(ctx, &range, policy_epoch.value);
    let recipient_commitment = assign_digest(ctx, &range, context.recipient_credential_commitment);
    let credit_commitment = assign_digest(ctx, &range, context.credit_commitment);
    let recipient_key = assign_digest(ctx, &range, context.recipient_one_time_key);
    let issuance = assign_digest(ctx, &range, witness.statement.issuance_commitment);
    let credit_id = assign_digest(ctx, &range, witness.statement.credit_id);
    let ciphertext = assign_digest(ctx, &range, witness.statement.ciphertext_digest);
    let hardware_authorization = assign_digest(ctx, &range, hardware_authorization);
    let opening_digest = assign_digest(
        ctx,
        &range,
        Sha256::digest(
            witness
                .credit_opening
                .canonical_bytes()
                .map_err(|error| error.to_string())?,
        )
        .into(),
    );
    for digest in [
        &semantic,
        &operation,
        &release,
        &suite,
        &vk,
        &manifest,
        &network,
        &asset,
        &incarnation,
        &pool,
        &payer,
        &recipient,
        &credential_id,
        &profile_id,
        &recipient_commitment,
        &credit_commitment,
        &recipient_key,
        &issuance,
        &credit_id,
        &ciphertext,
        &hardware_authorization,
        &opening_digest,
    ] {
        assert_digest_nonzero(ctx, &range, digest);
    }

    let enabled = ctx.load_constant(F::ONE);
    let profile_limbs = digest_limbs_assigned(ctx, &profile_id);
    constrain_enabled_hardware_profile_membership_v1(
        ctx,
        &range,
        enabled,
        profile_limbs,
        enabled_profiles,
    );
    let hardware_profile =
        bind_hardware_profile(ctx, &range, jobs, &witness.hardware_profile, &profile_id)?;

    let platform = assign_credential_statement_v1(ctx, &range, jobs, &witness.platform_credential)?;
    bind_platform_context(
        ctx,
        &range,
        &platform,
        &release,
        &suite,
        &network,
        &asset,
        &incarnation,
        scale.value,
        &pool,
        &profile_id,
        policy_epoch.value,
    )?;
    bind_compact_credential(
        ctx,
        &range,
        jobs,
        witness,
        &hardware_profile,
        &platform,
        &credential_id,
    )?;

    let recipient_opening = assign_digest(
        ctx,
        &range,
        witness.credit_opening.recipient_binding_opening,
    );
    let credit_opening = assign_digest(
        ctx,
        &range,
        witness.credit_opening.credit_commitment_opening,
    );
    assert_digest_nonzero(ctx, &range, &recipient_opening);
    assert_digest_nonzero(ctx, &range, &credit_opening);
    let expected_recipient_commitment = hash_framed(
        ctx,
        jobs,
        RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN_V1,
        RECIPIENT_COMMITMENT_PREIMAGE_BYTES_V1,
        [
            operation.to_vec(),
            credential_id.to_vec(),
            recipient_opening.to_vec(),
        ]
        .concat(),
    )?;
    bind_equal_digest(
        ctx,
        &range,
        &expected_recipient_commitment,
        &recipient_commitment,
    );
    let expected_credit_commitment = hash_framed(
        ctx,
        jobs,
        MINT_CREDIT_OPENING_COMMITMENT_DOMAIN_V1,
        CREDIT_COMMITMENT_PREIMAGE_BYTES_V1,
        [
            version.bytes.clone(),
            network.to_vec(),
            asset.to_vec(),
            incarnation.to_vec(),
            scale.bytes.clone(),
            pool.to_vec(),
            amount.bytes.clone(),
            recipient.to_vec(),
            recipient_key.to_vec(),
            credit_opening.to_vec(),
        ]
        .concat(),
    )?;
    bind_equal_digest(ctx, &range, &expected_credit_commitment, &credit_commitment);

    let encrypted = assign_bytes(ctx, &range, &witness.encrypted_credit);
    let expected_ciphertext = hash_framed(
        ctx,
        jobs,
        CIPHERTEXT_DIGEST_DOMAIN_V1,
        witness.encrypted_credit.len(),
        encrypted,
    )?;
    bind_equal_digest(ctx, &range, &expected_ciphertext, &ciphertext);

    let opening_version =
        assign_uint_le(ctx, &range, u128::from(witness.credit_opening.version), 16);
    let opening_credit_id = assign_digest(ctx, &range, witness.credit_opening.credit_id);
    let opening_amount = assign_uint_le(ctx, &range, witness.credit_opening.amount, 128);
    let recovery_nonce = assign_digest(ctx, &range, witness.credit_opening.recovery_nonce);
    ctx.constrain_equal(&opening_version.value, &version.value);
    ctx.constrain_equal(&opening_amount.value, &amount.value);
    bind_equal_digest(ctx, &range, &opening_credit_id, &credit_id);
    assert_digest_nonzero(ctx, &range, &recovery_nonce);
    let exact_opening_digest = hash(
        ctx,
        jobs,
        [
            opening_version.bytes,
            opening_credit_id.to_vec(),
            opening_amount.bytes,
            credit_opening.to_vec(),
            recipient_opening.to_vec(),
            recovery_nonce.to_vec(),
        ]
        .concat(),
    )?;
    bind_equal_digest(ctx, &range, &exact_opening_digest, &opening_digest);

    let device_secret = assign_digest(ctx, &range, witness.device_authority_secret);
    let key_handle = assign_digest(ctx, &range, witness.recipient_key_handle_opening);
    let authorization_nonce = assign_digest(ctx, &range, witness.hardware_authorization_nonce);
    for value in [&device_secret, &key_handle, &authorization_nonce] {
        assert_digest_nonzero(ctx, &range, value);
    }
    let expected_device_authority = hash(
        ctx,
        jobs,
        [
            constant_bytes(DEVICE_AUTHORITY_DOMAIN_V1),
            vec![PastaSha256ByteV1::constant(0)],
            device_secret.to_vec(),
        ]
        .concat(),
    )?;
    bind_equal_digest(
        ctx,
        &range,
        &expected_device_authority,
        &platform.device_authority_commitment,
    );
    let expected_hardware_authorization = hash(
        ctx,
        jobs,
        [
            constant_bytes(HARDWARE_AUTHORIZATION_DOMAIN_V1),
            vec![PastaSha256ByteV1::constant(0)],
            semantic.to_vec(),
            platform.digest.to_vec(),
            recipient_commitment.to_vec(),
            credit_commitment.to_vec(),
            recipient_key.to_vec(),
            ciphertext.to_vec(),
            opening_digest.to_vec(),
            key_handle.to_vec(),
            authorization_nonce.to_vec(),
            device_secret.to_vec(),
        ]
        .concat(),
    )?;
    bind_equal_digest(
        ctx,
        &range,
        &expected_hardware_authorization,
        &hardware_authorization,
    );

    let public_prefix = [
        vec![version.value],
        digest_limbs_assigned(ctx, &semantic).to_vec(),
        digest_limbs_assigned(ctx, &operation).to_vec(),
        digest_limbs_assigned(ctx, &release).to_vec(),
        digest_limbs_assigned(ctx, &suite).to_vec(),
        digest_limbs_assigned(ctx, &vk).to_vec(),
        digest_limbs_assigned(ctx, &manifest).to_vec(),
        digest_limbs_assigned(ctx, &network).to_vec(),
        digest_limbs_assigned(ctx, &asset).to_vec(),
        digest_limbs_assigned(ctx, &incarnation).to_vec(),
        vec![scale.value],
        digest_limbs_assigned(ctx, &pool).to_vec(),
        vec![amount.value],
        digest_limbs_assigned(ctx, &payer).to_vec(),
        digest_limbs_assigned(ctx, &recipient).to_vec(),
        digest_limbs_assigned(ctx, &credential_id).to_vec(),
        digest_limbs_assigned(ctx, &profile_id).to_vec(),
        vec![policy_epoch.value],
        digest_limbs_assigned(ctx, &recipient_commitment).to_vec(),
        digest_limbs_assigned(ctx, &credit_commitment).to_vec(),
        digest_limbs_assigned(ctx, &recipient_key).to_vec(),
        digest_limbs_assigned(ctx, &issuance).to_vec(),
        digest_limbs_assigned(ctx, &credit_id).to_vec(),
        digest_limbs_assigned(ctx, &ciphertext).to_vec(),
        digest_limbs_assigned(ctx, &hardware_authorization).to_vec(),
    ]
    .concat();
    if public_prefix.len() != public_instance::EQ_AUDIT_LO {
        return Err("mint-authorization public-prefix layout drift".to_owned());
    }
    Ok(AssignedAuthorizationV1 {
        public_prefix,
        platform_credential_digest: digest_limbs_assigned(ctx, &platform.digest),
    })
}

#[allow(clippy::too_many_arguments)]
fn bind_platform_context<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    platform: &AssignedCredentialV1<F>,
    release: &[PastaSha256ByteV1<F>; 32],
    suite: &[PastaSha256ByteV1<F>; 32],
    network: &[PastaSha256ByteV1<F>; 32],
    asset: &[PastaSha256ByteV1<F>; 32],
    incarnation: &[PastaSha256ByteV1<F>; 32],
    scale: AssignedValue<F>,
    pool: &[PastaSha256ByteV1<F>; 32],
    profile: &[PastaSha256ByteV1<F>; 32],
    policy_epoch: AssignedValue<F>,
) -> Result<(), String> {
    for (left, right) in [
        (&platform.release_id, release),
        (&platform.suite_id, suite),
        (&platform.network_id, network),
        (&platform.asset_id, asset),
        (&platform.asset_incarnation, incarnation),
        (&platform.liability_pool_id, pool),
        (&platform.hardware_profile_id, profile),
    ] {
        bind_equal_digest(ctx, range, left, right);
    }
    ctx.constrain_equal(&platform.asset_scale.value, &scale);
    ctx.constrain_equal(&platform.policy_epoch.value, &policy_epoch);
    Ok(())
}

struct AssignedHardwareProfileV1<F: OfflineCashPoseidonFieldV1> {
    firmware_policy_digest: [PastaSha256ByteV1<F>; 32],
    allowed_suite_commitment: [PastaSha256ByteV1<F>; 32],
    policy_epoch: AssignedUint<F>,
    valid_from_ms: AssignedUint<F>,
    expires_at_ms: AssignedUint<F>,
}

fn bind_hardware_profile<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    profile: &OfflineCashHardwareProfileV1,
    public_profile_id: &[PastaSha256ByteV1<F>; 32],
) -> Result<AssignedHardwareProfileV1<F>, String> {
    let gate = range.gate();
    let version = assign_uint_le(ctx, range, u128::from(profile.version), 16);
    let protocol_version = assign_uint_le(ctx, range, u128::from(profile.protocol_version), 16);
    gate.assert_is_const(ctx, &version.value, &F::ONE);
    gate.assert_is_const(ctx, &protocol_version.value, &F::ONE);
    let provider = assign_digest(ctx, range, profile.provider_id);
    let platform_class = assign_uint_le(
        ctx,
        range,
        u128::from(match profile.platform_class {
            iroha_data_model::offline::OfflineCashHardwarePlatformClassV1::AndroidOemService => 0_u8,
            iroha_data_model::offline::OfflineCashHardwarePlatformClassV1::AppleOemService => 1,
            iroha_data_model::offline::OfflineCashHardwarePlatformClassV1::DedicatedSecureElement => 2,
            iroha_data_model::offline::OfflineCashHardwarePlatformClassV1::OtherQualified => 3,
        }),
        32,
    );
    let product = assign_digest(ctx, range, profile.product_class_digest);
    let firmware = assign_digest(ctx, range, profile.firmware_policy_digest);
    let enrollment = assign_digest(ctx, range, profile.enrollment_attestation_verifier_digest);
    let trust_roots = assign_digest(ctx, range, profile.attestation_trust_roots_digest);
    let suite_commitment = assign_digest(ctx, range, profile.allowed_suite_commitment);
    let policy_epoch = assign_uint_le(ctx, range, u128::from(profile.policy_epoch), 64);
    let governance_key = assign_bytes(
        ctx,
        range,
        profile.governance_credential_public_key.as_sec1_bytes(),
    );
    let capabilities = assign_uint_le(ctx, range, u128::from(profile.capability_mask), 16);
    let qualification = assign_digest(ctx, range, profile.qualification_report_digest);
    let valid_from = assign_uint_le(ctx, range, u128::from(profile.valid_from_ms), 64);
    let expires = assign_uint_le(ctx, range, u128::from(profile.expires_at_ms), 64);
    for digest in [
        &provider,
        &product,
        &firmware,
        &enrollment,
        &trust_roots,
        &suite_commitment,
        &qualification,
    ] {
        assert_digest_nonzero(ctx, range, digest);
    }
    assert_nonzero(ctx, range, policy_epoch.value);
    gate.assert_is_const(
        ctx,
        &capabilities.value,
        &F::from(u64::from(OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1)),
    );
    gate.assert_is_const(
        ctx,
        &governance_key[0]
            .assigned()
            .expect("governance credential key byte"),
        &F::from(4),
    );
    let lifetime_valid = range.is_less_than(ctx, valid_from.value, expires.value, 64);
    gate.assert_is_const(ctx, &lifetime_valid, &F::ONE);
    let expected_profile = hash_framed(
        ctx,
        jobs,
        HARDWARE_PROFILE_ID_DOMAIN_V1,
        HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1,
        [
            version.bytes,
            protocol_version.bytes,
            provider.to_vec(),
            platform_class.bytes,
            product.to_vec(),
            firmware.to_vec(),
            enrollment.to_vec(),
            trust_roots.to_vec(),
            suite_commitment.to_vec(),
            policy_epoch.bytes.clone(),
            governance_key,
            capabilities.bytes,
            qualification.to_vec(),
            valid_from.bytes.clone(),
            expires.bytes.clone(),
        ]
        .concat(),
    )?;
    bind_equal_digest(ctx, range, &expected_profile, public_profile_id);
    Ok(AssignedHardwareProfileV1 {
        firmware_policy_digest: firmware,
        allowed_suite_commitment: suite_commitment,
        policy_epoch,
        valid_from_ms: valid_from,
        expires_at_ms: expires,
    })
}

fn bind_compact_credential<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    witness: &OfflineCashMintAuthorizationRelationWitnessV1,
    profile: &AssignedHardwareProfileV1<F>,
    platform: &AssignedCredentialV1<F>,
    public_credential_id: &[PastaSha256ByteV1<F>; 32],
) -> Result<(), String> {
    let credential = &witness.hardware_credential;
    let version = assign_uint_le(ctx, range, u128::from(credential.version), 16);
    let network = assign_digest(ctx, range, *credential.network_id.as_bytes());
    let credential_profile = assign_digest(ctx, range, credential.hardware_profile_id);
    let suite = assign_digest(ctx, range, credential.suite_id);
    let firmware = assign_digest(ctx, range, credential.firmware_policy_digest);
    let policy_epoch = assign_uint_le(ctx, range, u128::from(credential.policy_epoch), 64);
    let lane = assign_digest(ctx, range, credential.lane_commitment);
    let epoch = assign_digest(ctx, range, credential.hardware_epoch_id);
    let generation = assign_uint_le(
        ctx,
        range,
        u128::from(credential.hardware_epoch_generation),
        64,
    );
    let device_key = assign_bytes(ctx, range, credential.device_public_key.as_sec1_bytes());
    let key_reference = assign_digest(ctx, range, credential.device_key_reference);
    let issued = assign_uint_le(ctx, range, u128::from(credential.issued_at_ms), 64);
    let expires = assign_uint_le(ctx, range, u128::from(credential.expires_at_ms), 64);
    for (left, right) in [
        (&network, &platform.network_id),
        (&credential_profile, &platform.hardware_profile_id),
        (&suite, &platform.suite_id),
        (&lane, &platform.lane_id),
        (&epoch, &platform.epoch_id),
        (&key_reference, &platform.key_reference),
    ] {
        bind_equal_digest(ctx, range, left, right);
    }
    ctx.constrain_equal(&policy_epoch.value, &platform.policy_epoch.value);
    ctx.constrain_equal(&policy_epoch.value, &profile.policy_epoch.value);
    ctx.constrain_equal(&generation.value, &platform.epoch_generation.value);
    bind_equal_digest(ctx, range, &firmware, &profile.firmware_policy_digest);
    let expected_suite_commitment =
        hash_framed(ctx, jobs, SUITE_COMMITMENT_DOMAIN_V1, 32, suite.to_vec())?;
    bind_equal_digest(
        ctx,
        range,
        &expected_suite_commitment,
        &profile.allowed_suite_commitment,
    );
    let starts_before_profile =
        range.is_less_than(ctx, issued.value, profile.valid_from_ms.value, 64);
    let ends_after_profile =
        range.is_less_than(ctx, profile.expires_at_ms.value, expires.value, 64);
    let nonempty_lifetime = range.is_less_than(ctx, issued.value, expires.value, 64);
    range
        .gate()
        .assert_is_const(ctx, &starts_before_profile, &F::ZERO);
    range
        .gate()
        .assert_is_const(ctx, &ends_after_profile, &F::ZERO);
    range
        .gate()
        .assert_is_const(ctx, &nonempty_lifetime, &F::ONE);
    if device_key.len() != platform.device_public_key.len() {
        return Err("platform and compact credential device-key widths differ".to_owned());
    }
    for (left, right) in device_key.iter().zip(&platform.device_public_key) {
        ctx.constrain_equal(
            &left.assigned().expect("compact credential key byte"),
            &right.assigned().expect("platform credential key byte"),
        );
    }
    let expected_id = hash_framed(
        ctx,
        jobs,
        HARDWARE_CREDENTIAL_ID_DOMAIN_V1,
        HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1,
        [
            version.bytes,
            network.to_vec(),
            credential_profile.to_vec(),
            suite.to_vec(),
            firmware.to_vec(),
            policy_epoch.bytes,
            lane.to_vec(),
            epoch.to_vec(),
            generation.bytes,
            device_key,
            key_reference.to_vec(),
            issued.bytes,
            expires.bytes,
        ]
        .concat(),
    )?;
    bind_equal_digest(ctx, range, &expected_id, public_credential_id);
    bind_equal_digest(
        ctx,
        range,
        &expected_id,
        &platform.credential_issuance_digest,
    );
    Ok(())
}

fn validate_audits(eq: DigestV1, ep: DigestV1) -> Result<(), String> {
    if eq == [0; 32]
        || ep == [0; 32]
        || eq == ep
        || crate::zk::offline_cash_v1_poseidon::decode::<Fp>(eq).is_none()
        || crate::zk::offline_cash_v1_poseidon::decode::<Fq>(ep).is_none()
    {
        return Err("mint-authorization deferred audits are noncanonical".to_owned());
    }
    Ok(())
}

fn validate_enabled_profiles(
    profiles: &[DigestV1; COMMIT_WRAPPER_ENABLED_PROFILE_SLOTS_V1],
) -> Result<(), String> {
    let mut previous = None;
    let mut padding = false;
    for profile in profiles {
        if *profile == [0; 32] {
            padding = true;
            continue;
        }
        if padding || previous.is_some_and(|value| value >= *profile) {
            return Err(
                "mint-authorization enabled profiles must be a sorted distinct prefix".to_owned(),
            );
        }
        previous = Some(*profile);
    }
    if previous.is_none() {
        return Err("mint-authorization enabled profile table is empty".to_owned());
    }
    Ok(())
}

fn audit_cells<F: OfflineCashPoseidonFieldV1>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
) -> Result<[AssignedValue<F>; 2], String> {
    if builder
        .assigned_instances
        .first()
        .is_none_or(|column| column.len() != MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1)
    {
        return Err("mint-authorization public instance has wrong shape".to_owned());
    }
    builder.assigned_instances[0][offset..offset + 2]
        .try_into()
        .map_err(|_| "mint-authorization audit instance has wrong shape".to_owned())
}

#[derive(Clone)]
struct AssignedUint<F: OfflineCashPoseidonFieldV1> {
    value: AssignedValue<F>,
    bytes: Vec<PastaSha256ByteV1<F>>,
}

fn assign_uint_le<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: u128,
    bits: usize,
) -> AssignedUint<F> {
    let assigned = ctx.load_witness(from_u128(value));
    range.range_check(ctx, assigned, bits);
    let bit_cells = PastaSha256BitV1::decompose(ctx, range.gate(), assigned, bits);
    let bytes = bit_cells
        .chunks_exact(8)
        .map(|chunk| PastaSha256ByteV1::from_bits_le(ctx, range.gate(), chunk))
        .collect();
    AssignedUint {
        value: assigned,
        bytes,
    }
}

fn assign_digest<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: DigestV1,
) -> [PastaSha256ByteV1<F>; 32] {
    assign_bytes(ctx, range, &digest)
        .try_into()
        .expect("digest width")
}

fn assert_nonzero<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
) {
    let zero = range.gate().is_zero(ctx, value);
    range.gate().assert_is_const(ctx, &zero, &F::ZERO);
}

fn hash_framed<F: OfflineCashPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    domain: &[u8],
    preimage_len: usize,
    preimage: Vec<PastaSha256ByteV1<F>>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    if preimage.len() != preimage_len {
        return Err("mint-authorization framed hash preimage length drift".to_owned());
    }
    hash(
        ctx,
        jobs,
        [
            constant_bytes(domain),
            vec![PastaSha256ByteV1::constant(0)],
            constant_bytes(
                &u64::try_from(preimage_len)
                    .map_err(|_| "hash preimage length exceeds u64".to_owned())?
                    .to_le_bytes(),
            ),
            preimage,
        ]
        .concat(),
    )
}

fn account_identity_digest_v1(
    account: &iroha_data_model::account::AccountId,
) -> Result<DigestV1, String> {
    let bytes = norito::encode_canonical(account).map_err(|error| error.to_string())?;
    Ok(digest_framed_native_v1(
        ACCOUNT_IDENTITY_DIGEST_DOMAIN_V1,
        &bytes,
    ))
}

fn canonical_hardware_credential_id_v1(credential: &OfflineCashHardwareCredentialV1) -> DigestV1 {
    let mut bytes = Vec::with_capacity(HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1);
    bytes.extend_from_slice(&credential.version.to_le_bytes());
    bytes.extend_from_slice(credential.network_id.as_bytes());
    bytes.extend_from_slice(&credential.hardware_profile_id);
    bytes.extend_from_slice(&credential.suite_id);
    bytes.extend_from_slice(&credential.firmware_policy_digest);
    bytes.extend_from_slice(&credential.policy_epoch.to_le_bytes());
    bytes.extend_from_slice(&credential.lane_commitment);
    bytes.extend_from_slice(&credential.hardware_epoch_id);
    bytes.extend_from_slice(&credential.hardware_epoch_generation.to_le_bytes());
    bytes.extend_from_slice(credential.device_public_key.as_sec1_bytes());
    bytes.extend_from_slice(&credential.device_key_reference);
    bytes.extend_from_slice(&credential.issued_at_ms.to_le_bytes());
    bytes.extend_from_slice(&credential.expires_at_ms.to_le_bytes());
    assert_eq!(bytes.len(), HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1);
    digest_framed_native_v1(HARDWARE_CREDENTIAL_ID_DOMAIN_V1, &bytes)
}

fn canonical_hardware_profile_id_v1(profile: &OfflineCashHardwareProfileV1) -> DigestV1 {
    let mut bytes = Vec::with_capacity(HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1);
    bytes.extend_from_slice(&profile.version.to_le_bytes());
    bytes.extend_from_slice(&profile.protocol_version.to_le_bytes());
    bytes.extend_from_slice(&profile.provider_id);
    let platform_class = match profile.platform_class {
        iroha_data_model::offline::OfflineCashHardwarePlatformClassV1::AndroidOemService => 0_u32,
        iroha_data_model::offline::OfflineCashHardwarePlatformClassV1::AppleOemService => 1,
        iroha_data_model::offline::OfflineCashHardwarePlatformClassV1::DedicatedSecureElement => 2,
        iroha_data_model::offline::OfflineCashHardwarePlatformClassV1::OtherQualified => 3,
    };
    bytes.extend_from_slice(&platform_class.to_le_bytes());
    bytes.extend_from_slice(&profile.product_class_digest);
    bytes.extend_from_slice(&profile.firmware_policy_digest);
    bytes.extend_from_slice(&profile.enrollment_attestation_verifier_digest);
    bytes.extend_from_slice(&profile.attestation_trust_roots_digest);
    bytes.extend_from_slice(&profile.allowed_suite_commitment);
    bytes.extend_from_slice(&profile.policy_epoch.to_le_bytes());
    bytes.extend_from_slice(profile.governance_credential_public_key.as_sec1_bytes());
    bytes.extend_from_slice(&profile.capability_mask.to_le_bytes());
    bytes.extend_from_slice(&profile.qualification_report_digest);
    bytes.extend_from_slice(&profile.valid_from_ms.to_le_bytes());
    bytes.extend_from_slice(&profile.expires_at_ms.to_le_bytes());
    assert_eq!(bytes.len(), HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1);
    digest_framed_native_v1(HARDWARE_PROFILE_ID_DOMAIN_V1, &bytes)
}

fn digest_framed_native_v1(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

#[allow(clippy::too_many_arguments)]
fn hardware_authorization_digest_v1(
    semantic: DigestV1,
    platform_credential: DigestV1,
    recipient_commitment: DigestV1,
    credit_commitment: DigestV1,
    recipient_key: DigestV1,
    ciphertext: DigestV1,
    opening_digest: DigestV1,
    key_handle: DigestV1,
    authorization_nonce: DigestV1,
    device_secret: DigestV1,
) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(HARDWARE_AUTHORIZATION_DOMAIN_V1);
    hasher.update([0]);
    for digest in [
        semantic,
        platform_credential,
        recipient_commitment,
        credit_commitment,
        recipient_key,
        ciphertext,
        opening_digest,
        key_handle,
        authorization_nonce,
        device_secret,
    ] {
        hasher.update(digest);
    }
    hasher.finalize().into()
}

/// Construct one parity's exact verifier public column.
pub(crate) fn mint_authorization_public_instances_v1<F: OfflineCashPoseidonFieldV1>(
    statement: &OfflineCashMintAuthorizationStatementV1,
    hardware_authorization: DigestV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
    history: &[u8; OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<F>, String> {
    statement
        .validate_shape()
        .map_err(|error| format!("invalid mint authorization statement: {error}"))?;
    validate_audits(eq_audit, ep_audit)?;
    if hardware_authorization == [0; 32] {
        return Err("mint-authorization hardware digest is zero".to_owned());
    }
    let context = &statement.context;
    let mut output = vec![F::from(u64::from(statement.version))];
    for digest in [
        statement
            .canonical_digest()
            .map_err(|error| error.to_string())?,
        context.operation_id,
        context.release_id,
        context.suite_id,
        context.vk_digest,
        context.artifact_manifest_digest,
        *context.network_id.as_bytes(),
        offline_cash_asset_identity_digest_v1(&context.asset).map_err(|error| error.to_string())?,
        *context.asset_incarnation.as_bytes(),
    ] {
        output.extend(digest_limbs::<F>(digest));
    }
    output.push(F::from(u64::from(context.scale)));
    output.extend(digest_limbs::<F>(context.liability_pool_id));
    output.push(from_u128(context.amount));
    for digest in [
        account_identity_digest_v1(&context.payer)?,
        account_identity_digest_v1(&context.recipient)?,
        context.hardware_credential_id,
        context.hardware_profile_id,
    ] {
        output.extend(digest_limbs::<F>(digest));
    }
    output.push(F::from(context.policy_epoch));
    for digest in [
        context.recipient_credential_commitment,
        context.credit_commitment,
        context.recipient_one_time_key,
        statement.issuance_commitment,
        statement.credit_id,
        statement.ciphertext_digest,
        hardware_authorization,
        eq_audit,
        ep_audit,
    ] {
        output.extend(digest_limbs::<F>(digest));
    }
    for chunk in history.chunks_exact(16) {
        output.push(from_u128(u128::from_le_bytes(
            chunk.try_into().expect("history limb width"),
        )));
    }
    if output.len() != MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint-authorization public instance layout drift".to_owned());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        offline::{
            OFFLINE_CASH_WIRE_VERSION_V1, OfflineCashDevicePublicKeyV1,
            OfflineCashDeviceSignatureV1, OfflineCashHardwareCredentialV1,
            OfflineCashHardwarePlatformClassV1, OfflineCashHardwareProfileV1,
            offline_cash_device_key_reference_v1,
        },
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    use super::*;

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("P-256 signing key")
    }

    fn public_key(key: &SigningKey) -> OfflineCashDevicePublicKeyV1 {
        OfflineCashDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("device public key")
    }

    fn signature(key: &SigningKey) -> OfflineCashDeviceSignatureV1 {
        let signature: Signature = key.sign(b"mint-authorization-layout-test");
        let signature = signature.normalize_s().unwrap_or(signature);
        OfflineCashDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical signature")
    }

    #[test]
    fn fixed_profile_and_credential_projections_match_norito_identities() {
        let issuer = signing_key(0x31);
        let profile = OfflineCashHardwareProfileV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            protocol_version: OFFLINE_CASH_WIRE_VERSION_V1,
            hardware_profile_id: [0; 32],
            provider_id: [1; 32],
            platform_class: OfflineCashHardwarePlatformClassV1::DedicatedSecureElement,
            product_class_digest: [2; 32],
            firmware_policy_digest: [3; 32],
            enrollment_attestation_verifier_digest: [4; 32],
            attestation_trust_roots_digest: [5; 32],
            allowed_suite_commitment: [6; 32],
            policy_epoch: 7,
            governance_credential_public_key: public_key(&issuer),
            capability_mask: OFFLINE_CASH_HARDWARE_REQUIRED_CAPABILITIES_V1,
            qualification_report_digest: [8; 32],
            valid_from_ms: 9,
            expires_at_ms: 10_000,
        }
        .seal_hardware_profile_id()
        .expect("profile identity");
        assert_eq!(
            canonical_hardware_profile_id_v1(&profile),
            profile
                .expected_hardware_profile_id()
                .expect("canonical profile identity")
        );

        let device = signing_key(0x41);
        let device_public_key = public_key(&device);
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"offline-cash-v1-mint-authorization-layout",
            )));
        let credential = OfflineCashHardwareCredentialV1 {
            version: OFFLINE_CASH_WIRE_VERSION_V1,
            credential_id: [0; 32],
            network_id,
            hardware_profile_id: profile.hardware_profile_id,
            suite_id: [11; 32],
            firmware_policy_digest: profile.firmware_policy_digest,
            policy_epoch: profile.policy_epoch,
            lane_commitment: [12; 32],
            hardware_epoch_id: [13; 32],
            hardware_epoch_generation: 14,
            device_public_key,
            device_key_reference: offline_cash_device_key_reference_v1(&device_public_key),
            issued_at_ms: 15,
            expires_at_ms: 9_000,
            governance_signature: signature(&issuer),
        }
        .seal_credential_id()
        .expect("credential identity");
        assert_eq!(
            canonical_hardware_credential_id_v1(&credential),
            credential
                .expected_credential_id()
                .expect("canonical credential identity")
        );
    }
}
