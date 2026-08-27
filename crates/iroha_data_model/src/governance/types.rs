//! Canonical governance and SORA Parliament data-model types.
//!
//! V1 Parliament types distinguish immutable proposal content from retryable
//! governance, body-election, and ballot attempts. Consensus code must use the
//! typed identifiers and closed lifecycle enums defined here rather than
//! reconstructing stage or result semantics from strings.
//!
//! Notes:
//! - Use the `SignedBlock` v1 Norito serialization for any `call_selector(inner)` and certificate hashing contexts.
//! - Fixed-point thresholds are represented as integers; Q-format mapping is specified in docs.
#[cfg(test)]
use crate::isi::bridge::SccpRouteGovernanceActionV1;
use crate::{
    NetworkId,
    account::AccountId,
    asset::AssetId,
    isi::{
        governance::parliament_timed_ovn_required_chunk_blocks_v1,
        sorafs::SorafsProviderGovernanceActionV1,
    },
    musubi::MusubiParliamentActionV1,
    runtime::RuntimeUpgradeManifest,
    smart_contract::{ContractAddress, manifest::ManifestProvenance},
    validation_fee::{ValidationFeePolicyV1, ValidationFeeTreasuryPayoutBindingV1},
};
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize, Parser};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
    string::String,
    vec::Vec,
};
/// Voting mode for a referendum.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
pub enum VotingMode {
    /// Zero-knowledge voting flow (default ballot type).
    Zk,
    /// Plain-text quadratic voting flow.
    Plain,
}
#[cfg(feature = "json")]
impl norito::json::JsonSerialize for VotingMode {
    fn json_serialize(&self, out: &mut String) {
        norito::json::write_json_string(
            match self {
                Self::Zk => "Zk",
                Self::Plain => "Plain",
            },
            out,
        );
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(
            match self {
                Self::Zk => "Zk",
                Self::Plain => "Plain",
            },
            out,
        )
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for VotingMode {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Zk" => Ok(Self::Zk),
            "Plain" => Ok(Self::Plain),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}
/// Council derivation method.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
)]
pub enum CouncilDerivationKind {
    /// Derived automatically from deterministic bonded-citizen sortition.
    Sortition,
    /// Supplied explicitly by an authorized parliament administrator.
    #[default]
    Manual,
}
#[cfg(feature = "json")]
impl norito::json::JsonSerialize for CouncilDerivationKind {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            CouncilDerivationKind::Sortition => "Sortition",
            CouncilDerivationKind::Manual => "Manual",
        };
        norito::json::write_json_string(label, out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        let label = match self {
            CouncilDerivationKind::Sortition => "Sortition",
            CouncilDerivationKind::Manual => "Manual",
        };
        norito::json::write_json_string_to(label, out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for CouncilDerivationKind {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Sortition" => Ok(CouncilDerivationKind::Sortition),
            "Manual" => Ok(CouncilDerivationKind::Manual),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}
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
fn decode_lowercase_hex_array<const N: usize>(input: &str) -> Result<[u8; N], HashParseError> {
    if !input
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(HashParseError::InvalidHex {
            message: "hash must use lowercase hexadecimal without a prefix".to_owned(),
        });
    }
    decode_hex_array(input)
}
macro_rules! define_hash32_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema)]
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
                decode_lowercase_hex_array::<32>(input).map(Self)
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
            fn serialize(
                &self,
                writer: &mut norito::core::Encoder<'_>,
            ) -> Result<(), norito::core::Error> {
                let wire = HashWire32::new(self.0);
                <HashWire32 as norito::core::NoritoSerialize>::serialize(&wire, writer)
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
                let (wire, _used) = norito::core::decode_field_canonical::<HashWire32>(bytes)?;
                wire.try_into_bytes().map(Self)
            }
        }
        impl<'a> norito::core::DecodeFromSlice<'a> for $name {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let (wire, used) = norito::core::decode_field_canonical::<HashWire32>(bytes)?;
                wire.try_into_bytes().map(|array| (Self(array), used))
            }
        }
        #[cfg(feature = "json")]
        impl JsonSerialize for $name {
            fn json_serialize(&self, out: &mut String) {
                json::write_json_string(&self.to_hex(), out);
            }
            fn json_serialize_to(
                &self,
                out: &mut dyn json::JsonWriteSink,
            ) -> Result<(), json::BoundedJsonError> {
                json::write_json_string_to(&self.to_hex(), out)
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
define_hash32_newtype!(
    AgendaItemId,
    "Immutable content identifier for an admitted Parliament agenda item."
);
define_hash32_newtype!(
    DraftId,
    "Immutable content identifier for a deliberative Parliament draft."
);
define_hash32_newtype!(
    ProposalContentId,
    "Immutable identifier for proposal content shared by every retry attempt."
);
define_hash32_newtype!(
    GovernanceAttemptId,
    "Identifier for one retryable end-to-end governance attempt."
);
define_hash32_newtype!(
    BodyInstanceId,
    "Identifier for one sealed Parliament body instance."
);
define_hash32_newtype!(
    BodyElectionAttemptId,
    "Identifier for one retryable Parliament body-election attempt."
);
define_hash32_newtype!(
    AssignmentId,
    "Identifier for one deterministic Parliament service assignment."
);
define_hash32_newtype!(
    SortitionRequestId,
    "Identifier for one immutable future-pulse sortition request."
);
define_hash32_newtype!(
    BallotAttemptId,
    "Identifier for one retryable hidden Parliament ballot attempt."
);
define_hash32_newtype!(
    BeaconSessionId,
    "Stable network-scoped identifier for the logical Parliament beacon."
);
impl BeaconSessionId {
    /// Derive the canonical stable Parliament beacon identifier for a network.
    ///
    /// This logical identifier deliberately does not name a DKG key session;
    /// future Parliament slots remain valid across legitimate validator-roster
    /// and threshold-key rotations.
    #[must_use]
    pub fn for_network_v1(network_id: &NetworkId) -> Self {
        let mut bytes = crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::LOGICAL_BEACON_SESSION_ID_V1,
            network_id,
        );
        if bytes.iter().all(|byte| *byte == 0) {
            bytes[0] = 1;
        }
        Self::new(bytes)
    }
}
define_hash32_newtype!(
    BeaconPulseId,
    "Identifier for one finalized threshold-beacon pulse."
);
define_hash32_newtype!(
    TleSessionId,
    "Identifier for one ballot-specific threshold timelock-encryption session."
);
define_hash32_newtype!(
    TleKeySessionId,
    "Identifier for one finalized threshold-BLS key session dedicated to TLE releases."
);
define_hash32_newtype!(
    GovernanceCertificateId,
    "Content identifier for one finalized V1 governance certificate."
);

/// ABI version targeted by the contract manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
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
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.0.json_serialize_to(out)
    }
}
#[cfg(feature = "json")]
impl JsonDeserialize for AbiVersion {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        u16::json_deserialize(parser).map(Self::from)
    }
}
/// Largest integer admitted in first-release public JSON number fields.
///
/// Public proposal JSON is consumed by SDK runtimes whose number type is IEEE-754 binary64.
/// Bounding every number-encoded `u64` at `2^53 - 1` keeps those values exact without giving
/// structurally shared runtime, Musubi, or SCCP types a context-specific string encoding.
pub const FIRST_RELEASE_MAX_EXACT_JSON_U64: u64 = (1_u64 << 53) - 1;
/// Governance proposal kinds supported today.
#[expect(
    clippy::large_enum_variant,
    reason = "proposal variants retain their canonical public Norito payload shapes"
)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "payload", deny_unknown_fields),
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub enum ProposalKind {
    /// Deploy an IVM contract identified by its canonical public address and content hashes.
    #[codec(index = 0)]
    DeployContract(DeployContractProposal),
    /// Schedule a runtime upgrade manifest through governance.
    #[codec(index = 1)]
    RuntimeUpgrade(RuntimeUpgradeProposal),
    /// Apply one closed SCCP route-registry action through governance.
    #[codec(index = 2)]
    SccpRouteGovernance(SccpRouteGovernanceProposal),
    /// Enact one validation-fee policy through SORA Parliament.
    #[codec(index = 3)]
    ValidationFeePolicy(ValidationFeePolicyProposal),
    /// Authorize one exact validation-fee treasury payout lifecycle.
    #[codec(index = 4)]
    ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal),
    /// Enact one exact Musubi recovery, alias-retarget, takedown, or policy action.
    #[codec(index = 5)]
    MusubiRegistryGovernance(MusubiParliamentActionV1),
    /// Establish, replace, or remove one `SoraFS` provider owner through governance.
    #[codec(index = 6)]
    SorafsProviderGovernance(SorafsProviderGovernanceProposal),
}
/// Proposal payload for deploying an IVM contract via governance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DeployContractProposal {
    /// Canonical public contract address governed by the proposal.
    pub contract_address: ContractAddress,
    /// Blake2b-32 hash of the compiled `.to` bytecode.
    pub code_hash: ContractCodeHash,
    /// Blake2b-32 hash of the ABI surface expected by hosts.
    pub abi_hash: ContractAbiHash,
    /// ABI version (currently `1`).
    pub abi_version: AbiVersion,
    /// Optional manifest provenance used to attest the manifest when absent on-chain.
    pub manifest_provenance: Option<ManifestProvenance>,
}
/// Proposal payload for scheduling a runtime upgrade through governance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RuntimeUpgradeProposal {
    /// Canonical runtime-upgrade manifest payload.
    pub manifest: RuntimeUpgradeManifest,
}
/// Proposal payload for applying one closed SCCP registry action.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SccpRouteGovernanceProposal {
    /// Complete network- and action-bound SCCP Parliament effect preimage.
    pub anchor: Box<crate::isi::bridge::SccpRouteGovernanceAnchorV1>,
}
/// Proposal payload for one closed `SoraFS` provider-owner transition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SorafsProviderGovernanceProposal {
    /// Exact compare-and-set provider-owner action to execute on enactment.
    pub action: Box<SorafsProviderGovernanceActionV1>,
}
/// Proposal payload for one governed validation-fee policy.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePolicyProposal {
    /// Canonical transaction authority that created this proposal.
    ///
    /// Binding the operator into the typed preimage prevents transaction
    /// ordering from assigning an otherwise identical payload to a different
    /// retained proposer.
    pub proposal_operator: AccountId,
    /// Complete policy to append to the protected validation-fee registry.
    pub policy: ValidationFeePolicyV1,
    /// Exact previously enacted payout lifecycle required by a policy carrying a payout binding.
    pub payout_lifecycle_proposal_id: Option<[u8; 32]>,
}
/// Proposal payload authorizing one exact validation-fee payout lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePayoutLifecycleProposal {
    /// Canonical transaction authority that created this proposal.
    ///
    /// This identity is part of the exact proposal fingerprint and must agree
    /// with the retained governance record's proposer.
    pub proposal_operator: AccountId,
    /// Exact payout binding authorized by this lifecycle.
    ///
    /// Its deterministic non-zero lifecycle seal is derived from this complete
    /// payload, so the native proposal fingerprint binds the seal transitively
    /// without accepting a redundant caller-supplied value.
    pub payout_binding: ValidationFeeTreasuryPayoutBindingV1,
}
/// Inclusive execution window for enactment certificates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AtWindow {
    /// First block in the enactment window (inclusive).
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::u64_string"))]
    pub lower: u64,
    /// Last block in the enactment window (inclusive).
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::u64_string"))]
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
    pub deposit_base: Quantity,
    /// Additional deposit required per byte of preimage.
    pub deposit_per_byte: Quantity,
    /// Additional deposit required per block of desired enactment window.
    pub deposit_per_block: Quantity,
}
/// Content-addressable proposal identifier (32-byte hash).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, IntoSchema)]
pub struct ProposalId(pub [u8; 32]);
impl norito::core::NoritoSerialize for ProposalId {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let wire = HashWire32::new(self.0);
        <HashWire32 as norito::core::NoritoSerialize>::serialize(&wire, writer)
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
        let (wire, _used) = norito::core::decode_field_canonical::<HashWire32>(bytes)?;
        wire.try_into_bytes().map(Self)
    }
}
#[cfg(feature = "json")]
impl JsonSerialize for ProposalId {
    fn json_serialize(&self, out: &mut String) {
        json::write_json_string(&hex::encode(self.0), out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_json_string_to(&hex::encode(self.0), out)
    }
}
#[cfg(feature = "json")]
impl JsonDeserialize for ProposalId {
    fn json_deserialize(parser: &mut Parser<'_>) -> Result<Self, json::Error> {
        let buf = parser.parse_string()?;
        decode_lowercase_hex_array::<32>(&buf)
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
    pub deposit: Quantity,
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
/// Parliament governance body identifiers.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
pub enum ParliamentBody {
    /// Rules Committee — intake and rulebook gate.
    #[codec(index = 0)]
    RulesCommittee,
    /// Agenda Council — schedules referendum windows and admission order.
    #[default]
    #[codec(index = 1)]
    AgendaCouncil,
    /// Interest Panels — subject-matter reviewers for the proposal domain.
    #[codec(index = 2)]
    InterestPanel,
    /// Review Panel — cross-domain review ahead of policy jury.
    #[codec(index = 3)]
    ReviewPanel,
    /// Coordination Council — reconciles the review record before specialist review.
    #[codec(index = 4)]
    CoordinationCouncil,
    /// Monetary Policy Committee — specialist review for monetary-policy effects.
    #[codec(index = 5)]
    MpcCommittee,
    /// Financial Markets Authority committee — specialist review for market effects.
    #[codec(index = 6)]
    FmaCommittee,
    /// Oversight Committee — monitors deadlines and escalation paths.
    #[codec(index = 7)]
    OversightCommittee,
    /// Policy Jury — large-jury decision body for approval/rejection.
    #[codec(index = 8)]
    PolicyJury,
    /// Confirmation Jury — a fresh disjoint jury for narrowly approved proposals.
    #[codec(index = 9)]
    ConfirmationJury,
}
/// Canonical V1 Parliament body order used by sortition, pipelines, and APIs.
pub const PARLIAMENT_BODIES_V1: [ParliamentBody; 10] = [
    ParliamentBody::RulesCommittee,
    ParliamentBody::AgendaCouncil,
    ParliamentBody::InterestPanel,
    ParliamentBody::ReviewPanel,
    ParliamentBody::CoordinationCouncil,
    ParliamentBody::MpcCommittee,
    ParliamentBody::FmaCommittee,
    ParliamentBody::OversightCommittee,
    ParliamentBody::PolicyJury,
    ParliamentBody::ConfirmationJury,
];
#[cfg(feature = "json")]
impl json::JsonSerialize for ParliamentBody {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            ParliamentBody::RulesCommittee => "rules-committee",
            ParliamentBody::AgendaCouncil => "agenda-council",
            ParliamentBody::InterestPanel => "interest-panel",
            ParliamentBody::ReviewPanel => "review-panel",
            ParliamentBody::CoordinationCouncil => "coordination-council",
            ParliamentBody::MpcCommittee => "mpc-committee",
            ParliamentBody::FmaCommittee => "fma-committee",
            ParliamentBody::OversightCommittee => "oversight-committee",
            ParliamentBody::PolicyJury => "policy-jury",
            ParliamentBody::ConfirmationJury => "confirmation-jury",
        };
        json::write_json_string(label, out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        let label = match self {
            ParliamentBody::RulesCommittee => "rules-committee",
            ParliamentBody::AgendaCouncil => "agenda-council",
            ParliamentBody::InterestPanel => "interest-panel",
            ParliamentBody::ReviewPanel => "review-panel",
            ParliamentBody::CoordinationCouncil => "coordination-council",
            ParliamentBody::MpcCommittee => "mpc-committee",
            ParliamentBody::FmaCommittee => "fma-committee",
            ParliamentBody::OversightCommittee => "oversight-committee",
            ParliamentBody::PolicyJury => "policy-jury",
            ParliamentBody::ConfirmationJury => "confirmation-jury",
        };
        json::write_json_string_to(label, out)
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
            "coordination-council" => Ok(ParliamentBody::CoordinationCouncil),
            "mpc-committee" => Ok(ParliamentBody::MpcCommittee),
            "fma-committee" => Ok(ParliamentBody::FmaCommittee),
            "oversight-committee" => Ok(ParliamentBody::OversightCommittee),
            "policy-jury" => Ok(ParliamentBody::PolicyJury),
            "confirmation-jury" => Ok(ParliamentBody::ConfirmationJury),
            other => Err(json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}
/// Closed V1 governance risk classification.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "tier", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum RiskTierV1 {
    /// Deterministic execution already authorized by an active mandate.
    #[codec(index = 0)]
    Routine,
    /// Bounded, reversible policy change using the affected-domain bodies.
    #[default]
    #[codec(index = 1)]
    Standard,
    /// Constitutional, consensus, rights, monetary, or highly irreversible change.
    #[codec(index = 2)]
    Constitutional,
    /// Time-bounded containment action followed by constitutional retrospective review.
    #[codec(index = 3)]
    Emergency,
}
impl RiskTierV1 {
    const fn rank(self) -> u8 {
        match self {
            Self::Routine => 0,
            Self::Standard => 1,
            Self::Constitutional => 2,
            Self::Emergency => 3,
        }
    }

    /// Return whether `target` is an upward-only escalation from this tier.
    #[must_use]
    pub const fn can_escalate_to(self, target: Self) -> bool {
        self.rank() <= target.rank()
    }
}

/// Sequential stage occupied by a V1 governance attempt.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "stage", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum GovernanceStageV1 {
    /// Qualification and proposal-content admission.
    #[default]
    #[codec(index = 0)]
    Qualification,
    /// Rules Committee review.
    #[codec(index = 1)]
    Rules,
    /// Agenda Council scheduling.
    #[codec(index = 2)]
    Agenda,
    /// Public, nonbinding Interest Panel deliberation.
    #[codec(index = 3)]
    Interest,
    /// Affected-domain Review Panel deliberation.
    #[codec(index = 4)]
    Review,
    /// Coordination Council reconciliation.
    #[codec(index = 5)]
    Coordination,
    /// Applicable Monetary Policy Committee review.
    #[codec(index = 6)]
    Mpc,
    /// Applicable Financial Markets Authority review.
    #[codec(index = 7)]
    Fma,
    /// Oversight Committee review.
    #[codec(index = 8)]
    Oversight,
    /// Policy Jury decision.
    #[codec(index = 9)]
    PolicyJury,
    /// Disjoint Confirmation Jury decision when the first margin is narrow.
    #[codec(index = 10)]
    ConfirmationJury,
    /// Governance certificate construction.
    #[codec(index = 11)]
    Certification,
    /// Deterministic on-chain enactment.
    #[codec(index = 12)]
    Enactment,
}

/// Terminal or active state of a retryable governance attempt.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum GovernanceAttemptStatusV1 {
    /// The attempt is processing its current stage.
    #[default]
    #[codec(index = 0)]
    Active,
    /// The attempt produced a complete governance certificate.
    #[codec(index = 1)]
    Certified,
    /// A binding Parliament body rejected the proposal.
    #[codec(index = 2)]
    Rejected,
    /// The certified effect was enacted.
    #[codec(index = 3)]
    Enacted,
    /// A competing compare-and-set certificate won first.
    #[codec(index = 4)]
    Superseded,
    /// The exact certified effect failed deterministic execution.
    #[codec(index = 5)]
    ExecutionFailed,
}

/// Canonical snapshot of one retryable end-to-end governance attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct GovernanceAttemptV1 {
    /// Identifier unique to this retry attempt.
    pub id: GovernanceAttemptId,
    /// Immutable proposal content shared by all retries.
    pub proposal_content_id: ProposalContentId,
    /// Zero-based retry sequence for the proposal content.
    pub sequence: u32,
    /// Upward-only risk tier fixed before the Policy Jury draw.
    pub risk_tier: RiskTierV1,
    /// Current sequential stage.
    pub stage: GovernanceStageV1,
    /// Current attempt status.
    pub status: GovernanceAttemptStatusV1,
}

#[derive(Encode)]
struct GovernanceAttemptIdPreimageV1 {
    proposal_content_id: ProposalContentId,
    sequence: u32,
}

impl GovernanceAttemptId {
    /// Derive the only valid V1 identifier for a proposal retry sequence.
    #[must_use]
    pub fn derive_v1(proposal_content_id: ProposalContentId, sequence: u32) -> Self {
        Self::new(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::GOVERNANCE_ATTEMPT_ID_V1,
            &GovernanceAttemptIdPreimageV1 {
                proposal_content_id,
                sequence,
            },
        ))
    }
}

impl GovernanceAttemptV1 {
    /// Return whether the stored identifier is canonical for this retry.
    #[must_use]
    pub fn has_canonical_id(&self) -> bool {
        self.id == GovernanceAttemptId::derive_v1(self.proposal_content_id, self.sequence)
    }
}

/// Maximum target seats for any single V1 Parliament body.
///
/// The bound accommodates the largest permitted Confirmation Jury while
/// keeping ballot and certificate resource limits finite.
pub const MAX_PARLIAMENT_BODY_TARGET_SEATS_V1: u32 = 1_000;
/// Hard protocol ceiling for end-to-end governance retries after sequence zero.
pub const MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1: u32 = 16;
/// Hard protocol ceiling for future-pulse body-election retries after sequence zero.
pub const MAX_PARLIAMENT_SORTITION_RETRIES_V1: u32 = 16;
/// Hard protocol ceiling for private-ballot retries after the initial attempt.
pub const MAX_PARLIAMENT_BALLOT_RETRIES_V1: u32 = 16;
/// Hard protocol ceiling for registration, survivor, and ballot corpora.
pub const MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1: u32 = 1_000;
/// Hard protocol ceiling for one canonical framed Parliament attempt state.
pub const MAX_PARLIAMENT_ATTEMPT_STATE_BYTES_V1: usize = 16 * 1024 * 1024;

#[derive(Encode)]
struct ParliamentCandidateRootPreimageV1 {
    governance_attempt_id: GovernanceAttemptId,
    body: ParliamentBody,
    candidates: Vec<AccountId>,
}

/// Commit an exact canonically ordered candidate snapshot for one body draw.
#[must_use]
pub fn parliament_candidate_root_v1(
    governance_attempt_id: GovernanceAttemptId,
    body: ParliamentBody,
    candidates: &[AccountId],
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::CANDIDATE_ROOT_V1,
        &ParliamentCandidateRootPreimageV1 {
            governance_attempt_id,
            body,
            candidates: candidates.to_vec(),
        },
    )
}

/// Validation error for an immutable future-pulse sortition request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortitionRequestErrorV1 {
    /// One of the request, attempt, snapshot, or beacon-session bindings was zero.
    ZeroBinding,
    /// The request identifier did not hash the complete immutable request preimage.
    NonCanonicalIdentifier,
    /// The frozen candidate snapshot contained no eligible candidates.
    EmptyCandidateSnapshot,
    /// The requested body target had zero seats.
    ZeroTargetSeats,
    /// The requested body target exceeded the V1 protocol maximum.
    TargetSeatsExceedMaximum {
        /// Requested seat count.
        target_seats: u32,
        /// V1 protocol maximum.
        maximum: u32,
    },
    /// Height zero cannot identify a threshold-beacon pulse.
    ZeroPulseHeight,
    /// The pulse was not strictly later than request commitment.
    PulseNotStrictlyFuture {
        /// Height committing the immutable request.
        request_height: u64,
        /// Requested beacon-pulse height.
        pulse_height: u64,
    },
    /// The pulse height was already consumed in the same beacon session.
    PulseAlreadyConsumed {
        /// Requested beacon-pulse height.
        pulse_height: u64,
        /// Highest consumed pulse height supplied for the same session.
        last_consumed_pulse_height: u64,
    },
}
impl fmt::Display for SortitionRequestErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBinding => {
                f.write_str("sortition request bindings must be non-zero canonical digests")
            }
            Self::NonCanonicalIdentifier => {
                f.write_str("sortition request identifier is not canonical")
            }
            Self::EmptyCandidateSnapshot => f.write_str("sortition candidate snapshot is empty"),
            Self::ZeroTargetSeats => f.write_str("sortition target seats must be non-zero"),
            Self::TargetSeatsExceedMaximum {
                target_seats,
                maximum,
            } => write!(
                f,
                "sortition target seats {target_seats} exceed V1 maximum {maximum}"
            ),
            Self::ZeroPulseHeight => f.write_str("sortition pulse height must be non-zero"),
            Self::PulseNotStrictlyFuture {
                request_height,
                pulse_height,
            } => write!(
                f,
                "sortition pulse height {pulse_height} must be strictly after request height {request_height}"
            ),
            Self::PulseAlreadyConsumed {
                pulse_height,
                last_consumed_pulse_height,
            } => write!(
                f,
                "sortition pulse height {pulse_height} is not newer than consumed pulse height {last_consumed_pulse_height}"
            ),
        }
    }
}
impl std::error::Error for SortitionRequestErrorV1 {}

/// Immutable candidate-snapshot request committed before a future beacon pulse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct SortitionRequestV1 {
    /// Unique immutable request identifier.
    pub id: SortitionRequestId,
    /// End-to-end governance attempt requesting the body.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Body-election attempt that must consume this request.
    pub body_election_attempt_id: BodyElectionAttemptId,
    /// Parliament body to draw.
    pub body: ParliamentBody,
    /// Root of the frozen, canonically ordered candidate snapshot.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub candidate_root: [u8; 32],
    /// Candidates in the frozen snapshot.
    pub candidate_count: u32,
    /// Requested seats before feasible-pool concentration handling.
    pub target_seats: u32,
    /// Height at which this immutable request was committed.
    pub request_height: u64,
    /// Strictly future finalized pulse height consumed by the draw.
    pub pulse_height: u64,
    /// Stable network-scoped logical beacon expected to produce the pulse.
    pub beacon_session_id: BeaconSessionId,
}

#[derive(Encode)]
struct SortitionRequestIdPreimageV1 {
    governance_attempt_id: GovernanceAttemptId,
    body_election_attempt_id: BodyElectionAttemptId,
    body: ParliamentBody,
    candidate_root: [u8; 32],
    candidate_count: u32,
    target_seats: u32,
    request_height: u64,
    pulse_height: u64,
    beacon_session_id: BeaconSessionId,
}

impl SortitionRequestV1 {
    /// Derive the identifier that commits every immutable request field.
    #[must_use]
    pub fn canonical_id(&self) -> SortitionRequestId {
        SortitionRequestId::new(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::SORTITION_REQUEST_ID_V1,
            &SortitionRequestIdPreimageV1 {
                governance_attempt_id: self.governance_attempt_id,
                body_election_attempt_id: self.body_election_attempt_id,
                body: self.body,
                candidate_root: self.candidate_root,
                candidate_count: self.candidate_count,
                target_seats: self.target_seats,
                request_height: self.request_height,
                pulse_height: self.pulse_height,
                beacon_session_id: self.beacon_session_id,
            },
        ))
    }

    /// Construct a request and derive its identifier from the complete V1 preimage.
    ///
    /// # Errors
    /// Returns [`SortitionRequestErrorV1`] when any immutable request invariant fails.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor makes every consensus-bound request field explicit"
    )]
    pub fn try_new_canonical(
        governance_attempt_id: GovernanceAttemptId,
        body_election_attempt_id: BodyElectionAttemptId,
        body: ParliamentBody,
        candidate_root: [u8; 32],
        candidate_count: u32,
        target_seats: u32,
        request_height: u64,
        pulse_height: u64,
        beacon_session_id: BeaconSessionId,
        last_consumed_pulse_height: Option<u64>,
    ) -> Result<Self, SortitionRequestErrorV1> {
        let mut request = Self {
            id: SortitionRequestId::new([0; 32]),
            governance_attempt_id,
            body_election_attempt_id,
            body,
            candidate_root,
            candidate_count,
            target_seats,
            request_height,
            pulse_height,
            beacon_session_id,
        };
        request.id = request.canonical_id();
        request.validate(last_consumed_pulse_height)?;
        Ok(request)
    }

    /// Construct and validate an immutable sortition request.
    ///
    /// `last_consumed_pulse_height` must describe only the supplied beacon
    /// session. An undersubscribed but nonempty candidate snapshot is valid and
    /// is reported later through [`ParliamentConcentrationWarningV1`].
    ///
    /// # Errors
    /// Returns [`SortitionRequestErrorV1`] for an empty pool, an invalid target,
    /// or a pulse that is zero, non-future, or already consumed.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor makes every consensus-bound request field explicit"
    )]
    pub fn try_new(
        id: SortitionRequestId,
        governance_attempt_id: GovernanceAttemptId,
        body_election_attempt_id: BodyElectionAttemptId,
        body: ParliamentBody,
        candidate_root: [u8; 32],
        candidate_count: u32,
        target_seats: u32,
        request_height: u64,
        pulse_height: u64,
        beacon_session_id: BeaconSessionId,
        last_consumed_pulse_height: Option<u64>,
    ) -> Result<Self, SortitionRequestErrorV1> {
        let request = Self {
            id,
            governance_attempt_id,
            body_election_attempt_id,
            body,
            candidate_root,
            candidate_count,
            target_seats,
            request_height,
            pulse_height,
            beacon_session_id,
        };
        request.validate(last_consumed_pulse_height)?;
        Ok(request)
    }

    /// Revalidate the request against the highest consumed pulse in its session.
    ///
    /// # Errors
    /// Returns [`SortitionRequestErrorV1`] when any constructor invariant fails.
    pub fn validate(
        &self,
        last_consumed_pulse_height: Option<u64>,
    ) -> Result<(), SortitionRequestErrorV1> {
        if self.id.as_bytes() == &[0; 32]
            || self.governance_attempt_id.as_bytes() == &[0; 32]
            || self.body_election_attempt_id.as_bytes() == &[0; 32]
            || self.candidate_root == [0; 32]
            || self.beacon_session_id.as_bytes() == &[0; 32]
        {
            return Err(SortitionRequestErrorV1::ZeroBinding);
        }
        if self.id != self.canonical_id() {
            return Err(SortitionRequestErrorV1::NonCanonicalIdentifier);
        }
        if self.candidate_count == 0 {
            return Err(SortitionRequestErrorV1::EmptyCandidateSnapshot);
        }
        if self.target_seats == 0 {
            return Err(SortitionRequestErrorV1::ZeroTargetSeats);
        }
        if self.target_seats > MAX_PARLIAMENT_BODY_TARGET_SEATS_V1 {
            return Err(SortitionRequestErrorV1::TargetSeatsExceedMaximum {
                target_seats: self.target_seats,
                maximum: MAX_PARLIAMENT_BODY_TARGET_SEATS_V1,
            });
        }
        if self.pulse_height == 0 {
            return Err(SortitionRequestErrorV1::ZeroPulseHeight);
        }
        if self.pulse_height <= self.request_height {
            return Err(SortitionRequestErrorV1::PulseNotStrictlyFuture {
                request_height: self.request_height,
                pulse_height: self.pulse_height,
            });
        }
        if let Some(last_consumed_pulse_height) = last_consumed_pulse_height
            && self.pulse_height <= last_consumed_pulse_height
        {
            return Err(SortitionRequestErrorV1::PulseAlreadyConsumed {
                pulse_height: self.pulse_height,
                last_consumed_pulse_height,
            });
        }
        Ok(())
    }
}

/// Lifecycle state of one retryable Parliament body-election attempt.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum BodyElectionAttemptStatusV1 {
    /// The immutable request awaits its committed future pulse.
    #[default]
    #[codec(index = 0)]
    AwaitingPulse,
    /// Deterministic scores and cross-body assignments are being derived.
    #[codec(index = 1)]
    Drawing,
    /// Selected candidates and alternates may accept invitations.
    #[codec(index = 2)]
    AcceptingInvitations,
    /// A nonempty feasible roster was sealed into a body instance.
    #[codec(index = 3)]
    Sealed,
    /// No nonempty eligible roster could be sealed from the request.
    #[codec(index = 4)]
    NoRoster,
    /// A fresh election attempt replaced this one.
    #[codec(index = 5)]
    Superseded,
}

/// Binding error for a body-election attempt and its immutable request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyElectionAttemptErrorV1 {
    /// The request names a different governance attempt.
    GovernanceAttemptMismatch,
    /// The request names a different body-election attempt.
    ElectionAttemptMismatch,
    /// The attempt identifier did not bind its attempt, body, and retry sequence.
    NonCanonicalIdentifier,
}
impl fmt::Display for BodyElectionAttemptErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GovernanceAttemptMismatch => {
                f.write_str("sortition request governance attempt does not match election attempt")
            }
            Self::ElectionAttemptMismatch => {
                f.write_str("sortition request body-election id does not match election attempt")
            }
            Self::NonCanonicalIdentifier => {
                f.write_str("body-election attempt identifier is not canonical")
            }
        }
    }
}
impl std::error::Error for BodyElectionAttemptErrorV1 {}

/// Canonical snapshot of one retryable Parliament body-election attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct BodyElectionAttemptV1 {
    /// Unique body-election attempt identifier.
    pub id: BodyElectionAttemptId,
    /// End-to-end governance attempt served by the election.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Zero-based retry sequence within the body stage.
    pub sequence: u32,
    /// Immutable future-pulse request consumed by this election attempt.
    pub request: SortitionRequestV1,
    /// Current body-election lifecycle status.
    pub status: BodyElectionAttemptStatusV1,
}

#[derive(Encode)]
struct BodyElectionAttemptIdPreimageV1 {
    governance_attempt_id: GovernanceAttemptId,
    body: ParliamentBody,
    sequence: u32,
}

impl BodyElectionAttemptId {
    /// Derive the only valid V1 identifier for one body-election retry.
    #[must_use]
    pub fn derive_v1(
        governance_attempt_id: GovernanceAttemptId,
        body: ParliamentBody,
        sequence: u32,
    ) -> Self {
        Self::new(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::BODY_ELECTION_ATTEMPT_ID_V1,
            &BodyElectionAttemptIdPreimageV1 {
                governance_attempt_id,
                body,
                sequence,
            },
        ))
    }
}

impl BodyElectionAttemptV1 {
    /// Construct an election attempt whose IDs agree with its sortition request.
    ///
    /// # Errors
    /// Returns [`BodyElectionAttemptErrorV1`] when either request binding differs.
    pub fn try_new(
        id: BodyElectionAttemptId,
        governance_attempt_id: GovernanceAttemptId,
        sequence: u32,
        request: SortitionRequestV1,
        status: BodyElectionAttemptStatusV1,
    ) -> Result<Self, BodyElectionAttemptErrorV1> {
        if request.governance_attempt_id != governance_attempt_id {
            return Err(BodyElectionAttemptErrorV1::GovernanceAttemptMismatch);
        }
        if request.body_election_attempt_id != id {
            return Err(BodyElectionAttemptErrorV1::ElectionAttemptMismatch);
        }
        if id != BodyElectionAttemptId::derive_v1(governance_attempt_id, request.body, sequence) {
            return Err(BodyElectionAttemptErrorV1::NonCanonicalIdentifier);
        }
        Ok(Self {
            id,
            governance_attempt_id,
            sequence,
            request,
            status,
        })
    }
}

/// Deliberative phase occupied by a Parliament body instance.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "phase", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum DeliberationPhaseV1 {
    /// Member orientation and protocol briefing.
    #[default]
    #[codec(index = 0)]
    Orientation,
    /// Balanced evidence submission and review.
    #[codec(index = 1)]
    Evidence,
    /// Questions from selected members.
    #[codec(index = 2)]
    Questions,
    /// Responses entered into the public record.
    #[codec(index = 3)]
    Responses,
    /// Member deliberation.
    #[codec(index = 4)]
    Deliberation,
    /// Mandatory reflection interval.
    #[codec(index = 5)]
    Reflection,
    /// Hidden formal ballot, when the body is binding.
    #[codec(index = 6)]
    Vote,
}

/// Lifecycle state of one sealed Parliament body instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "phase", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum BodyInstanceStatusV1 {
    /// Candidate snapshot is frozen and awaiting a future beacon pulse.
    #[codec(index = 0)]
    AwaitingSortition,
    /// Selected members and alternates may accept their invitations.
    #[codec(index = 1)]
    AcceptingInvitations,
    /// A nonempty feasible roster has been sealed.
    #[codec(index = 2)]
    RosterSealed,
    /// The body is in a specified deliberative phase.
    #[codec(index = 3)]
    Deliberating(DeliberationPhaseV1),
    /// The body has an active hidden-ballot attempt.
    #[codec(index = 4)]
    Balloting,
    /// The body approved the proposal or issued its nonbinding finding.
    #[codec(index = 5)]
    Approved,
    /// The body rejected the proposal.
    #[codec(index = 6)]
    Rejected,
    /// Turnout did not meet the immutable original-seat quorum.
    #[codec(index = 7)]
    NoQuorum,
    /// Cryptographic opening did not produce a valid aggregate result.
    #[codec(index = 8)]
    NoResult,
    /// A fresh body instance replaced this one.
    #[codec(index = 9)]
    Superseded,
}

/// Canonical snapshot of one sealed Parliament body instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentBodyInstanceV1 {
    /// Unique body-instance identifier.
    pub id: BodyInstanceId,
    /// End-to-end governance attempt served by the body.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Sortition attempt that produced this instance.
    pub election_attempt_id: BodyElectionAttemptId,
    /// Body role served by this instance.
    pub body: ParliamentBody,
    /// Book-faithful requested seat count.
    pub target_seats: u32,
    /// Immutable actual-seat count used as the quorum denominator.
    pub original_seats: u32,
    /// Current body lifecycle state.
    pub status: BodyInstanceStatusV1,
}

#[derive(Encode)]
struct BodyInstanceIdPreimageV1 {
    election_attempt_id: BodyElectionAttemptId,
    roster_root: [u8; 32],
}

impl BodyInstanceId {
    /// Derive the V1 body identifier from its election and exact sealed roster.
    #[must_use]
    pub fn derive_v1(election_attempt_id: BodyElectionAttemptId, roster_root: [u8; 32]) -> Self {
        Self::new(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::BODY_INSTANCE_ID_V1,
            &BodyInstanceIdPreimageV1 {
                election_attempt_id,
                roster_root,
            },
        ))
    }
}

#[derive(Encode)]
struct AssignmentIdPreimageV1 {
    election_attempt_id: BodyElectionAttemptId,
    member: AccountId,
}

impl AssignmentId {
    /// Derive the V1 assignment identifier for a member in one election attempt.
    #[must_use]
    pub fn derive_v1(election_attempt_id: BodyElectionAttemptId, member: &AccountId) -> Self {
        Self::new(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::ASSIGNMENT_ID_V1,
            &AssignmentIdPreimageV1 {
                election_attempt_id,
                member: member.clone(),
            },
        ))
    }
}

/// Canonical assignment of one citizen to one sealed Parliament seat.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentSeatAssignmentV1 {
    /// Identifier derived from the election attempt and member identity.
    pub assignment_id: AssignmentId,
    /// Citizen selected for the seat.
    pub member: AccountId,
}

#[derive(Encode)]
struct ParliamentAssignmentPlanRootPreimageV1 {
    election_attempt_id: BodyElectionAttemptId,
    primary: Vec<ParliamentSeatAssignmentV1>,
    alternates: Vec<ParliamentSeatAssignmentV1>,
    cross_body_assignment_cap: u32,
}

/// Commit the exact ranked primary and alternate draw for one body election.
///
/// Unlike the sealed roster root, vector order is significant here: it fixes
/// the deterministic invitation and replacement order produced by the future
/// beacon pulse. Callers must supply canonical assignment identifiers and a
/// duplicate-free draw plan.
#[must_use]
pub fn parliament_assignment_plan_root_v1(
    election_attempt_id: BodyElectionAttemptId,
    primary: &[ParliamentSeatAssignmentV1],
    alternates: &[ParliamentSeatAssignmentV1],
    cross_body_assignment_cap: u32,
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::ASSIGNMENT_PLAN_ROOT_V1,
        &ParliamentAssignmentPlanRootPreimageV1 {
            election_attempt_id,
            primary: primary.to_vec(),
            alternates: alternates.to_vec(),
            cross_body_assignment_cap,
        },
    )
}

#[derive(Encode)]
struct ParliamentRosterRootPreimageV1 {
    election_attempt_id: BodyElectionAttemptId,
    assignments: Vec<ParliamentSeatAssignmentV1>,
}

/// Commit a canonically ordered, nonempty seated roster for one election.
#[must_use]
pub fn parliament_roster_root_v1(
    election_attempt_id: BodyElectionAttemptId,
    assignments: &[ParliamentSeatAssignmentV1],
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::ROSTER_ROOT_V1,
        &ParliamentRosterRootPreimageV1 {
            election_attempt_id,
            assignments: assignments.to_vec(),
        },
    )
}

/// Lifecycle state of one hidden Parliament ballot attempt.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum BallotAttemptStatusV1 {
    /// OVN registration keys and proofs are being accepted.
    #[default]
    #[codec(index = 0)]
    Registration,
    /// The canonical nonempty survivor roster is being frozen before ballots.
    #[codec(index = 1)]
    SurvivorFreeze,
    /// Folded timed-OVN ciphertexts and one-hot proofs are being accepted.
    #[codec(index = 2)]
    TimedCommitment,
    /// The complete seal awaits its target finalized height.
    #[codec(index = 3)]
    AwaitingRelease,
    /// Finalized timelock material is opening the canonical corpus.
    #[codec(index = 4)]
    Opening,
    /// A valid aggregate tally was finalized.
    #[codec(index = 5)]
    Finalized,
    /// The attempt terminally failed to produce a valid result.
    #[codec(index = 6)]
    NoResult,
    /// A fresh ballot attempt replaced this one.
    #[codec(index = 7)]
    Superseded,
}

/// Deterministic reason a private ballot attempt ended without a result.
///
/// Invalid caller-supplied proofs are rejected and never become lifecycle
/// state. These reasons are reserved for phase expiry derived from persisted
/// state or an unavailable finalized release pulse after its immutable height.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    norito(tag = "reason", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum ParliamentBallotFailureKindV1 {
    /// The proof-validated registration corpus was not frozen by its deadline.
    #[codec(index = 0)]
    RegistrationDeadlineExpired,
    /// A nonempty canonical survivor roster was not frozen by its deadline.
    #[codec(index = 1)]
    SurvivorDeadlineExpired,
    /// The complete survivor ballot corpus was not frozen by its deadline.
    #[codec(index = 2)]
    CommitmentDeadlineExpired,
    /// The exact committed threshold-beacon pulse was unavailable after its height.
    #[codec(index = 3)]
    ReleasePulseUnavailable,
    /// The aggregate was not validly opened before its immutable deadline.
    #[codec(index = 4)]
    OpeningDeadlineExpired,
}

/// Closed audit classification for every Parliament body that ends without a result.
///
/// Public-finding failures and private-ballot failures share this event-facing
/// vocabulary so telemetry and audit consumers never need to infer terminal
/// causes from identifiers or private protocol material.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    norito(tag = "reason", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum ParliamentNoResultKindV1 {
    /// Authenticated absences or immutable split endorsements made quorum unreachable.
    #[codec(index = 0)]
    PublicFindingQuorumUnreachable,
    /// A public-finding body remained below quorum after its frozen deadline.
    #[codec(index = 1)]
    PublicFindingDeadlineExpired,
    /// The proof-validated private-ballot registration corpus missed its deadline.
    #[codec(index = 2)]
    BallotRegistrationDeadlineExpired,
    /// The private-ballot survivor roster missed its deadline.
    #[codec(index = 3)]
    BallotSurvivorDeadlineExpired,
    /// The complete private-ballot commitment corpus missed its deadline.
    #[codec(index = 4)]
    BallotCommitmentDeadlineExpired,
    /// The exact committed private-ballot release pulse was unavailable.
    #[codec(index = 5)]
    BallotReleasePulseUnavailable,
    /// The private-ballot aggregate missed its immutable opening deadline.
    #[codec(index = 6)]
    BallotOpeningDeadlineExpired,
    /// The current body exhausted its bounded future-pulse sortition retries.
    #[codec(index = 7)]
    SortitionRetriesExhausted,
}

impl From<ParliamentBallotFailureKindV1> for ParliamentNoResultKindV1 {
    fn from(value: ParliamentBallotFailureKindV1) -> Self {
        match value {
            ParliamentBallotFailureKindV1::RegistrationDeadlineExpired => {
                Self::BallotRegistrationDeadlineExpired
            }
            ParliamentBallotFailureKindV1::SurvivorDeadlineExpired => {
                Self::BallotSurvivorDeadlineExpired
            }
            ParliamentBallotFailureKindV1::CommitmentDeadlineExpired => {
                Self::BallotCommitmentDeadlineExpired
            }
            ParliamentBallotFailureKindV1::ReleasePulseUnavailable => {
                Self::BallotReleasePulseUnavailable
            }
            ParliamentBallotFailureKindV1::OpeningDeadlineExpired => {
                Self::BallotOpeningDeadlineExpired
            }
        }
    }
}

#[derive(Encode)]
struct ParliamentBallotFailureRootPreimageV1 {
    governance_attempt_id: GovernanceAttemptId,
    ballot_attempt_id: BallotAttemptId,
    failure_kind: ParliamentBallotFailureKindV1,
    failure_height: u64,
}

/// Derive the only valid failure root for an objectively failed private ballot.
///
/// The root binds the exact governance and ballot attempts, the failure class
/// derived by Core, and the containing finalized block height. No lifecycle
/// submitter supplies discretionary failure evidence.
#[must_use]
pub fn parliament_ballot_failure_root_v1(
    governance_attempt_id: GovernanceAttemptId,
    ballot_attempt_id: BallotAttemptId,
    failure_kind: ParliamentBallotFailureKindV1,
    failure_height: u64,
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::PARLIAMENT_BALLOT_FAILURE_ROOT_V1,
        &ParliamentBallotFailureRootPreimageV1 {
            governance_attempt_id,
            ballot_attempt_id,
            failure_kind,
            failure_height,
        },
    )
}

/// Canonical snapshot of one retryable hidden Parliament ballot attempt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentBallotAttemptV1 {
    /// Unique ballot-attempt identifier.
    pub id: BallotAttemptId,
    /// Body instance whose formal decision the ballot determines.
    pub body_instance_id: BodyInstanceId,
    /// Zero-based retry sequence within the body instance.
    pub sequence: u32,
    /// Immutable original-seat quorum denominator copied from the body instance.
    pub original_seats: u32,
    /// Current ballot lifecycle state.
    pub status: BallotAttemptStatusV1,
}

#[derive(Encode)]
struct BallotAttemptIdPreimageV1 {
    body_instance_id: BodyInstanceId,
    sequence: u32,
}

impl BallotAttemptId {
    /// Derive the only valid V1 ballot identifier for one body retry.
    #[must_use]
    pub fn derive_v1(body_instance_id: BodyInstanceId, sequence: u32) -> Self {
        Self::new(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::BALLOT_ATTEMPT_ID_V1,
            &BallotAttemptIdPreimageV1 {
                body_instance_id,
                sequence,
            },
        ))
    }
}

#[derive(Encode)]
struct ParliamentBallotParticipantHashPreimageV1 {
    ballot_attempt_id: BallotAttemptId,
    member: AccountId,
}

/// Derive the only valid timed-OVN participant hash for one seated member.
///
/// Binding the participant identity to both the authenticated universal
/// account and the exact ballot attempt prevents a Parliament manager from
/// registering substitute masking keys or replaying a registration between
/// ballots.
#[must_use]
pub fn parliament_ballot_participant_hash_v1(
    ballot_attempt_id: BallotAttemptId,
    member: &AccountId,
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::BALLOT_PARTICIPANT_HASH_V1,
        &ParliamentBallotParticipantHashPreimageV1 {
            ballot_attempt_id,
            member: member.clone(),
        },
    )
}

#[derive(Encode)]
struct TleSessionIdPreimageV1 {
    ballot_attempt_id: BallotAttemptId,
    tle_key_session_id: TleKeySessionId,
    release_beacon_session_id: BeaconSessionId,
    release_height: u64,
}

impl TleSessionId {
    /// Derive the dedicated V1 timelock session for one ballot and release slot.
    #[must_use]
    pub fn derive_v1(
        ballot_attempt_id: BallotAttemptId,
        tle_key_session_id: TleKeySessionId,
        release_beacon_session_id: BeaconSessionId,
        release_height: u64,
    ) -> Self {
        Self::new(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::TLE_SESSION_ID_V1,
            &TleSessionIdPreimageV1 {
                ballot_attempt_id,
                tle_key_session_id,
                release_beacon_session_id,
                release_height,
            },
        ))
    }
}

/// Warning emitted when a sealed body is smaller or more concentrated than requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentConcentrationWarningV1 {
    /// Body instance affected by concentration.
    pub body_instance_id: BodyInstanceId,
    /// Parliament body affected by concentration.
    pub body: ParliamentBody,
    /// Requested seat count.
    pub target_seats: u32,
    /// Nonempty feasible seat count that was sealed.
    pub sealed_seats: u32,
    /// Eligible candidates in the frozen sortition snapshot.
    pub eligible_candidates: u32,
    /// Smallest feasible simultaneous cross-body assignment cap.
    pub cross_body_assignment_cap: u32,
}

/// Return the immutable-seat quorum `ceil(2 × original_seats / 3)`.
#[must_use]
pub const fn parliament_quorum_seats_v1(original_seats: u32) -> u32 {
    let quotient = original_seats / 3;
    let remainder = original_seats % 3;
    quotient * 2
        + match remainder {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => unreachable!(),
        }
}

/// Deterministic aggregate result of a hidden Parliament ballot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    norito(tag = "outcome", content = "details", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum ParliamentAggregateOutcomeV1 {
    /// Quorum was met and Aye strictly exceeded Nay.
    #[codec(index = 0)]
    Approved,
    /// Quorum was met but Aye did not strictly exceed Nay.
    #[codec(index = 1)]
    Rejected,
    /// The accepted corpus did not meet the immutable original-seat quorum.
    #[codec(index = 2)]
    NoQuorum,
    /// The cryptographic protocol terminally failed to yield a valid tally.
    #[codec(index = 3)]
    NoResult,
}

/// Error returned when aggregate ballot counts do not describe one canonical corpus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParliamentTallyErrorV1 {
    /// The option counts did not add up to the accepted corpus size.
    CountSumMismatch {
        /// Accepted corpus size committed by the attempt.
        accepted_ballots: u32,
        /// Sum of Aye, Nay, and Abstain counts.
        counted_ballots: u64,
    },
    /// The accepted corpus exceeded the immutable original-seat count.
    CorpusExceedsOriginalSeats {
        /// Accepted corpus size committed by the attempt.
        accepted_ballots: u32,
        /// Immutable original-seat count.
        original_seats: u32,
    },
}
impl fmt::Display for ParliamentTallyErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountSumMismatch {
                accepted_ballots,
                counted_ballots,
            } => write!(
                f,
                "accepted ballot corpus has {accepted_ballots} entries but option counts total {counted_ballots}"
            ),
            Self::CorpusExceedsOriginalSeats {
                accepted_ballots,
                original_seats,
            } => write!(
                f,
                "accepted ballot corpus {accepted_ballots} exceeds original seat count {original_seats}"
            ),
        }
    }
}
impl std::error::Error for ParliamentTallyErrorV1 {}

/// Canonical aggregate counts for one hidden Parliament ballot attempt.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema,
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentAggregateTallyV1 {
    /// Immutable actual seats selected before absence or dropout.
    pub original_seats: u32,
    /// Ballots in the frozen accepted corpus.
    pub accepted_ballots: u32,
    /// Final decoded Aye count.
    pub aye: u32,
    /// Final decoded Nay count.
    pub nay: u32,
    /// Final decoded Abstain count, included in turnout.
    pub abstain: u32,
}
impl ParliamentAggregateTallyV1 {
    /// Validate count conservation and the immutable-seat upper bound.
    ///
    /// # Errors
    /// Returns [`ParliamentTallyErrorV1`] when counts do not describe the
    /// accepted corpus or the corpus exceeds the original seats.
    pub fn validate(&self) -> Result<(), ParliamentTallyErrorV1> {
        let counted_ballots = u64::from(self.aye) + u64::from(self.nay) + u64::from(self.abstain);
        if counted_ballots != u64::from(self.accepted_ballots) {
            return Err(ParliamentTallyErrorV1::CountSumMismatch {
                accepted_ballots: self.accepted_ballots,
                counted_ballots,
            });
        }
        if self.accepted_ballots > self.original_seats {
            return Err(ParliamentTallyErrorV1::CorpusExceedsOriginalSeats {
                accepted_ballots: self.accepted_ballots,
                original_seats: self.original_seats,
            });
        }
        Ok(())
    }

    /// Evaluate quorum and the strict `Aye > Nay` approval rule.
    ///
    /// A zero-seat denominator deterministically returns
    /// [`ParliamentAggregateOutcomeV1::NoQuorum`].
    ///
    /// # Errors
    /// Returns [`ParliamentTallyErrorV1`] for malformed aggregate counts.
    pub fn decision(&self) -> Result<ParliamentAggregateOutcomeV1, ParliamentTallyErrorV1> {
        self.validate()?;
        if self.original_seats == 0
            || self.accepted_ballots < parliament_quorum_seats_v1(self.original_seats)
        {
            return Ok(ParliamentAggregateOutcomeV1::NoQuorum);
        }
        if self.aye > self.nay {
            Ok(ParliamentAggregateOutcomeV1::Approved)
        } else {
            Ok(ParliamentAggregateOutcomeV1::Rejected)
        }
    }

    /// Return whether an approved result has a strictly sub-five-percent margin.
    ///
    /// The comparison is performed by integer cross multiplication; no division
    /// or zero-denominator special case can change consensus output.
    ///
    /// # Errors
    /// Returns [`ParliamentTallyErrorV1`] for malformed aggregate counts.
    pub fn requires_confirmation(&self) -> Result<bool, ParliamentTallyErrorV1> {
        if self.decision()? != ParliamentAggregateOutcomeV1::Approved {
            return Ok(false);
        }
        let decisive_ballots = u64::from(self.aye) + u64::from(self.nay);
        if decisive_ballots == 0 {
            return Ok(false);
        }
        let margin = u64::from(self.aye.abs_diff(self.nay));
        Ok(margin * 100 < decisive_ballots * 5)
    }
}

#[derive(Encode)]
struct ParliamentBallotResultRootPreimageV1 {
    governance_attempt_id: GovernanceAttemptId,
    body_instance_id: BodyInstanceId,
    ballot_attempt_id: BallotAttemptId,
    opening_root: [u8; 32],
    tally: ParliamentAggregateTallyV1,
    outcome: ParliamentAggregateOutcomeV1,
    result_height: u64,
}

/// Derive the only valid result root for an aggregate-only private ballot.
///
/// The root binds the complete attempt/body/ballot lineage, the replay-derived
/// threshold-opening root, deterministic tally and outcome, and the finalized
/// result height. A lifecycle submitter cannot choose a private body result.
#[must_use]
pub fn parliament_ballot_result_root_v1(
    governance_attempt_id: GovernanceAttemptId,
    body_instance_id: BodyInstanceId,
    ballot_attempt_id: BallotAttemptId,
    opening_root: [u8; 32],
    tally: ParliamentAggregateTallyV1,
    outcome: ParliamentAggregateOutcomeV1,
    result_height: u64,
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::PARLIAMENT_BALLOT_RESULT_ROOT_V1,
        &ParliamentBallotResultRootPreimageV1 {
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            opening_root,
            tally,
            outcome,
            result_height,
        },
    )
}

#[derive(Encode)]
struct ParliamentPublicFindingEndorsementRootPreimageV1 {
    governance_attempt_id: GovernanceAttemptId,
    body_instance_id: BodyInstanceId,
    result_root: [u8; 32],
    endorsing_assignments: Vec<AssignmentId>,
}

/// Derive the canonical root of the exact seated assignments endorsing one
/// public, nonbinding finding.
///
/// Callers must supply assignment identifiers in strict canonical order. Core
/// derives this list from authority-authenticated endorsements; a lifecycle
/// submitter cannot choose the certificate binding.
#[must_use]
pub fn parliament_public_finding_endorsement_root_v1(
    governance_attempt_id: GovernanceAttemptId,
    body_instance_id: BodyInstanceId,
    result_root: [u8; 32],
    endorsing_assignments: &[AssignmentId],
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::PARLIAMENT_PUBLIC_FINDING_ENDORSEMENT_ROOT_V1,
        &ParliamentPublicFindingEndorsementRootPreimageV1 {
            governance_attempt_id,
            body_instance_id,
            result_root,
            endorsing_assignments: endorsing_assignments.to_vec(),
        },
    )
}

/// Absent compare-and-set head required by a governed effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct GovernanceExpectedHeadAbsentV1 {
    /// Stable hash identifying the governed registry subject.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub subject_id: [u8; 32],
}

/// Present compare-and-set head required by a governed effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct GovernanceExpectedHeadPresentV1 {
    /// Stable hash identifying the governed registry subject.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub subject_id: [u8; 32],
    /// Exact expected registry or policy version.
    pub version: u64,
    /// Exact expected canonical head root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub head_root: [u8; 32],
}

/// Typed compare-and-set head bound into a V1 governance certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "head", deny_unknown_fields)
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub enum GovernanceExpectedHeadV1 {
    /// The governed subject must not exist when the certificate executes.
    #[codec(index = 0)]
    Absent(GovernanceExpectedHeadAbsentV1),
    /// The governed subject must match this exact version and root.
    #[codec(index = 1)]
    Present(GovernanceExpectedHeadPresentV1),
}

/// Final ballot transcript bound into a body result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentBallotCertificateBindingV1 {
    /// Ballot attempt whose accepted corpus was opened.
    pub ballot_attempt_id: BallotAttemptId,
    /// Zero-based retry sequence of the ballot attempt.
    pub ballot_attempt_sequence: u32,
    /// Dedicated timelock-encryption key session.
    pub tle_session_id: TleSessionId,
    /// Long-lived threshold-BLS key session used only for TLE release signatures.
    pub tle_key_session_id: TleKeySessionId,
    /// Root of the complete proof-validated registration corpus.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub registration_root: [u8; 32],
    /// Root of the pre-ballot dropout decisions.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub dropout_root: [u8; 32],
    /// Root of the immutable survivor roster.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub survivor_root: [u8; 32],
    /// Root of the frozen accepted ballot corpus.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub corpus_root: [u8; 32],
    /// Sentinel root binding registration, pre-ballot dropout decisions, and the frozen survivor
    /// transcript to the mandatory absence of any post-freeze recovery path.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub no_recovery_root: [u8; 32],
    /// Root of the intrinsic timed-OVN ciphertext and one-hot-proof commitments.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub timed_commitment_root: [u8; 32],
    /// Stable network-scoped logical beacon committed for timed opening.
    pub release_beacon_session_id: BeaconSessionId,
    /// Height at which the ballot attempt and release slot were committed.
    pub registered_at_height: u64,
    /// First height after the proof-registration submission window.
    pub registration_close_height: u64,
    /// First height after the survivor-declaration window.
    pub survivor_freeze_height: u64,
    /// First height after the masked-ballot commitment window.
    pub commitment_close_height: u64,
    /// Height at which consensus froze the exact registration corpus.
    pub registration_closed_at_height: u64,
    /// Height at which consensus froze the exact survivor roster.
    pub survivors_frozen_at_height: u64,
    /// Height within the commitment window at which consensus completed the exact corpus.
    pub commitment_closed_at_height: u64,
    /// Configured retry limit frozen for this ballot lifecycle.
    pub max_ballot_retries: u32,
    /// Configured exact-corpus entry limit frozen for this ballot lifecycle.
    pub max_corpus_entries: u32,
    /// Earliest finalized pulse height permitted to release the aggregate.
    pub release_height: u64,
    /// Last height at which the release pulse or aggregate opening may be consumed.
    pub opening_deadline_height: u64,
    /// Exact finalized threshold-beacon pulse that released the aggregate.
    pub release_pulse_id: BeaconPulseId,
    /// Height at which the exact release pulse was consumed for opening.
    pub opening_height: u64,
    /// Root of the aggregate-only threshold opening transcript.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub opening_root: [u8; 32],
    /// Canonically decoded aggregate tally.
    pub tally: ParliamentAggregateTallyV1,
    /// Final ballot outcome, including protocol-level `NoResult` where applicable.
    pub outcome: ParliamentAggregateOutcomeV1,
}

/// Quorum evidence binding one public, nonbinding Parliament body finding.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentPublicFindingCertificateBindingV1 {
    /// Root of the strict assignment-id sequence endorsing the accepted result root.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub endorsement_root: [u8; 32],
    /// Strictly ordered distinct seated assignments whose authorities endorsed the result.
    pub endorsing_assignments: Vec<AssignmentId>,
    /// Number of distinct seated assignments endorsing the accepted result.
    pub endorsements: u32,
    /// Immutable `ceil(2 × original_seats / 3)` threshold.
    pub quorum: u32,
}

/// Sortition, roster, deliberation, and optional ballot result bound for one body.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct ParliamentBodyCertificateBindingV1 {
    /// Body instance contributing this result.
    pub body_instance_id: BodyInstanceId,
    /// Election attempt that produced the roster.
    pub election_attempt_id: BodyElectionAttemptId,
    /// Zero-based retry sequence of the body-election attempt.
    pub election_attempt_sequence: u32,
    /// Immutable sortition request committed before the future pulse.
    pub sortition_request_id: SortitionRequestId,
    /// Complete immutable request, including candidate snapshot and request/pulse heights.
    ///
    /// The individually repeated identifiers below are retained as convenient
    /// indexes, but validation requires exact equality with this request.
    pub sortition_request: SortitionRequestV1,
    /// Body role of this result.
    pub body: ParliamentBody,
    /// Immutable sealed-seat denominator for this body result.
    pub original_seats: u32,
    /// Beacon key session used for the sortition pulse.
    pub beacon_session_id: BeaconSessionId,
    /// Finalized future pulse consumed by sortition.
    pub beacon_pulse_id: BeaconPulseId,
    /// Root of the ordered sealed roster and alternates.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub roster_root: [u8; 32],
    /// Root of deterministic body and cross-body assignments.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub assignment_root: [u8; 32],
    /// Root of the public evidence, deliberation, and dissent record.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub result_root: [u8; 32],
    /// Height at which this body result became immutable.
    pub result_height: u64,
    /// Authority-authenticated quorum binding for a public nonbinding finding.
    pub public_finding: Option<ParliamentPublicFindingCertificateBindingV1>,
    /// Hidden-ballot binding for a formally voting body; absent for public nonbinding findings.
    pub ballot: Option<ParliamentBallotCertificateBindingV1>,
}

/// Complete automatic V1 governance certificate payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct GovernanceCertificateV1 {
    /// Immutable proposal content authorized by the certificate.
    pub proposal_content_id: ProposalContentId,
    /// Successful end-to-end attempt that produced the certificate.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Zero-based retry sequence of the successful governance attempt.
    pub governance_attempt_sequence: u32,
    /// Final upward-only risk tier applied by the attempt.
    pub risk_tier: RiskTierV1,
    /// Ordered binding for every required body instance.
    pub body_bindings: Vec<ParliamentBodyCertificateBindingV1>,
    /// Governance policy version under which every stage was evaluated.
    pub policy_version: u64,
    /// Hash of the exact deterministic effect preimage.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub effect_preimage_hash: [u8; 32],
    /// Compare-and-set head required when the effect executes.
    pub expected_head: GovernanceExpectedHeadV1,
    /// Height at which the complete certificate was finalized.
    pub certified_at_height: u64,
    /// Exact height at which deterministic enactment is due.
    pub enact_at_height: u64,
}

/// Structural validation failure for a complete V1 governance certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernanceCertificateErrorV1 {
    /// A typed identifier or commitment root used an inert all-zero value.
    ZeroBinding,
    /// A typed identifier did not match its complete domain-separated V1 preimage.
    NonCanonicalIdentifier,
    /// A successful certificate contained no Parliament body results.
    EmptyBodyBindings,
    /// Body results were not in strict canonical V1 body order.
    NonCanonicalBodyOrder,
    /// A body index disagreed with its complete immutable future-pulse request.
    SortitionRequestMismatch,
    /// Two body results reused an attempt, instance, request, ballot, or TLE identifier.
    DuplicateBinding,
    /// The certificate did not contain exactly one Policy Jury result.
    MissingPolicyJury,
    /// A Policy or Confirmation Jury result omitted its private ballot binding.
    MissingBindingBallot,
    /// A nonbinding body result omitted its authenticated public-finding quorum.
    MissingPublicFinding,
    /// A public-finding endorsement count/root/quorum was structurally invalid.
    InvalidPublicFinding,
    /// A successful certificate carried a rejected, no-quorum, or no-result ballot.
    NonApprovingBallot,
    /// The stored tally was malformed.
    InvalidTally(ParliamentTallyErrorV1),
    /// The stored outcome disagreed with the deterministic tally decision.
    TallyOutcomeMismatch,
    /// A private body result root was not derived from its immutable opening and tally.
    BallotResultRootMismatch,
    /// A narrow Policy Jury approval did not have exactly one fresh Confirmation Jury result,
    /// or a non-narrow result carried one.
    ConfirmationJuryMismatch,
    /// The policy version or certification/enactment height ordering was invalid.
    InvalidLifecycle,
    /// The compare-and-set head contained an inert subject or head commitment.
    InvalidExpectedHead,
}
impl fmt::Display for GovernanceCertificateErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBinding => f.write_str("governance certificate bindings must be non-zero"),
            Self::NonCanonicalIdentifier => {
                f.write_str("governance certificate contains a noncanonical identifier")
            }
            Self::EmptyBodyBindings => {
                f.write_str("governance certificate has no Parliament body bindings")
            }
            Self::NonCanonicalBodyOrder => {
                f.write_str("governance certificate body bindings are not in canonical order")
            }
            Self::SortitionRequestMismatch => {
                f.write_str("governance certificate sortition request binding is inconsistent")
            }
            Self::DuplicateBinding => {
                f.write_str("governance certificate reuses an attempt-local binding")
            }
            Self::MissingPolicyJury => {
                f.write_str("governance certificate must contain one Policy Jury result")
            }
            Self::MissingBindingBallot => {
                f.write_str("binding jury result is missing its private ballot")
            }
            Self::MissingPublicFinding => {
                f.write_str("nonbinding body result is missing public-finding quorum evidence")
            }
            Self::InvalidPublicFinding => {
                f.write_str("public-finding endorsement binding is invalid")
            }
            Self::NonApprovingBallot => {
                f.write_str("successful governance certificate contains a non-approving ballot")
            }
            Self::InvalidTally(error) => write!(f, "invalid governance tally: {error}"),
            Self::TallyOutcomeMismatch => {
                f.write_str("governance ballot outcome does not match its aggregate tally")
            }
            Self::BallotResultRootMismatch => {
                f.write_str("governance private-ballot result root is not canonical")
            }
            Self::ConfirmationJuryMismatch => {
                f.write_str("Confirmation Jury presence or future-pulse binding is invalid")
            }
            Self::InvalidLifecycle => {
                f.write_str("governance certificate lifecycle ordering is invalid")
            }
            Self::InvalidExpectedHead => {
                f.write_str("governance certificate compare-and-set head is invalid")
            }
        }
    }
}
impl std::error::Error for GovernanceCertificateErrorV1 {}
impl From<ParliamentTallyErrorV1> for GovernanceCertificateErrorV1 {
    fn from(error: ParliamentTallyErrorV1) -> Self {
        Self::InvalidTally(error)
    }
}

impl GovernanceCertificateV1 {
    /// Validate the certificate's context-free canonical structure.
    ///
    /// Consensus must additionally compare every binding with the persisted
    /// attempt, sortition, roster, beacon, ballot, effect, and compare-and-set
    /// state. This method rejects malformed certificates before those stateful
    /// checks and never infers a missing result.
    ///
    /// # Errors
    /// Returns GovernanceCertificateErrorV1 for an inert, duplicated,
    /// reordered, incomplete, non-approving, or temporally invalid certificate.
    pub fn validate(&self) -> Result<(), GovernanceCertificateErrorV1> {
        if self.proposal_content_id.as_bytes() == &[0; 32]
            || self.governance_attempt_id.as_bytes() == &[0; 32]
            || self.effect_preimage_hash == [0; 32]
        {
            return Err(GovernanceCertificateErrorV1::ZeroBinding);
        }
        if self.governance_attempt_id
            != GovernanceAttemptId::derive_v1(
                self.proposal_content_id,
                self.governance_attempt_sequence,
            )
        {
            return Err(GovernanceCertificateErrorV1::NonCanonicalIdentifier);
        }
        if self.body_bindings.is_empty() {
            return Err(GovernanceCertificateErrorV1::EmptyBodyBindings);
        }
        if self.policy_version == 0
            || self.certified_at_height == 0
            || self.enact_at_height <= self.certified_at_height
        {
            return Err(GovernanceCertificateErrorV1::InvalidLifecycle);
        }
        match self.expected_head {
            GovernanceExpectedHeadV1::Absent(head) => {
                if head.subject_id == [0; 32] {
                    return Err(GovernanceCertificateErrorV1::InvalidExpectedHead);
                }
            }
            GovernanceExpectedHeadV1::Present(head) => {
                if head.subject_id == [0; 32] || head.head_root == [0; 32] {
                    return Err(GovernanceCertificateErrorV1::InvalidExpectedHead);
                }
            }
        }

        let mut body_instance_ids = BTreeSet::new();
        let mut election_attempt_ids = BTreeSet::new();
        let mut sortition_request_ids = BTreeSet::new();
        let mut ballot_attempt_ids = BTreeSet::new();
        let mut tle_session_ids = BTreeSet::new();
        let mut sortition_pulse_ids = BTreeSet::new();
        let mut release_pulse_ids = BTreeSet::new();
        let mut release_slots = BTreeSet::new();
        let mut previous_body = None;
        let mut policy = None;
        let mut confirmation = None;

        for binding in &self.body_bindings {
            if previous_body.is_some_and(|previous| previous >= binding.body) {
                return Err(GovernanceCertificateErrorV1::NonCanonicalBodyOrder);
            }
            previous_body = Some(binding.body);
            let request = &binding.sortition_request;
            if binding.body_instance_id.as_bytes() == &[0; 32]
                || binding.election_attempt_id.as_bytes() == &[0; 32]
                || binding.sortition_request_id.as_bytes() == &[0; 32]
                || binding.beacon_session_id.as_bytes() == &[0; 32]
                || binding.beacon_pulse_id.as_bytes() == &[0; 32]
                || binding.roster_root == [0; 32]
                || binding.assignment_root == [0; 32]
                || binding.result_root == [0; 32]
                || binding.original_seats == 0
            {
                return Err(GovernanceCertificateErrorV1::ZeroBinding);
            }
            if request.validate(None).is_err()
                || request.id != binding.sortition_request_id
                || request.governance_attempt_id != self.governance_attempt_id
                || request.body_election_attempt_id != binding.election_attempt_id
                || request.body != binding.body
                || request.beacon_session_id != binding.beacon_session_id
            {
                return Err(GovernanceCertificateErrorV1::SortitionRequestMismatch);
            }
            if binding.election_attempt_id
                != BodyElectionAttemptId::derive_v1(
                    self.governance_attempt_id,
                    binding.body,
                    binding.election_attempt_sequence,
                )
                || binding.body_instance_id
                    != BodyInstanceId::derive_v1(binding.election_attempt_id, binding.roster_root)
            {
                return Err(GovernanceCertificateErrorV1::NonCanonicalIdentifier);
            }
            if binding.result_height <= request.pulse_height
                || binding.result_height > self.certified_at_height
            {
                return Err(GovernanceCertificateErrorV1::InvalidLifecycle);
            }
            sortition_pulse_ids.insert(binding.beacon_pulse_id);
            if !body_instance_ids.insert(binding.body_instance_id)
                || !election_attempt_ids.insert(binding.election_attempt_id)
                || !sortition_request_ids.insert(binding.sortition_request_id)
            {
                return Err(GovernanceCertificateErrorV1::DuplicateBinding);
            }

            if matches!(
                binding.body,
                ParliamentBody::PolicyJury | ParliamentBody::ConfirmationJury
            ) {
                if binding.ballot.is_none() || binding.public_finding.is_some() {
                    return Err(GovernanceCertificateErrorV1::MissingBindingBallot);
                }
            } else if binding.public_finding.is_none() || binding.ballot.is_some() {
                return Err(GovernanceCertificateErrorV1::MissingPublicFinding);
            }
            if let Some(public_finding) = binding.public_finding.as_ref() {
                let quorum = parliament_quorum_seats_v1(binding.original_seats);
                let endorsements = u32::try_from(public_finding.endorsing_assignments.len())
                    .map_err(|_| GovernanceCertificateErrorV1::InvalidPublicFinding)?;
                if public_finding.endorsement_root == [0; 32]
                    || public_finding.quorum != quorum
                    || public_finding.endorsements != quorum
                    || endorsements != public_finding.endorsements
                    || public_finding
                        .endorsing_assignments
                        .iter()
                        .any(|assignment| assignment.as_bytes() == &[0; 32])
                    || !public_finding
                        .endorsing_assignments
                        .windows(2)
                        .all(|pair| pair[0] < pair[1])
                    || public_finding.endorsement_root
                        != parliament_public_finding_endorsement_root_v1(
                            self.governance_attempt_id,
                            binding.body_instance_id,
                            binding.result_root,
                            &public_finding.endorsing_assignments,
                        )
                {
                    return Err(GovernanceCertificateErrorV1::InvalidPublicFinding);
                }
            }
            if let Some(ballot) = binding.ballot {
                if ballot.ballot_attempt_id.as_bytes() == &[0; 32]
                    || ballot.tle_session_id.as_bytes() == &[0; 32]
                    || ballot.tle_key_session_id.as_bytes() == &[0; 32]
                    || ballot.registration_root == [0; 32]
                    || ballot.dropout_root == [0; 32]
                    || ballot.survivor_root == [0; 32]
                    || ballot.corpus_root == [0; 32]
                    || ballot.no_recovery_root == [0; 32]
                    || ballot.timed_commitment_root == [0; 32]
                    || ballot.release_beacon_session_id.as_bytes() == &[0; 32]
                    || ballot.release_pulse_id.as_bytes() == &[0; 32]
                    || ballot.opening_root == [0; 32]
                {
                    return Err(GovernanceCertificateErrorV1::ZeroBinding);
                }
                if ballot.ballot_attempt_id
                    != BallotAttemptId::derive_v1(
                        binding.body_instance_id,
                        ballot.ballot_attempt_sequence,
                    )
                    || ballot.tle_session_id
                        != TleSessionId::derive_v1(
                            ballot.ballot_attempt_id,
                            ballot.tle_key_session_id,
                            ballot.release_beacon_session_id,
                            ballot.release_height,
                        )
                {
                    return Err(GovernanceCertificateErrorV1::NonCanonicalIdentifier);
                }
                if !ballot_attempt_ids.insert(ballot.ballot_attempt_id)
                    || !tle_session_ids.insert(ballot.tle_session_id)
                    || !release_pulse_ids.insert(ballot.release_pulse_id)
                    || !release_slots
                        .insert((ballot.release_beacon_session_id, ballot.release_height))
                {
                    return Err(GovernanceCertificateErrorV1::DuplicateBinding);
                }
                if ballot.registered_at_height == 0
                    || ballot.registration_close_height <= ballot.registered_at_height
                    || ballot.survivor_freeze_height <= ballot.registration_close_height
                    || ballot.commitment_close_height <= ballot.survivor_freeze_height
                    || ballot.release_height <= ballot.commitment_close_height
                    || ballot.opening_deadline_height <= ballot.release_height
                    || ballot.registration_closed_at_height != ballot.registration_close_height
                    || ballot.survivors_frozen_at_height != ballot.survivor_freeze_height
                    || ballot.commitment_closed_at_height <= ballot.survivor_freeze_height
                    || ballot.commitment_closed_at_height > ballot.commitment_close_height
                    || ballot
                        .registration_close_height
                        .saturating_sub(ballot.registered_at_height)
                        < u64::from(ballot.max_corpus_entries).saturating_add(1)
                    || ballot
                        .survivor_freeze_height
                        .saturating_sub(ballot.registration_close_height)
                        < u64::from(ballot.max_corpus_entries)
                    || ballot
                        .commitment_close_height
                        .saturating_sub(ballot.survivor_freeze_height)
                        < parliament_timed_ovn_required_chunk_blocks_v1(ballot.max_corpus_entries)
                    || ballot.max_ballot_retries > MAX_PARLIAMENT_BALLOT_RETRIES_V1
                    || ballot.ballot_attempt_sequence > ballot.max_ballot_retries
                    || !(1..=MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1)
                        .contains(&ballot.max_corpus_entries)
                    || ballot.max_corpus_entries < binding.original_seats
                    || ballot.tally.accepted_ballots > ballot.max_corpus_entries
                    || ballot.tally.original_seats != binding.original_seats
                    || ballot.opening_height < ballot.release_height
                    || ballot.opening_height > ballot.opening_deadline_height
                    || binding.result_height < ballot.opening_height
                    || binding.result_height > ballot.opening_deadline_height
                {
                    return Err(GovernanceCertificateErrorV1::InvalidLifecycle);
                }
                let decision = ballot.tally.decision()?;
                if decision != ballot.outcome {
                    return Err(GovernanceCertificateErrorV1::TallyOutcomeMismatch);
                }
                if binding.result_root
                    != parliament_ballot_result_root_v1(
                        self.governance_attempt_id,
                        binding.body_instance_id,
                        ballot.ballot_attempt_id,
                        ballot.opening_root,
                        ballot.tally,
                        ballot.outcome,
                        binding.result_height,
                    )
                {
                    return Err(GovernanceCertificateErrorV1::BallotResultRootMismatch);
                }
                if decision != ParliamentAggregateOutcomeV1::Approved {
                    return Err(GovernanceCertificateErrorV1::NonApprovingBallot);
                }
            }

            match binding.body {
                ParliamentBody::PolicyJury => policy = Some(binding),
                ParliamentBody::ConfirmationJury => confirmation = Some(binding),
                _ => {}
            }
        }

        if !sortition_pulse_ids.is_disjoint(&release_pulse_ids) {
            return Err(GovernanceCertificateErrorV1::DuplicateBinding);
        }

        let policy = policy.ok_or(GovernanceCertificateErrorV1::MissingPolicyJury)?;
        let policy_ballot = policy
            .ballot
            .ok_or(GovernanceCertificateErrorV1::MissingBindingBallot)?;
        let requires_confirmation = policy_ballot.tally.requires_confirmation()?;
        match (requires_confirmation, confirmation) {
            (false, None) => {}
            (true, Some(confirmation))
                if confirmation.sortition_request.request_height > policy.result_height
                    && (confirmation.beacon_session_id != policy.beacon_session_id
                        || confirmation.beacon_pulse_id != policy.beacon_pulse_id) => {}
            _ => return Err(GovernanceCertificateErrorV1::ConfirmationJuryMismatch),
        }
        Ok(())
    }
}

impl GovernanceCertificateId {
    /// Derive the content identifier for an exact validated V1 certificate.
    #[must_use]
    pub fn derive_v1(certificate: &GovernanceCertificateV1) -> Self {
        Self::new(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::GOVERNANCE_CERTIFICATE_ID_V1,
            certificate,
        ))
    }
}

#[derive(Encode)]
struct ParliamentExecutionFailureRootPreimageV1 {
    certificate: GovernanceCertificateV1,
    enactment_height: u64,
}

/// Derive the only valid failure root for deterministic certified-effect execution.
///
/// The root commits to the complete canonical certificate and the exact
/// finalized height at which its effect was due. Core derives it only after an
/// isolated effect transaction fails and validates that `enactment_height`
/// equals the certificate's committed `enact_at_height`; no lifecycle
/// submitter contributes error text or discretionary failure evidence.
#[must_use]
pub fn parliament_execution_failure_root_v1(
    certificate: &GovernanceCertificateV1,
    enactment_height: u64,
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::PARLIAMENT_EXECUTION_FAILURE_ROOT_V1,
        &ParliamentExecutionFailureRootPreimageV1 {
            certificate: certificate.clone(),
            enactment_height,
        },
    )
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
    /// Total eligible candidates considered by sortition, or roster entries for a manual roster.
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
impl ProposalKind {
    /// Return the first proposal-owned `u64` that cannot be represented exactly by every SDK.
    ///
    /// This is an exhaustive traversal of the closed first-release proposal enum, including
    /// nested SCCP and Musubi payloads. Admission and restore paths use it before proposal state
    /// can become authoritative.
    #[must_use]
    pub fn first_release_exact_json_u64_invariant_error(&self) -> Option<&'static str> {
        let maximum = FIRST_RELEASE_MAX_EXACT_JSON_U64;
        match self {
            Self::DeployContract(_)
            | Self::ValidationFeePolicy(_)
            | Self::ValidationFeePayoutLifecycle(_) => None,
            Self::RuntimeUpgrade(proposal) => {
                if proposal.manifest.start_height > maximum {
                    Some(
                        "runtime-upgrade proposal start height exceeds the exact JSON integer maximum",
                    )
                } else if proposal.manifest.end_height > maximum {
                    Some(
                        "runtime-upgrade proposal end height exceeds the exact JSON integer maximum",
                    )
                } else {
                    None
                }
            }
            Self::SccpRouteGovernance(proposal) => proposal
                .anchor
                .action
                .first_release_exact_json_u64_invariant_error(maximum),
            Self::MusubiRegistryGovernance(action) => {
                action.first_release_exact_json_u64_invariant_error(maximum)
            }
            Self::SorafsProviderGovernance(_) => None,
        }
    }

    /// Compute the deterministic, proposal-kind-separated fingerprint (`Blake2b-32`).
    #[must_use]
    pub fn fingerprint(&self) -> [u8; 32] {
        let domain = match self {
            Self::DeployContract(_) => crate::governance_fingerprint::DEPLOY_CONTRACT_V1,
            Self::RuntimeUpgrade(_) => crate::governance_fingerprint::RUNTIME_UPGRADE_V1,
            Self::SccpRouteGovernance(_) => crate::governance_fingerprint::SCCP_ROUTE_GOVERNANCE_V1,
            Self::ValidationFeePolicy(_) => crate::governance_fingerprint::VALIDATION_FEE_POLICY_V1,
            Self::ValidationFeePayoutLifecycle(_) => {
                crate::governance_fingerprint::VALIDATION_FEE_PAYOUT_LIFECYCLE_V1
            }
            Self::MusubiRegistryGovernance(_) => {
                crate::governance_fingerprint::MUSUBI_REGISTRY_GOVERNANCE_V1
            }
            Self::SorafsProviderGovernance(_) => {
                crate::governance_fingerprint::SORAFS_PROVIDER_GOVERNANCE_V1
            }
        };
        crate::governance_fingerprint::fingerprint(domain, self)
    }

    /// Hash the exact deterministic effect preimage under a purpose-distinct domain.
    ///
    /// This deliberately commits the complete closed proposal variant again instead of reusing
    /// its content identifier, so a certificate cannot substitute a proposal identity digest for
    /// the independently checked execution binding.
    #[must_use]
    pub fn effect_preimage_hash_v1(&self) -> [u8; 32] {
        crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::GOVERNANCE_EFFECT_PREIMAGE_V1,
            self,
        )
    }

    /// Derive the compare-and-set subject affected by this proposal.
    ///
    /// Proposal identity and governed-subject identity are deliberately distinct:
    /// competing proposals for the same contract, registry, package, alias, release,
    /// provider, or payout lifecycle must contend on one subject head even when their
    /// complete effect preimages differ.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the validation-fee payout binding cannot be
    /// encoded while deriving its canonical lifecycle seal.
    pub fn governed_subject_id_v1(&self) -> Result<[u8; 32], norito::Error> {
        let subject = match self {
            Self::DeployContract(proposal) => {
                GovernanceSubjectPreimageV1::Contract(proposal.contract_address.clone())
            }
            Self::RuntimeUpgrade(proposal) => {
                GovernanceSubjectPreimageV1::RuntimeUpgrade(proposal.manifest.id())
            }
            Self::SccpRouteGovernance(_) => GovernanceSubjectPreimageV1::SccpRouteRegistry,
            Self::ValidationFeePolicy(_) => {
                GovernanceSubjectPreimageV1::ValidationFeePolicyRegistry
            }
            Self::ValidationFeePayoutLifecycle(proposal) => {
                GovernanceSubjectPreimageV1::ValidationFeePayoutLifecycle(
                    proposal.payout_binding.lifecycle_seal()?,
                )
            }
            Self::MusubiRegistryGovernance(action) => match action {
                MusubiParliamentActionV1::RecoverPackageOwners(recovery) => {
                    GovernanceSubjectPreimageV1::MusubiPackage(recovery.package.clone())
                }
                MusubiParliamentActionV1::RetargetAlias(recovery) => {
                    GovernanceSubjectPreimageV1::MusubiAlias(recovery.alias.clone())
                }
                MusubiParliamentActionV1::TakedownArtifact(takedown) => {
                    GovernanceSubjectPreimageV1::MusubiRelease(takedown.release.clone())
                }
                MusubiParliamentActionV1::SetRegistryPolicy(_) => {
                    GovernanceSubjectPreimageV1::MusubiRegistryPolicy
                }
            },
            Self::SorafsProviderGovernance(proposal) => {
                GovernanceSubjectPreimageV1::SorafsProvider(proposal.action.provider_id())
            }
        };
        Ok(crate::governance_fingerprint::fingerprint(
            crate::governance_fingerprint::GOVERNANCE_SUBJECT_ID_V1,
            &subject,
        ))
    }
}

#[derive(Encode)]
enum GovernanceSubjectPreimageV1 {
    #[codec(index = 0)]
    Contract(ContractAddress),
    #[codec(index = 1)]
    RuntimeUpgrade(crate::runtime::RuntimeUpgradeId),
    #[codec(index = 2)]
    SccpRouteRegistry,
    #[codec(index = 3)]
    ValidationFeePolicyRegistry,
    #[codec(index = 4)]
    ValidationFeePayoutLifecycle([u8; 32]),
    #[codec(index = 5)]
    MusubiPackage(crate::musubi::MusubiPackageIdV1),
    #[codec(index = 6)]
    MusubiAlias(crate::musubi::MusubiAliasNameV1),
    #[codec(index = 7)]
    MusubiRelease(crate::musubi::MusubiReleaseIdV1),
    #[codec(index = 8)]
    MusubiRegistryPolicy,
    #[codec(index = 9)]
    SorafsProvider(crate::sorafs::capacity::ProviderId),
}

impl ProposalContentId {
    /// Derive the immutable identifier for exact typed proposal content.
    #[must_use]
    pub fn derive_v1(proposal: &ProposalKind) -> Self {
        Self::new(proposal.fingerprint())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AccountId, DomainId};
    use iroha_crypto::KeyPair;
    use iroha_crypto::blake2::{
        Blake2bVar,
        digest::{Update, VariableOutput},
    };
    use norito::core::DecodeFromSlice;
    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked governance fixture keypair")
    }
    fn checked_account_id() -> AccountId {
        AccountId::new(checked_random_keypair().public_key().clone())
    }
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
    fn contract_hash_rejects_uppercase_hex_alias() {
        let err = ContractCodeHash::from_hex_str(&"AA".repeat(ContractCodeHash::LENGTH))
            .expect_err("uppercase hash aliases must fail closed");
        assert!(matches!(err, HashParseError::InvalidHex { .. }));
        assert!(err.to_string().contains("lowercase hexadecimal"));
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
        norito::core::serialize_to_buffer(&non_canonical, &mut encoded).expect("encode vec");
        let result = <ContractCodeHash as DecodeFromSlice>::decode_from_slice(&encoded);
        assert!(result.is_err());
    }

    #[test]
    fn logical_beacon_session_id_is_stable_nonzero_and_network_scoped() {
        let network = |marker| {
            NetworkId::from_genesis_hash(
                iroha_crypto::HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(
                    iroha_crypto::Hash::prehashed([marker; iroha_crypto::Hash::LENGTH]),
                ),
            )
        };
        let first = network(0x51);
        let second = network(0x52);
        let first_id = BeaconSessionId::for_network_v1(&first);
        assert_eq!(first_id, BeaconSessionId::for_network_v1(&first));
        assert_ne!(first_id, BeaconSessionId::for_network_v1(&second));
        assert!(first_id.as_bytes().iter().any(|byte| *byte != 0));
    }
    #[test]
    fn hash_decode_rejects_versioned_payload_with_trailing_bytes() {
        let mut payload = Vec::with_capacity(4 + ContractCodeHash::LENGTH + 1);
        payload.extend_from_slice(&HASH_WIRE_VERSION_V1.to_le_bytes());
        payload.extend_from_slice(
            &u16::try_from(ContractCodeHash::LENGTH)
                .expect("contract hash length fits u16")
                .to_le_bytes(),
        );
        payload.extend_from_slice(&[0x11; ContractCodeHash::LENGTH]);
        payload.push(0xFF);
        let mut encoded = Vec::new();
        norito::core::serialize_to_buffer(&payload, &mut encoded)
            .expect("encode hostile versioned vector payload");
        assert!(<ContractCodeHash as DecodeFromSlice>::decode_from_slice(&encoded).is_err());
    }
    #[test]
    fn parliament_body_default_is_agenda() {
        assert_eq!(ParliamentBody::default(), ParliamentBody::AgendaCouncil);
    }
    #[test]
    fn ballot_failures_map_exhaustively_to_bounded_no_result_classes() {
        let cases = [
            (
                ParliamentBallotFailureKindV1::RegistrationDeadlineExpired,
                ParliamentNoResultKindV1::BallotRegistrationDeadlineExpired,
            ),
            (
                ParliamentBallotFailureKindV1::SurvivorDeadlineExpired,
                ParliamentNoResultKindV1::BallotSurvivorDeadlineExpired,
            ),
            (
                ParliamentBallotFailureKindV1::CommitmentDeadlineExpired,
                ParliamentNoResultKindV1::BallotCommitmentDeadlineExpired,
            ),
            (
                ParliamentBallotFailureKindV1::ReleasePulseUnavailable,
                ParliamentNoResultKindV1::BallotReleasePulseUnavailable,
            ),
            (
                ParliamentBallotFailureKindV1::OpeningDeadlineExpired,
                ParliamentNoResultKindV1::BallotOpeningDeadlineExpired,
            ),
        ];
        for (index, (ballot, audit)) in cases.into_iter().enumerate() {
            assert_eq!(ParliamentNoResultKindV1::from(ballot), audit);
            assert_eq!(
                ballot.encode(),
                u32::try_from(index)
                    .expect("ballot failure index fits u32")
                    .to_le_bytes()
            );
            assert_eq!(
                audit.encode(),
                u32::try_from(index + 2)
                    .expect("audit failure index fits u32")
                    .to_le_bytes()
            );
        }
    }
    #[test]
    fn governance_types_encode() {
        let code_hash = ContractCodeHash::from_hex_str(&"aa".repeat(32)).expect("code hash");
        let abi_hash = ContractAbiHash::from_hex_str(&"bb".repeat(32)).expect("abi hash");
        let proposal = DeployContractProposal {
            contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse()
                .expect("contract address"),
            code_hash,
            abi_hash,
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        };
        let payload = ProposalKind::DeployContract(proposal.clone());
        assert_ne!(payload.fingerprint(), payload.effect_preimage_hash_v1());
        assert_ne!(
            payload.fingerprint(),
            payload
                .governed_subject_id_v1()
                .expect("derive governed subject")
        );
        let framed = norito::to_bytes(&payload).expect("encode proposal kind");
        let decoded =
            norito::decode_from_bytes::<ProposalKind>(&framed).expect("decode proposal kind");
        match decoded {
            ProposalKind::DeployContract(inner) => {
                assert_eq!(inner.contract_address, proposal.contract_address);
                assert_eq!(inner.code_hash.to_hex(), proposal.code_hash.to_hex());
            }
            ProposalKind::RuntimeUpgrade(_) => panic!("unexpected runtime-upgrade proposal"),
            ProposalKind::SccpRouteGovernance(_) => {
                panic!("unexpected sccp-route-governance proposal")
            }
            ProposalKind::ValidationFeePolicy(_) => {
                panic!("unexpected validation-fee policy proposal")
            }
            ProposalKind::ValidationFeePayoutLifecycle(_) => {
                panic!("unexpected validation-fee payout lifecycle proposal")
            }
            ProposalKind::MusubiRegistryGovernance(_) => {
                panic!("unexpected Musubi registry proposal")
            }
            ProposalKind::SorafsProviderGovernance(_) => {
                panic!("unexpected SoraFS provider-governance proposal")
            }
        }
    }

    #[test]
    fn competing_contract_effects_share_one_governed_subject() {
        let contract_address: ContractAddress =
            "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse()
                .expect("contract address");
        let first = ProposalKind::DeployContract(DeployContractProposal {
            contract_address: contract_address.clone(),
            code_hash: ContractCodeHash::new([0x11; 32]),
            abi_hash: ContractAbiHash::new([0x22; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        });
        let second = ProposalKind::DeployContract(DeployContractProposal {
            contract_address,
            code_hash: ContractCodeHash::new([0x33; 32]),
            abi_hash: ContractAbiHash::new([0x44; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        });

        assert_ne!(first.fingerprint(), second.fingerprint());
        assert_eq!(
            first
                .governed_subject_id_v1()
                .expect("derive first governed subject"),
            second
                .governed_subject_id_v1()
                .expect("derive second governed subject")
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn deploy_proposal_json_rejects_unknown_payload_fields() {
        let proposal = ProposalKind::DeployContract(DeployContractProposal {
            contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse()
                .expect("contract address"),
            code_hash: ContractCodeHash::new([0x11; 32]),
            abi_hash: ContractAbiHash::new([0x22; 32]),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        });
        let canonical =
            norito::json::to_json(&proposal).expect("canonical governance proposal JSON encodes");
        let hostile = canonical.replacen("\"payload\":{", "\"payload\":{\"legacy\":true,", 1);
        assert_ne!(hostile, canonical);
        assert!(
            norito::json::from_json::<ProposalKind>(&hostile).is_err(),
            "governance proposal JSON must reject unknown payload fields"
        );

        let missing_provenance = canonical.replacen(",\"manifest_provenance\":null", "", 1);
        assert_ne!(
            missing_provenance, canonical,
            "canonical proposal JSON must carry the explicit optional provenance field"
        );
        assert!(
            norito::json::from_json::<ProposalKind>(&missing_provenance).is_err(),
            "governance proposal JSON must reject an omitted manifest_provenance field"
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn proposal_kind_json_rejects_unknown_fields_through_musubi_action() {
        let proposal =
            ProposalKind::MusubiRegistryGovernance(MusubiParliamentActionV1::SetRegistryPolicy(
                crate::musubi::MusubiSetRegistryPolicyActionV1 {
                    policy: crate::musubi::MusubiRegistryPolicyV1::default(),
                    expected_revision: 1,
                },
            ));
        let canonical =
            norito::json::to_json(&proposal).expect("canonical governance proposal JSON encodes");
        assert_eq!(
            norito::json::from_json::<ProposalKind>(&canonical)
                .expect("canonical governance proposal JSON decodes"),
            proposal
        );
        for (prefix, depth) in [
            ("{", "the proposal envelope"),
            ("\"payload\":{", "the Musubi action envelope"),
            ("\"value\":{", "the Musubi action payload"),
        ] {
            let replacement = format!("{prefix}\"legacy\":true,");
            let hostile = canonical.replacen(prefix, &replacement, 1);
            assert_ne!(
                hostile, canonical,
                "canonical governance proposal JSON must contain {depth}"
            );
            assert!(
                norito::json::from_json::<ProposalKind>(&hostile).is_err(),
                "governance proposal JSON must reject an unknown field at {depth}"
            );
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
            ProposalKind::SccpRouteGovernance(_) => {
                panic!("unexpected sccp-route-governance proposal")
            }
            ProposalKind::ValidationFeePolicy(_) => {
                panic!("unexpected validation-fee policy proposal")
            }
            ProposalKind::ValidationFeePayoutLifecycle(_) => {
                panic!("unexpected validation-fee payout lifecycle proposal")
            }
            ProposalKind::MusubiRegistryGovernance(_) => {
                panic!("unexpected Musubi registry proposal")
            }
            ProposalKind::SorafsProviderGovernance(_) => {
                panic!("unexpected SoraFS provider-governance proposal")
            }
        }
    }
    #[test]
    fn runtime_upgrade_proposal_bounds_number_encoded_heights() {
        let proposal = |start_height, end_height| {
            ProposalKind::RuntimeUpgrade(RuntimeUpgradeProposal {
                manifest: RuntimeUpgradeManifest {
                    name: "bounded runtime upgrade".to_owned(),
                    description: "exact JSON height fixture".to_owned(),
                    abi_version: 1,
                    abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
                    added_syscalls: Vec::new(),
                    added_pointer_types: Vec::new(),
                    start_height,
                    end_height,
                    sbom_digests: Vec::new(),
                    slsa_attestation: Vec::new(),
                    provenance: Vec::new(),
                },
            })
        };
        let maximum = FIRST_RELEASE_MAX_EXACT_JSON_U64;
        assert_eq!(
            proposal(maximum - 1, maximum).first_release_exact_json_u64_invariant_error(),
            None
        );
        assert!(
            proposal(maximum + 1, maximum + 1)
                .first_release_exact_json_u64_invariant_error()
                .is_some()
        );
        assert!(
            proposal(maximum, maximum + 1)
                .first_release_exact_json_u64_invariant_error()
                .is_some()
        );
    }
    #[test]
    fn sccp_route_governance_proposal_is_boxed_out_of_proposal_kind() {
        assert_eq!(
            core::mem::size_of::<SccpRouteGovernanceProposal>(),
            core::mem::size_of::<Box<crate::isi::bridge::SccpRouteGovernanceAnchorV1>>()
        );
        assert!(
            core::mem::size_of::<ProposalKind>()
                < core::mem::size_of::<SccpRouteGovernanceActionV1>(),
            "ProposalKind must not carry a complete SCCP route action inline"
        );
    }
    #[test]
    fn proposal_fingerprint_matches_manual_derivation() {
        let proposal = DeployContractProposal {
            contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse()
                .expect("contract address"),
            code_hash: ContractCodeHash::from_hex_str(&"11".repeat(32)).expect("code hash"),
            abi_hash: ContractAbiHash::from_hex_str(&"22".repeat(32)).expect("abi hash"),
            abi_version: AbiVersion::new(1),
            manifest_provenance: None,
        };
        let kind = ProposalKind::DeployContract(proposal);
        let fp = kind.fingerprint();
        let manual_bytes = Encode::encode(&kind);
        let domain = crate::governance_fingerprint::DEPLOY_CONTRACT_V1;
        let domain_len = u64::try_from(domain.len())
            .expect("test domain fits in u64")
            .to_le_bytes();
        let mut hasher = Blake2bVar::new(32).expect("Blake2bVar length");
        hasher.update(&domain_len);
        hasher.update(domain);
        hasher.update(&manual_bytes);
        let mut manual_arr = [0u8; 32];
        hasher
            .finalize_variable(&mut manual_arr)
            .expect("finalize Blake2bVar");
        assert_eq!(fp, manual_arr);
        assert_ne!(fp, [0; 32]);
    }
    #[test]
    fn vote_choice_roundtrip() {
        let vote = Vote {
            referendum_id: ProposalId([0x42; 32]),
            voter: AccountId::new(
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
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let members = vec![checked_account_id(), checked_account_id()];
        let alternates = vec![checked_account_id()];
        let roster = ParliamentRoster {
            body: ParliamentBody::RulesCommittee,
            epoch: 3,
            members: members.clone(),
            alternates: alternates.clone(),
            candidate_count: 3,
            derived_by: CouncilDerivationKind::Sortition,
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
    fn canonical_governance_ids_roundtrip_and_reject_legacy_vec_wire() {
        let content_id = ProposalContentId::new([0x41; 32]);
        let attempt_id = GovernanceAttemptId::new([0x42; 32]);
        let content_bytes = norito::to_bytes(&content_id).expect("encode proposal content id");
        let attempt_bytes = norito::to_bytes(&attempt_id).expect("encode governance attempt id");
        assert_eq!(
            norito::decode_from_bytes::<ProposalContentId>(&content_bytes)
                .expect("decode proposal content id"),
            content_id
        );
        assert_eq!(
            norito::decode_from_bytes::<GovernanceAttemptId>(&attempt_bytes)
                .expect("decode governance attempt id"),
            attempt_id
        );

        let mut legacy_vec = Vec::new();
        norito::core::serialize_to_buffer(&vec![0x41_u8; 32], &mut legacy_vec)
            .expect("encode legacy byte vector");
        assert!(
            <ProposalContentId as DecodeFromSlice>::decode_from_slice(&legacy_vec).is_err(),
            "new governance ids must accept only the versioned HashWire32 layout"
        );
    }
    #[test]
    fn risk_tier_only_allows_upward_escalation() {
        assert!(RiskTierV1::Routine.can_escalate_to(RiskTierV1::Routine));
        assert!(RiskTierV1::Routine.can_escalate_to(RiskTierV1::Constitutional));
        assert!(RiskTierV1::Constitutional.can_escalate_to(RiskTierV1::Emergency));
        assert!(!RiskTierV1::Constitutional.can_escalate_to(RiskTierV1::Standard));
        assert!(!RiskTierV1::Emergency.can_escalate_to(RiskTierV1::Constitutional));
    }
    fn checked_sortition_request(
        candidate_count: u32,
        target_seats: u32,
        request_height: u64,
        pulse_height: u64,
        last_consumed_pulse_height: Option<u64>,
    ) -> Result<SortitionRequestV1, SortitionRequestErrorV1> {
        let governance_attempt_id = GovernanceAttemptId::new([0x32; 32]);
        let body_election_attempt_id =
            BodyElectionAttemptId::derive_v1(governance_attempt_id, ParliamentBody::PolicyJury, 2);
        SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            body_election_attempt_id,
            ParliamentBody::PolicyJury,
            [0x34; 32],
            candidate_count,
            target_seats,
            request_height,
            pulse_height,
            BeaconSessionId::new([0x35; 32]),
            last_consumed_pulse_height,
        )
    }
    #[test]
    fn sortition_request_rejects_zero_current_and_reused_pulses() {
        assert_eq!(
            checked_sortition_request(500, 500, 0, 0, None),
            Err(SortitionRequestErrorV1::ZeroPulseHeight)
        );
        assert_eq!(
            checked_sortition_request(500, 500, 40, 40, None),
            Err(SortitionRequestErrorV1::PulseNotStrictlyFuture {
                request_height: 40,
                pulse_height: 40,
            })
        );
        assert_eq!(
            checked_sortition_request(500, 500, 40, 50, Some(50)),
            Err(SortitionRequestErrorV1::PulseAlreadyConsumed {
                pulse_height: 50,
                last_consumed_pulse_height: 50,
            })
        );
    }
    #[test]
    fn sortition_request_enforces_candidate_and_target_bounds() {
        assert_eq!(
            checked_sortition_request(0, 500, 40, 50, None),
            Err(SortitionRequestErrorV1::EmptyCandidateSnapshot)
        );
        assert_eq!(
            checked_sortition_request(500, 0, 40, 50, None),
            Err(SortitionRequestErrorV1::ZeroTargetSeats)
        );
        assert_eq!(
            checked_sortition_request(1_001, MAX_PARLIAMENT_BODY_TARGET_SEATS_V1 + 1, 40, 50, None,),
            Err(SortitionRequestErrorV1::TargetSeatsExceedMaximum {
                target_seats: MAX_PARLIAMENT_BODY_TARGET_SEATS_V1 + 1,
                maximum: MAX_PARLIAMENT_BODY_TARGET_SEATS_V1,
            })
        );
        let undersubscribed =
            checked_sortition_request(1, MAX_PARLIAMENT_BODY_TARGET_SEATS_V1, 40, 50, Some(49))
                .expect("a nonempty tiny electorate remains binding");
        undersubscribed
            .validate(Some(49))
            .expect("valid sortition request revalidates");
    }
    #[test]
    fn sortition_request_rejects_zero_digest_bindings() {
        let request =
            checked_sortition_request(500, 500, 40, 50, None).expect("baseline sortition request");
        for invalid in [
            SortitionRequestV1 {
                id: SortitionRequestId::new([0; 32]),
                ..request
            },
            SortitionRequestV1 {
                governance_attempt_id: GovernanceAttemptId::new([0; 32]),
                ..request
            },
            SortitionRequestV1 {
                body_election_attempt_id: BodyElectionAttemptId::new([0; 32]),
                ..request
            },
            SortitionRequestV1 {
                candidate_root: [0; 32],
                ..request
            },
            SortitionRequestV1 {
                beacon_session_id: BeaconSessionId::new([0; 32]),
                ..request
            },
        ] {
            assert_eq!(
                invalid.validate(None),
                Err(SortitionRequestErrorV1::ZeroBinding)
            );
        }
    }
    #[test]
    fn body_election_attempt_enforces_request_bindings_and_roundtrips() {
        let request =
            checked_sortition_request(500, 500, 40, 50, None).expect("valid future-pulse request");
        assert_eq!(
            BodyElectionAttemptV1::try_new(
                request.body_election_attempt_id,
                GovernanceAttemptId::new([0xFF; 32]),
                0,
                request,
                BodyElectionAttemptStatusV1::AwaitingPulse,
            ),
            Err(BodyElectionAttemptErrorV1::GovernanceAttemptMismatch)
        );
        assert_eq!(
            BodyElectionAttemptV1::try_new(
                BodyElectionAttemptId::new([0xFE; 32]),
                request.governance_attempt_id,
                0,
                request,
                BodyElectionAttemptStatusV1::AwaitingPulse,
            ),
            Err(BodyElectionAttemptErrorV1::ElectionAttemptMismatch)
        );
        let attempt = BodyElectionAttemptV1::try_new(
            request.body_election_attempt_id,
            request.governance_attempt_id,
            2,
            request,
            BodyElectionAttemptStatusV1::Drawing,
        )
        .expect("matching request bindings");
        let bytes = norito::to_bytes(&attempt).expect("encode body-election attempt");
        assert_eq!(
            norito::decode_from_bytes::<BodyElectionAttemptV1>(&bytes)
                .expect("decode body-election attempt"),
            attempt
        );
    }
    #[test]
    fn parliament_body_v1_includes_every_separate_body() {
        for body in PARLIAMENT_BODIES_V1 {
            let bytes = norito::to_bytes(&body).expect("encode Parliament body");
            assert_eq!(
                norito::decode_from_bytes::<ParliamentBody>(&bytes)
                    .expect("decode Parliament body"),
                body
            );
        }
    }
    #[test]
    fn parliament_quorum_is_ceil_two_thirds_without_overflow() {
        assert_eq!(parliament_quorum_seats_v1(0), 0);
        assert_eq!(parliament_quorum_seats_v1(1), 1);
        assert_eq!(parliament_quorum_seats_v1(2), 2);
        assert_eq!(parliament_quorum_seats_v1(3), 2);
        assert_eq!(parliament_quorum_seats_v1(4), 3);
        assert_eq!(parliament_quorum_seats_v1(500), 334);
        assert_eq!(parliament_quorum_seats_v1(u32::MAX), 2_863_311_530);
    }
    #[test]
    fn parliament_tally_validation_enforces_corpus_conservation() {
        let mismatched = ParliamentAggregateTallyV1 {
            original_seats: 5,
            accepted_ballots: 4,
            aye: 2,
            nay: 1,
            abstain: 0,
        };
        assert!(matches!(
            mismatched.validate(),
            Err(ParliamentTallyErrorV1::CountSumMismatch {
                accepted_ballots: 4,
                counted_ballots: 3
            })
        ));
        let oversized = ParliamentAggregateTallyV1 {
            original_seats: 2,
            accepted_ballots: 3,
            aye: 2,
            nay: 1,
            abstain: 0,
        };
        assert!(matches!(
            oversized.validate(),
            Err(ParliamentTallyErrorV1::CorpusExceedsOriginalSeats {
                accepted_ballots: 3,
                original_seats: 2
            })
        ));
    }
    #[test]
    fn parliament_decision_counts_abstain_for_quorum_and_requires_aye_majority() {
        let approved = ParliamentAggregateTallyV1 {
            original_seats: 6,
            accepted_ballots: 4,
            aye: 2,
            nay: 1,
            abstain: 1,
        };
        assert_eq!(
            approved.decision().expect("well-formed approved tally"),
            ParliamentAggregateOutcomeV1::Approved
        );
        let tied = ParliamentAggregateTallyV1 {
            aye: 1,
            nay: 1,
            abstain: 2,
            ..approved
        };
        assert_eq!(
            tied.decision().expect("well-formed tied tally"),
            ParliamentAggregateOutcomeV1::Rejected
        );
        let no_quorum = ParliamentAggregateTallyV1 {
            original_seats: 6,
            accepted_ballots: 3,
            aye: 3,
            nay: 0,
            abstain: 0,
        };
        assert_eq!(
            no_quorum.decision().expect("well-formed low-turnout tally"),
            ParliamentAggregateOutcomeV1::NoQuorum
        );
        assert_eq!(
            ParliamentAggregateTallyV1::default()
                .decision()
                .expect("zero-seat tally is defined"),
            ParliamentAggregateOutcomeV1::NoQuorum
        );
    }
    #[test]
    fn confirmation_margin_is_strictly_below_five_percent() {
        let below_five = ParliamentAggregateTallyV1 {
            original_seats: 41,
            accepted_ballots: 41,
            aye: 21,
            nay: 20,
            abstain: 0,
        };
        assert!(
            below_five
                .requires_confirmation()
                .expect("well-formed narrow tally")
        );
        let exactly_five = ParliamentAggregateTallyV1 {
            original_seats: 40,
            accepted_ballots: 40,
            aye: 21,
            nay: 19,
            abstain: 0,
        };
        assert!(
            !exactly_five
                .requires_confirmation()
                .expect("well-formed exact-boundary tally")
        );
        assert!(
            !ParliamentAggregateTallyV1::default()
                .requires_confirmation()
                .expect("zero denominator is defined")
        );
    }
    #[test]
    fn assignment_plan_root_binds_rank_and_cross_body_cap() {
        let governance_attempt_id = GovernanceAttemptId::new([0x4A; 32]);
        let election_attempt_id = BodyElectionAttemptId::derive_v1(
            governance_attempt_id,
            ParliamentBody::RulesCommittee,
            0,
        );
        let first = checked_account_id();
        let second = checked_account_id();
        let primary = vec![ParliamentSeatAssignmentV1 {
            assignment_id: AssignmentId::derive_v1(election_attempt_id, &first),
            member: first,
        }];
        let alternates = vec![ParliamentSeatAssignmentV1 {
            assignment_id: AssignmentId::derive_v1(election_attempt_id, &second),
            member: second,
        }];
        let root =
            parliament_assignment_plan_root_v1(election_attempt_id, &primary, &alternates, 1);
        assert_ne!(root, [0; 32]);
        assert_ne!(
            root,
            parliament_assignment_plan_root_v1(election_attempt_id, &alternates, &primary, 1,)
        );
        assert_ne!(
            root,
            parliament_assignment_plan_root_v1(election_attempt_id, &primary, &alternates, 2,)
        );
    }

    #[test]
    fn ballot_participant_hash_binds_authenticated_member_and_attempt() {
        let member = checked_account_id();
        let other_member = checked_account_id();
        let first_ballot = BallotAttemptId::new([0xA1; 32]);
        let other_ballot = BallotAttemptId::new([0xA2; 32]);
        let participant_hash = parliament_ballot_participant_hash_v1(first_ballot, &member);
        assert_ne!(participant_hash, [0; 32]);
        assert_eq!(
            participant_hash,
            parliament_ballot_participant_hash_v1(first_ballot, &member)
        );
        assert_ne!(
            participant_hash,
            parliament_ballot_participant_hash_v1(first_ballot, &other_member)
        );
        assert_ne!(
            participant_hash,
            parliament_ballot_participant_hash_v1(other_ballot, &member)
        );
    }
    #[test]
    fn parliament_lifecycle_snapshots_roundtrip() {
        let governance_attempt_id = GovernanceAttemptId::new([0x51; 32]);
        let body_instance_id = BodyInstanceId::new([0x52; 32]);
        let attempt = GovernanceAttemptV1 {
            id: governance_attempt_id,
            proposal_content_id: ProposalContentId::new([0x50; 32]),
            sequence: 2,
            risk_tier: RiskTierV1::Constitutional,
            stage: GovernanceStageV1::PolicyJury,
            status: GovernanceAttemptStatusV1::Active,
        };
        let body = ParliamentBodyInstanceV1 {
            id: body_instance_id,
            governance_attempt_id,
            election_attempt_id: BodyElectionAttemptId::new([0x53; 32]),
            body: ParliamentBody::PolicyJury,
            target_seats: 500,
            original_seats: 497,
            status: BodyInstanceStatusV1::Deliberating(DeliberationPhaseV1::Reflection),
        };
        let ballot = ParliamentBallotAttemptV1 {
            id: BallotAttemptId::new([0x54; 32]),
            body_instance_id,
            sequence: 1,
            original_seats: 497,
            status: BallotAttemptStatusV1::TimedCommitment,
        };
        for (name, encoded, expected) in [
            (
                "attempt",
                norito::to_bytes(&attempt).expect("encode attempt"),
                norito::to_bytes(&attempt).expect("encode expected attempt"),
            ),
            (
                "body",
                norito::to_bytes(&body).expect("encode body"),
                norito::to_bytes(&body).expect("encode expected body"),
            ),
            (
                "ballot",
                norito::to_bytes(&ballot).expect("encode ballot"),
                norito::to_bytes(&ballot).expect("encode expected ballot"),
            ),
        ] {
            assert_eq!(encoded, expected, "{name} encoding must be deterministic");
        }
        assert_eq!(
            norito::decode_from_bytes::<GovernanceAttemptV1>(
                &norito::to_bytes(&attempt).expect("encode governance attempt")
            )
            .expect("decode governance attempt"),
            attempt
        );
        assert_eq!(
            norito::decode_from_bytes::<ParliamentBodyInstanceV1>(
                &norito::to_bytes(&body).expect("encode body instance")
            )
            .expect("decode body instance"),
            body
        );
        assert_eq!(
            norito::decode_from_bytes::<ParliamentBallotAttemptV1>(
                &norito::to_bytes(&ballot).expect("encode ballot attempt")
            )
            .expect("decode ballot attempt"),
            ballot
        );
    }
    #[test]
    fn governance_certificate_v1_roundtrip_binds_body_and_ballot_roots() {
        let tally = ParliamentAggregateTallyV1 {
            original_seats: 500,
            accepted_ballots: 334,
            aye: 200,
            nay: 100,
            abstain: 34,
        };
        let proposal_content_id = ProposalContentId::new([0x61; 32]);
        let governance_attempt_sequence = 0;
        let governance_attempt_id =
            GovernanceAttemptId::derive_v1(proposal_content_id, governance_attempt_sequence);
        let election_attempt_sequence = 0;
        let election_attempt_id = BodyElectionAttemptId::derive_v1(
            governance_attempt_id,
            ParliamentBody::PolicyJury,
            election_attempt_sequence,
        );
        let sortition_request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            ParliamentBody::PolicyJury,
            [0x80; 32],
            1_000,
            500,
            100,
            101,
            BeaconSessionId::new([0x66; 32]),
            None,
        )
        .expect("canonical Policy Jury request");
        let roster_root = [0x68; 32];
        let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
        let ballot_attempt_sequence = 0;
        let ballot_attempt_id =
            BallotAttemptId::derive_v1(body_instance_id, ballot_attempt_sequence);
        let release_beacon_session_id = BeaconSessionId::new([0x85; 32]);
        let tle_key_session_id = TleKeySessionId::new([0x8E; 32]);
        let release_height = 1_757;
        let tle_session_id = TleSessionId::derive_v1(
            ballot_attempt_id,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        );
        let result_height = 1_800;
        let opening_root = [0x7E; 32];
        let outcome = ParliamentAggregateOutcomeV1::Approved;
        let result_root = parliament_ballot_result_root_v1(
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            opening_root,
            tally,
            outcome,
            result_height,
        );
        let certificate = GovernanceCertificateV1 {
            proposal_content_id,
            governance_attempt_id,
            governance_attempt_sequence,
            risk_tier: RiskTierV1::Standard,
            body_bindings: vec![ParliamentBodyCertificateBindingV1 {
                body_instance_id,
                election_attempt_id,
                election_attempt_sequence,
                sortition_request_id: sortition_request.id,
                sortition_request,
                body: ParliamentBody::PolicyJury,
                original_seats: tally.original_seats,
                beacon_session_id: BeaconSessionId::new([0x66; 32]),
                beacon_pulse_id: BeaconPulseId::new([0x67; 32]),
                roster_root,
                assignment_root: [0x69; 32],
                result_root,
                result_height,
                public_finding: None,
                ballot: Some(ParliamentBallotCertificateBindingV1 {
                    ballot_attempt_id,
                    ballot_attempt_sequence,
                    tle_session_id,
                    tle_key_session_id,
                    registration_root: [0x81; 32],
                    dropout_root: [0x82; 32],
                    survivor_root: [0x83; 32],
                    corpus_root: [0x6D; 32],
                    no_recovery_root: [0x6E; 32],
                    timed_commitment_root: [0x84; 32],
                    release_beacon_session_id,
                    registered_at_height: 140,
                    registration_close_height: 641,
                    survivor_freeze_height: 1_141,
                    commitment_close_height: 1_157,
                    registration_closed_at_height: 641,
                    survivors_frozen_at_height: 1_141,
                    commitment_closed_at_height: 1_157,
                    max_ballot_retries: 3,
                    max_corpus_entries: 500,
                    release_height,
                    opening_deadline_height: 2_357,
                    release_pulse_id: BeaconPulseId::new([0x7D; 32]),
                    opening_height: release_height,
                    opening_root,
                    tally,
                    outcome,
                }),
            }],
            policy_version: 7,
            effect_preimage_hash: [0x6F; 32],
            expected_head: GovernanceExpectedHeadV1::Present(GovernanceExpectedHeadPresentV1 {
                subject_id: [0x70; 32],
                version: 3,
                head_root: [0x71; 32],
            }),
            certified_at_height: 10_000,
            enact_at_height: 10_001,
        };
        let bytes = norito::to_bytes(&certificate).expect("encode GovernanceCertificateV1");
        assert_eq!(
            norito::decode_from_bytes::<GovernanceCertificateV1>(&bytes)
                .expect("decode GovernanceCertificateV1"),
            certificate
        );
        certificate
            .validate()
            .expect("wide Policy Jury approval is a complete structural certificate");

        let mut underprovisioned_commitment_window = certificate.clone();
        underprovisioned_commitment_window.body_bindings[0]
            .ballot
            .as_mut()
            .expect("fixture ballot")
            .max_corpus_entries = 513;
        assert_eq!(
            underprovisioned_commitment_window.validate(),
            Err(GovernanceCertificateErrorV1::InvalidLifecycle)
        );

        let mut early_completion = certificate.clone();
        early_completion.body_bindings[0]
            .ballot
            .as_mut()
            .expect("fixture ballot")
            .commitment_closed_at_height = 1_156;
        early_completion
            .validate()
            .expect("corpus completion may occur before the scheduled window close");
        let mut completion_at_freeze = early_completion.clone();
        completion_at_freeze.body_bindings[0]
            .ballot
            .as_mut()
            .expect("fixture ballot")
            .commitment_closed_at_height = 1_141;
        assert_eq!(
            completion_at_freeze.validate(),
            Err(GovernanceCertificateErrorV1::InvalidLifecycle)
        );
        let mut completion_after_close = early_completion;
        completion_after_close.body_bindings[0]
            .ballot
            .as_mut()
            .expect("fixture ballot")
            .commitment_closed_at_height = 1_158;
        assert_eq!(
            completion_after_close.validate(),
            Err(GovernanceCertificateErrorV1::InvalidLifecycle)
        );

        let mut with_public_finding = certificate.clone();
        let public_election_attempt_sequence = 0;
        let public_election_attempt_id = BodyElectionAttemptId::derive_v1(
            governance_attempt_id,
            ParliamentBody::RulesCommittee,
            public_election_attempt_sequence,
        );
        let public_request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            public_election_attempt_id,
            ParliamentBody::RulesCommittee,
            [0xB0; 32],
            3,
            3,
            80,
            81,
            BeaconSessionId::new([0xB1; 32]),
            None,
        )
        .expect("canonical public-finding request");
        let public_roster_root = [0xB2; 32];
        let public_body_instance_id =
            BodyInstanceId::derive_v1(public_election_attempt_id, public_roster_root);
        let public_result_root = [0xB3; 32];
        let endorsing_assignments =
            vec![AssignmentId::new([0xB4; 32]), AssignmentId::new([0xB5; 32])];
        let public_endorsement_root = parliament_public_finding_endorsement_root_v1(
            governance_attempt_id,
            public_body_instance_id,
            public_result_root,
            &endorsing_assignments,
        );
        with_public_finding.body_bindings.insert(
            0,
            ParliamentBodyCertificateBindingV1 {
                body_instance_id: public_body_instance_id,
                election_attempt_id: public_election_attempt_id,
                election_attempt_sequence: public_election_attempt_sequence,
                sortition_request_id: public_request.id,
                sortition_request: public_request,
                body: ParliamentBody::RulesCommittee,
                original_seats: 3,
                beacon_session_id: BeaconSessionId::new([0xB1; 32]),
                beacon_pulse_id: BeaconPulseId::new([0xB6; 32]),
                roster_root: public_roster_root,
                assignment_root: [0xB7; 32],
                result_root: public_result_root,
                result_height: 90,
                public_finding: Some(ParliamentPublicFindingCertificateBindingV1 {
                    endorsement_root: public_endorsement_root,
                    endorsing_assignments,
                    endorsements: 2,
                    quorum: 2,
                }),
                ballot: None,
            },
        );
        with_public_finding
            .validate()
            .expect("public finding carries a self-contained exact quorum binding");
        assert_eq!(
            norito::decode_from_bytes::<GovernanceCertificateV1>(
                &norito::to_bytes(&with_public_finding).expect("encode public-finding certificate")
            )
            .expect("decode public-finding certificate"),
            with_public_finding
        );

        let mut reordered_endorsers = with_public_finding.clone();
        reordered_endorsers.body_bindings[0]
            .public_finding
            .as_mut()
            .expect("public binding")
            .endorsing_assignments
            .swap(0, 1);
        assert_eq!(
            reordered_endorsers.validate(),
            Err(GovernanceCertificateErrorV1::InvalidPublicFinding)
        );
        let mut missing_endorser = with_public_finding.clone();
        missing_endorser.body_bindings[0]
            .public_finding
            .as_mut()
            .expect("public binding")
            .endorsing_assignments
            .pop();
        assert_eq!(
            missing_endorser.validate(),
            Err(GovernanceCertificateErrorV1::InvalidPublicFinding)
        );

        let execution_failure_root =
            parliament_execution_failure_root_v1(&certificate, certificate.enact_at_height);
        assert_ne!(execution_failure_root, [0; 32]);
        assert_ne!(
            execution_failure_root,
            parliament_execution_failure_root_v1(&certificate, certificate.enact_at_height + 1)
        );
        let mut different_certificate = certificate.clone();
        different_certificate.effect_preimage_hash[0] ^= 1;
        assert_ne!(
            execution_failure_root,
            parliament_execution_failure_root_v1(
                &different_certificate,
                certificate.enact_at_height,
            )
        );
        let mut noncanonical_result = certificate.clone();
        noncanonical_result.body_bindings[0].result_root[0] ^= 1;
        assert_eq!(
            noncanonical_result.validate(),
            Err(GovernanceCertificateErrorV1::BallotResultRootMismatch)
        );

        let mut narrow = certificate.clone();
        let policy = narrow
            .body_bindings
            .first_mut()
            .expect("fixture has one Policy Jury binding");
        policy.ballot.as_mut().expect("policy ballot").tally = ParliamentAggregateTallyV1 {
            original_seats: 500,
            accepted_ballots: 500,
            aye: 251,
            nay: 249,
            abstain: 0,
        };
        let policy_ballot = policy.ballot.expect("policy ballot");
        policy.result_root = parliament_ballot_result_root_v1(
            governance_attempt_id,
            policy.body_instance_id,
            policy_ballot.ballot_attempt_id,
            policy_ballot.opening_root,
            policy_ballot.tally,
            policy_ballot.outcome,
            policy.result_height,
        );
        assert_eq!(
            narrow.validate(),
            Err(GovernanceCertificateErrorV1::ConfirmationJuryMismatch)
        );

        let mut confirmation = narrow.body_bindings[0].clone();
        confirmation.body = ParliamentBody::ConfirmationJury;
        confirmation.election_attempt_sequence = 0;
        confirmation.election_attempt_id = BodyElectionAttemptId::derive_v1(
            governance_attempt_id,
            ParliamentBody::ConfirmationJury,
            confirmation.election_attempt_sequence,
        );
        confirmation.sortition_request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            confirmation.election_attempt_id,
            ParliamentBody::ConfirmationJury,
            [0x86; 32],
            500,
            500,
            1_801,
            1_802,
            BeaconSessionId::new([0x66; 32]),
            None,
        )
        .expect("canonical Confirmation Jury request");
        confirmation.sortition_request_id = confirmation.sortition_request.id;
        confirmation.beacon_pulse_id = BeaconPulseId::new([0x75; 32]);
        confirmation.roster_root = [0x76; 32];
        confirmation.body_instance_id =
            BodyInstanceId::derive_v1(confirmation.election_attempt_id, confirmation.roster_root);
        confirmation.assignment_root = [0x77; 32];
        confirmation.result_root = [0x78; 32];
        confirmation.result_height = 3_500;
        let confirmation_ballot_attempt_sequence = 0;
        let confirmation_ballot_attempt_id = BallotAttemptId::derive_v1(
            confirmation.body_instance_id,
            confirmation_ballot_attempt_sequence,
        );
        let confirmation_release_beacon_session_id = BeaconSessionId::new([0x8B; 32]);
        let confirmation_tle_key_session_id = TleKeySessionId::new([0x8F; 32]);
        let confirmation_release_height = 3_457;
        confirmation.ballot = Some(ParliamentBallotCertificateBindingV1 {
            ballot_attempt_id: confirmation_ballot_attempt_id,
            ballot_attempt_sequence: confirmation_ballot_attempt_sequence,
            tle_session_id: TleSessionId::derive_v1(
                confirmation_ballot_attempt_id,
                confirmation_tle_key_session_id,
                confirmation_release_beacon_session_id,
                confirmation_release_height,
            ),
            tle_key_session_id: confirmation_tle_key_session_id,
            registration_root: [0x87; 32],
            dropout_root: [0x88; 32],
            survivor_root: [0x89; 32],
            corpus_root: [0x7B; 32],
            no_recovery_root: [0x7C; 32],
            timed_commitment_root: [0x8A; 32],
            release_beacon_session_id: confirmation_release_beacon_session_id,
            registered_at_height: 1_840,
            registration_close_height: 2_341,
            survivor_freeze_height: 2_841,
            commitment_close_height: 2_857,
            registration_closed_at_height: 2_341,
            survivors_frozen_at_height: 2_841,
            commitment_closed_at_height: 2_857,
            max_ballot_retries: 3,
            max_corpus_entries: 500,
            release_height: confirmation_release_height,
            opening_deadline_height: 4_057,
            release_pulse_id: BeaconPulseId::new([0x8C; 32]),
            opening_height: confirmation_release_height,
            opening_root: [0x8D; 32],
            tally: ParliamentAggregateTallyV1 {
                original_seats: 500,
                accepted_ballots: 500,
                aye: 300,
                nay: 150,
                abstain: 50,
            },
            outcome: ParliamentAggregateOutcomeV1::Approved,
        });
        let confirmation_ballot = confirmation.ballot.expect("confirmation ballot");
        confirmation.result_root = parliament_ballot_result_root_v1(
            governance_attempt_id,
            confirmation.body_instance_id,
            confirmation_ballot.ballot_attempt_id,
            confirmation_ballot.opening_root,
            confirmation_ballot.tally,
            confirmation_ballot.outcome,
            confirmation.result_height,
        );
        narrow.body_bindings.push(confirmation);
        narrow
            .validate()
            .expect("narrow Policy Jury approval has a fresh Confirmation Jury result");

        let policy_pulse_id = narrow.body_bindings[0].beacon_pulse_id;
        narrow.body_bindings[1].beacon_pulse_id = policy_pulse_id;
        assert_eq!(
            narrow.validate(),
            Err(GovernanceCertificateErrorV1::ConfirmationJuryMismatch)
        );
    }

    #[test]
    fn private_ballot_result_root_binds_every_final_component() {
        let attempt = GovernanceAttemptId::new([0x91; 32]);
        let body = BodyInstanceId::new([0x92; 32]);
        let ballot = BallotAttemptId::new([0x93; 32]);
        let opening = [0x94; 32];
        let tally = ParliamentAggregateTallyV1 {
            original_seats: 5,
            accepted_ballots: 4,
            aye: 3,
            nay: 1,
            abstain: 0,
        };
        let outcome = ParliamentAggregateOutcomeV1::Approved;
        let height = 200;
        let expected = parliament_ballot_result_root_v1(
            attempt, body, ballot, opening, tally, outcome, height,
        );
        assert_ne!(
            expected,
            parliament_ballot_result_root_v1(
                GovernanceAttemptId::new([0x95; 32]),
                body,
                ballot,
                opening,
                tally,
                outcome,
                height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_result_root_v1(
                attempt,
                BodyInstanceId::new([0x96; 32]),
                ballot,
                opening,
                tally,
                outcome,
                height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_result_root_v1(
                attempt,
                body,
                BallotAttemptId::new([0x97; 32]),
                opening,
                tally,
                outcome,
                height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_result_root_v1(
                attempt, body, ballot, [0x98; 32], tally, outcome, height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_result_root_v1(
                attempt,
                body,
                ballot,
                opening,
                ParliamentAggregateTallyV1 {
                    aye: 2,
                    nay: 2,
                    ..tally
                },
                outcome,
                height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_result_root_v1(
                attempt,
                body,
                ballot,
                opening,
                tally,
                ParliamentAggregateOutcomeV1::Rejected,
                height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_result_root_v1(
                attempt,
                body,
                ballot,
                opening,
                tally,
                outcome,
                height + 1,
            )
        );
    }

    #[test]
    fn public_finding_endorsement_root_binds_exact_ordered_supporters() {
        let attempt = GovernanceAttemptId::new([0xA1; 32]);
        let body = BodyInstanceId::new([0xA2; 32]);
        let result = [0xA3; 32];
        let first = AssignmentId::new([0xA4; 32]);
        let second = AssignmentId::new([0xA5; 32]);
        let supporters = [first, second];
        let expected =
            parliament_public_finding_endorsement_root_v1(attempt, body, result, &supporters);

        assert_ne!(expected, [0; 32]);
        assert_ne!(
            expected,
            parliament_public_finding_endorsement_root_v1(
                GovernanceAttemptId::new([0xA6; 32]),
                body,
                result,
                &supporters,
            )
        );
        assert_ne!(
            expected,
            parliament_public_finding_endorsement_root_v1(
                attempt,
                BodyInstanceId::new([0xA7; 32]),
                result,
                &supporters,
            )
        );
        assert_ne!(
            expected,
            parliament_public_finding_endorsement_root_v1(attempt, body, [0xA8; 32], &supporters,)
        );
        assert_ne!(
            expected,
            parliament_public_finding_endorsement_root_v1(attempt, body, result, &[second, first],)
        );
        assert_ne!(
            expected,
            parliament_public_finding_endorsement_root_v1(attempt, body, result, &[first],)
        );
    }

    #[test]
    fn private_ballot_failure_root_binds_the_derived_failure_identity() {
        let attempt = GovernanceAttemptId::new([0xA1; 32]);
        let ballot = BallotAttemptId::new([0xA2; 32]);
        let kind = ParliamentBallotFailureKindV1::RegistrationDeadlineExpired;
        let height = 200;
        let expected = parliament_ballot_failure_root_v1(attempt, ballot, kind, height);

        assert_ne!(expected, [0; 32]);
        assert_ne!(
            expected,
            parliament_ballot_failure_root_v1(
                GovernanceAttemptId::new([0xA3; 32]),
                ballot,
                kind,
                height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_failure_root_v1(
                attempt,
                BallotAttemptId::new([0xA4; 32]),
                kind,
                height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_failure_root_v1(
                attempt,
                ballot,
                ParliamentBallotFailureKindV1::SurvivorDeadlineExpired,
                height,
            )
        );
        assert_ne!(
            expected,
            parliament_ballot_failure_root_v1(attempt, ballot, kind, height + 1)
        );
    }
}
