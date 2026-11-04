//! Core's own serialization of one capability manifest, for SDK parser conformance.
//!
//! SDK manifest parsers test against these bytes instead of hand-written layouts, so a Core
//! change to the manifest fails every parser that does not follow it.
use super::{PrivacyExact12CapabilityManifestBuildErrorV1, PrivacyExact12CapabilityManifestV1};
use crate::privacy::{
    ANONYMOUS_PGC_MAX_RECIPIENTS_V1, AnonymousPgcLimitsV1, IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
    IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1, IvmPrivateNoteLimitsV1,
    PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1, PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1,
    PrivacyCapabilityRowV1, PrivacyCapabilitySnapshotV1, PrivacyCompiledProfileCatalogRowV1,
    PrivacyCompiledProfileCatalogV1, PrivacyCompiledProfileCatalogValidationErrorV1,
    PrivacyCompiledProfileResultV1, PrivacyCompiledProfileSnapshotV1,
    PrivacyCompiledProfileUnavailableReasonV1, PrivacyCompiledStatementSchemaErrorV1,
    PrivacyConsensusLimitsV1, PrivacyConsensusPolicyUpdateV1, PrivacyConsensusPolicyV1,
    PrivacyExact12FixtureErrorV1, PrivacyProtocolLimitsUpdateV1, PrivacyProtocolPolicyV1,
    PrivacyRuntimeProtocolIdV1, PrivacyRuntimeProtocolLimitsV1, exact12_fixture::network_id,
    privacy_action_role_samples_v1,
};
use thiserror::Error;

/// Core's serialization of one committed capability manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivacyExact12CapabilityManifestFixtureV1 {
    /// Canonical catalog whose rows equal the compiled profiles of the manifest.
    pub compiled_profile_catalog_norito: Vec<u8>,
    /// Canonical manifest archive, as `GET /v1/privacy/capabilities` serves it.
    pub manifest_norito: Vec<u8>,
    /// Pretty JSON of the manifest, the projection the native SDK bridges emit compactly.
    pub manifest_json: String,
}

/// Failure building [`PrivacyExact12CapabilityManifestFixtureV1`].
#[derive(Debug, Error)]
pub enum PrivacyExact12CapabilityManifestFixtureErrorV1 {
    /// A structural sample profile could not be derived.
    #[error("capability manifest fixture sample failed: {0}")]
    Sample(#[from] PrivacyExact12FixtureErrorV1),
    /// A sample lost its protocol, so the manifest would miss a canonical row.
    #[error("capability manifest fixture has no sample profile for {0:?}")]
    MissingSample(PrivacyRuntimeProtocolIdV1),
    /// Core refused the snapshot or could not encode the manifest.
    #[error("capability manifest fixture failed: {0}")]
    Manifest(#[from] PrivacyExact12CapabilityManifestBuildErrorV1),
    /// Core refused the compiled-profile catalog.
    #[error("capability manifest fixture catalog is invalid: {0}")]
    Catalog(#[from] PrivacyCompiledProfileCatalogValidationErrorV1),
    /// Canonical catalog encoding failed.
    #[error("capability manifest fixture catalog encoding failed: {0}")]
    CatalogEncoding(#[from] norito::Error),
    /// JSON projection failed.
    #[error("capability manifest fixture JSON projection failed: {0}")]
    Json(#[from] norito::json::Error),
}

/// Build the capability-manifest conformance fixture from typed Rust values.
///
/// The manifest schedules consensus and protocol resource-limit updates. Its rows mix available
/// compiled profiles with every unavailable reason and with a fixed-limit protocol's policy.
/// Private-settlement validity belongs to its separate committed policy and is not part of this
/// manifest's limits.
///
/// # Errors
///
/// Returns an error if the samples cannot be built or Core refuses the resulting values.
pub fn privacy_exact12_capability_manifest_fixture_v1()
-> Result<PrivacyExact12CapabilityManifestFixtureV1, PrivacyExact12CapabilityManifestFixtureErrorV1>
{
    let current_limits = PrivacyConsensusLimitsV1::taira_default();
    let scheduled_at_height = 2;
    let effective_at_height = scheduled_at_height + 2;
    let mut next_limits = current_limits;
    next_limits.retained_root_count /= 2;

    let samples = privacy_action_role_samples_v1()?;
    let protocols = PrivacyRuntimeProtocolIdV1::ALL
        .into_iter()
        .map(|protocol_id| {
            let sample = samples
                .iter()
                .find(|sample| sample.profile.protocol_id == protocol_id)
                .ok_or(
                    PrivacyExact12CapabilityManifestFixtureErrorV1::MissingSample(protocol_id),
                )?;
            Ok(PrivacyCapabilityRowV1 {
                protocol_id,
                compiled_profile: compiled_profile(protocol_id, sample.profile),
                policy: protocol_policy(protocol_id, scheduled_at_height, effective_at_height),
            })
        })
        .collect::<Result<Vec<_>, PrivacyExact12CapabilityManifestFixtureErrorV1>>()?;

    let catalog = PrivacyCompiledProfileCatalogV1 {
        version: PRIVACY_COMPILED_PROFILE_CATALOG_VERSION_V1,
        protocols: protocols
            .iter()
            .map(|row| PrivacyCompiledProfileCatalogRowV1 {
                protocol_id: row.protocol_id,
                compiled_profile: row.compiled_profile,
            })
            .collect(),
    };
    catalog.validate()?;
    let manifest: PrivacyExact12CapabilityManifestV1 = PrivacyCapabilitySnapshotV1 {
        version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
        network_id: network_id(0x91),
        committed_height: scheduled_at_height + 1,
        consensus_policy: PrivacyConsensusPolicyV1 {
            current_limits,
            pending_update: Some(PrivacyConsensusPolicyUpdateV1 {
                scheduled_at_height,
                effective_at_height,
                next_limits,
            }),
        },
        protocols,
    }
    .exact12_capability_manifest_v1()?;
    Ok(PrivacyExact12CapabilityManifestFixtureV1 {
        compiled_profile_catalog_norito: norito::encode_canonical(&catalog)?,
        manifest_norito: manifest.canonical_bytes()?,
        manifest_json: norito::json::to_json_pretty(&manifest)?,
    })
}

/// One row per unavailable reason, so every reason's projection is Core's own.
fn compiled_profile(
    protocol_id: PrivacyRuntimeProtocolIdV1,
    available: PrivacyCompiledProfileSnapshotV1,
) -> PrivacyCompiledProfileResultV1 {
    use PrivacyCompiledProfileUnavailableReasonV1 as Reason;
    let reason = match protocol_id {
        PrivacyRuntimeProtocolIdV1::IrohaZkX509StarkP256V1 => {
            Reason::JointSecurityReductionIncomplete
        }
        PrivacyRuntimeProtocolIdV1::MoneroFcmpPlusPlusV1 => Reason::EngineUnavailable,
        PrivacyRuntimeProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
            Reason::ProfileInitializationFailed
        }
        PrivacyRuntimeProtocolIdV1::VegaExistingCredentialZkV1 => Reason::StatementSchemaInvalid(
            PrivacyCompiledStatementSchemaErrorV1::MissingTypeReference,
        ),
        _ => return PrivacyCompiledProfileResultV1::Available(available),
    };
    PrivacyCompiledProfileResultV1::Unavailable(reason)
}

fn protocol_policy(
    protocol_id: PrivacyRuntimeProtocolIdV1,
    scheduled_at_height: u64,
    effective_at_height: u64,
) -> Option<PrivacyProtocolPolicyV1> {
    let next_limits = match protocol_id {
        PrivacyRuntimeProtocolIdV1::IrohaZkX509StarkP256V1 => {
            return Some(PrivacyProtocolPolicyV1::canonical_default_v1(protocol_id));
        }
        PrivacyRuntimeProtocolIdV1::AnonymousPgcKOutOfNV1 => {
            PrivacyRuntimeProtocolLimitsV1::AnonymousPgcKOutOfNV1(AnonymousPgcLimitsV1 {
                max_anonymity_set_size: 32,
                max_recipient_count: ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
            })
        }
        PrivacyRuntimeProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
            PrivacyRuntimeProtocolLimitsV1::IrohaIvmPrivateNoteStarkV1(IvmPrivateNoteLimitsV1 {
                max_input_count: IVM_PRIVATE_NOTE_MAX_INPUTS_V1 - 1,
                max_output_count: IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
            })
        }
        _ => return None,
    };
    Some(PrivacyProtocolPolicyV1 {
        protocol_id,
        current_limits: PrivacyRuntimeProtocolLimitsV1::canonical_default_v1(protocol_id),
        pending_update: Some(PrivacyProtocolLimitsUpdateV1 {
            scheduled_at_height,
            effective_at_height,
            next_limits,
        }),
    })
}
