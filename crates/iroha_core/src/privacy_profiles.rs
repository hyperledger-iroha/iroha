//! Deterministic manifest for native first-release privacy engines.
//!
//! Governance does not get to turn arbitrary non-zero digests into executable
//! consensus code. Every activatable protocol must have one compiled profile
//! whose parameter, verifier, structurally derived statement-schema,
//! engine-manifest, and limit
//! bindings exactly match the proposed activation record. A protocol whose
//! complete verifier is not compiled is rejected before it enters world state.

use std::collections::BTreeMap;

#[cfg(feature = "zk-stark")]
use iroha_data_model::privacy::ZkAcePqAuthorizationStatementV1;
use iroha_data_model::privacy::{
    ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1, ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
    AnonymousPgcActivationLimitsV1, AnonymousPgcKOutOfNStatementV1,
    IrohaJindoPolynomialCommitmentStatementV1, IrohaZkAmsStatementV1, JindoActivationLimitsV1,
    ORCHARD_MAX_ACTIONS_V1 as ORCHARD_MODEL_MAX_ACTIONS_V1, OrchardActivationLimitsV1,
    OrchardHalo2ActionsStatementV1, PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
    PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1, PrivacyAssuranceV1, PrivacyCapabilityRowV1,
    PrivacyCapabilitySnapshotV1, PrivacyCapabilitySnapshotValidationErrorV1,
    PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1,
    PrivacyCompiledProfileUnavailableReasonV1, PrivacyCompiledStatementSchemaErrorV1,
    PrivacyConsensusPolicyV1, PrivacyEngineIdV1, PrivacyEngineManifestDigestV1,
    PrivacyOrchardPoolBootstrapV1, PrivacyParameterDigestV1, PrivacyParameterIdV1,
    PrivacyPgcAccountBootstrapV1, PrivacyProofSystemIdV1, PrivacyProtocolActivationLimitsV1,
    PrivacyProtocolActivationRecordV1, PrivacyProtocolIdV1, PrivacyProtocolLifecycleV1,
    PrivacyStatementSchemaDigestV1, PrivacyVerifierDigestV1,
    TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1, TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1,
    TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1, VERANGE_HARD_MAX_AGGREGATION_COUNT_V1,
    VeRangeActivationLimitsV1, VeRangeTransparentRangeStatementV1,
    VegaExistingCredentialStatementV1, ZK_AMS_MAX_BATCH_SIZE_V1, ZK_AMS_MAX_RING_SIZE_V1,
    ZK_AMS_RING_SIZES_V1 as ZK_AMS_MODEL_RING_SIZES_V1, ZkAmsActivationLimitsV1,
};
use iroha_schema::{FloatMode, IntMode, IntoSchema, MetaMapEntry, Metadata};
use iroha_zkp_halo2::vega::{
    MAX_VEGA_PROOF_BYTES_V1, MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1,
    VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1, VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
    ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1, ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1,
    vega_mdl_compiled_profile_digest_v1, zk_ams_admission_relation_dimensions_v1,
    zk_ams_compiled_profile_digest_v1, zk_ams_t256_generator_digest_v1,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(feature = "zk-stark")]
use crate::privacy_engines::zk_ace::{
    ZK_ACE_AIR_RELATION_SCHEMA_V1, ZK_ACE_AUTHORIZATION_PROJECTION_V1,
    ZK_ACE_POSEIDON_MANIFEST_SHA256_V1, ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1,
    ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1, ZK_ACE_PROOF_WIRE_V1, ZK_ACE_SOURCE_PROFILE_V1,
    zk_ace_compiled_profile_digest_v1, zk_ace_stark_profile_descriptor_v1,
};
use crate::privacy_engines::{
    anonymous_pgc::{
        ANONYMOUS_PGC_FULL_ENGINE_AVAILABLE_V1, AnonymousPgcParametersV1, PGC_SOURCE_PROFILE_V1,
        bootstrap::{
            MAX_PGC_BOOTSTRAP_NAMESPACE_BYTES_V1, MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,
            PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1, PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
            PGC_BOOTSTRAP_MAX_AGGREGATE_BALANCE_V1, PGC_BOOTSTRAP_PROOF_VERSION_V1,
            PGC_BOOTSTRAP_SUITE_V1, PGC_BOOTSTRAP_TABLE_DIGEST_DOMAIN_V1,
            PGC_BOOTSTRAP_TABLE_DIGEST_SCHEMA_V1,
        },
        payment::{
            MAX_PGC_PAYMENT_PROOF_BYTES_V1, PGC_PAYMENT_ANONYMITY_SET_SIZES_V1,
            PGC_PAYMENT_MAX_RECIPIENTS_V1, PGC_PAYMENT_POOL_INVARIANT_SCHEMA_V1,
            PGC_PAYMENT_PROOF_VERSION_V1, PGC_PAYMENT_SUITE_V1,
        },
    },
    jindo::{
        JINDO_MAX_BATCH_SIZE_V1, JINDO_NATIVE_PROOF_BYTES_V1, JINDO_PARAMETER_MANIFEST_V1,
        JINDO_SOURCE_PROFILE_V1, JINDO_SUITE_V1, jindo_crs_digest_v1,
    },
    orchard::{
        ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1,
        ORCHARD_MAX_ACTIONS_V1 as ORCHARD_ENGINE_MAX_ACTIONS_V1,
        ORCHARD_POST_NU6_3_CIRCUIT_DESCRIPTION_SHA256_V1, ORCHARD_UPSTREAM_CRATE_VERSION_V1,
        ORCHARD_UPSTREAM_REVISION_V1, orchard_authorization_wire_size_v1, orchard_empty_root_v1,
    },
    verange::{
        VERANGE_TYPE1_PROOF_VERSION_V1, VERANGE_TYPE1_SOURCE_PROFILE_V1, VERANGE_TYPE1_SUITE_V1,
        VeRangeBitLengthV1, VeRangeParametersV1,
    },
    zk_ams::{
        MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1, MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1,
        MAX_ZK_AMS_LSAG_PROOF_BYTES_V1, ZK_AMS_ADMISSION_POSSESSION_PROOF_VERSION_V1,
        ZK_AMS_ADMISSION_POSSESSION_SUITE_V1, ZK_AMS_LSAG_PROOF_VERSION_V1, ZK_AMS_LSAG_SUITE_V1,
        ZK_AMS_RING_SIZES_V1, ZK_AMS_SOURCE_PROFILE_V1, zk_ams_generator_digest_v1,
    },
};

const PROFILE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.digest.v1";
const PARAMETER_ID_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.parameter-id.v1";
const PARAMETER_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.parameter-digest.v1";
const VERIFIER_DIGEST_DOMAIN_V1: &[u8] = b"iroha.privacy.compiled-profile.verifier-digest.v1";
const CANONICAL_SCHEMA_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.compiled-profile.canonical-structural-schema.v1";
const ENGINE_MANIFEST_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.privacy.compiled-profile.engine-manifest-digest.v1";
const VERANGE_PROTOCOL_LABEL_V1: &[u8] = b"verange-transparent-range-v1";
const VERANGE_PARAMETER_SET_LABEL_V1: &[u8] = b"p256-type1-bits32-bits64-v1";
const VERANGE_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:ordered-independent-type1-batch:strict-exact:v1";
const VERANGE_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:eprint-2025-528:figure-1:type-1:v1";
const ANONYMOUS_PGC_PROTOCOL_LABEL_V1: &[u8] = b"anonymous-pgc-k-out-of-n-v1";
const ANONYMOUS_PGC_PARAMETER_SET_LABEL_V1: &[u8] = b"p256-twisted-elgamal-rfc9380-g-h-v1";
const ANONYMOUS_PGC_PAYMENT_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:anonymous-pgc-payment:strict-exact:v1";
const ANONYMOUS_PGC_BOOTSTRAP_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:anonymous-pgc-bootstrap:strict-exact:v1";
const ANONYMOUS_PGC_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:eprint-2025-884:sections-3-4-6:linear-legality-and-bounded-bootstrap:v1";
const JINDO_PROTOCOL_LABEL_V1: &[u8] = b"iroha-jindo-polynomial-commitment-v0";
const JINDO_PARAMETER_SET_LABEL_V1: &[u8] = b"jindo-univariate-batch4-degree256-transparent-v1";
const JINDO_PROOF_WIRE_LABEL_V1: &[u8] = b"IJP1:fixed-rns-le:30-outer:66-inner:strict-exact:v1";
const JINDO_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:eprint-2026-044:figures-1-5:univariate:v1";
const ORCHARD_PROTOCOL_LABEL_V1: &[u8] = b"orchard-halo2-actions-v1";
const ORCHARD_PARAMETER_SET_LABEL_V1: &[u8] = b"orchard-v3-post-nu6-3-halo2-ipa-pasta-v1";
const ORCHARD_PROOF_WIRE_LABEL_V1: &[u8] =
    b"ORC1:u8-actions:halo2-proof:ordered-redpallas-spend-signatures:binding-signature";
const ORCHARD_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:zcash-orchard-v3:post-nu6-3:first-release-no-legacy:v1";
const ORCHARD_FRONTIER_SCHEMA_V1: &[u8] =
    b"tree_size:u64|leaf:option<cmx32>|ommers:ordered<merkle_hash32>|root:32|depth:32";
const ORCHARD_VERIFIED_EFFECT_SCHEMA_V1: &[u8] = b"namespace:norito|bootstrap_digest:32|asset_definition_id:norito|reserve_account:norito|anchor:32|anchor_epoch:u64|current_root:32|current_epoch:u64|successor_frontier|ordered_nullifiers[32]|value_balance:direction+u128|fee:u128|expiry_height:u64";
const VEGA_PARAMETER_SET_LABEL_V1: &[u8] =
    b"vega-figure9-mdl-age-neutron-nova-spartan-hyrax-t256-v1";
const VEGA_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:vega-figure9-masked-relaxed-fold-spartan-hyrax:strict-exact:v1";
const VEGA_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:microsoft-vega-prover:c0ee259053cd12eaf43ed71b5cde375452b3ee4d:figure9:v1";
const ZK_AMS_PROTOCOL_LABEL_V1: &[u8] = b"iroha-zk-ams-v1";
const ZK_AMS_PARAMETER_SET_LABEL_V1: &[u8] =
    b"zk-ams-v2-masked-relaxed-spartan-t256-lsag-ristretto255-v1";
const ZK_AMS_BATCH_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:zk-ams-batch-admission:masked-relaxed-spartan+ordered-possession:strict-exact:v1";
const ZK_AMS_PROVISION_PROOF_WIRE_LABEL_V1: &[u8] =
    b"norito:zk-ams-provision-account:lsag-ristretto255:strict-exact:v1";
const ZK_AMS_IMPLEMENTATION_PROVENANCE_V1: &[u8] =
    b"iroha-native-rust:clean-room:arxiv-2602.16130v2:algorithms-1-4:appendices-a-c:closed-phase-v:v1";
const ZK_AMS_BATCH_EFFECT_SCHEMA_V1: &[u8] = b"issuer_id:32|registry_id:32|prior_root:32|prior_epoch:u64|next_root:32|next_epoch:u64|ordered_anchors[seed_key:32,link_tag:32]";
const ZK_AMS_PROVISION_EFFECT_SCHEMA_V1: &[u8] = b"issuer_id:32|registry_id:32|current_root:32|current_epoch:u64|ring[seed_key:32]|account_id:norito|key_image:32";
#[cfg(feature = "zk-stark")]
const ZK_ACE_PROTOCOL_LABEL_V1: &[u8] = b"zk-ace-pq-authorization-v0";
#[cfg(feature = "zk-stark")]
const ZK_ACE_PARAMETER_SET_LABEL_V1: &[u8] =
    b"goldilocks-poseidon2-trace4096-blowup16-mask255-three-lane-fri32-v1";
const ANONYMOUS_PGC_ACCOUNT_ROOT_SCHEMA_V1: &[u8] = b"namespace_len:u64le|namespace:norito|epoch:u64le|total_supply:u32le|account_count:u32le|accounts[public_key:33,cipher_left:33,cipher_right:33]";
const ANONYMOUS_PGC_VERIFIED_EFFECT_SCHEMA_V1: &[u8] = b"namespace:norito|total_supply:u32|current_root:32|current_epoch:u64|next_root:32|next_epoch:u64|complete_accounts[public_key:33,cipher_left:33,cipher_right:33]";

/// Exact compiled bindings for one native privacy protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompiledPrivacyProfileV1 {
    /// Closed protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Closed proof-system identity.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Closed native-engine identity.
    pub engine_id: PrivacyEngineIdV1,
    /// Deterministic identifier of the compiled parameter set.
    pub parameter_id: PrivacyParameterIdV1,
    /// Digest of the exact compiled parameters.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Digest of the exact verifier relation and proof wire.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Digest of the exact public-statement schema.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Digest of the complete compiled engine manifest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Exact protocol-specific limits compiled into the verifier.
    pub protocol_limits: PrivacyProtocolActivationLimitsV1,
}

impl CompiledPrivacyProfileV1 {
    /// Build the only valid activation record for this compiled profile.
    #[must_use]
    pub fn activation_record(
        self,
        lifecycle: PrivacyProtocolLifecycleV1,
    ) -> PrivacyProtocolActivationRecordV1 {
        PrivacyProtocolActivationRecordV1 {
            protocol_id: self.protocol_id,
            proof_system_id: self.proof_system_id,
            engine_id: self.engine_id,
            parameter_id: self.parameter_id,
            parameter_digest: self.parameter_digest,
            verifier_digest: self.verifier_digest,
            statement_schema_digest: self.statement_schema_digest,
            engine_manifest_digest: self.engine_manifest_digest,
            lifecycle,
            protocol_limits: self.protocol_limits,
            pending_protocol_limits_tightening: None,
            assurance: PrivacyAssuranceV1::Experimental,
        }
    }
}

impl From<CompiledPrivacyProfileV1> for PrivacyCompiledProfileSnapshotV1 {
    fn from(profile: CompiledPrivacyProfileV1) -> Self {
        Self {
            protocol_id: profile.protocol_id,
            proof_system_id: profile.proof_system_id,
            engine_id: profile.engine_id,
            parameter_id: profile.parameter_id,
            parameter_digest: profile.parameter_digest,
            verifier_digest: profile.verifier_digest,
            statement_schema_digest: profile.statement_schema_digest,
            engine_manifest_digest: profile.engine_manifest_digest,
            protocol_limits: profile.protocol_limits,
        }
    }
}

/// Build the exact local compiled-profile result for one public snapshot row.
#[must_use]
pub fn compiled_privacy_profile_snapshot_result_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> PrivacyCompiledProfileResultV1 {
    match compiled_privacy_profile_v1(protocol_id) {
        Ok(profile) => PrivacyCompiledProfileResultV1::Available(profile.into()),
        Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { .. }) => {
            PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
            )
        }
        Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { .. }) => {
            PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::ProfileInitializationFailed,
            )
        }
        Err(CompiledPrivacyProfileErrorV1::StatementSchemaInvalid { source, .. }) => {
            let source = match source {
                CanonicalSchemaDigestErrorV1::ConflictingStableTypeId => {
                    PrivacyCompiledStatementSchemaErrorV1::ConflictingStableTypeId
                }
                CanonicalSchemaDigestErrorV1::MissingTypeReference => {
                    PrivacyCompiledStatementSchemaErrorV1::MissingTypeReference
                }
            };
            PrivacyCompiledProfileResultV1::Unavailable(
                PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(source),
            )
        }
    }
}

/// Build and validate an authoritative committed privacy capability snapshot.
///
/// `activation_for` must read from the same immutable committed world view as
/// `consensus_policy` and `committed_height`. The closure is invoked exactly
/// once for every protocol in canonical discriminant order.
///
/// # Errors
///
/// Returns a deterministic structural or height-consistency error if the
/// committed state cannot be represented by the closed snapshot contract.
pub fn committed_privacy_capability_snapshot_v1(
    committed_height: u64,
    consensus_policy: PrivacyConsensusPolicyV1,
    mut activation_for: impl FnMut(PrivacyProtocolIdV1) -> Option<PrivacyProtocolActivationRecordV1>,
) -> Result<PrivacyCapabilitySnapshotV1, PrivacyCapabilitySnapshotValidationErrorV1> {
    let protocols = PrivacyProtocolIdV1::ALL
        .into_iter()
        .map(|protocol_id| PrivacyCapabilityRowV1 {
            protocol_id,
            compiled_profile: compiled_privacy_profile_snapshot_result_v1(protocol_id),
            activation: activation_for(protocol_id),
        })
        .collect();
    let snapshot = PrivacyCapabilitySnapshotV1 {
        version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
        committed_height,
        consensus_policy,
        protocols,
    };
    snapshot.validate()?;
    Ok(snapshot)
}

/// Return the exact compiled profile for an executable native verifier.
///
/// # Errors
///
/// Returns [`CompiledPrivacyProfileErrorV1::EngineUnavailable`] for a protocol
/// whose complete end-to-end verifier is not compiled, or
/// [`CompiledPrivacyProfileErrorV1::ProfileInitializationFailed`] if fixed
/// transparent parameters cannot be derived, or
/// [`CompiledPrivacyProfileErrorV1::StatementSchemaInvalid`] when the emitted
/// schema is ambiguous or internally unresolved.
pub fn compiled_privacy_profile_v1(
    protocol_id: PrivacyProtocolIdV1,
) -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    match protocol_id {
        #[cfg(feature = "zk-stark")]
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => compiled_zk_ace_profile_v1(),
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => compiled_anonymous_pgc_profile_v1(),
        PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => compiled_verange_profile_v1(),
        PrivacyProtocolIdV1::IrohaZkAmsV1 => compiled_zk_ams_profile_v1(),
        PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 => compiled_jindo_profile_v1(),
        PrivacyProtocolIdV1::VegaExistingCredentialZkV0 => compiled_vega_profile_v1(),
        PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => compiled_orchard_profile_v1(),
        // TODO(privacy-native-engines): remove each fail-closed branch only
        // after its complete canonical verifier, effect derivation, KATs, and
        // adversarial tests are compiled into this manifest.
        #[cfg(not(feature = "zk-stark"))]
        PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => {
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        }
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V0
        | PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
        | PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
        | PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
        | PrivacyProtocolIdV1::PqMaspStarkV0 => {
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        }
    }
}

fn compiled_orchard_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1>
{
    let protocol_id = PrivacyProtocolIdV1::OrchardHalo2ActionsV1;
    if ORCHARD_ENGINE_MAX_ACTIONS_V1
        != usize::try_from(ORCHARD_MODEL_MAX_ACTIONS_V1)
            .expect("compiled Orchard action count fits usize")
        || ORCHARD_MODEL_MAX_ACTIONS_V1 > TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
        || ORCHARD_MODEL_MAX_ACTIONS_V1 > TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let one_action_wire = orchard_authorization_wire_size_v1(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let two_action_wire = orchard_authorization_wire_size_v1(ORCHARD_ENGINE_MAX_ACTIONS_V1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    if one_action_wire == 0
        || one_action_wire >= two_action_wire
        || two_action_wire > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let action_count = ORCHARD_MODEL_MAX_ACTIONS_V1.to_be_bytes();
    let one_action_wire = one_action_wire.to_be_bytes();
    let two_action_wire = two_action_wire.to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let empty_root = orchard_empty_root_v1();
    let statement_schema_digest = canonical_schema_digest_v1::<OrchardHalo2ActionsStatementV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let bootstrap_schema_digest = canonical_schema_digest_v1::<PrivacyOrchardPoolBootstrapV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ORCHARD_PROTOCOL_LABEL_V1,
            ORCHARD_PARAMETER_SET_LABEL_V1,
            ORCHARD_UPSTREAM_CRATE_VERSION_V1.as_bytes(),
            ORCHARD_UPSTREAM_REVISION_V1.as_bytes(),
            ORCHARD_POST_NU6_3_CIRCUIT_DESCRIPTION_SHA256_V1.as_bytes(),
            &empty_root,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            ORCHARD_PROTOCOL_LABEL_V1,
            ORCHARD_PARAMETER_SET_LABEL_V1,
            ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1,
            ORCHARD_UPSTREAM_CRATE_VERSION_V1.as_bytes(),
            ORCHARD_UPSTREAM_REVISION_V1.as_bytes(),
            ORCHARD_POST_NU6_3_CIRCUIT_DESCRIPTION_SHA256_V1.as_bytes(),
            &empty_root,
            &action_count,
            &one_action_wire,
            &two_action_wire,
        ],
    );
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ORCHARD_PROTOCOL_LABEL_V1,
            ORCHARD_IMPLEMENTATION_PROVENANCE_V1,
            ORCHARD_PARAMETER_SET_LABEL_V1,
            ORCHARD_PROOF_WIRE_LABEL_V1,
            ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1,
            ORCHARD_FRONTIER_SCHEMA_V1,
            ORCHARD_VERIFIED_EFFECT_SCHEMA_V1,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &action_count,
            &one_action_wire,
            &two_action_wire,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ORCHARD_PROTOCOL_LABEL_V1,
            ORCHARD_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:halo2-ipa-pasta",
            b"engine:native-halo2-orchard",
            ORCHARD_PARAMETER_SET_LABEL_V1,
            ORCHARD_PROOF_WIRE_LABEL_V1,
            ORCHARD_COMPILED_PROFILE_DESCRIPTOR_V1,
            ORCHARD_FRONTIER_SCHEMA_V1,
            ORCHARD_VERIFIED_EFFECT_SCHEMA_V1,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            &empty_root,
            &action_count,
            &one_action_wire,
            &two_action_wire,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::Halo2IpaPasta,
        engine_id: PrivacyEngineIdV1::NativeHalo2Orchard,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(
            OrchardActivationLimitsV1 {
                max_action_count: ORCHARD_MODEL_MAX_ACTIONS_V1,
            },
        ),
    })
}

/// Require exact compiled cryptographic bindings and bounded governed policy.
///
/// # Errors
///
/// Returns a typed mismatch for the first differing consensus binding, or an
/// unavailable/initialization error when no executable profile exists.
pub fn validate_compiled_privacy_activation_v1(
    activation: &PrivacyProtocolActivationRecordV1,
) -> Result<(), CompiledPrivacyProfileValidationErrorV1> {
    let compiled = compiled_privacy_profile_v1(activation.protocol_id)
        .map_err(CompiledPrivacyProfileValidationErrorV1::Profile)?;
    if activation.proof_system_id != compiled.proof_system_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch);
    }
    if activation.engine_id != compiled.engine_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::EngineMismatch);
    }
    if activation.parameter_id != compiled.parameter_id {
        return Err(CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch);
    }
    if activation.parameter_digest != compiled.parameter_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch);
    }
    if activation.verifier_digest != compiled.verifier_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch);
    }
    if activation.statement_schema_digest != compiled.statement_schema_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch);
    }
    if activation.engine_manifest_digest != compiled.engine_manifest_digest {
        return Err(CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch);
    }
    activation
        .protocol_limits
        .validate_with_ceiling(&compiled.protocol_limits)
        .map_err(|_| CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch)?;
    if activation.assurance != PrivacyAssuranceV1::Experimental {
        return Err(CompiledPrivacyProfileValidationErrorV1::AssuranceMismatch);
    }
    Ok(())
}

#[cfg(feature = "zk-stark")]
fn compiled_zk_ace_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
    let compiled_profile_digest = zk_ace_compiled_profile_digest_v1();
    let proof_bytes = u64::from(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1);
    if proof_bytes > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1) {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let proof_bytes_encoded = proof_bytes.to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let poseidon_manifest = ZK_ACE_POSEIDON_MANIFEST_SHA256_V1.as_bytes();
    let stark_profile = zk_ace_stark_profile_descriptor_v1();
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ZK_ACE_PROTOCOL_LABEL_V1,
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            ZK_ACE_SOURCE_PROFILE_V1,
            poseidon_manifest,
            stark_profile,
            &compiled_profile_digest,
            &proof_bytes_encoded,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            ZK_ACE_PROTOCOL_LABEL_V1,
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            ZK_ACE_SOURCE_PROFILE_V1,
            ZK_ACE_AIR_RELATION_SCHEMA_V1,
            ZK_ACE_AUTHORIZATION_PROJECTION_V1,
            poseidon_manifest,
            stark_profile,
            &compiled_profile_digest,
            &proof_bytes_encoded,
        ],
    );
    let statement_schema_digest = canonical_schema_digest_v1::<ZkAcePqAuthorizationStatementV1>()
        .map_err(|source| {
        CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
            protocol_id,
            source,
        }
    })?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ZK_ACE_PROTOCOL_LABEL_V1,
            ZK_ACE_SOURCE_PROFILE_V1,
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.as_bytes(),
            ZK_ACE_PROOF_WIRE_V1,
            ZK_ACE_AIR_RELATION_SCHEMA_V1,
            ZK_ACE_AUTHORIZATION_PROJECTION_V1,
            poseidon_manifest,
            stark_profile,
            &compiled_profile_digest,
            &proof_bytes_encoded,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ZK_ACE_PROTOCOL_LABEL_V1,
            ZK_ACE_SOURCE_PROFILE_V1,
            b"proof-system:stark-fri-sha256-goldilocks",
            b"engine:native-goldilocks-stark-fri",
            ZK_ACE_PARAMETER_SET_LABEL_V1,
            ZK_ACE_PRIVACY_TRANSCRIPT_LABEL_V1.as_bytes(),
            ZK_ACE_PROOF_WIRE_V1,
            ZK_ACE_AIR_RELATION_SCHEMA_V1,
            ZK_ACE_AUTHORIZATION_PROJECTION_V1,
            poseidon_manifest,
            stark_profile,
            &compiled_profile_digest,
            &proof_bytes_encoded,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
        engine_id: PrivacyEngineIdV1::NativeGoldilocksStarkFri,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0,
    })
}

fn compiled_zk_ams_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaZkAmsV1;
    if ZK_AMS_RING_SIZES_V1 != [16, 32, 64]
        || ZK_AMS_MODEL_RING_SIZES_V1 != [16, 32, 64]
        || ZK_AMS_MAX_RING_SIZE_V1 != 64
        || ZK_AMS_MAX_BATCH_SIZE_V1 != 8
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }

    let dimensions = zk_ams_admission_relation_dimensions_v1()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let encode_usize = |value: usize| {
        u64::try_from(value)
            .map(u64::to_be_bytes)
            .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })
    };
    let variable_count = encode_usize(dimensions.variable_count)?;
    let constraint_count = encode_usize(dimensions.constraint_count)?;
    let public_input_count = encode_usize(dimensions.public_input_count)?;
    let witness_commitment_points = encode_usize(dimensions.witness_commitment_points)?;
    let error_commitment_points = encode_usize(dimensions.error_commitment_points)?;
    let outer_sumcheck_rounds = encode_usize(dimensions.outer_sumcheck_rounds)?;
    let inner_sumcheck_rounds = encode_usize(dimensions.inner_sumcheck_rounds)?;
    let declared_public_input_count = encode_usize(ZK_AMS_ADMISSION_PUBLIC_INPUTS_V1)?;
    if public_input_count != declared_public_input_count {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let phc_payload_bytes = encode_usize(ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1)?;

    let relation_proof_cap_value = u64::try_from(MAX_ZK_AMS_ADMISSION_RELATION_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let batch_proof_cap_value = u64::try_from(MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let possession_proof_cap_value = u64::try_from(MAX_ZK_AMS_ADMISSION_POSSESSION_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let provision_proof_cap_value = u64::try_from(MAX_ZK_AMS_LSAG_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let global_proof_cap_value = u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1);
    if batch_proof_cap_value > global_proof_cap_value
        || provision_proof_cap_value > global_proof_cap_value
    {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let relation_proof_cap = relation_proof_cap_value.to_be_bytes();
    let batch_proof_cap = batch_proof_cap_value.to_be_bytes();
    let possession_proof_cap = possession_proof_cap_value.to_be_bytes();
    let provision_proof_cap = provision_proof_cap_value.to_be_bytes();
    let global_proof_cap = global_proof_cap_value.to_be_bytes();

    let max_batch_size = ZK_AMS_MAX_BATCH_SIZE_V1.to_be_bytes();
    let ring_size_16 = ZK_AMS_MODEL_RING_SIZES_V1[0].to_be_bytes();
    let ring_size_32 = ZK_AMS_MODEL_RING_SIZES_V1[1].to_be_bytes();
    let ring_size_64 = ZK_AMS_MODEL_RING_SIZES_V1[2].to_be_bytes();
    let admission_possession_version = [ZK_AMS_ADMISSION_POSSESSION_PROOF_VERSION_V1];
    let lsag_version = [ZK_AMS_LSAG_PROOF_VERSION_V1];
    let compiled_relation_digest = zk_ams_compiled_profile_digest_v1();
    let t256_generator_digest = zk_ams_t256_generator_digest_v1();
    let combined_generator_digest = zk_ams_generator_digest_v1();

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ZK_AMS_PROTOCOL_LABEL_V1,
            ZK_AMS_PARAMETER_SET_LABEL_V1,
            ZK_AMS_SOURCE_PROFILE_V1,
            &compiled_relation_digest,
            &t256_generator_digest,
            &combined_generator_digest,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            ZK_AMS_PROTOCOL_LABEL_V1,
            ZK_AMS_PARAMETER_SET_LABEL_V1,
            ZK_AMS_SOURCE_PROFILE_V1,
            ZK_AMS_LSAG_SUITE_V1,
            ZK_AMS_ADMISSION_POSSESSION_SUITE_V1,
            &compiled_relation_digest,
            &t256_generator_digest,
            &combined_generator_digest,
            &variable_count,
            &constraint_count,
            &public_input_count,
            &witness_commitment_points,
            &error_commitment_points,
            &outer_sumcheck_rounds,
            &inner_sumcheck_rounds,
            &phc_payload_bytes,
            &max_batch_size,
            &ring_size_16,
            &ring_size_32,
            &ring_size_64,
        ],
    );
    let statement_schema_digest =
        canonical_schema_digest_v1::<IrohaZkAmsStatementV1>().map_err(|source| {
            CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            }
        })?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ZK_AMS_PROTOCOL_LABEL_V1,
            ZK_AMS_IMPLEMENTATION_PROVENANCE_V1,
            ZK_AMS_SOURCE_PROFILE_V1,
            ZK_AMS_PARAMETER_SET_LABEL_V1,
            ZK_AMS_BATCH_PROOF_WIRE_LABEL_V1,
            ZK_AMS_PROVISION_PROOF_WIRE_LABEL_V1,
            ZK_AMS_LSAG_SUITE_V1,
            ZK_AMS_ADMISSION_POSSESSION_SUITE_V1,
            &admission_possession_version,
            &lsag_version,
            &compiled_relation_digest,
            &t256_generator_digest,
            &combined_generator_digest,
            &variable_count,
            &constraint_count,
            &public_input_count,
            &witness_commitment_points,
            &error_commitment_points,
            &outer_sumcheck_rounds,
            &inner_sumcheck_rounds,
            &phc_payload_bytes,
            &relation_proof_cap,
            &possession_proof_cap,
            &batch_proof_cap,
            &provision_proof_cap,
            &max_batch_size,
            &ring_size_16,
            &ring_size_32,
            &ring_size_64,
            ZK_AMS_BATCH_EFFECT_SCHEMA_V1,
            ZK_AMS_PROVISION_EFFECT_SCHEMA_V1,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ZK_AMS_PROTOCOL_LABEL_V1,
            ZK_AMS_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
            b"engine:native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
            ZK_AMS_PARAMETER_SET_LABEL_V1,
            ZK_AMS_BATCH_PROOF_WIRE_LABEL_V1,
            ZK_AMS_PROVISION_PROOF_WIRE_LABEL_V1,
            ZK_AMS_BATCH_EFFECT_SCHEMA_V1,
            ZK_AMS_PROVISION_EFFECT_SCHEMA_V1,
            &compiled_relation_digest,
            &t256_generator_digest,
            &combined_generator_digest,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &relation_proof_cap,
            &possession_proof_cap,
            &batch_proof_cap,
            &provision_proof_cap,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512,
        engine_id: PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
            max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1,
            max_ring_size: ZK_AMS_MAX_RING_SIZE_V1,
        }),
    })
}

fn compiled_vega_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::VegaExistingCredentialZkV0;
    let compiled_profile_digest = vega_mdl_compiled_profile_digest_v1();
    let proof_bytes = u64::try_from(MAX_VEGA_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    if proof_bytes > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1) {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let proof_bytes_encoded = proof_bytes.to_be_bytes();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1,
            VEGA_PARAMETER_SET_LABEL_V1,
            VEGA_IMPLEMENTATION_PROVENANCE_V1,
            VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
            &compiled_profile_digest,
            &proof_bytes_encoded,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1,
            VEGA_PARAMETER_SET_LABEL_V1,
            VEGA_IMPLEMENTATION_PROVENANCE_V1,
            VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
            &compiled_profile_digest,
            &proof_bytes_encoded,
        ],
    );
    let statement_schema_digest = canonical_schema_digest_v1::<VegaExistingCredentialStatementV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1,
            VEGA_IMPLEMENTATION_PROVENANCE_V1,
            VEGA_PARAMETER_SET_LABEL_V1,
            VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
            VEGA_PROOF_WIRE_LABEL_V1,
            &compiled_profile_digest,
            &proof_bytes_encoded,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            VEGA_EXISTING_CREDENTIAL_PROTOCOL_LABEL_V1,
            VEGA_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:vega-neutron-nova-spartan-hyrax-t256",
            b"engine:native-vega",
            VEGA_PARAMETER_SET_LABEL_V1,
            VEGA_INTERNAL_TRANSCRIPT_PERSONA_V1,
            VEGA_PROOF_WIRE_LABEL_V1,
            &compiled_profile_digest,
            &proof_bytes_encoded,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256,
        engine_id: PrivacyEngineIdV1::NativeVega,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0,
    })
}

fn compiled_anonymous_pgc_profile_v1()
-> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1;
    if !ANONYMOUS_PGC_FULL_ENGINE_AVAILABLE_V1 {
        return Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id });
    }
    let parameters = AnonymousPgcParametersV1::get()
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            ANONYMOUS_PGC_PROTOCOL_LABEL_V1,
            ANONYMOUS_PGC_PARAMETER_SET_LABEL_V1,
            PGC_SOURCE_PROFILE_V1,
        ],
    );

    let payment_version = [PGC_PAYMENT_PROOF_VERSION_V1];
    let payment_count_16 = u32::try_from(PGC_PAYMENT_ANONYMITY_SET_SIZES_V1[0])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let payment_count_32 = u32::try_from(PGC_PAYMENT_ANONYMITY_SET_SIZES_V1[1])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let payment_count_64 = u32::try_from(PGC_PAYMENT_ANONYMITY_SET_SIZES_V1[2])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let payment_max_recipients = u32::try_from(PGC_PAYMENT_MAX_RECIPIENTS_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let payment_proof_cap = u32::try_from(MAX_PGC_PAYMENT_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();

    let bootstrap_version = [PGC_BOOTSTRAP_PROOF_VERSION_V1];
    let bootstrap_initial_epoch = PGC_BOOTSTRAP_INITIAL_EPOCH_V1.to_be_bytes();
    let bootstrap_count_16 = u32::try_from(PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1[0])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let bootstrap_count_32 = u32::try_from(PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1[1])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let bootstrap_count_64 = u32::try_from(PGC_BOOTSTRAP_ACCOUNT_COUNTS_V1[2])
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let bootstrap_namespace_cap = u32::try_from(MAX_PGC_BOOTSTRAP_NAMESPACE_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let bootstrap_max_aggregate = PGC_BOOTSTRAP_MAX_AGGREGATE_BALANCE_V1.to_be_bytes();
    let bootstrap_proof_cap = u32::try_from(MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?
        .to_be_bytes();
    let generator_digest = parameters.generator_digest();
    let bootstrap_schema_digest = canonical_schema_digest_v1::<PrivacyPgcAccountBootstrapV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            ANONYMOUS_PGC_PROTOCOL_LABEL_V1,
            PGC_SOURCE_PROFILE_V1,
            PGC_PAYMENT_SUITE_V1,
            &payment_version,
            ANONYMOUS_PGC_PAYMENT_PROOF_WIRE_LABEL_V1,
            PGC_PAYMENT_POOL_INVARIANT_SCHEMA_V1,
            &payment_count_16,
            &payment_count_32,
            &payment_count_64,
            &payment_max_recipients,
            &payment_proof_cap,
            PGC_BOOTSTRAP_SUITE_V1,
            &bootstrap_version,
            &bootstrap_initial_epoch,
            ANONYMOUS_PGC_BOOTSTRAP_PROOF_WIRE_LABEL_V1,
            &bootstrap_count_16,
            &bootstrap_count_32,
            &bootstrap_count_64,
            &bootstrap_namespace_cap,
            &bootstrap_max_aggregate,
            PGC_BOOTSTRAP_TABLE_DIGEST_DOMAIN_V1,
            PGC_BOOTSTRAP_TABLE_DIGEST_SCHEMA_V1,
            &bootstrap_proof_cap,
            &bootstrap_schema_digest,
            PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1,
            ANONYMOUS_PGC_ACCOUNT_ROOT_SCHEMA_V1,
            ANONYMOUS_PGC_VERIFIED_EFFECT_SCHEMA_V1,
            &generator_digest,
        ],
    );
    let statement_schema_digest = canonical_schema_digest_v1::<AnonymousPgcKOutOfNStatementV1>()
        .map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let parameter_digest = parameters.parameter_digest();
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            ANONYMOUS_PGC_PROTOCOL_LABEL_V1,
            ANONYMOUS_PGC_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:anonymous-pgc-p256",
            b"engine:native-anonymous-pgc-p256",
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &bootstrap_schema_digest,
            PGC_PAYMENT_POOL_INVARIANT_SCHEMA_V1,
            PGC_BOOTSTRAP_TABLE_DIGEST_SCHEMA_V1,
            &bootstrap_initial_epoch,
            PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1,
            ANONYMOUS_PGC_ACCOUNT_ROOT_SCHEMA_V1,
            ANONYMOUS_PGC_VERIFIED_EFFECT_SCHEMA_V1,
            &payment_proof_cap,
            &bootstrap_proof_cap,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::AnonymousPgcP256,
        engine_id: PrivacyEngineIdV1::NativeAnonymousPgcP256,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1,
                max_recipient_count: ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
            },
        ),
    })
}

fn compiled_jindo_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1> {
    let protocol_id = PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0;
    let max_polynomial_count =
        u32::try_from(JINDO_MAX_BATCH_SIZE_V1).expect("fixed Jindo batch size fits u32");
    let max_polynomial_count_bytes = max_polynomial_count.to_be_bytes();
    let proof_bytes = u64::try_from(JINDO_NATIVE_PROOF_BYTES_V1)
        .map_err(|_| CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id })?;
    let proof_bytes_encoded = proof_bytes.to_be_bytes();
    if proof_bytes > u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1) {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed { protocol_id });
    }
    let global_proof_cap = TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1.to_be_bytes();
    let crs_digest = jindo_crs_digest_v1();

    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            JINDO_PROTOCOL_LABEL_V1,
            JINDO_PARAMETER_SET_LABEL_V1,
            JINDO_SOURCE_PROFILE_V1,
            JINDO_SUITE_V1,
            JINDO_PARAMETER_MANIFEST_V1,
            &crs_digest,
            &proof_bytes_encoded,
            &max_polynomial_count_bytes,
        ],
    );
    let parameter_digest = digest_fields_v1(
        PARAMETER_DIGEST_DOMAIN_V1,
        &[
            JINDO_PROTOCOL_LABEL_V1,
            JINDO_PARAMETER_SET_LABEL_V1,
            JINDO_SOURCE_PROFILE_V1,
            JINDO_SUITE_V1,
            JINDO_PARAMETER_MANIFEST_V1,
            &crs_digest,
            &proof_bytes_encoded,
            &max_polynomial_count_bytes,
        ],
    );
    let statement_schema_digest =
        canonical_schema_digest_v1::<IrohaJindoPolynomialCommitmentStatementV1>().map_err(
            |source| CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id,
                source,
            },
        )?;
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            JINDO_PROTOCOL_LABEL_V1,
            JINDO_IMPLEMENTATION_PROVENANCE_V1,
            JINDO_SOURCE_PROFILE_V1,
            JINDO_SUITE_V1,
            JINDO_PARAMETER_MANIFEST_V1,
            JINDO_PROOF_WIRE_LABEL_V1,
            &crs_digest,
            &proof_bytes_encoded,
            &max_polynomial_count_bytes,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            JINDO_PROTOCOL_LABEL_V1,
            JINDO_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:jindo-polynomial-commitment",
            b"engine:native-jindo",
            JINDO_SOURCE_PROFILE_V1,
            JINDO_SUITE_V1,
            JINDO_PARAMETER_MANIFEST_V1,
            JINDO_PROOF_WIRE_LABEL_V1,
            &crs_digest,
            &proof_bytes_encoded,
            &max_polynomial_count_bytes,
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &global_proof_cap,
        ],
    );

    Ok(CompiledPrivacyProfileV1 {
        protocol_id,
        proof_system_id: PrivacyProofSystemIdV1::JindoPolynomialCommitment,
        engine_id: PrivacyEngineIdV1::NativeJindo,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
            JindoActivationLimitsV1 {
                max_polynomial_count,
            },
        ),
    })
}

fn compiled_verange_profile_v1() -> Result<CompiledPrivacyProfileV1, CompiledPrivacyProfileErrorV1>
{
    let bits_32 = VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits32).map_err(|_| {
        CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        }
    })?;
    let bits_64 = VeRangeParametersV1::for_profile(VeRangeBitLengthV1::Bits64).map_err(|_| {
        CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        }
    })?;
    if bits_32.parameter_digest() != bits_64.parameter_digest() {
        return Err(CompiledPrivacyProfileErrorV1::ProfileInitializationFailed {
            protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        });
    }

    let bits_32_descriptor = bits_32.descriptor();
    let bits_64_descriptor = bits_64.descriptor();
    let parameter_id = digest_fields_v1(
        PARAMETER_ID_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_PARAMETER_SET_LABEL_V1,
            VERANGE_TYPE1_SOURCE_PROFILE_V1,
        ],
    );
    let proof_version = [VERANGE_TYPE1_PROOF_VERSION_V1];
    let bit_length_32 = bits_32_descriptor.bit_length.to_be_bytes();
    let rows_32 = bits_32_descriptor.rows.to_be_bytes();
    let columns_32 = bits_32_descriptor.columns.to_be_bytes();
    let bit_length_64 = bits_64_descriptor.bit_length.to_be_bytes();
    let rows_64 = bits_64_descriptor.rows.to_be_bytes();
    let columns_64 = bits_64_descriptor.columns.to_be_bytes();
    let max_batch = bits_64_descriptor.max_batch_commitments.to_be_bytes();
    let max_batch_proof = bits_64_descriptor.max_batch_proof_bytes.to_be_bytes();
    let verifier_digest = digest_fields_v1(
        VERIFIER_DIGEST_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_TYPE1_SOURCE_PROFILE_V1,
            VERANGE_TYPE1_SUITE_V1,
            &proof_version,
            VERANGE_PROOF_WIRE_LABEL_V1,
            &bit_length_32,
            &rows_32,
            &columns_32,
            &bits_32_descriptor.generator_digest,
            &bit_length_64,
            &rows_64,
            &columns_64,
            &bits_64_descriptor.generator_digest,
            &max_batch,
            &max_batch_proof,
        ],
    );
    let statement_schema_digest =
        canonical_schema_digest_v1::<VeRangeTransparentRangeStatementV1>().map_err(|source| {
            CompiledPrivacyProfileErrorV1::StatementSchemaInvalid {
                protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                source,
            }
        })?;
    let parameter_digest = bits_32.parameter_digest();
    let global_commitment_cap = TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1.to_be_bytes();
    let engine_manifest_digest = digest_fields_v1(
        ENGINE_MANIFEST_DIGEST_DOMAIN_V1,
        &[
            VERANGE_PROTOCOL_LABEL_V1,
            VERANGE_IMPLEMENTATION_PROVENANCE_V1,
            b"proof-system:iroha-verange-p256",
            b"engine:native-verange-p256",
            &parameter_id,
            &parameter_digest,
            &verifier_digest,
            &statement_schema_digest,
            &global_commitment_cap,
        ],
    );
    let max_aggregation_count =
        VERANGE_HARD_MAX_AGGREGATION_COUNT_V1.min(TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1);

    Ok(CompiledPrivacyProfileV1 {
        protocol_id: PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
        proof_system_id: PrivacyProofSystemIdV1::IrohaVeRangeP256,
        engine_id: PrivacyEngineIdV1::NativeVeRangeP256,
        parameter_id: PrivacyParameterIdV1::new(parameter_id),
        parameter_digest: PrivacyParameterDigestV1::new(parameter_digest),
        verifier_digest: PrivacyVerifierDigestV1::new(verifier_digest),
        statement_schema_digest: PrivacyStatementSchemaDigestV1::new(statement_schema_digest),
        engine_manifest_digest: PrivacyEngineManifestDigestV1::new(engine_manifest_digest),
        protocol_limits: PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count,
            },
        ),
    })
}

fn digest_fields_v1(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, domain);
    hash.update(usize_to_u64_v1(fields.len()).to_be_bytes());
    for field in fields {
        append_digest_field_v1(&mut hash, field);
    }
    hash.finalize().into()
}

fn append_digest_field_v1(hash: &mut Sha256, field: &[u8]) {
    hash.update(usize_to_u64_v1(field.len()).to_be_bytes());
    hash.update(field);
}

/// Hash the complete structural schema of `T` in a platform-independent order.
///
/// The `iroha_schema` map is keyed internally by Rust [`core::any::TypeId`],
/// whose ordering is not a wire contract. This function replaces every such
/// reference with its declared stable string identifier, collapses only
/// representation aliases with identical canonical metadata, rejects
/// conflicting reuse of an identifier, sorts top-level entries by that stable
/// identifier, and preserves field/variant order where order is part of the
/// representation. Consequently, adding, deleting, reordering, or retyping a
/// statement field changes the governed digest without relying on a
/// hand-maintained schema string.
///
/// # Errors
///
/// Returns a typed error if the generated schema has duplicate stable type
/// identifiers or contains a reference to a type omitted from its own map.
pub fn canonical_schema_digest_v1<T: IntoSchema>() -> Result<[u8; 32], CanonicalSchemaDigestErrorV1>
{
    let schema = T::schema();
    let mut type_names = BTreeMap::new();
    for (rust_type_id, entry) in schema.iter() {
        type_names.insert(*rust_type_id, entry.type_id.clone());
    }

    // `iroha_schema` deliberately gives representation aliases such as
    // `String` and `Box<str>` the same stable wire identifier. Rust `TypeId`
    // still makes them separate map entries. Collapse aliases only after their
    // complete canonical metadata agrees; a reused identifier with a different
    // name or representation is an ambiguous consensus schema and must fail.
    let mut entries = BTreeMap::<String, CanonicalSchemaEntryV1>::new();
    for (_, entry) in schema.iter() {
        let canonical = CanonicalSchemaEntryV1 {
            type_name: entry.type_name.clone(),
            metadata: canonical_schema_metadata_v1(entry, &type_names)?,
        };
        if let Some(existing) = entries.get(&entry.type_id) {
            if existing != &canonical {
                return Err(CanonicalSchemaDigestErrorV1::ConflictingStableTypeId);
            }
        } else {
            entries.insert(entry.type_id.clone(), canonical);
        }
    }

    let mut hash = Sha256::new();
    hash.update(PROFILE_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, CANONICAL_SCHEMA_DIGEST_DOMAIN_V1);
    append_digest_field_v1(&mut hash, T::id().as_bytes());
    append_digest_count_v1(&mut hash, entries.len());
    for (stable_id, entry) in entries {
        append_digest_field_v1(&mut hash, stable_id.as_bytes());
        append_digest_field_v1(&mut hash, entry.type_name.as_bytes());
        append_digest_field_v1(&mut hash, &entry.metadata);
    }
    Ok(hash.finalize().into())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalSchemaEntryV1 {
    type_name: String,
    metadata: Vec<u8>,
}

fn canonical_schema_metadata_v1(
    entry: &MetaMapEntry,
    type_names: &BTreeMap<core::any::TypeId, String>,
) -> Result<Vec<u8>, CanonicalSchemaDigestErrorV1> {
    let mut bytes = Vec::new();
    match &entry.metadata {
        Metadata::Struct(fields) => {
            append_schema_field_v1(&mut bytes, b"struct");
            append_schema_count_v1(&mut bytes, fields.declarations.len());
            for field in &fields.declarations {
                append_schema_field_v1(&mut bytes, field.name.as_bytes());
                append_schema_type_reference_v1(&mut bytes, field.ty, type_names)?;
            }
        }
        Metadata::Tuple(fields) => {
            append_schema_field_v1(&mut bytes, b"tuple");
            append_schema_count_v1(&mut bytes, fields.types.len());
            for field_type in &fields.types {
                append_schema_type_reference_v1(&mut bytes, *field_type, type_names)?;
            }
        }
        Metadata::Enum(enum_meta) => {
            append_schema_field_v1(&mut bytes, b"enum");
            append_schema_count_v1(&mut bytes, enum_meta.variants.len());
            for variant in &enum_meta.variants {
                append_schema_field_v1(&mut bytes, variant.tag.as_bytes());
                append_schema_field_v1(&mut bytes, &[variant.discriminant]);
                match variant.ty {
                    Some(variant_type) => {
                        append_schema_field_v1(&mut bytes, b"some");
                        append_schema_type_reference_v1(&mut bytes, variant_type, type_names)?;
                    }
                    None => append_schema_field_v1(&mut bytes, b"none"),
                }
            }
        }
        Metadata::Int(mode) => {
            append_schema_field_v1(&mut bytes, b"int");
            append_schema_field_v1(
                &mut bytes,
                match mode {
                    IntMode::FixedWidth => b"fixed-width",
                    IntMode::Compact => b"compact",
                },
            );
        }
        Metadata::Float(mode) => {
            append_schema_field_v1(&mut bytes, b"float");
            append_schema_field_v1(
                &mut bytes,
                match mode {
                    FloatMode::Binary32 => b"binary32",
                    FloatMode::Binary64 => b"binary64",
                },
            );
        }
        Metadata::String => append_schema_field_v1(&mut bytes, b"string"),
        Metadata::Bool => append_schema_field_v1(&mut bytes, b"bool"),
        Metadata::FixedPoint(fixed) => {
            append_schema_field_v1(&mut bytes, b"fixed-point");
            append_schema_type_reference_v1(&mut bytes, fixed.base, type_names)?;
            append_schema_field_v1(&mut bytes, &fixed.decimal_places.to_be_bytes());
        }
        Metadata::Array(array) => {
            append_schema_field_v1(&mut bytes, b"array");
            append_schema_type_reference_v1(&mut bytes, array.ty, type_names)?;
            append_schema_field_v1(&mut bytes, &array.len.to_be_bytes());
        }
        Metadata::Vec(vector) => {
            append_schema_field_v1(&mut bytes, b"vec");
            append_schema_type_reference_v1(&mut bytes, vector.ty, type_names)?;
        }
        Metadata::Map(map) => {
            append_schema_field_v1(&mut bytes, b"map");
            append_schema_type_reference_v1(&mut bytes, map.key, type_names)?;
            append_schema_type_reference_v1(&mut bytes, map.value, type_names)?;
        }
        Metadata::Option(option_type) => {
            append_schema_field_v1(&mut bytes, b"option");
            append_schema_type_reference_v1(&mut bytes, *option_type, type_names)?;
        }
        Metadata::Result(result) => {
            append_schema_field_v1(&mut bytes, b"result");
            append_schema_type_reference_v1(&mut bytes, result.ok, type_names)?;
            append_schema_type_reference_v1(&mut bytes, result.err, type_names)?;
        }
        Metadata::Bitmap(bitmap) => {
            append_schema_field_v1(&mut bytes, b"bitmap");
            append_schema_type_reference_v1(&mut bytes, bitmap.repr, type_names)?;
            append_schema_count_v1(&mut bytes, bitmap.masks.len());
            for mask in &bitmap.masks {
                append_schema_field_v1(&mut bytes, mask.name.as_bytes());
                append_schema_field_v1(&mut bytes, &mask.mask.to_be_bytes());
            }
        }
    }
    Ok(bytes)
}

fn append_schema_type_reference_v1(
    bytes: &mut Vec<u8>,
    rust_type_id: core::any::TypeId,
    type_names: &BTreeMap<core::any::TypeId, String>,
) -> Result<(), CanonicalSchemaDigestErrorV1> {
    let stable_id = type_names
        .get(&rust_type_id)
        .ok_or(CanonicalSchemaDigestErrorV1::MissingTypeReference)?;
    append_schema_field_v1(bytes, stable_id.as_bytes());
    Ok(())
}

fn append_schema_field_v1(bytes: &mut Vec<u8>, field: &[u8]) {
    bytes.extend_from_slice(&usize_to_u64_v1(field.len()).to_be_bytes());
    bytes.extend_from_slice(field);
}

fn append_schema_count_v1(bytes: &mut Vec<u8>, count: usize) {
    bytes.extend_from_slice(&usize_to_u64_v1(count).to_be_bytes());
}

fn append_digest_count_v1(hash: &mut Sha256, count: usize) {
    hash.update(usize_to_u64_v1(count).to_be_bytes());
}

#[cfg(target_pointer_width = "64")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from_ne_bytes(value.to_ne_bytes())
}

#[cfg(target_pointer_width = "32")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from(u32::from_ne_bytes(value.to_ne_bytes()))
}

#[cfg(target_pointer_width = "16")]
fn usize_to_u64_v1(value: usize) -> u64 {
    u64::from(u16::from_ne_bytes(value.to_ne_bytes()))
}

#[cfg(not(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
)))]
compile_error!("privacy profile digest framing supports 16-, 32-, and 64-bit pointer widths");

/// Invalid structural schema emitted by an [`IntoSchema`] implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CanonicalSchemaDigestErrorV1 {
    /// Two distinct Rust types claimed one identifier for different wire shapes.
    #[error("privacy statement schema reuses a stable type identifier for conflicting shapes")]
    ConflictingStableTypeId,
    /// Metadata referenced a type absent from the generated schema map.
    #[error("privacy statement schema contains an unresolved type reference")]
    MissingTypeReference,
}

/// Failure constructing a locally compiled privacy profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CompiledPrivacyProfileErrorV1 {
    /// The protocol has no complete native verifier in this binary.
    #[error("native privacy engine for {protocol_id:?} is unavailable")]
    EngineUnavailable {
        /// Protocol whose verifier is intentionally fail-closed.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// Deterministic transparent parameter initialization failed.
    #[error("compiled privacy profile initialization failed for {protocol_id:?}")]
    ProfileInitializationFailed {
        /// Protocol whose fixed parameters failed to initialize.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// The compiled public-statement schema is ambiguous or internally broken.
    #[error("compiled privacy statement schema is invalid for {protocol_id:?}: {source}")]
    StatementSchemaInvalid {
        /// Protocol whose typed schema failed canonicalization.
        protocol_id: PrivacyProtocolIdV1,
        /// Exact canonicalization failure.
        source: CanonicalSchemaDigestErrorV1,
    },
}

/// Failure matching governance material to compiled consensus code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum CompiledPrivacyProfileValidationErrorV1 {
    /// No executable compiled profile could be obtained.
    #[error(transparent)]
    Profile(CompiledPrivacyProfileErrorV1),
    /// Proof-system identity differs.
    #[error("privacy activation proof system differs from compiled profile")]
    ProofSystemMismatch,
    /// Engine identity differs.
    #[error("privacy activation engine differs from compiled profile")]
    EngineMismatch,
    /// Parameter-set identity differs.
    #[error("privacy activation parameter id differs from compiled profile")]
    ParameterIdMismatch,
    /// Parameter digest differs.
    #[error("privacy activation parameter digest differs from compiled profile")]
    ParameterDigestMismatch,
    /// Verifier digest differs.
    #[error("privacy activation verifier digest differs from compiled profile")]
    VerifierDigestMismatch,
    /// Public-statement schema digest differs.
    #[error("privacy activation statement schema differs from compiled profile")]
    StatementSchemaDigestMismatch,
    /// Engine manifest digest differs.
    #[error("privacy activation engine manifest differs from compiled profile")]
    EngineManifestDigestMismatch,
    /// Protocol-specific limits are invalid, target another protocol, or exceed
    /// the compiled hard ceilings.
    #[error("privacy activation protocol limits are outside the compiled profile ceilings")]
    ProtocolLimitsMismatch,
    /// The first-release assurance tag differs.
    #[error("privacy activation assurance differs from compiled testnet profile")]
    AssuranceMismatch,
}

#[cfg(test)]
mod tests {
    use iroha_data_model::privacy::{
        AnonymousPgcActivationLimitsV1, PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
        PrivacyProposedLifecycleV1,
    };
    use iroha_schema::{Declaration, MetaMap, NamedFieldsMeta, TypeId};

    use super::*;

    struct SchemaOrderAb;

    impl TypeId for SchemaOrderAb {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaOrderAb {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u32::update_schema_map(map);
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u32>(),
                    },
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                ],
            }));
        }
    }

    struct SchemaOrderBa;

    impl TypeId for SchemaOrderBa {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaOrderBa {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u32::update_schema_map(map);
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u32>(),
                    },
                ],
            }));
        }
    }

    struct SchemaRetyped;

    impl TypeId for SchemaRetyped {
        fn id() -> String {
            "privacy-test::CanonicalSchema".to_owned()
        }
    }

    impl IntoSchema for SchemaRetyped {
        fn type_name() -> String {
            "CanonicalSchema".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            u64::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "alpha".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                    Declaration {
                        name: "beta".to_owned(),
                        ty: core::any::TypeId::of::<u64>(),
                    },
                ],
            }));
        }
    }

    struct SchemaEquivalentAliases;

    impl TypeId for SchemaEquivalentAliases {
        fn id() -> String {
            "privacy-test::EquivalentAliases".to_owned()
        }
    }

    impl IntoSchema for SchemaEquivalentAliases {
        fn type_name() -> String {
            "EquivalentAliases".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            String::update_schema_map(map);
            Box::<str>::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "owned".to_owned(),
                        ty: core::any::TypeId::of::<String>(),
                    },
                    Declaration {
                        name: "boxed".to_owned(),
                        ty: core::any::TypeId::of::<Box<str>>(),
                    },
                ],
            }));
        }
    }

    struct SchemaConflictLeft;

    impl TypeId for SchemaConflictLeft {
        fn id() -> String {
            "privacy-test::ConflictingAlias".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictLeft {
        fn type_name() -> String {
            "ConflictingAlias".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Int(IntMode::FixedWidth));
        }
    }

    struct SchemaConflictRight;

    impl TypeId for SchemaConflictRight {
        fn id() -> String {
            "privacy-test::ConflictingAlias".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictRight {
        fn type_name() -> String {
            "ConflictingAlias".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            map.insert::<Self>(Metadata::Bool);
        }
    }

    struct SchemaConflictingAliases;

    impl TypeId for SchemaConflictingAliases {
        fn id() -> String {
            "privacy-test::ConflictingAliases".to_owned()
        }
    }

    impl IntoSchema for SchemaConflictingAliases {
        fn type_name() -> String {
            "ConflictingAliases".to_owned()
        }

        fn update_schema_map(map: &mut MetaMap) {
            SchemaConflictLeft::update_schema_map(map);
            SchemaConflictRight::update_schema_map(map);
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: "left".to_owned(),
                        ty: core::any::TypeId::of::<SchemaConflictLeft>(),
                    },
                    Declaration {
                        name: "right".to_owned(),
                        ty: core::any::TypeId::of::<SchemaConflictRight>(),
                    },
                ],
            }));
        }
    }

    fn verange_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("fixed VeRange parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    #[cfg(feature = "zk-stark")]
    fn zk_ace_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("fixed ZK-ACE profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn pgc_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("fixed Anonymous-PGC parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn zk_ams_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1)
            .expect("fixed ZK-AMS profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn jindo_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
            .expect("fixed Jindo parameters derive")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn vega_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("fixed Vega profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    fn orchard_activation() -> PrivacyProtocolActivationRecordV1 {
        compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("fixed Orchard profile derives")
            .activation_record(PrivacyProtocolLifecycleV1::Proposed(
                PrivacyProposedLifecycleV1 {
                    proposed_at_height: 100,
                    activate_at_height: 400,
                },
            ))
    }

    #[test]
    fn only_complete_engines_have_compiled_profiles() {
        let available = PrivacyProtocolIdV1::ALL
            .into_iter()
            .filter(|protocol_id| compiled_privacy_profile_v1(*protocol_id).is_ok())
            .collect::<Vec<_>>();
        assert_eq!(
            available,
            vec![
                #[cfg(feature = "zk-stark")]
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                PrivacyProtocolIdV1::IrohaZkAmsV1,
                PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
                PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            ]
        );
    }

    #[test]
    fn orchard_profile_is_deterministic_complete_bounded_and_mutation_closed() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(first.proof_system_id, PrivacyProofSystemIdV1::Halo2IpaPasta);
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeHalo2Orchard);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: ORCHARD_MODEL_MAX_ACTIONS_V1,
            })
        );
        assert_eq!(ORCHARD_ENGINE_MAX_ACTIONS_V1, 2);
        assert_eq!(ORCHARD_MODEL_MAX_ACTIONS_V1, 2);
        assert!(
            orchard_authorization_wire_size_v1(2).expect("wire size")
                <= usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1)
                    .expect("global proof cap fits usize")
        );
        assert_ne!(orchard_empty_root_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "8d5a2946c58314ac12d2968ffe9e8e0c672e3bbceefaaefad6a87420ea7dd212".to_owned(),
                "d26ee7a1bea91d998bc18351f0e923023d9fb17c98e68b6cc62d53425bdbb40e".to_owned(),
                "7435c73519e5a4f324baa7ddbf8a289dc4a06850a7e2ef7713666383e8580894".to_owned(),
                "2b82f59b9cc711d6eb832ececfabc900277525a099684e326e588068c6c84fb3".to_owned(),
                "75a0ea774499b595b7841f76d84e3b77be451891b4d1f60356ddc9407f70d155".to_owned(),
            ),
            "every consensus-critical Orchard profile binding is a pinned KAT"
        );

        let valid = orchard_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [fn(&mut PrivacyProtocolActivationRecordV1); 7] = [
            |record| record.parameter_id.0[0] ^= 1,
            |record| record.parameter_digest.0[0] ^= 1,
            |record| record.verifier_digest.0[0] ^= 1,
            |record| record.statement_schema_digest.0[0] ^= 1,
            |record| record.engine_manifest_digest.0[0] ^= 1,
            |record| {
                record.proof_system_id = PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs
            },
            |record| record.engine_id = PrivacyEngineIdV1::NativeFcmpPlusPlus,
        ];
        for mutate in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert!(validate_compiled_privacy_activation_v1(&changed).is_err());
        }
    }

    #[cfg(not(feature = "zk-stark"))]
    #[test]
    fn zk_ace_remains_fail_closed_without_a_sound_compiled_profile() {
        let protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
        assert_eq!(
            compiled_privacy_profile_v1(protocol_id),
            Err(CompiledPrivacyProfileErrorV1::EngineUnavailable { protocol_id })
        );
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn zk_ace_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeGoldilocksStarkFri);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0
        );
        assert!(ZK_ACE_PRIVACY_MAX_PROOF_BYTES_V1 <= TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1);
        assert_ne!(zk_ace_compiled_profile_digest_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                *first.parameter_id.as_bytes(),
                *first.parameter_digest.as_bytes(),
                *first.verifier_digest.as_bytes(),
                *first.statement_schema_digest.as_bytes(),
                *first.engine_manifest_digest.as_bytes(),
            ),
            (
                [
                    254, 89, 164, 122, 154, 145, 162, 236, 231, 205, 17, 164, 154, 156, 201, 35,
                    145, 207, 25, 212, 20, 243, 87, 13, 218, 46, 219, 222, 219, 217, 163, 104,
                ],
                [
                    142, 231, 214, 75, 162, 22, 103, 94, 54, 34, 56, 163, 73, 119, 57, 59, 86, 59,
                    8, 253, 94, 90, 250, 52, 241, 214, 229, 4, 82, 81, 201, 123,
                ],
                [
                    194, 216, 227, 126, 62, 3, 55, 81, 151, 214, 242, 118, 160, 22, 137, 21, 215,
                    111, 147, 56, 192, 48, 75, 183, 222, 4, 45, 233, 147, 78, 181, 153,
                ],
                [
                    149, 246, 40, 87, 47, 97, 157, 153, 226, 108, 237, 86, 152, 3, 50, 15, 32, 43,
                    234, 29, 21, 110, 147, 161, 192, 15, 130, 16, 133, 137, 192, 74,
                ],
                [
                    163, 108, 213, 105, 204, 79, 241, 59, 78, 53, 142, 14, 82, 167, 172, 226, 159,
                    28, 153, 183, 164, 8, 175, 152, 101, 242, 175, 137, 239, 46, 133, 82,
                ],
            )
        );
    }

    #[cfg(feature = "zk-stark")]
    #[test]
    fn zk_ace_compiled_profile_rejects_every_binding_mismatch() {
        let valid = zk_ace_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::StarkFriPoseidon2Goldilocks;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeJindo,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0;
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn zk_ams_profile_is_deterministic_complete_and_bounded() {
        let first =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1).expect("profile");
        let second =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1).expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512
        );
        assert_eq!(
            first.engine_id,
            PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255
        );
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1,
                max_ring_size: ZK_AMS_MAX_RING_SIZE_V1,
            })
        );
        assert!(
            MAX_ZK_AMS_BATCH_ADMISSION_PROOF_BYTES_V1
                <= TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize
        );
        assert!(
            MAX_ZK_AMS_LSAG_PROOF_BYTES_V1 <= TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize
        );
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                *first.parameter_id.as_bytes(),
                *first.parameter_digest.as_bytes(),
                *first.verifier_digest.as_bytes(),
                *first.statement_schema_digest.as_bytes(),
                *first.engine_manifest_digest.as_bytes(),
            ),
            (
                [
                    70, 247, 173, 84, 190, 160, 14, 222, 101, 31, 104, 149, 181, 141, 22, 106, 147,
                    134, 93, 165, 55, 214, 247, 164, 72, 136, 205, 104, 245, 91, 223, 174,
                ],
                [
                    146, 154, 50, 33, 11, 226, 74, 161, 247, 30, 128, 27, 120, 182, 0, 157, 56, 93,
                    154, 34, 44, 20, 66, 167, 0, 191, 100, 27, 99, 33, 159, 187,
                ],
                [
                    238, 198, 206, 167, 90, 54, 63, 19, 34, 83, 19, 215, 202, 231, 140, 91, 161,
                    15, 46, 118, 142, 118, 155, 100, 77, 51, 213, 154, 197, 192, 223, 223,
                ],
                [
                    18, 199, 81, 196, 252, 61, 89, 247, 231, 62, 169, 237, 114, 196, 11, 100, 227,
                    131, 165, 43, 21, 119, 174, 3, 145, 12, 100, 135, 228, 185, 86, 147,
                ],
                [
                    113, 64, 67, 124, 150, 74, 153, 202, 122, 232, 168, 149, 113, 211, 162, 61, 90,
                    90, 216, 218, 235, 213, 180, 163, 251, 70, 89, 236, 19, 254, 192, 175,
                ],
            )
        );
    }

    #[test]
    fn zk_ams_compiled_profile_rejects_every_binding_mismatch() {
        let valid = zk_ams_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id =
                        PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeVega,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                            max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1 + 1,
                            max_ring_size: ZK_AMS_MAX_RING_SIZE_V1,
                        });
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn structural_schema_digest_detects_reordering_and_retyping() {
        let original = canonical_schema_digest_v1::<SchemaOrderAb>().expect("schema");
        let reordered = canonical_schema_digest_v1::<SchemaOrderBa>().expect("schema");
        let retyped = canonical_schema_digest_v1::<SchemaRetyped>().expect("schema");
        assert_ne!(original, reordered);
        assert_ne!(original, retyped);
        assert_ne!(reordered, retyped);
        assert_eq!(
            original,
            canonical_schema_digest_v1::<SchemaOrderAb>().expect("schema")
        );
    }

    #[test]
    fn structural_schema_digest_deduplicates_only_equivalent_aliases() {
        let equivalent =
            canonical_schema_digest_v1::<SchemaEquivalentAliases>().expect("equivalent aliases");
        assert_ne!(equivalent, [0; 32]);
        assert_eq!(
            canonical_schema_digest_v1::<SchemaEquivalentAliases>().expect("equivalent aliases"),
            equivalent
        );
        assert_eq!(
            canonical_schema_digest_v1::<SchemaConflictingAliases>(),
            Err(CanonicalSchemaDigestErrorV1::ConflictingStableTypeId)
        );
    }

    #[test]
    fn verange_profile_is_deterministic_and_uses_effective_global_cap() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                VeRangeActivationLimitsV1 {
                    max_aggregation_count: 8,
                }
            )
        );
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "97e8be40e495bb6723db0ca73c04d2441ff166cf2163ddd2662c7e6a083f2c32".to_owned(),
                "3d79fe744741f956cb589f45774f922b849cf93833e6a9ebdedf1f815f1b7b44".to_owned(),
                "9b1a285d43ddc306b4d9ca6eac525b49b073f7d281ecf94299730613f683aa13".to_owned(),
                "3e63a74f7fc2deea533f379ecb143bdac1bd1dc7bbc7bc711013b73dac6e00f6".to_owned(),
                "b8e9530eb2eee338ef1b6217055d5bddaef71f5f73cd48657130b192c6b2b6d6".to_owned(),
            )
        );
    }

    #[test]
    fn anonymous_pgc_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            PGC_BOOTSTRAP_INITIAL_EPOCH_V1,
            PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1
        );
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                AnonymousPgcActivationLimitsV1 {
                    max_anonymity_set_size: 64,
                    max_recipient_count: 8,
                }
            )
        );
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "58c1a93d39f23727ae8b5bbb661414f3dcadf2479575282cd7e3b9ebbb5589fc".to_owned(),
                "ca09d19ed5f3bb56ba7432a67b7ad14697c4874ab7870ea53441e4df0624bd7b".to_owned(),
                "96d998f0519b9bc9bc95a959ff5e5b70fa76e248e6e17fdff4e210e175fd9af3".to_owned(),
                "080aaf7d1f9d44c5dad6a5adc393034715fbf428d1dd1e5b59e33808c110aa96".to_owned(),
                "2b8fb7d4547f59791e002b6d7f7ac9dace8cca73182f2339b8e198bce10c3771".to_owned(),
            )
        );
    }

    #[test]
    fn jindo_profile_is_deterministic_complete_and_bounded() {
        let first =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("profile");
        let second =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::JindoPolynomialCommitment
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeJindo);
        assert_eq!(JINDO_NATIVE_PROOF_BYTES_V1, 393_224);
        assert_ne!(jindo_crs_digest_v1(), [0; 32]);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: u32::try_from(JINDO_MAX_BATCH_SIZE_V1)
                        .expect("fixed Jindo batch size fits u32"),
                }
            )
        );
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
                hex::encode(jindo_crs_digest_v1()),
            ),
            (
                "f31a2e933a87837aa21ea847e41c19742db3264a67388d12e7569824249895b5".to_owned(),
                "e242ffba43bef1752f53ff40161deabaee972324a1c90cf8658181fc597afae9".to_owned(),
                "c797afdf5fa8141f3cfc85e16e495a0729c97a041d0e8e3f6fbf96b7dcfcf9ae".to_owned(),
                "7b87a8f64c9345e3ce13c2f4ce02a183e3806a8d2cea0faf7b6b0a00491aed28".to_owned(),
                "bbcb401ed660711f5b959e1a4bbd41f6eeb5ffb124c40af495ba915c18e688d1".to_owned(),
                "0ed26c12d05daa25307810cb6bf26b388baab3aa3f7641248db8ea3e4424f6b9".to_owned(),
            )
        );
    }

    #[test]
    fn vega_profile_is_deterministic_complete_and_bounded() {
        let first = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("profile");
        let second = compiled_privacy_profile_v1(PrivacyProtocolIdV1::VegaExistingCredentialZkV0)
            .expect("profile");
        assert_eq!(first, second);
        assert_eq!(
            first.proof_system_id,
            PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256
        );
        assert_eq!(first.engine_id, PrivacyEngineIdV1::NativeVega);
        assert_eq!(
            first.protocol_limits,
            PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0
        );
        assert!(MAX_VEGA_PROOF_BYTES_V1 <= TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1 as usize);
        assert_ne!(vega_mdl_compiled_profile_digest_v1(), [0; 32]);
        for digest in [
            *first.parameter_id.as_bytes(),
            *first.parameter_digest.as_bytes(),
            *first.verifier_digest.as_bytes(),
            *first.statement_schema_digest.as_bytes(),
            *first.engine_manifest_digest.as_bytes(),
        ] {
            assert_ne!(digest, [0; 32]);
        }
        assert_eq!(
            (
                hex::encode(first.parameter_id.as_bytes()),
                hex::encode(first.parameter_digest.as_bytes()),
                hex::encode(first.verifier_digest.as_bytes()),
                hex::encode(first.statement_schema_digest.as_bytes()),
                hex::encode(first.engine_manifest_digest.as_bytes()),
            ),
            (
                "65a3df3b4d5565ed6d17f048742f6443bdf005f9f7e465de269a3895dd0f3150".to_owned(),
                "7ae98000975ff7b55613bd7f96d3a708e8c71cad6f62adf8c7ae3d7fb031089d".to_owned(),
                "18087d00cfde4642595cbcd191b1175d5dcfa7ef512cd99976f729eb7a3a2c68".to_owned(),
                "2f5cfb37e975ece2b89d526e5d7105bbb2266962be1505ec7c747ba9822e80ec".to_owned(),
                "c3f04482c1a012ccea43dc6c13c5ee2aea246d44aba7b034e30619bf3a112272".to_owned(),
            )
        );
    }

    #[test]
    fn vega_compiled_profile_rejects_every_binding_mismatch() {
        let valid = vega_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 7] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::IrohaVeRangeP256;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeVeRangeP256,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn jindo_compiled_profile_rejects_every_binding_and_policy_mismatch() {
        let valid = jindo_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 8] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| {
                    record.proof_system_id = PrivacyProofSystemIdV1::IrohaVeRangeP256;
                },
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeVeRangeP256,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch,
                |record| {
                    record.protocol_limits =
                        PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                            JindoActivationLimitsV1 {
                                max_polynomial_count: 5,
                            },
                        );
                },
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn every_compiled_cryptographic_binding_is_immutable() {
        let valid = verange_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");

        let mutations: [(
            CompiledPrivacyProfileValidationErrorV1,
            fn(&mut PrivacyProtocolActivationRecordV1),
        ); 7] = [
            (
                CompiledPrivacyProfileValidationErrorV1::ProofSystemMismatch,
                |record| record.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineMismatch,
                |record| record.engine_id = PrivacyEngineIdV1::NativeGoldilocksStarkFri,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterIdMismatch,
                |record| record.parameter_id.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::ParameterDigestMismatch,
                |record| record.parameter_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::VerifierDigestMismatch,
                |record| record.verifier_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::StatementSchemaDigestMismatch,
                |record| record.statement_schema_digest.0[0] ^= 1,
            ),
            (
                CompiledPrivacyProfileValidationErrorV1::EngineManifestDigestMismatch,
                |record| record.engine_manifest_digest.0[0] ^= 1,
            ),
        ];
        for (expected, mutate) in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert_eq!(
                validate_compiled_privacy_activation_v1(&changed),
                Err(expected)
            );
        }
    }

    #[test]
    fn compiled_validation_accepts_lower_protocol_policy_without_changing_digests() {
        let verange_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::VeRangeTransparentRangeV1)
                .expect("VeRange profile");
        let mut verange = verange_activation();
        verange.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&verange).expect("lower VeRange policy");
        assert_eq!(verange.parameter_id, verange_compiled.parameter_id);
        assert_eq!(verange.parameter_digest, verange_compiled.parameter_digest);
        assert_eq!(verange.verifier_digest, verange_compiled.verifier_digest);
        assert_eq!(
            verange.statement_schema_digest,
            verange_compiled.statement_schema_digest
        );
        assert_eq!(
            verange.engine_manifest_digest,
            verange_compiled.engine_manifest_digest
        );

        let pgc_compiled = compiled_privacy_profile_v1(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1)
            .expect("PGC profile");
        let mut pgc = pgc_activation();
        pgc.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&pgc).expect("lower PGC policy");
        assert_eq!(pgc.parameter_id, pgc_compiled.parameter_id);
        assert_eq!(pgc.parameter_digest, pgc_compiled.parameter_digest);
        assert_eq!(pgc.verifier_digest, pgc_compiled.verifier_digest);
        assert_eq!(
            pgc.statement_schema_digest,
            pgc_compiled.statement_schema_digest
        );
        assert_eq!(
            pgc.engine_manifest_digest,
            pgc_compiled.engine_manifest_digest
        );

        let zk_ams_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaZkAmsV1).expect("ZK-AMS profile");
        let mut zk_ams = zk_ams_activation();
        zk_ams.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                max_batch_size: 1,
                max_ring_size: 16,
            });
        validate_compiled_privacy_activation_v1(&zk_ams).expect("lower ZK-AMS policy");
        assert_eq!(zk_ams.parameter_id, zk_ams_compiled.parameter_id);
        assert_eq!(zk_ams.parameter_digest, zk_ams_compiled.parameter_digest);
        assert_eq!(zk_ams.verifier_digest, zk_ams_compiled.verifier_digest);
        assert_eq!(
            zk_ams.statement_schema_digest,
            zk_ams_compiled.statement_schema_digest
        );
        assert_eq!(
            zk_ams.engine_manifest_digest,
            zk_ams_compiled.engine_manifest_digest
        );

        let jindo_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0)
                .expect("Jindo profile");
        let mut jindo = jindo_activation();
        jindo.protocol_limits = PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
            JindoActivationLimitsV1 {
                max_polynomial_count: 1,
            },
        );
        validate_compiled_privacy_activation_v1(&jindo).expect("lower Jindo policy");
        assert_eq!(jindo.parameter_id, jindo_compiled.parameter_id);
        assert_eq!(jindo.parameter_digest, jindo_compiled.parameter_digest);
        assert_eq!(jindo.verifier_digest, jindo_compiled.verifier_digest);
        assert_eq!(
            jindo.statement_schema_digest,
            jindo_compiled.statement_schema_digest
        );
        assert_eq!(
            jindo.engine_manifest_digest,
            jindo_compiled.engine_manifest_digest
        );

        let orchard_compiled =
            compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
                .expect("Orchard profile");
        let mut orchard = orchard_activation();
        orchard.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: 1,
            });
        validate_compiled_privacy_activation_v1(&orchard).expect("lower Orchard policy");
        assert_eq!(orchard.parameter_id, orchard_compiled.parameter_id);
        assert_eq!(orchard.parameter_digest, orchard_compiled.parameter_digest);
        assert_eq!(orchard.verifier_digest, orchard_compiled.verifier_digest);
        assert_eq!(
            orchard.statement_schema_digest,
            orchard_compiled.statement_schema_digest
        );
        assert_eq!(
            orchard.engine_manifest_digest,
            orchard_compiled.engine_manifest_digest
        );
    }

    #[test]
    fn compiled_validation_rejects_protocol_limit_overflow_mismatch_and_invalid_lowering() {
        let mut invalid = Vec::new();

        let mut verange_over = verange_activation();
        verange_over.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 9,
            },
        );
        invalid.push(verange_over);

        let mut pgc_n_over = pgc_activation();
        pgc_n_over.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 65,
                max_recipient_count: 8,
            },
        );
        invalid.push(pgc_n_over);

        let mut pgc_k_over = pgc_activation();
        pgc_k_over.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 64,
                max_recipient_count: 9,
            },
        );
        invalid.push(pgc_k_over);

        let mut pgc_bad_closed_set = pgc_activation();
        pgc_bad_closed_set.protocol_limits =
            PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                AnonymousPgcActivationLimitsV1 {
                    max_anonymity_set_size: 17,
                    max_recipient_count: 1,
                },
            );
        invalid.push(pgc_bad_closed_set);

        let mut zk_ams_batch_over = zk_ams_activation();
        zk_ams_batch_over.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1 + 1,
                max_ring_size: ZK_AMS_MAX_RING_SIZE_V1,
            });
        invalid.push(zk_ams_batch_over);

        let mut zk_ams_ring_over = zk_ams_activation();
        zk_ams_ring_over.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1,
                max_ring_size: ZK_AMS_MAX_RING_SIZE_V1 + 1,
            });
        invalid.push(zk_ams_ring_over);

        let mut zk_ams_bad_closed_set = zk_ams_activation();
        zk_ams_bad_closed_set.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                max_batch_size: 1,
                max_ring_size: 17,
            });
        invalid.push(zk_ams_bad_closed_set);

        let mut zero_verange = verange_activation();
        zero_verange.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 0,
            },
        );
        invalid.push(zero_verange);

        let mut jindo_over = jindo_activation();
        jindo_over.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: 5,
                },
            );
        invalid.push(jindo_over);

        let mut zero_jindo = jindo_activation();
        zero_jindo.protocol_limits =
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: 0,
                },
            );
        invalid.push(zero_jindo);

        let mut orchard_over = orchard_activation();
        orchard_over.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: ORCHARD_MODEL_MAX_ACTIONS_V1 + 1,
            });
        invalid.push(orchard_over);

        let mut zero_orchard = orchard_activation();
        zero_orchard.protocol_limits =
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: 0,
            });
        invalid.push(zero_orchard);

        let mut wrong_variant = verange_activation();
        wrong_variant.protocol_limits = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 1,
            },
        );
        invalid.push(wrong_variant);

        for activation in invalid {
            assert_eq!(
                validate_compiled_privacy_activation_v1(&activation),
                Err(CompiledPrivacyProfileValidationErrorV1::ProtocolLimitsMismatch)
            );
        }
    }

    #[test]
    fn unavailable_protocol_fails_before_governance_state_mutation() {
        let mut activation = verange_activation();
        activation.protocol_id = PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1;
        activation.proof_system_id = activation.protocol_id.expected_proof_system();
        activation.engine_id = activation.protocol_id.expected_engine();
        assert_eq!(
            validate_compiled_privacy_activation_v1(&activation),
            Err(CompiledPrivacyProfileValidationErrorV1::Profile(
                CompiledPrivacyProfileErrorV1::EngineUnavailable {
                    protocol_id: PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                }
            ))
        );
    }

    #[test]
    fn anonymous_pgc_compiled_bindings_are_immutable() {
        let valid = pgc_activation();
        validate_compiled_privacy_activation_v1(&valid).expect("exact profile");
        let mutations: [fn(&mut PrivacyProtocolActivationRecordV1); 5] = [
            |record| {
                record.parameter_digest.0[0] ^= 1;
            },
            |record| record.verifier_digest.0[0] ^= 1,
            |record| record.statement_schema_digest.0[0] ^= 1,
            |record| record.engine_manifest_digest.0[0] ^= 1,
            |record| {
                let PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(ref mut limits) =
                    record.protocol_limits
                else {
                    unreachable!("fixture is Anonymous PGC");
                };
                limits.max_recipient_count += 1;
            },
        ];
        for mutate in mutations {
            let mut changed = valid;
            mutate(&mut changed);
            assert!(validate_compiled_privacy_activation_v1(&changed).is_err());
        }
    }
}
