//! Contract manifests and trigger descriptors of C10 contracts.
//!
//! A manifest's public closure reaches a transaction status through its entrypoints' trigger
//! declarations (`ContractManifest` → `EntrypointDescriptor` → `TriggerDescriptor` →
//! `EventFilterBox` → pipeline transaction filter → `TransactionStatus`). C10 changed that status
//! (its rejection family), so the three enclosing types are new definitions here. Every other
//! member is the baseline manifest's own type, field for field and in the same order, so a C10
//! manifest differs from a baseline manifest only where a trigger filter names a C10 status.

use iroha_crypto::Hash;
use iroha_model_base::metadata::Metadata;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::{string::String, vec::Vec};

use super::events::EventFilterBoxC10V1;
use crate::{
    DeriveFastJson as DeriveFast, DeriveJsonDeserialize as DeriveJsonDe,
    DeriveJsonSerialize as DeriveJsonSer,
    account::AccountId,
    smart_contract::{
        entrypoint::{EntrypointArgumentSchemaV1, EntrypointValueTypeV1},
        manifest::{
            AccessSetHints, ContractEnumTypeDescriptorV1, ContractErrorMessage,
            ContractErrorTypeDescriptor, ContractEventDescriptorV1, ContractPermissionDescriptorV1,
            EntryPointKind, EntrypointAuthorizationV1, EntrypointParamDescriptor,
            KotobaTranslationEntry, ManifestProvenance, StateDescriptor, TriggerCallback,
        },
    },
    trigger::{TriggerId, action::Repeats},
};

/// Declarative trigger metadata attached to a C10 entrypoint.
///
/// The baseline `TriggerDescriptor` with a C10 event filter.
#[derive(
    Debug,
    Clone,
    Encode,
    Decode,
    IntoSchema,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    DeriveFast,
    DeriveJsonSer,
    DeriveJsonDe,
    norito::NoritoSchema,
)]
#[norito(no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::ivm_c10::TriggerDescriptorC10V1")]
pub struct TriggerDescriptorC10V1 {
    /// Trigger identifier.
    pub id: TriggerId,
    /// Repeat policy for the trigger action.
    pub repeats: Repeats,
    /// Event filter that drives execution, with the C10 status family.
    pub filter: EventFilterBoxC10V1,
    /// Optional explicit authority override.
    #[norito(default)]
    pub authority: Option<AccountId>,
    /// Trigger metadata payload (JSON map).
    #[norito(default)]
    pub metadata: Metadata,
    /// Callback target for this trigger.
    pub callback: TriggerCallback,
}

/// Declarative metadata for a compiled C10 entrypoint.
///
/// The baseline `EntrypointDescriptor` with C10 trigger descriptors.
#[derive(
    Debug,
    Clone,
    Encode,
    Decode,
    IntoSchema,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    DeriveFast,
    DeriveJsonSer,
    DeriveJsonDe,
    norito::NoritoSchema,
)]
#[norito(no_fast_from_json)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::ivm_c10::EntrypointDescriptorC10V1")]
pub struct EntrypointDescriptorC10V1 {
    /// Symbol name as declared in the Kotodama source file.
    pub name: String,
    /// Logical kind of the entrypoint.
    pub kind: EntryPointKind,
    /// Ordered public parameters advertised by the compiler.
    #[norito(default)]
    pub params: Vec<EntrypointParamDescriptor>,
    /// Exact recursive schema for the complete public argument record.
    #[norito(default)]
    pub argument_schema: Option<EntrypointArgumentSchemaV1>,
    /// Exact return type, including `()` when the source omits its return annotation.
    #[norito(default)]
    pub return_type: Option<String>,
    /// Exact recursive schema for every public return value.
    #[norito(default)]
    pub return_schema: Option<EntrypointValueTypeV1>,
    /// Explicit dispatcher authorization authenticated by the artifact.
    pub authorization: EntrypointAuthorizationV1,
    /// Advisory read keys for this entrypoint (flattened `state:...` strings).
    #[norito(default)]
    pub read_keys: Vec<String>,
    /// Advisory write keys for this entrypoint.
    #[norito(default)]
    pub write_keys: Vec<String>,
    /// Whether access-set hints are complete or explicitly provided.
    #[norito(default)]
    pub access_hints_complete: Option<bool>,
    /// Reasons access hints were skipped for this entrypoint.
    #[norito(default)]
    pub access_hints_skipped: Vec<String>,
    /// Trigger declarations that call this entrypoint.
    #[norito(default)]
    pub triggers: Vec<TriggerDescriptorC10V1>,
}

/// Manifest of a C10 contract.
///
/// The baseline `ContractManifest` with C10 entrypoint descriptors. Its `abi_hash` names a C10
/// host profile and its `features_bitmap` is the C10 artifact's capability bitmap (the ZK bit
/// only).
#[derive(
    Debug,
    Clone,
    Encode,
    Decode,
    IntoSchema,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    DeriveFast,
    DeriveJsonSer,
    DeriveJsonDe,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito(no_fast_from_json)]
#[norito_schema(name = "iroha_data_model::ivm_c10::ContractManifestC10V1")]
pub struct ContractManifestC10V1 {
    /// Canonical source-level seiyaku name embedded by the compiler.
    #[norito(default)]
    pub seiyaku_name: Option<String>,
    /// Content-addressed hash of the compiled `.to` bytecode.
    pub code_hash: Option<Hash>,
    /// ABI hash of the C10 host profile the artifact binds.
    pub abi_hash: Option<Hash>,
    /// Optional compiler fingerprint.
    pub compiler_fingerprint: Option<String>,
    /// Compiler-derived, hash-covered execution capability bitmap.
    pub features_bitmap: Option<u64>,
    /// Optional advisory access-set hints for the scheduler.
    #[norito(default)]
    pub access_set_hints: Option<AccessSetHints>,
    /// Sorted, unique permission declarations authenticated by the artifact.
    pub permissions: Vec<ContractPermissionDescriptorV1>,
    /// Sorted, unique source event declarations authenticated by the artifact.
    pub events: Vec<ContractEventDescriptorV1>,
    /// Optional entrypoint descriptors advertised by the compiler.
    #[norito(default)]
    pub entrypoints: Option<Vec<EntrypointDescriptorC10V1>>,
    /// Optional durable state schema advertised by the compiler.
    #[norito(default)]
    pub states: Option<Vec<StateDescriptor>>,
    /// Exact nominal error type identities and variant schemas advertised by the compiler.
    #[norito(default)]
    pub error_types: Option<Vec<ContractErrorTypeDescriptor>>,
    /// Complete ordinary enum declaration inventory, sorted by nominal identity.
    pub enum_types: Vec<ContractEnumTypeDescriptorV1>,
    /// Authenticated presentation text, separate from nominal error schemas.
    #[norito(default)]
    pub error_messages: Option<Vec<ContractErrorMessage>>,
    /// Optional localization tables extracted from `kotoba { ... }` blocks.
    #[norito(default)]
    pub kotoba: Option<Vec<KotobaTranslationEntry>>,
    /// Provenance metadata for the manifest, including signer and signature.
    #[norito(default)]
    pub provenance: Option<ManifestProvenance>,
}

mod signing;
pub use signing::ManifestSignaturePayloadC10V1;

/// Opt-in representation of a registered C10 manifest on the signed contract-artifact route.
pub const REGISTERED_MANIFEST_MEDIA_TYPE: &str = "application/vnd.iroha.c10-manifest.v1+json";

/// Registered manifest and captured host from the same network and data space.
#[derive(Debug, Clone, PartialEq, Eq, DeriveJsonSer, DeriveJsonDe)]
#[norito(deny_unknown_fields)]
pub struct RegisteredManifestC10V1 {
    /// Network whose registry was queried.
    pub network_id: crate::NetworkId,
    /// Exact data space and code hash used for the read.
    pub artifact_id: crate::smart_contract::ContractArtifactId,
    /// Host definition captured when the manifest was registered.
    pub host_profile: crate::runtime::definition::DefinitionId,
    /// Signed content stored under the selected C10 interface.
    pub manifest: ContractManifestC10V1,
}
