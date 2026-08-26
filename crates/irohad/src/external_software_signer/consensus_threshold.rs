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
//! compromise. Operators rotate the supervised broker with a replacement
//! credential inventory; Core's committed-state retirement gates remain the
//! authority for deciding when an old share may be removed.

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
use std::{fmt, path::Path, sync::Arc};
use zeroize::{Zeroize as _, Zeroizing};

/// Fixed supervisor credential containing global-beacon software shares.
pub const GLOBAL_BEACON_PARTIAL_SIGNER_CREDENTIAL_NAME_V1: &str =
    "iroha-global-beacon-partial-signer-v1.norito";
/// Fixed supervisor credential containing Parliament TLE release shares.
pub const PARLIAMENT_TLE_PARTIAL_RELEASE_SIGNER_CREDENTIAL_NAME_V1: &str =
    "iroha-parliament-tle-partial-release-signer-v1.norito";

const CONSENSUS_THRESHOLD_CREDENTIAL_MAGIC_V1: [u8; 8] = *b"IRTHR001";
const CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1: u16 = 1;
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
struct RuntimeGlobalBeaconShareCredentialWireV1 {
    public_session: GlobalThresholdBeaconKeySessionV1,
    signer_index: u16,
    components: RuntimeSecretScalarTripleWireV1,
}

#[derive(NoritoSerialize, NoritoDeserialize)]
struct RuntimeGlobalBeaconSignerCredentialWireV1 {
    header: RuntimeConsensusThresholdCredentialHeaderWireV1,
    sessions: Vec<RuntimeGlobalBeaconShareCredentialWireV1>,
}

#[derive(NoritoSerialize, NoritoDeserialize)]
struct RuntimeParliamentTleShareCredentialWireV1 {
    public_session: TleKeySessionPublicStateV1,
    participant_index: u16,
    components: RuntimeSecretScalarTripleWireV1,
}

#[derive(NoritoSerialize, NoritoDeserialize)]
struct RuntimeParliamentTleSignerCredentialWireV1 {
    header: RuntimeConsensusThresholdCredentialHeaderWireV1,
    sessions: Vec<RuntimeParliamentTleShareCredentialWireV1>,
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
    validate_provisioning_header_v1(
        &network_id,
        &handle.into(),
        revision,
        policy_digest,
        |handle| {
            let header = credential_header_v1(
                IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
                network_id,
                handle,
                revision,
                policy_digest,
            );
            let sessions = encode_global_beacon_sessions_v1(&network_id, sessions)?;
            encode_secret_credential_v1(&RuntimeGlobalBeaconSignerCredentialWireV1 {
                header,
                sessions,
            })
        },
    )
}

fn encode_global_beacon_sessions_v1(
    network_id: &NetworkId,
    sessions: Vec<RuntimeGlobalBeaconShareProvisioningV1>,
) -> Result<
    Vec<RuntimeGlobalBeaconShareCredentialWireV1>,
    RuntimeConsensusThresholdSignerCredentialErrorV1,
> {
    validate_session_count_v1(sessions.len())?;
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
    validate_provisioning_header_v1(
        &network_id,
        &handle.into(),
        revision,
        policy_digest,
        |handle| {
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
        },
    )
}

fn encode_parliament_tle_sessions_v1(
    network_id: &NetworkId,
    sessions: Vec<RuntimeParliamentTleShareProvisioningV1>,
) -> Result<
    Vec<RuntimeParliamentTleShareCredentialWireV1>,
    RuntimeConsensusThresholdSignerCredentialErrorV1,
> {
    validate_session_count_v1(sessions.len())?;
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
        norito::encode_canonical(wire)
            .map_err(|_| RuntimeConsensusThresholdSignerCredentialErrorV1::Encoding)?,
    );
    if encoded.is_empty() || encoded.len() > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1 {
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
            .map_err(|_| GlobalBeaconPartialSignerBrokerBackendErrorV1)
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
            .map_err(|_| ParliamentTlePartialReleaseSignerBrokerBackendErrorV1)
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
