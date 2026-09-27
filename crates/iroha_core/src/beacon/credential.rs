//! Runtime credential codec for global-beacon seat shares.
//!
//! A seat credential carries complete public DKG transcripts plus the
//! zeroizing aggregate scalar triple owned by one signer seat. Its header binds
//! the exact runtime-provider qualification (slot, handle, revision and public
//! inventory digest) and the genesis-derived network identity. The producer
//! ([`encode_global_beacon_partial_signer_credential_v1`]) and the runtime
//! importer ([`decode_global_beacon_partial_signer_credential_v1`]) replay the
//! same Core share-import checks, so a credential is neither written nor loaded
//! unless every share matches its public transcript and seat.
//!
//! The consensus-threshold framing (header, secret scalar triple, public
//! inventory digest and bounded canonical encoding) is shared with the
//! Parliament timed-release credential owned by the node's runtime broker.
//! Neither producer nor importer performs file I/O; credential bytes are handed
//! directly to a supervisor credential facility.

use std::io::Read as _;

use iroha_crypto::sha256_reader_bounded;
use iroha_data_model::{NetworkId, consensus::GlobalThresholdBeaconKeySessionV1};
use norito::{DecodeLimits, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;
use zeroize::{Zeroize as _, Zeroizing};

use super::{GlobalThresholdBeaconSessionBindingV1, RuntimeGlobalThresholdBeaconShareCustodyV1};

/// Stable runtime-provider slot wire identifier of the global-beacon partial signer.
pub const GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1: u16 = 59;
/// Sole first-release consensus-threshold credential and inventory version.
pub const CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1: u16 = 1;
/// Maximum canonical byte length of one consensus-threshold credential.
pub const MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum number of key sessions carried by one consensus-threshold credential.
pub const MAX_CONSENSUS_THRESHOLD_CREDENTIAL_SESSIONS_V1: usize = 64;
/// Bounded decode limits for one consensus-threshold credential of `len` encoded bytes.
///
/// Sequence length and nesting depth are fixed by the credential layout. Total
/// elements and allocation follow Norito's canonical per-length bounds: a
/// signed all-edge DKG transcript decodes to far more than it encodes (two
/// 31-seat sessions encode to 3.8 MB and allocate 48–64 MiB and 2–4 million
/// elements), so a fixed budget rejects valid large committees, while a
/// per-length budget keeps a small input from reserving a large one.
/// [`decode_consensus_threshold_credential_v1`] rejects credentials above
/// [`MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1`] before decoding.
#[must_use]
pub const fn consensus_threshold_credential_decode_limits_v1(len: usize) -> DecodeLimits {
    let canonical = norito::canonical_decode_limits(len);
    DecodeLimits::new(
        16_384,
        canonical.max_field_bytes(),
        canonical.max_total_elements(),
        canonical.max_total_allocated_bytes(),
        64,
    )
}

/// Decode one canonical consensus-threshold credential frame.
///
/// # Errors
///
/// Returns [`ConsensusThresholdCredentialErrorV1::Rejected`] for empty bytes,
/// bytes above [`MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1`], or bytes that
/// are not one canonical frame within
/// [`consensus_threshold_credential_decode_limits_v1`].
pub fn decode_consensus_threshold_credential_v1<T>(
    bytes: &[u8],
) -> Result<T, ConsensusThresholdCredentialErrorV1>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1 {
        return Err(ConsensusThresholdCredentialErrorV1::Rejected);
    }
    norito::decode_canonical_with_limits(
        bytes,
        consensus_threshold_credential_decode_limits_v1(bytes.len()),
    )
    .map_err(|_| ConsensusThresholdCredentialErrorV1::Rejected)
}

const CONSENSUS_THRESHOLD_CREDENTIAL_MAGIC_V1: [u8; 8] = *b"IRTHR001";
const CONSENSUS_THRESHOLD_PUBLIC_INVENTORY_DOMAIN_V1: &[u8] =
    b"iroha.runtime-consensus-threshold.public-inventory.v1";

/// Payload-free consensus-threshold credential codec failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ConsensusThresholdCredentialErrorV1 {
    /// The qualification, network, session inventory, or share was invalid.
    #[error("consensus threshold-signer credential was rejected")]
    Rejected,
    /// Canonical encoding failed or exceeded its fixed byte ceiling.
    #[error("consensus threshold-signer credential encoding failed")]
    Encoding,
}

/// Public header framing every consensus-threshold signer credential.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ConsensusThresholdCredentialHeaderV1 {
    /// Fixed credential magic `IRTHR001`.
    pub magic: [u8; 8],
    /// Credential version; only [`CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1`] is admitted.
    pub version: u16,
    /// Stable runtime-provider slot wire identifier.
    pub slot: u16,
    /// Genesis-derived network identity bound by every carried session.
    pub network_id: NetworkId,
    /// Production runtime-provider handle.
    pub handle: String,
    /// Nonzero public provider-catalog revision.
    pub revision: u64,
    /// Public inventory digest configured as the provider policy digest.
    pub policy_digest: [u8; 32],
}

impl ConsensusThresholdCredentialHeaderV1 {
    /// Build a current-version header for one runtime-provider slot.
    #[must_use]
    pub fn new(
        slot: u16,
        network_id: NetworkId,
        handle: String,
        revision: u64,
        policy_digest: [u8; 32],
    ) -> Self {
        Self {
            magic: CONSENSUS_THRESHOLD_CREDENTIAL_MAGIC_V1,
            version: CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1,
            slot,
            network_id,
            handle,
            revision,
            policy_digest,
        }
    }

    /// Require this header to name exactly the expected slot, network and qualification.
    ///
    /// # Errors
    ///
    /// Returns [`ConsensusThresholdCredentialErrorV1::Rejected`] for a wrong
    /// magic, version, slot, network, handle, revision or policy digest.
    pub fn validate(
        &self,
        slot: u16,
        network_id: &NetworkId,
        handle: &str,
        revision: u64,
        policy_digest: [u8; 32],
    ) -> Result<(), ConsensusThresholdCredentialErrorV1> {
        if self.magic != CONSENSUS_THRESHOLD_CREDENTIAL_MAGIC_V1
            || self.version != CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1
            || self.slot != slot
            || self.network_id != *network_id
            || self.handle != handle
            || self.revision != revision
            || self.policy_digest != policy_digest
            || self.revision == 0
            || self.policy_digest == [0; 32]
        {
            return Err(ConsensusThresholdCredentialErrorV1::Rejected);
        }
        Ok(())
    }
}

/// Zeroizing wire owner of one secret `(s, r, u)` scalar triple.
///
/// It has no `Clone` or `Debug` surface and scrubs itself on drop.
#[derive(NoritoSerialize, NoritoDeserialize)]
pub struct ConsensusThresholdSecretScalarTripleV1([[u8; 32]; 3]);

impl ConsensusThresholdSecretScalarTripleV1 {
    /// Move a zeroizing triple into its wire owner, scrubbing the source.
    #[must_use]
    pub fn from_zeroizing(mut components: Zeroizing<[[u8; 32]; 3]>) -> Self {
        Self(std::mem::take(&mut *components))
    }

    /// Move the triple back into a zeroizing owner, scrubbing the wire owner.
    #[must_use]
    pub fn into_zeroizing(mut self) -> Zeroizing<[[u8; 32]; 3]> {
        Zeroizing::new(std::mem::take(&mut self.0))
    }
}

impl Drop for ConsensusThresholdSecretScalarTripleV1 {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Compute the domain-separated digest of one canonical public inventory.
///
/// V1 computes `SHA-256(domain || u64_be(encoded_len) || canonical_norito(inventory))`.
///
/// # Errors
///
/// Returns [`ConsensusThresholdCredentialErrorV1::Encoding`] for an empty or
/// oversized encoding and [`ConsensusThresholdCredentialErrorV1::Rejected`]
/// for the all-zero digest.
pub fn consensus_threshold_public_inventory_digest_v1<T: NoritoSerialize>(
    inventory: &T,
) -> Result<[u8; 32], ConsensusThresholdCredentialErrorV1> {
    let encoded = norito::encode_canonical(inventory)
        .map_err(|_| ConsensusThresholdCredentialErrorV1::Encoding)?;
    if encoded.is_empty() || encoded.len() > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1 {
        return Err(ConsensusThresholdCredentialErrorV1::Encoding);
    }
    let encoded_len =
        u64::try_from(encoded.len()).map_err(|_| ConsensusThresholdCredentialErrorV1::Encoding)?;
    let encoded_len_bytes = encoded_len.to_be_bytes();
    let digest_input_len = CONSENSUS_THRESHOLD_PUBLIC_INVENTORY_DOMAIN_V1
        .len()
        .checked_add(encoded_len_bytes.len())
        .and_then(|len| len.checked_add(encoded.len()))
        .and_then(|len| u64::try_from(len).ok())
        .ok_or(ConsensusThresholdCredentialErrorV1::Encoding)?;
    let (digest, observed_len) = sha256_reader_bounded(
        CONSENSUS_THRESHOLD_PUBLIC_INVENTORY_DOMAIN_V1
            .chain(encoded_len_bytes.as_slice())
            .chain(encoded.as_slice()),
        digest_input_len,
    )
    .map_err(|_| ConsensusThresholdCredentialErrorV1::Encoding)?;
    if observed_len != digest_input_len {
        return Err(ConsensusThresholdCredentialErrorV1::Encoding);
    }
    if digest == [0; 32] {
        return Err(ConsensusThresholdCredentialErrorV1::Rejected);
    }
    Ok(digest)
}

/// Encode one secret credential with the canonical layout and a fixed byte ceiling.
///
/// Ambient Norito layout flags are ignored, so every producer emits the one
/// canonical byte string accepted by the importer.
///
/// # Errors
///
/// Returns [`ConsensusThresholdCredentialErrorV1::Encoding`] when encoding
/// fails, is empty, or exceeds [`MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1`].
pub fn encode_consensus_threshold_secret_credential_v1<T: NoritoSerialize>(
    wire: &T,
) -> Result<Zeroizing<Vec<u8>>, ConsensusThresholdCredentialErrorV1> {
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let encoded = Zeroizing::new(
        norito::core::to_bytes_bounded(wire, MAX_CONSENSUS_THRESHOLD_CREDENTIAL_BYTES_V1)
            .map_err(|_| ConsensusThresholdCredentialErrorV1::Encoding)?,
    );
    if encoded.is_empty() {
        return Err(ConsensusThresholdCredentialErrorV1::Encoding);
    }
    Ok(encoded)
}

/// Require a nonempty session inventory within the fixed credential ceiling.
///
/// # Errors
///
/// Returns [`ConsensusThresholdCredentialErrorV1::Rejected`] for an empty or
/// excessive inventory.
pub fn validate_consensus_threshold_session_count_v1(
    count: usize,
) -> Result<(), ConsensusThresholdCredentialErrorV1> {
    if count == 0 || count > MAX_CONSENSUS_THRESHOLD_CREDENTIAL_SESSIONS_V1 {
        return Err(ConsensusThresholdCredentialErrorV1::Rejected);
    }
    Ok(())
}

/// Validate the public qualification a producer is about to bind into a credential.
///
/// # Errors
///
/// Returns [`ConsensusThresholdCredentialErrorV1::Rejected`] for an all-zero
/// network, a non-production handle, a zero revision, or a zero digest.
pub fn validate_consensus_threshold_provisioning_v1(
    network_id: &NetworkId,
    handle: &str,
    revision: u64,
    policy_digest: [u8; 32],
) -> Result<(), ConsensusThresholdCredentialErrorV1> {
    if network_id.as_bytes().iter().all(|byte| *byte == 0)
        || iroha_config::parameters::validate_production_runtime_handle(handle).is_err()
        || revision == 0
        || policy_digest == [0; 32]
    {
        return Err(ConsensusThresholdCredentialErrorV1::Rejected);
    }
    Ok(())
}

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

    /// Complete public DKG transcript this share belongs to.
    #[must_use]
    pub fn public_session(&self) -> &GlobalThresholdBeaconKeySessionV1 {
        &self.public_session
    }

    /// One-based signer seat of this share.
    #[must_use]
    pub fn signer_index(&self) -> u16 {
        self.signer_index
    }
}

#[derive(NoritoSerialize, NoritoDeserialize)]
struct RuntimeGlobalBeaconShareCredentialWireV1 {
    public_session: GlobalThresholdBeaconKeySessionV1,
    signer_index: u16,
    components: ConsensusThresholdSecretScalarTripleV1,
}

#[derive(NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::beacon::credential::RuntimeGlobalBeaconSignerCredentialWireV1",
    frame = "iroha.runtime_provider_broker.v1.consensus_threshold.global_beacon_signer_credential"
)]
struct RuntimeGlobalBeaconSignerCredentialWireV1 {
    header: ConsensusThresholdCredentialHeaderV1,
    sessions: Vec<RuntimeGlobalBeaconShareCredentialWireV1>,
}

#[derive(NoritoSerialize)]
struct RuntimeGlobalBeaconPublicInventoryEntryWireV1 {
    public_session: GlobalThresholdBeaconKeySessionV1,
    signer_index: u16,
}

#[derive(NoritoSerialize, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::beacon::credential::RuntimeGlobalBeaconPublicInventoryWireV1",
    frame = "iroha.runtime_provider_broker.v1.consensus_threshold.global_beacon_public_inventory"
)]
struct RuntimeGlobalBeaconPublicInventoryWireV1 {
    version: u16,
    slot: u16,
    network_id: NetworkId,
    sessions: Vec<RuntimeGlobalBeaconPublicInventoryEntryWireV1>,
}

fn global_beacon_public_inventory_wire_v1(
    network_id: NetworkId,
    sessions: impl IntoIterator<Item = (GlobalThresholdBeaconKeySessionV1, u16)>,
) -> Result<RuntimeGlobalBeaconPublicInventoryWireV1, ConsensusThresholdCredentialErrorV1> {
    let mut sessions = sessions
        .into_iter()
        .map(|(public_session, signer_index)| {
            if public_session.network_id != network_id {
                return Err(ConsensusThresholdCredentialErrorV1::Rejected);
            }
            Ok(RuntimeGlobalBeaconPublicInventoryEntryWireV1 {
                public_session,
                signer_index,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_consensus_threshold_session_count_v1(sessions.len())?;
    sessions.sort_by(|left, right| {
        left.public_session
            .session_id
            .cmp(&right.public_session.session_id)
            .then_with(|| left.signer_index.cmp(&right.signer_index))
    });
    Ok(RuntimeGlobalBeaconPublicInventoryWireV1 {
        version: CONSENSUS_THRESHOLD_CREDENTIAL_VERSION_V1,
        slot: GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1,
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
) -> Result<[u8; 32], ConsensusThresholdCredentialErrorV1> {
    let inventory = global_beacon_public_inventory_wire_v1(
        network_id,
        sessions
            .iter()
            .map(|session| (session.public_session.clone(), session.signer_index)),
    )?;
    consensus_threshold_public_inventory_digest_v1(&inventory)
}

/// Compute the same beacon inventory binding from public sessions and seats only.
///
/// # Errors
///
/// Rejects empty, excessive, or cross-network inventories and encoding failure.
pub fn global_beacon_partial_signer_public_inventory_digest_v1(
    network_id: NetworkId,
    sessions: &[(GlobalThresholdBeaconKeySessionV1, u16)],
) -> Result<[u8; 32], ConsensusThresholdCredentialErrorV1> {
    let inventory = global_beacon_public_inventory_wire_v1(network_id, sessions.iter().cloned())?;
    consensus_threshold_public_inventory_digest_v1(&inventory)
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
) -> Result<Zeroizing<Vec<u8>>, ConsensusThresholdCredentialErrorV1> {
    let handle = handle.into();
    if global_beacon_partial_signer_inventory_digest_v1(network_id, &sessions)? != policy_digest {
        return Err(ConsensusThresholdCredentialErrorV1::Rejected);
    }
    validate_consensus_threshold_provisioning_v1(&network_id, &handle, revision, policy_digest)?;
    let header = ConsensusThresholdCredentialHeaderV1::new(
        GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1,
        network_id,
        handle,
        revision,
        policy_digest,
    );
    let sessions = encode_global_beacon_sessions_v1(&network_id, sessions)?;
    encode_consensus_threshold_secret_credential_v1(&RuntimeGlobalBeaconSignerCredentialWireV1 {
        header,
        sessions,
    })
}

fn encode_global_beacon_sessions_v1(
    network_id: &NetworkId,
    mut sessions: Vec<RuntimeGlobalBeaconShareProvisioningV1>,
) -> Result<Vec<RuntimeGlobalBeaconShareCredentialWireV1>, ConsensusThresholdCredentialErrorV1> {
    validate_consensus_threshold_session_count_v1(sessions.len())?;
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
            return Err(ConsensusThresholdCredentialErrorV1::Rejected);
        }
        let binding = global_beacon_session_binding_v1(&public_session);
        validation_custody
            .import_components(
                public_session.clone(),
                &binding,
                signer_index,
                Zeroizing::new(*components),
            )
            .map_err(|_| ConsensusThresholdCredentialErrorV1::Rejected)?;
        encoded.push(RuntimeGlobalBeaconShareCredentialWireV1 {
            public_session,
            signer_index,
            components: ConsensusThresholdSecretScalarTripleV1::from_zeroizing(components),
        });
    }
    Ok(encoded)
}

/// Decode and import one global-beacon seat credential for an exact qualification.
///
/// The header must name the global-beacon slot, `network_id` and the configured
/// `handle`, `revision` and `policy_digest`. The session vector must be
/// strictly sorted, and the recomputed public inventory digest must equal the
/// header digest before any share is imported. Every share is replayed through
/// Core's share-import validation.
///
/// # Errors
///
/// Returns [`ConsensusThresholdCredentialErrorV1::Rejected`] for noncanonical
/// bytes, a substituted header, a reordered or duplicate inventory, a digest
/// mismatch, or a share that does not match its public transcript and seat.
pub fn decode_global_beacon_partial_signer_credential_v1(
    bytes: &[u8],
    network_id: &NetworkId,
    handle: &str,
    revision: u64,
    policy_digest: [u8; 32],
) -> Result<RuntimeGlobalThresholdBeaconShareCustodyV1, ConsensusThresholdCredentialErrorV1> {
    let wire: RuntimeGlobalBeaconSignerCredentialWireV1 =
        decode_consensus_threshold_credential_v1(bytes)?;
    wire.header.validate(
        GLOBAL_BEACON_PARTIAL_SIGNER_SLOT_WIRE_ID_V1,
        network_id,
        handle,
        revision,
        policy_digest,
    )?;
    validate_consensus_threshold_session_count_v1(wire.sessions.len())?;
    if wire.sessions.windows(2).any(|pair| {
        pair[0]
            .public_session
            .session_id
            .cmp(&pair[1].public_session.session_id)
            .then_with(|| pair[0].signer_index.cmp(&pair[1].signer_index))
            .is_ge()
    }) {
        return Err(ConsensusThresholdCredentialErrorV1::Rejected);
    }
    let public_inventory = global_beacon_public_inventory_wire_v1(
        *network_id,
        wire.sessions
            .iter()
            .map(|session| (session.public_session.clone(), session.signer_index)),
    )?;
    if consensus_threshold_public_inventory_digest_v1(&public_inventory)?
        != wire.header.policy_digest
    {
        return Err(ConsensusThresholdCredentialErrorV1::Rejected);
    }
    let custody = RuntimeGlobalThresholdBeaconShareCustodyV1::new();
    for session in wire.sessions {
        if session.public_session.network_id != *network_id {
            return Err(ConsensusThresholdCredentialErrorV1::Rejected);
        }
        let binding = global_beacon_session_binding_v1(&session.public_session);
        custody
            .import_components(
                session.public_session,
                &binding,
                session.signer_index,
                session.components.into_zeroizing(),
            )
            .map_err(|_| ConsensusThresholdCredentialErrorV1::Rejected)?;
    }
    Ok(custody)
}

/// Decode one global-beacon seat credential for an exact qualification and return its shares.
///
/// The credential passes every check of [`decode_global_beacon_partial_signer_credential_v1`]
/// before any share is returned, so each share matches its public transcript and seat. The shares
/// keep the credential's canonical order. A caller that extends a retained inventory, such as a
/// prepared committee rotation appending its pending share, re-encodes the complete inventory
/// through [`encode_global_beacon_partial_signer_credential_v1`].
///
/// # Errors
///
/// The errors of [`decode_global_beacon_partial_signer_credential_v1`].
pub fn decode_global_beacon_partial_signer_credential_shares_v1(
    bytes: &[u8],
    network_id: &NetworkId,
    handle: &str,
    revision: u64,
    policy_digest: [u8; 32],
) -> Result<Vec<RuntimeGlobalBeaconShareProvisioningV1>, ConsensusThresholdCredentialErrorV1> {
    let custody = decode_global_beacon_partial_signer_credential_v1(
        bytes,
        network_id,
        handle,
        revision,
        policy_digest,
    )?;
    drop(custody);
    let RuntimeGlobalBeaconSignerCredentialWireV1 { header, sessions } =
        decode_consensus_threshold_credential_v1(bytes)?;
    drop(header);
    Ok(sessions
        .into_iter()
        .map(|session| {
            RuntimeGlobalBeaconShareProvisioningV1::new(
                session.public_session,
                session.signer_index,
                session.components.into_zeroizing(),
            )
        })
        .collect())
}

/// Read the public header of one canonical global-beacon seat credential.
///
/// The complete credential is decoded canonically, so no header is read from
/// malformed, truncated or trailing bytes. Every carried share is dropped, and
/// scrubbed, before this returns. The header is not checked against any
/// expected qualification: a launcher that derives the provider binding from
/// it must still import the credential through
/// [`decode_global_beacon_partial_signer_credential_v1`] with the header's
/// handle, revision and policy digest and the expected network.
///
/// # Errors
///
/// Returns [`ConsensusThresholdCredentialErrorV1::Rejected`] for bytes that
/// are not one canonical global-beacon credential.
pub fn global_beacon_partial_signer_credential_header_v1(
    bytes: &[u8],
) -> Result<ConsensusThresholdCredentialHeaderV1, ConsensusThresholdCredentialErrorV1> {
    let RuntimeGlobalBeaconSignerCredentialWireV1 { header, sessions } =
        decode_consensus_threshold_credential_v1(bytes)?;
    drop(sessions);
    Ok(header)
}

fn global_beacon_session_binding_v1(
    record: &GlobalThresholdBeaconKeySessionV1,
) -> GlobalThresholdBeaconSessionBindingV1 {
    GlobalThresholdBeaconSessionBindingV1 {
        network_id: record.network_id,
        session_id: record.session_id,
        roster_hash: record.roster_hash,
        transcript_hash: record.transcript_hash,
    }
}

#[cfg(test)]
#[path = "credential_tests.rs"]
mod tests;
