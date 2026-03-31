//! Iroha Data Model contains structures for Domains, Peers, Accounts and Assets with simple,
//! non-specific functions like serialization.
//!
//! ## Note about IVM and deserialization
//! Some structs perform validation during deserialization
//! (e.g. `transaction::candidate::SignedTransactionCandidate`).
//! However, when targeting the Iroha Virtual Machine (IVM), this validation
//! is disabled. Validation inside the IVM is not necessary
//! because it has already been performed on the host side,
//! which is a trusted entity.
//! This gives about 50% performance boost, see #4995.

#![allow(unexpected_cfgs)]
#![allow(semicolon_in_expressions_from_macros)]

#[allow(unused_extern_crates)]
extern crate bech32;
#[allow(unused_extern_crates)]
extern crate self as iroha_data_model;
// NOTE: Documentation coverage is enforced at the workspace level. If a
// module lacks coverage, add targeted documentation at the module boundary
// rather than silencing the lint at the crate root.

pub use iroha_crypto::PublicKey;
pub use iroha_data_model_derive::model;
pub use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
pub use norito::json::{JsonDeserialize, JsonSerialize};
#[cfg(feature = "json")]
pub use norito_derive::{
    FastJson as DeriveFastJson, FastJsonWrite as DeriveFastJsonWrite,
    JsonDeserialize as DeriveJsonDeserialize, JsonSerialize as DeriveJsonSerialize,
};

use crate::name::Name;

/// Data model compatibility version for SDK and node handshakes.
pub const DATA_MODEL_VERSION: u32 = 1;

#[macro_use]
mod id_macros;

// NOTE: `iroha_ffi` is an optional dependency used only when the `ffi_export`
// or `ffi_import` features are enabled. Many types in this crate previously
// applied the `ffi_type` attribute unconditionally, which forced Cargo to build
// the heavy `iroha_ffi` procedural macros even when FFI support was not
// required. To avoid long build times and hangs, all usages of `ffi_type` are
// now wrapped in `cfg_attr` so that the attribute is only expanded when one of
// the FFI features is explicitly activated.

pub mod account;
/// Account domain model types and queries.
pub mod alias;
/// Asset definitions, instances, and utilities.
pub mod asset;
pub use asset::{AssetDefinitionId, AssetId};
/// Block-level data structures and helpers.
pub mod block;
/// Shared primitives reused across data model modules.
pub mod common;
/// Compute lane requests, manifests, and receipts.
pub mod compute;
/// Confidential registries and parameter descriptors.
pub mod confidential;
/// Consensus-related messages and state representations.
pub mod consensus;
/// Static content hosting records.
pub mod content;
/// Data availability ingest, manifest, and governance types.
pub mod da;
/// Domain metadata and registration structures.
pub mod domain;
/// Common error types surfaced by the data model.
pub mod error;
/// Canonical error codes and structured contexts surfaced by AMX/DA/settlement.
pub mod errors;
/// Event payloads emitted by the ledger.
pub mod events;
/// Executor configuration and API types.
pub mod executor;
/// FASTPQ-specific transcripts shared between host and prover.
pub mod fastpq;
/// Fraud detection and risk scoring data types.
pub mod fraud;
/// Hijiri reputation system data types.
pub mod hijiri;
/// Identifier newtypes and supporting helpers.
pub mod id;
/// Hidden-function-backed identifier policy and claim types.
pub mod identifier;
/// IPFS helper types used for off-chain references.
pub mod ipfs;
/// Instruction-set interface (ISI) data types.
pub mod isi;
mod json_helpers;
/// Jurisdiction Data Guardian attestations and committee types.
pub mod jurisdiction;
/// Kaigi session descriptors and billing profile definitions.
pub mod kaigi;
/// Log-level and severity utilities.
pub mod level;
/// Merge-ledger data structures.
pub mod merge;
/// Generic metadata containers and helpers.
pub mod metadata;
/// Ministry transparency/governance payload types.
pub mod ministry;
/// Name parsing and validation utilities.
pub mod name;
/// Nexus-lane scaffolding and identifiers.
pub mod nexus;
/// Non-fungible token structures and specs.
pub mod nft;
/// Offline allowance commitments, certificates, and transfer proofs.
pub mod offline;
/// Oracle feed schemas and deterministic committee helpers.
pub mod oracle;
/// Runtime parameter definitions and schema.
pub mod parameter;
/// Peer and network-topology types.
pub mod peer;
/// Permission token and grant model.
pub mod permission;
/// Petal stream framing for offline payload handoff.
pub mod petal_stream;
/// Zero-knowledge proof payload types
pub mod proof;
/// QR stream framing types for offline payload handoff.
pub mod qr_stream;
/// Query builders, predicates, and parameter types.
pub mod query;
/// Generic hidden-program RAM-LFE program policies and receipts.
pub mod ram_lfe;
/// Repo agreement descriptors and governance knobs.
pub mod repo;
/// Role-based access control definitions.
pub mod role;
/// Runtime upgrade payloads and manifests.
pub mod runtime;
/// Real-world asset lot structures and controls.
pub mod rwa;
/// Smart contract descriptors and execution metadata.
pub mod smart_contract;
/// Sora Name Service registrar data structures.
pub mod sns;
/// Viral social incentive records and escrows.
pub mod social;
/// SoraCloud manifests for deterministic service hosting.
pub mod soracloud;
/// SoraFS data structures (pin registry, manifests).
pub mod sorafs;
/// Strict `sorafs://...` URI literals.
pub mod sorafs_uri;
/// SoraNet transport and privacy data model extensions.
pub mod soranet;
/// World state snapshot representations.
pub mod state;
/// Subscription metadata schemas for trigger-based billing.
pub mod subscription;
/// Taikai broadcast metadata and segment envelope types.
pub mod taikai;
/// Transaction structures, payloads, and signatures.
pub mod transaction;
/// Extended transaction responses for Torii/SDK integrations.
pub mod transactions;
/// Trigger definitions and scheduling utilities.
pub mod trigger;
/// Validation failure diagnostics surfaced to clients.
/// Permission tokens and helpers related to validators.
pub mod validator;
/// Verification helper traits and host bindings.
pub mod verification;
/// Visitor traits for traversing data-model structures.
pub mod visit;

/// SoraDNS attestation and directory data structures.
pub mod soradns;
/// Zero-knowledge proof payload types.
pub mod zk;

#[cfg(feature = "json")]
mod json_key_codec;

/// Bridge-related data types.
pub mod bridge;

/// Governance-related data types (feature-gated)
#[cfg(feature = "governance")]
pub mod governance;
/// Test fixtures exposed for SDK/guardrail consumers.
#[cfg(any(test, feature = "test-fixtures"))]
pub mod testing;
/// Helpers for constructing and accessing instruction registries used by the IVM.
pub mod instruction_registry {
    pub use crate::isi::{InstructionRegistry, registry::default};
}

/// Build-time constants generated during the build process (e.g., keyword
/// tables). Not part of the public API surface.
mod build_consts {
    include!(concat!(env!("OUT_DIR"), "/build_consts.rs"));
}

pub use build_consts::PRECOMPUTED_KEYWORDS;

// Include API version.
#[cfg(feature = "transparent_api")]
include!(concat!(env!("CARGO_MANIFEST_DIR"), "/transparent_api.rs"));

#[cfg(not(feature = "transparent_api"))]
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/non_transparent_api.rs"
));

// Slice-based Norito decoders for model types used in packed sequences and
// options. These forward to the archived Norito representation to avoid
// duplicating decoding logic.
mod norito_slice_decode;

/// Private module defining sealing traits for `iroha_data_model`.
mod seal {
    /// Seals the [`Instruction`](crate::isi::Instruction) trait.
    pub trait Instruction {}

    /// Seals the [`SingularQuery`](crate::query::SingularQuery) trait.
    pub trait SingularQuery {}

    /// Seals the [`crate::query::Query`](crate::query::Query) trait.
    pub trait Query {}
}

pub use error::{EnumTryAsError, ParseError};
pub use errors::{
    AmxStage, CanonicalError, CanonicalErrorKind, CircuitBreakerKind, SettlementRouterOutage,
};
pub use executor::ValidationFail;
pub use id::{ChainId, IdBox};
pub use level::Level;
pub use nexus::{DataSpaceId, LaneId};

/// Uniquely identifiable entity ([`domain::Domain`], [`account::Account`], etc.).
/// This trait should always be derived with `IdEqOrdHash`.
pub trait Identifiable: Ord + Eq {
    /// Type of the entity identifier
    type Id: Into<IdBox> + Ord + Eq + core::hash::Hash;

    /// Get reference to the type identifier
    fn id(&self) -> &Self::Id;
}

/// Trait for proxy objects used for registration.
pub trait Registrable {
    /// Constructed type
    type Target;

    /// Construct [`Self::Target`] with given authority
    fn build(self, authority: &crate::account::AccountId) -> Self::Target;
}

/// Trait that marks the entity as having metadata.
pub trait HasMetadata {
    // type Metadata = metadata::Metadata;
    // Uncomment when stable.

    /// The metadata associated to this object.
    fn metadata(&self) -> &metadata::Metadata;
}

/// Trait for objects that are registered by proxy.
pub trait Registered: Identifiable {
    /// The proxy type that is used to register this entity. Usually
    /// `Self`, but if you have a complex structure where most fields
    /// would be empty, to save space you create a builder for it, and
    /// set `With` to the builder's type.
    type With;
}

/// Auxiliary trait for objects which are stored in parts in `World`.
/// E.g. `Account` is stored as `AccountId`+`AccountEntry` in `World::accounts`
pub trait IntoKeyValue {
    /// Object ID
    type Key;
    /// Object data
    type Value;
    /// Method to split object into parts
    fn into_key_value(self) -> (Self::Key, Self::Value);
}

mod ffi {
    //! Definitions and implementations of FFI related functionalities

    #[cfg(any(feature = "ffi_export", feature = "ffi_import"))]
    use super::*;

    #[cfg(any(feature = "ffi_export", feature = "ffi_import"))]
    iroha_ffi::handles! {
        account::Account,
        asset::value::Asset,
        domain::Domain,
        metadata::Metadata,
        permission::Permission,
        role::Role,
    }

    #[cfg(feature = "ffi_import")]
    iroha_ffi::decl_ffi_fns! { link_prefix="iroha_data_model" Drop, Clone, Eq, Ord }
    #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
    iroha_ffi::def_ffi_fns! { link_prefix="iroha_data_model"
        Drop: { account::Account, asset::value::Asset, domain::Domain, metadata::Metadata, permission::Permission, role::Role },
        Clone: { account::Account, asset::value::Asset, domain::Domain, metadata::Metadata, permission::Permission, role::Role },
        Eq: { account::Account, asset::value::Asset, domain::Domain, metadata::Metadata, permission::Permission, role::Role },
        Ord: { account::Account, asset::value::Asset, domain::Domain, metadata::Metadata, permission::Permission, role::Role },
    }

    // NOTE: Makes sure that only one `dealloc` is exported per generated dynamic library
    #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
    mod dylib {
        use std::alloc;

        iroha_ffi::def_ffi_fns! {dealloc}
    }
}

#[allow(ambiguous_glob_reexports)]
pub mod prelude {
    //! Prelude: re-export of most commonly used traits, structs and macros in this crate.
    pub use iroha_crypto::{
        Algorithm, ExposedPrivateKey, Hash, HashOf, KeyPair, MerkleTree, PrivateKey, PublicKey,
        Signature, SignatureOf,
    };
    pub use iroha_primitives::{
        json::Json,
        numeric::{Numeric, NumericSpec, numeric},
    };

    pub use super::{
        ChainId, Decode, Encode, HasMetadata, IdBox, Identifiable, Level, Registrable,
        ValidationFail,
        account::prelude::*,
        asset::prelude::*,
        block::prelude::*,
        bridge::*,
        confidential::prelude::*,
        content::prelude::*,
        domain::prelude::*,
        error::EnumTryAsError,
        events::prelude::*,
        executor::prelude::*,
        fastpq::*,
        identifier::prelude::*,
        ipfs::IpfsPath,
        isi::prelude::*,
        kaigi::prelude::*,
        metadata::prelude::*,
        name::prelude::*,
        nexus::{
            DataSpaceCatalog, DataSpaceCatalogError, DataSpaceId, DataSpaceMetadata,
            DomainCommittee, DomainEndorsement, DomainEndorsementPolicy, DomainEndorsementScope,
            DomainEndorsementSignature, LaneCatalog, LaneCatalogError, LaneConfig, LaneId,
            LaneIdError, LaneLifecyclePlan, LaneRelayEnvelope, LaneRelayEnvelopeRef,
            LaneStorageProfile, LaneStorageProfileParseError, LaneVisibility,
            LaneVisibilityParseError, VerifiedLaneRelayRecord,
        },
        nft::prelude::*,
        parameter::prelude::*,
        peer::prelude::*,
        permission::prelude::*,
        query::prelude::*,
        ram_lfe::prelude::*,
        repo::prelude::*,
        role::prelude::*,
        rwa::prelude::*,
        smart_contract::prelude::*,
        sns::prelude::*,
        social::prelude::*,
        sorafs_uri::SorafsUri,
        subscription::prelude::*,
        transaction::prelude::*,
        trigger::prelude::*,
    };
}
