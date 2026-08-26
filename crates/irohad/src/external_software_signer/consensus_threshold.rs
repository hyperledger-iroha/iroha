//! Runtime-credential software custody for consensus threshold-signing broker slots.
//!
//! The standard broker reads two fixed supervisor credential names only when
//! the public catalog requests the corresponding slot. Credential envelopes
//! bind the exact provider qualification and genesis-derived network identity
//! to complete public DKG transcripts plus zeroizing scalar triples. Startup
//! replays Core's cryptographic import checks before exposing either backend.
//!
//! This is ordinary process-local software custody. It does not claim
//! proactive refresh, post-quantum security, or tolerance of process-memory
//! compromise. Rotation is a supervisor generation change: publish a bumped
//! public provider-catalog revision with the matching replacement credential
//! inventory, then restart the broker so the old process and shares are gone.
//! There is no hot reload, and rolling back both catalog and credential rolls
//! back this operational retirement. Core's committed-state retirement gates
//! remain the authority for deciding when an old share may be removed.

use super::unix::{SoftwareSignerCredentialErrorV1, load_bounded_software_signer_credential_v1};
use crate::{
    ConsensusSignerProviderQualificationV1, GlobalBeaconPartialSignerBrokerBackendErrorV1,
    GlobalBeaconPartialSignerBrokerBackendV1, IrohaRuntimeProviderBindingV1,
    IrohaRuntimeProviderBindingsV1, IrohaRuntimeProviderRegistryErrorV1,
    IrohaRuntimeProviderSlotV1, ParliamentTlePartialReleaseSignerBrokerBackendErrorV1,
    ParliamentTlePartialReleaseSignerBrokerBackendV1, RuntimeProviderBrokerBackendRegistryV1,
    RuntimeProviderBrokerBackendsV1,
};
use iroha_core::{
    beacon::{
        GlobalThresholdBeaconPartialSignerV1 as _, GlobalThresholdBeaconSessionBindingV1,
        RuntimeGlobalThresholdBeaconShareCustodyV1, ValidatedGlobalThresholdBeaconSessionV1,
    },
    tle_release::{
        RuntimeTleReleaseShareCustodyV1, TleKeySessionPublicStateV1, TlePartialReleaseShareV1,
        TleProjectedPartialReleaseSignerV1 as _, ValidatedTleReleaseProjectionV1,
    },
};
use iroha_data_model::{
    NetworkId,
    consensus::{GlobalThresholdBeaconKeySessionV1, GlobalThresholdBeaconPartialSignatureV1},
};
use norito::{DecodeLimits, NoritoDeserialize, NoritoSerialize};
use sha2::{Digest as _, Sha256};
use std::{fmt, io::Read as _, path::Path, sync::Arc};
use zeroize::{Zeroize as _, Zeroizing};

/// Fixed supervisor credential containing global-beacon software shares.
pub const GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1: &str =
    "iroha-global-beacon-partial-signer-v1.norito";
/// Fixed supervisor credential containing Parliament TLE release shares.
pub const PARLIAMENT_TLE_PARTIAL_RELEASE_SIGNER_CREDENTIAL_NAME_V1: &str =
    "iroha-parliament-tle-partial-release-signer-v1.norito";

const CONSENSUS_THRESHOLD_CREDENTIAL_MAGIC_V1: [u8; 8] = *b"IRTHR001";
const CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1: u16 = 1;
const CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_MAGIC_V1: [u8; 8] = *b"IRTHB001";
const CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_HEADER_BYTES_V1: usize = 28;
const CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_GLOBAL_BEACON_V1: u16 = 1 << 0;
const CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_PARLIAMENT_TLE_V1: u16 = 1 << 1;
const CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_KNOWN_FLAGS_V1: u16 =
    CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_GLOBAL_BEACON_V1
        | CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_PARLIAMENT_TLE_V1;
const CONSENSUS_THRESHOLD_PUBLIC_INVENTORY_DOMAIN_V1: &[u8] =
    b"iroha.runtime-consensus-threshold.public-inventory.v1";
#[cfg(test)]
const GLOBAL_BEACON_SIGNER_CREDENTIAL_SCHEMA_NAME_V1: &str =
    "iroha.runtime_provider_broker.v1.consensus_threshold.global_beacon_signer_credential";
#[cfg(test)]
const PARLIAMENT_TLE_SIGNER_CREDENTIAL_SCHEMA_NAME_V1: &str =
    "iroha.runtime_provider_broker.v1.consensus_threshold.parliament_tle_signer_credential";
#[cfg(test)]
const GLOBAL_BEACON_PUBLIC_INVENTORY_SCHEMA_NAME_V1: &str =
    "iroha.runtime_provider_broker.v1.consensus_threshold.global_beacon_public_inventory";
#[cfg(test)]
const PARLIAMENT_TLE_PUBLIC_INVENTORY_SCHEMA_NAME_V1: &str =
    "iroha.runtime_provider_broker.v1.consensus_threshold.parliament_tle_public_inventory";
const MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1: usize = 16 * 1024 * 1024;
const MAX_CONSENSUS_THRESHOLD_CREDENTIAL_SESSIONS_V1: usize = 64;
const CONSENSUS_THRESHOLD_CREDENTIAL_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    16_384,
    MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1,
    2_000_000,
    MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1,
    64,
);

/// Payload-free runtime threshold-signer credential failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeConsensusThresholdSignerCredentialErrorV1 {
    /// The public qualification, network, session inventory, or share was invalid.
    Rejected,
    /// The fixed runtime credential could not be securely opened or read.
    Unavailable,
    /// Canonical credential encoding failed or exceeded its fixed byte ceiling.
    Encoding,
}

impl fmt::Display for RuntimeConsensusThresholdSignerCredentialErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Rejected => "consensus threshold-signer runtime credential was rejected",
            Self::Unavailable => "consensus threshold-signer runtime credential is unavailable",
            Self::Encoding => "consensus threshold-signer runtime credential encoding failed",
        })
    }
}

impl std::error::Error for RuntimeConsensusThresholdSignerCredentialErrorV1 {}

/// One global-beacon share supplied by an authenticated DKG provisioning path.
///
/// The type has no serialization, cloning, or debug surface. Use
/// [`encode_global_beacon_partial_signer_credential_v1`] to produce the
/// zeroizing bytes handed directly to a supervisor credential facility.
pub struct RuntimeGlobalBeaconShareProvisioningV1 {
    public_session: GlobalThresholdBeaconKeySessionV1,
    signer_index: u16,
    components: Zeroizing<[[u8; 32]; 3]>,
}

impl RuntimeGlobalBeaconShareProvisioningV1 {
    /// Consume one public transcript and its zeroizing aggregate share.
    #[must_use]
    pub fn new(
        public_session: GlobalThresholdBeaconKeySessionV1,
        signer_index: u16,
        components: Zeroizing<[[u8; 32]; 3]>,
    ) -> Self {
        Self {
            public_session,
            signer_index,
            components,
        }
    }
}

/// One Parliament TLE share supplied by an authenticated DKG provisioning path.
///
/// The type has no serialization, cloning, or debug surface. Use
/// [`encode_parliament_tle_partial_release_signer_credential_v1`] to produce
/// the zeroizing bytes handed directly to a supervisor credential facility.
pub struct RuntimeParliamentTleShareProvisioningV1 {
    public_session: TleKeySessionPublicStateV1,
    participant_index: u16,
    components: Zeroizing<[[u8; 32]; 3]>,
}

impl RuntimeParliamentTleShareProvisioningV1 {
    /// Consume one public transcript and its zeroizing aggregate share.
    #[must_use]
    pub fn new(
        public_session: TleKeySessionPublicStateV1,
        participant_index: u16,
        components: Zeroizing<[[u8; 32]; 3]>,
    ) -> Self {
        Self {
            public_session,
            participant_index,
            components,
        }
    }
}

#[derive(NoritoSerialize, NoritoDeserialize)]
#[norito(schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.credential_header")]
struct RuntimeConsensusThresholdCredentialHeaderWireV1 {
    magic: [u8; 8],
    version: u16,
    slot: u16,
    network_id: NetworkId,
    handle: String,
    revision: u64,
    policy_digest: [u8; 32],
}

#[derive(NoritoSerialize, NoritoDeserialize)]
#[norito(schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.secret_scalar_triple")]
struct RuntimeSecretScalarTripleWireV1([[u8; 32]; 3]);

impl RuntimeSecretScalarTripleWireV1 {
    fn from_zeroizing(mut components: Zeroizing<[[u8; 32]; 3]>) -> Self {
        Self(std::mem::take(&mut *components))
    }

    fn into_zeroizing(mut self) -> Zeroizing<[[u8; 32]; 3]> {
        Zeroizing::new(std::mem::take(&mut self.0))
    }
}

impl Drop for RuntimeSecretScalarTripleWireV1 {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(NoritoSerialize, NoritoDeserialize)]
#[norito(
    schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.global_beacon_share_credential"
)]
struct RuntimeGlobalBeaconShareCredentialWireV1 {
    public_session: GlobalThresholdBeaconKeySessionV1,
    signer_index: u16,
    components: RuntimeSecretScalarTripleWireV1,
}

#[derive(NoritoSerialize, NoritoDeserialize)]
#[norito(
    schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.global_beacon_signer_credential"
)]
struct RuntimeGlobalBeaconSignerCredentialWireV1 {
    header: RuntimeConsensusThresholdCredentialHeaderWireV1,
    sessions: Vec<RuntimeGlobalBeaconShareCredentialWireV1>,
}

#[derive(NoritoSerialize, NoritoDeserialize)]
#[norito(
    schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.parliament_tle_share_credential"
)]
struct RuntimeParliamentTleShareCredentialWireV1 {
    public_session: TleKeySessionPublicStateV1,
    participant_index: u16,
    components: RuntimeSecretScalarTripleWireV1,
}

#[derive(NoritoSerialize, NoritoDeserialize)]
#[norito(
    schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.parliament_tle_signer_credential"
)]
struct RuntimeParliamentTleSignerCredentialWireV1 {
    header: RuntimeConsensusThresholdCredentialHeaderWireV1,
    sessions: Vec<RuntimeParliamentTleShareCredentialWireV1>,
}

#[derive(NoritoSerialize)]
#[norito(
    schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.global_beacon_public_inventory_entry"
)]
struct RuntimeGlobalBeaconPublicInventoryEntryWireV1 {
    public_session: GlobalThresholdBeaconKeySessionV1,
    signer_index: u16,
}

#[derive(NoritoSerialize)]
#[norito(
    schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.global_beacon_public_inventory"
)]
struct RuntimeGlobalBeaconPublicInventoryWireV1 {
    version: u16,
    slot: u16,
    network_id: NetworkId,
    sessions: Vec<RuntimeGlobalBeaconPublicInventoryEntryWireV1>,
}

#[derive(NoritoSerialize)]
#[norito(
    schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.parliament_tle_public_inventory_entry"
)]
struct RuntimeParliamentTlePublicInventoryEntryWireV1 {
    public_session: TleKeySessionPublicStateV1,
    participant_index: u16,
}

#[derive(NoritoSerialize)]
#[norito(
    schema_name = "iroha.runtime_provider_broker.v1.consensus_threshold.parliament_tle_public_inventory"
)]
struct RuntimeParliamentTlePublicInventoryWireV1 {
    version: u16,
    slot: u16,
    network_id: NetworkId,
    sessions: Vec<RuntimeParliamentTlePublicInventoryEntryWireV1>,
}

fn canonical_public_inventory_digest_v1<T: NoritoSerialize>(
    inventory: &T,
) -> Result<[u8; 32], RuntimeConsensusThresholdSignerCredentialErrorV1> {
    let encoded = norito::encode_canonical(inventory)
        .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding)?;
    if encoded.is_empty() || encoded.len() > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1 {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding);
    }
    let encoded_len = u64::try_from(encoded.len())
        .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding)?;
    let mut hasher = Sha256::new();
    hasher.update(CONSENSUS_THRESHOLD_PUBLIC_INVENTORY_DOMAIN_V1);
    hasher.update(encoded_len.to_be_bytes());
    hasher.update(encoded);
    let digest: [u8; 32] = hasher.finalize().into();
    if digest == [0; 32] {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    Ok(digest)
}

fn global_beacon_public_inventory_wire_v1(
    network_id: NetworkId,
    sessions: impl IntoIterator<Item = (GlobalThresholdBeaconKeySessionV1, u16)>,
) -> Result<
    RuntimeGlobalBeaconPublicInventoryWireV1,
    RuntimeConsensusThresholdSignerCredentialErrorV1,
> {
    let mut sessions = sessions
        .into_iter()
        .map(|(public_session, signer_index)| {
            if public_session.network_id != network_id {
                return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
            }
            Ok(RuntimeGlobalBeaconPublicInventoryEntryWireV1 {
                public_session,
                signer_index,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_session_count_v1(sessions.len())?;
    sessions.sort_by(|left, right| {
        left.public_session
            .session_id
            .cmp(&right.public_session.session_id)
            .then_with(|| left.signer_index.cmp(&right.signer_index))
    });
    Ok(RuntimeGlobalBeaconPublicInventoryWireV1 {
        version: CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1,
        slot: IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner.wire_id(),
        network_id,
        sessions,
    })
}

fn parliament_tle_public_inventory_wire_v1(
    network_id: NetworkId,
    sessions: impl IntoIterator<Item = (TleKeySessionPublicStateV1, u16)>,
) -> Result<
    RuntimeParliamentTlePublicInventoryWireV1,
    RuntimeConsensusThresholdSignerCredentialErrorV1,
> {
    let mut sessions = sessions
        .into_iter()
        .map(|(public_session, participant_index)| {
            if public_session.network_id != *network_id.as_bytes() {
                return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
            }
            Ok(RuntimeParliamentTlePublicInventoryEntryWireV1 {
                public_session,
                participant_index,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_session_count_v1(sessions.len())?;
    sessions.sort_by(|left, right| {
        left.public_session
            .key_session_id
            .cmp(&right.public_session.key_session_id)
            .then_with(|| left.participant_index.cmp(&right.participant_index))
    });
    Ok(RuntimeParliamentTlePublicInventoryWireV1 {
        version: CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1,
        slot: IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner.wire_id(),
        network_id,
        sessions,
    })
}

/// Compute the canonical public global-beacon session-and-seat inventory digest.
///
/// This digest is the exact value configured as the provider policy digest. It
/// commits to the complete public DKG transcript for every provisioned session
/// and to the local signer seat, but never serializes or hashes private share
/// components. V1 computes
/// `SHA-256(domain || u64_be(encoded_len) || canonical_norito(inventory))`,
/// where the inventory contains version 1, the role slot, the exact network,
/// and entries sorted by session identifier then signer index.
///
/// # Errors
///
/// Rejects empty, excessive, or cross-network inventories and encoding failure.
pub fn global_beacon_partial_signer_inventory_digest_v1(
    network_id: NetworkId,
    sessions: &[RuntimeGlobalBeaconShareProvisioningV1],
) -> Result<[u8; 32], RuntimeConsensusThresholdSignerCredentialErrorV1> {
    let inventory = global_beacon_public_inventory_wire_v1(
        network_id,
        sessions
            .iter()
            .map(|session| (session.public_session.clone(), session.signer_index)),
    )?;
    canonical_public_inventory_digest_v1(&inventory)
}

/// Compute the canonical public Parliament-TLE session-and-seat inventory digest.
///
/// This digest is the exact value configured as the provider policy digest. It
/// commits to every complete public TLE transcript and the local participant
/// seat without serializing or hashing private share components. V1 computes
/// `SHA-256(domain || u64_be(encoded_len) || canonical_norito(inventory))`,
/// where the inventory contains version 1, the role slot, the exact network,
/// and entries sorted by key-session identifier then participant index.
///
/// # Errors
///
/// Rejects empty, excessive, or cross-network inventories and encoding failure.
pub fn parliament_tle_partial_release_signer_inventory_digest_v1(
    network_id: NetworkId,
    sessions: &[RuntimeParliamentTleShareProvisioningV1],
) -> Result<[u8; 32], RuntimeConsensusThresholdSignerCredentialErrorV1> {
    let inventory = parliament_tle_public_inventory_wire_v1(
        network_id,
        sessions
            .iter()
            .map(|session| (session.public_session.clone(), session.participant_index)),
    )?;
    canonical_public_inventory_digest_v1(&inventory)
}

/// Frame the two optional threshold credentials for one launchd stdin handoff.
///
/// The bundle contains only a fixed header and the already canonical secret
/// credential frames. Its allocation is reserved once and zeroized on drop.
/// A deployment-owned administrator writes the returned bytes once to the
/// root-protected launchd FIFO; neither secret bytes nor source paths enter the
/// broker argv, environment, plist, public catalog, or filesystem namespace.
///
/// # Errors
///
/// Rejects empty or oversized credential frames and total-length overflow.
pub fn encode_consensus_threshold_credential_bundle_v1(
    global_beacon: Option<&[u8]>,
    parliament_tle: Option<&[u8]>,
) -> Result<Zeroizing<Vec<u8>>, RuntimeConsensusThresholdSignerCredentialErrorV1> {
    fn credential_len_v1(
        credential: Option<&[u8]>,
    ) -> Result<usize, RuntimeConsensusThresholdSignerCredentialErrorV1> {
        let Some(credential) = credential else {
            return Ok(0);
        };
        if credential.is_empty() || credential.len() > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1 {
            return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
        }
        Ok(credential.len())
    }

    let global_len = credential_len_v1(global_beacon)?;
    let tle_len = credential_len_v1(parliament_tle)?;
    let encoded_len = CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_HEADER_BYTES_V1
        .checked_add(global_len)
        .and_then(|len| len.checked_add(tle_len))
        .ok_or(RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding)?;
    let mut allocation = Vec::new();
    allocation
        .try_reserve_exact(encoded_len)
        .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding)?;
    let mut encoded = Zeroizing::new(allocation);
    let mut flags = 0_u16;
    if global_beacon.is_some() {
        flags |= CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_GLOBAL_BEACON_V1;
    }
    if parliament_tle.is_some() {
        flags |= CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_PARLIAMENT_TLE_V1;
    }
    encoded.extend_from_slice(&CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_MAGIC_V1);
    encoded.extend_from_slice(&CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1.to_be_bytes());
    encoded.extend_from_slice(&flags.to_be_bytes());
    encoded.extend_from_slice(
        &u64::try_from(global_len)
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding)?
            .to_be_bytes(),
    );
    encoded.extend_from_slice(
        &u64::try_from(tle_len)
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding)?
            .to_be_bytes(),
    );
    if let Some(credential) = global_beacon {
        encoded.extend_from_slice(credential);
    }
    if let Some(credential) = parliament_tle {
        encoded.extend_from_slice(credential);
    }
    debug_assert_eq!(encoded.len(), encoded_len);
    Ok(encoded)
}

/// Canonically encode a cryptographically validated global-beacon share inventory.
///
/// The returned allocation scrubs itself on drop and is intended to be passed
/// directly to the supervisor credential facility. This function performs no
/// file I/O and never writes private material to configuration or ledger state.
///
/// # Errors
///
/// Rejects invalid production qualification, empty or excessive inventories,
/// cross-network transcripts, duplicate sessions, and shares that do not match
/// the complete public DKG transcript and signer seat.
pub fn encode_global_beacon_partial_signer_credential_v1(
    network_id: NetworkId,
    handle: impl Into<String>,
    revision: u64,
    policy_digest: [u8; 32],
    sessions: Vec<RuntimeGlobalBeaconShareProvisioningV1>,
) -> Result<Zeroizing<Vec<u8>>, RuntimeConsensusThresholdSignerCredentialErrorV1> {
    let handle = handle.into();
    if global_beacon_partial_signer_inventory_digest_v1(network_id, &sessions)? != policy_digest {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    validate_provisioning_header_v1(&network_id, &handle, revision, policy_digest, |handle| {
        let header = credential_header_v1(
            IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
            network_id,
            handle,
            revision,
            policy_digest,
        );
        let sessions = encode_global_beacon_sessions_v1(&network_id, sessions)?;
        encode_secret_credential_v1(&RuntimeGlobalBeaconSignerCredentialWireV1 { header, sessions })
    })
}

fn encode_global_beacon_sessions_v1(
    network_id: &NetworkId,
    mut sessions: Vec<RuntimeGlobalBeaconShareProvisioningV1>,
) -> Result<
    Vec<RuntimeGlobalBeaconShareCredentialWireV1>,
    RuntimeConsensusThresholdSignerCredentialErrorV1,
> {
    validate_session_count_v1(sessions.len())?;
    sessions.sort_by(|left, right| {
        left.public_session
            .session_id
            .cmp(&right.public_session.session_id)
            .then_with(|| left.signer_index.cmp(&right.signer_index))
    });
    let validation_custody = RuntimeGlobalThresholdBeaconShareCustodyV1::new();
    let mut encoded = Vec::with_capacity(sessions.len());
    for session in sessions {
        let RuntimeGlobalBeaconShareProvisioningV1 {
            public_session,
            signer_index,
            components,
        } = session;
        if public_session.network_id != *network_id {
            return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
        }
        let binding = beacon_binding_v1(&public_session);
        validation_custody
            .import_components(
                public_session.clone(),
                &binding,
                signer_index,
                Zeroizing::new(*components),
            )
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
        encoded.push(RuntimeGlobalBeaconShareCredentialWireV1 {
            public_session,
            signer_index,
            components: RuntimeSecretScalarTripleWireV1::from_zeroizing(components),
        });
    }
    Ok(encoded)
}

/// Canonically encode a cryptographically validated Parliament TLE share inventory.
///
/// The returned allocation scrubs itself on drop and is intended to be passed
/// directly to the supervisor credential facility. This function performs no
/// file I/O and never writes private material to configuration or ledger state.
///
/// # Errors
///
/// Rejects invalid production qualification, empty or excessive inventories,
/// cross-network transcripts, duplicate sessions, and shares that do not match
/// the complete public DKG transcript and participant seat.
pub fn encode_parliament_tle_partial_release_signer_credential_v1(
    network_id: NetworkId,
    handle: impl Into<String>,
    revision: u64,
    policy_digest: [u8; 32],
    sessions: Vec<RuntimeParliamentTleShareProvisioningV1>,
) -> Result<Zeroizing<Vec<u8>>, RuntimeConsensusThresholdSignerCredentialErrorV1> {
    let handle = handle.into();
    if parliament_tle_partial_release_signer_inventory_digest_v1(network_id, &sessions)?
        != policy_digest
    {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    validate_provisioning_header_v1(&network_id, &handle, revision, policy_digest, |handle| {
        let header = credential_header_v1(
            IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner,
            network_id,
            handle,
            revision,
            policy_digest,
        );
        let sessions = encode_parliament_tle_sessions_v1(&network_id, sessions)?;
        encode_secret_credential_v1(&RuntimeParliamentTleSignerCredentialWireV1 {
            header,
            sessions,
        })
    })
}

fn encode_parliament_tle_sessions_v1(
    network_id: &NetworkId,
    mut sessions: Vec<RuntimeParliamentTleShareProvisioningV1>,
) -> Result<
    Vec<RuntimeParliamentTleShareCredentialWireV1>,
    RuntimeConsensusThresholdSignerCredentialErrorV1,
> {
    validate_session_count_v1(sessions.len())?;
    sessions.sort_by(|left, right| {
        left.public_session
            .key_session_id
            .cmp(&right.public_session.key_session_id)
            .then_with(|| left.participant_index.cmp(&right.participant_index))
    });
    let validation_custody = RuntimeTleReleaseShareCustodyV1::new();
    let mut encoded = Vec::with_capacity(sessions.len());
    for session in sessions {
        let RuntimeParliamentTleShareProvisioningV1 {
            public_session,
            participant_index,
            components,
        } = session;
        if public_session.network_id != *network_id.as_bytes() {
            return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
        }
        validation_custody
            .import_components(
                public_session.clone(),
                participant_index,
                Zeroizing::new(*components),
            )
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
        encoded.push(RuntimeParliamentTleShareCredentialWireV1 {
            public_session,
            participant_index,
            components: RuntimeSecretScalarTripleWireV1::from_zeroizing(components),
        });
    }
    Ok(encoded)
}

fn validate_provisioning_header_v1<T>(
    network_id: &NetworkId,
    handle: &str,
    revision: u64,
    policy_digest: [u8; 32],
    build: impl FnOnce(String) -> Result<T, RuntimeConsensusThresholdSignerCredentialErrorV1>,
) -> Result<T, RuntimeConsensusThresholdSignerCredentialErrorV1> {
    if network_id.as_bytes().iter().all(|byte| *byte == 0)
        || iroha_config::parameters::validate_production_runtime_handle(handle).is_err()
        || revision == 0
        || policy_digest == [0; 32]
    {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    build(handle.to_owned())
}

fn credential_header_v1(
    slot: IrohaRuntimeProviderSlotV1,
    network_id: NetworkId,
    handle: String,
    revision: u64,
    policy_digest: [u8; 32],
) -> RuntimeConsensusThresholdCredentialHeaderWireV1 {
    RuntimeConsensusThresholdCredentialHeaderWireV1 {
        magic: CONSENSUS_THRESHOLD_CREDENTIAL_MAGIC_V1,
        version: CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1,
        slot: slot.wire_id(),
        network_id,
        handle,
        revision,
        policy_digest,
    }
}

fn encode_secret_credential_v1<T: NoritoSerialize>(
    wire: &T,
) -> Result<Zeroizing<Vec<u8>>, RuntimeConsensusThresholdSignerCredentialErrorV1> {
    let encoded = Zeroizing::new(
        norito::core::to_bytes_bounded(wire, MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1)
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding)?,
    );
    if encoded.is_empty() {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding);
    }
    Ok(encoded)
}

fn validate_session_count_v1(
    count: usize,
) -> Result<(), RuntimeConsensusThresholdSignerCredentialErrorV1> {
    if count == 0 || count > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_SESSIONS_V1 {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    Ok(())
}

fn beacon_binding_v1(
    record: &GlobalThresholdBeaconKeySessionV1,
) -> GlobalThresholdBeaconSessionBindingV1 {
    GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.network_id,
        session_id: record.session_id,
        roster_hash: record.roster_hash,
        transcript_hash: record.transcript_hash,
    }
}

/// Exact two-slot backend registry populated from runtime credentials.
///
/// The registry owns only zeroizing Core custody objects plus stable public
/// qualification. It exposes no share inventory or secret-export operation.
#[derive(Clone, Default)]
pub struct RuntimeConsensusThresholdSignerBackendsV1 {
    global_beacon: Option<Arc<RuntimeGlobalBeaconPartialSignerBackendV1>>,
    parliament_tle: Option<Arc<RuntimeParliamentTlePartialReleaseSignerBackendV1>>,
}

impl RuntimeConsensusThresholdSignerBackendsV1 {
    /// Construct an empty exact registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            global_beacon: None,
            parliament_tle: None,
        }
    }

    /// Consume one exact launchd stdin credential bundle.
    ///
    /// launchd opens a root-protected FIFO before changing to the broker UID.
    /// The independent administrator writes one bundle and closes the writer;
    /// this reader never resolves a secret pathname. Bundle presence must
    /// exactly match the two threshold slots requested by the public catalog.
    ///
    /// # Errors
    ///
    /// Rejects a missing, truncated, trailing, oversized, unknown-version, or
    /// catalog-mismatched bundle and applies the same complete credential and
    /// public-transcript validation as the systemd credential path.
    pub fn load_from_launchd_credential_bundle_v1(
        catalog: &IrohaRuntimeProviderBindingsV1,
        reader: &mut impl std::io::Read,
    ) -> Result<Self, RuntimeConsensusThresholdSignerCredentialErrorV1> {
        let mut requested_flags = 0_u16;
        for configured in catalog.iter() {
            let flag = match configured.slot() {
                IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner => {
                    CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_GLOBAL_BEACON_V1
                }
                IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner => {
                    CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_PARLIAMENT_TLE_V1
                }
                _ => continue,
            };
            if requested_flags & flag != 0 {
                return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
            }
            requested_flags |= flag;
        }

        let mut header = [0_u8; CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_HEADER_BYTES_V1];
        reader
            .read_exact(&mut header)
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)?;
        let version = u16::from_be_bytes([header[8], header[9]]);
        let flags = u16::from_be_bytes([header[10], header[11]]);
        let global_len = usize::try_from(u64::from_be_bytes(
            header[12..20]
                .try_into()
                .expect("fixed credential-bundle global length"),
        ))
        .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
        let tle_len = usize::try_from(u64::from_be_bytes(
            header[20..28]
                .try_into()
                .expect("fixed credential-bundle TLE length"),
        ))
        .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
        let global_present = flags & CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_GLOBAL_BEACON_V1 != 0;
        let tle_present = flags & CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_PARLIAMENT_TLE_V1 != 0;
        if header[..8] != CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_MAGIC_V1
            || version != CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1
            || flags & !CONSENSUS_THRESHOLD_CREDENTIAL_BUNDLE_KNOWN_FLAGS_V1 != 0
            || flags != requested_flags
            || global_present != (global_len != 0)
            || tle_present != (tle_len != 0)
            || global_len > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1
            || tle_len > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1
        {
            return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
        }
        let payload_len = global_len
            .checked_add(tle_len)
            .ok_or(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
        let mut allocation = Vec::new();
        allocation
            .try_reserve_exact(payload_len)
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)?;
        allocation.resize(payload_len, 0);
        let mut payload = Zeroizing::new(allocation);
        reader
            .read_exact(&mut payload)
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)?;
        let mut trailing = [0_u8; 1];
        let trailing_len = reader
            .read(&mut trailing)
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)?;
        trailing.zeroize();
        if trailing_len != 0 {
            return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
        }

        let (global_credential, tle_credential) = payload.split_at(global_len);
        let mut loaded = Self::new();
        for configured in catalog.iter() {
            match configured.slot() {
                IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner => {
                    loaded.global_beacon = Some(decode_global_beacon_credential_v1(
                        global_credential,
                        catalog.network_id(),
                        configured,
                    )?);
                }
                IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner => {
                    loaded.parliament_tle = Some(decode_parliament_tle_credential_v1(
                        tle_credential,
                        catalog.network_id(),
                        configured,
                    )?);
                }
                _ => {}
            }
        }
        Ok(loaded)
    }

    /// Load every requested threshold signer from its fixed runtime credential.
    ///
    /// `credential_directory` is normally the supervisor-provided
    /// `CREDENTIALS_DIRECTORY`. It may be absent only when the catalog requests
    /// neither threshold-signing slot. Credential file names are fixed by this
    /// module; no path or secret value is accepted through configuration.
    ///
    /// # Errors
    ///
    /// Rejects a missing or insecure credential, noncanonical encoding,
    /// catalog substitution, invalid public transcript, duplicate session, or
    /// private share that does not match its advertised participant seat.
    pub fn load_from_credential_directory_v1(
        catalog: &IrohaRuntimeProviderBindingsV1,
        credential_directory: Option<&Path>,
    ) -> Result<Self, RuntimeConsensusThresholdSignerCredentialErrorV1> {
        let mut loaded = Self::new();
        for configured in catalog.iter() {
            match configured.slot() {
                IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner => {
                    if loaded.global_beacon.is_some() {
                        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
                    }
                    let directory = credential_directory
                        .ok_or(RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)?;
                    let bytes = load_fixed_credential_v1(
                        directory,
                        GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1,
                    )?;
                    loaded.global_beacon = Some(decode_global_beacon_credential_v1(
                        &bytes,
                        catalog.network_id(),
                        configured,
                    )?);
                }
                IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner => {
                    if loaded.parliament_tle.is_some() {
                        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
                    }
                    let directory = credential_directory
                        .ok_or(RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)?;
                    let bytes = load_fixed_credential_v1(
                        directory,
                        PARLIAMENT_TLE_PARTIAL_RELEASE_SIGNER_CREDENTIAL_NAME_V1,
                    )?;
                    loaded.parliament_tle = Some(decode_parliament_tle_credential_v1(
                        &bytes,
                        catalog.network_id(),
                        configured,
                    )?);
                }
                _ => {}
            }
        }
        Ok(loaded)
    }
}

fn load_fixed_credential_v1(
    directory: &Path,
    name: &str,
) -> Result<Zeroizing<Vec<u8>>, RuntimeConsensusThresholdSignerCredentialErrorV1> {
    load_bounded_software_signer_credential_v1(
        &directory.join(name),
        1,
        MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1,
    )
    .map_err(|error| match error {
        SoftwareSignerCredentialErrorV1::Unavailable => {
            RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable
        }
        SoftwareSignerCredentialErrorV1::InvalidSource
        | SoftwareSignerCredentialErrorV1::InvalidLength => {
            RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected
        }
    })
}

fn decode_global_beacon_credential_v1(
    bytes: &[u8],
    network_id: &NetworkId,
    configured: &IrohaRuntimeProviderBindingV1,
) -> Result<
    Arc<RuntimeGlobalBeaconPartialSignerBackendV1>,
    RuntimeConsensusThresholdSignerCredentialErrorV1,
> {
    let wire: RuntimeGlobalBeaconSignerCredentialWireV1 = norito::decode_canonical_with_limits(
        bytes,
        CONSENSUS_THRESHOLD_CREDENTIAL_DECODE_LIMITS_V1,
    )
    .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
    let qualification = validate_credential_header_v1(
        &wire.header,
        IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
        network_id,
        configured,
    )?;
    validate_session_count_v1(wire.sessions.len())?;
    if wire.sessions.windows(2).any(|pair| {
        pair[0]
            .public_session
            .session_id
            .cmp(&pair[1].public_session.session_id)
            .then_with(|| pair[0].signer_index.cmp(&pair[1].signer_index))
            .is_ge()
    }) {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    let public_inventory = global_beacon_public_inventory_wire_v1(
        *network_id,
        wire.sessions
            .iter()
            .map(|session| (session.public_session.clone(), session.signer_index)),
    )?;
    if canonical_public_inventory_digest_v1(&public_inventory)? != qualification.policy_digest {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    let custody = Arc::new(RuntimeGlobalThresholdBeaconShareCustodyV1::new());
    for session in wire.sessions {
        if session.public_session.network_id != *network_id {
            return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
        }
        let binding = beacon_binding_v1(&session.public_session);
        custody
            .import_components(
                session.public_session,
                &binding,
                session.signer_index,
                session.components.into_zeroizing(),
            )
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
    }
    Ok(Arc::new(RuntimeGlobalBeaconPartialSignerBackendV1 {
        handle: wire.header.handle,
        qualification,
        custody,
    }))
}

fn decode_parliament_tle_credential_v1(
    bytes: &[u8],
    network_id: &NetworkId,
    configured: &IrohaRuntimeProviderBindingV1,
) -> Result<
    Arc<RuntimeParliamentTlePartialReleaseSignerBackendV1>,
    RuntimeConsensusThresholdSignerCredentialErrorV1,
> {
    let wire: RuntimeParliamentTleSignerCredentialWireV1 = norito::decode_canonical_with_limits(
        bytes,
        CONSENSUS_THRESHOLD_CREDENTIAL_DECODE_LIMITS_V1,
    )
    .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
    let qualification = validate_credential_header_v1(
        &wire.header,
        IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner,
        network_id,
        configured,
    )?;
    validate_session_count_v1(wire.sessions.len())?;
    if wire.sessions.windows(2).any(|pair| {
        pair[0]
            .public_session
            .key_session_id
            .cmp(&pair[1].public_session.key_session_id)
            .then_with(|| pair[0].participant_index.cmp(&pair[1].participant_index))
            .is_ge()
    }) {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    let public_inventory = parliament_tle_public_inventory_wire_v1(
        *network_id,
        wire.sessions
            .iter()
            .map(|session| (session.public_session.clone(), session.participant_index)),
    )?;
    if canonical_public_inventory_digest_v1(&public_inventory)? != qualification.policy_digest {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    let custody = Arc::new(RuntimeTleReleaseShareCustodyV1::new());
    for session in wire.sessions {
        if session.public_session.network_id != *network_id.as_bytes() {
            return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
        }
        custody
            .import_components(
                session.public_session,
                session.participant_index,
                session.components.into_zeroizing(),
            )
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)?;
    }
    Ok(Arc::new(
        RuntimeParliamentTlePartialReleaseSignerBackendV1 {
            handle: wire.header.handle,
            qualification,
            custody,
        },
    ))
}

fn validate_credential_header_v1(
    header: &RuntimeConsensusThresholdCredentialHeaderWireV1,
    slot: IrohaRuntimeProviderSlotV1,
    network_id: &NetworkId,
    configured: &IrohaRuntimeProviderBindingV1,
) -> Result<ConsensusSignerProviderQualificationV1, RuntimeConsensusThresholdSignerCredentialErrorV1>
{
    if header.magic != CONSENSUS_THRESHOLD_CREDENTIAL_MAGIC_V1
        || header.version != CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1
        || header.slot != slot.wire_id()
        || header.network_id != *network_id
        || configured.slot() != slot
        || configured.handle() != header.handle
        || configured.revision() != Some(header.revision)
        || configured.policy_digest() != Some(header.policy_digest)
        || header.revision == 0
        || header.policy_digest == [0; 32]
    {
        return Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected);
    }
    Ok(ConsensusSignerProviderQualificationV1::new(
        header.revision,
        header.policy_digest,
        false,
    ))
}

struct RuntimeGlobalBeaconPartialSignerBackendV1 {
    handle: String,
    qualification: ConsensusSignerProviderQualificationV1,
    custody: Arc<RuntimeGlobalThresholdBeaconShareCustodyV1>,
}

impl GlobalBeaconPartialSignerBrokerBackendV1 for RuntimeGlobalBeaconPartialSignerBackendV1 {
    fn handle(&self) -> &str {
        &self.handle
    }

    fn qualification(
        &self,
    ) -> Result<ConsensusSignerProviderQualificationV1, GlobalBeaconPartialSignerBrokerBackendErrorV1>
    {
        Ok(self.qualification)
    }

    fn sign_partial(
        &self,
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        payload: &[u8],
    ) -> Result<
        GlobalThresholdBeaconPartialSignatureV1,
        GlobalBeaconPartialSignerBrokerBackendErrorV1,
    > {
        self.custody
            .sign_partial(session, payload)
            .map_err(|_| GlobalBeaconPartialSignerBrokerBackendErrorV1::Rejected)
    }
}

struct RuntimeParliamentTlePartialReleaseSignerBackendV1 {
    handle: String,
    qualification: ConsensusSignerProviderQualificationV1,
    custody: Arc<RuntimeTleReleaseShareCustodyV1>,
}

impl ParliamentTlePartialReleaseSignerBrokerBackendV1
    for RuntimeParliamentTlePartialReleaseSignerBackendV1
{
    fn handle(&self) -> &str {
        &self.handle
    }

    fn qualification(
        &self,
    ) -> Result<
        ConsensusSignerProviderQualificationV1,
        ParliamentTlePartialReleaseSignerBrokerBackendErrorV1,
    > {
        Ok(self.qualification)
    }

    fn sign_projected_partial_release(
        &self,
        projection: &ValidatedTleReleaseProjectionV1,
    ) -> Result<TlePartialReleaseShareV1, ParliamentTlePartialReleaseSignerBrokerBackendErrorV1>
    {
        self.custody
            .sign_projected_partial_release(projection)
            .map_err(|_| ParliamentTlePartialReleaseSignerBrokerBackendErrorV1::Rejected)
    }
}

impl RuntimeProviderBrokerBackendRegistryV1 for RuntimeConsensusThresholdSignerBackendsV1 {
    fn resolve(
        &self,
        bindings: &IrohaRuntimeProviderBindingsV1,
    ) -> Result<RuntimeProviderBrokerBackendsV1, IrohaRuntimeProviderRegistryErrorV1> {
        let mut requested_global = false;
        let mut requested_tle = false;
        for configured in bindings.iter() {
            match configured.slot() {
                IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner => {
                    let backend = self
                        .global_beacon
                        .as_deref()
                        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
                    exact_backend_binding_v1(configured, backend.handle(), backend.qualification)?;
                    if requested_global {
                        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
                    }
                    requested_global = true;
                }
                IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner => {
                    let backend = self
                        .parliament_tle
                        .as_deref()
                        .ok_or(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution)?;
                    exact_backend_binding_v1(configured, backend.handle(), backend.qualification)?;
                    if requested_tle {
                        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
                    }
                    requested_tle = true;
                }
                _ => return Err(IrohaRuntimeProviderRegistryErrorV1::IncompleteResolution),
            }
        }
        if requested_global != self.global_beacon.is_some()
            || requested_tle != self.parliament_tle.is_some()
        {
            return Err(IrohaRuntimeProviderRegistryErrorV1::UnexpectedProviders);
        }
        let mut resolved = RuntimeProviderBrokerBackendsV1::new();
        if let Some(backend) = &self.global_beacon {
            resolved = resolved.with_global_beacon_partial_signer(backend.clone());
        }
        if let Some(backend) = &self.parliament_tle {
            resolved = resolved.with_parliament_tle_partial_release_signer(backend.clone());
        }
        Ok(resolved)
    }
}

fn exact_backend_binding_v1(
    configured: &IrohaRuntimeProviderBindingV1,
    handle: &str,
    qualification: ConsensusSignerProviderQualificationV1,
) -> Result<(), IrohaRuntimeProviderRegistryErrorV1> {
    if qualification.test_marked {
        return Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected);
    }
    if configured.handle() != handle
        || configured.revision() != Some(qualification.revision)
        || configured.policy_digest() != Some(qualification.policy_digest)
    {
        return Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch);
    }
    Ok(())
}

/// Direct credential tests and proof-valid fixtures shared with broker socket tests.
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::external_software_signer::ExternalSoftwareSignerBackendsV1;
    use iroha_config::parameters::actual::Root as Config;
    use iroha_config_base::toml::TomlSource;
    use iroha_core::{
        beacon::{
            AdaptiveGlobalThresholdBeaconDkgCryptoV1, GlobalThresholdBeaconDkgStateV1,
            GlobalThresholdBeaconPulseAggregatorV1, validate_global_threshold_beacon_session_v1,
        },
        governance::timed_ovn::TimedOvnReleaseIdentityPublicV1,
        tle_release::{
            AuthorizedTleReleaseProjectionV1, TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1,
            ValidatedTleKeySessionV1,
        },
    };
    use iroha_crypto::{
        Hash, HashOf,
        threshold_bls::{
            AdaptiveThresholdBlsParameters, BeaconPurpose, DasRenDealerSecret,
            THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1, ThresholdBlsSession, TleReleasePurpose,
            ValidatedDealerCommitment,
        },
        tle::TleReleaseIdentityV1,
    };
    use iroha_data_model::{
        block::BlockHeader,
        consensus::{
            GLOBAL_THRESHOLD_BEACON_VERSION_V1, GlobalThresholdBeaconChainAnchorV1,
            GlobalThresholdBeaconDkgConstantProofV1, GlobalThresholdBeaconDkgDealerCommitmentV1,
            GlobalThresholdBeaconDkgSessionV1,
        },
        governance::types::BallotAttemptId,
    };
    use rand::{SeedableRng as _, rngs::StdRng};
    use std::{
        fs,
        io::{Cursor, Write as _},
        os::unix::fs::{PermissionsExt as _, symlink},
        path::{Path, PathBuf},
    };

    const HANDLE: &str = "software://iroha/consensus-threshold/primary";
    const REVISION: u64 = 7;
    const POLICY_DIGEST: [u8; 32] = [0xA7; 32];
    const BLS12_381_SCALAR_MODULUS_BE_V1: [u8; 32] = [
        0x73, 0xED, 0xA7, 0x53, 0x29, 0x9D, 0x7D, 0x48, 0x33, 0x39, 0xD8, 0x08, 0x09, 0xA1, 0xD8,
        0x05, 0x53, 0xBD, 0xA4, 0x02, 0xFF, 0xFE, 0x5B, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00,
        0x00, 0x01,
    ];

    struct BeaconFixtureV1 {
        record: GlobalThresholdBeaconKeySessionV1,
        validated: ValidatedGlobalThresholdBeaconSessionV1,
        components: Zeroizing<[[u8; 32]; 3]>,
    }

    struct TleFixtureV1 {
        threshold_session: ThresholdBlsSession<TleReleasePurpose>,
        validated: ValidatedTleKeySessionV1,
        components: Zeroizing<[[u8; 32]; 3]>,
    }

    /// Fully resolved beacon signer fixture used by authenticated broker tests.
    pub(crate) struct ConsensusThresholdBeaconBrokerTestFixtureV1 {
        /// Exact runtime-provider catalog bound into the credential.
        pub(crate) catalog: IrohaRuntimeProviderBindingsV1,
        /// Broker backends resolved from the canonical credential.
        pub(crate) backends: RuntimeProviderBrokerBackendsV1,
        /// Independently validated public session used to verify partials.
        pub(crate) session: ValidatedGlobalThresholdBeaconSessionV1,
    }

    /// Fully resolved TLE signer fixture used by authenticated broker tests.
    pub(crate) struct ConsensusThresholdTleBrokerTestFixtureV1 {
        /// Exact runtime-provider catalog bound into the credential.
        pub(crate) catalog: IrohaRuntimeProviderBindingsV1,
        /// Broker backends resolved from the canonical credential.
        pub(crate) backends: RuntimeProviderBrokerBackendsV1,
        /// Authorized public release projection used by the broker operation.
        pub(crate) projection: AuthorizedTleReleaseProjectionV1,
        /// Independently validated public session used to verify partials.
        pub(crate) session: ValidatedTleKeySessionV1,
        /// Public release identity used to verify partials.
        pub(crate) identity: TleReleaseIdentityV1,
    }

    fn accumulate_canonical_scalar_v1(total: &mut [u8; 32], term: &[u8; 32]) {
        assert!(
            *total < BLS12_381_SCALAR_MODULUS_BE_V1 && *term < BLS12_381_SCALAR_MODULUS_BE_V1,
            "DKG fixture components must be canonical BLS12-381 scalars"
        );
        let mut carry = 0_u16;
        for (total_byte, term_byte) in total.iter_mut().rev().zip(term.iter().rev()) {
            let sum = u16::from(*total_byte) + u16::from(*term_byte) + carry;
            *total_byte = u8::try_from(sum & 0xFF).expect("masked scalar byte fits u8");
            carry = sum >> 8;
        }
        assert_eq!(
            carry, 0,
            "sum of two reduced BLS12-381 scalars fits 256 bits"
        );
        if *total < BLS12_381_SCALAR_MODULUS_BE_V1 {
            return;
        }
        let mut borrow = 0_i16;
        for (total_byte, modulus_byte) in total
            .iter_mut()
            .rev()
            .zip(BLS12_381_SCALAR_MODULUS_BE_V1.iter().rev())
        {
            let difference = i16::from(*total_byte) - i16::from(*modulus_byte) - borrow;
            if difference < 0 {
                *total_byte =
                    u8::try_from(difference + 256).expect("borrow-reduced scalar byte fits u8");
                borrow = 1;
            } else {
                *total_byte = u8::try_from(difference).expect("reduced scalar byte fits u8");
                borrow = 0;
            }
        }
        assert_eq!(borrow, 0, "reduction subtracts a smaller modulus");
    }

    fn accumulate_component_triple_v1(
        total: &mut Zeroizing<[[u8; 32]; 3]>,
        contribution: &Zeroizing<[[u8; 32]; 3]>,
    ) {
        for (total_component, contribution_component) in total.iter_mut().zip(contribution.iter()) {
            accumulate_canonical_scalar_v1(total_component, contribution_component);
        }
    }

    fn beacon_fixture_with_committee_v1(
        network_id: NetworkId,
        session_byte: u8,
        committee_size: u16,
    ) -> BeaconFixtureV1 {
        let threshold = committee_size
            .checked_sub(1)
            .map(|fault_numerator| fault_numerator / 3 + 1)
            .expect("beacon fixture committee is nonzero");
        let dkg_session = GlobalThresholdBeaconDkgSessionV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id,
            session_id: [session_byte; 32],
            roster_hash: [0x31; 32],
            committee_size,
            threshold,
            start_height: 1,
            sharing_end_height: 2,
            complaints_end_height: 3,
            responses_end_height: 4,
        };
        let threshold_session = ThresholdBlsSession::<BeaconPurpose>::new(
            *network_id.as_bytes(),
            dkg_session.session_id,
            dkg_session.roster_hash,
            dkg_session.committee_size,
            dkg_session.threshold,
        )
        .expect("construct beacon threshold session");
        let parameters = AdaptiveThresholdBlsParameters::derive(&threshold_session)
            .expect("derive beacon fixture parameters");
        let mut rng = StdRng::from_seed([session_byte.wrapping_add(0x21); 32]);
        let crypto = AdaptiveGlobalThresholdBeaconDkgCryptoV1;
        let mut reducer = GlobalThresholdBeaconDkgStateV1::new(dkg_session, &crypto)
            .expect("start beacon fixture DKG");
        let mut components = Zeroizing::new([[0_u8; 32]; 3]);
        for dealer_index in 1_u16..=dkg_session.committee_size {
            let (dealer_secret, dealer) =
                DasRenDealerSecret::generate_with_rng(&parameters, dealer_index, &mut rng)
                    .expect("generate beacon fixture dealer");
            let contribution = dealer_secret
                .private_share(&parameters, &dealer, 1)
                .expect("derive authenticated beacon fixture contribution")
                .components_for_authenticated_encryption();
            accumulate_component_triple_v1(&mut components, &contribution);
            reducer
                .record_dealer_commitment(1, beacon_dealer_wire_v1(&dealer), &crypto)
                .expect("record proof-valid beacon dealer");
        }
        let record = reducer
            .finalize(dkg_session.responses_end_height, &crypto)
            .expect("finalize beacon fixture DKG")
            .clone();
        let binding = beacon_binding_v1(&record);
        let validated = validate_global_threshold_beacon_session_v1(record.clone(), &binding)
            .expect("revalidate beacon fixture transcript");
        BeaconFixtureV1 {
            record,
            validated,
            components,
        }
    }

    fn beacon_fixture_v1(network_id: NetworkId, session_byte: u8) -> BeaconFixtureV1 {
        beacon_fixture_with_committee_v1(network_id, session_byte, 4)
    }

    fn beacon_dealer_wire_v1(
        dealer: &ValidatedDealerCommitment<BeaconPurpose>,
    ) -> GlobalThresholdBeaconDkgDealerCommitmentV1 {
        GlobalThresholdBeaconDkgDealerCommitmentV1 {
            dealer_index: dealer.dealer_index(),
            coefficient_commitments: dealer
                .coefficients()
                .iter()
                .map(|coefficient| *coefficient.as_bytes())
                .collect(),
            constant_term_proof: GlobalThresholdBeaconDkgConstantProofV1 {
                commitment: *dealer.constant_proof().commitment_bytes(),
                response: *dealer.constant_proof().response_bytes(),
            },
        }
    }

    fn tle_fixture_with_committee_v1(
        network_id: NetworkId,
        session_byte: u8,
        committee_size: u16,
    ) -> TleFixtureV1 {
        let threshold = committee_size
            .checked_sub(1)
            .map(|fault_numerator| fault_numerator / 3 + 1)
            .expect("TLE fixture committee is nonzero");
        let threshold_session = ThresholdBlsSession::<TleReleasePurpose>::new(
            *network_id.as_bytes(),
            [session_byte; 32],
            [0x41; 32],
            committee_size,
            threshold,
        )
        .expect("construct TLE threshold session");
        let parameters = AdaptiveThresholdBlsParameters::derive(&threshold_session)
            .expect("derive TLE fixture parameters");
        let mut rng = StdRng::from_seed([session_byte.wrapping_add(0x31); 32]);
        let mut dealers = Vec::with_capacity(usize::from(committee_size));
        let mut components = Zeroizing::new([[0_u8; 32]; 3]);
        for dealer_index in 1_u16..=committee_size {
            let (dealer_secret, dealer) =
                DasRenDealerSecret::generate_with_rng(&parameters, dealer_index, &mut rng)
                    .expect("generate TLE fixture dealer");
            let contribution = dealer_secret
                .private_share(&parameters, &dealer, 1)
                .expect("derive authenticated TLE fixture contribution")
                .components_for_authenticated_encryption();
            accumulate_component_triple_v1(&mut components, &contribution);
            dealers.push(dealer);
        }
        let qualified_dealers = (1_u16..=committee_size).collect::<Vec<_>>();
        let validated = ValidatedTleKeySessionV1::from_qualified_dealers(
            threshold_session,
            &dealers,
            &qualified_dealers,
            [0x51; 32],
        )
        .expect("finalize proof-valid TLE fixture");
        TleFixtureV1 {
            threshold_session,
            validated,
            components,
        }
    }

    fn tle_fixture_v1(network_id: NetworkId, session_byte: u8) -> TleFixtureV1 {
        tle_fixture_with_committee_v1(network_id, session_byte, 4)
    }

    fn configured_consensus_catalog_v1(
        slot: IrohaRuntimeProviderSlotV1,
        revision: u64,
        policy_digest: [u8; 32],
    ) -> IrohaRuntimeProviderBindingsV1 {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../defaults/kagami/iroha3-dev/config.toml");
        let source = fs::read_to_string(path).expect("read checked-in default daemon config");
        let mut table: toml::Table = toml::from_str(&source).expect("parse default daemon config");
        let genesis = table
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .expect("default daemon genesis table");
        assert_eq!(
            genesis
                .remove("expected_hash_file")
                .and_then(|value| value.as_str().map(str::to_owned))
                .as_deref(),
            Some("genesis.expected_hash")
        );
        assert!(
            genesis
                .insert(
                    "expected_hash".to_owned(),
                    toml::Value::String(Hash::prehashed([0xC1; 32]).to_string()),
                )
                .is_none()
        );
        let mut config = Config::from_toml_source(TomlSource::inline(table))
            .expect("resolve checked-in default daemon config for threshold catalog test");
        match slot {
            IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner => {
                config.sumeragi.global_beacon_partial_signer_provider_handle =
                    Some(HANDLE.to_owned());
                config
                    .sumeragi
                    .global_beacon_partial_signer_provider_revision = Some(revision);
                config
                    .sumeragi
                    .global_beacon_partial_signer_provider_policy_digest = Some(policy_digest);
            }
            IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner => {
                config
                    .gov
                    .parliament_tle_partial_release_signer_provider_handle =
                    Some(HANDLE.to_owned());
                config
                    .gov
                    .parliament_tle_partial_release_signer_provider_revision = Some(revision);
                config
                    .gov
                    .parliament_tle_partial_release_signer_provider_policy_digest =
                    Some(policy_digest);
            }
            _ => panic!("unsupported threshold test slot {slot:?}"),
        }
        let catalog = IrohaRuntimeProviderBindingsV1::try_from_config(&config)
            .expect("project threshold signer through production config collector");
        let binding = catalog
            .iter()
            .find(|binding| binding.slot() == slot)
            .expect("configured threshold slot must be projected");
        assert_eq!(binding.handle(), HANDLE);
        assert_eq!(binding.revision(), Some(revision));
        assert_eq!(binding.policy_digest(), Some(policy_digest));
        assert_eq!(catalog.network_id(), &network_id_v1(0xC1));
        catalog
    }

    fn beacon_catalog_with_revision_v1(
        revision: u64,
        policy_digest: [u8; 32],
    ) -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1::qualified_for_test(
            "consensus-threshold-credential-test",
            IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
            HANDLE,
            revision,
            policy_digest,
        )
        .with_network_id_for_test(network_id_v1(0xC1))
    }

    fn beacon_catalog_v1(policy_digest: [u8; 32]) -> IrohaRuntimeProviderBindingsV1 {
        beacon_catalog_with_revision_v1(REVISION, policy_digest)
    }

    fn tle_catalog_with_revision_v1(
        revision: u64,
        policy_digest: [u8; 32],
    ) -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1::qualified_for_test(
            "consensus-threshold-credential-test",
            IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner,
            HANDLE,
            revision,
            policy_digest,
        )
        .with_network_id_for_test(network_id_v1(0xC1))
    }

    fn tle_catalog_v1(policy_digest: [u8; 32]) -> IrohaRuntimeProviderBindingsV1 {
        tle_catalog_with_revision_v1(REVISION, policy_digest)
    }

    fn beacon_pulse_aggregator_v1(
        session: &ValidatedGlobalThresholdBeaconSessionV1,
        block_hash_byte: u8,
    ) -> GlobalThresholdBeaconPulseAggregatorV1 {
        let anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: 40,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [block_hash_byte; 32],
            )),
        };
        GlobalThresholdBeaconPulseAggregatorV1::new(session.clone(), 41, anchor)
            .expect("construct canonical beacon pulse")
    }

    fn consensus_threshold_beacon_broker_test_fixture_for_committee_v1(
        committee_size: u16,
        session_byte: u8,
    ) -> ConsensusThresholdBeaconBrokerTestFixtureV1 {
        let network_id = network_id_v1(0xC1);
        let fixture = beacon_fixture_with_committee_v1(network_id, session_byte, committee_size);
        let session = fixture.validated;
        let provisioning = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            fixture.record,
            1,
            fixture.components,
        )];
        let policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &provisioning)
                .expect("derive broker-roundtrip beacon inventory digest");
        let catalog = beacon_catalog_v1(policy_digest);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            policy_digest,
            provisioning,
        )
        .expect("encode broker-roundtrip beacon credential");
        let backend = decode_global_beacon_credential_v1(
            &credential,
            catalog.network_id(),
            catalog.iter().next().expect("one beacon binding"),
        )
        .expect("decode broker-roundtrip beacon credential");
        let registry = RuntimeConsensusThresholdSignerBackendsV1 {
            global_beacon: Some(backend),
            parliament_tle: None,
        };
        let backends = registry
            .resolve(&catalog)
            .expect("resolve broker-roundtrip beacon backend");
        ConsensusThresholdBeaconBrokerTestFixtureV1 {
            catalog,
            backends,
            session,
        }
    }

    /// Builds a beacon fixture through canonical provisioning, decode, and resolution.
    pub(crate) fn consensus_threshold_beacon_broker_test_fixture_v1()
    -> ConsensusThresholdBeaconBrokerTestFixtureV1 {
        consensus_threshold_beacon_broker_test_fixture_for_committee_v1(4, 0x79)
    }

    /// Builds a maximum-committee beacon fixture for ordinary-stack broker tests.
    pub(crate) fn consensus_threshold_beacon_broker_max_committee_test_fixture_v1()
    -> ConsensusThresholdBeaconBrokerTestFixtureV1 {
        consensus_threshold_beacon_broker_test_fixture_for_committee_v1(
            THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1,
            0x7F,
        )
    }

    fn consensus_threshold_tle_broker_test_fixture_for_committee_v1(
        committee_size: u16,
        session_byte: u8,
    ) -> ConsensusThresholdTleBrokerTestFixtureV1 {
        let network_id = network_id_v1(0xC1);
        let fixture = tle_fixture_with_committee_v1(network_id, session_byte, committee_size);
        let (projection, identity) = tle_projection_v1(&fixture);
        let projection = projection.projection().clone();
        let session = fixture.validated.clone();
        let provisioning = vec![RuntimeParliamentTleShareProvisioningV1::new(
            fixture.validated.public_state().clone(),
            1,
            fixture.components,
        )];
        let policy_digest =
            parliament_tle_partial_release_signer_inventory_digest_v1(network_id, &provisioning)
                .expect("derive broker-roundtrip TLE inventory digest");
        let catalog = tle_catalog_v1(policy_digest);
        let credential = encode_parliament_tle_partial_release_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            policy_digest,
            provisioning,
        )
        .expect("encode broker-roundtrip TLE credential");
        let backend = decode_parliament_tle_credential_v1(
            &credential,
            catalog.network_id(),
            catalog.iter().next().expect("one TLE binding"),
        )
        .expect("decode broker-roundtrip TLE credential");
        let registry = RuntimeConsensusThresholdSignerBackendsV1 {
            global_beacon: None,
            parliament_tle: Some(backend),
        };
        let backends = registry
            .resolve(&catalog)
            .expect("resolve broker-roundtrip TLE backend");
        ConsensusThresholdTleBrokerTestFixtureV1 {
            catalog,
            backends,
            projection,
            session,
            identity,
        }
    }

    /// Builds a TLE fixture through canonical provisioning, decode, and resolution.
    pub(crate) fn consensus_threshold_tle_broker_test_fixture_v1()
    -> ConsensusThresholdTleBrokerTestFixtureV1 {
        consensus_threshold_tle_broker_test_fixture_for_committee_v1(4, 0x7A)
    }

    /// Builds a maximum-committee TLE fixture for ordinary-stack broker tests.
    pub(crate) fn consensus_threshold_tle_broker_max_committee_test_fixture_v1()
    -> ConsensusThresholdTleBrokerTestFixtureV1 {
        consensus_threshold_tle_broker_test_fixture_for_committee_v1(
            THRESHOLD_BLS_MAX_COMMITTEE_SIZE_V1,
            0x80,
        )
    }

    fn secure_credential_directory_v1() -> (tempfile::TempDir, PathBuf) {
        let directory = tempfile::tempdir().expect("create credential directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .expect("harden credential directory");
        let canonical = fs::canonicalize(directory.path()).expect("canonical credential path");
        (directory, canonical)
    }

    fn write_credential_v1(directory: &Path, name: &str, bytes: &[u8], mode: u32) -> PathBuf {
        let path = directory.join(name);
        let mut file = fs::File::create(&path).expect("create runtime credential");
        file.write_all(bytes).expect("write runtime credential");
        file.sync_all().expect("sync runtime credential");
        fs::set_permissions(&path, fs::Permissions::from_mode(mode))
            .expect("set runtime credential mode");
        path
    }

    fn tle_projection_v1(
        fixture: &TleFixtureV1,
    ) -> (ValidatedTleReleaseProjectionV1, TleReleaseIdentityV1) {
        let identity = TleReleaseIdentityV1::new(
            fixture.threshold_session,
            [0x61; 32],
            [0x62; 32],
            [0x63; 32],
            [0x64; 32],
            [0x65; 32],
            100,
            [0x66; 32],
        )
        .expect("construct exact TLE release identity");
        let identity_payload = identity
            .payload_bytes()
            .try_into()
            .expect("fixed-size TLE identity payload");
        let identity_digest = Sha256::digest(
            identity
                .release_message()
                .expect("frame TLE release identity"),
        )
        .into();
        let projection = AuthorizedTleReleaseProjectionV1 {
            version: TLE_AUTHORIZED_RELEASE_PROJECTION_VERSION_V1,
            ballot_attempt_id: BallotAttemptId::new([0x63; 32]),
            opening_deadline_height: 110,
            finalized_height: 100,
            key_session: fixture.validated.public_state().clone(),
            public_release_identity: TimedOvnReleaseIdentityPublicV1 {
                tle_key_session_id: fixture.validated.public_state().key_session_id,
                governance_attempt_id: [0x61; 32],
                body_instance_id: [0x62; 32],
                ballot_attempt_id: [0x63; 32],
                survivor_corpus_root: [0x64; 32],
                no_recovery_root: [0x65; 32],
                target_finalized_height: 100,
                parameter_hash: [0x66; 32],
            },
            identity_payload,
            identity_digest,
        }
        .validate()
        .expect("validate TLE broker projection");
        (projection, identity)
    }

    #[test]
    fn global_beacon_credential_loads_resolves_and_signs_verified_partial() {
        let network_id = network_id_v1(0xC1);
        let fixture = beacon_fixture_v1(network_id, 0x71);
        let validated = fixture.validated.clone();
        let provisioning = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            fixture.record,
            1,
            fixture.components,
        )];
        let policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &provisioning)
                .expect("derive beacon inventory digest");
        let catalog = configured_consensus_catalog_v1(
            IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
            REVISION,
            policy_digest,
        );
        let credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            policy_digest,
            provisioning,
        )
        .expect("encode beacon runtime credential");
        let bundle = encode_consensus_threshold_credential_bundle_v1(Some(&credential), None)
            .expect("frame beacon credential for launchd stdin");
        let bundled =
            RuntimeConsensusThresholdSignerBackendsV1::load_from_launchd_credential_bundle_v1(
                &catalog,
                &mut Cursor::new(bundle.as_slice()),
            )
            .expect("load beacon credential from launchd bundle");
        bundled
            .resolve(&catalog)
            .expect("resolve launchd-bundled beacon backend");
        let mut wrong_presence = Zeroizing::new(bundle.as_slice().to_vec());
        wrong_presence[10..12].copy_from_slice(&0_u16.to_be_bytes());
        assert!(matches!(
            RuntimeConsensusThresholdSignerBackendsV1::load_from_launchd_credential_bundle_v1(
                &catalog,
                &mut Cursor::new(wrong_presence.as_slice()),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));
        assert!(matches!(
            RuntimeConsensusThresholdSignerBackendsV1::load_from_launchd_credential_bundle_v1(
                &catalog,
                &mut Cursor::new(&bundle[..bundle.len() - 1]),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)
        ));
        let (_guard, directory) = secure_credential_directory_v1();
        write_credential_v1(
            &directory,
            GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1,
            &credential,
            0o600,
        );
        let loaded = RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
            &catalog,
            Some(&directory),
        )
        .expect("load beacon runtime credential");
        loaded
            .resolve(&catalog)
            .expect("resolve exact beacon backend");
        ExternalSoftwareSignerBackendsV1::new()
            .with_base_registry(Arc::new(loaded.clone()))
            .resolve(&catalog)
            .expect("compose threshold backend beneath external signer registry");

        let anchor = GlobalThresholdBeaconChainAnchorV1 {
            height: 40,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x81; 32])),
        };
        let mut verifier =
            GlobalThresholdBeaconPulseAggregatorV1::new(validated.clone(), 41, anchor)
                .expect("construct canonical beacon pulse");
        let partial = loaded
            .global_beacon
            .as_deref()
            .expect("loaded beacon backend")
            .sign_partial(&validated, verifier.payload())
            .expect("sign exact beacon pulse");
        assert!(
            verifier
                .accept_partial(partial)
                .expect("independently verify beacon partial")
        );
    }

    #[test]
    fn parliament_tle_credential_loads_resolves_and_signs_verified_partial() {
        let network_id = network_id_v1(0xC1);
        let fixture = tle_fixture_v1(network_id, 0x72);
        let (projection, identity) = tle_projection_v1(&fixture);
        let validated = fixture.validated.clone();
        let provisioning = vec![RuntimeParliamentTleShareProvisioningV1::new(
            fixture.validated.public_state().clone(),
            1,
            fixture.components,
        )];
        let policy_digest =
            parliament_tle_partial_release_signer_inventory_digest_v1(network_id, &provisioning)
                .expect("derive Parliament TLE inventory digest");
        let catalog = configured_consensus_catalog_v1(
            IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner,
            REVISION,
            policy_digest,
        );
        let credential = encode_parliament_tle_partial_release_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            policy_digest,
            provisioning,
        )
        .expect("encode Parliament TLE runtime credential");
        let bundle = encode_consensus_threshold_credential_bundle_v1(None, Some(&credential))
            .expect("frame Parliament TLE credential for launchd stdin");
        let bundled =
            RuntimeConsensusThresholdSignerBackendsV1::load_from_launchd_credential_bundle_v1(
                &catalog,
                &mut Cursor::new(bundle.as_slice()),
            )
            .expect("load Parliament TLE credential from launchd bundle");
        bundled
            .resolve(&catalog)
            .expect("resolve launchd-bundled Parliament TLE backend");
        let (_guard, directory) = secure_credential_directory_v1();
        write_credential_v1(
            &directory,
            PARLIAMENT_TLE_PARTIAL_RELEASE_SIGNER_CREDENTIAL_NAME_V1,
            &credential,
            0o600,
        );
        let loaded = RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
            &catalog,
            Some(&directory),
        )
        .expect("load Parliament TLE runtime credential");
        loaded.resolve(&catalog).expect("resolve exact TLE backend");
        ExternalSoftwareSignerBackendsV1::new()
            .with_base_registry(Arc::new(loaded.clone()))
            .resolve(&catalog)
            .expect("compose TLE backend beneath external signer registry");

        let partial = loaded
            .parliament_tle
            .as_deref()
            .expect("loaded TLE backend")
            .sign_projected_partial_release(&projection)
            .expect("sign exact Parliament TLE projection");
        validated
            .verify_partial_release(&identity, 100, &partial)
            .expect("independently verify Parliament TLE partial");
    }

    #[test]
    fn beacon_supervisor_restart_rotation_requires_revision_bump_and_removes_predecessor() {
        let network_id = network_id_v1(0xC1);
        let predecessor = beacon_fixture_v1(network_id, 0x7B);
        let predecessor_session = predecessor.validated.clone();
        let successor = beacon_fixture_v1(network_id, 0x7C);
        let successor_session = successor.validated.clone();
        let old_provisioning = vec![
            RuntimeGlobalBeaconShareProvisioningV1::new(
                predecessor.record,
                1,
                predecessor.components,
            ),
            RuntimeGlobalBeaconShareProvisioningV1::new(successor.record, 1, successor.components),
        ];
        let old_policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &old_provisioning)
                .expect("derive predecessor-plus-successor beacon inventory digest");
        let old_catalog = beacon_catalog_v1(old_policy_digest);
        let old_credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            old_policy_digest,
            old_provisioning,
        )
        .expect("encode predecessor-plus-successor beacon credential");
        let (old_directory_guard, old_directory) = secure_credential_directory_v1();
        write_credential_v1(
            &old_directory,
            GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1,
            &old_credential,
            0o600,
        );
        let old_backend =
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &old_catalog,
                Some(&old_directory),
            )
            .expect("start revision-N beacon backend");
        old_backend
            .resolve(&old_catalog)
            .expect("resolve revision-N beacon backend");
        for (session, marker) in [(&predecessor_session, 0x91), (&successor_session, 0x92)] {
            let mut verifier = beacon_pulse_aggregator_v1(session, marker);
            let partial = old_backend
                .global_beacon
                .as_deref()
                .expect("revision-N beacon backend")
                .sign_partial(session, verifier.payload())
                .expect("revision-N inventory signs both sessions");
            assert!(
                verifier
                    .accept_partial(partial)
                    .expect("verify revision-N beacon partial")
            );
        }

        let replacement_successor = beacon_fixture_v1(network_id, 0x7C);
        let new_provisioning = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            replacement_successor.record,
            1,
            replacement_successor.components,
        )];
        let new_policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &new_provisioning)
                .expect("derive successor-only beacon inventory digest");
        let new_catalog = beacon_catalog_with_revision_v1(REVISION + 1, new_policy_digest);
        assert_eq!(old_catalog.network_id(), new_catalog.network_id());
        assert!(matches!(
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &new_catalog,
                Some(&old_directory),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));
        drop(old_backend);
        drop(old_credential);
        drop(old_directory_guard);

        let new_credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION + 1,
            new_policy_digest,
            new_provisioning,
        )
        .expect("encode successor-only beacon credential");
        let (_new_directory_guard, new_directory) = secure_credential_directory_v1();
        write_credential_v1(
            &new_directory,
            GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1,
            &new_credential,
            0o600,
        );
        let restarted_backend =
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &new_catalog,
                Some(&new_directory),
            )
            .expect("restart with revision-N-plus-one beacon backend");
        restarted_backend
            .resolve(&new_catalog)
            .expect("resolve revision-N-plus-one beacon backend");

        let predecessor_pulse = beacon_pulse_aggregator_v1(&predecessor_session, 0x93);
        assert!(
            restarted_backend
                .global_beacon
                .as_deref()
                .expect("restarted beacon backend")
                .sign_partial(&predecessor_session, predecessor_pulse.payload())
                .is_err(),
            "the restarted successor-only inventory must not retain the predecessor"
        );
        let mut successor_pulse = beacon_pulse_aggregator_v1(&successor_session, 0x94);
        let successor_partial = restarted_backend
            .global_beacon
            .as_deref()
            .expect("restarted beacon backend")
            .sign_partial(&successor_session, successor_pulse.payload())
            .expect("restarted backend signs the successor session");
        assert!(
            successor_pulse
                .accept_partial(successor_partial)
                .expect("verify restarted successor beacon partial")
        );
    }

    #[test]
    fn tle_supervisor_restart_rotation_requires_revision_bump_and_removes_predecessor() {
        let network_id = network_id_v1(0xC1);
        let predecessor = tle_fixture_v1(network_id, 0x7D);
        let predecessor_session = predecessor.validated.clone();
        let (predecessor_projection, predecessor_identity) = tle_projection_v1(&predecessor);
        let successor = tle_fixture_v1(network_id, 0x7E);
        let successor_session = successor.validated.clone();
        let (successor_projection, successor_identity) = tle_projection_v1(&successor);
        let old_provisioning = vec![
            RuntimeParliamentTleShareProvisioningV1::new(
                predecessor.validated.public_state().clone(),
                1,
                predecessor.components,
            ),
            RuntimeParliamentTleShareProvisioningV1::new(
                successor.validated.public_state().clone(),
                1,
                successor.components,
            ),
        ];
        let old_policy_digest = parliament_tle_partial_release_signer_inventory_digest_v1(
            network_id,
            &old_provisioning,
        )
        .expect("derive predecessor-plus-successor TLE inventory digest");
        let old_catalog = tle_catalog_v1(old_policy_digest);
        let old_credential = encode_parliament_tle_partial_release_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            old_policy_digest,
            old_provisioning,
        )
        .expect("encode predecessor-plus-successor TLE credential");
        let (old_directory_guard, old_directory) = secure_credential_directory_v1();
        write_credential_v1(
            &old_directory,
            PARLIAMENT_TLE_PARTIAL_RELEASE_SIGNER_CREDENTIAL_NAME_V1,
            &old_credential,
            0o600,
        );
        let old_backend =
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &old_catalog,
                Some(&old_directory),
            )
            .expect("start revision-N TLE backend");
        old_backend
            .resolve(&old_catalog)
            .expect("resolve revision-N TLE backend");
        for (session, projection, identity) in [
            (
                &predecessor_session,
                &predecessor_projection,
                &predecessor_identity,
            ),
            (
                &successor_session,
                &successor_projection,
                &successor_identity,
            ),
        ] {
            let partial = old_backend
                .parliament_tle
                .as_deref()
                .expect("revision-N TLE backend")
                .sign_projected_partial_release(projection)
                .expect("revision-N inventory signs both sessions");
            session
                .verify_partial_release(identity, 100, &partial)
                .expect("verify revision-N TLE partial");
        }

        let replacement_successor = tle_fixture_v1(network_id, 0x7E);
        let new_provisioning = vec![RuntimeParliamentTleShareProvisioningV1::new(
            replacement_successor.validated.public_state().clone(),
            1,
            replacement_successor.components,
        )];
        let new_policy_digest = parliament_tle_partial_release_signer_inventory_digest_v1(
            network_id,
            &new_provisioning,
        )
        .expect("derive successor-only TLE inventory digest");
        let new_catalog = tle_catalog_with_revision_v1(REVISION + 1, new_policy_digest);
        assert_eq!(old_catalog.network_id(), new_catalog.network_id());
        assert!(matches!(
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &new_catalog,
                Some(&old_directory),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));
        drop(old_backend);
        drop(old_credential);
        drop(old_directory_guard);

        let new_credential = encode_parliament_tle_partial_release_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION + 1,
            new_policy_digest,
            new_provisioning,
        )
        .expect("encode successor-only TLE credential");
        let (_new_directory_guard, new_directory) = secure_credential_directory_v1();
        write_credential_v1(
            &new_directory,
            PARLIAMENT_TLE_PARTIAL_RELEASE_SIGNER_CREDENTIAL_NAME_V1,
            &new_credential,
            0o600,
        );
        let restarted_backend =
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &new_catalog,
                Some(&new_directory),
            )
            .expect("restart with revision-N-plus-one TLE backend");
        restarted_backend
            .resolve(&new_catalog)
            .expect("resolve revision-N-plus-one TLE backend");

        assert!(
            restarted_backend
                .parliament_tle
                .as_deref()
                .expect("restarted TLE backend")
                .sign_projected_partial_release(&predecessor_projection)
                .is_err(),
            "the restarted successor-only inventory must not retain the predecessor"
        );
        let successor_partial = restarted_backend
            .parliament_tle
            .as_deref()
            .expect("restarted TLE backend")
            .sign_projected_partial_release(&successor_projection)
            .expect("restarted backend signs the successor session");
        successor_session
            .verify_partial_release(&successor_identity, 100, &successor_partial)
            .expect("verify restarted successor TLE partial");
    }

    #[test]
    fn consensus_threshold_top_level_schema_hashes_are_golden() {
        for (name, expected_hash_hex, actual_hash) in [
            (
                GLOBAL_BEACON_SIGNER_CREDENTIAL_SCHEMA_NAME_V1,
                "0b311f1a10d971b693860f8fb160ed1c",
                <RuntimeGlobalBeaconSignerCredentialWireV1 as NoritoSerialize>::schema_hash(),
            ),
            (
                PARLIAMENT_TLE_SIGNER_CREDENTIAL_SCHEMA_NAME_V1,
                "4071e4e5876f8a71466b3e94581b710b",
                <RuntimeParliamentTleSignerCredentialWireV1 as NoritoSerialize>::schema_hash(),
            ),
            (
                GLOBAL_BEACON_PUBLIC_INVENTORY_SCHEMA_NAME_V1,
                "ea71fde9b50685c39f6977c4f472ac39",
                <RuntimeGlobalBeaconPublicInventoryWireV1 as NoritoSerialize>::schema_hash(),
            ),
            (
                PARLIAMENT_TLE_PUBLIC_INVENTORY_SCHEMA_NAME_V1,
                "3087d1f9251cf172e937752119357b34",
                <RuntimeParliamentTlePublicInventoryWireV1 as NoritoSerialize>::schema_hash(),
            ),
        ] {
            assert_eq!(hex::encode(actual_hash), expected_hash_hex);
            assert_eq!(
                actual_hash,
                norito::core::schema_hash_for_name(name),
                "derived schema hash drifted for {name}"
            );
        }
        assert_eq!(
            <RuntimeGlobalBeaconSignerCredentialWireV1 as NoritoDeserialize<'static>>::schema_hash(
            ),
            <RuntimeGlobalBeaconSignerCredentialWireV1 as NoritoSerialize>::schema_hash(),
        );
        assert_eq!(
            <RuntimeParliamentTleSignerCredentialWireV1 as NoritoDeserialize<'static>>::schema_hash(
            ),
            <RuntimeParliamentTleSignerCredentialWireV1 as NoritoSerialize>::schema_hash(),
        );
    }

    #[test]
    fn public_inventory_digests_are_order_stable_and_seat_bound() {
        let network_id = network_id_v1(0xC1);
        let first = beacon_fixture_v1(network_id, 0x81);
        let second = beacon_fixture_v1(network_id, 0x82);
        let mut beacon_inventory = vec![
            RuntimeGlobalBeaconShareProvisioningV1::new(first.record, 1, first.components),
            RuntimeGlobalBeaconShareProvisioningV1::new(second.record, 1, second.components),
        ];
        let beacon_forward =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &beacon_inventory)
                .expect("derive forward beacon inventory digest");
        beacon_inventory.reverse();
        let beacon_reverse =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &beacon_inventory)
                .expect("derive reverse beacon inventory digest");
        assert_eq!(beacon_forward, beacon_reverse);
        let reverse_credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            beacon_reverse,
            beacon_inventory,
        )
        .expect("encode reverse-ordered beacon inventory");
        let first = beacon_fixture_v1(network_id, 0x81);
        let second = beacon_fixture_v1(network_id, 0x82);
        let forward_credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            beacon_forward,
            vec![
                RuntimeGlobalBeaconShareProvisioningV1::new(first.record, 1, first.components),
                RuntimeGlobalBeaconShareProvisioningV1::new(second.record, 1, second.components),
            ],
        )
        .expect("encode forward-ordered beacon inventory");
        assert_eq!(&*forward_credential, &*reverse_credential);
        let beacon_catalog = beacon_catalog_v1(beacon_forward);
        let mut reordered_wire: RuntimeGlobalBeaconSignerCredentialWireV1 =
            norito::decode_canonical_with_limits(
                &forward_credential,
                CONSENSUS_THRESHOLD_CREDENTIAL_DECODE_LIMITS_V1,
            )
            .expect("decode canonical beacon credential for order substitution");
        reordered_wire.sessions.reverse();
        let reordered_credential = encode_secret_credential_v1(&reordered_wire)
            .expect("encode noncanonical beacon session order");
        assert!(matches!(
            decode_global_beacon_credential_v1(
                &reordered_credential,
                beacon_catalog.network_id(),
                beacon_catalog.iter().next().expect("one beacon binding"),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));

        let seat_fixture = beacon_fixture_v1(network_id, 0x83);
        let seat_one = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            seat_fixture.record.clone(),
            1,
            Zeroizing::new(*seat_fixture.components),
        )];
        let seat_two = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            seat_fixture.record,
            2,
            seat_fixture.components,
        )];
        assert_ne!(
            global_beacon_partial_signer_inventory_digest_v1(network_id, &seat_one)
                .expect("derive beacon seat-one inventory digest"),
            global_beacon_partial_signer_inventory_digest_v1(network_id, &seat_two)
                .expect("derive beacon seat-two inventory digest")
        );

        let first = tle_fixture_v1(network_id, 0x84);
        let second = tle_fixture_v1(network_id, 0x85);
        let mut tle_inventory = vec![
            RuntimeParliamentTleShareProvisioningV1::new(
                first.validated.public_state().clone(),
                1,
                first.components,
            ),
            RuntimeParliamentTleShareProvisioningV1::new(
                second.validated.public_state().clone(),
                1,
                second.components,
            ),
        ];
        let tle_forward =
            parliament_tle_partial_release_signer_inventory_digest_v1(network_id, &tle_inventory)
                .expect("derive forward TLE inventory digest");
        tle_inventory.reverse();
        let tle_reverse =
            parliament_tle_partial_release_signer_inventory_digest_v1(network_id, &tle_inventory)
                .expect("derive reverse TLE inventory digest");
        assert_eq!(tle_forward, tle_reverse);
        let reverse_credential = encode_parliament_tle_partial_release_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            tle_reverse,
            tle_inventory,
        )
        .expect("encode reverse-ordered TLE inventory");
        let first = tle_fixture_v1(network_id, 0x84);
        let second = tle_fixture_v1(network_id, 0x85);
        let forward_credential = encode_parliament_tle_partial_release_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            tle_forward,
            vec![
                RuntimeParliamentTleShareProvisioningV1::new(
                    first.validated.public_state().clone(),
                    1,
                    first.components,
                ),
                RuntimeParliamentTleShareProvisioningV1::new(
                    second.validated.public_state().clone(),
                    1,
                    second.components,
                ),
            ],
        )
        .expect("encode forward-ordered TLE inventory");
        assert_eq!(&*forward_credential, &*reverse_credential);
        let tle_catalog = tle_catalog_v1(tle_forward);
        let mut reordered_wire: RuntimeParliamentTleSignerCredentialWireV1 =
            norito::decode_canonical_with_limits(
                &forward_credential,
                CONSENSUS_THRESHOLD_CREDENTIAL_DECODE_LIMITS_V1,
            )
            .expect("decode canonical TLE credential for order substitution");
        reordered_wire.sessions.reverse();
        let reordered_credential = encode_secret_credential_v1(&reordered_wire)
            .expect("encode noncanonical TLE session order");
        assert!(matches!(
            decode_parliament_tle_credential_v1(
                &reordered_credential,
                tle_catalog.network_id(),
                tle_catalog.iter().next().expect("one TLE binding"),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));

        let seat_fixture = tle_fixture_v1(network_id, 0x86);
        let seat_one = vec![RuntimeParliamentTleShareProvisioningV1::new(
            seat_fixture.validated.public_state().clone(),
            1,
            Zeroizing::new(*seat_fixture.components),
        )];
        let seat_two = vec![RuntimeParliamentTleShareProvisioningV1::new(
            seat_fixture.validated.public_state().clone(),
            2,
            seat_fixture.components,
        )];
        assert_ne!(
            parliament_tle_partial_release_signer_inventory_digest_v1(network_id, &seat_one)
                .expect("derive TLE seat-one inventory digest"),
            parliament_tle_partial_release_signer_inventory_digest_v1(network_id, &seat_two)
                .expect("derive TLE seat-two inventory digest")
        );
    }

    #[test]
    fn same_revision_public_inventory_substitution_fails_closed() {
        let network_id = network_id_v1(0xC1);
        let expected = beacon_fixture_v1(network_id, 0x87);
        let expected_inventory = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            expected.record,
            1,
            expected.components,
        )];
        let expected_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &expected_inventory)
                .expect("derive expected beacon inventory digest");
        let catalog = beacon_catalog_v1(expected_digest);
        drop(expected_inventory);

        let substituted = beacon_fixture_v1(network_id, 0x88);
        let substituted_inventory = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            substituted.record,
            1,
            substituted.components,
        )];
        let substituted_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &substituted_inventory)
                .expect("derive substituted beacon inventory digest");
        assert_ne!(expected_digest, substituted_digest);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            substituted_digest,
            substituted_inventory,
        )
        .expect("encode substituted beacon inventory");
        let mut wire: RuntimeGlobalBeaconSignerCredentialWireV1 =
            norito::decode_canonical_with_limits(
                &credential,
                CONSENSUS_THRESHOLD_CREDENTIAL_DECODE_LIMITS_V1,
            )
            .expect("decode substituted beacon inventory");
        wire.header.policy_digest = expected_digest;
        let rebound = encode_secret_credential_v1(&wire)
            .expect("rebind substituted beacon header to expected policy digest");
        assert!(matches!(
            decode_global_beacon_credential_v1(
                &rebound,
                catalog.network_id(),
                catalog.iter().next().expect("one beacon binding"),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));

        let expected = tle_fixture_v1(network_id, 0x89);
        let expected_inventory = vec![RuntimeParliamentTleShareProvisioningV1::new(
            expected.validated.public_state().clone(),
            1,
            expected.components,
        )];
        let expected_digest = parliament_tle_partial_release_signer_inventory_digest_v1(
            network_id,
            &expected_inventory,
        )
        .expect("derive expected TLE inventory digest");
        let catalog = tle_catalog_v1(expected_digest);
        drop(expected_inventory);

        let substituted = tle_fixture_v1(network_id, 0x8A);
        let substituted_inventory = vec![RuntimeParliamentTleShareProvisioningV1::new(
            substituted.validated.public_state().clone(),
            1,
            substituted.components,
        )];
        let substituted_digest = parliament_tle_partial_release_signer_inventory_digest_v1(
            network_id,
            &substituted_inventory,
        )
        .expect("derive substituted TLE inventory digest");
        assert_ne!(expected_digest, substituted_digest);
        let credential = encode_parliament_tle_partial_release_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            substituted_digest,
            substituted_inventory,
        )
        .expect("encode substituted TLE inventory");
        let mut wire: RuntimeParliamentTleSignerCredentialWireV1 =
            norito::decode_canonical_with_limits(
                &credential,
                CONSENSUS_THRESHOLD_CREDENTIAL_DECODE_LIMITS_V1,
            )
            .expect("decode substituted TLE inventory");
        wire.header.policy_digest = expected_digest;
        let rebound = encode_secret_credential_v1(&wire)
            .expect("rebind substituted TLE header to expected policy digest");
        assert!(matches!(
            decode_parliament_tle_credential_v1(
                &rebound,
                catalog.network_id(),
                catalog.iter().next().expect("one TLE binding"),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));
    }

    #[test]
    fn credential_header_substitution_and_noncanonical_bytes_fail_closed() {
        let network_id = network_id_v1(0xC1);
        let fixture = beacon_fixture_v1(network_id, 0x73);
        let provisioning = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            fixture.record,
            1,
            fixture.components,
        )];
        let policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &provisioning)
                .expect("derive substituted-header inventory digest");
        let catalog = beacon_catalog_v1(policy_digest);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            policy_digest,
            provisioning,
        )
        .expect("encode valid beacon credential");
        for substitution in 0..5 {
            let mut wire: RuntimeGlobalBeaconSignerCredentialWireV1 =
                norito::decode_canonical_with_limits(
                    &credential,
                    CONSENSUS_THRESHOLD_CREDENTIAL_DECODE_LIMITS_V1,
                )
                .expect("decode valid credential for adversarial mutation");
            match substitution {
                0 => wire.header.network_id = network_id_v1(0x91),
                1 => wire.header.handle = "software://iroha/consensus-threshold/other".to_owned(),
                2 => wire.header.revision += 1,
                3 => wire.header.policy_digest = [0xA8; 32],
                4 => {
                    wire.header.slot =
                        IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner.wire_id();
                }
                _ => unreachable!(),
            }
            let substituted = encode_secret_credential_v1(&wire)
                .expect("encode structurally canonical substituted credential");
            assert!(matches!(
                decode_global_beacon_credential_v1(
                    &substituted,
                    catalog.network_id(),
                    catalog.iter().next().expect("one binding"),
                ),
                Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
            ));
        }

        let mut noncanonical = Zeroizing::new(credential.to_vec());
        noncanonical.push(0);
        assert!(matches!(
            decode_global_beacon_credential_v1(
                &noncanonical,
                catalog.network_id(),
                catalog.iter().next().expect("one binding"),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));
    }

    #[test]
    fn duplicate_empty_and_invalid_session_inventories_fail_closed() {
        let network_id = network_id_v1(0xC1);
        assert!(matches!(
            encode_global_beacon_partial_signer_credential_v1(
                network_id,
                HANDLE,
                REVISION,
                POLICY_DIGEST,
                Vec::new(),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));

        let fixture = beacon_fixture_v1(network_id, 0x74);
        let duplicate = beacon_fixture_v1(network_id, 0x74);
        let duplicate_provisioning = vec![
            RuntimeGlobalBeaconShareProvisioningV1::new(fixture.record, 1, fixture.components),
            RuntimeGlobalBeaconShareProvisioningV1::new(duplicate.record, 1, duplicate.components),
        ];
        let duplicate_policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &duplicate_provisioning)
                .expect("derive duplicate beacon inventory digest");
        assert!(matches!(
            encode_global_beacon_partial_signer_credential_v1(
                network_id,
                HANDLE,
                REVISION,
                duplicate_policy_digest,
                duplicate_provisioning,
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));

        let fixture = tle_fixture_v1(network_id, 0x75);
        let invalid_provisioning = vec![RuntimeParliamentTleShareProvisioningV1::new(
            fixture.validated.public_state().clone(),
            1,
            Zeroizing::new([[0; 32]; 3]),
        )];
        let invalid_policy_digest = parliament_tle_partial_release_signer_inventory_digest_v1(
            network_id,
            &invalid_provisioning,
        )
        .expect("derive invalid-share TLE inventory digest");
        assert!(matches!(
            encode_parliament_tle_partial_release_signer_credential_v1(
                network_id,
                HANDLE,
                REVISION,
                invalid_policy_digest,
                invalid_provisioning,
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));
    }

    #[test]
    fn missing_insecure_and_symlink_credentials_fail_closed() {
        let network_id = network_id_v1(0xC1);
        let fixture = beacon_fixture_v1(network_id, 0x76);
        let provisioning = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            fixture.record,
            1,
            fixture.components,
        )];
        let policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &provisioning)
                .expect("derive credential-source inventory digest");
        let catalog = beacon_catalog_v1(policy_digest);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            policy_digest,
            provisioning,
        )
        .expect("encode valid credential");
        assert!(matches!(
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &catalog, None,
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)
        ));
        let (_guard, directory) = secure_credential_directory_v1();
        assert!(matches!(
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &catalog,
                Some(&directory),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)
        ));

        let path = write_credential_v1(
            &directory,
            GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1,
            &credential,
            0o644,
        );
        assert!(matches!(
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &catalog,
                Some(&directory),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));
        fs::remove_file(&path).expect("remove insecure credential");
        let target = write_credential_v1(&directory, "credential-target", &credential, 0o600);
        symlink(&target, &path).expect("substitute credential symlink");
        assert!(matches!(
            RuntimeConsensusThresholdSignerBackendsV1::load_from_credential_directory_v1(
                &catalog,
                Some(&directory),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
                | Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Unavailable)
        ));
    }

    #[test]
    fn qualification_drift_and_test_marking_remain_fail_closed() {
        let network_id = network_id_v1(0xC1);
        let fixture = beacon_fixture_v1(network_id, 0x77);
        let provisioning = vec![RuntimeGlobalBeaconShareProvisioningV1::new(
            fixture.record,
            1,
            fixture.components,
        )];
        let policy_digest =
            global_beacon_partial_signer_inventory_digest_v1(network_id, &provisioning)
                .expect("derive qualification inventory digest");
        let catalog = beacon_catalog_v1(policy_digest);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            network_id,
            HANDLE,
            REVISION,
            policy_digest,
            provisioning,
        )
        .expect("encode valid credential");
        let backend = decode_global_beacon_credential_v1(
            &credential,
            catalog.network_id(),
            catalog.iter().next().expect("one binding"),
        )
        .expect("decode exact credential");
        let mut drifted = RuntimeConsensusThresholdSignerBackendsV1 {
            global_beacon: Some(backend),
            parliament_tle: None,
        };
        Arc::get_mut(
            drifted
                .global_beacon
                .as_mut()
                .expect("unique beacon backend"),
        )
        .expect("backend has one owner")
        .qualification =
            ConsensusSignerProviderQualificationV1::new(REVISION + 1, policy_digest, false);
        assert!(matches!(
            drifted.resolve(&catalog),
            Err(IrohaRuntimeProviderRegistryErrorV1::BindingMismatch)
        ));
        Arc::get_mut(
            drifted
                .global_beacon
                .as_mut()
                .expect("unique beacon backend"),
        )
        .expect("backend has one owner")
        .qualification = ConsensusSignerProviderQualificationV1::new(REVISION, policy_digest, true);
        assert!(matches!(
            drifted.resolve(&catalog),
            Err(IrohaRuntimeProviderRegistryErrorV1::TestProviderRejected)
        ));
    }

    fn network_id_v1(marker: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([marker; 32]),
        ))
    }
}
