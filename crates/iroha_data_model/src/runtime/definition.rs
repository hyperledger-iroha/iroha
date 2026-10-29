//! Live-upgrade definitions and root selections (`specs/live_runtime_upgrades.md` §3).
//!
//! A definition is the unit of publication. It never changes: its [`DefinitionId`] hashes its
//! kind and canonical descriptor, and the descriptor names every definition it depends on, so
//! adding a definition never alters an existing one. Only implementations, interfaces and
//! migrations reference other definitions, and only definitions that already exist, so the
//! artifact graph is acyclic.
//!
//! An execution root executes exactly one [`RootSelectionV1`] at each height.
//! [`RootSelectionV1::check_closure`] enforces the closure rules: every dependency of an active
//! definition is active or installed, every active interface has exactly one bound
//! implementation, services are root-wide, and every retained completion closure resolves.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;
use norito::codec::{Decode, Encode};

/// Domain separator of [`DefinitionId`].
pub const DEFINITION_ID_DOMAIN: &[u8] = b"iroha:runtime:definition:v1\0";
/// Domain separator of [`SelectionId`].
pub const SELECTION_ID_DOMAIN: &[u8] = b"iroha:runtime:selection:v1\0";
/// Descriptor format version bound into every [`DefinitionId`].
pub const DEFINITION_FORMAT_VERSION: u8 = 1;
/// Maximum UTF-8 length of a name, wire ID, namespace or codec in a descriptor.
pub const MAX_DEFINITION_TEXT_BYTES: usize = 192;
/// Maximum number of identities one descriptor list may reference.
///
/// The bound admits the baseline bundle, which answers every interface of the engine it
/// registers and depends on all of its storage (`live_runtime_upgrades.md` §3.1).
pub const MAX_DEFINITION_REFERENCES: usize = 4096;
/// Maximum number of nodes in a [`CarrierSchemaV1`].
pub const MAX_CARRIER_NODES: usize = 256;
/// Maximum nesting depth of a [`CarrierSchemaV1`].
pub const MAX_CARRIER_DEPTH: usize = 16;
/// Maximum number of fields of a carrier struct or variants of a carrier enum.
pub const MAX_CARRIER_MEMBERS: usize = 64;
/// Maximum declared length bound of a carrier string, byte string, vector or map.
pub const MAX_CARRIER_LENGTH_BOUND: u32 = 1 << 20;
/// Maximum initial bytes of a storage cell or parameter definition.
pub const MAX_INITIAL_BYTES: usize = 64 * 1024;

/// Content-addressed identity of one immutable definition.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::DefinitionId")]
pub struct DefinitionId(pub [u8; 32]);

/// Hex string form shared by both identities.
macro_rules! hex_identity_json {
    ($identity:ty) => {
        impl norito::json::JsonSerialize for $identity {
            fn json_serialize(&self, out: &mut String) {
                crate::json_helpers::fixed_bytes_hex::serialize(&self.0, out);
            }

            fn json_serialize_to(
                &self,
                out: &mut dyn norito::json::JsonWriteSink,
            ) -> Result<(), norito::json::BoundedJsonError> {
                crate::json_helpers::fixed_bytes_hex::serialize_bounded(&self.0, out)
            }
        }

        impl norito::json::JsonDeserialize for $identity {
            fn json_deserialize(
                parser: &mut norito::json::Parser<'_>,
            ) -> Result<Self, norito::json::Error> {
                crate::json_helpers::fixed_bytes_hex::deserialize(parser).map(Self)
            }
        }
    };
}

hex_identity_json!(DefinitionId);
hex_identity_json!(SelectionId);

impl core::fmt::Display for DefinitionId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

/// Identity of one resolved root selection.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::SelectionId")]
pub struct SelectionId(pub [u8; 32]);

impl core::fmt::Display for SelectionId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

/// Kind of a definition. The tag is part of the definition's identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DefinitionKind {
    /// One instruction wire ID with its payload schema and behavioural contract.
    Interface,
    /// The semantics of one execution entrypoint.
    Implementation,
    /// One canonical logical storage namespace.
    Storage,
    /// One event identity and schema.
    Event,
    /// One permission name and payload schema.
    Permission,
    /// One parameter identity, schema and initial value.
    Parameter,
    /// IVM behaviour and host semantics.
    HostProfile,
    /// Root-wide behaviour no single instruction owns.
    ServicesProfile,
    /// How State is committed.
    CommitmentProfile,
    /// Sequencing, framing, atomic publication and restricted recovery.
    Kernel,
    /// A bounded transform between storage definitions.
    Migration,
}

impl DefinitionKind {
    /// Stable tag hashed into [`DefinitionId`].
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Interface => 0,
            Self::Implementation => 1,
            Self::Storage => 2,
            Self::Event => 3,
            Self::Permission => 4,
            Self::Parameter => 5,
            Self::HostProfile => 6,
            Self::ServicesProfile => 7,
            Self::CommitmentProfile => 8,
            Self::Kernel => 9,
            Self::Migration => 10,
        }
    }
}

/// Schema of a payload, storage value, key, event or parameter.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::SchemaV1")]
#[norito(tag = "kind", content = "content")]
pub enum SchemaV1 {
    /// A native Norito type, written as in `sumeragi.md` §16.1:
    /// `norito:<nominal name>@<major>.<minor>.<flags>` or `semantic:<identity>@<major>.<minor>.<flags>`.
    Codec(String),
    /// A bounded generic carrier, for definitions implemented without native Rust types.
    Carrier(CarrierSchemaV1),
}

/// Bounded schema of a generic carrier.
///
/// Nodes are listed children first: a node refers only to nodes before it, and the last node
/// is the root. The list is therefore acyclic by construction.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::CarrierSchemaV1")]
pub struct CarrierSchemaV1 {
    /// Schema nodes, children before parents; the last node is the root.
    pub nodes: Vec<CarrierNodeV1>,
}

/// One node of a [`CarrierSchemaV1`]. Indices refer to earlier nodes of the same schema.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::CarrierNodeV1")]
#[norito(tag = "kind", content = "content")]
pub enum CarrierNodeV1 {
    /// The empty value.
    Unit,
    /// A boolean.
    Bool,
    /// An unsigned 8-bit integer.
    U8,
    /// An unsigned 16-bit integer.
    U16,
    /// An unsigned 32-bit integer.
    U32,
    /// An unsigned 64-bit integer.
    U64,
    /// An unsigned 128-bit integer.
    U128,
    /// A signed 64-bit integer.
    I64,
    /// A UTF-8 string of at most `max_bytes` bytes.
    String {
        /// Maximum encoded length in bytes.
        max_bytes: u32,
    },
    /// A byte string of at most `max_bytes` bytes.
    Bytes {
        /// Maximum length in bytes.
        max_bytes: u32,
    },
    /// A canonical account identity.
    AccountId,
    /// A canonical asset definition identity.
    AssetDefinitionId,
    /// A 32-byte chain hash.
    Hash,
    /// An optional value of node `inner`.
    Option {
        /// Index of the inner node.
        inner: u32,
    },
    /// A vector of at most `max_len` values of node `element`.
    Vec {
        /// Index of the element node.
        element: u32,
        /// Maximum number of elements.
        max_len: u32,
    },
    /// A map in canonical key order with at most `max_len` entries.
    Map {
        /// Index of the key node.
        key: u32,
        /// Index of the value node.
        value: u32,
        /// Maximum number of entries.
        max_len: u32,
    },
    /// A struct with named fields in declaration order.
    Struct {
        /// The fields.
        fields: Vec<CarrierFieldV1>,
    },
    /// An enum with explicit, strictly increasing discriminants.
    Enum {
        /// The variants.
        variants: Vec<CarrierVariantV1>,
    },
}

/// One field of a carrier struct.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::CarrierFieldV1")]
pub struct CarrierFieldV1 {
    /// Field name.
    pub name: String,
    /// Index of the field's node.
    pub node: u32,
}

/// One variant of a carrier enum.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::CarrierVariantV1")]
pub struct CarrierVariantV1 {
    /// Wire discriminant.
    pub discriminant: u32,
    /// Variant name.
    pub name: String,
    /// Index of the variant's payload node.
    pub node: u32,
}

/// One instruction wire ID: payload schema and behavioural contract.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::InterfaceDescriptorV1")]
pub struct InterfaceDescriptorV1 {
    /// Instruction wire ID.
    pub wire_id: String,
    /// Payload schema.
    pub payload: SchemaV1,
    /// Stable rejection codes the interface may return, sorted and unique.
    pub rejection_codes: Vec<String>,
    /// Event definitions the interface may emit, sorted and unique.
    pub events: Vec<DefinitionId>,
    /// Metering class every implementation is charged under.
    pub metering_class: String,
    /// Digest of the archived normative statement of observable behaviour.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub contract_statement: [u8; 32],
}

/// Where an implementation's code comes from.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::ImplementationCodeV1")]
#[norito(tag = "kind", content = "content")]
pub enum ImplementationCodeV1 {
    /// Native code compiled into releases.
    Native {
        /// Stable module name.
        module: String,
        /// Digest of the archived reference-source closure.
        #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
        reference_closure: [u8; 32],
    },
    /// IVM bytecode.
    Ivm {
        /// Hash of the `.to` artifact.
        #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
        code_hash: [u8; 32],
        /// Entrypoint inside the artifact.
        entrypoint: String,
    },
}

/// What an implementation answers.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::EntrypointV1")]
#[norito(tag = "kind", content = "content")]
pub enum EntrypointV1 {
    /// The instructions of interface definitions, sorted and unique. One implementation answers
    /// several only as a bundle whose handlers cannot yet be separated, such as the baseline.
    Instructions(Vec<DefinitionId>),
    /// A named hook of the services profile.
    Hook(String),
    /// A named query.
    Query(String),
}

/// Resource bounds of one invocation or migration stage.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::ResourceLimitsV1")]
pub struct ResourceLimitsV1 {
    /// Maximum gas.
    pub max_gas: u64,
    /// Maximum number of effects.
    pub max_effects: u32,
    /// Maximum nested call depth.
    pub max_call_depth: u16,
}

/// The semantics of one execution entrypoint and everything it relies on.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::ImplementationDescriptorV1")]
pub struct ImplementationDescriptorV1 {
    /// What this implementation answers.
    pub entrypoint: EntrypointV1,
    /// Its code.
    pub code: ImplementationCodeV1,
    /// Storage it reads, sorted and unique.
    pub reads: Vec<DefinitionId>,
    /// Storage it writes, sorted and unique.
    pub writes: Vec<DefinitionId>,
    /// Storage it scans, sorted and unique.
    pub scans: Vec<DefinitionId>,
    /// Events it emits, sorted and unique.
    pub events: Vec<DefinitionId>,
    /// Permissions it checks, sorted and unique.
    pub permissions: Vec<DefinitionId>,
    /// Parameters it reads, sorted and unique.
    pub parameters: Vec<DefinitionId>,
    /// Interfaces it invokes, sorted and unique.
    pub invokes: Vec<DefinitionId>,
    /// Host profile; required for IVM code and absent for native code.
    pub host_profile: Option<DefinitionId>,
    /// Execution services profile it relies on.
    pub services_profile: DefinitionId,
    /// Resource bounds of one invocation.
    pub limits: ResourceLimitsV1,
}

/// Shape of a storage namespace.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::StorageShapeV1")]
#[norito(tag = "kind", content = "content")]
pub enum StorageShapeV1 {
    /// Keyed entries in canonical key order.
    Table {
        /// Key schema.
        key: SchemaV1,
    },
    /// One value.
    Cell,
}

/// How storage values enter the State commitment.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::ProjectionV1")]
#[norito(tag = "kind", content = "content")]
pub enum ProjectionV1 {
    /// The bare canonical Norito value.
    BareNorito,
    /// A registered semantic projection.
    Semantic(String),
}

/// One canonical logical storage namespace.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::StorageDescriptorV1")]
pub struct StorageDescriptorV1 {
    /// Canonical namespace identity.
    pub namespace: String,
    /// Table or cell.
    pub shape: StorageShapeV1,
    /// Value schema.
    pub value: SchemaV1,
    /// Initial value bytes a cell's installation writes. `None` for a table, which starts
    /// empty, and for a cell populated by the act that founds its root (a genesis or an
    /// adoption); a change set cannot install such a cell.
    #[norito(json = "crate::json_helpers::base64_vec::option")]
    pub initial: Option<Vec<u8>>,
    /// Digest of the archived invariant statement.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub invariants: [u8; 32],
    /// Commitment projection.
    pub projection: ProjectionV1,
}

/// One event identity and schema.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::EventDescriptorV1")]
pub struct EventDescriptorV1 {
    /// Event identity.
    pub name: String,
    /// Event schema.
    pub schema: SchemaV1,
}

/// One permission name and payload schema.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::PermissionDescriptorV1")]
pub struct PermissionDescriptorV1 {
    /// Permission name.
    pub name: String,
    /// Permission payload schema.
    pub payload: SchemaV1,
}

/// One parameter identity, schema and initial value.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::ParameterDescriptorV1")]
pub struct ParameterDescriptorV1 {
    /// Parameter identity.
    pub name: String,
    /// Parameter schema.
    pub schema: SchemaV1,
    /// Initial value bytes.
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub initial: Vec<u8>,
}

/// IVM behaviour and host semantics.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::HostProfileDescriptorV1")]
pub struct HostProfileDescriptorV1 {
    /// ABI version contracts declare.
    pub abi_version: u16,
    /// Hash of the complete ABI descriptor (`ivm::syscalls::compute_abi_hash`).
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub abi_hash: [u8; 32],
    /// Hash of the metering schedule.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub metering: [u8; 32],
}

/// One named root-wide service and its reference closure.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::ServiceEntryV1")]
pub struct ServiceEntryV1 {
    /// Service name.
    pub name: String,
    /// Digest of the archived reference-source closure.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub reference_closure: [u8; 32],
}

/// Root-wide behaviour no single instruction owns.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::ServicesProfileDescriptorV1")]
pub struct ServicesProfileDescriptorV1 {
    /// Services, sorted by unique name.
    pub services: Vec<ServiceEntryV1>,
}

/// How State is committed.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::CommitmentProfileDescriptorV1")]
pub struct CommitmentProfileDescriptorV1 {
    /// Commitment scheme identity.
    pub scheme: String,
    /// Digest of the scheme's fixed parameters.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub parameters: [u8; 32],
}

/// Sequencing, framing, atomic publication and restricted recovery.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::KernelDescriptorV1")]
pub struct KernelDescriptorV1 {
    /// Kernel name.
    pub name: String,
    /// Digest of the archived reference-source closure.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub reference_closure: [u8; 32],
}

/// A bounded transform between storage definitions.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::MigrationDescriptorV1")]
pub struct MigrationDescriptorV1 {
    /// Migration code.
    pub code: ImplementationCodeV1,
    /// Storage it reads, sorted and unique.
    pub from: Vec<DefinitionId>,
    /// Storage it fills, sorted and unique.
    pub to: Vec<DefinitionId>,
    /// Schema of the committed cursor.
    pub cursor: SchemaV1,
    /// Host profile; required for IVM code and absent for native code.
    pub host_profile: Option<DefinitionId>,
    /// Execution services profile it relies on.
    pub services_profile: DefinitionId,
    /// Resource bounds of one stage.
    pub limits: ResourceLimitsV1,
}

/// The canonical descriptor of one definition.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::DefinitionDescriptorV1")]
#[norito(tag = "kind", content = "content")]
pub enum DefinitionDescriptorV1 {
    /// An interface.
    Interface(InterfaceDescriptorV1),
    /// An implementation.
    Implementation(ImplementationDescriptorV1),
    /// A storage namespace.
    Storage(StorageDescriptorV1),
    /// An event.
    Event(EventDescriptorV1),
    /// A permission.
    Permission(PermissionDescriptorV1),
    /// A parameter.
    Parameter(ParameterDescriptorV1),
    /// A host profile.
    HostProfile(HostProfileDescriptorV1),
    /// An execution services profile.
    ServicesProfile(ServicesProfileDescriptorV1),
    /// A commitment profile.
    CommitmentProfile(CommitmentProfileDescriptorV1),
    /// A kernel.
    Kernel(KernelDescriptorV1),
    /// A migration.
    Migration(MigrationDescriptorV1),
}

/// A descriptor that violates its format rules.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DefinitionError {
    /// A name, wire ID, namespace or codec is empty, too long or uses a forbidden character.
    #[error("invalid {field}: {reason}")]
    InvalidText {
        /// Which field.
        field: &'static str,
        /// Why it is invalid.
        reason: &'static str,
    },
    /// A reference list is unsorted, repeated or too long.
    #[error("{field} must be sorted, unique and within its length bound")]
    NonCanonicalList {
        /// Which field.
        field: &'static str,
    },
    /// A carrier schema breaks its bounds or ordering.
    #[error("invalid carrier schema: {0}")]
    InvalidCarrier(&'static str),
    /// Initial bytes are missing, present where forbidden, or too large.
    #[error("invalid initial bytes: {0}")]
    InvalidInitial(&'static str),
    /// IVM code lacks a host profile, or native code names one.
    #[error("{0}")]
    HostProfileMismatch(&'static str),
    /// Resource limits are zero.
    #[error("resource limits must be non-zero")]
    ZeroLimits,
    /// The descriptor cannot be encoded canonically.
    #[error("descriptor encoding failed: {0}")]
    Encoding(String),
}

impl DefinitionDescriptorV1 {
    /// Kind of this definition.
    #[must_use]
    pub const fn kind(&self) -> DefinitionKind {
        match self {
            Self::Interface(_) => DefinitionKind::Interface,
            Self::Implementation(_) => DefinitionKind::Implementation,
            Self::Storage(_) => DefinitionKind::Storage,
            Self::Event(_) => DefinitionKind::Event,
            Self::Permission(_) => DefinitionKind::Permission,
            Self::Parameter(_) => DefinitionKind::Parameter,
            Self::HostProfile(_) => DefinitionKind::HostProfile,
            Self::ServicesProfile(_) => DefinitionKind::ServicesProfile,
            Self::CommitmentProfile(_) => DefinitionKind::CommitmentProfile,
            Self::Kernel(_) => DefinitionKind::Kernel,
            Self::Migration(_) => DefinitionKind::Migration,
        }
    }

    /// Canonical Norito bytes of this descriptor.
    ///
    /// # Errors
    /// The descriptor cannot be encoded.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DefinitionError> {
        norito::encode_canonical(self).map_err(|error| DefinitionError::Encoding(error.to_string()))
    }

    /// Content-addressed identity:
    /// `H(DEFINITION_ID_DOMAIN ‖ version ‖ kind tag ‖ le64(len) ‖ canonical bytes)`.
    ///
    /// # Errors
    /// The descriptor violates its format rules or cannot be encoded.
    pub fn id(&self) -> Result<DefinitionId, DefinitionError> {
        self.validate()?;
        let bytes = self.canonical_bytes()?;
        let length = u64::try_from(bytes.len())
            .map_err(|_| DefinitionError::Encoding("descriptor length exceeds u64".into()))?;
        let hash = Hash::new_from_chunks(&[
            DEFINITION_ID_DOMAIN,
            &[DEFINITION_FORMAT_VERSION, self.kind().tag()],
            &length.to_le_bytes(),
            &bytes,
        ]);
        Ok(DefinitionId(hash.into()))
    }

    /// Every definition this one references, grouped by the kind each must have.
    #[must_use]
    pub fn typed_dependencies(&self) -> Vec<(DefinitionKind, DefinitionId)> {
        let mut out = Vec::new();
        let mut push = |kind, ids: &[DefinitionId]| {
            out.extend(ids.iter().map(|id| (kind, *id)));
        };
        match self {
            Self::Interface(interface) => push(DefinitionKind::Event, &interface.events),
            Self::Implementation(implementation) => {
                if let EntrypointV1::Instructions(interfaces) = &implementation.entrypoint {
                    push(DefinitionKind::Interface, interfaces);
                }
                push(DefinitionKind::Storage, &implementation.reads);
                push(DefinitionKind::Storage, &implementation.writes);
                push(DefinitionKind::Storage, &implementation.scans);
                push(DefinitionKind::Event, &implementation.events);
                push(DefinitionKind::Permission, &implementation.permissions);
                push(DefinitionKind::Parameter, &implementation.parameters);
                push(DefinitionKind::Interface, &implementation.invokes);
                if let Some(host) = &implementation.host_profile {
                    push(DefinitionKind::HostProfile, core::slice::from_ref(host));
                }
                push(
                    DefinitionKind::ServicesProfile,
                    core::slice::from_ref(&implementation.services_profile),
                );
            }
            Self::Migration(migration) => {
                push(DefinitionKind::Storage, &migration.from);
                push(DefinitionKind::Storage, &migration.to);
                if let Some(host) = &migration.host_profile {
                    push(DefinitionKind::HostProfile, core::slice::from_ref(host));
                }
                push(
                    DefinitionKind::ServicesProfile,
                    core::slice::from_ref(&migration.services_profile),
                );
            }
            Self::Storage(_)
            | Self::Event(_)
            | Self::Permission(_)
            | Self::Parameter(_)
            | Self::HostProfile(_)
            | Self::ServicesProfile(_)
            | Self::CommitmentProfile(_)
            | Self::Kernel(_) => {}
        }
        out
    }

    /// Check the format rules that need no other definition.
    ///
    /// # Errors
    /// The first rule the descriptor violates.
    pub fn validate(&self) -> Result<(), DefinitionError> {
        match self {
            Self::Interface(interface) => {
                check_name("wire_id", &interface.wire_id)?;
                check_schema(&interface.payload)?;
                check_sorted_text("rejection_codes", &interface.rejection_codes)?;
                check_ids("events", &interface.events)?;
                check_name("metering_class", &interface.metering_class)
            }
            Self::Implementation(implementation) => {
                match &implementation.entrypoint {
                    EntrypointV1::Instructions(interfaces) => {
                        if interfaces.is_empty() {
                            return Err(DefinitionError::NonCanonicalList {
                                field: "entrypoint",
                            });
                        }
                        check_ids("entrypoint", interfaces)?;
                    }
                    EntrypointV1::Hook(name) | EntrypointV1::Query(name) => {
                        check_name("entrypoint", name)?;
                    }
                }
                check_code(&implementation.code, implementation.host_profile.as_ref())?;
                check_ids("reads", &implementation.reads)?;
                check_ids("writes", &implementation.writes)?;
                check_ids("scans", &implementation.scans)?;
                check_ids("events", &implementation.events)?;
                check_ids("permissions", &implementation.permissions)?;
                check_ids("parameters", &implementation.parameters)?;
                check_ids("invokes", &implementation.invokes)?;
                check_limits(implementation.limits)
            }
            Self::Storage(storage) => {
                check_name("namespace", &storage.namespace)?;
                match (&storage.shape, &storage.initial) {
                    (StorageShapeV1::Table { key }, None) => check_schema(key)?,
                    (StorageShapeV1::Table { .. }, Some(_)) => {
                        return Err(DefinitionError::InvalidInitial(
                            "a table starts empty and has no initial bytes",
                        ));
                    }
                    (StorageShapeV1::Cell, Some(initial)) => check_initial(initial)?,
                    (StorageShapeV1::Cell, None) => {}
                }
                check_schema(&storage.value)?;
                if let ProjectionV1::Semantic(identity) = &storage.projection {
                    check_codec("projection", identity)?;
                }
                Ok(())
            }
            Self::Event(event) => {
                check_name("event name", &event.name)?;
                check_schema(&event.schema)
            }
            Self::Permission(permission) => {
                check_name("permission name", &permission.name)?;
                check_schema(&permission.payload)
            }
            Self::Parameter(parameter) => {
                check_name("parameter name", &parameter.name)?;
                check_schema(&parameter.schema)?;
                check_initial(&parameter.initial)
            }
            Self::HostProfile(_) => Ok(()),
            Self::ServicesProfile(services) => {
                if services.services.is_empty() {
                    return Err(DefinitionError::NonCanonicalList { field: "services" });
                }
                let names: Vec<String> = services
                    .services
                    .iter()
                    .map(|service| service.name.clone())
                    .collect();
                check_sorted_text("services", &names)
            }
            Self::CommitmentProfile(profile) => check_name("scheme", &profile.scheme),
            Self::Kernel(kernel) => check_name("kernel name", &kernel.name),
            Self::Migration(migration) => {
                check_code(&migration.code, migration.host_profile.as_ref())?;
                check_ids("from", &migration.from)?;
                check_ids("to", &migration.to)?;
                if migration.from.is_empty() || migration.to.is_empty() {
                    return Err(DefinitionError::NonCanonicalList {
                        field: "migration storage",
                    });
                }
                check_schema(&migration.cursor)?;
                check_limits(migration.limits)
            }
        }
    }
}

fn check_name(field: &'static str, value: &str) -> Result<(), DefinitionError> {
    check_length(field, value)?;
    if value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-' | b'@' | b'/')
    }) {
        Ok(())
    } else {
        Err(DefinitionError::InvalidText {
            field,
            reason: "only ASCII letters, digits and . _ : - @ / are allowed",
        })
    }
}

fn check_codec(field: &'static str, value: &str) -> Result<(), DefinitionError> {
    check_length(field, value)?;
    if value.starts_with(' ') || value.ends_with(' ') {
        return Err(DefinitionError::InvalidText {
            field,
            reason: "leading or trailing space",
        });
    }
    if value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
        Ok(())
    } else {
        Err(DefinitionError::InvalidText {
            field,
            reason: "only printable ASCII is allowed",
        })
    }
}

fn check_length(field: &'static str, value: &str) -> Result<(), DefinitionError> {
    if value.is_empty() {
        Err(DefinitionError::InvalidText {
            field,
            reason: "empty",
        })
    } else if value.len() > MAX_DEFINITION_TEXT_BYTES {
        Err(DefinitionError::InvalidText {
            field,
            reason: "too long",
        })
    } else {
        Ok(())
    }
}

fn check_sorted_text(field: &'static str, values: &[String]) -> Result<(), DefinitionError> {
    if values.len() > MAX_DEFINITION_REFERENCES || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(DefinitionError::NonCanonicalList { field });
    }
    values.iter().try_for_each(|value| check_name(field, value))
}

fn check_ids(field: &'static str, ids: &[DefinitionId]) -> Result<(), DefinitionError> {
    if ids.len() > MAX_DEFINITION_REFERENCES || ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(DefinitionError::NonCanonicalList { field })
    } else {
        Ok(())
    }
}

fn check_initial(initial: &[u8]) -> Result<(), DefinitionError> {
    if initial.is_empty() {
        Err(DefinitionError::InvalidInitial(
            "a cell or parameter needs its initial bytes",
        ))
    } else if initial.len() > MAX_INITIAL_BYTES {
        Err(DefinitionError::InvalidInitial("initial bytes too large"))
    } else {
        Ok(())
    }
}

fn check_limits(limits: ResourceLimitsV1) -> Result<(), DefinitionError> {
    if limits.max_gas == 0 || limits.max_effects == 0 || limits.max_call_depth == 0 {
        Err(DefinitionError::ZeroLimits)
    } else {
        Ok(())
    }
}

fn check_code(
    code: &ImplementationCodeV1,
    host_profile: Option<&DefinitionId>,
) -> Result<(), DefinitionError> {
    match (code, host_profile) {
        (ImplementationCodeV1::Native { module, .. }, None) => check_name("module", module),
        (ImplementationCodeV1::Ivm { entrypoint, .. }, Some(_)) => {
            check_name("entrypoint", entrypoint)
        }
        (ImplementationCodeV1::Native { .. }, Some(_)) => Err(
            DefinitionError::HostProfileMismatch("native code does not run on an IVM host profile"),
        ),
        (ImplementationCodeV1::Ivm { .. }, None) => Err(DefinitionError::HostProfileMismatch(
            "IVM code needs a host profile",
        )),
    }
}

fn check_schema(schema: &SchemaV1) -> Result<(), DefinitionError> {
    match schema {
        SchemaV1::Codec(codec) => {
            if codec.starts_with("norito:") || codec.starts_with("semantic:") {
                check_codec("codec", codec)
            } else {
                Err(DefinitionError::InvalidText {
                    field: "codec",
                    reason: "must start with norito: or semantic:",
                })
            }
        }
        SchemaV1::Carrier(carrier) => carrier.validate(),
    }
}

impl CarrierSchemaV1 {
    /// Check node order, bounds, depth and member names.
    ///
    /// # Errors
    /// The first rule the schema violates.
    pub fn validate(&self) -> Result<(), DefinitionError> {
        if self.nodes.is_empty() || self.nodes.len() > MAX_CARRIER_NODES {
            return Err(DefinitionError::InvalidCarrier("node count out of bounds"));
        }
        let mut depth = Vec::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            let child = |reference: u32| -> Result<usize, DefinitionError> {
                let reference = usize::try_from(reference)
                    .map_err(|_| DefinitionError::InvalidCarrier("child index out of range"))?;
                if reference < index {
                    Ok(depth[reference])
                } else {
                    Err(DefinitionError::InvalidCarrier(
                        "a node may only refer to earlier nodes",
                    ))
                }
            };
            let bound = |value: u32| {
                if value == 0 || value > MAX_CARRIER_LENGTH_BOUND {
                    Err(DefinitionError::InvalidCarrier("length bound out of range"))
                } else {
                    Ok(())
                }
            };
            let node_depth = match node {
                CarrierNodeV1::Unit
                | CarrierNodeV1::Bool
                | CarrierNodeV1::U8
                | CarrierNodeV1::U16
                | CarrierNodeV1::U32
                | CarrierNodeV1::U64
                | CarrierNodeV1::U128
                | CarrierNodeV1::I64
                | CarrierNodeV1::AccountId
                | CarrierNodeV1::AssetDefinitionId
                | CarrierNodeV1::Hash => 1,
                CarrierNodeV1::String { max_bytes } | CarrierNodeV1::Bytes { max_bytes } => {
                    bound(*max_bytes)?;
                    1
                }
                CarrierNodeV1::Option { inner } => child(*inner)? + 1,
                CarrierNodeV1::Vec { element, max_len } => {
                    bound(*max_len)?;
                    child(*element)? + 1
                }
                CarrierNodeV1::Map {
                    key,
                    value,
                    max_len,
                } => {
                    bound(*max_len)?;
                    child(*key)?.max(child(*value)?) + 1
                }
                CarrierNodeV1::Struct { fields } => {
                    check_members(fields.iter().map(|field| field.name.as_str()))?;
                    let mut deepest = 0;
                    for field in fields {
                        deepest = deepest.max(child(field.node)?);
                    }
                    deepest + 1
                }
                CarrierNodeV1::Enum { variants } => {
                    check_members(variants.iter().map(|variant| variant.name.as_str()))?;
                    if variants
                        .windows(2)
                        .any(|pair| pair[0].discriminant >= pair[1].discriminant)
                    {
                        return Err(DefinitionError::InvalidCarrier(
                            "enum discriminants must be strictly increasing",
                        ));
                    }
                    let mut deepest = 0;
                    for variant in variants {
                        deepest = deepest.max(child(variant.node)?);
                    }
                    deepest + 1
                }
            };
            if node_depth > MAX_CARRIER_DEPTH {
                return Err(DefinitionError::InvalidCarrier("schema too deep"));
            }
            depth.push(node_depth);
        }
        Ok(())
    }
}

fn check_members<'a>(names: impl ExactSizeIterator<Item = &'a str>) -> Result<(), DefinitionError> {
    if names.len() == 0 || names.len() > MAX_CARRIER_MEMBERS {
        return Err(DefinitionError::InvalidCarrier(
            "member count out of bounds",
        ));
    }
    let mut seen = BTreeSet::new();
    for name in names {
        check_name("member name", name)?;
        if !seen.insert(name) {
            return Err(DefinitionError::InvalidCarrier("duplicate member name"));
        }
    }
    Ok(())
}

/// Lookup of published definitions by identity.
pub trait DefinitionLookup {
    /// The descriptor published under `id`, if any.
    fn definition(&self, id: &DefinitionId) -> Option<&DefinitionDescriptorV1>;
}

impl DefinitionLookup for BTreeMap<DefinitionId, DefinitionDescriptorV1> {
    fn definition(&self, id: &DefinitionId) -> Option<&DefinitionDescriptorV1> {
        self.get(id)
    }
}

/// Binding of one active interface's wire ID to its implementation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::BindingV1")]
pub struct BindingV1 {
    /// Instruction wire ID.
    pub wire_id: String,
    /// The active interface that owns the wire ID, so resolving an envelope needs no lookup.
    pub interface: DefinitionId,
    /// Bound implementation.
    pub implementation: DefinitionId,
}

/// Restriction of one definition (`live_runtime_upgrades.md` §7.1).
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::RestrictionV1")]
#[norito(tag = "kind", content = "content")]
pub enum RestrictionV1 {
    /// No new deployments, bindings or dependents since the height.
    Deprecated(u64),
    /// No new work since the height.
    AdmissionClosed(u64),
    /// Only completion capabilities run since the height.
    CompletionOnly(u64),
    /// No execution since the height.
    Disabled(u64),
}

/// One restricted definition.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::RestrictionEntryV1")]
pub struct RestrictionEntryV1 {
    /// Restricted definition.
    pub definition: DefinitionId,
    /// Its restriction.
    pub restriction: RestrictionV1,
}

/// Definitions a live obligation needs to complete (`live_runtime_upgrades.md` §7.2).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::CompletionClosureV1")]
pub struct CompletionClosureV1 {
    /// Identity of the obligation registered through the kernel.
    #[norito(json = "crate::json_helpers::fixed_bytes_hex")]
    pub obligation: [u8; 32],
    /// Definitions its completion needs, sorted and unique.
    pub definitions: Vec<DefinitionId>,
}

/// The one resolved selection an execution root executes at a height.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::runtime::definition::RootSelectionV1")]
pub struct RootSelectionV1 {
    /// Kernel definition.
    pub kernel: DefinitionId,
    /// Commitment profile definition.
    pub commitment_profile: DefinitionId,
    /// Execution services profile shared by every implementation of the root.
    pub services_profile: DefinitionId,
    /// Installed storage definitions, sorted and unique.
    pub installed_storage: Vec<DefinitionId>,
    /// Active interfaces, implementations, events, permissions, parameters, host profiles and
    /// migrations, sorted and unique.
    pub active: Vec<DefinitionId>,
    /// Bindings, sorted by unique wire ID.
    pub bindings: Vec<BindingV1>,
    /// Restrictions, sorted by unique definition.
    pub restrictions: Vec<RestrictionEntryV1>,
    /// Completion closures of live obligations, sorted by unique obligation.
    pub completion_closures: Vec<CompletionClosureV1>,
}

/// A selection that breaks the closure rules.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SelectionError {
    /// A list is unsorted or repeated.
    #[error("{0} must be sorted and unique")]
    NonCanonical(&'static str),
    /// A referenced definition is not published.
    #[error("definition {0} is not published")]
    Unpublished(DefinitionId),
    /// A definition has the wrong kind for where it is used.
    #[error("definition {id} must be a {expected:?} definition")]
    WrongKind {
        /// The definition.
        id: DefinitionId,
        /// The kind it must have.
        expected: DefinitionKind,
    },
    /// A definition cannot be in the active set.
    #[error("definition {0} has a kind that is not activated through the active set")]
    NotActivatable(DefinitionId),
    /// A dependency is neither active nor installed.
    #[error("definition {dependent} depends on {missing}, which the selection does not contain")]
    MissingDependency {
        /// The definition that depends on it.
        dependent: DefinitionId,
        /// The missing dependency.
        missing: DefinitionId,
    },
    /// An implementation relies on another services profile than the root's.
    #[error("definition {0} relies on a services profile other than the root's")]
    ForeignServices(DefinitionId),
    /// Two active interfaces share a wire ID.
    #[error("two active interfaces share wire ID {0}")]
    DuplicateWireId(String),
    /// Two installed storage definitions share a namespace.
    #[error("two installed storage definitions share namespace {0}")]
    DuplicateNamespace(String),
    /// An active interface has no binding.
    #[error("active interface {0} has no binding")]
    Unbound(String),
    /// A binding does not resolve to an active implementation of an active interface.
    #[error("binding of {0} does not resolve to an active implementation of that interface")]
    BadBinding(String),
    /// A restriction or completion closure names a definition outside the selection.
    #[error("definition {0} is restricted or retained but not in the selection")]
    Outside(DefinitionId),
    /// The selection cannot be encoded canonically.
    #[error("selection encoding failed: {0}")]
    Encoding(String),
}

impl RootSelectionV1 {
    /// Content-addressed identity: `H(SELECTION_ID_DOMAIN ‖ canonical bytes)`.
    ///
    /// # Errors
    /// The selection cannot be encoded.
    pub fn id(&self) -> Result<SelectionId, SelectionError> {
        let bytes = norito::encode_canonical(self)
            .map_err(|error| SelectionError::Encoding(error.to_string()))?;
        Ok(SelectionId(
            Hash::new_from_chunks(&[SELECTION_ID_DOMAIN, &bytes]).into(),
        ))
    }

    /// Check the closure rules against the published definitions.
    ///
    /// # Errors
    /// The first rule the selection violates.
    pub fn check_closure(&self, lookup: &impl DefinitionLookup) -> Result<(), SelectionError> {
        let canonical = |field, ordered: bool| {
            if ordered {
                Ok(())
            } else {
                Err(SelectionError::NonCanonical(field))
            }
        };
        canonical(
            "installed_storage",
            strictly_increasing(&self.installed_storage),
        )?;
        canonical("active", strictly_increasing(&self.active))?;
        canonical(
            "bindings",
            self.bindings
                .windows(2)
                .all(|pair| pair[0].wire_id < pair[1].wire_id),
        )?;
        canonical(
            "restrictions",
            self.restrictions
                .windows(2)
                .all(|pair| pair[0].definition < pair[1].definition),
        )?;
        canonical(
            "completion_closures",
            self.completion_closures
                .windows(2)
                .all(|pair| pair[0].obligation < pair[1].obligation),
        )?;

        let typed = |id: &DefinitionId, expected: DefinitionKind| {
            let descriptor = lookup
                .definition(id)
                .ok_or(SelectionError::Unpublished(*id))?;
            if descriptor.kind() == expected {
                Ok(descriptor)
            } else {
                Err(SelectionError::WrongKind { id: *id, expected })
            }
        };
        typed(&self.kernel, DefinitionKind::Kernel)?;
        typed(&self.commitment_profile, DefinitionKind::CommitmentProfile)?;
        typed(&self.services_profile, DefinitionKind::ServicesProfile)?;
        let mut namespaces = BTreeSet::new();
        for storage in &self.installed_storage {
            if let DefinitionDescriptorV1::Storage(descriptor) =
                typed(storage, DefinitionKind::Storage)?
                && !namespaces.insert(descriptor.namespace.as_str())
            {
                return Err(SelectionError::DuplicateNamespace(
                    descriptor.namespace.clone(),
                ));
            }
        }

        let installed: BTreeSet<&DefinitionId> = self.installed_storage.iter().collect();
        let active: BTreeSet<&DefinitionId> = self.active.iter().collect();
        let contains = |id: &DefinitionId| {
            installed.contains(id) || active.contains(id) || *id == self.services_profile
        };

        let mut interfaces = BTreeMap::new();
        for id in &self.active {
            let descriptor = lookup
                .definition(id)
                .ok_or(SelectionError::Unpublished(*id))?;
            match descriptor {
                DefinitionDescriptorV1::Storage(_)
                | DefinitionDescriptorV1::ServicesProfile(_)
                | DefinitionDescriptorV1::CommitmentProfile(_)
                | DefinitionDescriptorV1::Kernel(_) => {
                    return Err(SelectionError::NotActivatable(*id));
                }
                DefinitionDescriptorV1::Interface(interface) => {
                    if interfaces.insert(interface.wire_id.as_str(), *id).is_some() {
                        return Err(SelectionError::DuplicateWireId(interface.wire_id.clone()));
                    }
                }
                DefinitionDescriptorV1::Implementation(implementation)
                    if implementation.services_profile != self.services_profile =>
                {
                    return Err(SelectionError::ForeignServices(*id));
                }
                DefinitionDescriptorV1::Migration(migration)
                    if migration.services_profile != self.services_profile =>
                {
                    return Err(SelectionError::ForeignServices(*id));
                }
                _ => {}
            }
            for (kind, dependency) in descriptor.typed_dependencies() {
                typed(&dependency, kind)?;
                if !contains(&dependency) {
                    return Err(SelectionError::MissingDependency {
                        dependent: *id,
                        missing: dependency,
                    });
                }
            }
        }

        for binding in &self.bindings {
            let interface = interfaces
                .get(binding.wire_id.as_str())
                .ok_or_else(|| SelectionError::BadBinding(binding.wire_id.clone()))?;
            let resolves = binding.interface == *interface
                && active.contains(&binding.implementation)
                && matches!(
                    lookup.definition(&binding.implementation),
                    Some(DefinitionDescriptorV1::Implementation(ImplementationDescriptorV1 {
                        entrypoint: EntrypointV1::Instructions(targets),
                        ..
                    })) if targets.binary_search(interface).is_ok()
                );
            if !resolves {
                return Err(SelectionError::BadBinding(binding.wire_id.clone()));
            }
        }
        if let Some(unbound) = interfaces.keys().find(|wire_id| {
            self.bindings
                .binary_search_by(|binding| binding.wire_id.as_str().cmp(**wire_id))
                .is_err()
        }) {
            return Err(SelectionError::Unbound((*unbound).to_owned()));
        }

        for entry in &self.restrictions {
            if !contains(&entry.definition) {
                return Err(SelectionError::Outside(entry.definition));
            }
        }
        for closure in &self.completion_closures {
            if !strictly_increasing(&closure.definitions) {
                return Err(SelectionError::NonCanonical(
                    "completion closure definitions",
                ));
            }
            if let Some(outside) = closure.definitions.iter().find(|id| !contains(id)) {
                return Err(SelectionError::Outside(*outside));
            }
        }
        Ok(())
    }
}

fn strictly_increasing(ids: &[DefinitionId]) -> bool {
    ids.windows(2).all(|pair| pair[0] < pair[1])
}
