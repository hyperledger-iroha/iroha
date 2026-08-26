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

/// Direct credential tests and proof-valid fixtures shared with broker socket tests.
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::external_software_signer::ExternalSoftwareSignerBackendsV1;
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
            AdaptiveThresholdBlsParameters, BeaconPurpose, DasRenDealerSecret, ThresholdBlsSession,
            TleReleasePurpose, ValidatedDealerCommitment,
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
    use sha2::{Digest as _, Sha256};
    use std::{
        fs,
        io::Write as _,
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

    fn beacon_fixture_v1(network_id: NetworkId, session_byte: u8) -> BeaconFixtureV1 {
        let dkg_session = GlobalThresholdBeaconDkgSessionV1 {
            version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
            network_id,
            session_id: [session_byte; 32],
            roster_hash: [0x31; 32],
            committee_size: 4,
            threshold: 2,
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
        .expect("construct four-seat beacon threshold session");
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

    fn tle_fixture_v1(network_id: NetworkId, session_byte: u8) -> TleFixtureV1 {
        let threshold_session = ThresholdBlsSession::<TleReleasePurpose>::new(
            *network_id.as_bytes(),
            [session_byte; 32],
            [0x41; 32],
            4,
            2,
        )
        .expect("construct four-seat TLE threshold session");
        let parameters = AdaptiveThresholdBlsParameters::derive(&threshold_session)
            .expect("derive TLE fixture parameters");
        let mut rng = StdRng::from_seed([session_byte.wrapping_add(0x31); 32]);
        let mut dealers = Vec::with_capacity(4);
        let mut components = Zeroizing::new([[0_u8; 32]; 3]);
        for dealer_index in 1_u16..=4 {
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
        let validated = ValidatedTleKeySessionV1::from_qualified_dealers(
            threshold_session,
            &dealers,
            &[1, 2, 3, 4],
            [0x51; 32],
        )
        .expect("finalize proof-valid TLE fixture");
        TleFixtureV1 {
            threshold_session,
            validated,
            components,
        }
    }

    fn beacon_catalog_with_revision_v1(revision: u64) -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1::qualified_for_test(
            "consensus-threshold-credential-test",
            IrohaRuntimeProviderSlotV1::GlobalBeaconPartialSigner,
            HANDLE,
            revision,
            POLICY_DIGEST,
        )
    }

    fn beacon_catalog_v1() -> IrohaRuntimeProviderBindingsV1 {
        beacon_catalog_with_revision_v1(REVISION)
    }

    fn tle_catalog_with_revision_v1(revision: u64) -> IrohaRuntimeProviderBindingsV1 {
        IrohaRuntimeProviderBindingsV1::qualified_for_test(
            "consensus-threshold-credential-test",
            IrohaRuntimeProviderSlotV1::ParliamentTlePartialReleaseSigner,
            HANDLE,
            revision,
            POLICY_DIGEST,
        )
    }

    fn tle_catalog_v1() -> IrohaRuntimeProviderBindingsV1 {
        tle_catalog_with_revision_v1(REVISION)
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

    /// Builds a beacon fixture through canonical provisioning, decode, and resolution.
    pub(crate) fn consensus_threshold_beacon_broker_test_fixture_v1()
    -> ConsensusThresholdBeaconBrokerTestFixtureV1 {
        let catalog = beacon_catalog_v1();
        let fixture = beacon_fixture_v1(*catalog.network_id(), 0x79);
        let session = fixture.validated;
        let credential = encode_global_beacon_partial_signer_credential_v1(
            *catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![RuntimeGlobalBeaconShareProvisioningV1::new(
                fixture.record,
                1,
                fixture.components,
            )],
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

    /// Builds a TLE fixture through canonical provisioning, decode, and resolution.
    pub(crate) fn consensus_threshold_tle_broker_test_fixture_v1()
    -> ConsensusThresholdTleBrokerTestFixtureV1 {
        let catalog = tle_catalog_v1();
        let fixture = tle_fixture_v1(*catalog.network_id(), 0x7A);
        let (projection, identity) = tle_projection_v1(&fixture);
        let projection = projection.projection().clone();
        let session = fixture.validated.clone();
        let credential = encode_parliament_tle_partial_release_signer_credential_v1(
            *catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![RuntimeParliamentTleShareProvisioningV1::new(
                fixture.validated.public_state().clone(),
                1,
                fixture.components,
            )],
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
        let catalog = beacon_catalog_v1();
        let fixture = beacon_fixture_v1(*catalog.network_id(), 0x71);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            *catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![RuntimeGlobalBeaconShareProvisioningV1::new(
                fixture.record,
                1,
                fixture.components,
            )],
        )
        .expect("encode beacon runtime credential");
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
            GlobalThresholdBeaconPulseAggregatorV1::new(fixture.validated.clone(), 41, anchor)
                .expect("construct canonical beacon pulse");
        let partial = loaded
            .global_beacon
            .as_deref()
            .expect("loaded beacon backend")
            .sign_partial(&fixture.validated, verifier.payload())
            .expect("sign exact beacon pulse");
        assert!(
            verifier
                .accept_partial(partial)
                .expect("independently verify beacon partial")
        );
    }

    #[test]
    fn parliament_tle_credential_loads_resolves_and_signs_verified_partial() {
        let catalog = tle_catalog_v1();
        let fixture = tle_fixture_v1(*catalog.network_id(), 0x72);
        let (projection, identity) = tle_projection_v1(&fixture);
        let validated = fixture.validated.clone();
        let credential = encode_parliament_tle_partial_release_signer_credential_v1(
            *catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![RuntimeParliamentTleShareProvisioningV1::new(
                fixture.validated.public_state().clone(),
                1,
                fixture.components,
            )],
        )
        .expect("encode Parliament TLE runtime credential");
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
        let old_catalog = beacon_catalog_v1();
        let new_catalog = beacon_catalog_with_revision_v1(REVISION + 1);
        assert_eq!(old_catalog.network_id(), new_catalog.network_id());

        let predecessor = beacon_fixture_v1(*old_catalog.network_id(), 0x7B);
        let predecessor_session = predecessor.validated.clone();
        let successor = beacon_fixture_v1(*old_catalog.network_id(), 0x7C);
        let successor_session = successor.validated.clone();
        let old_credential = encode_global_beacon_partial_signer_credential_v1(
            *old_catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![
                RuntimeGlobalBeaconShareProvisioningV1::new(
                    predecessor.record,
                    1,
                    predecessor.components,
                ),
                RuntimeGlobalBeaconShareProvisioningV1::new(
                    successor.record,
                    1,
                    successor.components,
                ),
            ],
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

        let replacement_successor = beacon_fixture_v1(*new_catalog.network_id(), 0x7C);
        let new_credential = encode_global_beacon_partial_signer_credential_v1(
            *new_catalog.network_id(),
            HANDLE,
            REVISION + 1,
            POLICY_DIGEST,
            vec![RuntimeGlobalBeaconShareProvisioningV1::new(
                replacement_successor.record,
                1,
                replacement_successor.components,
            )],
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
        let old_catalog = tle_catalog_v1();
        let new_catalog = tle_catalog_with_revision_v1(REVISION + 1);
        assert_eq!(old_catalog.network_id(), new_catalog.network_id());

        let predecessor = tle_fixture_v1(*old_catalog.network_id(), 0x7D);
        let predecessor_session = predecessor.validated.clone();
        let (predecessor_projection, predecessor_identity) = tle_projection_v1(&predecessor);
        let successor = tle_fixture_v1(*old_catalog.network_id(), 0x7E);
        let successor_session = successor.validated.clone();
        let (successor_projection, successor_identity) = tle_projection_v1(&successor);
        let old_credential = encode_parliament_tle_partial_release_signer_credential_v1(
            *old_catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![
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
            ],
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

        let replacement_successor = tle_fixture_v1(*new_catalog.network_id(), 0x7E);
        let new_credential = encode_parliament_tle_partial_release_signer_credential_v1(
            *new_catalog.network_id(),
            HANDLE,
            REVISION + 1,
            POLICY_DIGEST,
            vec![RuntimeParliamentTleShareProvisioningV1::new(
                replacement_successor.validated.public_state().clone(),
                1,
                replacement_successor.components,
            )],
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
    fn credential_header_substitution_and_noncanonical_bytes_fail_closed() {
        let catalog = beacon_catalog_v1();
        let fixture = beacon_fixture_v1(*catalog.network_id(), 0x73);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            *catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![RuntimeGlobalBeaconShareProvisioningV1::new(
                fixture.record,
                1,
                fixture.components,
            )],
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
        let catalog = beacon_catalog_v1();
        assert!(matches!(
            encode_global_beacon_partial_signer_credential_v1(
                *catalog.network_id(),
                HANDLE,
                REVISION,
                POLICY_DIGEST,
                Vec::new(),
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));

        let fixture = beacon_fixture_v1(*catalog.network_id(), 0x74);
        let duplicate = beacon_fixture_v1(*catalog.network_id(), 0x74);
        assert!(matches!(
            encode_global_beacon_partial_signer_credential_v1(
                *catalog.network_id(),
                HANDLE,
                REVISION,
                POLICY_DIGEST,
                vec![
                    RuntimeGlobalBeaconShareProvisioningV1::new(
                        fixture.record,
                        1,
                        fixture.components,
                    ),
                    RuntimeGlobalBeaconShareProvisioningV1::new(
                        duplicate.record,
                        1,
                        duplicate.components,
                    ),
                ],
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));

        let fixture = tle_fixture_v1(*catalog.network_id(), 0x75);
        assert!(matches!(
            encode_parliament_tle_partial_release_signer_credential_v1(
                *catalog.network_id(),
                HANDLE,
                REVISION,
                POLICY_DIGEST,
                vec![RuntimeParliamentTleShareProvisioningV1::new(
                    fixture.validated.public_state().clone(),
                    1,
                    Zeroizing::new([[0; 32]; 3]),
                )],
            ),
            Err(RuntimeConsensusThresholdSignerCredentialErrorV1::Rejected)
        ));
    }

    #[test]
    fn missing_insecure_and_symlink_credentials_fail_closed() {
        let catalog = beacon_catalog_v1();
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

        let fixture = beacon_fixture_v1(*catalog.network_id(), 0x76);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            *catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![RuntimeGlobalBeaconShareProvisioningV1::new(
                fixture.record,
                1,
                fixture.components,
            )],
        )
        .expect("encode valid credential");
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
        let catalog = beacon_catalog_v1();
        let fixture = beacon_fixture_v1(*catalog.network_id(), 0x77);
        let credential = encode_global_beacon_partial_signer_credential_v1(
            *catalog.network_id(),
            HANDLE,
            REVISION,
            POLICY_DIGEST,
            vec![RuntimeGlobalBeaconShareProvisioningV1::new(
                fixture.record,
                1,
                fixture.components,
            )],
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
            ConsensusSignerProviderQualificationV1::new(REVISION + 1, POLICY_DIGEST, false);
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
        .qualification = ConsensusSignerProviderQualificationV1::new(REVISION, POLICY_DIGEST, true);
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
