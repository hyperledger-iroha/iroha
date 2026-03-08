//! Governance data model shells (feature-gated)
//!
//! Minimal types to anchor implementation against gov.md and roadmap.md. Fields
//! and semantics will be expanded during implementation.
//!
//! Notes:
//! - Use the `SignedBlock` v1 Norito serialization for any `call_selector(inner)` and certificate hashing contexts.
//! - Fixed-point thresholds are represented as integers; Q-format mapping is specified in docs.

use std::{collections::BTreeMap, fmt, str::FromStr, string::String, vec::Vec};

use iroha_crypto::{
    PublicKey, SignatureOf,
    blake2::{
        Blake2bVar,
        digest::{Update, VariableOutput},
    },
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize, Parser};

use crate::{
    account::AccountId, asset::AssetId, isi::governance::CouncilDerivationKind,
    runtime::RuntimeUpgradeManifest, smart_contract::manifest::ManifestProvenance,
};

/// Errors emitted when parsing hex-encoded hashes used by governance payloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HashParseError {
    /// The decoded byte length differed from the expected size.
    InvalidLength {
        /// Number of bytes required by the target hash type.
        expected: usize,
        /// Actual number of bytes produced by the decoder.
        actual: usize,
    },
    /// The provided string was not valid lowercase hexadecimal.
    InvalidHex {
        /// Human-readable error message returned by the hex decoder.
        message: String,
    },
}

impl fmt::Display for HashParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { expected, actual } => {
                write!(f, "expected {expected} bytes, got {actual}")
            }
            Self::InvalidHex { message } => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for HashParseError {}

const HASH_WIRE_VERSION_V1: u16 = 1;

#[derive(Clone, Copy, Debug, Encode, Decode)]
struct HashWire32 {
    version: u16,
    declared_len: u16,
    bytes: [u8; 32],
}

impl HashWire32 {
    const fn new(bytes: [u8; 32]) -> Self {
        Self {
            version: HASH_WIRE_VERSION_V1,
            declared_len: 32,
            bytes,
        }
    }

    fn try_into_bytes(self) -> Result<[u8; 32], norito::core::Error> {
        if self.version != HASH_WIRE_VERSION_V1 {
            return Err(norito::core::Error::LengthMismatch);
        }
        if self.declared_len as usize != 32 {
            return Err(norito::core::Error::LengthMismatch);
        }
        Ok(self.bytes)
    }
}

fn decode_hex_array<const N: usize>(input: &str) -> Result<[u8; N], HashParseError> {
    let bytes = hex::decode(input).map_err(|err| HashParseError::InvalidHex {
        message: format!("{err}"),
    })?;
    if bytes.len() != N {
        return Err(HashParseError::InvalidLength {
            expected: N,
            actual: bytes.len(),
        });
    }
    let mut array = [0_u8; N];
    array.copy_from_slice(&bytes);
    Ok(array)
}

fn parse_hash_payload<const N: usize>(payload: &[u8]) -> Result<[u8; N], norito::core::Error> {
    if payload.len() == N {
        let mut array = [0u8; N];
        array.copy_from_slice(payload);
        return Ok(array);
    }
    if payload.len() >= 4 + N {
        let mut version_bytes = [0u8; 2];
        version_bytes.copy_from_slice(&payload[..2]);
        let version = u16::from_le_bytes(version_bytes);
        let mut len_bytes = [0u8; 2];
        len_bytes.copy_from_slice(&payload[2..4]);
        let declared = u16::from_le_bytes(len_bytes) as usize;
        if version == HASH_WIRE_VERSION_V1 && declared == N {
            let mut array = [0u8; N];
            array.copy_from_slice(&payload[4..4 + N]);
            return Ok(array);
        }
    }
    Err(norito::core::Error::LengthMismatch)
}

macro_rules! define_hash32_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, IntoSchema)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Number of bytes in the encoded hash.
            pub const LENGTH: usize = 32;

            /// Construct the hash wrapper from raw bytes.
            pub const fn new(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Attempt to parse the hash from a lowercase hexadecimal string.
            ///
            /// # Errors
            /// Returns [`HashParseError`] when the input is not a valid 32-byte hex string.
            pub fn from_hex_str(input: &str) -> Result<Self, HashParseError> {
                decode_hex_array::<32>(input).map(Self)
            }

            /// Borrow the raw hash bytes.
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Consume the wrapper and return the raw bytes.
            pub const fn into_bytes(self) -> [u8; 32] {
                self.0
            }

            /// Render the hash as a lowercase hexadecimal string.
            pub fn to_hex(&self) -> String {
                hex::encode(self.0)
            }
        }

        impl From<[u8; 32]> for $name {
            fn from(bytes: [u8; 32]) -> Self {
                Self::new(bytes)
            }
        }

        impl From<$name> for [u8; 32] {
            fn from(hash: $name) -> Self {
                hash.into_bytes()
            }
        }

        impl AsRef<[u8; 32]> for $name {
            fn as_ref(&self) -> &[u8; 32] {
                self.as_bytes()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_hex())
            }
        }

        impl norito::core::NoritoSerialize for $name {
            fn serialize<W: std::io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), norito::core::Error> {
                let wire = HashWire32::new(self.0);
                <HashWire32 as norito::core::NoritoSerialize>::serialize(&wire, &mut writer)
            }

            fn encoded_len_hint(&self) -> Option<usize> {
                let wire = HashWire32::new(self.0);
                <HashWire32 as norito::core::NoritoSerialize>::encoded_len_hint(&wire)
            }

            fn encoded_len_exact(&self) -> Option<usize> {
                let wire = HashWire32::new(self.0);
                <HashWire32 as norito::core::NoritoSerialize>::encoded_len_exact(&wire)
            }
        }

        impl<'de> norito::core::NoritoDeserialize<'de> for $name {
            fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
                Self::try_deserialize(archived).expect("fixed-length hash decode should succeed")
            }

            fn try_deserialize(
                archived: &'de norito::core::Archived<Self>,
            ) -> Result<Self, norito::core::Error> {
                let ptr = core::ptr::from_ref(archived).cast::<u8>();
                let bytes = norito::core::payload_slice_from_ptr(ptr)?;
                match norito::core::decode_field_canonical::<HashWire32>(bytes) {
                    Ok((wire, _used)) => {
                        if let Ok(array) = wire.try_into_bytes() {
                            return Ok(Self(array));
                        }
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "hash wire mismatch version={} len={}",
                                wire.version, wire.declared_len
                            );
                            let preview = bytes.iter().take(40).copied().collect::<Vec<_>>();
                            eprintln!("hash wire bytes preview {:?}", preview);
                        }
                    }
                    Err(err) => {
                        if norito::debug_trace_enabled() {
                            let preview = bytes.iter().take(40).copied().collect::<Vec<_>>();
                            eprintln!(
                                "hash wire canonical decode failed err={:?} preview {:?}",
                                err, preview
                            );
                        }
                    }
                }
                let (vec, _used) = norito::core::decode_field_canonical::<Vec<u8>>(bytes)?;
                match parse_hash_payload::<32>(&vec) {
                    Ok(array) => Ok(Self(array)),
                    Err(norito::core::Error::LengthMismatch) => {
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "decode hash fallback vec_len={} prefix={:?}",
                                vec.len(),
                                vec.iter().take(32).collect::<Vec<_>>()
                            );
                        }
                        if vec.len() != 32 {
                            return Err(norito::core::Error::LengthMismatch);
                        }
                        let mut array = [0u8; 32];
                        array.copy_from_slice(&vec);
                        Ok(Self(array))
                    }
                    Err(err) => Err(err),
                }
            }
        }

        impl<'a> norito::core::DecodeFromSlice<'a> for $name {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                if let Ok((wire, used)) = norito::core::decode_field_canonical::<HashWire32>(bytes)
                {
                    if let Ok(array) = wire.try_into_bytes() {
                        return Ok((Self(array), used));
                    }
                    if norito::debug_trace_enabled() {
                        eprintln!(
                            "hash wire slice mismatch version={} len={}",
                            wire.version, wire.declared_len
                        );
                    }
                }
                let (vec, used) = norito::core::decode_field_canonical::<Vec<u8>>(bytes)?;
                match parse_hash_payload::<32>(&vec) {
                    Ok(array) => Ok((Self(array), used)),
                    Err(norito::core::Error::LengthMismatch) => {
                        if norito::debug_trace_enabled() {
                            eprintln!(
                                "decode_from_slice fallback vec_len={} prefix={:?}",
                                vec.len(),
                                vec.iter().take(32).collect::<Vec<_>>()
                            );
                        }
                        if vec.len() != 32 {
                            return Err(norito::core::Error::LengthMismatch);
                        }
                        let mut array = [0u8; 32];
                        array.copy_from_slice(&vec);
                        Ok((Self(array), used))
                    }
                    Err(err) => Err(err),
                }
            }
        }

        #[cfg(feature = "json")]
        impl JsonSerialize for $name {
            fn json_serialize(&self, out: &mut String) {
                json::write_json_string(&self.to_hex(), out);
            }
        }

        #[cfg(feature = "json")]
        impl JsonDeserialize for $name {
            fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
                let buf = parser.parse_string()?;
                Self::from_hex_str(&buf).map_err(|err| json::Error::Message(format!("{err}")))
            }
        }

        impl FromStr for $name {
            type Err = HashParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::from_hex_str(s)
            }
        }
    };
}

define_hash32_newtype!(
    ContractCodeHash,
    "Blake2b-32 hash identifying a contract bytecode artifact."
);

define_hash32_newtype!(
    ContractAbiHash,
    "Blake2b-32 hash describing the ABI surface of a contract."
);

/// ABI version targeted by the contract manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct AbiVersion(u16);

impl AbiVersion {
    /// Create a new ABI version wrapper.
    pub const fn new(version: u16) -> Self {
        Self(version)
    }

    /// Borrow the underlying numeric version.
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl Default for AbiVersion {
    fn default() -> Self {
        Self::new(1)
    }
}

impl From<u16> for AbiVersion {
    fn from(value: u16) -> Self {
        Self::new(value)
    }
}

impl From<AbiVersion> for u16 {
    fn from(version: AbiVersion) -> Self {
        version.get()
    }
}

impl fmt::Display for AbiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for AbiVersion {
    fn json_serialize(&self, out: &mut String) {
        json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for AbiVersion {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        u16::json_deserialize(parser).map(Self::from)
    }
}

/// Governance proposal kinds supported today.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "payload"),
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub enum ProposalKind {
    /// Deploy an IVM contract identified by namespace + contract id and content hashes.
    DeployContract(DeployContractProposal),
    /// Schedule a runtime upgrade manifest through governance.
    RuntimeUpgrade(RuntimeUpgradeProposal),
}

/// Proposal payload for deploying an IVM contract via governance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DeployContractProposal {
    /// Governance namespace to which the proposal applies.
    pub namespace: String,
    /// Logical contract identifier within the namespace.
    pub contract_id: String,
    /// Blake2b-32 hash of the compiled `.to` bytecode.
    pub code_hash_hex: ContractCodeHash,
    /// Blake2b-32 hash of the ABI surface expected by hosts.
    pub abi_hash_hex: ContractAbiHash,
    /// ABI version string (e.g., `1`).
    pub abi_version: AbiVersion,
    /// Optional manifest provenance used to attest the manifest when absent on-chain.
    #[norito(default)]
    pub manifest_provenance: Option<ManifestProvenance>,
}

/// Proposal payload for scheduling a runtime upgrade through governance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RuntimeUpgradeProposal {
    /// Canonical runtime-upgrade manifest payload.
    pub manifest: RuntimeUpgradeManifest,
}

/// Inclusive execution window for enactment certificates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AtWindow {
    /// First block in the enactment window (inclusive).
    pub lower: u64,
    /// Last block in the enactment window (inclusive).
    pub upper: u64,
}

/// Governance parameters (subset) — see gov.md for full spec.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct GovernanceParameters {
    /// Asset used to denominate voting power.
    pub voting_asset: AssetId,
    /// Base lock period applied to deposits in blocks.
    pub base_lock_period_blocks: u32,
    /// Whether abstain ballots count toward turnout.
    pub count_abstain_in_turnout: bool,
    /// Approval threshold encoded as Q32.32.
    pub approval_threshold: u64,
    /// Quorum threshold encoded as Q32.32.
    pub quorum_threshold: u64,
    /// Maximum number of simultaneously active referenda.
    pub max_active_referenda: u16,
    /// Maximum reduction in lock period for fast-track referenda (blocks).
    pub fast_track_max_reduction_blocks: u32,
    /// Slack applied when validating enactment windows (blocks).
    pub window_slack_blocks: u32,
    /// Base deposit required to submit a proposal.
    pub deposit_base: u128,
    /// Additional deposit required per byte of preimage.
    pub deposit_per_byte: u128,
    /// Additional deposit required per block of desired enactment window.
    pub deposit_per_block: u128,
}

/// Content-addressable proposal identifier (32-byte hash).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, IntoSchema)]
pub struct ProposalId(pub [u8; 32]);

impl norito::core::NoritoSerialize for ProposalId {
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), norito::core::Error> {
        let wire = HashWire32::new(self.0);
        <HashWire32 as norito::core::NoritoSerialize>::serialize(&wire, &mut writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for ProposalId {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("ProposalId must decode from fixed-length payload")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let bytes =
            norito::core::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        if let Ok((wire, _used)) = norito::core::decode_field_canonical::<HashWire32>(bytes)
            && let Ok(array) = wire.try_into_bytes()
        {
            return Ok(Self(array));
        }
        let (vec, _used) = norito::core::decode_field_canonical::<Vec<u8>>(bytes)?;
        match parse_hash_payload::<32>(&vec) {
            Ok(array) => Ok(Self(array)),
            Err(norito::core::Error::LengthMismatch) => {
                if norito::debug_trace_enabled() {
                    eprintln!(
                        "decode ProposalId fallback vec_len={} prefix={:?}",
                        vec.len(),
                        vec.iter().take(32).collect::<Vec<_>>()
                    );
                }
                if vec.len() != 32 {
                    return Err(norito::core::Error::LengthMismatch);
                }
                let mut array = [0u8; 32];
                array.copy_from_slice(&vec);
                Ok(Self(array))
            }
            Err(err) => Err(err),
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for ProposalId {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&hex::encode(self.0), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for ProposalId {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let buf = parser.parse_string()?;
        decode_hex_array::<32>(&buf)
            .map(Self)
            .map_err(|err| json::Error::Message(format!("{err}")))
    }
}

/// Minimal referendum status enumeration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum ReferendumStatus {
    /// Referendum has been submitted but not yet opened for voting.
    Proposed,
    /// Referendum is open for voting between the provided block bounds (inclusive `start`, `end`).
    Open(AtWindow),
    /// Referendum was approved by the electorate.
    Approved,
    /// Referendum was rejected by the electorate.
    Rejected,
    /// Referendum has been enacted on-chain.
    Enacted,
    /// Referendum was superseded by a newer proposal.
    Superseded,
    /// Referendum expired without reaching a conclusion.
    Expired,
}

/// Referendum shell (subset of fields).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct Referendum {
    /// Deterministic identifier derived from the referendum preimage.
    pub id: ProposalId,
    /// Account that submitted the referendum.
    pub proposer: AccountId,
    /// Blake2b-32 hash of the preimage payload.
    pub preimage_hash: [u8; 32],
    /// Referenda that must be approved prior to enacting this one.
    pub requires: Vec<ProposalId>,
    /// Human-readable summary of the proposal intent.
    pub summary: String,
    /// Deposit locked while the referendum is active.
    pub deposit: u128,
    /// Current referendum lifecycle stage.
    pub status: ReferendumStatus,
    /// Optional enactment window associated with approval.
    pub schedule: Option<AtWindow>,
}

/// Voter choice variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum VoteChoice {
    /// Support the referendum (Aye).
    Aye,
    /// Oppose the referendum (Nay).
    Nay,
    /// Neither support nor oppose (Abstain).
    Abstain,
}

/// Vote shell (conviction index is abstract for now).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct Vote {
    /// Referendum being voted on.
    pub referendum_id: ProposalId,
    /// Account casting the vote.
    pub voter: AccountId,
    /// Conviction strength index (e.g., `0..=k_max`); mapping in docs.
    pub conviction: u8,
    /// Ballot choice (Aye, Nay, or Abstain).
    pub choice: VoteChoice,
}

/// Simple threshold scheme with per-signer signatures.
pub const ENACTMENT_SIGNATURE_SCHEME_SIMPLE_THRESHOLD: u16 = 1;

/// Supported signature schemes for enactment certificates.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
pub enum EnactmentSignatureScheme {
    /// Per-signer signatures validated against the council roster.
    #[default]
    SimpleThreshold,
}

impl EnactmentSignatureScheme {
    /// Numeric scheme identifier for on-wire payloads.
    #[must_use]
    pub const fn scheme_id(self) -> u16 {
        match self {
            Self::SimpleThreshold => ENACTMENT_SIGNATURE_SCHEME_SIMPLE_THRESHOLD,
        }
    }

    /// Canonical string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SimpleThreshold => "simple_threshold",
        }
    }
}

impl fmt::Display for EnactmentSignatureScheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when parsing an enactment signature scheme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnactmentSignatureSchemeParseError(pub String);

impl fmt::Display for EnactmentSignatureSchemeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid enactment signature scheme `{}`", self.0)
    }
}

impl std::error::Error for EnactmentSignatureSchemeParseError {}

impl FromStr for EnactmentSignatureScheme {
    type Err = EnactmentSignatureSchemeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "simple_threshold" | "simple-threshold" | "simple" => Ok(Self::SimpleThreshold),
            other => Err(EnactmentSignatureSchemeParseError(other.to_string())),
        }
    }
}

/// Error returned when converting an unknown scheme id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnactmentSignatureSchemeIdError {
    /// Unsupported scheme identifier.
    pub scheme_id: u16,
}

impl fmt::Display for EnactmentSignatureSchemeIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unsupported enactment signature scheme id {}",
            self.scheme_id
        )
    }
}

impl std::error::Error for EnactmentSignatureSchemeIdError {}

impl TryFrom<u16> for EnactmentSignatureScheme {
    type Error = EnactmentSignatureSchemeIdError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            ENACTMENT_SIGNATURE_SCHEME_SIMPLE_THRESHOLD => Ok(Self::SimpleThreshold),
            scheme_id => Err(EnactmentSignatureSchemeIdError { scheme_id }),
        }
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for EnactmentSignatureScheme {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(self.as_str(), out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for EnactmentSignatureScheme {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        Self::from_str(&value).map_err(|_| json::Error::unknown_field(value))
    }
}

/// Governance enactment certificate payload (signed via [`EnactmentSignatureScheme`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct GovernanceEnactment {
    /// Referendum that authorised the enactment.
    pub referendum_id: ProposalId,
    /// Blake2b-32 hash of the referendum preimage.
    pub preimage_hash: [u8; 32],
    /// Inclusive window in which enactment is valid.
    pub at_window: AtWindow,
}

/// Signature over a governance enactment payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct GovernanceEnactmentSignature {
    /// Council member account that produced the signature.
    pub signer: AccountId,
    /// Public key used to verify the signature.
    pub public_key: PublicKey,
    /// Signature of the enactment payload.
    pub signature: SignatureOf<GovernanceEnactment>,
}

/// Signature bundle for governance enactment certificates.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct GovernanceEnactmentSignatureSet {
    /// Signature scheme identifier.
    pub scheme: EnactmentSignatureScheme,
    /// Collected signatures ordered deterministically by signer account id.
    pub signatures: Vec<GovernanceEnactmentSignature>,
}

/// Signed governance enactment certificate.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct GovernanceEnactmentCertificate {
    /// Enactment payload being authorized.
    pub payload: GovernanceEnactment,
    /// Signature bundle proving authorization.
    pub signatures: GovernanceEnactmentSignatureSet,
}

/// Parliament governance body identifiers.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
pub enum ParliamentBody {
    /// Rules Committee — intake and rulebook gate.
    RulesCommittee,
    /// Agenda Council — schedules referendum windows and admission order.
    #[default]
    AgendaCouncil,
    /// Interest Panels — subject-matter reviewers for the proposal domain.
    InterestPanel,
    /// Review Panel — cross-domain review ahead of policy jury.
    ReviewPanel,
    /// Policy Jury — large-jury decision body for approval/rejection.
    PolicyJury,
    /// Oversight Committee — monitors deadlines and escalation paths.
    OversightCommittee,
    /// MPC/FMA board — final multi-party check for enactment readiness.
    FmaCommittee,
}

#[cfg(feature = "json")]
impl json::JsonSerialize for ParliamentBody {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            ParliamentBody::RulesCommittee => "rules-committee",
            ParliamentBody::AgendaCouncil => "agenda-council",
            ParliamentBody::InterestPanel => "interest-panel",
            ParliamentBody::ReviewPanel => "review-panel",
            ParliamentBody::PolicyJury => "policy-jury",
            ParliamentBody::OversightCommittee => "oversight-committee",
            ParliamentBody::FmaCommittee => "fma-committee",
        };
        json::write_json_string(label, out);
    }
}

#[cfg(feature = "json")]
impl json::JsonDeserialize for ParliamentBody {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "rules-committee" => Ok(ParliamentBody::RulesCommittee),
            "agenda-council" => Ok(ParliamentBody::AgendaCouncil),
            "interest-panel" => Ok(ParliamentBody::InterestPanel),
            "review-panel" => Ok(ParliamentBody::ReviewPanel),
            "policy-jury" => Ok(ParliamentBody::PolicyJury),
            "oversight-committee" => Ok(ParliamentBody::OversightCommittee),
            "fma-committee" => Ok(ParliamentBody::FmaCommittee),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}

/// Parliament roster for a single body.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentRoster {
    /// Body this roster applies to.
    pub body: ParliamentBody,
    /// Epoch/term index for the roster.
    pub epoch: u64,
    /// Ordered members assigned to the body.
    pub members: Vec<AccountId>,
    /// Alternates that may replace missing members (ordered).
    #[norito(default)]
    pub alternates: Vec<AccountId>,
    /// Number of candidates whose VRF proofs verified successfully.
    #[norito(default)]
    pub verified: u32,
    /// Total candidates considered (verified + rejected).
    #[norito(default)]
    pub candidate_count: u32,
    /// Derivation method used to compute the roster.
    #[norito(default)]
    pub derived_by: CouncilDerivationKind,
}

/// Parliament configuration and rosters for all bodies selected in an epoch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentBodies {
    /// Epoch index used to derive the bodies.
    pub selection_epoch: u64,
    /// Rosters keyed by body.
    #[norito(default)]
    pub rosters: BTreeMap<ParliamentBody, ParliamentRoster>,
}

/// Parliament enactment certificate payload (signed via [`EnactmentSignatureScheme`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentEnactment {
    /// Blake2b-32 hash of the enactment preimage.
    pub preimage_hash: [u8; 32],
    /// Inclusive window in which enactment is valid.
    pub at_window: AtWindow,
}

/// Signature over a parliament enactment payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentEnactmentSignature {
    /// Parliament member account that produced the signature.
    pub signer: AccountId,
    /// Public key used to verify the signature.
    pub public_key: PublicKey,
    /// Signature of the enactment payload.
    pub signature: SignatureOf<ParliamentEnactment>,
}

/// Signature bundle for parliament enactment certificates.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentEnactmentSignatureSet {
    /// Signature scheme identifier.
    pub scheme: EnactmentSignatureScheme,
    /// Collected signatures ordered deterministically by signer account id.
    pub signatures: Vec<ParliamentEnactmentSignature>,
}

/// Signed parliament enactment certificate.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentEnactmentCertificate {
    /// Enactment payload being authorized.
    pub payload: ParliamentEnactment,
    /// Signature bundle proving authorization.
    pub signatures: ParliamentEnactmentSignatureSet,
}

/// Domain separator for proposal fingerprints (`Blake2b-256`).
pub const PROPOSAL_FINGERPRINT_DOMAIN: &[u8] = b"gov:proposal:v1";

impl ProposalKind {
    /// Compute the deterministic proposal fingerprint (`Blake2b-32`).
    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        let encoded = Encode::encode(self);
        let mut hasher = Blake2bVar::new(32).expect("Blake2bVar length");
        hasher.update(PROPOSAL_FINGERPRINT_DOMAIN);
        hasher.update(&encoded);
        let mut out = [0u8; 32];
        hasher
            .finalize_variable(&mut out)
            .expect("finalize Blake2bVar");
        out
    }
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;
    use iroha_crypto::KeyPair;
    use norito::core::{DecodeFromSlice, NoritoSerialize};

    use super::*;
    use crate::{AccountId, DomainId};

    #[test]
    fn contract_hash_roundtrips_hex() {
        let raw = [0xAAu8; 32];
        let hash = ContractCodeHash::new(raw);
        let encoded = hash.to_hex();
        let parsed = ContractCodeHash::from_hex_str(&encoded).expect("parse hex");
        assert_eq!(parsed, hash);
    }

    #[test]
    fn hash_parse_rejects_wrong_length() {
        let err =
            ContractAbiHash::from_hex_str("deadbeef").expect_err("length mismatch should error");
        match err {
            HashParseError::InvalidLength { expected, actual } => {
                assert_eq!(expected, 32);
                assert_eq!(actual, 4);
            }
            _ => panic!("unexpected error variant"),
        }
    }

    #[test]
    fn contract_hash_from_hex_roundtrip() {
        let raw = "aa".repeat(ContractCodeHash::LENGTH);
        let parsed = ContractCodeHash::from_hex_str(&raw).expect("parse contract hash");
        assert_eq!(parsed.to_hex(), raw);
    }

    #[test]
    fn hash_decode_rejects_non_canonical_vec_layout() {
        let mut non_canonical = Vec::new();
        non_canonical.extend_from_slice(&32u64.to_le_bytes());
        for idx in 0..=32u64 {
            non_canonical.extend_from_slice(&idx.to_le_bytes());
        }
        non_canonical.extend_from_slice(&[0x11u8; 32]);

        let mut encoded = Vec::new();
        NoritoSerialize::serialize(&non_canonical, &mut encoded).expect("encode vec");
        let result = <ContractCodeHash as DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(result.is_err());
    }

    #[test]
    fn parliament_body_default_is_agenda() {
        assert_eq!(ParliamentBody::default(), ParliamentBody::AgendaCouncil);
    }

    #[test]
    fn governance_types_encode() {
        let code_hash = ContractCodeHash::from_hex_str(&"aa".repeat(32)).expect("code hash");
        let abi_hash = ContractAbiHash::from_hex_str(&"bb".repeat(32)).expect("abi hash");
        let proposal = DeployContractProposal {
            namespace: "demo".to_owned(),
            contract_id: "mint".to_owned(),
            code_hash_hex: code_hash,
            abi_hash_hex: abi_hash,
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        };

        let payload = ProposalKind::DeployContract(proposal.clone());
        let framed = norito::to_bytes(&payload).expect("encode proposal kind");
        let decoded =
            norito::decode_from_bytes::<ProposalKind>(&framed).expect("decode proposal kind");
        match decoded {
            ProposalKind::DeployContract(inner) => {
                assert_eq!(inner.contract_id, proposal.contract_id);
                assert_eq!(
                    inner.code_hash_hex.to_hex(),
                    proposal.code_hash_hex.to_hex()
                );
            }
            ProposalKind::RuntimeUpgrade(_) => panic!("unexpected runtime-upgrade proposal"),
        }
    }

    #[test]
    fn runtime_upgrade_proposal_roundtrip() {
        let manifest = RuntimeUpgradeManifest {
            name: "gov runtime upgrade".to_owned(),
            description: "runtime proposal roundtrip".to_owned(),
            abi_version: 1,
            abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            added_syscalls: Vec::new(),
            added_pointer_types: Vec::new(),
            start_height: 42,
            end_height: 99,
            sbom_digests: Vec::new(),
            slsa_attestation: Vec::new(),
            provenance: Vec::new(),
        };
        let payload = ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal { manifest });
        let framed = norito::to_bytes(&payload).expect("encode runtime-upgrade proposal");
        let decoded = norito::decode_from_bytes::<ProposalKind>(&framed)
            .expect("decode runtime-upgrade proposal");
        match decoded {
            ProposalKind::RuntimeUpgrade(inner) => {
                assert_eq!(inner.manifest.abi_version, 1);
                assert_eq!(inner.manifest.start_height, 42);
            }
            ProposalKind::DeployContract(_) => panic!("unexpected deploy-contract proposal"),
        }
    }

    #[test]
    fn proposal_fingerprint_matches_manual_derivation() {
        let proposal = DeployContractProposal {
            namespace: "apps".to_owned(),
            contract_id: "demo.contract".to_owned(),
            code_hash_hex: ContractCodeHash::from_hex_str(&"11".repeat(32)).expect("code hash"),
            abi_hash_hex: ContractAbiHash::from_hex_str(&"22".repeat(32)).expect("abi hash"),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        };
        let kind = ProposalKind::DeployContract(proposal);

        let fp = kind.fingerprint();

        let manual_bytes = Encode::encode(&kind);
        let mut hasher = Blake2bVar::new(32).expect("Blake2bVar length");
        hasher.update(PROPOSAL_FINGERPRINT_DOMAIN);
        hasher.update(&manual_bytes);
        let mut manual_arr = [0u8; 32];
        hasher
            .finalize_variable(&mut manual_arr)
            .expect("finalize Blake2bVar");

        assert_eq!(fp, manual_arr);

        // Spot check deterministic value to guard accidental changes.
        assert_eq!(
            fp,
            hex!("43354427fc23f14104fd6da3d0d6e17fcae2c955206fb971e21a901404e7f911")
        );
    }

    #[test]
    fn vote_choice_roundtrip() {
        let vote = Vote {
            referendum_id: ProposalId([0x42; 32]),
            voter: AccountId::new(
                "wonderland".parse().expect("domain id"),
                "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                    .parse()
                    .expect("public key"),
            ),
            conviction: 3,
            choice: VoteChoice::Aye,
        };

        let framed = norito::to_bytes(&vote).expect("encode vote");
        let decoded = norito::decode_from_bytes::<Vote>(&framed).expect("decode vote");
        assert_eq!(decoded.choice, VoteChoice::Aye);
    }

    #[test]
    #[cfg(feature = "json")]
    fn proposal_id_json_roundtrip() {
        let id = ProposalId([0xAB; 32]);
        let json = norito::json::to_json(&id).expect("serialize proposal id");
        let decoded: ProposalId = norito::json::from_json(&json).expect("deserialize proposal id");
        assert_eq!(decoded, id);
    }

    #[test]
    fn parliament_bodies_roundtrip() {
        use std::collections::BTreeMap;

        let domain: DomainId = "wonderland".parse().expect("domain id");
        let members = vec![
            AccountId::new(domain.clone(), KeyPair::random().public_key().clone()),
            AccountId::new(domain.clone(), KeyPair::random().public_key().clone()),
        ];
        let alternates = vec![AccountId::new(
            domain,
            KeyPair::random().public_key().clone(),
        )];

        let roster = ParliamentRoster {
            body: ParliamentBody::RulesCommittee,
            epoch: 3,
            members: members.clone(),
            alternates: alternates.clone(),
            verified: 2,
            candidate_count: 3,
            derived_by: CouncilDerivationKind::Vrf,
        };
        let mut rosters = BTreeMap::new();
        rosters.insert(ParliamentBody::RulesCommittee, roster.clone());

        let bodies = ParliamentBodies {
            selection_epoch: 3,
            rosters,
        };

        let framed = norito::to_bytes(&bodies).expect("encode bodies");
        let decoded =
            norito::decode_from_bytes::<ParliamentBodies>(&framed).expect("decode bodies");
        assert_eq!(decoded.selection_epoch, bodies.selection_epoch);
        let back = decoded
            .rosters
            .get(&ParliamentBody::RulesCommittee)
            .expect("rules roster");
        assert_eq!(back.members, roster.members);
        assert_eq!(back.derived_by, roster.derived_by);
    }

    #[test]
    fn enactment_signature_scheme_ids_and_labels() {
        assert_eq!(
            EnactmentSignatureScheme::SimpleThreshold.scheme_id(),
            ENACTMENT_SIGNATURE_SCHEME_SIMPLE_THRESHOLD
        );
        assert_eq!(
            EnactmentSignatureScheme::SimpleThreshold.as_str(),
            "simple_threshold"
        );
        assert_eq!(
            EnactmentSignatureScheme::try_from(ENACTMENT_SIGNATURE_SCHEME_SIMPLE_THRESHOLD)
                .expect("valid scheme id"),
            EnactmentSignatureScheme::SimpleThreshold
        );
    }

    #[test]
    fn enactment_signature_scheme_parsing_rejects_unknown() {
        let err = "unknown".parse::<EnactmentSignatureScheme>().unwrap_err();
        assert_eq!(err.0, "unknown");
    }

    #[test]
    fn governance_enactment_certificate_roundtrip() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let signer = AccountId::new(domain, keypair.public_key().clone());
        let payload = GovernanceEnactment {
            referendum_id: ProposalId([0x10; 32]),
            preimage_hash: [0x22; 32],
            at_window: AtWindow {
                lower: 12,
                upper: 18,
            },
        };
        let signature = SignatureOf::new(keypair.private_key(), &payload);
        let cert = GovernanceEnactmentCertificate {
            payload,
            signatures: GovernanceEnactmentSignatureSet {
                scheme: EnactmentSignatureScheme::SimpleThreshold,
                signatures: vec![GovernanceEnactmentSignature {
                    signer,
                    public_key: keypair.public_key().clone(),
                    signature,
                }],
            },
        };

        let encoded = norito::to_bytes(&cert).expect("encode enactment certificate");
        let decoded = norito::decode_from_bytes::<GovernanceEnactmentCertificate>(&encoded)
            .expect("decode enactment certificate");
        assert_eq!(decoded, cert);
    }

    #[test]
    fn parliament_enactment_certificate_roundtrip() {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let keypair = KeyPair::random();
        let signer = AccountId::new(domain, keypair.public_key().clone());
        let payload = ParliamentEnactment {
            preimage_hash: [0x33; 32],
            at_window: AtWindow {
                lower: 20,
                upper: 28,
            },
        };
        let signature = SignatureOf::new(keypair.private_key(), &payload);
        let cert = ParliamentEnactmentCertificate {
            payload,
            signatures: ParliamentEnactmentSignatureSet {
                scheme: EnactmentSignatureScheme::SimpleThreshold,
                signatures: vec![ParliamentEnactmentSignature {
                    signer,
                    public_key: keypair.public_key().clone(),
                    signature,
                }],
            },
        };

        let encoded = norito::to_bytes(&cert).expect("encode parliament certificate");
        let decoded = norito::decode_from_bytes::<ParliamentEnactmentCertificate>(&encoded)
            .expect("decode parliament certificate");
        assert_eq!(decoded, cert);
    }
}
