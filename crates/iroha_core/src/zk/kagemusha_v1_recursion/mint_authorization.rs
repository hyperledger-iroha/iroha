//! Release-pinned recipient authorization for reserve-backed mint credits.
//!
//! The reserve must not trust a host-side credential check.  Each parity recursively verifies the
//! provider-issued platform credential, proves possession of the device-only authority, opens the
//! two fresh mint commitments, and binds the exact encrypted-credit bytes.  X25519 and
//! XChaCha20-Poly1305 stay in the qualified non-forking hardware service; the circuit authenticates
//! that service's one-use authorization over the exact key, opening and ciphertext transcript.

use ff::{Field as _, PrimeField as _};
use halo2_base::{
    AssignedValue, Context,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt},
};
use halo2_proofs::{
    arithmetic::best_multiexp,
    circuit::{Layouter, V1},
    halo2curves::{
        CurveAffine,
        group::{Curve as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    },
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ASSET_SCALE_MAX_V1, KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1,
    KAGEMUSHA_HALO2_K_V1, KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1,
    KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_FIELD_RANGES_V1,
    KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1,
    KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_FIELD_RANGES_V1,
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
    KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_BYTES_V1,
    KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
    KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_BYTES_V1,
    KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_FIELD_RANGES_V1, KagemushaCreditOpeningV1,
    KagemushaEncryptedCreditEnvelopeV1, KagemushaHardwareCredentialV1, KagemushaHardwareProfileV1,
    KagemushaMintAuthorizationStatementV1, kagemusha_asset_identity_digest_v1,
    kagemusha_ciphertext_digest_v1, kagemusha_credit_opening_canonical_layout_v1,
    kagemusha_hardware_credential_id_preimage_layout_v1,
    kagemusha_hardware_profile_id_preimage_layout_v1,
    kagemusha_mint_credit_opening_commitment_preimage_layout_v1,
    kagemusha_mint_credit_opening_commitment_v1,
    kagemusha_recipient_credential_commitment_preimage_layout_v1,
    kagemusha_recipient_credential_commitment_v1,
};
use sha2::{Digest as _, Sha256};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};

use super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KagemushaEpAccumulatorV1,
    KagemushaEpFoldProofV1, KagemushaEqAccumulatorV1, KagemushaEqFoldProofV1,
    KagemushaPastaParityV1,
    canonical_preimage::assemble_canonical_preimage_v1,
    deferred_parent::{
        DeferredAccumulator, KagemushaNativeDeferredBatchV1, accumulator_limb_count,
        bind_accumulator_limbs, constrain_reciprocal_native_batch_with_carrier_v1,
        deferred_field_chips_v1, deferred_loader_v1,
        derive_native_deferred_batch_with_u128_binding_v1, kagemusha_protocol_structure_digest_v1,
        load_and_constrain_parent_protocol_v1, load_native_accumulator,
        native_parent_protocol_digest_v1, ordinary_ipa_proof_profile_v1, verify_fold,
        verify_ordinary_proof_v1, verify_two_carrier_hybrid_ordinary_proof_and_stream_v1,
    },
    guard_bundle::{
        AssignedCredentialV1, KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1,
        KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1,
        KagemushaPlatformCredentialStatementV1, assert_digest_nonzero, assign_bytes,
        assign_credential_statement_v1, bind_equal_digest, constant_bytes,
        constrain_enabled_hardware_profile_membership_v1, device_authority_commitment_v1,
        digest_limbs_assigned, hash, platform_credential_public_instance,
    },
    mint_hash_claim_fold::{
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1,
        canonical_claim_carrier_binding_tail_v1, constrain_complete_claim_against_sha_jobs_v1,
        public_instance as hash_claim_public,
    },
};
use crate::zk::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, digest_limbs, from_u128},
    pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const DEVICE_AUTHORITY_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:device-proof-authority";
const HARDWARE_CREDENTIAL_ID_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:hardware-credential-id";
const HARDWARE_PROFILE_ID_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:hardware-profile";
const SUITE_COMMITMENT_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:suite-commitment";
const RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:recipient-credential-commitment";
const MINT_CREDIT_OPENING_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:mint-credit-opening-commitment";
const CIPHERTEXT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:ciphertext";
const ACCOUNT_IDENTITY_DIGEST_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:account-identity";
const HARDWARE_AUTHORIZATION_DOMAIN_V1: &[u8] = b"iroha:kagemusha:v1:mint-hardware-authorization";
const MINT_AUTHORIZATION_CREDENTIAL_EQUATION_TAG_V1: u32 = 5;
const MINT_AUTHORIZATION_HASH_CLAIM_EQUATION_TAG_V1: u32 = 14;
const _: () = assert!(KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1 == 14);

/// Non-history public cells in one mint-authorization parity.
pub(crate) const MINT_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1: usize = 50;
/// Public cells in one mint-authorization parity, including the complete recursive history.
pub(crate) const MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1: usize =
    MINT_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1 + 34;
/// Small semantic column of the internal authorization proof. The first 84
/// cells retain the transport ABI; the final four bind both proof-carrier
/// commitments across the paired parities.
pub(super) const MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1: usize =
    MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 + 4;

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
    pub(super) const EQ_CARRIER_COMMITMENT_LO: usize =
        super::MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1;
    pub(super) const EP_CARRIER_COMMITMENT_LO: usize = EQ_CARRIER_COMMITMENT_LO + 2;
}

/// Exact private recipient material consumed by both authorization parities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaMintAuthorizationRelationWitnessV1 {
    /// Public statement which will be carried by the top-up request.
    pub statement: KagemushaMintAuthorizationStatementV1,
    /// Exact enabled profile resolved from the authenticated release.
    pub hardware_profile: KagemushaHardwareProfileV1,
    /// Compact credential issued to the recipient device.
    pub hardware_credential: KagemushaHardwareCredentialV1,
    /// Provider-proof statement recursively authenticated by this relation.
    pub platform_credential: KagemushaPlatformCredentialStatementV1,
    /// Device-only authority retained by the qualified hardware service.
    pub device_authority_secret: DigestV1,
    /// Fixed private credit opening protected by the encrypted envelope.
    pub credit_opening: KagemushaCreditOpeningV1,
    /// Opaque non-exportable key-handle opening maintained by hardware.
    pub recipient_key_handle_opening: DigestV1,
    /// Fresh hardware authorization nonce preventing transcript reuse.
    pub hardware_authorization_nonce: DigestV1,
    /// Exact canonical encrypted-credit envelope bytes.
    pub encrypted_credit: Vec<u8>,
}

impl KagemushaMintAuthorizationRelationWitnessV1 {
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
        KagemushaEncryptedCreditEnvelopeV1::decode_canonical_shape_exact_against_recipient_key(
            &self.encrypted_credit,
            self.statement.context.recipient_one_time_key,
        )
        .map_err(|error| format!("invalid mint encrypted-credit envelope: {error}"))?;

        let context = &self.statement.context;
        let credential = &self.hardware_credential;
        let platform = &self.platform_credential;
        let asset_id = kagemusha_asset_identity_digest_v1(&context.asset)
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
                != kagemusha_recipient_credential_commitment_v1(
                    context.operation_id,
                    context.hardware_credential_id,
                    self.credit_opening.recipient_binding_opening,
                )
                .map_err(|error| error.to_string())?
            || context.credit_commitment
                != kagemusha_mint_credit_opening_commitment_v1(
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
                != kagemusha_ciphertext_digest_v1(&self.encrypted_credit)
        {
            return Err("mint authorization public/private relation mismatch".to_owned());
        }
        if canonical_hardware_credential_id_v1(credential)? != credential.credential_id {
            return Err("hardware credential canonical layout drift".to_owned());
        }
        if canonical_hardware_profile_id_v1(&self.hardware_profile)?
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

/// Complete paired credential and own-hash claim inputs for one mint authorization.
pub(crate) struct KagemushaMintAuthorizationRecursiveWitnessV1<'a> {
    pub(crate) relation: KagemushaMintAuthorizationRelationWitnessV1,
    pub(crate) enabled_hardware_profiles: [DigestV1; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
    pub(crate) eq_hash_claim_protocol_digest: DigestV1,
    pub(crate) ep_hash_claim_protocol_digest: DigestV1,
    pub(crate) eq_hash_shard_protocol_digest: DigestV1,
    pub(crate) ep_hash_shard_protocol_digest: DigestV1,
    pub(crate) eq_credential_protocol_digest: DigestV1,
    pub(crate) eq_credential_protocol: &'a PlonkProtocol<EqAffine>,
    pub(crate) eq_credential_instances: &'a [Vec<Fp>],
    pub(crate) eq_credential_proof: &'a [u8],
    pub(crate) eq_credential_claim_history: &'a KagemushaEqAccumulatorV1,
    pub(crate) eq_credential_history_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(crate) eq_hash_claim_protocol: &'a PlonkProtocol<EqAffine>,
    pub(crate) eq_hash_claim_instances: &'a [Vec<Fp>],
    pub(crate) eq_hash_claim_proof: &'a [u8],
    pub(crate) eq_hash_claim_history: &'a KagemushaEqAccumulatorV1,
    pub(crate) eq_hash_claim_history_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(crate) eq_hash_claim_merge_fold_proof: &'a KagemushaEqFoldProofV1,
    pub(crate) eq_successor_history: &'a KagemushaEqAccumulatorV1,
    pub(crate) ep_credential_protocol_digest: DigestV1,
    pub(crate) ep_credential_protocol: &'a PlonkProtocol<EpAffine>,
    pub(crate) ep_credential_instances: &'a [Vec<Fq>],
    pub(crate) ep_credential_proof: &'a [u8],
    pub(crate) ep_credential_claim_history: &'a KagemushaEpAccumulatorV1,
    pub(crate) ep_credential_history_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(crate) ep_hash_claim_protocol: &'a PlonkProtocol<EpAffine>,
    pub(crate) ep_hash_claim_instances: &'a [Vec<Fq>],
    pub(crate) ep_hash_claim_proof: &'a [u8],
    pub(crate) ep_hash_claim_history: &'a KagemushaEpAccumulatorV1,
    pub(crate) ep_hash_claim_history_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(crate) ep_hash_claim_merge_fold_proof: &'a KagemushaEpFoldProofV1,
    pub(crate) ep_successor_history: &'a KagemushaEpAccumulatorV1,
    pub(crate) eq_deferred_audit: DigestV1,
    pub(crate) ep_deferred_audit: DigestV1,
}

/// Base and reciprocal dense-MSM configuration for one authorization parity.
#[derive(Clone, Debug)]
pub(crate) struct KagemushaMintAuthorizationCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp mint-authorization circuit.
#[derive(Clone)]
pub(crate) struct KagemushaMintAuthorizationEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq mint-authorization circuit.
#[derive(Clone)]
pub(crate) struct KagemushaMintAuthorizationEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

macro_rules! impl_mint_authorization_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaMintAuthorizationCircuitConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
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
                KagemushaMintAuthorizationCircuitConfigV1 {
                    base,
                    dense: PastaDenseMsmConfigV1::configure::<$opposite>(meta),
                }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let result = self.synthesize(config, layouter);
                self.builder.reset_synthesis_state();
                result
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
    KagemushaMintAuthorizationEqCircuitV1,
    Fp,
    EpAffine,
    "Kagemusha Eq mint authorization"
);
impl_mint_authorization_circuit!(
    KagemushaMintAuthorizationEpCircuitV1,
    Fq,
    EqAffine,
    "Kagemusha Ep mint authorization"
);

/// Compact native Eq/Ep verifier audits retained after their scalar builders are dropped.
pub(crate) struct KagemushaMintAuthorizationDeferredAuditsV1 {
    eq: KagemushaNativeDeferredBatchV1<EqAffine>,
    ep: KagemushaNativeDeferredBatchV1<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
    eq_carrier_commitment: EqAffine,
    ep_carrier_commitment: EpAffine,
}

impl KagemushaMintAuthorizationDeferredAuditsV1 {
    #[must_use]
    pub(crate) const fn eq_digest(&self) -> DigestV1 {
        self.eq_digest
    }

    #[must_use]
    pub(crate) const fn ep_digest(&self) -> DigestV1 {
        self.ep_digest
    }

    #[must_use]
    pub(super) const fn eq_carrier_commitment(&self) -> EqAffine {
        self.eq_carrier_commitment
    }

    #[must_use]
    pub(super) const fn ep_carrier_commitment(&self) -> EpAffine {
        self.ep_carrier_commitment
    }
}

fn native_carrier_values_v1<C>(
    output: &KagemushaNativeDeferredBatchV1<C>,
) -> Result<Vec<C::ScalarExt>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    output
        .carrier_cells_v1()
        .map(|cells| cells.into_iter().map(|cell| *cell.value()).collect())
        .map_err(|error| format!("mint-authorization carrier shape is invalid: {error:?}"))
}

fn canonical_carrier_commitment_v1<C>(
    parameters: &ParamsIPA<C>,
    output: &KagemushaNativeDeferredBatchV1<C>,
) -> Result<C, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    let values = native_carrier_values_v1(output)?;
    let bases = parameters
        .get_g_lagrange()
        .get(..values.len())
        .ok_or_else(|| "mint-authorization carrier exceeds the IPA domain".to_owned())?;
    let commitment =
        (best_multiexp::<C>(&values, bases) + parameters.get_blind_base().to_curve()).to_affine();
    if bool::from(commitment.is_identity()) {
        return Err("mint-authorization carrier commitment is the identity".to_owned());
    }
    Ok(commitment)
}

fn point_u128_limbs_v1<C: CurveAffine>(point: C) -> [u128; 2] {
    let bytes = point.to_bytes();
    let bytes = bytes.as_ref();
    std::array::from_fn(|half| {
        u128::from_le_bytes(
            bytes[half * 16..(half + 1) * 16]
                .try_into()
                .expect("Pasta compressed point half has sixteen bytes"),
        )
    })
}

fn append_inner_carrier_commitments_v1<F: KagemushaPoseidonFieldV1>(
    semantic: &mut Vec<F>,
    eq_commitment: EqAffine,
    ep_commitment: EpAffine,
) -> Result<(), String> {
    if semantic.len() != MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("mint-authorization semantic prefix has the wrong shape".to_owned());
    }
    semantic.extend(
        point_u128_limbs_v1(eq_commitment)
            .into_iter()
            .chain(point_u128_limbs_v1(ep_commitment))
            .map(F::from_u128),
    );
    if semantic.len() != MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1 {
        return Err("mint-authorization inner semantic layout drift".to_owned());
    }
    Ok(())
}

fn validate_credential_public_instances_v1<F: KagemushaPoseidonFieldV1>(
    parity: KagemushaPastaParityV1,
    instances: &[Vec<F>],
    credential_digest: DigestV1,
    carried_history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<(), String> {
    if instances.len() != 1
        || instances[0].len() != KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err(format!(
            "{parity:?} mint-authorization credential public shape is not exactly one-by-{KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1}"
        ));
    }
    let column = &instances[0];
    if column.get(
        platform_credential_public_instance::CREDENTIAL_LO
            ..platform_credential_public_instance::CREDENTIAL_LO + 2,
    ) != Some(digest_limbs::<F>(credential_digest).as_slice())
    {
        return Err(format!(
            "{parity:?} mint-authorization credential digest does not match public rows 0..2"
        ));
    }
    let expected_history = carried_history
        .chunks_exact(16)
        .map(|chunk| {
            F::from_u128(u128::from_le_bytes(
                chunk.try_into().expect("history limb width"),
            ))
        })
        .collect::<Vec<_>>();
    if expected_history.len() != accumulator_limb_count()
        || column.get(platform_credential_public_instance::HISTORY_START..)
            != Some(expected_history.as_slice())
    {
        return Err(format!(
            "{parity:?} mint-authorization carried claim history does not match credential public rows 6..40"
        ));
    }
    Ok(())
}

fn validate_hash_claim_public_instances_v1<F: KagemushaPoseidonFieldV1>(
    parity: KagemushaPastaParityV1,
    instances: &[Vec<F>],
    carried_history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<[u128; KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1], String> {
    if instances.len() != 3
        || instances[0].len() != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || instances[1].len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || instances[2].len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err(format!(
            "{parity:?} mint-authorization hash-claim internal shape is not exactly [{KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1}, {KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1}, {KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1}]"
        ));
    }
    let expected_history = carried_history
        .chunks_exact(16)
        .map(|chunk| {
            F::from_u128(u128::from_le_bytes(
                chunk.try_into().expect("history limb width"),
            ))
        })
        .collect::<Vec<_>>();
    if expected_history.len() != accumulator_limb_count()
        || instances[0].get(
            hash_claim_public::HISTORY_START..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1,
        ) != Some(expected_history.as_slice())
    {
        return Err(format!(
            "{parity:?} mint-authorization carried hash-claim history does not match claim public rows 63..97"
        ));
    }
    canonical_claim_carrier_binding_tail_v1(instances).map_err(|error| {
        format!("{parity:?} mint-authorization hash-claim carrier binding is invalid: {error}")
    })
}

fn validate_recursive_witness_v1(
    witness: &KagemushaMintAuthorizationRecursiveWitnessV1<'_>,
) -> Result<DigestV1, String> {
    witness.relation.validate_shape()?;
    validate_enabled_profiles(&witness.enabled_hardware_profiles)?;
    validate_audits(witness.eq_deferred_audit, witness.ep_deferred_audit)?;
    if witness.eq_credential_protocol_digest == [0; 32]
        || witness.ep_credential_protocol_digest == [0; 32]
        || witness.eq_hash_claim_protocol_digest == [0; 32]
        || witness.ep_hash_claim_protocol_digest == [0; 32]
        || witness.eq_hash_shard_protocol_digest == [0; 32]
        || witness.ep_hash_shard_protocol_digest == [0; 32]
        || witness.eq_credential_protocol_digest == witness.ep_credential_protocol_digest
        || witness.eq_hash_claim_protocol_digest == witness.ep_hash_claim_protocol_digest
        || witness.eq_hash_shard_protocol_digest == witness.ep_hash_shard_protocol_digest
    {
        return Err("mint-authorization protocol identity is absent or parity-aliased".to_owned());
    }
    let eq_credential_protocol_digest = native_parent_protocol_digest_v1(
        witness.eq_credential_protocol,
        KagemushaPastaParityV1::Eq,
    )?;
    let ep_credential_protocol_digest = native_parent_protocol_digest_v1(
        witness.ep_credential_protocol,
        KagemushaPastaParityV1::Ep,
    )?;
    let eq_hash_claim_protocol_digest = native_parent_protocol_digest_v1(
        witness.eq_hash_claim_protocol,
        KagemushaPastaParityV1::Eq,
    )?;
    let ep_hash_claim_protocol_digest = native_parent_protocol_digest_v1(
        witness.ep_hash_claim_protocol,
        KagemushaPastaParityV1::Ep,
    )?;
    if eq_credential_protocol_digest != witness.eq_credential_protocol_digest
        || ep_credential_protocol_digest != witness.ep_credential_protocol_digest
        || eq_hash_claim_protocol_digest != witness.eq_hash_claim_protocol_digest
        || ep_hash_claim_protocol_digest != witness.ep_hash_claim_protocol_digest
    {
        return Err(
            "mint-authorization recursive protocol differs from its authenticated identity"
                .to_owned(),
        );
    }
    let credential_digest = witness.relation.platform_credential.canonical_digest();
    validate_credential_public_instances_v1(
        KagemushaPastaParityV1::Eq,
        witness.eq_credential_instances,
        credential_digest,
        witness.eq_credential_claim_history.as_bytes(),
    )?;
    validate_credential_public_instances_v1(
        KagemushaPastaParityV1::Ep,
        witness.ep_credential_instances,
        credential_digest,
        witness.ep_credential_claim_history.as_bytes(),
    )?;
    let eq_hash_claim_carrier_binding = validate_hash_claim_public_instances_v1(
        KagemushaPastaParityV1::Eq,
        witness.eq_hash_claim_instances,
        witness.eq_hash_claim_history.as_bytes(),
    )?;
    let ep_hash_claim_carrier_binding = validate_hash_claim_public_instances_v1(
        KagemushaPastaParityV1::Ep,
        witness.ep_hash_claim_instances,
        witness.ep_hash_claim_history.as_bytes(),
    )?;
    if eq_hash_claim_carrier_binding != ep_hash_claim_carrier_binding {
        return Err("mint-authorization Eq/Ep hash-claim carrier bindings do not match".to_owned());
    }
    witness.relation.hardware_authorization_digest()
}

/// Build the release-pinned, mutually audited mint-authorization pair.
pub(crate) fn build_kagemusha_mint_authorization_pair_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    eq_svk: &IpaSuccinctVerifyingKey<EqAffine>,
    ep_svk: &IpaSuccinctVerifyingKey<EpAffine>,
    witness: &KagemushaMintAuthorizationRecursiveWitnessV1<'_>,
) -> Result<
    (
        KagemushaMintAuthorizationEqCircuitV1,
        KagemushaMintAuthorizationEpCircuitV1,
        DigestV1,
        DigestV1,
    ),
    String,
> {
    let audits = derive_kagemusha_mint_authorization_deferred_audits_v1(
        eq_parameters,
        ep_parameters,
        eq_svk,
        ep_svk,
        witness,
    )?;
    let (eq, _) =
        build_kagemusha_mint_authorization_eq_v1(ep_parameters, eq_svk, witness, &audits)?;
    let (ep, _) =
        build_kagemusha_mint_authorization_ep_v1(eq_parameters, ep_svk, witness, &audits)?;
    Ok((eq, ep, audits.eq_digest, audits.ep_digest))
}

/// Discover both deferred recursive audits while retaining no paired Base circuit graphs.
///
/// Eq is reduced to its compact native equation witness and digest before Ep construction starts.
pub(crate) fn derive_kagemusha_mint_authorization_deferred_audits_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_parameters: &ParamsIPA<EpAffine>,
    eq_svk: &IpaSuccinctVerifyingKey<EqAffine>,
    ep_svk: &IpaSuccinctVerifyingKey<EpAffine>,
    witness: &KagemushaMintAuthorizationRecursiveWitnessV1<'_>,
) -> Result<KagemushaMintAuthorizationDeferredAuditsV1, String> {
    let hardware_authorization = validate_recursive_witness_v1(&witness)?;
    let eq_credential_claim_history =
        witness
            .eq_credential_claim_history
            .to_native()
            .map_err(|error| {
                format!("failed to decode Eq PlatformCredential claim history: {error}")
            })?;
    let eq_hash_claim_history = witness.eq_hash_claim_history.to_native().map_err(|error| {
        format!("failed to decode Eq MintAuthorization hash-claim history: {error}")
    })?;
    let (mut eq_builder, eq_output) = build_scalar_half_v1::<EqAffine>(
        eq_svk,
        KagemushaPastaParityV1::Eq,
        &witness.relation,
        &witness.enabled_hardware_profiles,
        witness.eq_credential_protocol_digest,
        witness.eq_credential_protocol,
        witness.eq_credential_instances,
        witness.eq_credential_proof,
        &eq_credential_claim_history,
        witness.eq_credential_history_fold_proof.as_bytes(),
        witness.eq_hash_claim_protocol_digest,
        witness.ep_hash_claim_protocol_digest,
        witness.eq_hash_shard_protocol_digest,
        witness.ep_hash_shard_protocol_digest,
        witness.eq_hash_claim_protocol,
        witness.eq_hash_claim_instances,
        witness.eq_hash_claim_proof,
        &eq_hash_claim_history,
        witness.eq_hash_claim_history_fold_proof.as_bytes(),
        witness.eq_hash_claim_merge_fold_proof.as_bytes(),
        witness.eq_successor_history.as_bytes(),
        hardware_authorization,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        None,
    )?;
    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let eq_digest = super::composite::assigned_digest_bytes(&eq_output.challenge_limbs)?;
    drop(eq_builder);
    halo2_proofs::release_allocator_slack();

    let ep_credential_claim_history =
        witness
            .ep_credential_claim_history
            .to_native()
            .map_err(|error| {
                format!("failed to decode Ep PlatformCredential claim history: {error}")
            })?;
    let ep_hash_claim_history = witness.ep_hash_claim_history.to_native().map_err(|error| {
        format!("failed to decode Ep MintAuthorization hash-claim history: {error}")
    })?;
    let (mut ep_builder, ep_output) = build_scalar_half_v1::<EpAffine>(
        ep_svk,
        KagemushaPastaParityV1::Ep,
        &witness.relation,
        &witness.enabled_hardware_profiles,
        witness.ep_credential_protocol_digest,
        witness.ep_credential_protocol,
        witness.ep_credential_instances,
        witness.ep_credential_proof,
        &ep_credential_claim_history,
        witness.ep_credential_history_fold_proof.as_bytes(),
        witness.eq_hash_claim_protocol_digest,
        witness.ep_hash_claim_protocol_digest,
        witness.eq_hash_shard_protocol_digest,
        witness.ep_hash_shard_protocol_digest,
        witness.ep_hash_claim_protocol,
        witness.ep_hash_claim_instances,
        witness.ep_hash_claim_proof,
        &ep_hash_claim_history,
        witness.ep_hash_claim_history_fold_proof.as_bytes(),
        witness.ep_hash_claim_merge_fold_proof.as_bytes(),
        witness.ep_successor_history.as_bytes(),
        hardware_authorization,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        None,
    )?;
    if eq_output.bound_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1
        || eq_output.bound_u128_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1
        || ep_output.bound_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1
        || ep_output.bound_u128_values.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1
    {
        return Err("mint-authorization hash-claim carrier binding count drifted".to_owned());
    }
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let ep_digest = super::composite::assigned_digest_bytes(&ep_output.challenge_limbs)?;
    drop(ep_builder);
    halo2_proofs::release_allocator_slack();

    let eq_carrier_commitment = canonical_carrier_commitment_v1(eq_parameters, &eq_output)?;
    let ep_carrier_commitment = canonical_carrier_commitment_v1(ep_parameters, &ep_output)?;
    Ok(KagemushaMintAuthorizationDeferredAuditsV1 {
        eq: eq_output,
        ep: ep_output,
        eq_digest,
        ep_digest,
        eq_carrier_commitment,
        ep_carrier_commitment,
    })
}

/// Build one exact Eq mint-authorization circuit from compact reciprocal audit material.
pub(crate) fn build_kagemusha_mint_authorization_eq_v1(
    ep_parameters: &ParamsIPA<EpAffine>,
    eq_svk: &IpaSuccinctVerifyingKey<EqAffine>,
    witness: &KagemushaMintAuthorizationRecursiveWitnessV1<'_>,
    audits: &KagemushaMintAuthorizationDeferredAuditsV1,
) -> Result<(KagemushaMintAuthorizationEqCircuitV1, Vec<Vec<Fp>>), String> {
    let hardware_authorization = validate_recursive_witness_v1(&witness)?;
    if witness.eq_deferred_audit != audits.eq_digest
        || witness.ep_deferred_audit != audits.ep_digest
    {
        return Err("mint-authorization metadata does not bind the derived audit pair".to_owned());
    }
    let mut semantic_instances = mint_authorization_public_instances_v1::<Fp>(
        &witness.relation.statement,
        hardware_authorization,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        witness.eq_successor_history.as_bytes(),
    )?;
    append_inner_carrier_commitments_v1(
        &mut semantic_instances,
        audits.eq_carrier_commitment,
        audits.ep_carrier_commitment,
    )?;
    let credential_claim_history =
        witness
            .eq_credential_claim_history
            .to_native()
            .map_err(|error| {
                format!("failed to decode Eq PlatformCredential claim history: {error}")
            })?;
    let hash_claim_history = witness.eq_hash_claim_history.to_native().map_err(|error| {
        format!("failed to decode Eq MintAuthorization hash-claim history: {error}")
    })?;
    let (mut builder, output) = build_scalar_half_v1::<EqAffine>(
        eq_svk,
        KagemushaPastaParityV1::Eq,
        &witness.relation,
        &witness.enabled_hardware_profiles,
        witness.eq_credential_protocol_digest,
        witness.eq_credential_protocol,
        witness.eq_credential_instances,
        witness.eq_credential_proof,
        &credential_claim_history,
        witness.eq_credential_history_fold_proof.as_bytes(),
        witness.eq_hash_claim_protocol_digest,
        witness.ep_hash_claim_protocol_digest,
        witness.eq_hash_shard_protocol_digest,
        witness.ep_hash_shard_protocol_digest,
        witness.eq_hash_claim_protocol,
        witness.eq_hash_claim_instances,
        witness.eq_hash_claim_proof,
        &hash_claim_history,
        witness.eq_hash_claim_history_fold_proof.as_bytes(),
        witness.eq_hash_claim_merge_fold_proof.as_bytes(),
        witness.eq_successor_history.as_bytes(),
        hardware_authorization,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        Some((audits.eq_carrier_commitment, audits.ep_carrier_commitment)),
    )?;
    let expected_ep = audit_cells(&builder, public_instance::EP_AUDIT_LO)?;
    let expected_ep_commitment = audit_cells(&builder, public_instance::EP_CARRIER_COMMITMENT_LO)?;
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    let ep_carrier_len = native_carrier_values_v1(&audits.ep)?.len();
    let ep_lagrange_bases = ep_parameters
        .get_g_lagrange()
        .get(..ep_carrier_len)
        .ok_or_else(|| "Ep mint-authorization carrier exceeds the IPA domain".to_owned())?;
    constrain_reciprocal_native_batch_with_carrier_v1::<EpAffine>(
        &mut builder,
        &audits.ep,
        &expected_ep,
        ep_lagrange_bases,
        ep_parameters.get_blind_base(),
        audits.ep_carrier_commitment,
        &expected_ep_commitment,
        &output.bound_values,
        &mut dense_jobs,
    )?;
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << KAGEMUSHA_HALO2_K_V1) - MINIMUM_UNUSABLE_ROWS;
    dense_jobs.validate_capacity(usable_rows)?;
    if super::composite::assigned_digest_bytes(&output.challenge_limbs)? != audits.eq_digest
        || native_carrier_values_v1(&output)? != native_carrier_values_v1(&audits.eq)?
    {
        return Err("Eq mint-authorization audit changed after exact public rebinding".to_owned());
    }
    let public_instances = vec![semantic_instances, native_carrier_values_v1(&output)?];
    Ok((
        KagemushaMintAuthorizationEqCircuitV1 {
            builder,
            dense_jobs,
        },
        public_instances,
    ))
}

/// Build one exact Ep mint-authorization circuit from compact reciprocal audit material.
pub(crate) fn build_kagemusha_mint_authorization_ep_v1(
    eq_parameters: &ParamsIPA<EqAffine>,
    ep_svk: &IpaSuccinctVerifyingKey<EpAffine>,
    witness: &KagemushaMintAuthorizationRecursiveWitnessV1<'_>,
    audits: &KagemushaMintAuthorizationDeferredAuditsV1,
) -> Result<(KagemushaMintAuthorizationEpCircuitV1, Vec<Vec<Fq>>), String> {
    let hardware_authorization = validate_recursive_witness_v1(&witness)?;
    if witness.eq_deferred_audit != audits.eq_digest
        || witness.ep_deferred_audit != audits.ep_digest
    {
        return Err("mint-authorization metadata does not bind the derived audit pair".to_owned());
    }
    let mut semantic_instances = mint_authorization_public_instances_v1::<Fq>(
        &witness.relation.statement,
        hardware_authorization,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        witness.ep_successor_history.as_bytes(),
    )?;
    append_inner_carrier_commitments_v1(
        &mut semantic_instances,
        audits.eq_carrier_commitment,
        audits.ep_carrier_commitment,
    )?;
    let credential_claim_history =
        witness
            .ep_credential_claim_history
            .to_native()
            .map_err(|error| {
                format!("failed to decode Ep PlatformCredential claim history: {error}")
            })?;
    let hash_claim_history = witness.ep_hash_claim_history.to_native().map_err(|error| {
        format!("failed to decode Ep MintAuthorization hash-claim history: {error}")
    })?;
    let (mut builder, output) = build_scalar_half_v1::<EpAffine>(
        ep_svk,
        KagemushaPastaParityV1::Ep,
        &witness.relation,
        &witness.enabled_hardware_profiles,
        witness.ep_credential_protocol_digest,
        witness.ep_credential_protocol,
        witness.ep_credential_instances,
        witness.ep_credential_proof,
        &credential_claim_history,
        witness.ep_credential_history_fold_proof.as_bytes(),
        witness.eq_hash_claim_protocol_digest,
        witness.ep_hash_claim_protocol_digest,
        witness.eq_hash_shard_protocol_digest,
        witness.ep_hash_shard_protocol_digest,
        witness.ep_hash_claim_protocol,
        witness.ep_hash_claim_instances,
        witness.ep_hash_claim_proof,
        &hash_claim_history,
        witness.ep_hash_claim_history_fold_proof.as_bytes(),
        witness.ep_hash_claim_merge_fold_proof.as_bytes(),
        witness.ep_successor_history.as_bytes(),
        hardware_authorization,
        witness.eq_deferred_audit,
        witness.ep_deferred_audit,
        Some((audits.eq_carrier_commitment, audits.ep_carrier_commitment)),
    )?;
    let expected_eq = audit_cells(&builder, public_instance::EQ_AUDIT_LO)?;
    let expected_eq_commitment = audit_cells(&builder, public_instance::EQ_CARRIER_COMMITMENT_LO)?;
    let mut dense_jobs = PastaDenseMsmJobsV1::default();
    let eq_carrier_len = native_carrier_values_v1(&audits.eq)?.len();
    let eq_lagrange_bases = eq_parameters
        .get_g_lagrange()
        .get(..eq_carrier_len)
        .ok_or_else(|| "Eq mint-authorization carrier exceeds the IPA domain".to_owned())?;
    constrain_reciprocal_native_batch_with_carrier_v1::<EqAffine>(
        &mut builder,
        &audits.eq,
        &expected_eq,
        eq_lagrange_bases,
        eq_parameters.get_blind_base(),
        audits.eq_carrier_commitment,
        &expected_eq_commitment,
        &output.bound_values,
        &mut dense_jobs,
    )?;
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << KAGEMUSHA_HALO2_K_V1) - MINIMUM_UNUSABLE_ROWS;
    dense_jobs.validate_capacity(usable_rows)?;
    if super::composite::assigned_digest_bytes(&output.challenge_limbs)? != audits.ep_digest
        || native_carrier_values_v1(&output)? != native_carrier_values_v1(&audits.ep)?
    {
        return Err("Ep mint-authorization audit changed after exact public rebinding".to_owned());
    }
    let public_instances = vec![semantic_instances, native_carrier_values_v1(&output)?];
    Ok((
        KagemushaMintAuthorizationEpCircuitV1 {
            builder,
            dense_jobs,
        },
        public_instances,
    ))
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_scalar_half_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    relation: &KagemushaMintAuthorizationRelationWitnessV1,
    enabled_profiles: &[DigestV1; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
    credential_protocol_digest: DigestV1,
    credential_protocol: &PlonkProtocol<C>,
    credential_public_instances: &[Vec<C::ScalarExt>],
    credential_proof: &[u8],
    credential_claim_history: &IpaAccumulator<C, NativeLoader>,
    credential_history_fold_proof: &[u8],
    eq_hash_claim_protocol_digest: DigestV1,
    ep_hash_claim_protocol_digest: DigestV1,
    eq_hash_shard_protocol_digest: DigestV1,
    ep_hash_shard_protocol_digest: DigestV1,
    hash_claim_protocol: &PlonkProtocol<C>,
    hash_claim_public_instances: &[Vec<C::ScalarExt>],
    hash_claim_proof: &[u8],
    hash_claim_history: &IpaAccumulator<C, NativeLoader>,
    hash_claim_history_fold_proof: &[u8],
    hash_claim_merge_fold_proof: &[u8],
    successor_history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    hardware_authorization: DigestV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
    carrier_commitments: Option<(EqAffine, EpAffine)>,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        KagemushaNativeDeferredBatchV1<C>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if credential_protocol.num_instance != [KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1]
        || credential_public_instances.len() != 1
        || credential_public_instances[0].len()
            != KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("mint-authorization credential proof has wrong fixed shape".to_owned());
    }
    if hash_claim_protocol.num_instance
        != [
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]
        || hash_claim_public_instances.len() != 3
        || hash_claim_public_instances[0].len()
            != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || hash_claim_public_instances[1].len()
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || hash_claim_public_instances[2].len()
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err(
            "mint-authorization terminal hash-claim proof has wrong fixed shape".to_owned(),
        );
    }
    let credential_proof_len = ordinary_ipa_proof_profile_v1(credential_protocol)
        .map_err(|error| {
            format!("mint-authorization credential proof profile is invalid: {error}")
        })?
        .byte_len;
    if credential_proof.len() != credential_proof_len {
        return Err("mint-authorization credential proof has wrong fixed shape".to_owned());
    }
    let (mut builder, claimed_sha, assigned) =
        mint_authorization_relation_builder_v1(relation, enabled_profiles, hardware_authorization)?;
    if claimed_sha.compression_blocks()? == 0 {
        return Err("mint-authorization emitted an empty typed SHA queue".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let eq_audit_bytes = assign_digest(ctx, &range, eq_audit);
    let ep_audit_bytes = assign_digest(ctx, &range, ep_audit);
    let expected_credential_protocol = digest_limbs::<C::ScalarExt>(credential_protocol_digest)
        .map(|value| ctx.load_constant(value));
    // These four identities have no state-public cells. Fixed columns make them properties of the
    // release-authenticated MintAuthorization VKs. Both parity claim columns expose all four and
    // are equality-bound below, so a mismatched claim/shard suite cannot cross the pair boundary.
    let eq_hash_claim_protocol = constant_digest(ctx, eq_hash_claim_protocol_digest);
    let ep_hash_claim_protocol = constant_digest(ctx, ep_hash_claim_protocol_digest);
    let eq_hash_shard_protocol = constant_digest(ctx, eq_hash_shard_protocol_digest);
    let ep_hash_shard_protocol = constant_digest(ctx, ep_hash_shard_protocol_digest);
    let history_instances = successor_history
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
        return Err("mint-authorization successor history has wrong shape".to_owned());
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
    let structure = kagemusha_protocol_structure_digest_v1(credential_protocol, parity)?;
    let loaded_protocol = load_and_constrain_parent_protocol_v1(
        &loader,
        credential_protocol,
        parity,
        structure,
        &expected_credential_protocol,
    )
    .map_err(|error| format!("mint-authorization credential protocol binding failed: {error:?}"))?;
    let credential_instances = credential_public_instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| loader.assign_scalar(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let credential_column = credential_instances
        .first()
        .ok_or_else(|| "mint-authorization credential public column is absent".to_owned())?;
    for (actual, expected) in credential_column
        .get(
            platform_credential_public_instance::CREDENTIAL_LO
                ..platform_credential_public_instance::CREDENTIAL_LO + 2,
        )
        .ok_or_else(|| "mint-authorization credential digest rows are absent".to_owned())?
        .iter()
        .zip(assigned.platform_credential_digest)
    {
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&actual.assigned(), &expected);
    }
    let credential_accumulator: DeferredAccumulator<'_, C> = verify_ordinary_proof_v1(
        &loader,
        succinct_vk,
        &loaded_protocol.protocol,
        &credential_instances,
        credential_proof,
    )
    .map_err(|error| format!("mint-authorization credential verifier failed: {error:?}"))?;
    let credential_proof_equation_count = loader.ecc_chip().equation_count();
    if credential_proof_equation_count == 0 {
        return Err("mint-authorization credential verifier emitted no equations".to_owned());
    }
    let carried_history = load_native_accumulator(&loader, credential_claim_history)
        .map_err(|error| format!("mint-authorization claim history load failed: {error:?}"))?;
    let carried_history_cells = credential_column
        .get(platform_credential_public_instance::HISTORY_START..)
        .ok_or_else(|| "mint-authorization credential claim-history rows are absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &carried_history, &carried_history_cells)
        .map_err(|error| format!("mint-authorization claim history binding failed: {error:?}"))?;
    drop(carried_history_cells);
    drop(credential_instances);
    let credential_complete = verify_fold(
        &loader,
        succinct_vk,
        &[credential_accumulator, carried_history],
        credential_history_fold_proof,
    )
    .map_err(|error| format!("mint-authorization credential history fold failed: {error:?}"))?;
    let credential_equation_count = loader.ecc_chip().equation_count();
    if credential_equation_count <= credential_proof_equation_count {
        return Err("mint-authorization credential-history fold emitted no equation".to_owned());
    }

    let expected_hash_claim_protocol = match parity {
        KagemushaPastaParityV1::Eq => eq_hash_claim_protocol,
        KagemushaPastaParityV1::Ep => ep_hash_claim_protocol,
    };
    let hash_claim_structure = kagemusha_protocol_structure_digest_v1(hash_claim_protocol, parity)?;
    let loaded_hash_claim = load_and_constrain_parent_protocol_v1(
        &loader,
        hash_claim_protocol,
        parity,
        hash_claim_structure,
        &expected_hash_claim_protocol,
    )
    .map_err(|error| format!("mint-authorization hash-claim protocol binding failed: {error:?}"))?;
    if loaded_hash_claim.protocol.num_instance
        != [
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]
    {
        return Err("mint-authorization terminal hash-claim protocol shape changed".to_owned());
    }
    let hash_claim_semantic = hash_claim_public_instances[0]
        .iter()
        .map(|value| loader.assign_scalar(*value))
        .collect::<Vec<_>>();
    let hash_claim_current = verify_two_carrier_hybrid_ordinary_proof_and_stream_v1(
        &loader,
        succinct_vk,
        &loaded_hash_claim.protocol,
        &hash_claim_semantic,
        match parity {
            KagemushaPastaParityV1::Eq => [
                [
                    hash_claim_public::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO,
                    hash_claim_public::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO + 1,
                ],
                [
                    hash_claim_public::EQ_PROOF_EP_CARRIER_COMMITMENT_LO,
                    hash_claim_public::EQ_PROOF_EP_CARRIER_COMMITMENT_LO + 1,
                ],
            ],
            KagemushaPastaParityV1::Ep => [
                [
                    hash_claim_public::EP_PROOF_EQ_CARRIER_COMMITMENT_LO,
                    hash_claim_public::EP_PROOF_EQ_CARRIER_COMMITMENT_LO + 1,
                ],
                [
                    hash_claim_public::EP_PROOF_EP_CARRIER_COMMITMENT_LO,
                    hash_claim_public::EP_PROOF_EP_CARRIER_COMMITMENT_LO + 1,
                ],
            ],
        },
        hash_claim_proof,
    )
    .map_err(|error| format!("mint-authorization hash-claim verifier failed: {error:?}"))?;
    let hash_claim_carrier_binding = hash_claim_semantic
        .get(
            KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
                ..hash_claim_public::CARRIER_BINDING_END,
        )
        .ok_or_else(|| "mint-authorization hash-claim carrier binding is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    if hash_claim_carrier_binding.len() != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1 {
        return Err("mint-authorization hash-claim carrier binding shape drifted".to_owned());
    }
    let hash_claim_verifier_equation_count = loader.ecc_chip().equation_count();
    if hash_claim_verifier_equation_count <= credential_equation_count {
        return Err("mint-authorization hash-claim verifier emitted no equation".to_owned());
    }
    let hash_claim_column =
        &hash_claim_semantic[..KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1];
    let carried_hash_claim_history = load_native_accumulator(&loader, hash_claim_history)
        .map_err(|error| format!("mint-authorization hash-claim history load failed: {error:?}"))?;
    let hash_claim_history_cells = hash_claim_column
        .get(hash_claim_public::HISTORY_START..)
        .ok_or_else(|| "mint-authorization hash-claim history rows are absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(
        &loader,
        &carried_hash_claim_history,
        &hash_claim_history_cells,
    )
    .map_err(|error| format!("mint-authorization hash-claim history binding failed: {error:?}"))?;
    {
        let chip = loader.ecc_chip();
        let mut loader_ctx = loader.ctx_mut();
        let assigned_claim = hash_claim_column
            .iter()
            .map(|value| *value.assigned())
            .collect::<Vec<_>>();
        constrain_complete_claim_against_sha_jobs_v1(
            loader_ctx.main(),
            chip.range(),
            &claimed_sha,
            &assigned_claim,
            parity,
            assigned.release_id,
            eq_hash_claim_protocol,
            ep_hash_claim_protocol,
            eq_hash_shard_protocol,
            ep_hash_shard_protocol,
        )?;
    }
    drop(claimed_sha);
    drop(hash_claim_history_cells);
    drop(hash_claim_semantic);
    let complete_hash_claim = verify_fold(
        &loader,
        succinct_vk,
        &[hash_claim_current.accumulator, carried_hash_claim_history],
        hash_claim_history_fold_proof,
    )
    .map_err(|error| format!("mint-authorization hash-claim history fold failed: {error:?}"))?;
    let hash_claim_fold_equation_count = loader.ecc_chip().equation_count();
    if hash_claim_fold_equation_count <= hash_claim_verifier_equation_count {
        return Err("mint-authorization hash-claim history fold emitted no equation".to_owned());
    }
    let successor = verify_fold(
        &loader,
        succinct_vk,
        &[credential_complete, complete_hash_claim],
        hash_claim_merge_fold_proof,
    )
    .map_err(|error| format!("mint-authorization hash-claim merge fold failed: {error:?}"))?;
    bind_accumulator_limbs(&loader, &successor, &history_instances)
        .map_err(|error| format!("mint-authorization successor history failed: {error:?}"))?;

    let equation_count = loader.ecc_chip().equation_count();
    if equation_count <= hash_claim_fold_equation_count {
        return Err("mint-authorization hash-claim merge fold emitted no equation".to_owned());
    }
    let mut equation_tags =
        vec![MINT_AUTHORIZATION_CREDENTIAL_EQUATION_TAG_V1; credential_equation_count];
    equation_tags.resize(
        equation_count,
        MINT_AUTHORIZATION_HASH_CLAIM_EQUATION_TAG_V1,
    );
    let assigned_selectors = (0..equation_count)
        .map(|_| loader.ctx_mut().main().load_constant(C::ScalarExt::ONE))
        .collect::<Vec<_>>();
    let output = derive_native_deferred_batch_with_u128_binding_v1(
        &mut builder,
        loader,
        equation_tags,
        assigned_selectors,
        &hash_claim_carrier_binding,
    )
    .map_err(|error| format!("mint-authorization recursive audit failed: {error:?}"))?;
    let expected_offset = match parity {
        KagemushaPastaParityV1::Eq => public_instance::EQ_AUDIT_LO,
        KagemushaPastaParityV1::Ep => public_instance::EP_AUDIT_LO,
    };
    let expected = audit_cells(&builder, expected_offset)?;
    for (actual, expected) in output.challenge_limbs.iter().zip(expected) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    let (eq_commitment, ep_commitment) =
        carrier_commitments.unwrap_or_else(|| (EqAffine::generator(), EpAffine::generator()));
    let commitment_values = point_u128_limbs_v1(eq_commitment)
        .into_iter()
        .chain(point_u128_limbs_v1(ep_commitment))
        .map(C::ScalarExt::from_u128)
        .map(|value| {
            let assigned = builder.main(0).load_witness(value);
            range.range_check(builder.main(0), assigned, 128);
            assigned
        })
        .collect::<Vec<_>>();
    builder
        .assigned_instances
        .first_mut()
        .ok_or_else(|| "mint-authorization semantic instance is missing".to_owned())?
        .extend(commitment_values);
    if builder.assigned_instances[0].len() != MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1 {
        return Err("mint-authorization inner semantic instance has wrong shape".to_owned());
    }
    let carrier = output
        .carrier_cells_v1()
        .map_err(|error| format!("mint-authorization carrier failed: {error:?}"))?;
    builder.assigned_instances.push(carrier);
    Ok((builder, output))
}

struct AssignedAuthorizationV1<F: KagemushaPoseidonFieldV1> {
    public_prefix: Vec<AssignedValue<F>>,
    platform_credential_digest: [AssignedValue<F>; 2],
    release_id: [AssignedValue<F>; 2],
}

fn mint_authorization_relation_builder_v1<F: KagemushaPoseidonFieldV1>(
    witness: &KagemushaMintAuthorizationRelationWitnessV1,
    enabled_profiles: &[DigestV1; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
    hardware_authorization: DigestV1,
) -> Result<
    (
        BaseCircuitBuilder<F>,
        PastaSha256JobsV1<F>,
        AssignedAuthorizationV1<F>,
    ),
    String,
> {
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(usize::try_from(KAGEMUSHA_HALO2_K_V1).expect("k fits usize"))
        .use_lookup_bits(usize::try_from(KAGEMUSHA_HALO2_K_V1 - 1).expect("lookup bits fit usize"))
        .use_instance_columns(2);
    let mut jobs = PastaSha256JobsV1::default();
    let assigned = constrain_relation_v1(
        &mut builder,
        &mut jobs,
        witness,
        enabled_profiles,
        hardware_authorization,
    )?;
    Ok((builder, jobs, assigned))
}

/// Extract the exact typed SHA queue emitted by the production mint-authorization relation.
///
/// This shares the relation builder used by the real recursive circuit, so the shard plan cannot
/// drift onto a second host encoder. The bytes remain a private proof plan and grant no authority
/// until the recursive consumer constrains this assigned queue against a completed claim.
#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn mint_authorization_sha_messages_v1<F>(
    witness: &KagemushaMintAuthorizationRelationWitnessV1,
    enabled_profiles: &[DigestV1; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
    hardware_authorization: DigestV1,
) -> Result<Vec<Vec<u8>>, String>
where
    F: KagemushaPoseidonFieldV1,
{
    let (builder, jobs, assigned) = mint_authorization_relation_builder_v1::<F>(
        witness,
        enabled_profiles,
        hardware_authorization,
    )?;
    let messages = jobs.canonical_messages()?;
    drop(assigned);
    drop(jobs);
    drop(builder);
    Ok(messages)
}

fn constrain_relation_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    witness: &KagemushaMintAuthorizationRelationWitnessV1,
    enabled_profiles: &[DigestV1; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
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
        kagemusha_asset_identity_digest_v1(&context.asset).map_err(|error| error.to_string())?,
    );
    let incarnation = assign_digest(ctx, &range, *context.asset_incarnation.as_bytes());
    let scale = assign_uint_le(ctx, &range, u128::from(context.scale), 32);
    let scale_ok = range.is_less_than_safe(
        ctx,
        scale.value,
        u64::from(KAGEMUSHA_ASSET_SCALE_MAX_V1) + 1,
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
    let recipient_preimage = assemble_canonical_preimage_v1(
        ctx,
        &range,
        &kagemusha_recipient_credential_commitment_preimage_layout_v1()
            .map_err(|error| error.to_string())?,
        &KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
        &[&operation, &credential_id, &recipient_opening],
    )?;
    let expected_recipient_commitment = hash_framed(
        ctx,
        jobs,
        RECIPIENT_CREDENTIAL_COMMITMENT_DOMAIN_V1,
        KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_BYTES_V1,
        recipient_preimage,
    )?;
    bind_equal_digest(
        ctx,
        &range,
        &expected_recipient_commitment,
        &recipient_commitment,
    );
    let credit_preimage = assemble_canonical_preimage_v1(
        ctx,
        &range,
        &kagemusha_mint_credit_opening_commitment_preimage_layout_v1()
            .map_err(|error| error.to_string())?,
        &KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
        &[
            &version.bytes,
            &network,
            &asset,
            &incarnation,
            &scale.bytes,
            &pool,
            &amount.bytes,
            &recipient,
            &recipient_key,
            &credit_opening,
        ],
    )?;
    let expected_credit_commitment = hash_framed(
        ctx,
        jobs,
        MINT_CREDIT_OPENING_COMMITMENT_DOMAIN_V1,
        KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_BYTES_V1,
        credit_preimage,
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
    let exact_opening = assemble_canonical_preimage_v1(
        ctx,
        &range,
        &kagemusha_credit_opening_canonical_layout_v1().map_err(|error| error.to_string())?,
        &KAGEMUSHA_CREDIT_OPENING_CANONICAL_FIELD_RANGES_V1,
        &[
            &opening_version.bytes,
            &opening_credit_id,
            &opening_amount.bytes,
            &credit_opening,
            &recipient_opening,
            &recovery_nonce,
        ],
    )?;
    let exact_opening_digest = hash(ctx, jobs, exact_opening)?;
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

    let release_id = digest_limbs_assigned(ctx, &release);
    let public_prefix = [
        vec![version.value],
        digest_limbs_assigned(ctx, &semantic).to_vec(),
        digest_limbs_assigned(ctx, &operation).to_vec(),
        release_id.to_vec(),
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
        release_id,
    })
}

#[allow(clippy::too_many_arguments)]
fn bind_platform_context<F: KagemushaPoseidonFieldV1>(
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

struct AssignedHardwareProfileV1<F: KagemushaPoseidonFieldV1> {
    firmware_policy_digest: [PastaSha256ByteV1<F>; 32],
    allowed_suite_commitment: [PastaSha256ByteV1<F>; 32],
    policy_epoch: AssignedUint<F>,
    valid_from_ms: AssignedUint<F>,
    expires_at_ms: AssignedUint<F>,
}

fn bind_hardware_profile<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    profile: &KagemushaHardwareProfileV1,
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
            iroha_data_model::kagemusha::KagemushaHardwarePlatformClassV1::AndroidOemService => 0_u8,
            iroha_data_model::kagemusha::KagemushaHardwarePlatformClassV1::AppleOemService => 1,
            iroha_data_model::kagemusha::KagemushaHardwarePlatformClassV1::DedicatedSecureElement => 2,
            iroha_data_model::kagemusha::KagemushaHardwarePlatformClassV1::OtherQualified => 3,
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
        &F::from(u64::from(KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1)),
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
    let preimage = assemble_canonical_preimage_v1(
        ctx,
        range,
        &kagemusha_hardware_profile_id_preimage_layout_v1().map_err(|error| error.to_string())?,
        &KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_FIELD_RANGES_V1,
        &[
            &version.bytes,
            &protocol_version.bytes,
            &provider,
            &platform_class.bytes,
            &product,
            &firmware,
            &enrollment,
            &trust_roots,
            &suite_commitment,
            &policy_epoch.bytes,
            &governance_key,
            &capabilities.bytes,
            &qualification,
            &valid_from.bytes,
            &expires.bytes,
        ],
    )?;
    let expected_profile = hash_framed(
        ctx,
        jobs,
        HARDWARE_PROFILE_ID_DOMAIN_V1,
        KAGEMUSHA_HARDWARE_PROFILE_ID_PREIMAGE_BYTES_V1,
        preimage,
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

fn bind_compact_credential<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    witness: &KagemushaMintAuthorizationRelationWitnessV1,
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
    let preimage = assemble_canonical_preimage_v1(
        ctx,
        range,
        &kagemusha_hardware_credential_id_preimage_layout_v1()
            .map_err(|error| error.to_string())?,
        &KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_FIELD_RANGES_V1,
        &[
            &version.bytes,
            &network,
            &credential_profile,
            &suite,
            &firmware,
            &policy_epoch.bytes,
            &lane,
            &epoch,
            &generation.bytes,
            &device_key,
            &key_reference,
            &issued.bytes,
            &expires.bytes,
        ],
    )?;
    let expected_id = hash_framed(
        ctx,
        jobs,
        HARDWARE_CREDENTIAL_ID_DOMAIN_V1,
        KAGEMUSHA_HARDWARE_CREDENTIAL_ID_PREIMAGE_BYTES_V1,
        preimage,
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
        || crate::zk::kagemusha_v1_poseidon::decode::<Fp>(eq).is_none()
        || crate::zk::kagemusha_v1_poseidon::decode::<Fq>(ep).is_none()
    {
        return Err("mint-authorization deferred audits are noncanonical".to_owned());
    }
    Ok(())
}

fn validate_enabled_profiles(
    profiles: &[DigestV1; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
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

fn audit_cells<F: KagemushaPoseidonFieldV1>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
) -> Result<[AssignedValue<F>; 2], String> {
    if builder.assigned_instances.first().is_none_or(|column| {
        column.len() != MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1
            && column.len() != MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1
    }) {
        return Err("mint-authorization public instance has wrong shape".to_owned());
    }
    builder.assigned_instances[0][offset..offset + 2]
        .try_into()
        .map_err(|_| "mint-authorization audit instance has wrong shape".to_owned())
}

#[derive(Clone)]
struct AssignedUint<F: KagemushaPoseidonFieldV1> {
    value: AssignedValue<F>,
    bytes: Vec<PastaSha256ByteV1<F>>,
}

fn assign_uint_le<F: KagemushaPoseidonFieldV1>(
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

fn assign_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: DigestV1,
) -> [PastaSha256ByteV1<F>; 32] {
    assign_bytes(ctx, range, &digest)
        .try_into()
        .expect("digest width")
}

fn constant_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    digest: DigestV1,
) -> [AssignedValue<F>; 2] {
    digest_limbs::<F>(digest).map(|limb| ctx.load_constant(limb))
}

fn assert_nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
) {
    let zero = range.gate().is_zero(ctx, value);
    range.gate().assert_is_const(ctx, &zero, &F::ZERO);
}

fn hash_framed<F: KagemushaPoseidonFieldV1>(
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

fn canonical_hardware_credential_id_v1(
    credential: &KagemushaHardwareCredentialV1,
) -> Result<DigestV1, String> {
    let bytes = credential
        .canonical_id_preimage_bytes()
        .map_err(|error| error.to_string())?;
    Ok(digest_framed_native_v1(
        HARDWARE_CREDENTIAL_ID_DOMAIN_V1,
        &bytes,
    ))
}

fn canonical_hardware_profile_id_v1(
    profile: &KagemushaHardwareProfileV1,
) -> Result<DigestV1, String> {
    let bytes = profile
        .canonical_id_preimage_bytes()
        .map_err(|error| error.to_string())?;
    Ok(digest_framed_native_v1(
        HARDWARE_PROFILE_ID_DOMAIN_V1,
        &bytes,
    ))
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
pub(crate) fn mint_authorization_public_instances_v1<F: KagemushaPoseidonFieldV1>(
    statement: &KagemushaMintAuthorizationStatementV1,
    hardware_authorization: DigestV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
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
        kagemusha_asset_identity_digest_v1(&context.asset).map_err(|error| error.to_string())?,
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
#[path = "mint_authorization_canonical_tests.rs"]
mod canonical_tests;

#[cfg(test)]
mod tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        kagemusha::{
            KAGEMUSHA_WIRE_VERSION_V1, KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1,
            KagemushaHardwareCredentialV1, KagemushaHardwarePlatformClassV1,
            KagemushaHardwareProfileV1, kagemusha_device_key_reference_v1,
        },
    };
    use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};

    use super::*;

    fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_bytes((&[seed; 32]).into()).expect("P-256 signing key")
    }

    fn public_key(key: &SigningKey) -> KagemushaDevicePublicKeyV1 {
        KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .expect("device public key")
    }

    fn signature(key: &SigningKey) -> KagemushaDeviceSignatureV1 {
        let signature: Signature = key.sign(b"mint-authorization-layout-test");
        let signature = signature.normalize_s().unwrap_or(signature);
        KagemushaDeviceSignatureV1::from_raw_bytes(signature.to_bytes().as_ref())
            .expect("canonical signature")
    }

    #[test]
    fn fixed_profile_and_credential_projections_match_norito_identities() {
        let issuer = signing_key(0x31);
        let profile = KagemushaHardwareProfileV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
            hardware_profile_id: [0; 32],
            provider_id: [1; 32],
            platform_class: KagemushaHardwarePlatformClassV1::DedicatedSecureElement,
            product_class_digest: [2; 32],
            firmware_policy_digest: [3; 32],
            enrollment_attestation_verifier_digest: [4; 32],
            attestation_trust_roots_digest: [5; 32],
            allowed_suite_commitment: [6; 32],
            policy_epoch: 7,
            governance_credential_public_key: public_key(&issuer),
            capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
            qualification_report_digest: [8; 32],
            valid_from_ms: 9,
            expires_at_ms: 10_000,
        }
        .seal_hardware_profile_id()
        .expect("profile identity");
        assert_eq!(
            canonical_hardware_profile_id_v1(&profile).expect("profile preimage"),
            profile
                .expected_hardware_profile_id()
                .expect("canonical profile identity")
        );

        let device = signing_key(0x41);
        let device_public_key = public_key(&device);
        let network_id =
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"kagemusha-v1-mint-authorization-layout",
            )));
        let credential = KagemushaHardwareCredentialV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
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
            device_key_reference: kagemusha_device_key_reference_v1(&device_public_key),
            issued_at_ms: 15,
            expires_at_ms: 9_000,
            governance_signature: signature(&issuer),
        }
        .seal_credential_id()
        .expect("credential identity");
        assert_eq!(
            canonical_hardware_credential_id_v1(&credential).expect("credential preimage"),
            credential
                .expected_credential_id()
                .expect("canonical credential identity")
        );
    }
}
