//! Provider-issued hardware credentials and normalized GuardBundle relations.
//!
//! A static platform credential proves, once per hardware epoch, that an approved provider
//! authorized a device-owned proof key, the transport P-256 key, the exact lane, and all sixteen
//! non-forking lifecycle capabilities.  The provider secret never reaches the device: the device
//! stores the resulting paired credential proof and later proves knowledge of its own proof key
//! in each GuardBundle. Consequently neither a host-side platform signature nor knowledge of a
//! provider secret by wallet software can authorize money.

#[cfg(feature = "zk-halo2-ipa")]
use ff::PrimeField as _;
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
};
#[cfg(feature = "zk-halo2-ipa")]
use halo2_proofs::halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq};
use halo2_proofs::{
    circuit::{Layouter, V1},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ASSET_SCALE_MAX_V1, KAGEMUSHA_HALO2_K_V1,
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KAGEMUSHA_WIRE_VERSION_V1,
    KagemushaDevicePublicKeyV1, kagemusha_device_key_reference_v1,
};
use iroha_data_model::nexus::AxtAssetIncarnationV1;
use sha2::{Digest as _, Sha256};

use super::{DigestV1, KagemushaNormalizedGuardStatementV1, KagemushaOperationV1};
#[cfg(feature = "zk-halo2-ipa")]
use super::{
    KagemushaEpAccumulatorV1, KagemushaEpFoldProofV1, KagemushaEqAccumulatorV1,
    KagemushaEqFoldProofV1, KagemushaPastaParityV1,
};
#[cfg(feature = "zk-halo2-ipa")]
use crate::zk::pasta_dense_msm::{PastaDenseMsmConfigV1, PastaDenseMsmJobsV1};
use crate::zk::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, from_u128},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256ConfigV1, PastaSha256JobsV1},
};
#[cfg(feature = "zk-halo2-ipa")]
use halo2_base::utils::{BigPrimeField, CurveAffineExt};
#[cfg(feature = "zk-halo2-ipa")]
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};

#[cfg(feature = "zk-halo2-ipa")]
use super::deferred_parent::{
    DeferredAccumulator, KagemushaDeferredParentOutputV1, accumulator_limb_count,
    bind_accumulator_limbs, constrain_reciprocal_output_with_u128_binding_v1,
    constrain_reciprocal_tagged_audit_v1, deferred_field_chips_v1, deferred_loader_v1,
    finalize_tagged_deferred_audit_v1, finalize_tagged_deferred_audit_with_u128_binding_v1,
    kagemusha_protocol_structure_digest_v1, load_and_constrain_parent_protocol_v1,
    load_native_accumulator, native_parent_protocol_digest_v1, ordinary_ipa_proof_profile_v1,
    verify_fold, verify_ordinary_proof_v1, verify_two_carrier_hybrid_ordinary_proof_and_stream_v1,
};

/// Fixed provider-profile registry depth.
///
/// This bounds a release to 65,536 qualified hardware profiles, not payments, devices, credits,
/// hops, or proof history.  A profile may issue any number of device credentials.
pub const KAGEMUSHA_HARDWARE_POLICY_TREE_DEPTH_V1: usize = 16;

/// Fixed release-manifest table width used by circuits that authenticate a qualified profile.
///
/// This bounds the number of hardware profiles in one release manifest. It does not bound
/// devices, payments, received credits, balance history, proof depth, or the ability to spend a
/// valid credit.
pub(crate) const KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1: usize = 64;

/// Constrain a hidden hardware-profile identifier to an enabled release-manifest entry.
///
/// Every slot, including canonical zero padding, is loaded as a circuit constant so setup and
/// proving retain identical topology. `selector` controls whether membership is required.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn constrain_enabled_hardware_profile_membership_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    hidden_profile: [AssignedValue<F>; 2],
    enabled_profiles: &[DigestV1; KAGEMUSHA_ENABLED_HARDWARE_PROFILE_SLOTS_V1],
) {
    let gate = range.gate();
    let mut matched = ctx.load_constant(F::ZERO);
    for profile in enabled_profiles {
        let limbs = crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(*profile);
        let low = ctx.load_constant(limbs[0]);
        let high = ctx.load_constant(limbs[1]);
        let low_equal = gate.is_equal(ctx, hidden_profile[0], low);
        let high_equal = gate.is_equal(ctx, hidden_profile[1], high);
        let slot_match = gate.and(ctx, low_equal, high_equal);
        let product = gate.mul(ctx, matched, slot_match);
        let sum = gate.add(ctx, matched, slot_match);
        matched = gate.sub(ctx, sum, product);
    }
    let missing = gate.not(ctx, matched);
    let selected = gate.mul(ctx, selector, missing);
    gate.assert_is_const(ctx, &selected, &F::ZERO);
}

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const DEVICE_KEY_REFERENCE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:device-key-reference";
const DEVICE_AUTHORITY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:device-proof-authority";
const PROVIDER_AUTHORITY_DOMAIN: &[u8] = b"iroha:kagemusha:v1:provider-proof-authority";
const POLICY_LEAF_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-policy-leaf";
const POLICY_NODE_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-policy-node";
const CREDENTIAL_STATEMENT_DOMAIN: &[u8] = b"iroha:kagemusha:v1:platform-credential-statement";

pub(super) fn normalized_guard_statement_digest_v1(
    statement: &super::KagemushaNormalizedGuardStatementV1,
) -> DigestV1 {
    let payload = normalized_guard_statement_payload_v1(statement);
    let mut hasher = Sha256::new();
    hasher.update(
        u64::try_from(super::GUARD_STATEMENT_DIGEST_DOMAIN.len())
            .expect("fixed GuardBundle digest domain fits u64")
            .to_be_bytes(),
    );
    hasher.update(super::GUARD_STATEMENT_DIGEST_DOMAIN);
    hasher.update(
        u64::try_from(payload.len())
            .expect("fixed GuardBundle statement fits u64")
            .to_be_bytes(),
    );
    hasher.update(payload);
    hasher.finalize().into()
}

fn normalized_guard_statement_payload_v1(
    statement: &super::KagemushaNormalizedGuardStatementV1,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(1_249);
    bytes.extend_from_slice(&statement.version.to_le_bytes());
    bytes.extend_from_slice(&statement.protocol_version.to_le_bytes());
    bytes.extend_from_slice(&statement.predecessor_suite_id);
    bytes.extend_from_slice(&statement.predecessor_vk_digest);
    bytes.extend_from_slice(&statement.successor_suite_id);
    bytes.extend_from_slice(&statement.successor_vk_digest);
    bytes.push(operation_tag(statement.operation));
    bytes.extend_from_slice(&statement.amount.to_le_bytes());
    bytes.extend_from_slice(&statement.peer_credit_id);
    bytes.extend_from_slice(&statement.recipient_encryption_key_binding);
    bytes.extend_from_slice(&statement.mint_finality_proof_binding_digest);
    bytes.extend_from_slice(&statement.predecessor_release_id);
    bytes.extend_from_slice(&statement.release_id);
    bytes.extend_from_slice(&statement.network_id);
    bytes.extend_from_slice(&statement.asset_id);
    bytes.extend_from_slice(statement.asset_incarnation.as_bytes());
    bytes.extend_from_slice(&statement.asset_scale.to_le_bytes());
    bytes.extend_from_slice(&statement.liability_pool_id);
    bytes.extend_from_slice(&statement.hardware_profile_id);
    bytes.extend_from_slice(&statement.policy_epoch.to_le_bytes());
    bytes.extend_from_slice(&statement.lane_id);
    bytes.extend_from_slice(&statement.predecessor_state_commitment);
    bytes.extend_from_slice(&statement.successor_state_commitment);
    bytes.extend_from_slice(&statement.predecessor_state_nonce_commitment);
    bytes.extend_from_slice(&statement.successor_state_nonce_commitment);
    bytes.extend_from_slice(&statement.predecessor_logical_sequence.to_le_bytes());
    bytes.extend_from_slice(&statement.successor_logical_sequence.to_le_bytes());
    bytes.extend_from_slice(
        &statement
            .predecessor_hardware_epoch_generation
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&statement.successor_hardware_epoch_generation.to_le_bytes());
    bytes.extend_from_slice(&statement.predecessor_hardware_epoch_id);
    bytes.extend_from_slice(&statement.successor_hardware_epoch_id);
    bytes.extend_from_slice(&statement.predecessor_key_reference);
    bytes.extend_from_slice(&statement.successor_key_reference);
    bytes.extend_from_slice(&statement.predecessor_hardware_policy_id);
    bytes.extend_from_slice(&statement.successor_hardware_policy_id);
    bytes.extend_from_slice(&statement.journal_revision_before.to_le_bytes());
    bytes.extend_from_slice(&statement.journal_revision_after.to_le_bytes());
    bytes.extend_from_slice(&statement.lifecycle_binding_digest);
    bytes.extend_from_slice(&statement.prepared_transition_binding_digest);
    bytes.extend_from_slice(&statement.terminal_commit_binding_digest);
    bytes.extend_from_slice(&statement.sender_one_time_authorization_digest);
    bytes.extend_from_slice(&statement.receive_credit_binding_digest);
    bytes.extend_from_slice(&statement.transition_intent_digest);
    bytes.extend_from_slice(&statement.transition_effect_digest);
    bytes.extend_from_slice(&statement.recovery_record_digest);
    bytes.extend_from_slice(&statement.durable_inbox_effect_digest);
    bytes.extend_from_slice(&statement.durable_outbox_effect_digest);
    bytes
}

const fn operation_tag(operation: super::KagemushaOperationV1) -> u8 {
    match operation {
        super::KagemushaOperationV1::Bootstrap => 0,
        super::KagemushaOperationV1::MintFold => 1,
        super::KagemushaOperationV1::SendSplit => 2,
        super::KagemushaOperationV1::ReceiveFold => 3,
        super::KagemushaOperationV1::RedeemSplit => 4,
        super::KagemushaOperationV1::Rotate => 5,
    }
}

/// Provider-authorized fixed statement for one hardware epoch.
///
/// The provider creates the paired proof for this statement during qualification/provisioning.
/// The per-device proof-authority secret is not present here and remains sealed in hardware.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KagemushaPlatformCredentialStatementV1 {
    /// Credential relation version.
    pub version: u16,
    /// Exact Kagemusha protocol version authorized by the profile.
    pub protocol_version: u16,
    /// Governed proof suite authorized for this credential.
    pub suite_id: DigestV1,
    /// Authenticated recursive-proof release.
    pub release_id: DigestV1,
    /// Exact network identity.
    pub network_id: DigestV1,
    /// Canonical typed asset identity digest.
    pub asset_id: DigestV1,
    /// Exact asset incarnation authorized for this credential.
    pub asset_incarnation: AxtAssetIncarnationV1,
    /// Authoritative asset scale.
    pub asset_scale: u32,
    /// Network-and-asset pooled reserve identity.
    pub liability_pool_id: DigestV1,
    /// Stable hardware-controlled lane.
    pub lane_id: DigestV1,
    /// Hardware epoch generation.
    pub hardware_epoch_generation: u128,
    /// Hardware epoch identity.
    pub hardware_epoch_id: DigestV1,
    /// Reference to the transport P-256 device key.
    pub key_reference: DigestV1,
    /// Exact canonical P-256 device key bound to this credential.
    pub device_public_key: KagemushaDevicePublicKeyV1,
    /// Root of the release-approved provider-profile registry.
    pub hardware_policy_id: DigestV1,
    /// Commitment to the device-only proof-authority secret.
    pub device_authority_commitment: DigestV1,
    /// Qualified hardware-profile identity.
    pub hardware_profile_id: DigestV1,
    /// Governed hardware-policy epoch.
    pub policy_epoch: u64,
    /// Provider-neutral platform-class tag.
    pub platform_class: u8,
    /// Exact sixteen secure-device capability bits.
    pub capability_mask: u16,
    /// Commitment to the provider proof-authority secret registered by policy.
    pub provider_authority_commitment: DigestV1,
    /// Digest of the complete platform attestation evidence.
    pub platform_attestation_digest: DigestV1,
    /// Digest of the provider's credential-issuance record.
    pub credential_issuance_digest: DigestV1,
    /// Release-pinned canonical empty inbox/outbox effect.
    pub canonical_empty_effect_digest: DigestV1,
    /// Provider-profile leaf position in the fixed policy tree.
    pub provider_profile_index: u16,
}

impl KagemushaPlatformCredentialStatementV1 {
    /// Return the exact field-neutral credential digest exposed by both Pasta parities.
    #[must_use]
    pub fn canonical_digest(&self) -> DigestV1 {
        Sha256::digest(credential_statement_preimage(self)).into()
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        self.device_public_key
            .validate()
            .map_err(|error| format!("invalid Kagemusha credential device key: {error}"))?;
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self.protocol_version != KAGEMUSHA_WIRE_VERSION_V1
            || self.asset_incarnation.validate().is_err()
            || self.policy_epoch == 0
            || self.asset_scale > KAGEMUSHA_ASSET_SCALE_MAX_V1
            || self.hardware_epoch_generation == 0
            || self.platform_class == 0
            || self.capability_mask != KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1
            || [
                self.release_id,
                self.suite_id,
                self.network_id,
                self.asset_id,
                self.liability_pool_id,
                self.lane_id,
                self.hardware_epoch_id,
                self.key_reference,
                self.hardware_policy_id,
                self.device_authority_commitment,
                self.hardware_profile_id,
                self.provider_authority_commitment,
                self.platform_attestation_digest,
                self.credential_issuance_digest,
                self.canonical_empty_effect_digest,
            ]
            .contains(&[0; 32])
            || self.key_reference != kagemusha_device_key_reference_v1(&self.device_public_key)
        {
            return Err("invalid Kagemusha platform credential statement".to_owned());
        }
        Ok(())
    }
}

/// Complete private provider witness for one static platform credential proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaPlatformCredentialRelationWitnessV1 {
    /// Public semantic credential statement.
    pub statement: KagemushaPlatformCredentialStatementV1,
    /// Provider-only proof-authority secret.
    pub provider_authority_secret: DigestV1,
    /// Bottom-up provider-profile policy path.
    pub policy_siblings: [DigestV1; KAGEMUSHA_HARDWARE_POLICY_TREE_DEPTH_V1],
}

impl KagemushaPlatformCredentialRelationWitnessV1 {
    /// Validate the complete provider secret and policy path before proving.
    pub fn validate(&self) -> Result<(), String> {
        self.statement.validate()?;
        if self.provider_authority_secret == [0; 32]
            || authority_commitment(PROVIDER_AUTHORITY_DOMAIN, self.provider_authority_secret)
                != self.statement.provider_authority_commitment
            || self.policy_siblings.contains(&[0; 32])
            || self.policy_root() != self.statement.hardware_policy_id
        {
            return Err("invalid Kagemusha provider credential authority".to_owned());
        }
        Ok(())
    }

    fn policy_root(&self) -> DigestV1 {
        let mut current = policy_leaf(&self.statement);
        for (depth, sibling) in self.policy_siblings.iter().copied().enumerate() {
            let direction = (self.statement.provider_profile_index >> depth) & 1;
            current = if direction == 0 {
                policy_node(current, sibling)
            } else {
                policy_node(sibling, current)
            };
        }
        current
    }
}

/// Complete semantic witness for one hardware-authorized aggregate transition.
///
/// The two credential proofs are supplied separately to the aggregate recursion circuit. This witness
/// contains only their exact statements and the device-owned secrets that each credential binds.
/// A rotation proves possession of both the consumed and replacement device authorities. A suite
/// upgrade keeps the hardware authority fixed while bridging to the successor suite credential;
/// every other operation uses one byte-identical credential in both fixed slots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaGuardBundleRelationWitnessV1 {
    /// Canonical fixed-layout transition statement.
    pub statement: KagemushaNormalizedGuardStatementV1,
    /// Release-pinned canonical empty inbox/outbox effect.
    pub canonical_empty_effect_digest: DigestV1,
    /// Credential authorizing consumption of the predecessor hardware epoch.
    pub predecessor_credential: KagemushaPlatformCredentialStatementV1,
    /// Credential binding the successor hardware epoch.
    pub successor_credential: KagemushaPlatformCredentialStatementV1,
    /// Device-only authority secret for the predecessor credential.
    pub predecessor_device_authority_secret: DigestV1,
    /// Device-only authority secret for the successor credential.
    pub successor_device_authority_secret: DigestV1,
}

impl KagemushaGuardBundleRelationWitnessV1 {
    /// Validate every fixed semantic binding before recursive proving.
    pub fn validate(&self) -> Result<(), String> {
        self.statement
            .validate_shape()
            .map_err(|error| error.to_string())?;
        self.statement
            .validate_release_effects(self.canonical_empty_effect_digest)
            .map_err(|error| error.to_string())?;
        self.predecessor_credential.validate()?;
        self.successor_credential.validate()?;
        if self.canonical_empty_effect_digest == [0; 32]
            || self.predecessor_device_authority_secret == [0; 32]
            || self.successor_device_authority_secret == [0; 32]
            || device_authority_commitment_v1(self.predecessor_device_authority_secret)
                != self.predecessor_credential.device_authority_commitment
            || device_authority_commitment_v1(self.successor_device_authority_secret)
                != self.successor_credential.device_authority_commitment
        {
            return Err("invalid Kagemusha GuardBundle device authority".to_owned());
        }
        validate_common_credential_binding(
            &self.statement,
            self.canonical_empty_effect_digest,
            &self.predecessor_credential,
        )?;
        validate_common_credential_binding(
            &self.statement,
            self.canonical_empty_effect_digest,
            &self.successor_credential,
        )?;
        validate_successor_credential_binding(&self.statement, &self.successor_credential)?;

        match self.statement.operation {
            KagemushaOperationV1::Bootstrap => {
                if self.predecessor_credential != self.successor_credential
                    || self.predecessor_device_authority_secret
                        != self.successor_device_authority_secret
                {
                    return Err("Kagemusha bootstrap credential slots must be identical".to_owned());
                }
            }
            KagemushaOperationV1::Rotate => {
                validate_predecessor_credential_binding(
                    &self.statement,
                    &self.predecessor_credential,
                )?;
                if self.statement.successor_hardware_epoch_generation
                    != self
                        .statement
                        .predecessor_hardware_epoch_generation
                        .checked_add(1)
                        .ok_or_else(|| "Kagemusha hardware epoch overflow".to_owned())?
                    || self.statement.successor_hardware_epoch_id
                        == self.statement.predecessor_hardware_epoch_id
                    || self.statement.successor_key_reference
                        == self.statement.predecessor_key_reference
                    || self.predecessor_credential == self.successor_credential
                {
                    return Err("invalid Kagemusha hardware rotation".to_owned());
                }
            }
            _ => {
                validate_predecessor_credential_binding(
                    &self.statement,
                    &self.predecessor_credential,
                )?;
                if self.predecessor_credential != self.successor_credential
                    || self.predecessor_device_authority_secret
                        != self.successor_device_authority_secret
                    || self.statement.predecessor_hardware_epoch_generation
                        != self.statement.successor_hardware_epoch_generation
                    || self.statement.predecessor_hardware_epoch_id
                        != self.statement.successor_hardware_epoch_id
                    || self.statement.predecessor_key_reference
                        != self.statement.successor_key_reference
                    || self.statement.predecessor_hardware_policy_id
                        != self.statement.successor_hardware_policy_id
                {
                    return Err("non-rotation changed the Kagemusha credential".to_owned());
                }
            }
        }
        Ok(())
    }

    /// Return the field-neutral public GuardBundle statement digest.
    #[must_use]
    pub fn statement_digest(&self) -> DigestV1 {
        normalized_guard_statement_digest_v1(&self.statement)
    }

    /// Return the two credential statement digests consumed by the fixed recursive slots.
    #[must_use]
    pub fn credential_digests(&self) -> [DigestV1; 2] {
        [
            self.predecessor_credential.canonical_digest(),
            self.successor_credential.canonical_digest(),
        ]
    }
}

fn validate_common_credential_binding(
    guard: &KagemushaNormalizedGuardStatementV1,
    canonical_empty_effect_digest: DigestV1,
    credential: &KagemushaPlatformCredentialStatementV1,
) -> Result<(), String> {
    if credential.version != guard.version
        || credential.protocol_version != guard.protocol_version
        || credential.network_id != guard.network_id
        || credential.asset_id != guard.asset_id
        || credential.asset_incarnation != guard.asset_incarnation
        || credential.asset_scale != guard.asset_scale
        || credential.liability_pool_id != guard.liability_pool_id
        || credential.hardware_profile_id != guard.hardware_profile_id
        || credential.policy_epoch != guard.policy_epoch
        || credential.lane_id != guard.lane_id
        || credential.canonical_empty_effect_digest != canonical_empty_effect_digest
    {
        return Err("Kagemusha credential/GuardBundle lane binding mismatch".to_owned());
    }
    Ok(())
}

fn validate_predecessor_credential_binding(
    guard: &KagemushaNormalizedGuardStatementV1,
    credential: &KagemushaPlatformCredentialStatementV1,
) -> Result<(), String> {
    if credential.release_id != guard.predecessor_release_id
        || credential.suite_id != guard.predecessor_suite_id
        || credential.hardware_epoch_generation != guard.predecessor_hardware_epoch_generation
        || credential.hardware_epoch_id != guard.predecessor_hardware_epoch_id
        || credential.key_reference != guard.predecessor_key_reference
        || credential.hardware_policy_id != guard.predecessor_hardware_policy_id
    {
        return Err("Kagemusha predecessor credential binding mismatch".to_owned());
    }
    Ok(())
}

fn validate_successor_credential_binding(
    guard: &KagemushaNormalizedGuardStatementV1,
    credential: &KagemushaPlatformCredentialStatementV1,
) -> Result<(), String> {
    if credential.release_id != guard.release_id
        || credential.suite_id != guard.successor_suite_id
        || credential.hardware_epoch_generation != guard.successor_hardware_epoch_generation
        || credential.hardware_epoch_id != guard.successor_hardware_epoch_id
        || credential.key_reference != guard.successor_key_reference
        || credential.hardware_policy_id != guard.successor_hardware_policy_id
    {
        return Err("Kagemusha successor credential binding mismatch".to_owned());
    }
    Ok(())
}

/// Public PlatformCredential layout: statement, mutually authenticated audits, then the complete
/// claim history which a monetary consumer must carry and ultimately decide.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) mod platform_credential_public_instance {
    pub(crate) const CREDENTIAL_LO: usize = 0;
    pub(crate) const EQ_AUDIT_LO: usize = 2;
    pub(crate) const EP_AUDIT_LO: usize = 4;
    pub(crate) const HISTORY_START: usize = 6;
}

/// Exact PlatformCredential public width for one parity.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) const KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1: usize =
    platform_credential_public_instance::HISTORY_START + accumulator_limb_count();

#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
enum KagemushaPlatformCredentialDenseJobsV1 {
    Eq(PastaDenseMsmJobsV1<EpAffine>),
    Ep(PastaDenseMsmJobsV1<EqAffine>),
}

/// Fixed paired-Pasta provider credential relation.
///
/// Construction is intentionally restricted to the paired hash-claim builders below. A semantic
/// credential witness alone no longer creates a circuit, because that would silently reintroduce
/// either the infeasible k=16 SHA machine or an unauthenticated host digest.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub struct KagemushaPlatformCredentialRelationCircuitV1<F: halo2_base::utils::ScalarField> {
    builder: BaseCircuitBuilder<F>,
    dense_jobs: KagemushaPlatformCredentialDenseJobsV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl<F> KagemushaPlatformCredentialRelationCircuitV1<F>
where
    F: KagemushaPoseidonFieldV1,
{
    /// Reject the obsolete relation-only constructor. A completed, release-bound hash claim is
    /// mandatory and is accepted only by [`build_kagemusha_platform_credential_eq_v1`] and
    /// [`build_kagemusha_platform_credential_ep_v1`].
    pub fn new(witness: KagemushaPlatformCredentialRelationWitnessV1) -> Result<Self, String> {
        witness.validate()?;
        Err(
            "Kagemusha PlatformCredential requires a completed paired SHA claim and carried history"
                .to_owned(),
        )
    }

    /// Return the exact public column built by the authenticated paired constructor.
    pub fn public_instances(&self) -> Result<Vec<F>, String> {
        let column = self
            .builder
            .assigned_instances
            .first()
            .ok_or_else(|| "Kagemusha PlatformCredential public column is absent".to_owned())?;
        if column.len() != KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1 {
            return Err("Kagemusha PlatformCredential public column has wrong shape".to_owned());
        }
        Ok(column.iter().map(|value| *value.value()).collect())
    }
}

/// Base and reciprocal dense-MSM configuration. SHA compression lives only in k=12 shard proofs.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub struct KagemushaPlatformCredentialCircuitConfigV1<F: KagemushaPoseidonFieldV1> {
    base: BaseConfig<F>,
    dense: PastaDenseMsmConfigV1,
}

#[cfg(feature = "zk-halo2-ipa")]
macro_rules! impl_platform_credential_circuit {
    ($field:ty, $opposite:ty, $variant:ident, $label:literal) => {
        impl Circuit<$field> for KagemushaPlatformCredentialRelationCircuitV1<$field> {
            type Config = KagemushaPlatformCredentialCircuitConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn without_witnesses(&self) -> Self {
                let dense_jobs = match &self.dense_jobs {
                    KagemushaPlatformCredentialDenseJobsV1::$variant(jobs) => {
                        KagemushaPlatformCredentialDenseJobsV1::$variant(jobs.unknown())
                    }
                    _ => unreachable!(concat!($label, " has the wrong reciprocal job parity")),
                };
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                    dense_jobs,
                }
            }

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable_rows);
                KagemushaPlatformCredentialCircuitConfigV1 {
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
                    layouter.namespace(|| concat!($label, " Base relation")),
                )?;
                let jobs = match &self.dense_jobs {
                    KagemushaPlatformCredentialDenseJobsV1::$variant(jobs) => jobs,
                    _ => unreachable!(concat!($label, " has the wrong reciprocal job parity")),
                };
                jobs.synthesize(
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

#[cfg(feature = "zk-halo2-ipa")]
impl_platform_credential_circuit!(Fp, EpAffine, Eq, "Kagemusha Eq PlatformCredential");
#[cfg(feature = "zk-halo2-ipa")]
impl_platform_credential_circuit!(Fq, EqAffine, Ep, "Kagemusha Ep PlatformCredential");

struct KagemushaAssignedPlatformCredentialV1<F: KagemushaPoseidonFieldV1> {
    credential_digest: [PastaSha256ByteV1<F>; 32],
    release_id: [AssignedValue<F>; 2],
}

fn credential_builder<F>(
    witness: Option<&KagemushaPlatformCredentialRelationWitnessV1>,
) -> Result<
    (
        BaseCircuitBuilder<F>,
        PastaSha256JobsV1<F>,
        KagemushaAssignedPlatformCredentialV1<F>,
    ),
    String,
>
where
    F: KagemushaPoseidonFieldV1,
{
    if let Some(witness) = witness {
        witness.validate()?;
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(usize::try_from(KAGEMUSHA_HALO2_K_V1).expect("k fits usize"))
        .use_lookup_bits(usize::try_from(KAGEMUSHA_HALO2_K_V1 - 1).expect("lookup bits fit usize"))
        .use_instance_columns(1);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let mut jobs = PastaSha256JobsV1::default();
    let statement = witness.map_or_else(blank_credential_statement, |value| value.statement);

    let version = assign_uint_le(ctx, &range, u128::from(statement.version), 16);
    gate.assert_is_const(ctx, &version.value, &F::ONE);
    let protocol_version = assign_uint_le(ctx, &range, u128::from(statement.protocol_version), 16);
    gate.assert_is_const(ctx, &protocol_version.value, &F::ONE);
    let suite_id = assign_digest(ctx, &range, statement.suite_id);
    let release = assign_digest(ctx, &range, statement.release_id);
    let network = assign_digest(ctx, &range, statement.network_id);
    let asset = assign_digest(ctx, &range, statement.asset_id);
    let asset_incarnation = assign_digest(ctx, &range, *statement.asset_incarnation.as_bytes());
    let scale = assign_uint_le(ctx, &range, u128::from(statement.asset_scale), 32);
    let scale_ok = range.is_less_than_safe(
        ctx,
        scale.value,
        u64::from(KAGEMUSHA_ASSET_SCALE_MAX_V1) + 1,
    );
    gate.assert_is_const(ctx, &scale_ok, &F::ONE);
    let pool = assign_digest(ctx, &range, statement.liability_pool_id);
    let lane = assign_digest(ctx, &range, statement.lane_id);
    let epoch_generation = assign_uint_le(ctx, &range, statement.hardware_epoch_generation, 128);
    assert_nonzero(ctx, &range, epoch_generation.value);
    let epoch_id = assign_digest(ctx, &range, statement.hardware_epoch_id);
    let key_reference = assign_digest(ctx, &range, statement.key_reference);
    let device_public_key = assign_bytes(ctx, &range, statement.device_public_key.as_sec1_bytes());
    gate.assert_is_const(
        ctx,
        &device_public_key[0]
            .assigned()
            .expect("device key byte is assigned"),
        &F::from(4),
    );
    let policy_id = assign_digest(ctx, &range, statement.hardware_policy_id);
    let device_authority = assign_digest(ctx, &range, statement.device_authority_commitment);
    let profile = assign_digest(ctx, &range, statement.hardware_profile_id);
    let policy_epoch = assign_uint_le(ctx, &range, u128::from(statement.policy_epoch), 64);
    let platform_class = assign_uint_le(ctx, &range, u128::from(statement.platform_class), 8);
    assert_nonzero(ctx, &range, platform_class.value);
    let capabilities = assign_uint_le(ctx, &range, u128::from(statement.capability_mask), 16);
    gate.assert_is_const(
        ctx,
        &capabilities.value,
        &F::from(u64::from(KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1)),
    );
    let provider_authority = assign_digest(ctx, &range, statement.provider_authority_commitment);
    let attestation = assign_digest(ctx, &range, statement.platform_attestation_digest);
    let issuance = assign_digest(ctx, &range, statement.credential_issuance_digest);
    let empty_effect = assign_digest(ctx, &range, statement.canonical_empty_effect_digest);
    let profile_index = assign_uint_le(
        ctx,
        &range,
        u128::from(statement.provider_profile_index),
        16,
    );

    for digest in [
        &release,
        &suite_id,
        &network,
        &asset,
        &asset_incarnation,
        &pool,
        &lane,
        &epoch_id,
        &key_reference,
        &policy_id,
        &device_authority,
        &profile,
        &provider_authority,
        &attestation,
        &issuance,
        &empty_effect,
    ] {
        assert_digest_nonzero(ctx, &range, digest);
    }
    assert_nonzero(ctx, &range, policy_epoch.value);
    let incarnation_bits = gate.num_to_bits(
        ctx,
        asset_incarnation[31]
            .assigned()
            .expect("asset incarnation byte is assigned"),
        8,
    );
    gate.assert_is_const(ctx, &incarnation_bits[0], &F::ONE);

    let key_digest = hash(
        ctx,
        &mut jobs,
        [
            constant_bytes(DEVICE_KEY_REFERENCE_DOMAIN),
            vec![PastaSha256ByteV1::constant(0)],
            device_public_key.clone(),
        ]
        .concat(),
    )?;
    bind_equal_digest(ctx, &range, &key_digest, &key_reference);

    let provider_secret = assign_bytes(
        ctx,
        &range,
        &witness.map_or([0; 32], |value| value.provider_authority_secret),
    );
    assert_bytes_nonzero(ctx, &range, &provider_secret);
    let provider_digest = hash(
        ctx,
        &mut jobs,
        [
            constant_bytes(PROVIDER_AUTHORITY_DOMAIN),
            vec![PastaSha256ByteV1::constant(0)],
            provider_secret.clone(),
        ]
        .concat(),
    )?;
    bind_equal_digest(ctx, &range, &provider_digest, &provider_authority);

    let leaf = hash(
        ctx,
        &mut jobs,
        [
            constant_bytes(POLICY_LEAF_DOMAIN),
            vec![PastaSha256ByteV1::constant(0)],
            profile.to_vec(),
            platform_class.bytes.to_vec(),
            capabilities.bytes.to_vec(),
            provider_authority.to_vec(),
            empty_effect.to_vec(),
        ]
        .concat(),
    )?;
    let index_bits = gate.num_to_bits(ctx, profile_index.value, 16);
    let siblings = witness.map_or(
        [[0; 32]; KAGEMUSHA_HARDWARE_POLICY_TREE_DEPTH_V1],
        |value| value.policy_siblings,
    );
    let mut root = leaf;
    for (depth, sibling) in siblings.into_iter().enumerate() {
        let sibling = assign_digest(ctx, &range, sibling);
        assert_digest_nonzero(ctx, &range, &sibling);
        let direction = index_bits[depth];
        let left = select_digest(ctx, &range, direction, &sibling, &root);
        let right = select_digest(ctx, &range, direction, &root, &sibling);
        root = hash(
            ctx,
            &mut jobs,
            [
                constant_bytes(POLICY_NODE_DOMAIN),
                vec![PastaSha256ByteV1::constant(0)],
                left.to_vec(),
                right.to_vec(),
            ]
            .concat(),
        )?;
    }
    bind_equal_digest(ctx, &range, &root, &policy_id);

    let credential_digest = hash(
        ctx,
        &mut jobs,
        [
            constant_bytes(CREDENTIAL_STATEMENT_DOMAIN),
            vec![PastaSha256ByteV1::constant(0)],
            version.bytes.to_vec(),
            protocol_version.bytes.to_vec(),
            suite_id.to_vec(),
            release.to_vec(),
            network.to_vec(),
            asset.to_vec(),
            asset_incarnation.to_vec(),
            scale.bytes.to_vec(),
            pool.to_vec(),
            lane.to_vec(),
            epoch_generation.bytes.to_vec(),
            epoch_id.to_vec(),
            key_reference.to_vec(),
            device_public_key,
            policy_id.to_vec(),
            device_authority.to_vec(),
            profile.to_vec(),
            policy_epoch.bytes.to_vec(),
            platform_class.bytes.to_vec(),
            capabilities.bytes.to_vec(),
            provider_authority.to_vec(),
            attestation.to_vec(),
            issuance.to_vec(),
            empty_effect.to_vec(),
            profile_index.bytes.to_vec(),
        ]
        .concat(),
    )?;
    let release_id = digest_limbs_assigned(ctx, &release);
    Ok((
        builder,
        jobs,
        KagemushaAssignedPlatformCredentialV1 {
            credential_digest,
            release_id,
        },
    ))
}

/// Extract the exact typed SHA queue emitted by the production credential relation.
///
/// The helper deliberately invokes `credential_builder` instead of duplicating its host
/// encodings. The returned bytes are only a private shard/claim proof plan; the paired producer
/// below separately constrains the same assigned queue against the completed recursive claim.
#[cfg(feature = "zk-halo2-ipa")]
pub(super) fn platform_credential_sha_messages_v1<F>(
    witness: &KagemushaPlatformCredentialRelationWitnessV1,
) -> Result<Vec<Vec<u8>>, String>
where
    F: KagemushaPoseidonFieldV1,
{
    let (builder, jobs, assigned) = credential_builder::<F>(Some(witness))?;
    let messages = jobs.canonical_messages()?;
    drop(assigned);
    drop(jobs);
    drop(builder);
    Ok(messages)
}

/// One parity's terminal ordered-hash claim consumed by a PlatformCredential proof.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Copy)]
pub(crate) struct KagemushaPlatformCredentialHashClaimParityWitnessV1<'a, C>
where
    C: CurveAffineExt,
{
    pub(crate) claim_protocol: &'a PlonkProtocol<C>,
    pub(crate) claim_instances: &'a [Vec<C::ScalarExt>],
    pub(crate) claim_proof: &'a [u8],
    pub(crate) claim_history: &'a IpaAccumulator<C, NativeLoader>,
    pub(crate) claim_history_fold_proof: &'a [u8],
    pub(crate) successor_history: &'a [u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}

/// Complete paired claim material required to create one provider credential proof pair.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaPlatformCredentialHashClaimPairWitnessV1<'a> {
    pub(crate) relation: KagemushaPlatformCredentialRelationWitnessV1,
    pub(crate) eq_claim_protocol_digest: DigestV1,
    pub(crate) ep_claim_protocol_digest: DigestV1,
    pub(crate) eq_shard_protocol_digest: DigestV1,
    pub(crate) ep_shard_protocol_digest: DigestV1,
    pub(crate) eq: KagemushaPlatformCredentialHashClaimParityWitnessV1<'a, EqAffine>,
    pub(crate) ep: KagemushaPlatformCredentialHashClaimParityWitnessV1<'a, EpAffine>,
}

/// Detached reciprocal plans discovered without retaining either PlatformCredential Base graph.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaPlatformCredentialAuditDiscoveryV1 {
    eq_output: KagemushaDeferredParentOutputV1<EqAffine>,
    ep_output: KagemushaDeferredParentOutputV1<EpAffine>,
    eq_digest: DigestV1,
    ep_digest: DigestV1,
}

#[cfg(feature = "zk-halo2-ipa")]
impl KagemushaPlatformCredentialAuditDiscoveryV1 {
    #[must_use]
    pub(crate) const fn eq_digest(&self) -> DigestV1 {
        self.eq_digest
    }

    #[must_use]
    pub(crate) const fn ep_digest(&self) -> DigestV1 {
        self.ep_digest
    }
}

#[cfg(feature = "zk-halo2-ipa")]
const PLATFORM_CREDENTIAL_HASH_CLAIM_EQUATION_TAG_V1: u32 = 13;
#[cfg(feature = "zk-halo2-ipa")]
const PLATFORM_CREDENTIAL_BASE_BOUND_U128_COUNT_V1: usize =
    2 + 2 + 8 + 2 * accumulator_limb_count();
#[cfg(feature = "zk-halo2-ipa")]
const PLATFORM_CREDENTIAL_PAIR_BOUND_U128_COUNT_V1: usize =
    PLATFORM_CREDENTIAL_BASE_BOUND_U128_COUNT_V1
        + super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_BINDING_COUNT_V1;
#[cfg(feature = "zk-halo2-ipa")]
const PLATFORM_CREDENTIAL_EQ_BOUND_U128_COUNT_V1: usize =
    PLATFORM_CREDENTIAL_PAIR_BOUND_U128_COUNT_V1 + 2;

#[cfg(feature = "zk-halo2-ipa")]
fn validate_platform_credential_claim_pair_v1(
    witness: &KagemushaPlatformCredentialHashClaimPairWitnessV1<'_>,
) -> Result<(), String> {
    witness.relation.validate()?;
    let identities = [
        witness.eq_claim_protocol_digest,
        witness.ep_claim_protocol_digest,
        witness.eq_shard_protocol_digest,
        witness.ep_shard_protocol_digest,
    ];
    if identities.contains(&[0; 32])
        || identities
            .iter()
            .enumerate()
            .any(|(index, value)| identities[index + 1..].contains(value))
    {
        return Err(
            "Kagemusha PlatformCredential hash-claim protocols are absent or aliased".to_owned(),
        );
    }
    if witness.eq.claim_protocol.num_instance
        != [
            super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]
        || witness.ep.claim_protocol.num_instance
            != [
                super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
                super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
                super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            ]
        || witness.eq.claim_instances.len() != 3
        || witness.ep.claim_instances.len() != 3
        || witness.eq.claim_instances[0].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || witness.ep.claim_instances[0].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || witness.eq.claim_instances[1].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.ep.claim_instances[1].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.eq.claim_instances[2].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.ep.claim_instances[2].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err("Kagemusha PlatformCredential hash-claim public shape is not fixed".to_owned());
    }
    let eq_claim_tail = super::mint_hash_claim_fold::canonical_claim_carrier_binding_tail_v1(
        witness.eq.claim_instances,
    )?;
    let ep_claim_tail = super::mint_hash_claim_fold::canonical_claim_carrier_binding_tail_v1(
        witness.ep.claim_instances,
    )?;
    if eq_claim_tail != ep_claim_tail {
        return Err(
            "Kagemusha PlatformCredential paired claim carrier-binding tails are not canonical and identical"
                .to_owned(),
        );
    }
    if native_parent_protocol_digest_v1(witness.eq.claim_protocol, KagemushaPastaParityV1::Eq)?
        != witness.eq_claim_protocol_digest
        || native_parent_protocol_digest_v1(witness.ep.claim_protocol, KagemushaPastaParityV1::Ep)?
            != witness.ep_claim_protocol_digest
    {
        return Err(
            "Kagemusha PlatformCredential hash-claim protocol differs from its authenticated identity"
                .to_owned(),
        );
    }
    Ok(())
}

#[cfg(feature = "zk-halo2-ipa")]
fn assign_platform_credential_history_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<Vec<AssignedValue<F>>, String> {
    let limbs = history
        .chunks_exact(16)
        .map(|chunk| {
            let value = F::from_u128(u128::from_le_bytes(
                chunk.try_into().expect("history limb is sixteen bytes"),
            ));
            let assigned = ctx.load_witness(value);
            range.range_check(ctx, assigned, 128);
            assigned
        })
        .collect::<Vec<_>>();
    if limbs.len() != accumulator_limb_count() {
        return Err("Kagemusha PlatformCredential history has wrong shape".to_owned());
    }
    Ok(limbs)
}

#[cfg(feature = "zk-halo2-ipa")]
fn constant_platform_credential_digest_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    digest: DigestV1,
) -> [AssignedValue<F>; 2] {
    crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(digest).map(|limb| ctx.load_constant(limb))
}

#[cfg(feature = "zk-halo2-ipa")]
fn assigned_platform_credential_digest_v1<F: KagemushaPoseidonFieldV1>(
    limbs: &[AssignedValue<F>; 2],
) -> Result<DigestV1, String> {
    let mut digest = [0_u8; 32];
    for (index, limb) in limbs.iter().enumerate() {
        let bytes = halo2_base::utils::fe_to_biguint(limb.value()).to_bytes_le();
        if bytes.len() > 16 {
            return Err("Kagemusha PlatformCredential audit limb exceeds u128".to_owned());
        }
        digest[index * 16..index * 16 + bytes.len()].copy_from_slice(&bytes);
    }
    if digest == [0; 32] {
        return Err("Kagemusha PlatformCredential audit is zero".to_owned());
    }
    Ok(digest)
}

#[cfg(feature = "zk-halo2-ipa")]
fn platform_credential_audit_cells_v1<F: KagemushaPoseidonFieldV1>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
) -> Result<[AssignedValue<F>; 2], String> {
    let column = builder
        .assigned_instances
        .first()
        .ok_or_else(|| "Kagemusha PlatformCredential public column is absent".to_owned())?;
    if column.len() != KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("Kagemusha PlatformCredential public column has wrong shape".to_owned());
    }
    column[offset..offset + 2]
        .try_into()
        .map_err(|_| "Kagemusha PlatformCredential audit cells have wrong shape".to_owned())
}

#[cfg(feature = "zk-halo2-ipa")]
struct KagemushaPlatformCredentialScalarHalfV1<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + halo2_base::utils::ScalarField,
{
    builder: BaseCircuitBuilder<C::ScalarExt>,
    output: KagemushaDeferredParentOutputV1<C>,
    pair_binding: Vec<AssignedValue<C::ScalarExt>>,
}

#[cfg(feature = "zk-halo2-ipa")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn build_platform_credential_scalar_half_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    relation: &KagemushaPlatformCredentialRelationWitnessV1,
    eq_claim_protocol_digest: DigestV1,
    ep_claim_protocol_digest: DigestV1,
    eq_shard_protocol_digest: DigestV1,
    ep_shard_protocol_digest: DigestV1,
    eq_audit: DigestV1,
    ep_audit: DigestV1,
    eq_successor_history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    ep_successor_history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    claim: KagemushaPlatformCredentialHashClaimParityWitnessV1<'_, C>,
) -> Result<KagemushaPlatformCredentialScalarHalfV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let (mut builder, claimed_sha, assigned) = credential_builder(Some(relation))?;
    if claimed_sha.compression_blocks()? == 0 {
        return Err("Kagemusha PlatformCredential emitted an empty typed SHA queue".to_owned());
    }
    let range = builder.range_chip();
    let credential_digest = digest_limbs_assigned(builder.main(0), &assigned.credential_digest);
    let eq_claim_protocol =
        constant_platform_credential_digest_v1(builder.main(0), eq_claim_protocol_digest);
    let ep_claim_protocol =
        constant_platform_credential_digest_v1(builder.main(0), ep_claim_protocol_digest);
    let eq_shard_protocol =
        constant_platform_credential_digest_v1(builder.main(0), eq_shard_protocol_digest);
    let ep_shard_protocol =
        constant_platform_credential_digest_v1(builder.main(0), ep_shard_protocol_digest);
    let eq_audit = assign_digest(builder.main(0), &range, eq_audit);
    let eq_audit = digest_limbs_assigned(builder.main(0), &eq_audit);
    let ep_audit = assign_digest(builder.main(0), &range, ep_audit);
    let ep_audit = digest_limbs_assigned(builder.main(0), &ep_audit);
    let eq_history =
        assign_platform_credential_history_v1(builder.main(0), &range, eq_successor_history)?;
    let ep_history =
        assign_platform_credential_history_v1(builder.main(0), &range, ep_successor_history)?;
    let own_history = match parity {
        KagemushaPastaParityV1::Eq => &eq_history,
        KagemushaPastaParityV1::Ep => &ep_history,
    };
    builder.assigned_instances = vec![
        credential_digest
            .into_iter()
            .chain(eq_audit)
            .chain(ep_audit)
            .chain(own_history.iter().copied())
            .collect(),
    ];
    if builder.assigned_instances[0].len() != KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("Kagemusha PlatformCredential public ABI drifted".to_owned());
    }

    let mut pair_binding = credential_digest
        .into_iter()
        .chain(assigned.release_id)
        .chain(eq_claim_protocol)
        .chain(ep_claim_protocol)
        .chain(eq_shard_protocol)
        .chain(ep_shard_protocol)
        .chain(eq_history.iter().copied())
        .chain(ep_history.iter().copied())
        .collect::<Vec<_>>();
    if pair_binding.len() != PLATFORM_CREDENTIAL_BASE_BOUND_U128_COUNT_V1 {
        return Err("Kagemusha PlatformCredential base pair binding shape drifted".to_owned());
    }

    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    let structure = kagemusha_protocol_structure_digest_v1(claim.claim_protocol, parity)?;
    let expected_claim_protocol = match parity {
        KagemushaPastaParityV1::Eq => eq_claim_protocol,
        KagemushaPastaParityV1::Ep => ep_claim_protocol,
    };
    let loaded_claim = load_and_constrain_parent_protocol_v1(
        &loader,
        claim.claim_protocol,
        parity,
        structure,
        &expected_claim_protocol,
    )
    .map_err(|error| {
        format!("failed to bind Kagemusha PlatformCredential hash-claim protocol: {error:?}")
    })?;
    if loaded_claim.protocol.num_instance
        != [
            super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]
        || claim.claim_instances.len() != 3
        || claim.claim_instances[0].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || claim.claim_instances[1].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || claim.claim_instances[2].len()
            != super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err("Kagemusha PlatformCredential hash-claim public shape changed".to_owned());
    }
    let claim_semantic = claim.claim_instances[0]
        .iter()
        .map(|value| loader.assign_scalar(*value))
        .collect::<Vec<_>>();
    // Carry the proof-supplied commitment limbs, shared challenges, and both cross-carrier RLCs
    // into the reciprocal audit. The host check above guarantees one canonical u128 tail across
    // Pasta fields; the existing Ep-then-Eq reciprocal topology enforces it in both proofs.
    pair_binding.extend(
        claim_semantic[super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1
            ..super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1]
            .iter()
            .map(|value| *value.assigned()),
    );
    if pair_binding.len() != PLATFORM_CREDENTIAL_PAIR_BOUND_U128_COUNT_V1 {
        return Err("Kagemusha PlatformCredential claim-bound pair shape drifted".to_owned());
    }
    let claim_current = verify_two_carrier_hybrid_ordinary_proof_and_stream_v1(
        &loader,
        succinct_vk,
        &loaded_claim.protocol,
        &claim_semantic,
        match parity {
            KagemushaPastaParityV1::Eq => [
                [
                    super::mint_hash_claim_fold::public_instance::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO,
                    super::mint_hash_claim_fold::public_instance::EQ_PROOF_EQ_CARRIER_COMMITMENT_LO
                        + 1,
                ],
                [
                    super::mint_hash_claim_fold::public_instance::EQ_PROOF_EP_CARRIER_COMMITMENT_LO,
                    super::mint_hash_claim_fold::public_instance::EQ_PROOF_EP_CARRIER_COMMITMENT_LO
                        + 1,
                ],
            ],
            KagemushaPastaParityV1::Ep => [
                [
                    super::mint_hash_claim_fold::public_instance::EP_PROOF_EQ_CARRIER_COMMITMENT_LO,
                    super::mint_hash_claim_fold::public_instance::EP_PROOF_EQ_CARRIER_COMMITMENT_LO
                        + 1,
                ],
                [
                    super::mint_hash_claim_fold::public_instance::EP_PROOF_EP_CARRIER_COMMITMENT_LO,
                    super::mint_hash_claim_fold::public_instance::EP_PROOF_EP_CARRIER_COMMITMENT_LO
                        + 1,
                ],
            ],
        },
        claim.claim_proof,
    )
    .map_err(|error| {
        format!("failed to verify Kagemusha PlatformCredential terminal hash claim: {error:?}")
    })?;
    let claim_current_accumulator = claim_current.accumulator;
    drop(claim_current.loaded_stream);
    let claim_equation_count = loader.ecc_chip().equation_count();
    if claim_equation_count == 0 {
        return Err(
            "Kagemusha PlatformCredential hash-claim verifier emitted no equations".to_owned(),
        );
    }
    let claim_column = &claim_semantic
        [..super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1];
    {
        let chip = loader.ecc_chip();
        let mut loader_ctx = loader.ctx_mut();
        let assigned_claim = claim_column
            .iter()
            .map(|value| *value.assigned())
            .collect::<Vec<_>>();
        super::mint_hash_claim_fold::constrain_complete_claim_against_sha_jobs_v1(
            loader_ctx.main(),
            chip.range(),
            &claimed_sha,
            &assigned_claim,
            parity,
            assigned.release_id,
            eq_claim_protocol,
            ep_claim_protocol,
            eq_shard_protocol,
            ep_shard_protocol,
        )?;
    }
    drop(claimed_sha);
    let claim_history = load_native_accumulator(&loader, claim.claim_history)
        .map_err(|error| format!("failed to load PlatformCredential claim history: {error:?}"))?;
    let claim_history_cells = claim_column
        .get(super::mint_hash_claim_fold::public_instance::HISTORY_START..)
        .ok_or_else(|| "Kagemusha PlatformCredential claim history is absent".to_owned())?
        .iter()
        .map(|value| *value.assigned())
        .collect::<Vec<_>>();
    bind_accumulator_limbs(&loader, &claim_history, &claim_history_cells).map_err(|error| {
        format!("failed to bind Kagemusha PlatformCredential claim history: {error:?}")
    })?;
    drop(claim_history_cells);
    drop(claim_semantic);
    let complete_claim = verify_fold(
        &loader,
        succinct_vk,
        &[claim_current_accumulator, claim_history],
        claim.claim_history_fold_proof,
    )
    .map_err(|error| {
        format!("failed to fold Kagemusha PlatformCredential claim history: {error:?}")
    })?;
    bind_accumulator_limbs(&loader, &complete_claim, own_history).map_err(|error| {
        format!("failed to bind Kagemusha PlatformCredential successor history: {error:?}")
    })?;
    if loader.ecc_chip().equation_count() <= claim_equation_count {
        return Err(
            "Kagemusha PlatformCredential claim-history fold emitted no equation".to_owned(),
        );
    }
    let mut audit_binding = pair_binding.clone();
    if parity == KagemushaPastaParityV1::Eq {
        audit_binding.extend(ep_audit);
    }
    let expected_bound_count = match parity {
        KagemushaPastaParityV1::Eq => PLATFORM_CREDENTIAL_EQ_BOUND_U128_COUNT_V1,
        KagemushaPastaParityV1::Ep => PLATFORM_CREDENTIAL_PAIR_BOUND_U128_COUNT_V1,
    };
    if audit_binding.len() != expected_bound_count {
        return Err("Kagemusha PlatformCredential audit binding shape drifted".to_owned());
    }
    let output = finalize_tagged_deferred_audit_with_u128_binding_v1(
        &mut builder,
        loader,
        PLATFORM_CREDENTIAL_HASH_CLAIM_EQUATION_TAG_V1,
        &audit_binding,
    )
    .map_err(|error| {
        format!("failed to finalize Kagemusha PlatformCredential claim audit: {error:?}")
    })?;
    Ok(KagemushaPlatformCredentialScalarHalfV1 {
        builder,
        output,
        pair_binding,
    })
}

/// Discover the paired PlatformCredential claim audits while retaining no complete Base graph.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn discover_kagemusha_platform_credential_audits_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaPlatformCredentialHashClaimPairWitnessV1<'_>,
) -> Result<KagemushaPlatformCredentialAuditDiscoveryV1, String> {
    validate_platform_credential_claim_pair_v1(witness)?;
    let placeholder_eq = [1_u8; 32];
    let placeholder_ep = [2_u8; 32];
    let ep_svk = super::composite::ep_succinct_vk(ep_params);
    let ep = build_platform_credential_scalar_half_v1::<EpAffine>(
        &ep_svk,
        KagemushaPastaParityV1::Ep,
        &witness.relation,
        witness.eq_claim_protocol_digest,
        witness.ep_claim_protocol_digest,
        witness.eq_shard_protocol_digest,
        witness.ep_shard_protocol_digest,
        placeholder_eq,
        placeholder_ep,
        witness.eq.successor_history,
        witness.ep.successor_history,
        witness.ep,
    )?;
    let ep_digest = assigned_platform_credential_digest_v1(&ep.output.audit_digest_limbs)?;
    let ep_output = ep.output;
    drop(ep.builder);
    drop(ep.pair_binding);
    halo2_proofs::release_allocator_slack();

    let eq_svk = super::composite::eq_succinct_vk(eq_params);
    let eq = build_platform_credential_scalar_half_v1::<EqAffine>(
        &eq_svk,
        KagemushaPastaParityV1::Eq,
        &witness.relation,
        witness.eq_claim_protocol_digest,
        witness.ep_claim_protocol_digest,
        witness.eq_shard_protocol_digest,
        witness.ep_shard_protocol_digest,
        placeholder_eq,
        ep_digest,
        witness.eq.successor_history,
        witness.ep.successor_history,
        witness.eq,
    )?;
    let eq_digest = assigned_platform_credential_digest_v1(&eq.output.audit_digest_limbs)?;
    let eq_output = eq.output;
    drop(eq.builder);
    drop(eq.pair_binding);
    halo2_proofs::release_allocator_slack();
    Ok(KagemushaPlatformCredentialAuditDiscoveryV1 {
        eq_output,
        ep_output,
        eq_digest,
        ep_digest,
    })
}

/// Build the exact Eq PlatformCredential producer from detached reciprocal audit material.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn build_kagemusha_platform_credential_eq_v1(
    eq_params: &ParamsIPA<EqAffine>,
    witness: &KagemushaPlatformCredentialHashClaimPairWitnessV1<'_>,
    discovery: &KagemushaPlatformCredentialAuditDiscoveryV1,
) -> Result<KagemushaPlatformCredentialRelationCircuitV1<Fp>, String> {
    validate_platform_credential_claim_pair_v1(witness)?;
    let eq_svk = super::composite::eq_succinct_vk(eq_params);
    let KagemushaPlatformCredentialScalarHalfV1 {
        builder: mut builder,
        output,
        pair_binding,
    } = build_platform_credential_scalar_half_v1::<EqAffine>(
        &eq_svk,
        KagemushaPastaParityV1::Eq,
        &witness.relation,
        witness.eq_claim_protocol_digest,
        witness.ep_claim_protocol_digest,
        witness.eq_shard_protocol_digest,
        witness.ep_shard_protocol_digest,
        discovery.eq_digest,
        discovery.ep_digest,
        witness.eq.successor_history,
        witness.ep.successor_history,
        witness.eq,
    )?;
    if assigned_platform_credential_digest_v1(&output.audit_digest_limbs)? != discovery.eq_digest {
        return Err("Kagemusha Eq PlatformCredential audit changed after discovery".to_owned());
    }
    for (actual, expected) in
        output
            .audit_digest_limbs
            .iter()
            .zip(platform_credential_audit_cells_v1(
                &builder,
                platform_credential_public_instance::EQ_AUDIT_LO,
            )?)
    {
        builder.main(0).constrain_equal(actual, &expected);
    }
    let expected_ep = platform_credential_audit_cells_v1(
        &builder,
        platform_credential_public_instance::EP_AUDIT_LO,
    )?;
    let mut dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_output_with_u128_binding_v1::<EpAffine>(
        &mut builder,
        &discovery.ep_output,
        &expected_ep,
        &pair_binding,
        &mut dense,
    )?;
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    dense.validate_capacity((1_usize << KAGEMUSHA_HALO2_K_V1) - MINIMUM_UNUSABLE_ROWS)?;
    Ok(KagemushaPlatformCredentialRelationCircuitV1 {
        builder,
        dense_jobs: KagemushaPlatformCredentialDenseJobsV1::Eq(dense),
    })
}

/// Build the exact Ep PlatformCredential producer from detached reciprocal audit material.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn build_kagemusha_platform_credential_ep_v1(
    ep_params: &ParamsIPA<EpAffine>,
    witness: &KagemushaPlatformCredentialHashClaimPairWitnessV1<'_>,
    discovery: &KagemushaPlatformCredentialAuditDiscoveryV1,
) -> Result<KagemushaPlatformCredentialRelationCircuitV1<Fq>, String> {
    validate_platform_credential_claim_pair_v1(witness)?;
    let ep_svk = super::composite::ep_succinct_vk(ep_params);
    let KagemushaPlatformCredentialScalarHalfV1 {
        builder: mut builder,
        output,
        mut pair_binding,
    } = build_platform_credential_scalar_half_v1::<EpAffine>(
        &ep_svk,
        KagemushaPastaParityV1::Ep,
        &witness.relation,
        witness.eq_claim_protocol_digest,
        witness.ep_claim_protocol_digest,
        witness.eq_shard_protocol_digest,
        witness.ep_shard_protocol_digest,
        discovery.eq_digest,
        discovery.ep_digest,
        witness.eq.successor_history,
        witness.ep.successor_history,
        witness.ep,
    )?;
    if assigned_platform_credential_digest_v1(&output.audit_digest_limbs)? != discovery.ep_digest {
        return Err("Kagemusha Ep PlatformCredential audit changed after discovery".to_owned());
    }
    for (actual, expected) in
        output
            .audit_digest_limbs
            .iter()
            .zip(platform_credential_audit_cells_v1(
                &builder,
                platform_credential_public_instance::EP_AUDIT_LO,
            )?)
    {
        builder.main(0).constrain_equal(actual, &expected);
    }
    let expected_eq = platform_credential_audit_cells_v1(
        &builder,
        platform_credential_public_instance::EQ_AUDIT_LO,
    )?;
    pair_binding.extend(platform_credential_audit_cells_v1(
        &builder,
        platform_credential_public_instance::EP_AUDIT_LO,
    )?);
    if pair_binding.len() != PLATFORM_CREDENTIAL_EQ_BOUND_U128_COUNT_V1 {
        return Err("Kagemusha Eq PlatformCredential reciprocal binding drifted".to_owned());
    }
    let mut dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_output_with_u128_binding_v1::<EqAffine>(
        &mut builder,
        &discovery.eq_output,
        &expected_eq,
        &pair_binding,
        &mut dense,
    )?;
    builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    dense.validate_capacity((1_usize << KAGEMUSHA_HALO2_K_V1) - MINIMUM_UNUSABLE_ROWS)?;
    Ok(KagemushaPlatformCredentialRelationCircuitV1 {
        builder,
        dense_jobs: KagemushaPlatformCredentialDenseJobsV1::Ep(dense),
    })
}

/// Assigned semantic outputs consumed by the aggregate recursion circuit.
pub(super) struct KagemushaAssignedGuardBundleV1<F: KagemushaPoseidonFieldV1> {
    /// Canonical normalized statement digest.
    pub(super) guard_digest: [PastaSha256ByteV1<F>; 32],
    /// Canonical predecessor and successor credential statement digests.
    pub(super) credential_digests: [[PastaSha256ByteV1<F>; 32]; 2],
    pub(super) protocol_version: AssignedValue<F>,
    pub(super) predecessor_suite_id: [AssignedValue<F>; 2],
    pub(super) predecessor_vk_digest: [AssignedValue<F>; 2],
    pub(super) successor_suite_id: [AssignedValue<F>; 2],
    pub(super) successor_vk_digest: [AssignedValue<F>; 2],
    pub(super) operation: AssignedValue<F>,
    pub(super) amount: AssignedValue<F>,
    pub(super) peer_credit_id: [AssignedValue<F>; 2],
    pub(super) recipient_encryption_key_binding: [AssignedValue<F>; 2],
    pub(super) mint_finality_proof_binding_digest: [AssignedValue<F>; 2],
    pub(super) predecessor_release_id: [AssignedValue<F>; 2],
    pub(super) release_id: [AssignedValue<F>; 2],
    pub(super) network_id: [AssignedValue<F>; 2],
    pub(super) asset_id: [AssignedValue<F>; 2],
    pub(super) asset_incarnation: [AssignedValue<F>; 2],
    pub(super) asset_scale: AssignedValue<F>,
    pub(super) liability_pool_id: [AssignedValue<F>; 2],
    pub(super) hardware_profile_id: [AssignedValue<F>; 2],
    pub(super) policy_epoch: AssignedValue<F>,
    pub(super) lane_id: [AssignedValue<F>; 2],
    pub(super) predecessor_state: [AssignedValue<F>; 2],
    pub(super) successor_state: [AssignedValue<F>; 2],
    pub(super) predecessor_nonce: [AssignedValue<F>; 2],
    pub(super) successor_nonce: [AssignedValue<F>; 2],
    pub(super) predecessor_sequence: AssignedValue<F>,
    pub(super) successor_sequence: AssignedValue<F>,
    pub(super) predecessor_generation: AssignedValue<F>,
    pub(super) successor_generation: AssignedValue<F>,
    pub(super) predecessor_epoch: [AssignedValue<F>; 2],
    pub(super) successor_epoch: [AssignedValue<F>; 2],
    pub(super) predecessor_key: [AssignedValue<F>; 2],
    pub(super) successor_key: [AssignedValue<F>; 2],
    pub(super) predecessor_policy: [AssignedValue<F>; 2],
    pub(super) successor_policy: [AssignedValue<F>; 2],
    pub(super) journal_before: AssignedValue<F>,
    pub(super) journal_after: AssignedValue<F>,
    pub(super) lifecycle_binding_digest: [AssignedValue<F>; 2],
    pub(super) prepared_transition_binding_digest: [AssignedValue<F>; 2],
    pub(super) terminal_commit_binding_digest: [AssignedValue<F>; 2],
    pub(super) sender_one_time_authorization_digest: [AssignedValue<F>; 2],
    pub(super) receive_credit_binding_digest: [AssignedValue<F>; 2],
    pub(super) transition_intent: [AssignedValue<F>; 2],
    pub(super) transition_effect: [AssignedValue<F>; 2],
    pub(super) recovery_record: [AssignedValue<F>; 2],
    pub(super) durable_inbox_effect: [AssignedValue<F>; 2],
    pub(super) durable_outbox_effect: [AssignedValue<F>; 2],
}

#[derive(Clone)]
pub(super) struct AssignedCredentialV1<F: KagemushaPoseidonFieldV1> {
    pub(super) version: AssignedUint<F>,
    pub(super) protocol_version: AssignedUint<F>,
    pub(super) suite_id: [PastaSha256ByteV1<F>; 32],
    pub(super) release_id: [PastaSha256ByteV1<F>; 32],
    pub(super) network_id: [PastaSha256ByteV1<F>; 32],
    pub(super) asset_id: [PastaSha256ByteV1<F>; 32],
    pub(super) asset_incarnation: [PastaSha256ByteV1<F>; 32],
    pub(super) asset_scale: AssignedUint<F>,
    pub(super) liability_pool_id: [PastaSha256ByteV1<F>; 32],
    pub(super) lane_id: [PastaSha256ByteV1<F>; 32],
    pub(super) epoch_generation: AssignedUint<F>,
    pub(super) epoch_id: [PastaSha256ByteV1<F>; 32],
    pub(super) key_reference: [PastaSha256ByteV1<F>; 32],
    pub(super) device_public_key: Vec<PastaSha256ByteV1<F>>,
    pub(super) policy_id: [PastaSha256ByteV1<F>; 32],
    pub(super) hardware_profile_id: [PastaSha256ByteV1<F>; 32],
    pub(super) policy_epoch: AssignedUint<F>,
    pub(super) device_authority_commitment: [PastaSha256ByteV1<F>; 32],
    pub(super) credential_issuance_digest: [PastaSha256ByteV1<F>; 32],
    pub(super) empty_effect: [PastaSha256ByteV1<F>; 32],
    pub(super) digest: [PastaSha256ByteV1<F>; 32],
}

/// Constrain the complete field-neutral GuardBundle statement and credential bindings.
///
/// The enclosing paired recursive circuit must additionally verify the two credential ordinary
/// proofs against `credential_digests`, finalize one tagged scalar audit, and enforce the
/// reciprocal audit in the other Pasta parity. This function deliberately exposes no host-side
/// boolean that could stand in for those proof constraints.
pub(super) fn constrain_guard_bundle_semantics_v1<F>(
    builder: &mut BaseCircuitBuilder<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    witness: &KagemushaGuardBundleRelationWitnessV1,
) -> Result<KagemushaAssignedGuardBundleV1<F>, String>
where
    F: KagemushaPoseidonFieldV1,
{
    witness.validate()?;
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let gate = range.gate();
    let statement = &witness.statement;

    let version = assign_uint_le(ctx, &range, u128::from(statement.version), 16);
    gate.assert_is_const(ctx, &version.value, &F::ONE);
    let protocol_version = assign_uint_le(ctx, &range, u128::from(statement.protocol_version), 16);
    gate.assert_is_const(ctx, &protocol_version.value, &F::ONE);
    let predecessor_suite = assign_digest(ctx, &range, statement.predecessor_suite_id);
    let predecessor_vk = assign_digest(ctx, &range, statement.predecessor_vk_digest);
    let successor_suite = assign_digest(ctx, &range, statement.successor_suite_id);
    let successor_vk = assign_digest(ctx, &range, statement.successor_vk_digest);
    let operation = assign_uint_le(
        ctx,
        &range,
        u128::from(operation_tag(statement.operation)),
        8,
    );
    let selectors: [AssignedValue<F>; 6] = core::array::from_fn(|tag| {
        gate.is_equal(
            ctx,
            operation.value,
            QuantumCell::Constant(F::from(tag as u64)),
        )
    });
    let selector_sum = gate.sum(ctx, selectors);
    gate.assert_is_const(ctx, &selector_sum, &F::ONE);
    let amount = assign_uint_le(ctx, &range, statement.amount, 128);
    let bootstrap = selectors[0];
    let inbound = gate.add(ctx, selectors[1], selectors[3]);
    let outbound = gate.add(ctx, selectors[2], selectors[4]);
    let uses_outbox = gate.add(ctx, selectors[2], selectors[4]);
    let rotate = selectors[5];
    let exact_next = gate.sum(ctx, selectors[1..5].iter().copied());
    let journal_next = gate.sum(ctx, selectors[1..5].iter().copied());
    let non_bootstrap = gate.not(ctx, bootstrap);
    let empty_effect_operation = gate.add(ctx, bootstrap, rotate);
    let monetary = gate.sum(ctx, selectors[1..5].iter().copied());
    assert_if_zero(ctx, &range, empty_effect_operation, amount.value);
    assert_if_nonzero(ctx, &range, monetary, amount.value);
    let peer_credit_id = assign_digest(ctx, &range, statement.peer_credit_id);
    let recipient_encryption_key_binding =
        assign_digest(ctx, &range, statement.recipient_encryption_key_binding);
    let mint_finality_proof_binding_digest =
        assign_digest(ctx, &range, statement.mint_finality_proof_binding_digest);
    let peer = selectors[2];
    let not_peer = gate.not(ctx, peer);
    for digest in [&peer_credit_id, &recipient_encryption_key_binding] {
        assert_if_digest_nonzero(ctx, &range, peer, digest);
        assert_if_digest_zero(ctx, &range, not_peer, digest);
    }
    assert_if_digest_nonzero(
        ctx,
        &range,
        selectors[1],
        &mint_finality_proof_binding_digest,
    );
    let not_mint = gate.not(ctx, selectors[1]);
    assert_if_digest_zero(ctx, &range, not_mint, &mint_finality_proof_binding_digest);

    let predecessor_release = assign_digest(ctx, &range, statement.predecessor_release_id);
    let release = assign_digest(ctx, &range, statement.release_id);
    let network = assign_digest(ctx, &range, statement.network_id);
    let asset = assign_digest(ctx, &range, statement.asset_id);
    let asset_incarnation = assign_digest(ctx, &range, *statement.asset_incarnation.as_bytes());
    let scale = assign_uint_le(ctx, &range, u128::from(statement.asset_scale), 32);
    let scale_ok = range.is_less_than_safe(
        ctx,
        scale.value,
        u64::from(KAGEMUSHA_ASSET_SCALE_MAX_V1) + 1,
    );
    gate.assert_is_const(ctx, &scale_ok, &F::ONE);
    let pool = assign_digest(ctx, &range, statement.liability_pool_id);
    let hardware_profile = assign_digest(ctx, &range, statement.hardware_profile_id);
    let policy_epoch = assign_uint_le(ctx, &range, u128::from(statement.policy_epoch), 64);
    let lane = assign_digest(ctx, &range, statement.lane_id);
    let predecessor_state = assign_digest(ctx, &range, statement.predecessor_state_commitment);
    let successor_state = assign_digest(ctx, &range, statement.successor_state_commitment);
    let predecessor_nonce =
        assign_digest(ctx, &range, statement.predecessor_state_nonce_commitment);
    let successor_nonce = assign_digest(ctx, &range, statement.successor_state_nonce_commitment);
    let predecessor_sequence =
        assign_uint_le(ctx, &range, statement.predecessor_logical_sequence, 128);
    let successor_sequence = assign_uint_le(ctx, &range, statement.successor_logical_sequence, 128);
    let predecessor_generation = assign_uint_le(
        ctx,
        &range,
        statement.predecessor_hardware_epoch_generation,
        128,
    );
    let successor_generation = assign_uint_le(
        ctx,
        &range,
        statement.successor_hardware_epoch_generation,
        128,
    );
    let predecessor_epoch = assign_digest(ctx, &range, statement.predecessor_hardware_epoch_id);
    let successor_epoch = assign_digest(ctx, &range, statement.successor_hardware_epoch_id);
    let predecessor_key = assign_digest(ctx, &range, statement.predecessor_key_reference);
    let successor_key = assign_digest(ctx, &range, statement.successor_key_reference);
    let predecessor_policy = assign_digest(ctx, &range, statement.predecessor_hardware_policy_id);
    let successor_policy = assign_digest(ctx, &range, statement.successor_hardware_policy_id);
    let journal_before = assign_uint_le(ctx, &range, statement.journal_revision_before, 128);
    let journal_after = assign_uint_le(ctx, &range, statement.journal_revision_after, 128);
    let lifecycle = assign_digest(ctx, &range, statement.lifecycle_binding_digest);
    let prepared_transition =
        assign_digest(ctx, &range, statement.prepared_transition_binding_digest);
    let terminal_commit = assign_digest(ctx, &range, statement.terminal_commit_binding_digest);
    let sender_authorization =
        assign_digest(ctx, &range, statement.sender_one_time_authorization_digest);
    let receive_credit_binding =
        assign_digest(ctx, &range, statement.receive_credit_binding_digest);
    let intent = assign_digest(ctx, &range, statement.transition_intent_digest);
    let effect = assign_digest(ctx, &range, statement.transition_effect_digest);
    let recovery = assign_digest(ctx, &range, statement.recovery_record_digest);
    let inbox = assign_digest(ctx, &range, statement.durable_inbox_effect_digest);
    let outbox = assign_digest(ctx, &range, statement.durable_outbox_effect_digest);
    let empty_effect = assign_digest(ctx, &range, witness.canonical_empty_effect_digest);

    for digest in [
        &release,
        &network,
        &asset,
        &asset_incarnation,
        &pool,
        &hardware_profile,
        &successor_suite,
        &successor_vk,
        &lifecycle,
        &lane,
        &successor_state,
        &successor_nonce,
        &successor_epoch,
        &successor_key,
        &successor_policy,
        &intent,
        &effect,
        &recovery,
        &inbox,
        &outbox,
        &empty_effect,
    ] {
        assert_digest_nonzero(ctx, &range, digest);
    }
    assert_nonzero(ctx, &range, policy_epoch.value);
    let incarnation_bits = gate.num_to_bits(
        ctx,
        asset_incarnation[31]
            .assigned()
            .expect("asset incarnation byte is assigned"),
        8,
    );
    gate.assert_is_const(ctx, &incarnation_bits[0], &F::ONE);
    assert_if_digest_zero(ctx, &range, bootstrap, &predecessor_suite);
    assert_if_digest_zero(ctx, &range, bootstrap, &predecessor_vk);
    assert_if_digest_zero(ctx, &range, bootstrap, &predecessor_release);
    assert_if_digest_zero(ctx, &range, bootstrap, &predecessor_state);
    assert_if_digest_zero(ctx, &range, bootstrap, &predecessor_nonce);
    assert_if_digest_zero(ctx, &range, bootstrap, &predecessor_epoch);
    assert_if_digest_zero(ctx, &range, bootstrap, &predecessor_key);
    assert_if_digest_zero(ctx, &range, bootstrap, &predecessor_policy);
    assert_if_zero(ctx, &range, bootstrap, predecessor_sequence.value);
    assert_if_zero(ctx, &range, bootstrap, successor_sequence.value);
    assert_if_zero(ctx, &range, bootstrap, predecessor_generation.value);
    assert_if_zero(ctx, &range, bootstrap, journal_before.value);
    assert_if_zero(ctx, &range, bootstrap, journal_after.value);

    for digest in [
        &predecessor_release,
        &predecessor_suite,
        &predecessor_vk,
        &predecessor_state,
        &predecessor_nonce,
        &predecessor_epoch,
        &predecessor_key,
        &predecessor_policy,
    ] {
        assert_if_digest_nonzero(ctx, &range, non_bootstrap, digest);
    }
    assert_if_digest_equal(ctx, &range, non_bootstrap, &predecessor_release, &release);
    assert_if_digest_equal(
        ctx,
        &range,
        non_bootstrap,
        &predecessor_suite,
        &successor_suite,
    );
    assert_if_digest_equal(ctx, &range, non_bootstrap, &predecessor_vk, &successor_vk);

    assert_if_digest_nonzero(ctx, &range, uses_outbox, &prepared_transition);
    let no_outbox = gate.not(ctx, uses_outbox);
    assert_if_digest_zero(ctx, &range, no_outbox, &prepared_transition);
    let not_outbound = gate.not(ctx, outbound);
    assert_if_digest_zero(ctx, &range, not_outbound, &terminal_commit);
    let not_send = gate.not(ctx, selectors[2]);
    assert_if_digest_zero(ctx, &range, not_send, &sender_authorization);
    let terminal_limbs = digest_limbs_assigned(ctx, &terminal_commit);
    let terminal_low_zero = gate.is_zero(ctx, terminal_limbs[0]);
    let terminal_high_zero = gate.is_zero(ctx, terminal_limbs[1]);
    let terminal_absent = gate.and(ctx, terminal_low_zero, terminal_high_zero);
    let terminal_present = gate.not(ctx, terminal_absent);
    let sender_authorization_limbs = digest_limbs_assigned(ctx, &sender_authorization);
    let sender_low_zero = gate.is_zero(ctx, sender_authorization_limbs[0]);
    let sender_high_zero = gate.is_zero(ctx, sender_authorization_limbs[1]);
    let sender_absent = gate.and(ctx, sender_low_zero, sender_high_zero);
    let sender_present = gate.not(ctx, sender_absent);
    assert_if_equal_value(ctx, &range, peer, terminal_present, sender_present);
    let receive = selectors[3];
    let not_receive = gate.not(ctx, receive);
    assert_if_digest_nonzero(ctx, &range, receive, &receive_credit_binding);
    assert_if_digest_zero(ctx, &range, not_receive, &receive_credit_binding);
    let state_changes = gate.add(ctx, exact_next, rotate);
    assert_if_digest_different(
        ctx,
        &range,
        state_changes,
        &predecessor_state,
        &successor_state,
    );
    assert_if_digest_different(
        ctx,
        &range,
        state_changes,
        &predecessor_nonce,
        &successor_nonce,
    );
    assert_if_increment(
        ctx,
        &range,
        exact_next,
        predecessor_sequence.value,
        successor_sequence.value,
    );
    assert_if_increment(
        ctx,
        &range,
        journal_next,
        journal_before.value,
        journal_after.value,
    );
    assert_if_zero(ctx, &range, rotate, successor_sequence.value);
    assert_if_zero(ctx, &range, rotate, journal_after.value);

    assert_if_equal_value(
        ctx,
        &range,
        journal_next,
        predecessor_generation.value,
        successor_generation.value,
    );
    for (before, after) in [
        (&predecessor_epoch, &successor_epoch),
        (&predecessor_key, &successor_key),
        (&predecessor_policy, &successor_policy),
    ] {
        assert_if_digest_equal(ctx, &range, journal_next, before, after);
    }
    assert_if_increment(
        ctx,
        &range,
        rotate,
        predecessor_generation.value,
        successor_generation.value,
    );
    assert_if_digest_different(ctx, &range, rotate, &predecessor_epoch, &successor_epoch);
    assert_if_digest_different(ctx, &range, rotate, &predecessor_key, &successor_key);

    assert_if_digest_different(ctx, &range, inbound, &inbox, &empty_effect);
    assert_if_digest_equal(ctx, &range, inbound, &outbox, &empty_effect);
    assert_if_digest_equal(ctx, &range, uses_outbox, &inbox, &empty_effect);
    assert_if_digest_different(ctx, &range, uses_outbox, &outbox, &empty_effect);
    assert_if_digest_equal(ctx, &range, empty_effect_operation, &inbox, &empty_effect);
    assert_if_digest_equal(ctx, &range, empty_effect_operation, &outbox, &empty_effect);

    let predecessor_credential =
        assign_credential_statement_v1(ctx, &range, jobs, &witness.predecessor_credential)?;
    let successor_credential =
        assign_credential_statement_v1(ctx, &range, jobs, &witness.successor_credential)?;
    for credential in [&predecessor_credential, &successor_credential] {
        assert_equal_value(ctx, credential.version.value, version.value);
        assert_equal_value(
            ctx,
            credential.protocol_version.value,
            protocol_version.value,
        );
        for (credential_value, guard_value) in [
            (&credential.asset_incarnation, &asset_incarnation),
            (&credential.network_id, &network),
            (&credential.asset_id, &asset),
            (&credential.liability_pool_id, &pool),
            (&credential.hardware_profile_id, &hardware_profile),
            (&credential.lane_id, &lane),
            (&credential.empty_effect, &empty_effect),
        ] {
            bind_equal_digest(ctx, &range, credential_value, guard_value);
        }
        assert_equal_value(ctx, credential.asset_scale.value, scale.value);
        assert_equal_value(ctx, credential.policy_epoch.value, policy_epoch.value);
    }
    bind_equal_digest(ctx, &range, &successor_credential.release_id, &release);
    assert_if_digest_equal(
        ctx,
        &range,
        non_bootstrap,
        &predecessor_credential.release_id,
        &predecessor_release,
    );
    bind_equal_digest(
        ctx,
        &range,
        &successor_credential.suite_id,
        &successor_suite,
    );
    bind_equal_digest(
        ctx,
        &range,
        &successor_credential.epoch_id,
        &successor_epoch,
    );
    bind_equal_digest(
        ctx,
        &range,
        &successor_credential.key_reference,
        &successor_key,
    );
    bind_equal_digest(
        ctx,
        &range,
        &successor_credential.policy_id,
        &successor_policy,
    );
    assert_equal_value(
        ctx,
        successor_credential.epoch_generation.value,
        successor_generation.value,
    );
    assert_if_digest_equal(
        ctx,
        &range,
        non_bootstrap,
        &predecessor_credential.suite_id,
        &predecessor_suite,
    );
    assert_if_digest_equal(
        ctx,
        &range,
        non_bootstrap,
        &predecessor_credential.epoch_id,
        &predecessor_epoch,
    );
    assert_if_digest_equal(
        ctx,
        &range,
        non_bootstrap,
        &predecessor_credential.key_reference,
        &predecessor_key,
    );
    assert_if_digest_equal(
        ctx,
        &range,
        non_bootstrap,
        &predecessor_credential.policy_id,
        &predecessor_policy,
    );
    assert_if_equal_value(
        ctx,
        &range,
        non_bootstrap,
        predecessor_credential.epoch_generation.value,
        predecessor_generation.value,
    );
    let same_credential = gate.add(ctx, bootstrap, monetary);
    assert_if_digest_equal(
        ctx,
        &range,
        same_credential,
        &predecessor_credential.digest,
        &successor_credential.digest,
    );
    assert_if_digest_different(
        ctx,
        &range,
        rotate,
        &predecessor_credential.digest,
        &successor_credential.digest,
    );

    for (secret, credential) in [
        (
            witness.predecessor_device_authority_secret,
            &predecessor_credential,
        ),
        (
            witness.successor_device_authority_secret,
            &successor_credential,
        ),
    ] {
        let secret = assign_bytes(ctx, &range, &secret);
        assert_bytes_nonzero(ctx, &range, &secret);
        let commitment = hash(
            ctx,
            jobs,
            [
                constant_bytes(DEVICE_AUTHORITY_DOMAIN),
                vec![PastaSha256ByteV1::constant(0)],
                secret,
            ]
            .concat(),
        )?;
        bind_equal_digest(
            ctx,
            &range,
            &commitment,
            &credential.device_authority_commitment,
        );
    }

    let payload_length = normalized_guard_statement_payload_v1(statement).len();
    let guard_digest = hash(
        ctx,
        jobs,
        [
            constant_bytes(
                &u64::try_from(super::GUARD_STATEMENT_DIGEST_DOMAIN.len())
                    .expect("fixed GuardBundle digest domain fits u64")
                    .to_be_bytes(),
            ),
            constant_bytes(super::GUARD_STATEMENT_DIGEST_DOMAIN),
            constant_bytes(
                &u64::try_from(payload_length)
                    .expect("fixed GuardBundle statement fits u64")
                    .to_be_bytes(),
            ),
            version.bytes,
            protocol_version.bytes,
            predecessor_suite.to_vec(),
            predecessor_vk.to_vec(),
            successor_suite.to_vec(),
            successor_vk.to_vec(),
            operation.bytes,
            amount.bytes,
            peer_credit_id.to_vec(),
            recipient_encryption_key_binding.to_vec(),
            mint_finality_proof_binding_digest.to_vec(),
            predecessor_release.to_vec(),
            release.to_vec(),
            network.to_vec(),
            asset.to_vec(),
            asset_incarnation.to_vec(),
            scale.bytes,
            pool.to_vec(),
            hardware_profile.to_vec(),
            policy_epoch.bytes,
            lane.to_vec(),
            predecessor_state.to_vec(),
            successor_state.to_vec(),
            predecessor_nonce.to_vec(),
            successor_nonce.to_vec(),
            predecessor_sequence.bytes,
            successor_sequence.bytes,
            predecessor_generation.bytes,
            successor_generation.bytes,
            predecessor_epoch.to_vec(),
            successor_epoch.to_vec(),
            predecessor_key.to_vec(),
            successor_key.to_vec(),
            predecessor_policy.to_vec(),
            successor_policy.to_vec(),
            journal_before.bytes,
            journal_after.bytes,
            lifecycle.to_vec(),
            prepared_transition.to_vec(),
            terminal_commit.to_vec(),
            sender_authorization.to_vec(),
            receive_credit_binding.to_vec(),
            intent.to_vec(),
            effect.to_vec(),
            recovery.to_vec(),
            inbox.to_vec(),
            outbox.to_vec(),
        ]
        .concat(),
    )?;
    Ok(KagemushaAssignedGuardBundleV1 {
        guard_digest,
        credential_digests: [predecessor_credential.digest, successor_credential.digest],
        protocol_version: protocol_version.value,
        predecessor_suite_id: digest_limbs_assigned(ctx, &predecessor_suite),
        predecessor_vk_digest: digest_limbs_assigned(ctx, &predecessor_vk),
        successor_suite_id: digest_limbs_assigned(ctx, &successor_suite),
        successor_vk_digest: digest_limbs_assigned(ctx, &successor_vk),
        operation: operation.value,
        amount: amount.value,
        peer_credit_id: digest_limbs_assigned(ctx, &peer_credit_id),
        recipient_encryption_key_binding: digest_limbs_assigned(
            ctx,
            &recipient_encryption_key_binding,
        ),
        mint_finality_proof_binding_digest: digest_limbs_assigned(
            ctx,
            &mint_finality_proof_binding_digest,
        ),
        predecessor_release_id: digest_limbs_assigned(ctx, &predecessor_release),
        release_id: digest_limbs_assigned(ctx, &release),
        network_id: digest_limbs_assigned(ctx, &network),
        asset_id: digest_limbs_assigned(ctx, &asset),
        asset_incarnation: digest_limbs_assigned(ctx, &asset_incarnation),
        asset_scale: scale.value,
        liability_pool_id: digest_limbs_assigned(ctx, &pool),
        hardware_profile_id: digest_limbs_assigned(ctx, &hardware_profile),
        policy_epoch: policy_epoch.value,
        lane_id: digest_limbs_assigned(ctx, &lane),
        predecessor_state: digest_limbs_assigned(ctx, &predecessor_state),
        successor_state: digest_limbs_assigned(ctx, &successor_state),
        predecessor_nonce: digest_limbs_assigned(ctx, &predecessor_nonce),
        successor_nonce: digest_limbs_assigned(ctx, &successor_nonce),
        predecessor_sequence: predecessor_sequence.value,
        successor_sequence: successor_sequence.value,
        predecessor_generation: predecessor_generation.value,
        successor_generation: successor_generation.value,
        predecessor_epoch: digest_limbs_assigned(ctx, &predecessor_epoch),
        successor_epoch: digest_limbs_assigned(ctx, &successor_epoch),
        predecessor_key: digest_limbs_assigned(ctx, &predecessor_key),
        successor_key: digest_limbs_assigned(ctx, &successor_key),
        predecessor_policy: digest_limbs_assigned(ctx, &predecessor_policy),
        successor_policy: digest_limbs_assigned(ctx, &successor_policy),
        journal_before: journal_before.value,
        journal_after: journal_after.value,
        lifecycle_binding_digest: digest_limbs_assigned(ctx, &lifecycle),
        prepared_transition_binding_digest: digest_limbs_assigned(ctx, &prepared_transition),
        terminal_commit_binding_digest: digest_limbs_assigned(ctx, &terminal_commit),
        sender_one_time_authorization_digest: digest_limbs_assigned(ctx, &sender_authorization),
        receive_credit_binding_digest: digest_limbs_assigned(ctx, &receive_credit_binding),
        transition_intent: digest_limbs_assigned(ctx, &intent),
        transition_effect: digest_limbs_assigned(ctx, &effect),
        recovery_record: digest_limbs_assigned(ctx, &recovery),
        durable_inbox_effect: digest_limbs_assigned(ctx, &inbox),
        durable_outbox_effect: digest_limbs_assigned(ctx, &outbox),
    })
}

pub(super) fn assign_credential_statement_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    statement: &KagemushaPlatformCredentialStatementV1,
) -> Result<AssignedCredentialV1<F>, String> {
    let version = assign_uint_le(ctx, range, u128::from(statement.version), 16);
    let protocol_version = assign_uint_le(ctx, range, u128::from(statement.protocol_version), 16);
    let suite_id = assign_digest(ctx, range, statement.suite_id);
    let release_id = assign_digest(ctx, range, statement.release_id);
    let network_id = assign_digest(ctx, range, statement.network_id);
    let asset_id = assign_digest(ctx, range, statement.asset_id);
    let asset_incarnation = assign_digest(ctx, range, *statement.asset_incarnation.as_bytes());
    let asset_scale = assign_uint_le(ctx, range, u128::from(statement.asset_scale), 32);
    let liability_pool_id = assign_digest(ctx, range, statement.liability_pool_id);
    let lane_id = assign_digest(ctx, range, statement.lane_id);
    let epoch_generation = assign_uint_le(ctx, range, statement.hardware_epoch_generation, 128);
    let epoch_id = assign_digest(ctx, range, statement.hardware_epoch_id);
    let key_reference = assign_digest(ctx, range, statement.key_reference);
    let device_public_key = assign_bytes(ctx, range, statement.device_public_key.as_sec1_bytes());
    let policy_id = assign_digest(ctx, range, statement.hardware_policy_id);
    let device_authority_commitment =
        assign_digest(ctx, range, statement.device_authority_commitment);
    let profile = assign_digest(ctx, range, statement.hardware_profile_id);
    let policy_epoch = assign_uint_le(ctx, range, u128::from(statement.policy_epoch), 64);
    let platform_class = assign_uint_le(ctx, range, u128::from(statement.platform_class), 8);
    let capabilities = assign_uint_le(ctx, range, u128::from(statement.capability_mask), 16);
    let provider_authority = assign_digest(ctx, range, statement.provider_authority_commitment);
    let attestation = assign_digest(ctx, range, statement.platform_attestation_digest);
    let issuance = assign_digest(ctx, range, statement.credential_issuance_digest);
    let empty_effect = assign_digest(ctx, range, statement.canonical_empty_effect_digest);
    let profile_index =
        assign_uint_le(ctx, range, u128::from(statement.provider_profile_index), 16);
    let digest = hash(
        ctx,
        jobs,
        [
            constant_bytes(CREDENTIAL_STATEMENT_DOMAIN),
            vec![PastaSha256ByteV1::constant(0)],
            version.bytes.clone(),
            protocol_version.bytes.clone(),
            suite_id.to_vec(),
            release_id.to_vec(),
            network_id.to_vec(),
            asset_id.to_vec(),
            asset_incarnation.to_vec(),
            asset_scale.bytes.clone(),
            liability_pool_id.to_vec(),
            lane_id.to_vec(),
            epoch_generation.bytes.clone(),
            epoch_id.to_vec(),
            key_reference.to_vec(),
            device_public_key.clone(),
            policy_id.to_vec(),
            device_authority_commitment.to_vec(),
            profile.to_vec(),
            policy_epoch.bytes.clone(),
            platform_class.bytes,
            capabilities.bytes,
            provider_authority.to_vec(),
            attestation.to_vec(),
            issuance.to_vec(),
            empty_effect.to_vec(),
            profile_index.bytes,
        ]
        .concat(),
    )?;
    Ok(AssignedCredentialV1 {
        version,
        protocol_version,
        suite_id,
        release_id,
        network_id,
        asset_id,
        asset_incarnation,
        asset_scale,
        liability_pool_id,
        lane_id,
        epoch_generation,
        epoch_id,
        key_reference,
        device_public_key,
        policy_id,
        hardware_profile_id: profile,
        policy_epoch,
        device_authority_commitment,
        credential_issuance_digest: issuance,
        empty_effect,
        digest,
    })
}

#[cfg(feature = "zk-halo2-ipa")]
const GUARD_CREDENTIAL_EQUATION_TAG_V1: u32 = 3;
#[cfg(feature = "zk-halo2-ipa")]
const GUARD_PUBLIC_INSTANCE_COUNT_V1: usize = 6;
#[cfg(feature = "zk-halo2-ipa")]
pub(super) const GUARD_HISTORY_OFFSET_V1: usize = GUARD_PUBLIC_INSTANCE_COUNT_V1;
#[cfg(feature = "zk-halo2-ipa")]
pub(super) const GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1: usize =
    GUARD_HISTORY_OFFSET_V1 + accumulator_limb_count();
#[cfg(feature = "zk-halo2-ipa")]
pub(super) const GUARD_EQ_AUDIT_OFFSET_V1: usize = 2;
#[cfg(feature = "zk-halo2-ipa")]
pub(super) const GUARD_EP_AUDIT_OFFSET_V1: usize = 4;

/// Complete paired credential-proof material for one GuardBundle proof pair.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) struct KagemushaGuardBundleRecursiveWitnessV1<'a> {
    /// Complete normalized guard and device-authority witness.
    pub relation: KagemushaGuardBundleRelationWitnessV1,
    /// Eq credential ordinary-proof protocol baked into the GuardBundle key.
    pub eq_credential_protocol: &'a PlonkProtocol<EqAffine>,
    /// Ep credential ordinary-proof protocol baked into the GuardBundle key.
    pub ep_credential_protocol: &'a PlonkProtocol<EpAffine>,
    /// Eq predecessor-credential proof.
    pub eq_predecessor_credential_proof: &'a [u8],
    /// Eq successor-credential proof.
    pub eq_successor_credential_proof: &'a [u8],
    /// Eq BGH19 proof folding both accepted credential opening claims.
    pub eq_credential_fold_proof: &'a KagemushaEqFoldProofV1,
    /// Eq folded credential history exposed by the GuardBundle proof.
    pub eq_credential_history: &'a KagemushaEqAccumulatorV1,
    /// Ep predecessor-credential proof.
    pub ep_predecessor_credential_proof: &'a [u8],
    /// Ep successor-credential proof.
    pub ep_successor_credential_proof: &'a [u8],
    /// Ep BGH19 proof folding both accepted credential opening claims.
    pub ep_credential_fold_proof: &'a KagemushaEpFoldProofV1,
    /// Ep folded credential history exposed by the GuardBundle proof.
    pub ep_credential_history: &'a KagemushaEpAccumulatorV1,
    /// Common canonical Eq credential-equation audit.
    pub eq_credential_audit: DigestV1,
    /// Common canonical Ep credential-equation audit.
    pub ep_credential_audit: DigestV1,
}

/// Base, Table16, and reciprocal dense-MSM configuration for a GuardBundle parity.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone, Debug)]
pub(crate) struct KagemushaGuardBundleCircuitConfigV1<F: halo2_base::utils::ScalarField> {
    base: BaseConfig<F>,
    sha: PastaSha256ConfigV1,
    dense: PastaDenseMsmConfigV1,
}

/// Eq/Fp half of the hardware-authoritative GuardBundle relation.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub(crate) struct KagemushaGuardBundleEqCircuitV1 {
    builder: BaseCircuitBuilder<Fp>,
    sha_jobs: PastaSha256JobsV1<Fp>,
    dense_jobs: PastaDenseMsmJobsV1<EpAffine>,
}

/// Ep/Fq half of the hardware-authoritative GuardBundle relation.
#[cfg(feature = "zk-halo2-ipa")]
#[derive(Clone)]
pub(crate) struct KagemushaGuardBundleEpCircuitV1 {
    builder: BaseCircuitBuilder<Fq>,
    sha_jobs: PastaSha256JobsV1<Fq>,
    dense_jobs: PastaDenseMsmJobsV1<EqAffine>,
}

#[cfg(feature = "zk-halo2-ipa")]
macro_rules! impl_guard_bundle_circuit {
    ($circuit:ty, $field:ty, $opposite:ty, $label:literal) => {
        impl Circuit<$field> for $circuit {
            type Config = KagemushaGuardBundleCircuitConfigV1<$field>;
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
                KagemushaGuardBundleCircuitConfigV1 {
                    base,
                    sha: PastaSha256ConfigV1::configure(meta),
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

#[cfg(feature = "zk-halo2-ipa")]
impl_guard_bundle_circuit!(
    KagemushaGuardBundleEqCircuitV1,
    Fp,
    EpAffine,
    "Kagemusha Eq GuardBundle"
);
#[cfg(feature = "zk-halo2-ipa")]
impl_guard_bundle_circuit!(
    KagemushaGuardBundleEpCircuitV1,
    Fq,
    EqAffine,
    "Kagemusha Ep GuardBundle"
);

/// Build the two mutually audited GuardBundle circuits.
#[cfg(feature = "zk-halo2-ipa")]
pub(crate) fn build_kagemusha_guard_bundle_pair_v1(
    eq_svk: &IpaSuccinctVerifyingKey<EqAffine>,
    ep_svk: &IpaSuccinctVerifyingKey<EpAffine>,
    witness: KagemushaGuardBundleRecursiveWitnessV1<'_>,
) -> Result<
    (
        KagemushaGuardBundleEqCircuitV1,
        KagemushaGuardBundleEpCircuitV1,
        DigestV1,
        DigestV1,
    ),
    String,
> {
    witness.relation.validate()?;
    if witness.eq_credential_audit == [0; 32]
        || witness.ep_credential_audit == [0; 32]
        || witness.eq_credential_audit == witness.ep_credential_audit
        || crate::zk::kagemusha_v1_poseidon::decode::<Fp>(witness.eq_credential_audit).is_none()
        || crate::zk::kagemusha_v1_poseidon::decode::<Fq>(witness.ep_credential_audit).is_none()
    {
        return Err("Kagemusha GuardBundle credential audit is noncanonical".to_owned());
    }
    let (mut eq_builder, eq_sha, eq_output) = build_guard_scalar_half_v1::<EqAffine>(
        eq_svk,
        KagemushaPastaParityV1::Eq,
        witness.relation.clone(),
        witness.eq_credential_protocol,
        [
            witness.eq_predecessor_credential_proof,
            witness.eq_successor_credential_proof,
        ],
        witness.eq_credential_fold_proof.as_bytes(),
        witness.eq_credential_history.as_bytes(),
        witness.eq_credential_audit,
        witness.ep_credential_audit,
    )?;
    let (mut ep_builder, ep_sha, ep_output) = build_guard_scalar_half_v1::<EpAffine>(
        ep_svk,
        KagemushaPastaParityV1::Ep,
        witness.relation,
        witness.ep_credential_protocol,
        [
            witness.ep_predecessor_credential_proof,
            witness.ep_successor_credential_proof,
        ],
        witness.ep_credential_fold_proof.as_bytes(),
        witness.ep_credential_history.as_bytes(),
        witness.eq_credential_audit,
        witness.ep_credential_audit,
    )?;

    let eq_expected_ep_audit = guard_audit_cells(&eq_builder, GUARD_EP_AUDIT_OFFSET_V1)?;
    let mut eq_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_tagged_audit_v1::<EpAffine>(
        &mut eq_builder,
        &ep_output.audit,
        &ep_output.equation_selectors,
        &eq_expected_ep_audit,
        GUARD_CREDENTIAL_EQUATION_TAG_V1,
        &mut eq_dense,
    )?;
    let ep_expected_eq_audit = guard_audit_cells(&ep_builder, GUARD_EQ_AUDIT_OFFSET_V1)?;
    let mut ep_dense = PastaDenseMsmJobsV1::default();
    constrain_reciprocal_tagged_audit_v1::<EqAffine>(
        &mut ep_builder,
        &eq_output.audit,
        &eq_output.equation_selectors,
        &ep_expected_eq_audit,
        GUARD_CREDENTIAL_EQUATION_TAG_V1,
        &mut ep_dense,
    )?;

    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let usable_rows = (1_usize << KAGEMUSHA_HALO2_K_V1) - MINIMUM_UNUSABLE_ROWS;
    eq_sha.validate_capacity(usable_rows)?;
    ep_sha.validate_capacity(usable_rows)?;
    eq_dense.validate_capacity(usable_rows)?;
    ep_dense.validate_capacity(usable_rows)?;
    let eq_audit = super::composite::assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    let ep_audit = super::composite::assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    Ok((
        KagemushaGuardBundleEqCircuitV1 {
            builder: eq_builder,
            sha_jobs: eq_sha,
            dense_jobs: eq_dense,
        },
        KagemushaGuardBundleEpCircuitV1 {
            builder: ep_builder,
            sha_jobs: ep_sha,
            dense_jobs: ep_dense,
        },
        eq_audit,
        ep_audit,
    ))
}

#[cfg(feature = "zk-halo2-ipa")]
fn build_guard_scalar_half_v1<C>(
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    parity: KagemushaPastaParityV1,
    relation: KagemushaGuardBundleRelationWitnessV1,
    credential_protocol: &PlonkProtocol<C>,
    credential_proofs: [&[u8]; 2],
    credential_fold_proof: &[u8],
    credential_history: &[u8; super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    eq_audit: DigestV1,
    ep_audit: DigestV1,
) -> Result<
    (
        BaseCircuitBuilder<C::ScalarExt>,
        PastaSha256JobsV1<C::ScalarExt>,
        super::deferred_parent::KagemushaDeferredParentOutputV1<C>,
    ),
    String,
>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if credential_protocol.num_instance != [2] {
        return Err("Kagemusha credential proof has wrong fixed shape".to_owned());
    }
    let credential_proof_len = ordinary_ipa_proof_profile_v1(credential_protocol)
        .map_err(|error| format!("Kagemusha credential proof profile is invalid: {error}"))?
        .byte_len;
    if credential_proofs
        .iter()
        .any(|proof| proof.len() != credential_proof_len)
    {
        return Err("Kagemusha credential proof has wrong fixed shape".to_owned());
    }
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(usize::try_from(KAGEMUSHA_HALO2_K_V1).expect("k fits usize"))
        .use_lookup_bits(usize::try_from(KAGEMUSHA_HALO2_K_V1 - 1).expect("lookup bits fit usize"))
        .use_instance_columns(1);
    let mut sha_jobs = PastaSha256JobsV1::default();
    let assigned = constrain_guard_bundle_semantics_v1(&mut builder, &mut sha_jobs, &relation)?;
    let range = builder.range_chip();
    let guard_instances = digest_limbs_assigned(builder.main(0), &assigned.guard_digest);
    let eq_audit_assigned = assign_digest(builder.main(0), &range, eq_audit);
    let eq_audit_instances = digest_limbs_assigned(builder.main(0), &eq_audit_assigned);
    let ep_audit_assigned = assign_digest(builder.main(0), &range, ep_audit);
    let ep_audit_instances = digest_limbs_assigned(builder.main(0), &ep_audit_assigned);
    let credential_instances = assigned
        .credential_digests
        .map(|digest| digest_limbs_assigned(builder.main(0), &digest));
    let credential_history_instances = credential_history
        .chunks_exact(16)
        .map(|chunk| {
            let value = C::ScalarExt::from_u128(u128::from_le_bytes(
                chunk
                    .try_into()
                    .expect("Guard history limb is sixteen bytes"),
            ));
            let assigned = builder.main(0).load_witness(value);
            range.range_check(builder.main(0), assigned, 128);
            assigned
        })
        .collect::<Vec<_>>();
    if credential_history_instances.len() != accumulator_limb_count() {
        return Err("Kagemusha GuardBundle credential history is not fixed".to_owned());
    }
    builder.assigned_instances = vec![
        guard_instances
            .into_iter()
            .chain(eq_audit_instances)
            .chain(ep_audit_instances)
            .chain(credential_history_instances.iter().copied())
            .collect(),
    ];

    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    // The credential protocol is non-self-referential and loaded as circuit constants. Its exact
    // preprocessed points are therefore committed by the generated GuardBundle verifying key.
    let loaded_protocol = credential_protocol.loaded(&loader);
    let mut current_credentials: Vec<DeferredAccumulator<'_, C>> = Vec::with_capacity(2);
    for (instances, proof) in credential_instances.into_iter().zip(credential_proofs) {
        let instances = instances
            .into_iter()
            .map(|value| loader.scalar_from_assigned(value))
            .collect::<Vec<_>>();
        current_credentials.push(
            verify_ordinary_proof_v1(&loader, succinct_vk, &loaded_protocol, &[instances], proof)
                .map_err(|error| {
                format!("Kagemusha credential scalar verifier failed: {error:?}")
            })?,
        );
    }
    let folded_credentials = verify_fold(
        &loader,
        succinct_vk,
        &current_credentials,
        credential_fold_proof,
    )
    .map_err(|error| format!("Kagemusha credential fold failed: {error:?}"))?;
    bind_accumulator_limbs(&loader, &folded_credentials, &credential_history_instances)
        .map_err(|error| format!("Kagemusha credential history binding failed: {error:?}"))?;
    let output =
        finalize_tagged_deferred_audit_v1(&mut builder, loader, GUARD_CREDENTIAL_EQUATION_TAG_V1)
            .map_err(|error| format!("Kagemusha credential audit failed: {error:?}"))?;
    let expected_offset = match parity {
        KagemushaPastaParityV1::Eq => GUARD_EQ_AUDIT_OFFSET_V1,
        KagemushaPastaParityV1::Ep => GUARD_EP_AUDIT_OFFSET_V1,
    };
    let expected = guard_audit_cells(&builder, expected_offset)?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok((builder, sha_jobs, output))
}

#[cfg(feature = "zk-halo2-ipa")]
fn guard_audit_cells<F: KagemushaPoseidonFieldV1>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
) -> Result<[AssignedValue<F>; 2], String> {
    if builder
        .assigned_instances
        .first()
        .is_none_or(|column| column.len() != GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1)
    {
        return Err("Kagemusha GuardBundle public instance has wrong shape".to_owned());
    }
    builder.assigned_instances[0][offset..offset + 2]
        .try_into()
        .map_err(|_| "Kagemusha GuardBundle audit instance has wrong shape".to_owned())
}

#[derive(Clone)]
pub(super) struct AssignedUint<F: KagemushaPoseidonFieldV1> {
    pub(super) value: AssignedValue<F>,
    pub(super) bytes: Vec<PastaSha256ByteV1<F>>,
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

pub(super) fn assign_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[u8],
) -> Vec<PastaSha256ByteV1<F>> {
    bytes
        .iter()
        .copied()
        .map(|byte| {
            let assigned = ctx.load_witness(F::from(u64::from(byte)));
            PastaSha256ByteV1::range_checked(ctx, range, assigned)
        })
        .collect()
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

pub(super) fn constant_bytes<F: KagemushaPoseidonFieldV1>(
    bytes: &[u8],
) -> Vec<PastaSha256ByteV1<F>> {
    bytes
        .iter()
        .copied()
        .map(PastaSha256ByteV1::constant)
        .collect()
}

pub(super) fn hash<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    message: Vec<PastaSha256ByteV1<F>>,
) -> Result<[PastaSha256ByteV1<F>; 32], String> {
    let words = jobs.digest_constrained(ctx, &message)?;
    let mut bytes = Vec::with_capacity(32);
    for word in words {
        let bits =
            PastaSha256BitV1::decompose(ctx, &halo2_base::gates::GateChip::default(), word, 32);
        for offset in [24, 16, 8, 0] {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                &halo2_base::gates::GateChip::default(),
                &bits[offset..offset + 8],
            ));
        }
    }
    Ok(bytes.try_into().expect("SHA-256 digest width"))
}

pub(super) fn digest_limbs_assigned<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    digest: &[PastaSha256ByteV1<F>; 32],
) -> [AssignedValue<F>; 2] {
    let gate = halo2_base::gates::GateChip::default();
    core::array::from_fn(|half| {
        gate.inner_product(
            ctx,
            digest[half * 16..half * 16 + 16]
                .iter()
                .copied()
                .map(PastaSha256ByteV1::quantum_cell),
            (0..16).map(|index| QuantumCell::Constant(F::from_u128(1_u128 << (index * 8)))),
        )
    })
}

pub(super) fn bind_equal_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    _range: &RangeChip<F>,
    left: &[PastaSha256ByteV1<F>; 32],
    right: &[PastaSha256ByteV1<F>; 32],
) {
    for (left, right) in left.iter().copied().zip(right.iter().copied()) {
        let left = left
            .assigned()
            .expect("computed SHA-256 digest byte is assigned");
        let right = right
            .assigned()
            .expect("credential statement digest byte is assigned");
        ctx.constrain_equal(&left, &right);
    }
}

fn select_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    when_true: &[PastaSha256ByteV1<F>; 32],
    when_false: &[PastaSha256ByteV1<F>; 32],
) -> [PastaSha256ByteV1<F>; 32] {
    core::array::from_fn(|index| {
        let selected = range.gate().select(
            ctx,
            when_true[index].quantum_cell(),
            when_false[index].quantum_cell(),
            selector,
        );
        PastaSha256ByteV1::range_checked(ctx, range, selected)
    })
}

fn assert_nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
) {
    let is_zero = range.gate().is_zero(ctx, value);
    range.gate().assert_is_const(ctx, &is_zero, &F::ZERO);
}

pub(super) fn assert_digest_nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    digest: &[PastaSha256ByteV1<F>; 32],
) {
    let limbs = digest_limbs_assigned(ctx, digest);
    let low_zero = range.gate().is_zero(ctx, limbs[0]);
    let high_zero = range.gate().is_zero(ctx, limbs[1]);
    let both_zero = range.gate().and(ctx, low_zero, high_zero);
    range.gate().assert_is_const(ctx, &both_zero, &F::ZERO);
}

pub(super) fn assert_bytes_nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
) {
    let mut any = ctx.load_zero();
    for byte in bytes.iter().copied() {
        let byte = byte
            .assigned()
            .expect("provider authority secret byte is assigned");
        let is_zero = range.gate().is_zero(ctx, byte);
        let nonzero = range.gate().not(ctx, is_zero);
        any = range.gate().or(ctx, any, nonzero);
    }
    range.gate().assert_is_const(ctx, &any, &F::ONE);
}

fn assert_equal_value<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
) {
    ctx.constrain_equal(&left, &right);
}

fn assert_if_equal_value<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    left: AssignedValue<F>,
    right: AssignedValue<F>,
) {
    let difference = range.gate().sub(ctx, left, right);
    let selected = range.gate().mul(ctx, selector, difference);
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn assert_if_zero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    value: AssignedValue<F>,
) {
    let selected = range.gate().mul(ctx, selector, value);
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn assert_if_nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    value: AssignedValue<F>,
) {
    let is_zero = range.gate().is_zero(ctx, value);
    let invalid = range.gate().mul(ctx, selector, is_zero);
    range.gate().assert_is_const(ctx, &invalid, &F::ZERO);
}

fn assert_if_increment<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    before: AssignedValue<F>,
    after: AssignedValue<F>,
) {
    let incremented = range.gate().inc(ctx, before);
    assert_if_equal_value(ctx, range, selector, incremented, after);
}

fn assert_if_digest_zero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    digest: &[PastaSha256ByteV1<F>; 32],
) {
    for limb in digest_limbs_assigned(ctx, digest) {
        assert_if_zero(ctx, range, selector, limb);
    }
}

fn assert_if_digest_nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    digest: &[PastaSha256ByteV1<F>; 32],
) {
    let limbs = digest_limbs_assigned(ctx, digest);
    let low_zero = range.gate().is_zero(ctx, limbs[0]);
    let high_zero = range.gate().is_zero(ctx, limbs[1]);
    let both_zero = range.gate().and(ctx, low_zero, high_zero);
    let invalid = range.gate().mul(ctx, selector, both_zero);
    range.gate().assert_is_const(ctx, &invalid, &F::ZERO);
}

fn assert_if_digest_equal<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    left: &[PastaSha256ByteV1<F>; 32],
    right: &[PastaSha256ByteV1<F>; 32],
) {
    for (left, right) in digest_limbs_assigned(ctx, left)
        .into_iter()
        .zip(digest_limbs_assigned(ctx, right))
    {
        assert_if_equal_value(ctx, range, selector, left, right);
    }
}

fn assert_if_digest_different<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    selector: AssignedValue<F>,
    left: &[PastaSha256ByteV1<F>; 32],
    right: &[PastaSha256ByteV1<F>; 32],
) {
    let left = digest_limbs_assigned(ctx, left);
    let right = digest_limbs_assigned(ctx, right);
    let low_equal = range.gate().is_equal(ctx, left[0], right[0]);
    let high_equal = range.gate().is_equal(ctx, left[1], right[1]);
    let equal = range.gate().and(ctx, low_equal, high_equal);
    let invalid = range.gate().mul(ctx, selector, equal);
    range.gate().assert_is_const(ctx, &invalid, &F::ZERO);
}

fn authority_commitment(domain: &[u8], secret: DigestV1) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update([0]);
    hasher.update(secret);
    hasher.finalize().into()
}

fn policy_leaf(statement: &KagemushaPlatformCredentialStatementV1) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_LEAF_DOMAIN);
    hasher.update([0]);
    hasher.update(statement.hardware_profile_id);
    hasher.update([statement.platform_class]);
    hasher.update(statement.capability_mask.to_le_bytes());
    hasher.update(statement.provider_authority_commitment);
    hasher.update(statement.canonical_empty_effect_digest);
    hasher.finalize().into()
}

fn policy_node(left: DigestV1, right: DigestV1) -> DigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_NODE_DOMAIN);
    hasher.update([0]);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn credential_statement_preimage(statement: &KagemushaPlatformCredentialStatementV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(640);
    bytes.extend_from_slice(CREDENTIAL_STATEMENT_DOMAIN);
    bytes.push(0);
    bytes.extend_from_slice(&statement.version.to_le_bytes());
    bytes.extend_from_slice(&statement.protocol_version.to_le_bytes());
    bytes.extend_from_slice(&statement.suite_id);
    bytes.extend_from_slice(&statement.release_id);
    bytes.extend_from_slice(&statement.network_id);
    bytes.extend_from_slice(&statement.asset_id);
    bytes.extend_from_slice(statement.asset_incarnation.as_bytes());
    bytes.extend_from_slice(&statement.asset_scale.to_le_bytes());
    bytes.extend_from_slice(&statement.liability_pool_id);
    bytes.extend_from_slice(&statement.lane_id);
    bytes.extend_from_slice(&statement.hardware_epoch_generation.to_le_bytes());
    bytes.extend_from_slice(&statement.hardware_epoch_id);
    bytes.extend_from_slice(&statement.key_reference);
    bytes.extend_from_slice(statement.device_public_key.as_sec1_bytes());
    bytes.extend_from_slice(&statement.hardware_policy_id);
    bytes.extend_from_slice(&statement.device_authority_commitment);
    bytes.extend_from_slice(&statement.hardware_profile_id);
    bytes.extend_from_slice(&statement.policy_epoch.to_le_bytes());
    bytes.push(statement.platform_class);
    bytes.extend_from_slice(&statement.capability_mask.to_le_bytes());
    bytes.extend_from_slice(&statement.provider_authority_commitment);
    bytes.extend_from_slice(&statement.platform_attestation_digest);
    bytes.extend_from_slice(&statement.credential_issuance_digest);
    bytes.extend_from_slice(&statement.canonical_empty_effect_digest);
    bytes.extend_from_slice(&statement.provider_profile_index.to_le_bytes());
    bytes
}

fn blank_credential_statement() -> KagemushaPlatformCredentialStatementV1 {
    // A witness-free circuit keeps the exact same constraint and SHA job shape.  The values are
    // deliberately invalid; no proof can be created from this constructor.
    let key = KagemushaDevicePublicKeyV1::from_sec1_bytes(&[
        4, 107, 23, 209, 242, 225, 44, 66, 71, 248, 188, 230, 229, 99, 164, 64, 242, 119, 3, 125,
        129, 45, 235, 51, 160, 244, 161, 57, 69, 216, 152, 194, 150, 79, 227, 66, 226, 254, 26,
        127, 155, 142, 231, 235, 74, 124, 15, 158, 22, 43, 206, 51, 87, 107, 49, 94, 206, 203, 182,
        64, 104, 55, 191, 81, 245,
    ])
    .expect("P-256 generator is canonical");
    KagemushaPlatformCredentialStatementV1 {
        version: 0,
        protocol_version: 0,
        suite_id: [0; 32],
        release_id: [0; 32],
        network_id: [0; 32],
        asset_id: [0; 32],
        asset_incarnation: placeholder_asset_incarnation_v1(),
        asset_scale: 0,
        liability_pool_id: [0; 32],
        lane_id: [0; 32],
        hardware_epoch_generation: 0,
        hardware_epoch_id: [0; 32],
        key_reference: [0; 32],
        device_public_key: key,
        hardware_policy_id: [0; 32],
        device_authority_commitment: [0; 32],
        hardware_profile_id: [0; 32],
        policy_epoch: 0,
        platform_class: 0,
        capability_mask: 0,
        provider_authority_commitment: [0; 32],
        platform_attestation_digest: [0; 32],
        credential_issuance_digest: [0; 32],
        canonical_empty_effect_digest: [0; 32],
        provider_profile_index: 0,
    }
}

fn placeholder_asset_incarnation_v1() -> AxtAssetIncarnationV1 {
    let mut bytes = [0x42_u8; 32];
    bytes[31] |= 1;
    AxtAssetIncarnationV1::try_from_bytes(bytes)
        .expect("fixed placeholder asset incarnation is canonical")
}

/// Derive the device proof-key commitment used by a provider credential.
#[must_use]
pub(crate) fn device_authority_commitment_v1(secret: DigestV1) -> DigestV1 {
    authority_commitment(DEVICE_AUTHORITY_DOMAIN, secret)
}

#[cfg(test)]
mod tests {
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };

    use super::*;
    use crate::zk::kagemusha_v1_poseidon::digest_limbs;

    fn credential_witness() -> KagemushaPlatformCredentialRelationWitnessV1 {
        let device_public_key = blank_credential_statement().device_public_key;
        let provider_authority_secret = [0x51; 32];
        let policy_siblings = core::array::from_fn(|depth| {
            let mut sibling = [0_u8; 32];
            sibling.fill(u8::try_from(depth + 1).expect("policy depth fits u8"));
            sibling
        });
        let mut statement = KagemushaPlatformCredentialStatementV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            protocol_version: KAGEMUSHA_WIRE_VERSION_V1,
            suite_id: [0x31; 32],
            release_id: [1; 32],
            network_id: [2; 32],
            asset_id: [3; 32],
            asset_incarnation: placeholder_asset_incarnation_v1(),
            asset_scale: 18,
            liability_pool_id: [4; 32],
            lane_id: [5; 32],
            hardware_epoch_generation: 7,
            hardware_epoch_id: [6; 32],
            key_reference: kagemusha_device_key_reference_v1(&device_public_key),
            device_public_key,
            hardware_policy_id: [1; 32],
            device_authority_commitment: device_authority_commitment_v1([0x41; 32]),
            hardware_profile_id: [8; 32],
            policy_epoch: 1,
            platform_class: 3,
            capability_mask: KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
            provider_authority_commitment: authority_commitment(
                PROVIDER_AUTHORITY_DOMAIN,
                provider_authority_secret,
            ),
            platform_attestation_digest: [9; 32],
            credential_issuance_digest: [10; 32],
            canonical_empty_effect_digest: [11; 32],
            provider_profile_index: 0xa531,
        };
        let provisional = KagemushaPlatformCredentialRelationWitnessV1 {
            statement,
            provider_authority_secret,
            policy_siblings,
        };
        statement.hardware_policy_id = provisional.policy_root();
        KagemushaPlatformCredentialRelationWitnessV1 {
            statement,
            provider_authority_secret,
            policy_siblings,
        }
    }

    fn guard_witness() -> KagemushaGuardBundleRelationWitnessV1 {
        let credential = credential_witness().statement;
        KagemushaGuardBundleRelationWitnessV1 {
            statement: KagemushaNormalizedGuardStatementV1 {
                version: KAGEMUSHA_WIRE_VERSION_V1,
                protocol_version: credential.protocol_version,
                predecessor_suite_id: credential.suite_id,
                predecessor_vk_digest: [0x32; 32],
                successor_suite_id: credential.suite_id,
                successor_vk_digest: [0x32; 32],
                operation: KagemushaOperationV1::SendSplit,
                amount: 7,
                peer_credit_id: [20; 32],
                recipient_encryption_key_binding: [21; 32],
                mint_finality_proof_binding_digest: [0; 32],
                predecessor_release_id: credential.release_id,
                release_id: credential.release_id,
                network_id: credential.network_id,
                asset_id: credential.asset_id,
                asset_incarnation: credential.asset_incarnation,
                asset_scale: credential.asset_scale,
                liability_pool_id: credential.liability_pool_id,
                hardware_profile_id: credential.hardware_profile_id,
                policy_epoch: credential.policy_epoch,
                lane_id: credential.lane_id,
                predecessor_state_commitment: [12; 32],
                successor_state_commitment: [13; 32],
                predecessor_state_nonce_commitment: [14; 32],
                successor_state_nonce_commitment: [15; 32],
                predecessor_logical_sequence: 10,
                successor_logical_sequence: 11,
                predecessor_hardware_epoch_generation: credential.hardware_epoch_generation,
                successor_hardware_epoch_generation: credential.hardware_epoch_generation,
                predecessor_hardware_epoch_id: credential.hardware_epoch_id,
                successor_hardware_epoch_id: credential.hardware_epoch_id,
                predecessor_key_reference: credential.key_reference,
                successor_key_reference: credential.key_reference,
                predecessor_hardware_policy_id: credential.hardware_policy_id,
                successor_hardware_policy_id: credential.hardware_policy_id,
                journal_revision_before: 20,
                journal_revision_after: 21,
                lifecycle_binding_digest: [0x33; 32],
                prepared_transition_binding_digest: [0x34; 32],
                terminal_commit_binding_digest: [0; 32],
                sender_one_time_authorization_digest: [0; 32],
                receive_credit_binding_digest: [0; 32],
                transition_intent_digest: [16; 32],
                transition_effect_digest: [17; 32],
                recovery_record_digest: [18; 32],
                durable_inbox_effect_digest: credential.canonical_empty_effect_digest,
                durable_outbox_effect_digest: [19; 32],
            },
            canonical_empty_effect_digest: credential.canonical_empty_effect_digest,
            predecessor_credential: credential,
            successor_credential: credential,
            predecessor_device_authority_secret: [0x41; 32],
            successor_device_authority_secret: [0x41; 32],
        }
    }

    #[test]
    fn credential_authority_policy_and_key_are_jointly_bound() {
        let witness = credential_witness();
        witness.validate().expect("valid credential witness");

        let mut wrong_secret = witness.clone();
        wrong_secret.provider_authority_secret[0] ^= 1;
        assert!(wrong_secret.validate().is_err());

        let mut wrong_policy = witness.clone();
        wrong_policy.policy_siblings[4][0] ^= 1;
        assert!(wrong_policy.validate().is_err());

        let mut wrong_key_reference = witness.clone();
        wrong_key_reference.statement.key_reference[0] ^= 1;
        assert!(wrong_key_reference.validate().is_err());

        for bit in 0..16 {
            let mut missing_capability = witness.clone();
            missing_capability.statement.capability_mask &= !(1_u16 << bit);
            assert!(
                missing_capability.validate().is_err(),
                "credential omitted capability bit {bit}"
            );
        }
    }

    #[test]
    fn credential_public_digest_is_field_neutral() {
        let witness = credential_witness();
        let digest = witness.statement.canonical_digest();
        let expected_fp = digest_limbs::<Fp>(digest).to_vec();
        let expected_fq = digest_limbs::<Fq>(digest).to_vec();
        assert_eq!(expected_fp.len(), 2);
        assert_eq!(expected_fq.len(), 2);
        assert!(KagemushaPlatformCredentialRelationCircuitV1::<Fp>::new(witness.clone()).is_err());
        assert!(KagemushaPlatformCredentialRelationCircuitV1::<Fq>::new(witness).is_err());
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn platform_credential_sha_queue_has_exact_job_and_block_profile() {
        let witness = credential_witness();
        let eq = platform_credential_sha_messages_v1::<Fp>(&witness).expect("Eq SHA queue");
        let ep = platform_credential_sha_messages_v1::<Fq>(&witness).expect("Ep SHA queue");
        let expected_lengths = [105, 76, 139]
            .into_iter()
            .chain(core::iter::repeat_n(
                104,
                KAGEMUSHA_HARDWARE_POLICY_TREE_DEPTH_V1,
            ))
            .chain([663])
            .collect::<Vec<_>>();

        assert_eq!(
            eq.iter().map(Vec::len).collect::<Vec<_>>(),
            expected_lengths
        );
        assert_eq!(
            ep.iter().map(Vec::len).collect::<Vec<_>>(),
            expected_lengths
        );
        let blocks = |messages: &[Vec<u8>]| {
            messages
                .iter()
                .map(|message| (message.len() + 9).div_ceil(64))
                .sum::<usize>()
        };
        assert_eq!((eq.len(), blocks(&eq)), (20, 50));
        assert_eq!((ep.len(), blocks(&ep)), (20, 50));
    }

    #[cfg(feature = "zk-halo2-ipa")]
    #[test]
    fn platform_credential_pair_audit_binding_covers_the_claim_carrier_tail() {
        let credential_and_release_digests = 2 + 2;
        let four_hash_protocol_digests = 4 * 2;
        let paired_histories = 2 * accumulator_limb_count();
        assert_eq!(
            PLATFORM_CREDENTIAL_BASE_BOUND_U128_COUNT_V1,
            credential_and_release_digests + four_hash_protocol_digests + paired_histories
        );
        let claim_carrier_binding_tail =
            super::super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
                - super::super::mint_hash_claim_fold::KAGEMUSHA_MINT_HASH_CLAIM_PUBLIC_INSTANCE_COUNT_V1;
        assert_eq!(claim_carrier_binding_tail, 14);
        assert_eq!(PLATFORM_CREDENTIAL_BASE_BOUND_U128_COUNT_V1, 80);
        assert_eq!(PLATFORM_CREDENTIAL_PAIR_BOUND_U128_COUNT_V1, 94);
        assert_eq!(
            PLATFORM_CREDENTIAL_PAIR_BOUND_U128_COUNT_V1,
            PLATFORM_CREDENTIAL_BASE_BOUND_U128_COUNT_V1 + claim_carrier_binding_tail
        );
        assert_eq!(PLATFORM_CREDENTIAL_EQ_BOUND_U128_COUNT_V1, 96);
        assert_eq!(
            PLATFORM_CREDENTIAL_EQ_BOUND_U128_COUNT_V1,
            PLATFORM_CREDENTIAL_PAIR_BOUND_U128_COUNT_V1 + 2
        );
    }

    #[test]
    fn credential_relation_without_completed_hash_claim_fails_closed() {
        let witness = credential_witness();
        for result in [
            KagemushaPlatformCredentialRelationCircuitV1::<Fp>::new(witness.clone()).map(|_| ()),
            KagemushaPlatformCredentialRelationCircuitV1::<Fq>::new(witness).map(|_| ()),
        ] {
            assert!(
                result
                    .expect_err("relation-only PlatformCredential construction must fail closed")
                    .contains("completed paired SHA claim")
            );
        }
        assert_eq!(platform_credential_public_instance::CREDENTIAL_LO, 0);
        assert_eq!(platform_credential_public_instance::EQ_AUDIT_LO, 2);
        assert_eq!(platform_credential_public_instance::EP_AUDIT_LO, 4);
        assert_eq!(platform_credential_public_instance::HISTORY_START, 6);
        assert_eq!(KAGEMUSHA_PLATFORM_CREDENTIAL_PUBLIC_INSTANCE_COUNT_V1, 40);
    }

    #[test]
    fn guard_bundle_enforces_exact_next_effects_and_device_authority() {
        let witness = guard_witness();
        witness.validate().expect("valid GuardBundle witness");

        let mut skipped_sequence = witness.clone();
        skipped_sequence.statement.successor_logical_sequence += 1;
        assert!(skipped_sequence.validate().is_err());

        let mut missing_outbox = witness.clone();
        missing_outbox.statement.durable_outbox_effect_digest =
            missing_outbox.canonical_empty_effect_digest;
        assert!(missing_outbox.validate().is_err());

        let mut wrong_device_secret = witness;
        wrong_device_secret.predecessor_device_authority_secret[0] ^= 1;
        assert!(wrong_device_secret.validate().is_err());
    }

    #[test]
    fn guard_rejects_release_transition_substitution() {
        let mut witness = guard_witness();
        witness.statement.predecessor_release_id[0] ^= 1;
        witness.predecessor_credential.release_id = witness.statement.predecessor_release_id;
        assert!(witness.validate().is_err());
    }

    #[test]
    fn guard_digest_is_fixed_layout_and_field_sensitive() {
        let witness = guard_witness();
        assert_eq!(
            normalized_guard_statement_payload_v1(&witness.statement).len(),
            1_249
        );
        let digest = witness.statement_digest();
        let mut changed = witness;
        changed.statement.policy_epoch += 1;
        assert_ne!(digest, changed.statement_digest());
    }
}
