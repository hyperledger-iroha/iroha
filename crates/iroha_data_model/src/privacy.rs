//! Canonical first-release privacy protocol wire types.
//!
//! The types in this module deliberately form a closed protocol surface.
//! Protocol identities, proof systems, and native verifier engines are separate
//! enums, and proof envelopes must bind all three together with governed
//! parameter, verifier, statement-schema, and engine-manifest digests. There
//! are no free-form identifiers, aliases, or fallback proof variants.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use crate::{AssetDefinitionId, ChainId, account::AccountId};

pub use iroha_zkp_halo2::vega::{
    VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1, VEGA_MDL_BIRTH_RANDOM_BYTES_V1,
    VEGA_MDL_FULL_DATE_TEXT_BYTES_V1, VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1,
    VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1, VEGA_MDL_MAX_PRESENTATION_YEAR_V1,
    VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1, VEGA_MDL_MIN_PRESENTATION_YEAR_V1,
    VEGA_MDL_MSO_PAYLOAD_BYTES_V1, VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1,
};

/// Domain separator used to hash canonical [`PrivacyStatementV1`] values.
pub const PRIVACY_STATEMENT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:statement:v1";
/// Domain separator used to hash canonical [`PrivacyNativeConsensusBindingV1`] values.
pub const PRIVACY_NATIVE_CONSENSUS_BINDING_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:native-consensus-binding:v1";
/// Permanent Norito schema identity for the top-level typed privacy statement.
pub const PRIVACY_STATEMENT_SCHEMA_NAME_V1: &str = "iroha.privacy.statement.v1";
/// Permanent Norito schema identity for the shared native consensus binding.
pub const PRIVACY_NATIVE_CONSENSUS_BINDING_SCHEMA_NAME_V1: &str =
    "iroha.privacy.native-consensus-binding.v1";
/// Permanent Norito schema identity for the top-level typed privacy proof.
pub const PRIVACY_PROOF_SCHEMA_NAME_V1: &str = "iroha.privacy.proof.v1";
/// Permanent Norito schema identity for the cross-SDK privacy proof envelope.
pub const PRIVACY_PROOF_ENVELOPE_SCHEMA_NAME_V1: &str = "iroha.privacy.proof-envelope.v1";
/// Permanent Norito schema identity for the concrete ZK-ACE authorization statement.
pub const ZK_ACE_AUTHORIZATION_STATEMENT_SCHEMA_NAME_V1: &str =
    "iroha.privacy.zk-ace.authorization-statement.v1";
/// Permanent Norito schema identity for authoritative ZK-ACE policy digest material.
pub const ZK_ACE_POLICY_DIGEST_MATERIAL_SCHEMA_NAME_V1: &str =
    "iroha.privacy.zk-ace.policy-digest-material.v1";
/// Domain separator used to hash canonical [`PrivacyRootPublicationV1`] values.
pub const PRIVACY_ROOT_PUBLICATION_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:root-publication:v1";
/// Domain separator used to hash canonical [`PrivacyOrchardPoolBootstrapV1`] values.
pub const PRIVACY_ORCHARD_POOL_BOOTSTRAP_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:orchard-pool-bootstrap:v1";
/// Domain separator for FCMP++, private-IVM, and PQ-MASP pool bootstraps.
pub const PRIVACY_PROOF_MANAGED_POOL_BOOTSTRAP_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:proof-managed-pool-bootstrap:v1";
/// Domain separator for the ledger index of one complete FCMP++ `(O, I, C)` tuple.
///
/// This identifier is never substituted for the tuple in the curve tree or
/// membership relation.
pub const PRIVACY_FCMP_OUTPUT_ID_DOMAIN_V1: &[u8] =
    b"iroha.privacy.monero-fcmp-plus-plus.output-id.v1";
/// Domain separator for the shared root-history commitment to a typed FCMP++
/// Selene/Helios root.
pub const PRIVACY_FCMP_ROOT_COMMITMENT_DOMAIN_V1: &[u8] =
    b"iroha.privacy.monero-fcmp-plus-plus.root-commitment.v1";
/// Domain separator used to hash canonical [`PrivacyPgcAccountBootstrapV1`] payloads.
pub const PRIVACY_PGC_ACCOUNT_BOOTSTRAP_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:pgc-account-bootstrap:v1";
/// Domain separator used to hash canonical Anonymous PGC bootstrap proof bytes.
pub const PRIVACY_PGC_BOOTSTRAP_PROOF_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:pgc-bootstrap-proof:v1";
/// Domain separator for core's deterministic PGC account-state root derivation.
pub const PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha:privacy:pgc-account-state-root:v1";
/// Domain separator for the exact private-IVM action selected by a statement.
pub const IVM_PRIVATE_NOTE_ACTION_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:ivm-private-note:action:v1";
/// Domain separator for canonical Bootle/Lantern issuer-policy record digests.
pub const BOOTLE_LANTERN_ISSUER_POLICY_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:bootle-lantern:issuer-policy:v1";
/// Domain separator for the exact Bootle/Lantern issuer verification matrix.
pub const BOOTLE_LANTERN_ISSUER_PARAMETER_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:bootle-lantern:issuer-parameter:v1";
/// Domain separator for canonical Vega issuer-key/policy revision self-digests.
pub const VEGA_ISSUER_RECORD_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:vega:issuer-record:v1";
/// Domain for canonical length-delimited Vega issuer-record hashing.
pub const VEGA_ISSUER_RECORD_HASH_FRAME_DOMAIN_V1: &[u8] =
    b"iroha.privacy.vega.issuer-record.sha256-frame.v1";
/// Implicit version committed by every Vega issuer governance record.
pub const VEGA_ISSUER_GOVERNANCE_RECORD_VERSION_V1: u16 = 1;
/// Domain separator for canonical ZK-AMS Personhood Credential hashes.
pub const ZK_AMS_PHC_HASH_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-ams:phc:v1";
/// Domain separator for canonical ZK-AMS issuer-policy records.
pub const ZK_AMS_ISSUER_POLICY_RECORD_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-ams:issuer-policy-record:v1";
/// Domain separator for canonical ZK-AMS registry-snapshot records.
pub const ZK_AMS_REGISTRY_RECORD_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-ams:registry-record:v1";
/// Domain separator for canonical ZK-AMS registry bootstrap provenance.
pub const ZK_AMS_REGISTRY_BOOTSTRAP_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-ams:registry-bootstrap:v1";
/// Domain separator for canonical authoritative ZK-ACE policy records.
pub const ZK_ACE_POLICY_RECORD_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-ace:policy-record:v1";
/// Domain separator for canonical X.509 trust-anchor revision self-digests.
pub const ZK_X509_TRUST_ANCHOR_RECORD_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:trust-anchor-record:v1";
/// Domain separator for canonical X.509 certificate-policy revision self-digests.
pub const ZK_X509_CERTIFICATE_POLICY_RECORD_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:certificate-policy-record:v1";
/// Domain separator for canonical signed-CRL revision self-digests.
pub const ZK_X509_CRL_RECORD_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:crl-record:v1";
/// Domain for canonical SHA-256 field framing in the X.509 relation.
pub const ZK_X509_HASH_FRAME_DOMAIN_V1: &[u8] = b"iroha.zk-x509.sha256.frame.v1";
/// Domain for the exact signed DER CRL digest.
pub const ZK_X509_CRL_DER_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.crl.der.v1";
/// Domain for the exact CRL issuer-SPKI digest.
pub const ZK_X509_CRL_ISSUER_SPKI_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.crl.issuer-spki.v1";
/// Implicit version committed by every X.509 governance-record digest.
pub const ZK_X509_GOVERNANCE_RECORD_VERSION_V1: u16 = 1;

/// Maximum privacy actions admitted in one Taira transaction.
pub const TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1: u32 = 1;
/// Maximum privacy actions admitted in one Taira block.
pub const TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1: u32 = 2;
/// Maximum proof payload bytes admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1: u32 = 9 * 1024 * 1024;
/// Maximum canonical bytes admitted for one Anonymous PGC bootstrap proof.
pub const TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1: u32 = 4 * 1024 * 1024;
/// The only account-state epoch admitted for an Anonymous PGC bootstrap.
///
/// Successor proofs advance this epoch by exactly one. Keeping the origin
/// fixed prevents governance or a caller from creating ambiguous histories.
pub const PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1: u64 = 1;
/// The sole initial epoch for a node-derived Orchard empty frontier.
pub const PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1: u64 = 1;
/// The only authorization epoch admitted for a new ZK-ACE policy.
pub const PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1: u64 = 1;
/// Maximum number of source accounts in one authoritative ZK-ACE policy.
pub const PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1: usize = 256;
/// Maximum number of authoritative ZK-ACE policy lineages in world state.
pub const PRIVACY_ZK_ACE_MAX_POLICIES_V1: usize = 4_096;
/// Maximum encoded bytes admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_ACTION_BYTES_V1: u32 = 9 * 1024 * 1024;
/// Maximum privacy bytes admitted in one Taira transaction.
pub const TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1: u32 = 9 * 1024 * 1024;
/// Maximum privacy bytes admitted in one Taira block.
pub const TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1: u32 = 18 * 1024 * 1024;
/// Maximum public-statement and encrypted-output payload bytes in one Taira transaction.
pub const TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1: u32 =
    256 * 1024;
/// Maximum nullifiers admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1: u32 = 8;
/// Maximum commitments admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1: u32 = 8;
/// Number of recent privacy roots retained by the Taira first-release profile.
pub const TAIRA_PRIVACY_RETAINED_ROOT_COUNT_V1: u32 = 2_048;
/// Minimum on-chain notice before a privacy-policy tightening becomes effective.
pub const MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1: u64 = 300;

/// Canonical first-release privacy protocol identity.
///
/// Variant order is part of the Norito wire contract. New protocols require a
/// new data-model release; unknown discriminants are rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "protocol", content = "value", deny_unknown_fields)
)]
pub enum PrivacyProtocolIdV1 {
    /// Native ZK-ACE post-quantum authorization protocol v0.
    #[cfg_attr(feature = "json", norito(rename = "zk-ace-pq-authorization-v0"))]
    ZkAcePqAuthorizationV0,
    /// Anonymous PGC k-out-of-n payment protocol v1.
    #[cfg_attr(feature = "json", norito(rename = "anonymous-pgc-k-out-of-n-v1"))]
    AnonymousPgcKOutOfNV1,
    /// `VeRange` transparent range-proof protocol v1.
    #[cfg_attr(feature = "json", norito(rename = "verange-transparent-range-v1"))]
    VeRangeTransparentRangeV1,
    /// Native Iroha ZK-AMS admission and anonymous-account provisioning suite v1.
    #[cfg_attr(feature = "json", norito(rename = "iroha-zk-ams-v1"))]
    IrohaZkAmsV1,
    /// Vega proof over an existing credential v0.
    #[cfg_attr(feature = "json", norito(rename = "vega-existing-credential-zk-v0"))]
    VegaExistingCredentialZkV0,
    /// Native Iroha P-256 X.509 predicate STARK protocol v0.
    #[cfg_attr(feature = "json", norito(rename = "iroha-zk-x509-stark-p256-v0"))]
    IrohaZkX509StarkP256V0,
    /// Native Iroha Jindo batched univariate lattice polynomial-commitment protocol v0.
    #[cfg_attr(
        feature = "json",
        norito(rename = "iroha-jindo-polynomial-commitment-v0")
    )]
    IrohaJindoPolynomialCommitmentV0,
    /// Native Bootle Lantern/LNP22 module-lattice anonymous credential v1.
    #[cfg_attr(feature = "json", norito(rename = "iroha-bootle-lantern-anoncred-v1"))]
    IrohaBootleLanternAnoncredV1,
    /// Orchard Halo2 action protocol v1.
    #[cfg_attr(feature = "json", norito(rename = "orchard-halo2-actions-v1"))]
    OrchardHalo2ActionsV1,
    /// Monero FCMP++ full-chain membership protocol v1.
    #[cfg_attr(feature = "json", norito(rename = "monero-fcmp-plus-plus-v1"))]
    MoneroFcmpPlusPlusV1,
    /// Native IVM private-note STARK protocol v1.
    #[cfg_attr(feature = "json", norito(rename = "iroha-ivm-private-note-stark-v1"))]
    IrohaIvmPrivateNoteStarkV1,
    /// Post-quantum MASP STARK protocol v0.
    #[cfg_attr(feature = "json", norito(rename = "pq-masp-stark-v0"))]
    PqMaspStarkV0,
}

/// Frozen protocol labels retired before the first-release privacy registry.
///
/// These identifiers remain reserved so generic proof systems, SDKs, and
/// governance tooling cannot reuse them for a different protocol. Order is
/// the canonical order carried by `fixtures/privacy/exact12_v1.tsv`.
pub const PRIVACY_RETIRED_PROTOCOL_LABELS_V1: [&str; 10] = [
    "zkat-policy-private-auth-v1",
    "zk-ams-recursive-admission-v0",
    "silent-threshold-anoncred-v0",
    "zk-x509-onchain-identity-v0",
    "jindo-lattice-pcs-zk-v0",
    "sis-hints-anoncred-pq-v0",
    "sis-with-hints",
    "penumbra-masp-v1",
    "miden-stark-note-v1",
    "aztec-private-rollup-v1",
];

impl PrivacyProtocolIdV1 {
    /// Number of protocols in the closed first-release registry.
    pub const COUNT: usize = 12;

    /// Every protocol in canonical Norito discriminant order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::ZkAcePqAuthorizationV0,
        Self::AnonymousPgcKOutOfNV1,
        Self::VeRangeTransparentRangeV1,
        Self::IrohaZkAmsV1,
        Self::VegaExistingCredentialZkV0,
        Self::IrohaZkX509StarkP256V0,
        Self::IrohaJindoPolynomialCommitmentV0,
        Self::IrohaBootleLanternAnoncredV1,
        Self::OrchardHalo2ActionsV1,
        Self::MoneroFcmpPlusPlusV1,
        Self::IrohaIvmPrivateNoteStarkV1,
        Self::PqMaspStarkV0,
    ];

    /// Exact external identifier used by SDK catalogs, governance tooling, and
    /// the BOI Privacy Lab.
    ///
    /// These labels are part of the first-release contract. Callers must not
    /// trim, case-fold, normalize, or accept aliases for them.
    #[must_use]
    pub const fn canonical_label(self) -> &'static str {
        match self {
            Self::ZkAcePqAuthorizationV0 => "zk-ace-pq-authorization-v0",
            Self::AnonymousPgcKOutOfNV1 => "anonymous-pgc-k-out-of-n-v1",
            Self::VeRangeTransparentRangeV1 => "verange-transparent-range-v1",
            Self::IrohaZkAmsV1 => "iroha-zk-ams-v1",
            Self::VegaExistingCredentialZkV0 => "vega-existing-credential-zk-v0",
            Self::IrohaZkX509StarkP256V0 => "iroha-zk-x509-stark-p256-v0",
            Self::IrohaJindoPolynomialCommitmentV0 => "iroha-jindo-polynomial-commitment-v0",
            Self::IrohaBootleLanternAnoncredV1 => "iroha-bootle-lantern-anoncred-v1",
            Self::OrchardHalo2ActionsV1 => "orchard-halo2-actions-v1",
            Self::MoneroFcmpPlusPlusV1 => "monero-fcmp-plus-plus-v1",
            Self::IrohaIvmPrivateNoteStarkV1 => "iroha-ivm-private-note-stark-v1",
            Self::PqMaspStarkV0 => "pq-masp-stark-v0",
        }
    }

    /// Exact Norito statement/proof variant label carried by the first-release
    /// cross-SDK matrix.
    ///
    /// Statement and proof envelopes deliberately use the same closed variant
    /// name for each protocol. Keeping this mapping next to the canonical
    /// protocol identifier lets release tooling validate the complete matrix
    /// without maintaining a second, drift-prone list.
    #[must_use]
    pub const fn canonical_typed_variant_label(self) -> &'static str {
        match self {
            Self::ZkAcePqAuthorizationV0 => "ZkAcePqAuthorizationV0",
            Self::AnonymousPgcKOutOfNV1 => "AnonymousPgcKOutOfNV1",
            Self::VeRangeTransparentRangeV1 => "VeRangeTransparentRangeV1",
            Self::IrohaZkAmsV1 => "IrohaZkAmsV1",
            Self::VegaExistingCredentialZkV0 => "VegaExistingCredentialZkV0",
            Self::IrohaZkX509StarkP256V0 => "IrohaZkX509StarkP256V0",
            Self::IrohaJindoPolynomialCommitmentV0 => "IrohaJindoPolynomialCommitmentV0",
            Self::IrohaBootleLanternAnoncredV1 => "IrohaBootleLanternAnoncredV1",
            Self::OrchardHalo2ActionsV1 => "OrchardHalo2ActionsV1",
            Self::MoneroFcmpPlusPlusV1 => "MoneroFcmpPlusPlusV1",
            Self::IrohaIvmPrivateNoteStarkV1 => "IrohaIvmPrivateNoteStarkV1",
            Self::PqMaspStarkV0 => "PqMaspStarkV0",
        }
    }

    /// Parse one exact first-release external identifier.
    ///
    /// Returns `None` for aliases, retired identifiers, and non-canonical
    /// spellings.
    #[must_use]
    pub const fn from_canonical_label(label: &str) -> Option<Self> {
        match label.as_bytes() {
            b"zk-ace-pq-authorization-v0" => Some(Self::ZkAcePqAuthorizationV0),
            b"anonymous-pgc-k-out-of-n-v1" => Some(Self::AnonymousPgcKOutOfNV1),
            b"verange-transparent-range-v1" => Some(Self::VeRangeTransparentRangeV1),
            b"iroha-zk-ams-v1" => Some(Self::IrohaZkAmsV1),
            b"vega-existing-credential-zk-v0" => Some(Self::VegaExistingCredentialZkV0),
            b"iroha-zk-x509-stark-p256-v0" => Some(Self::IrohaZkX509StarkP256V0),
            b"iroha-jindo-polynomial-commitment-v0" => Some(Self::IrohaJindoPolynomialCommitmentV0),
            b"iroha-bootle-lantern-anoncred-v1" => Some(Self::IrohaBootleLanternAnoncredV1),
            b"orchard-halo2-actions-v1" => Some(Self::OrchardHalo2ActionsV1),
            b"monero-fcmp-plus-plus-v1" => Some(Self::MoneroFcmpPlusPlusV1),
            b"iroha-ivm-private-note-stark-v1" => Some(Self::IrohaIvmPrivateNoteStarkV1),
            b"pq-masp-stark-v0" => Some(Self::PqMaspStarkV0),
            _ => None,
        }
    }

    /// Exact proof system required by this protocol.
    #[must_use]
    pub const fn expected_proof_system(self) -> PrivacyProofSystemIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV0
            | Self::IrohaZkX509StarkP256V0
            | Self::IrohaIvmPrivateNoteStarkV1
            | Self::PqMaspStarkV0 => PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
            Self::IrohaBootleLanternAnoncredV1 => {
                PrivacyProofSystemIdV1::LanternLnp22ModuleLinearNorm
            }
            Self::IrohaZkAmsV1 => {
                PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512
            }
            Self::AnonymousPgcKOutOfNV1 => PrivacyProofSystemIdV1::AnonymousPgcP256,
            Self::VeRangeTransparentRangeV1 => PrivacyProofSystemIdV1::IrohaVeRangeP256,
            Self::VegaExistingCredentialZkV0 => {
                PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256
            }
            Self::IrohaJindoPolynomialCommitmentV0 => {
                PrivacyProofSystemIdV1::JindoPolynomialCommitment
            }
            Self::OrchardHalo2ActionsV1 => PrivacyProofSystemIdV1::Halo2IpaPasta,
            Self::MoneroFcmpPlusPlusV1 => PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs,
        }
    }

    /// Exact native verifier engine required by this protocol.
    #[must_use]
    pub const fn expected_engine(self) -> PrivacyEngineIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV0
            | Self::IrohaZkX509StarkP256V0
            | Self::IrohaIvmPrivateNoteStarkV1
            | Self::PqMaspStarkV0 => PrivacyEngineIdV1::NativeGoldilocksStarkFri,
            Self::IrohaBootleLanternAnoncredV1 => PrivacyEngineIdV1::NativeLanternLnp22,
            Self::IrohaZkAmsV1 => {
                PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255
            }
            Self::AnonymousPgcKOutOfNV1 => PrivacyEngineIdV1::NativeAnonymousPgcP256,
            Self::VeRangeTransparentRangeV1 => PrivacyEngineIdV1::NativeVeRangeP256,
            Self::VegaExistingCredentialZkV0 => PrivacyEngineIdV1::NativeVega,
            Self::IrohaJindoPolynomialCommitmentV0 => PrivacyEngineIdV1::NativeJindo,
            Self::OrchardHalo2ActionsV1 => PrivacyEngineIdV1::NativeHalo2Orchard,
            Self::MoneroFcmpPlusPlusV1 => PrivacyEngineIdV1::NativeFcmpPlusPlus,
        }
    }
}

#[cfg(any(test, feature = "privacy-exact12-conformance"))]
pub use exact12_fixture::{
    PRIVACY_EXACT12_FIXTURE_BUNDLE_MAX_BYTES_V1, PrivacyExact12FixtureBundleV1,
    PrivacyExact12FixtureBundleValidationStatusV1, PrivacyExact12FixtureErrorV1,
    PrivacyExact12TypedEnvelopeRowV1, PrivacyExact12TypedFixtureRowV1,
    privacy_exact12_fixture_bundle_bytes_v1, privacy_exact12_fixture_bundle_v1,
    privacy_exact12_matrix_bytes_v1, privacy_exact12_typed_envelope_rows_v1,
    validate_privacy_exact12_fixture_bundle_v1,
};

/// Return whether `label` is reserved by the first-release privacy registry.
///
/// Active and retired labels are both exact, case-sensitive reservations.
/// This predicate deliberately performs no trimming, normalization, or alias
/// matching so near-miss identifiers remain available to unrelated protocols.
#[must_use]
pub fn privacy_protocol_label_is_reserved_v1(label: &str) -> bool {
    PrivacyProtocolIdV1::from_canonical_label(label).is_some()
        || PRIVACY_RETIRED_PROTOCOL_LABELS_V1.contains(&label)
}

/// Exact proof-system profile selected by a privacy protocol.
///
/// This is intentionally distinct from [`PrivacyProtocolIdV1`] because several
/// protocols can share one proof system without becoming interchangeable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "proof_system", content = "value", deny_unknown_fields)
)]
pub enum PrivacyProofSystemIdV1 {
    /// STARK/FRI over Goldilocks with SHA-256 transcript and commitments.
    #[cfg_attr(feature = "json", norito(rename = "stark-fri-sha256-goldilocks"))]
    StarkFriSha256Goldilocks,
    /// ZK-AMS masked relaxed-R1CS admission plus Ristretto255 possession and LSAG.
    ///
    /// Batch admission uses Poseidon2/Goldilocks commitment digests and a
    /// transparent STARK/FRI proof. Account provisioning uses MLSAGS over
    /// Ristretto255 with SHA3-512 for the transcript and hash-to-group suite.
    #[cfg_attr(
        feature = "json",
        norito(rename = "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512")
    )]
    ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512,
    /// Anonymous PGC k-out-of-n proof system over P-256.
    #[cfg_attr(feature = "json", norito(rename = "anonymous-pgc-p256"))]
    AnonymousPgcP256,
    /// Iroha Type-1 `VeRange` profile over P-256 with SHA-256.
    ///
    /// This profile is distinct from the upstream BN254-and-Keccak reference.
    #[cfg_attr(feature = "json", norito(rename = "iroha-verange-p256"))]
    IrohaVeRangeP256,
    /// Vega Neutron/Nova/Spartan proof system with Hyrax commitments over T256.
    #[cfg_attr(
        feature = "json",
        norito(rename = "vega-neutron-nova-spartan-hyrax-t256")
    )]
    VegaNeutronNovaSpartanHyraxT256,
    /// Jindo batched univariate lattice polynomial-commitment proof system.
    #[cfg_attr(feature = "json", norito(rename = "jindo-polynomial-commitment"))]
    JindoPolynomialCommitment,
    /// Halo2 IPA proof system over the Pasta curve cycle.
    #[cfg_attr(feature = "json", norito(rename = "halo2-ipa-pasta"))]
    Halo2IpaPasta,
    /// FCMP++ Curve Tree and Bulletproofs proof composition.
    #[cfg_attr(
        feature = "json",
        norito(rename = "fcmp-plus-plus-curve-tree-bulletproofs")
    )]
    FcmpPlusPlusCurveTreeBulletproofs,
    /// Bootle Lantern/LNP22 module-lattice linear-and-norm proof system.
    #[cfg_attr(feature = "json", norito(rename = "lantern-lnp22-module-linear-norm"))]
    LanternLnp22ModuleLinearNorm,
}

/// Native verifier engine implementation selected by a privacy protocol.
///
/// Engine identity binds the pinned experimental Rust implementation
/// independently of the mathematical proof-system profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "engine", content = "value", deny_unknown_fields)
)]
pub enum PrivacyEngineIdV1 {
    /// Native Goldilocks STARK/FRI verifier.
    #[cfg_attr(feature = "json", norito(rename = "native-goldilocks-stark-fri"))]
    NativeGoldilocksStarkFri,
    /// Native ZK-AMS masked relaxed-R1CS and Ristretto255 verifier suite.
    #[cfg_attr(
        feature = "json",
        norito(rename = "native-zk-ams-masked-relaxed-spartan-t256-ristretto255")
    )]
    NativeZkAmsMaskedRelaxedSpartanT256Ristretto255,
    /// Native Anonymous PGC verifier over P-256.
    #[cfg_attr(feature = "json", norito(rename = "native-anonymous-pgc-p256"))]
    NativeAnonymousPgcP256,
    /// Native `VeRange` verifier over P-256.
    #[cfg_attr(feature = "json", norito(rename = "native-verange-p256"))]
    NativeVeRangeP256,
    /// Native Vega verifier.
    #[cfg_attr(feature = "json", norito(rename = "native-vega"))]
    NativeVega,
    /// Native Jindo verifier.
    #[cfg_attr(feature = "json", norito(rename = "native-jindo"))]
    NativeJindo,
    /// Native Orchard Halo2 verifier.
    #[cfg_attr(feature = "json", norito(rename = "native-halo2-orchard"))]
    NativeHalo2Orchard,
    /// Native FCMP++ verifier.
    #[cfg_attr(feature = "json", norito(rename = "native-fcmp-plus-plus"))]
    NativeFcmpPlusPlus,
    /// Native Bootle Lantern/LNP22 module-lattice verifier.
    #[cfg_attr(feature = "json", norito(rename = "native-lantern-lnp22"))]
    NativeLanternLnp22,
}

macro_rules! define_privacy_digest {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[repr(transparent)]
        #[norito(transparent, decode_from_slice)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        pub struct $name(
            /// The exact 32-byte digest value.
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
            pub [u8; 32],
        );

        impl $name {
            /// Construct a digest from exactly 32 bytes.
            #[must_use]
            pub const fn new(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Borrow the exact digest bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Consume the digest and return its exact bytes.
            #[must_use]
            pub const fn into_bytes(self) -> [u8; 32] {
                self.0
            }

            /// Return `true` when every digest byte is zero.
            #[must_use]
            pub fn is_zero(&self) -> bool {
                self.0.iter().all(|byte| *byte == 0)
            }
        }

        impl From<[u8; 32]> for $name {
            fn from(bytes: [u8; 32]) -> Self {
                Self::new(bytes)
            }
        }

        impl From<$name> for [u8; 32] {
            fn from(value: $name) -> Self {
                value.into_bytes()
            }
        }

        impl AsRef<[u8; 32]> for $name {
            fn as_ref(&self) -> &[u8; 32] {
                self.as_bytes()
            }
        }
    };
}

define_privacy_digest!(
    /// Digest of the governed public parameter set for a protocol.
    PrivacyParameterDigestV1
);
define_privacy_digest!(
    /// Digest of the exact native verifier key or verifier artifact.
    PrivacyVerifierDigestV1
);
define_privacy_digest!(
    /// Digest of the canonical public-statement schema.
    PrivacyStatementSchemaDigestV1
);
define_privacy_digest!(
    /// Digest of the pinned experimental native engine manifest.
    PrivacyEngineManifestDigestV1
);
define_privacy_digest!(
    /// Digest of a canonical protocol-specific public statement.
    PrivacyStatementDigestV1
);
define_privacy_digest!(
    /// Digest of the canonical transaction-intent projection bound by a privacy statement.
    PrivacyTransactionIntentDigestV1
);
define_privacy_digest!(
    /// Digest of the canonical chain, genesis, action, and governed-artifact binding
    /// consumed by a native privacy prover or verifier.
    PrivacyNativeConsensusBindingDigestV1
);
define_privacy_digest!(
    /// Digest of a canonical governance root publication.
    PrivacyRootPublicationDigestV1
);
define_privacy_digest!(
    /// Digest of one canonical governed Orchard pool bootstrap.
    PrivacyOrchardPoolBootstrapDigestV1
);
define_privacy_digest!(
    /// Digest of one canonical governed FCMP++, private-IVM, or PQ-MASP pool bootstrap.
    PrivacyProofManagedPoolBootstrapDigestV1
);
define_privacy_digest!(
    /// Ledger-only identifier of one complete canonical FCMP++ `(O, I, C)` output tuple.
    PrivacyFcmpOutputIdV1
);
define_privacy_digest!(
    /// Typed FCMP++ linkability key image `L` used by the durable replay registry.
    PrivacyFcmpKeyImageV1
);
define_privacy_digest!(
    /// Digest of a canonical PGC account bootstrap payload.
    PrivacyPgcAccountBootstrapDigestV1
);
define_privacy_digest!(
    /// Digest of exact canonical Anonymous PGC bootstrap proof bytes.
    PrivacyPgcBootstrapProofDigestV1
);
define_privacy_digest!(
    /// Fixed identifier of a governed privacy parameter set.
    PrivacyParameterIdV1
);
define_privacy_digest!(
    /// Canonical replay-prevention nullifier emitted by a privacy action.
    PrivacyNullifierV1
);
define_privacy_digest!(
    /// Canonical output or account commitment emitted by a privacy action.
    PrivacyCommitmentV1
);
define_privacy_digest!(
    /// Fixed identifier of a privacy pool or accumulator namespace.
    PrivacyPoolIdV1
);
define_privacy_digest!(
    /// Fixed identifier of a ZK-AMS admitted-identity registry.
    PrivacyZkAmsRegistryIdV1
);
define_privacy_digest!(
    /// Fixed identifier of a governed privacy policy.
    PrivacyPolicyIdV1
);
define_privacy_digest!(
    /// Digest of the governed contents of a privacy policy.
    PrivacyPolicyDigestV1
);
define_privacy_digest!(
    /// Self-digest of one complete authoritative ZK-ACE policy record.
    PrivacyZkAcePolicyRecordDigestV1
);
define_privacy_digest!(
    /// Digest of one canonical committed Bootle/Lantern issuer-policy record.
    PrivacyBootleLanternIssuerPolicyDigestV1
);
define_privacy_digest!(
    /// Fixed identifier of a credential or certificate issuer.
    PrivacyIssuerIdV1
);
define_privacy_digest!(
    /// Digest of one canonically encoded credential attribute.
    PrivacyAttributeDigestV1
);
define_privacy_digest!(
    /// Reader or wallet challenge bound into a credential presentation.
    PrivacyChallengeV1
);
define_privacy_digest!(
    /// Digest of an ISO 18013-5 session transcript.
    PrivacySessionTranscriptDigestV1
);
define_privacy_digest!(
    /// Public SHA-256 device-authentication digest `H_dev` in Vega Figure 9.
    PrivacyVegaDeviceAuthenticationDigestV1
);
define_privacy_digest!(
    /// Self-digest of one complete authoritative Vega issuer-key/policy revision.
    PrivacyVegaIssuerRecordDigestV1
);
define_privacy_digest!(
    /// Fixed cryptographic recipient identity used by an encrypted output.
    PrivacyRecipientIdV1
);
define_privacy_digest!(
    /// Fixed ephemeral public encryption key used by an encrypted output.
    PrivacyEncryptionKeyV1
);
define_privacy_digest!(
    /// Fixed identifier of a private IVM program.
    PrivacyProgramIdV1
);
define_privacy_digest!(
    /// Digest of the exact private IVM action selected by a statement.
    PrivacyActionDigestV1
);
define_privacy_digest!(
    /// Digest of a certificate subject public key.
    PrivacyCertificateKeyDigestV1
);
define_privacy_digest!(
    /// Digest of one canonical ordered RFC 5280 P-256/SHA-256 trust store.
    PrivacyX509TrustStoreDigestV1
);
define_privacy_digest!(
    /// Self-digest of one immutable authoritative X.509 trust-anchor revision.
    PrivacyZkX509TrustAnchorRecordDigestV1
);
define_privacy_digest!(
    /// Self-digest of one immutable authoritative X.509 certificate-policy revision.
    PrivacyZkX509CertificatePolicyRecordDigestV1
);
define_privacy_digest!(
    /// Domain-framed SHA-256 digest of one exact signed DER certificate-revocation list.
    PrivacyX509CrlDerDigestV1
);
define_privacy_digest!(
    /// Domain-framed SHA-256 digest of the exact SPKI that signs one governed CRL lineage.
    PrivacyX509CrlIssuerSpkiDigestV1
);
define_privacy_digest!(
    /// Self-digest of one immutable authoritative signed-CRL revision.
    PrivacyZkX509CrlRecordDigestV1
);

impl PrivacyX509CrlDerDigestV1 {
    /// Hash the complete exact signed DER CRL with the canonical X.509 frame.
    #[must_use]
    pub fn digest_exact_der(der: &[u8]) -> Self {
        Self::new(privacy_zk_x509_sha256_frame_v1(
            ZK_X509_CRL_DER_DIGEST_DOMAIN_V1,
            &[der],
        ))
    }
}

impl PrivacyX509CrlIssuerSpkiDigestV1 {
    /// Hash the complete exact issuer SPKI DER with the canonical X.509 frame.
    #[must_use]
    pub fn digest_exact_der(spki_der: &[u8]) -> Self {
        Self::new(privacy_zk_x509_sha256_frame_v1(
            ZK_X509_CRL_ISSUER_SPKI_DIGEST_DOMAIN_V1,
            &[spki_der],
        ))
    }
}

define_privacy_digest!(
    /// Canonical commitment-tree or accumulator root.
    PrivacyRootV1
);
define_privacy_digest!(
    /// Digest of a post-quantum transaction authorization key.
    PrivacyAuthorizationKeyDigestV1
);
define_privacy_digest!(
    /// Digest of a post-quantum note-encryption key.
    PrivacyNoteEncryptionKeyDigestV1
);
define_privacy_digest!(
    /// Fixed 32-byte digest of one canonical Personhood Credential in ZK-AMS.
    PrivacyZkAmsPhcHashV1
);
define_privacy_digest!(
    /// Hidden subject commitment carried by a canonical ZK-AMS credential.
    PrivacyZkAmsSubjectCommitmentV1
);
define_privacy_digest!(
    /// Issuer-selected nonce making one canonical ZK-AMS credential unique.
    PrivacyZkAmsCredentialNonceV1
);
define_privacy_digest!(
    /// Digest of an authoritative ZK-AMS issuer-policy record.
    PrivacyZkAmsIssuerPolicyRecordDigestV1
);
define_privacy_digest!(
    /// Digest of an authoritative ZK-AMS registry snapshot record.
    PrivacyZkAmsRegistryRecordDigestV1
);
define_privacy_digest!(
    /// Digest of one canonical governed ZK-AMS registry bootstrap.
    PrivacyZkAmsRegistryBootstrapDigestV1
);

macro_rules! define_ristretto255_encoding {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Decode,
            Encode,
            IntoSchema,
        )]
        #[repr(transparent)]
        #[norito(transparent, decode_from_slice)]
        #[cfg_attr(
            feature = "json",
            derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
        )]
        pub struct $name(
            /// Exact canonical compressed Ristretto255 encoding.
            #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
            pub [u8; 32],
        );

        impl $name {
            /// Construct from exactly 32 encoded bytes.
            #[must_use]
            pub const fn new(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Borrow the exact compressed bytes.
            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Return `true` for the all-zero sentinel encoding.
            ///
            /// The native engine additionally decodes the point, rejects
            /// non-canonical encodings, and rejects the group identity.
            #[must_use]
            pub fn is_zero(&self) -> bool {
                self.0.iter().all(|byte| *byte == 0)
            }
        }
    };
}

define_ristretto255_encoding!(
    /// Canonical compressed Ristretto255 ZK-AMS seed public key.
    PrivacyZkAmsSeedPublicKeyV1
);
define_ristretto255_encoding!(
    /// Canonical compressed Ristretto255 MLSAGS key image.
    PrivacyZkAmsKeyImageV1
);

// Keep the implementation in this public module: textual includes improve
// navigation without changing path-derived Norito identities.
include!("privacy/protocol.rs");
include!("privacy/credentials.rs");
include!("privacy/statements.rs");
include!("privacy/proofs.rs");

#[cfg(test)]
mod tests {
    include!("privacy/tests/protocol_and_proofs.rs");
    include!("privacy/tests/namespaces_and_governance.rs");
    include!("privacy/tests/adversarial.rs");
}
