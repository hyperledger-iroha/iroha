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
/// Permanent Norito schema identity for the top-level typed privacy statement.
pub const PRIVACY_STATEMENT_SCHEMA_NAME_V1: &str = "iroha.privacy.statement.v1";
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
    /// VeRange transparent range-proof protocol v1.
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

#[cfg(any(test, feature = "test-fixtures"))]
pub use exact12_fixture::{
    PrivacyExact12FixtureErrorV1, PrivacyExact12TypedEnvelopeRowV1,
    privacy_exact12_typed_envelope_rows_v1,
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
    /// Iroha Type-1 VeRange profile over P-256 with SHA-256.
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
    /// Native VeRange verifier over P-256.
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

/// Exact compressed SEC1 encoding of a P-256 point.
///
/// This wire type fixes the external width. Native P-256 engines additionally
/// enforce canonical SEC1 form, curve membership, and non-identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PrivacyP256PointV1(
    /// The exact 33-byte compressed SEC1 value.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub [u8; 33],
);

impl PrivacyP256PointV1 {
    /// Construct a point encoding from exactly 33 bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 33]) -> Self {
        Self(bytes)
    }

    /// Borrow the exact compressed SEC1 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 33] {
        &self.0
    }

    /// Consume the value and return its compressed SEC1 bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 33] {
        self.0
    }

    /// Return `true` when every encoded byte is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
}

impl From<[u8; 33]> for PrivacyP256PointV1 {
    fn from(bytes: [u8; 33]) -> Self {
        Self::new(bytes)
    }
}

impl From<PrivacyP256PointV1> for [u8; 33] {
    fn from(value: PrivacyP256PointV1) -> Self {
        value.into_bytes()
    }
}

impl AsRef<[u8; 33]> for PrivacyP256PointV1 {
    fn as_ref(&self) -> &[u8; 33] {
        self.as_bytes()
    }
}

/// Canonical twisted-ElGamal ciphertext `(C_L, C_R)` over P-256.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyP256CiphertextV1 {
    /// `C_L = pk^r`.
    pub left: PrivacyP256PointV1,
    /// `C_R = g^r h^v`.
    pub right: PrivacyP256PointV1,
}

/// Governed policy namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPolicyNamespaceV1 {
    /// Exact policy identity.
    pub policy_id: PrivacyPolicyIdV1,
}

/// Pool namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPoolNamespaceV1 {
    /// Exact pool identity.
    pub pool_id: PrivacyPoolIdV1,
}

/// Issuer, admitted-identity registry, and policy namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyIssuerRegistryPolicyNamespaceV1 {
    /// Exact credential issuer.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Exact admitted-identity registry.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Exact admission policy.
    pub policy_id: PrivacyPolicyIdV1,
}

/// Trust-anchor-wide namespace payload.
///
/// This scope owns the single CA-membership root derived from a complete
/// trust store. It deliberately excludes certificate-policy identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyTrustAnchorNamespaceV1 {
    /// Exact trust-anchor issuer.
    pub trust_anchor_id: PrivacyIssuerIdV1,
}

/// Trust-anchor and certificate-policy namespace payload.
///
/// This scope owns policy-specific statement state and the corresponding
/// issuer CRL root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyTrustAnchorPolicyNamespaceV1 {
    /// Exact trust-anchor issuer.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Exact certificate policy.
    pub policy_id: PrivacyPolicyIdV1,
}

/// Governed parameter namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyParameterNamespaceV1 {
    /// Exact parameter-set identity.
    pub parameter_id: PrivacyParameterIdV1,
}

/// Issuer and selective-disclosure policy namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyIssuerPolicyNamespaceV1 {
    /// Exact credential issuer.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Exact selective-disclosure policy.
    pub policy_id: PrivacyPolicyIdV1,
}

/// Pool and private-program namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPoolProgramNamespaceV1 {
    /// Exact private-note pool.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact private program.
    pub program_id: PrivacyProgramIdV1,
}

/// Protocol-specific portion of a replay, output, or root namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "scope", content = "value", deny_unknown_fields)
)]
pub enum PrivacyNamespaceScopeV1 {
    /// Governed authorization or range policy.
    Policy(PrivacyPolicyNamespaceV1),
    /// Anonymous-account or private-note pool.
    Pool(PrivacyPoolNamespaceV1),
    /// Credential issuer, admitted-identity registry, and admission policy.
    IssuerRegistryPolicy(PrivacyIssuerRegistryPolicyNamespaceV1),
    /// Certificate trust anchor, independent of certificate policy.
    TrustAnchor(PrivacyTrustAnchorNamespaceV1),
    /// Certificate trust anchor and certificate policy.
    TrustAnchorPolicy(PrivacyTrustAnchorPolicyNamespaceV1),
    /// Governed polynomial-commitment parameter set.
    Parameter(PrivacyParameterNamespaceV1),
    /// Anonymous-credential issuer and selective-disclosure policy.
    IssuerPolicy(PrivacyIssuerPolicyNamespaceV1),
    /// Private-note pool and exact IVM program.
    PoolProgram(PrivacyPoolProgramNamespaceV1),
}

/// Namespace component selected by validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyNamespaceComponentV1 {
    /// Policy identifier.
    Policy,
    /// Pool identifier.
    Pool,
    /// ZK-AMS admitted-identity registry identifier.
    Registry,
    /// Issuer or trust-anchor identifier.
    Issuer,
    /// Governed parameter-set identifier.
    Parameter,
    /// Private IVM program identifier.
    Program,
}

/// Closed namespace for one protocol's replay, output, and root state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyNamespaceV1 {
    protocol_id: PrivacyProtocolIdV1,
    scope: PrivacyNamespaceScopeV1,
}

impl PrivacyNamespaceV1 {
    /// Construct a namespace from a closed protocol and scope.
    #[must_use]
    pub const fn new(protocol_id: PrivacyProtocolIdV1, scope: PrivacyNamespaceScopeV1) -> Self {
        Self { protocol_id, scope }
    }

    /// Derive the only canonical namespace for a typed public statement.
    #[must_use]
    pub const fn from_statement(statement: &PrivacyStatementV1) -> Self {
        match statement {
            PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) => Self::new(
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
                PrivacyNamespaceScopeV1::Policy(PrivacyPolicyNamespaceV1 {
                    policy_id: statement.policy_id,
                }),
            ),
            PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement) => Self::new(
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: statement.pool_id,
                }),
            ),
            PrivacyStatementV1::VeRangeTransparentRangeV1(statement) => Self::new(
                PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                PrivacyNamespaceScopeV1::Policy(PrivacyPolicyNamespaceV1 {
                    policy_id: statement.policy_id,
                }),
            ),
            PrivacyStatementV1::IrohaZkAmsV1(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaZkAmsV1,
                PrivacyNamespaceScopeV1::IssuerRegistryPolicy(
                    PrivacyIssuerRegistryPolicyNamespaceV1 {
                        issuer_id: statement.issuer_id,
                        registry_id: statement.registry_id,
                        policy_id: statement.policy_id,
                    },
                ),
            ),
            PrivacyStatementV1::VegaExistingCredentialZkV0(statement) => Self::new(
                PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
                PrivacyNamespaceScopeV1::Parameter(PrivacyParameterNamespaceV1 {
                    parameter_id: statement.context.parameter_id,
                }),
            ),
            PrivacyStatementV1::IrohaZkX509StarkP256V0(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                PrivacyNamespaceScopeV1::TrustAnchorPolicy(PrivacyTrustAnchorPolicyNamespaceV1 {
                    trust_anchor_id: statement.trust_anchor_id,
                    policy_id: statement.certificate_policy_id,
                }),
            ),
            PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
                PrivacyNamespaceScopeV1::Parameter(PrivacyParameterNamespaceV1 {
                    parameter_id: statement.context.parameter_id,
                }),
            ),
            PrivacyStatementV1::IrohaBootleLanternAnoncredV1(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
                PrivacyNamespaceScopeV1::IssuerPolicy(PrivacyIssuerPolicyNamespaceV1 {
                    issuer_id: statement.issuer_id,
                    policy_id: statement.policy_id,
                }),
            ),
            PrivacyStatementV1::OrchardHalo2ActionsV1(statement) => Self::new(
                PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: statement.pool_id,
                }),
            ),
            PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) => Self::new(
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: statement.pool_id,
                }),
            ),
            PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                PrivacyNamespaceScopeV1::PoolProgram(PrivacyPoolProgramNamespaceV1 {
                    pool_id: statement.pool_id,
                    program_id: statement.program_id,
                }),
            ),
            PrivacyStatementV1::PqMaspStarkV0(statement) => Self::new(
                PrivacyProtocolIdV1::PqMaspStarkV0,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: statement.pool_id,
                }),
            ),
        }
    }

    /// Return the protocol owning this namespace.
    #[must_use]
    pub const fn protocol_id(self) -> PrivacyProtocolIdV1 {
        self.protocol_id
    }

    /// Return the protocol-specific scope.
    #[must_use]
    pub const fn scope(self) -> PrivacyNamespaceScopeV1 {
        self.scope
    }

    /// Validate protocol/scope compatibility and nonzero components.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyNamespaceValidationError`] for a mismatched closed
    /// variant or zero namespace component.
    pub fn validate(self) -> Result<(), PrivacyNamespaceValidationError> {
        let compatible = matches!(
            (self.protocol_id, self.scope),
            (
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0
                    | PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                PrivacyNamespaceScopeV1::Policy(_)
            ) | (
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
                    | PrivacyProtocolIdV1::OrchardHalo2ActionsV1
                    | PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1
                    | PrivacyProtocolIdV1::PqMaspStarkV0,
                PrivacyNamespaceScopeV1::Pool(_)
            ) | (
                PrivacyProtocolIdV1::IrohaZkAmsV1,
                PrivacyNamespaceScopeV1::IssuerRegistryPolicy(_)
            ) | (
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                PrivacyNamespaceScopeV1::TrustAnchor(_)
                    | PrivacyNamespaceScopeV1::TrustAnchorPolicy(_)
            ) | (
                PrivacyProtocolIdV1::VegaExistingCredentialZkV0
                    | PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
                PrivacyNamespaceScopeV1::Parameter(_)
            ) | (
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
                PrivacyNamespaceScopeV1::IssuerPolicy(_)
            ) | (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                PrivacyNamespaceScopeV1::PoolProgram(_)
            )
        );
        if !compatible {
            return Err(PrivacyNamespaceValidationError::IncompatibleScope {
                protocol_id: self.protocol_id,
            });
        }
        match self.scope {
            PrivacyNamespaceScopeV1::Policy(scope) => validate_namespace_component(
                !scope.policy_id.is_zero(),
                PrivacyNamespaceComponentV1::Policy,
            ),
            PrivacyNamespaceScopeV1::Pool(scope) => validate_namespace_component(
                !scope.pool_id.is_zero(),
                PrivacyNamespaceComponentV1::Pool,
            ),
            PrivacyNamespaceScopeV1::IssuerRegistryPolicy(scope) => {
                validate_namespace_component(
                    !scope.issuer_id.is_zero(),
                    PrivacyNamespaceComponentV1::Issuer,
                )?;
                validate_namespace_component(
                    !scope.registry_id.is_zero(),
                    PrivacyNamespaceComponentV1::Registry,
                )?;
                validate_namespace_component(
                    !scope.policy_id.is_zero(),
                    PrivacyNamespaceComponentV1::Policy,
                )
            }
            PrivacyNamespaceScopeV1::TrustAnchor(scope) => validate_namespace_component(
                !scope.trust_anchor_id.is_zero(),
                PrivacyNamespaceComponentV1::Issuer,
            ),
            PrivacyNamespaceScopeV1::TrustAnchorPolicy(scope) => {
                validate_namespace_component(
                    !scope.trust_anchor_id.is_zero(),
                    PrivacyNamespaceComponentV1::Issuer,
                )?;
                validate_namespace_component(
                    !scope.policy_id.is_zero(),
                    PrivacyNamespaceComponentV1::Policy,
                )
            }
            PrivacyNamespaceScopeV1::Parameter(scope) => validate_namespace_component(
                !scope.parameter_id.is_zero(),
                PrivacyNamespaceComponentV1::Parameter,
            ),
            PrivacyNamespaceScopeV1::IssuerPolicy(scope) => {
                validate_namespace_component(
                    !scope.issuer_id.is_zero(),
                    PrivacyNamespaceComponentV1::Issuer,
                )?;
                validate_namespace_component(
                    !scope.policy_id.is_zero(),
                    PrivacyNamespaceComponentV1::Policy,
                )
            }
            PrivacyNamespaceScopeV1::PoolProgram(scope) => {
                validate_namespace_component(
                    !scope.pool_id.is_zero(),
                    PrivacyNamespaceComponentV1::Pool,
                )?;
                validate_namespace_component(
                    !scope.program_id.is_zero(),
                    PrivacyNamespaceComponentV1::Program,
                )
            }
        }
    }
}

fn validate_namespace_component(
    nonzero: bool,
    component: PrivacyNamespaceComponentV1,
) -> Result<(), PrivacyNamespaceValidationError> {
    if !nonzero {
        return Err(PrivacyNamespaceValidationError::ZeroComponent { component });
    }
    Ok(())
}

/// Validation failure for [`PrivacyNamespaceV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyNamespaceValidationError {
    /// Protocol and scope closed variants are incompatible.
    #[error("privacy namespace scope is incompatible with protocol {protocol_id:?}")]
    IncompatibleScope {
        /// Protocol carrying the incompatible scope.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// A required scope component is zero.
    #[error("privacy namespace component {component:?} must be non-zero")]
    ZeroComponent {
        /// Invalid component.
        component: PrivacyNamespaceComponentV1,
    },
}

/// Authority responsible for advancing one root role after initialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "management", content = "value", deny_unknown_fields)
)]
pub enum PrivacyRootManagementV1 {
    /// Roots advance only through an admitted proof-managed state transition.
    ProofManaged,
    /// Roots advance through an authorized governance publication.
    GovernanceManaged,
}

/// Semantic role of one canonical root inside a protocol namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "role", content = "value", deny_unknown_fields)
)]
pub enum PrivacyRootRoleV1 {
    /// Mutable encrypted PGC account table.
    PgcAccountState,
    /// ZK-AMS admitted identities, seed keys, and provisioning records.
    AccountRegistry,
    /// Credential revocation accumulator.
    Revocation,
    /// X.509 CA-membership accumulator.
    CertificateAuthorityMembership,
    /// Orchard or PQ-MASP note-commitment anchor.
    NoteCommitmentAnchor,
    /// FCMP++ complete output-set accumulator.
    OutputSet,
    /// Private IVM program state.
    ProgramState,
}

impl PrivacyRootRoleV1 {
    /// Return the sole authority model for this root role.
    #[must_use]
    pub const fn management(self) -> PrivacyRootManagementV1 {
        match self {
            Self::PgcAccountState
            | Self::AccountRegistry
            | Self::NoteCommitmentAnchor
            | Self::OutputSet
            | Self::ProgramState => PrivacyRootManagementV1::ProofManaged,
            Self::Revocation | Self::CertificateAuthorityMembership => {
                PrivacyRootManagementV1::GovernanceManaged
            }
        }
    }

    /// Return whether this role is meaningful for `protocol_id`.
    #[must_use]
    pub const fn is_compatible_with(self, protocol_id: PrivacyProtocolIdV1) -> bool {
        matches!(
            (protocol_id, self),
            (
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                Self::PgcAccountState
            ) | (PrivacyProtocolIdV1::IrohaZkAmsV1, Self::AccountRegistry)
                | (
                    PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
                    Self::Revocation
                )
                | (
                    PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                    Self::CertificateAuthorityMembership
                )
                | (
                    PrivacyProtocolIdV1::OrchardHalo2ActionsV1 | PrivacyProtocolIdV1::PqMaspStarkV0,
                    Self::NoteCommitmentAnchor
                )
                | (PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1, Self::OutputSet)
                | (
                    PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                    Self::ProgramState
                )
        )
    }

    /// Return whether this role is meaningful for the exact namespace scope.
    ///
    /// X.509 has one trust-anchor-wide CA-membership root. Its signed CRL is a
    /// governed record, not a second root-bearing state machine.
    #[must_use]
    pub const fn is_compatible_with_namespace(self, namespace: PrivacyNamespaceV1) -> bool {
        if !self.is_compatible_with(namespace.protocol_id()) {
            return false;
        }
        match self {
            Self::CertificateAuthorityMembership => {
                matches!(namespace.scope(), PrivacyNamespaceScopeV1::TrustAnchor(_))
            }
            Self::PgcAccountState
            | Self::AccountRegistry
            | Self::Revocation
            | Self::NoteCommitmentAnchor
            | Self::OutputSet
            | Self::ProgramState => true,
        }
    }
}

/// Governance payload publishing one canonical privacy root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyRootPublicationV1 {
    /// Exact protocol-scoped root namespace.
    pub namespace: PrivacyNamespaceV1,
    /// Semantic root role inside the namespace.
    pub role: PrivacyRootRoleV1,
    /// Monotonically advancing root epoch.
    pub epoch: u64,
    /// Published canonical root.
    pub root: PrivacyRootV1,
}

impl PrivacyRootPublicationV1 {
    /// Construct and validate a root publication.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyRootPublicationValidationError`] for a malformed
    /// namespace, incompatible role, zero epoch, or zero root.
    pub fn new(
        namespace: PrivacyNamespaceV1,
        role: PrivacyRootRoleV1,
        epoch: u64,
        root: PrivacyRootV1,
    ) -> Result<Self, PrivacyRootPublicationValidationError> {
        let publication = Self {
            namespace,
            role,
            epoch,
            root,
        };
        publication.validate()?;
        Ok(publication)
    }

    /// Validate this root publication.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyRootPublicationValidationError`] for any malformed
    /// field or closed protocol/role mismatch.
    pub fn validate(&self) -> Result<(), PrivacyRootPublicationValidationError> {
        self.namespace
            .validate()
            .map_err(PrivacyRootPublicationValidationError::Namespace)?;
        if !self.role.is_compatible_with(self.namespace.protocol_id()) {
            return Err(PrivacyRootPublicationValidationError::IncompatibleRole {
                protocol_id: self.namespace.protocol_id(),
                role: self.role,
            });
        }
        if !self.role.is_compatible_with_namespace(self.namespace) {
            return Err(
                PrivacyRootPublicationValidationError::IncompatibleNamespaceScope {
                    scope: self.namespace.scope(),
                    role: self.role,
                },
            );
        }
        if self.epoch == 0 {
            return Err(PrivacyRootPublicationValidationError::ZeroEpoch);
        }
        if self.root.is_zero() {
            return Err(PrivacyRootPublicationValidationError::ZeroRoot);
        }
        Ok(())
    }

    /// Hash this publication using canonical Norito bytes and its own domain.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyRootPublicationDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_ROOT_PUBLICATION_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyRootPublicationDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}

/// Validation failure for [`PrivacyRootPublicationV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyRootPublicationValidationError {
    /// Namespace is malformed.
    #[error("privacy root namespace is invalid: {0}")]
    Namespace(PrivacyNamespaceValidationError),
    /// Root role is incompatible with the namespace protocol.
    #[error("privacy root role {role:?} is incompatible with protocol {protocol_id:?}")]
    IncompatibleRole {
        /// Namespace protocol.
        protocol_id: PrivacyProtocolIdV1,
        /// Incompatible role.
        role: PrivacyRootRoleV1,
    },
    /// Root role is incompatible with the namespace's exact scope.
    #[error("privacy root role {role:?} is incompatible with namespace scope {scope:?}")]
    IncompatibleNamespaceScope {
        /// Rejected namespace scope.
        scope: PrivacyNamespaceScopeV1,
        /// Rejected root role.
        role: PrivacyRootRoleV1,
    },
    /// Root epoch is zero.
    #[error("privacy root publication epoch must be non-zero")]
    ZeroEpoch,
    /// Root is zero.
    #[error("privacy root publication root must be non-zero")]
    ZeroRoot,
}

/// Governance payload establishing one Orchard pool's immutable public bridge.
///
/// The payload deliberately contains neither an initial root nor an initial
/// epoch. Core derives the pinned Orchard V3 empty-tree root and installs it at
/// [`PRIVACY_ORCHARD_POOL_INITIAL_EPOCH_V1`], so governance cannot choose an
/// alternate accumulator origin.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyOrchardPoolBootstrapV1 {
    /// Stable Orchard pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact public asset represented by this private pool.
    pub asset_definition_id: AssetDefinitionId,
    /// Governed public reserve account used for deposits and withdrawals.
    pub reserve_account: AccountId,
}

impl PrivacyOrchardPoolBootstrapV1 {
    /// Construct and validate one canonical Orchard pool bootstrap.
    ///
    /// # Errors
    ///
    /// Rejects the all-zero pool identifier.
    pub fn new(
        pool_id: PrivacyPoolIdV1,
        asset_definition_id: AssetDefinitionId,
        reserve_account: AccountId,
    ) -> Result<Self, PrivacyOrchardPoolBootstrapValidationErrorV1> {
        let bootstrap = Self {
            pool_id,
            asset_definition_id,
            reserve_account,
        };
        bootstrap.validate()?;
        Ok(bootstrap)
    }

    /// Return the sole protocol-scoped namespace for this pool.
    #[must_use]
    pub const fn namespace(&self) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: self.pool_id,
            }),
        )
    }

    /// Validate the closed Orchard namespace fields.
    ///
    /// # Errors
    ///
    /// Rejects the all-zero pool identifier.
    pub fn validate(&self) -> Result<(), PrivacyOrchardPoolBootstrapValidationErrorV1> {
        if self.pool_id.is_zero() {
            return Err(PrivacyOrchardPoolBootstrapValidationErrorV1::ZeroPoolId);
        }
        self.namespace()
            .validate()
            .map_err(PrivacyOrchardPoolBootstrapValidationErrorV1::Namespace)
    }

    /// Hash the exact canonical bootstrap in its own provenance domain.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyOrchardPoolBootstrapDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_ORCHARD_POOL_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyOrchardPoolBootstrapDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}

/// Structural failure for [`PrivacyOrchardPoolBootstrapV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyOrchardPoolBootstrapValidationErrorV1 {
    /// The stable pool identifier is all zero.
    #[error("Orchard pool bootstrap pool id must be non-zero")]
    ZeroPoolId,
    /// The derived closed namespace is malformed.
    #[error("Orchard pool bootstrap namespace is invalid: {0}")]
    Namespace(PrivacyNamespaceValidationError),
}

/// Exact unframed byte width of one FCMP++ `(O,I,C)` tuple.
pub const PRIVACY_FCMP_OUTPUT_TUPLE_BYTES_V1: usize = 3 * 32;

/// One complete FCMP++ output-tree leaf `(O, I, C)`.
///
/// All three values are canonical compressed prime-order Edwards points. The
/// data model preserves the exact encodings; the native FCMP++ engine performs
/// the curve, subgroup, and non-identity checks before governance or proof
/// admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpOutputTupleV1 {
    /// One-time output key `O`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub output_key: [u8; 32],
    /// Per-output linking-tag generator `I`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub linking_tag_generator: [u8; 32],
    /// Amount commitment `C`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub amount_commitment: [u8; 32],
}

impl PrivacyFcmpOutputTupleV1 {
    /// Derive the ledger-only identifier used for duplicate detection and
    /// output lookup.
    ///
    /// The native curve tree and FCMP++ relation always consume the full tuple.
    #[must_use]
    pub fn output_id(self) -> PrivacyFcmpOutputIdV1 {
        let mut hasher = Sha256::new();
        hasher.update(PRIVACY_FCMP_OUTPUT_ID_DOMAIN_V1);
        hasher.update(self.output_key);
        hasher.update(self.linking_tag_generator);
        hasher.update(self.amount_commitment);
        PrivacyFcmpOutputIdV1::new(hasher.finalize().into())
    }

    /// Reject the reserved all-zero encoding before native curve validation.
    ///
    /// # Errors
    ///
    /// Returns the exact zero component.
    pub fn validate_nonzero(self) -> Result<(), PrivacyFcmpOutputTupleValidationErrorV1> {
        if self.output_key.iter().all(|byte| *byte == 0) {
            return Err(PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                component: PrivacyFcmpOutputComponentV1::OutputKey,
            });
        }
        if self.linking_tag_generator.iter().all(|byte| *byte == 0) {
            return Err(PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                component: PrivacyFcmpOutputComponentV1::LinkingTagGenerator,
            });
        }
        if self.amount_commitment.iter().all(|byte| *byte == 0) {
            return Err(PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                component: PrivacyFcmpOutputComponentV1::AmountCommitment,
            });
        }
        Ok(())
    }
}

/// FCMP++ output-tuple component selected by validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyFcmpOutputComponentV1 {
    /// One-time output key `O`.
    OutputKey,
    /// Per-output linking-tag generator `I`.
    LinkingTagGenerator,
    /// Amount commitment `C`.
    AmountCommitment,
}

/// Structural failure for one FCMP++ output tuple.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyFcmpOutputTupleValidationErrorV1 {
    /// A mandatory point uses the reserved all-zero encoding.
    #[error("FCMP++ output tuple component {component:?} must be non-zero")]
    ZeroComponent {
        /// Rejected component.
        component: PrivacyFcmpOutputComponentV1,
    },
}

/// Canonical typed root of the alternating FCMP++ Selene/Helios curve tree.
///
/// Odd layer counts identify Selene roots and even layer counts identify
/// Helios roots. The layer count is cryptographically significant and cannot
/// be inferred from the compressed point.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpTreeRootV1 {
    /// Number of alternating curve-tree layers.
    pub layers: u8,
    /// Canonical compressed Selene or Helios point selected by layer parity.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub point: [u8; 32],
}

impl PrivacyFcmpTreeRootV1 {
    /// Largest layer count admitted by the first-release FCMP++ wire.
    pub const MAX_LAYERS: u8 = 32;

    /// Validate the closed structural shape before native curve validation.
    ///
    /// # Errors
    ///
    /// Rejects zero/excessive layers or the all-zero point sentinel.
    pub fn validate(self) -> Result<(), PrivacyFcmpTreeRootValidationErrorV1> {
        if self.layers == 0 || self.layers > Self::MAX_LAYERS {
            return Err(PrivacyFcmpTreeRootValidationErrorV1::InvalidLayerCount {
                layers: self.layers,
                max: Self::MAX_LAYERS,
            });
        }
        if self.point.iter().all(|byte| *byte == 0) {
            return Err(PrivacyFcmpTreeRootValidationErrorV1::ZeroPoint);
        }
        Ok(())
    }

    /// Commit the typed root into the shared 32-byte retained-root index.
    ///
    /// Durable FCMP++ accumulator state still stores the complete typed root;
    /// this digest is only the history/map key.
    #[must_use]
    pub fn history_commitment(self) -> PrivacyRootV1 {
        let mut hasher = Sha256::new();
        hasher.update(PRIVACY_FCMP_ROOT_COMMITMENT_DOMAIN_V1);
        hasher.update([self.layers]);
        hasher.update(self.point);
        PrivacyRootV1::new(hasher.finalize().into())
    }
}

/// Structural failure for one typed FCMP++ tree root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyFcmpTreeRootValidationErrorV1 {
    /// Layer count is outside the closed first-release bound.
    #[error("FCMP++ tree layers {layers} are outside 1..={max}")]
    InvalidLayerCount {
        /// Rejected layer count.
        layers: u8,
        /// Compiled maximum.
        max: u8,
    },
    /// Compressed root point is the reserved all-zero sentinel.
    #[error("FCMP++ tree root point must be non-zero")]
    ZeroPoint,
}

/// Complete public FCMP++ relation for one hidden consumed output.
///
/// The IFC1 proof duplicates `O~`, `I~`, and `R`; the native decoder must
/// compare those bytes exactly. `C~` and the key image `L` remain
/// statement-only public inputs to the complete relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpInputPublicV1 {
    /// Rerandomized output key `O~`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub output_key_tilde: [u8; 32],
    /// Rerandomized linking-tag generator `I~`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub linking_tag_generator_tilde: [u8; 32],
    /// Rerandomization commitment `R`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub rerandomization_commitment: [u8; 32],
    /// Pseudo output amount commitment `C~`.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub pseudo_out: [u8; 32],
    /// Linkability key image `L`.
    pub key_image: PrivacyFcmpKeyImageV1,
}

impl PrivacyFcmpInputPublicV1 {
    /// Reject the reserved all-zero encoding before native curve validation.
    ///
    /// # Errors
    ///
    /// Returns the exact zero component.
    pub fn validate_nonzero(self) -> Result<(), PrivacyFcmpInputValidationErrorV1> {
        for (component, point) in [
            (
                PrivacyFcmpInputComponentV1::OutputKeyTilde,
                self.output_key_tilde,
            ),
            (
                PrivacyFcmpInputComponentV1::LinkingTagGeneratorTilde,
                self.linking_tag_generator_tilde,
            ),
            (
                PrivacyFcmpInputComponentV1::RerandomizationCommitment,
                self.rerandomization_commitment,
            ),
            (PrivacyFcmpInputComponentV1::PseudoOut, self.pseudo_out),
            (
                PrivacyFcmpInputComponentV1::KeyImage,
                self.key_image.into_bytes(),
            ),
        ] {
            if point.iter().all(|byte| *byte == 0) {
                return Err(PrivacyFcmpInputValidationErrorV1::ZeroComponent { component });
            }
        }
        Ok(())
    }
}

/// FCMP++ input component selected by validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyFcmpInputComponentV1 {
    /// Rerandomized output key `O~`.
    OutputKeyTilde,
    /// Rerandomized linking-tag generator `I~`.
    LinkingTagGeneratorTilde,
    /// Rerandomization commitment `R`.
    RerandomizationCommitment,
    /// Pseudo output amount commitment `C~`.
    PseudoOut,
    /// Linkability key image `L`.
    KeyImage,
}

/// Structural failure for one FCMP++ public input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyFcmpInputValidationErrorV1 {
    /// A mandatory point uses the reserved all-zero encoding.
    #[error("FCMP++ public input component {component:?} must be non-zero")]
    ZeroComponent {
        /// Rejected component.
        component: PrivacyFcmpInputComponentV1,
    },
}

/// Magic prefix of the sole first-release FCMP++ wallet ciphertext codec.
pub const PRIVACY_FCMP_ENCRYPTED_OUTPUT_MAGIC_V1: [u8; 4] = *b"IFCE";
/// XChaCha20-Poly1305 nonce width in the FCMP++ wallet ciphertext.
pub const PRIVACY_FCMP_ENCRYPTED_OUTPUT_NONCE_BYTES_V1: usize = 24;
/// Fixed plaintext width: magic, output id, complete `(O,I,C)` tuple, positive
/// `u64` amount, amount-commitment mask, and the two spend-opening scalars.
pub const PRIVACY_FCMP_NOTE_PLAINTEXT_BYTES_V1: usize =
    4 + 32 + PRIVACY_FCMP_OUTPUT_TUPLE_BYTES_V1 + 8 + 32 + 32 + 32;
/// Poly1305 authentication-tag width.
pub const PRIVACY_FCMP_ENCRYPTED_OUTPUT_TAG_BYTES_V1: usize = 16;
/// Exact ciphertext field width, including codec magic and explicit nonce.
pub const PRIVACY_FCMP_ENCRYPTED_OUTPUT_BYTES_V1: usize = 4
    + PRIVACY_FCMP_ENCRYPTED_OUTPUT_NONCE_BYTES_V1
    + PRIVACY_FCMP_NOTE_PLAINTEXT_BYTES_V1
    + PRIVACY_FCMP_ENCRYPTED_OUTPUT_TAG_BYTES_V1;

/// Magic prefix of the sole first-release private-IVM wallet ciphertext.
pub const PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1: [u8; 4] = *b"IPNE";
/// XChaCha20-Poly1305 nonce width in a private-IVM wallet ciphertext.
pub const PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1: usize = 24;
/// Fixed plaintext width: magic, public commitment, `u128` value, spending
/// authority, note nonce, commitment blinding, and wallet memo digest.
pub const PRIVACY_IVM_PRIVATE_NOTE_PLAINTEXT_BYTES_V1: usize = 4 + 32 + 16 + 32 + 32 + 32 + 32;
/// Poly1305 authentication-tag width.
pub const PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_TAG_BYTES_V1: usize = 16;
/// Exact private-IVM ciphertext width, including codec magic and explicit
/// nonce.
pub const PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1: usize = 4
    + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1
    + PRIVACY_IVM_PRIVATE_NOTE_PLAINTEXT_BYTES_V1
    + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_TAG_BYTES_V1;

/// Typed encrypted payload for one complete FCMP++ output tuple.
///
/// The output identifier is a ledger index only. The statement and native
/// curve tree always retain and consume the corresponding full `(O, I, C)`
/// tuple.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpEncryptedOutputV1 {
    /// Cryptographic recipient identity.
    pub recipient: PrivacyRecipientIdV1,
    /// Ephemeral public encryption key.
    pub ephemeral_public_key: PrivacyEncryptionKeyV1,
    /// Identifier of the ordered public output tuple.
    pub output_id: PrivacyFcmpOutputIdV1,
    /// Exact `IFCE || nonce || XChaCha20-Poly1305(FCMP note)` bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub ciphertext: Vec<u8>,
}

/// Immutable governance payload for one FCMP++ complete-output-set pool.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyFcmpPoolBootstrapV1 {
    /// Stable FCMP++ output-set identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact public asset represented by the confidential outputs.
    pub asset_definition_id: AssetDefinitionId,
    /// Non-empty complete genesis output set in strict output-identifier order.
    pub initial_outputs: Vec<PrivacyFcmpOutputTupleV1>,
}

/// Immutable governance payload for one private-IVM program pool.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyIvmPrivateNotePoolBootstrapV1 {
    /// Stable private-note pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact public asset manipulated by the private program.
    pub asset_definition_id: AssetDefinitionId,
    /// Public reserve account used by explicit value-balance bridges.
    pub reserve_account: AccountId,
    /// Exact compiled private-program digest accepted by this pool.
    pub program_id: PrivacyProgramIdV1,
    /// Non-empty genesis note set in strict commitment order.
    pub initial_note_commitments: Vec<PrivacyCommitmentV1>,
}

/// Immutable governance payload for one PQ-MASP note pool.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPqMaspPoolBootstrapV1 {
    /// Stable PQ-MASP note-pool identifier.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact public asset represented by the private notes.
    pub asset_definition_id: AssetDefinitionId,
    /// Non-empty genesis note set in strict commitment order.
    pub initial_note_commitments: Vec<PrivacyCommitmentV1>,
}

/// Closed typed bootstrap for proof-managed pools that do not use Orchard's
/// compact frontier.
///
/// Initial roots and epochs are deliberately absent. Core derives each
/// protocol's pinned root from the complete canonical genesis commitment set at
/// epoch one, preventing governance from selecting an alternate accumulator
/// origin.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "protocol", content = "bootstrap", deny_unknown_fields)
)]
pub enum PrivacyProofManagedPoolBootstrapV1 {
    /// FCMP++ complete-output-set origin.
    #[cfg_attr(feature = "json", norito(rename = "monero-fcmp-plus-plus-v1"))]
    MoneroFcmpPlusPlusV1(PrivacyFcmpPoolBootstrapV1),
    /// Native private-IVM program-state origin.
    #[cfg_attr(feature = "json", norito(rename = "iroha-ivm-private-note-stark-v1"))]
    IrohaIvmPrivateNoteStarkV1(PrivacyIvmPrivateNotePoolBootstrapV1),
    /// PQ-MASP note-commitment origin.
    #[cfg_attr(feature = "json", norito(rename = "pq-masp-stark-v0"))]
    PqMaspStarkV0(PrivacyPqMaspPoolBootstrapV1),
}

impl PrivacyProofManagedPoolBootstrapV1 {
    /// Return the exact protocol initialized by this payload.
    #[must_use]
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        match self {
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            Self::PqMaspStarkV0(_) => PrivacyProtocolIdV1::PqMaspStarkV0,
        }
    }

    /// Return the exact proof-managed root role initialized by this payload.
    #[must_use]
    pub const fn root_role(&self) -> PrivacyRootRoleV1 {
        match self {
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyRootRoleV1::OutputSet,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyRootRoleV1::ProgramState,
            Self::PqMaspStarkV0(_) => PrivacyRootRoleV1::NoteCommitmentAnchor,
        }
    }

    /// Return the sole protocol-scoped namespace initialized by this payload.
    #[must_use]
    pub const fn namespace(&self) -> PrivacyNamespaceV1 {
        match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: bootstrap.pool_id,
                }),
            ),
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                PrivacyNamespaceScopeV1::PoolProgram(PrivacyPoolProgramNamespaceV1 {
                    pool_id: bootstrap.pool_id,
                    program_id: bootstrap.program_id,
                }),
            ),
            Self::PqMaspStarkV0(bootstrap) => PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::PqMaspStarkV0,
                PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                    pool_id: bootstrap.pool_id,
                }),
            ),
        }
    }

    /// Return the exact backing asset definition.
    #[must_use]
    pub const fn asset_definition_id(&self) -> &AssetDefinitionId {
        match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => &bootstrap.asset_definition_id,
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => &bootstrap.asset_definition_id,
            Self::PqMaspStarkV0(bootstrap) => &bootstrap.asset_definition_id,
        }
    }

    /// Return the public reserve account when the protocol supports an
    /// explicit value-balance bridge.
    #[must_use]
    pub const fn reserve_account(&self) -> Option<&AccountId> {
        match self {
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => Some(&bootstrap.reserve_account),
            Self::MoneroFcmpPlusPlusV1(_) | Self::PqMaspStarkV0(_) => None,
        }
    }

    /// Return the pinned private-program digest for private-IVM pools.
    #[must_use]
    pub const fn program_id(&self) -> Option<PrivacyProgramIdV1> {
        match self {
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => Some(bootstrap.program_id),
            Self::MoneroFcmpPlusPlusV1(_) | Self::PqMaspStarkV0(_) => None,
        }
    }

    /// Return the complete canonical genesis note-commitment set.
    ///
    /// FCMP++ uses full typed output tuples and therefore returns `None`.
    #[must_use]
    pub fn initial_note_commitments(&self) -> Option<&[PrivacyCommitmentV1]> {
        match self {
            Self::MoneroFcmpPlusPlusV1(_) => None,
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => {
                Some(&bootstrap.initial_note_commitments)
            }
            Self::PqMaspStarkV0(bootstrap) => Some(&bootstrap.initial_note_commitments),
        }
    }

    /// Return the complete canonical FCMP++ genesis output set.
    #[must_use]
    pub fn initial_fcmp_outputs(&self) -> Option<&[PrivacyFcmpOutputTupleV1]> {
        match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => Some(&bootstrap.initial_outputs),
            Self::IrohaIvmPrivateNoteStarkV1(_) | Self::PqMaspStarkV0(_) => None,
        }
    }

    /// Validate the exact closed namespace and required non-zero identifiers.
    ///
    /// # Errors
    ///
    /// Rejects zero pool/program identifiers or a malformed derived namespace.
    pub fn validate(&self) -> Result<(), PrivacyProofManagedPoolBootstrapValidationErrorV1> {
        let pool_id = match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => bootstrap.pool_id,
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => {
                if bootstrap.program_id.is_zero() {
                    return Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::ZeroProgramId);
                }
                bootstrap.pool_id
            }
            Self::PqMaspStarkV0(bootstrap) => bootstrap.pool_id,
        };
        if pool_id.is_zero() {
            return Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::ZeroPoolId);
        }
        match self {
            Self::MoneroFcmpPlusPlusV1(bootstrap) => {
                if bootstrap.initial_outputs.is_empty() {
                    return Err(
                        PrivacyProofManagedPoolBootstrapValidationErrorV1::EmptyInitialFcmpOutputs,
                    );
                }
                if bootstrap.initial_outputs.len() > PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1 {
                    return Err(
                        PrivacyProofManagedPoolBootstrapValidationErrorV1::TooManyInitialFcmpOutputs {
                            count: bootstrap.initial_outputs.len(),
                            max: PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1,
                        },
                    );
                }
                let mut previous = None;
                for (index, output) in bootstrap.initial_outputs.iter().copied().enumerate() {
                    output.validate_nonzero().map_err(|source| {
                        PrivacyProofManagedPoolBootstrapValidationErrorV1::InvalidInitialFcmpOutput {
                            index,
                            source,
                        }
                    })?;
                    let output_id = output.output_id();
                    if previous.is_some_and(|value| value >= output_id) {
                        return Err(
                            PrivacyProofManagedPoolBootstrapValidationErrorV1::InitialFcmpOutputIdsNotStrictlyIncreasing {
                                index,
                            },
                        );
                    }
                    previous = Some(output_id);
                }
            }
            Self::IrohaIvmPrivateNoteStarkV1(bootstrap) => {
                validate_initial_note_commitments(&bootstrap.initial_note_commitments)?;
            }
            Self::PqMaspStarkV0(bootstrap) => {
                validate_initial_note_commitments(&bootstrap.initial_note_commitments)?;
            }
        }
        let namespace = self.namespace();
        namespace
            .validate()
            .map_err(PrivacyProofManagedPoolBootstrapValidationErrorV1::Namespace)?;
        if !self.root_role().is_compatible_with_namespace(namespace) {
            return Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::IncompatibleRootRole);
        }
        Ok(())
    }

    /// Hash the exact canonical bootstrap in its own provenance domain.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyProofManagedPoolBootstrapDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PROOF_MANAGED_POOL_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyProofManagedPoolBootstrapDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}

fn validate_initial_note_commitments(
    commitments: &[PrivacyCommitmentV1],
) -> Result<(), PrivacyProofManagedPoolBootstrapValidationErrorV1> {
    if commitments.is_empty() {
        return Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::EmptyInitialCommitments);
    }
    if commitments.len() > PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1 {
        return Err(
            PrivacyProofManagedPoolBootstrapValidationErrorV1::TooManyInitialCommitments {
                count: commitments.len(),
                max: PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1,
            },
        );
    }
    let mut previous = None;
    for (index, commitment) in commitments.iter().copied().enumerate() {
        if commitment.is_zero() {
            return Err(
                PrivacyProofManagedPoolBootstrapValidationErrorV1::ZeroInitialCommitment { index },
            );
        }
        if previous.is_some_and(|value| value >= commitment) {
            return Err(
                PrivacyProofManagedPoolBootstrapValidationErrorV1::InitialCommitmentsNotStrictlyIncreasing {
                    index,
                },
            );
        }
        previous = Some(commitment);
    }
    Ok(())
}

/// Structural failure for [`PrivacyProofManagedPoolBootstrapV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyProofManagedPoolBootstrapValidationErrorV1 {
    /// The stable pool identifier is all zero.
    #[error("proof-managed privacy pool bootstrap pool id must be non-zero")]
    ZeroPoolId,
    /// The private-IVM program digest is all zero.
    #[error("private-IVM pool bootstrap program id must be non-zero")]
    ZeroProgramId,
    /// No complete FCMP++ genesis output was supplied.
    #[error("FCMP++ pool bootstrap requires at least one initial output tuple")]
    EmptyInitialFcmpOutputs,
    /// The FCMP++ genesis output set exceeds the hard first-release bound.
    #[error("FCMP++ pool bootstrap has {count} initial outputs; maximum is {max}")]
    TooManyInitialFcmpOutputs {
        /// Observed output count.
        count: usize,
        /// Hard first-release maximum.
        max: usize,
    },
    /// One FCMP++ genesis tuple has a structurally invalid component.
    #[error("FCMP++ pool bootstrap output {index} is invalid: {source}")]
    InvalidInitialFcmpOutput {
        /// Zero-based output position.
        index: usize,
        /// Exact structural tuple failure.
        source: PrivacyFcmpOutputTupleValidationErrorV1,
    },
    /// FCMP++ genesis output identifiers contain a duplicate or are reordered.
    #[error("FCMP++ pool bootstrap output ids must be strictly increasing at index {index}")]
    InitialFcmpOutputIdsNotStrictlyIncreasing {
        /// First non-increasing output position.
        index: usize,
    },
    /// No genesis commitment was supplied.
    #[error("proof-managed privacy pool bootstrap requires at least one initial commitment")]
    EmptyInitialCommitments,
    /// The genesis commitment set exceeds the hard first-release bound.
    #[error(
        "proof-managed privacy pool bootstrap has {count} initial commitments; maximum is {max}"
    )]
    TooManyInitialCommitments {
        /// Observed commitment count.
        count: usize,
        /// Hard first-release maximum.
        max: usize,
    },
    /// One genesis commitment is the all-zero sentinel.
    #[error("proof-managed privacy pool bootstrap commitment {index} must be non-zero")]
    ZeroInitialCommitment {
        /// Zero-based commitment index.
        index: usize,
    },
    /// Genesis commitments are duplicated or reordered.
    #[error(
        "proof-managed privacy pool bootstrap commitments stop increasing strictly at index {index}"
    )]
    InitialCommitmentsNotStrictlyIncreasing {
        /// First invalid zero-based position.
        index: usize,
    },
    /// The derived closed namespace is malformed.
    #[error("proof-managed privacy pool bootstrap namespace is invalid: {0}")]
    Namespace(PrivacyNamespaceValidationError),
    /// The inferred root role is incompatible with the exact namespace.
    #[error("proof-managed privacy pool bootstrap root role is incompatible")]
    IncompatibleRootRole,
}

/// One canonical encrypted account in a PGC account-state bootstrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPgcAccountV1 {
    /// Canonical compressed P-256 account public key.
    pub public_key: PrivacyP256PointV1,
    /// Initial twisted-ElGamal encrypted balance.
    pub encrypted_balance: PrivacyP256CiphertextV1,
}

/// Point position selected by PGC bootstrap validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyPgcAccountPointV1 {
    /// Account public key.
    PublicKey,
    /// Encrypted-balance left component.
    EncryptedBalanceLeft,
    /// Encrypted-balance right component.
    EncryptedBalanceRight,
}

/// Exact canonical native proof for an Anonymous PGC account bootstrap.
///
/// This proof has a dedicated wire type and a tighter first-release bound than
/// ordinary privacy actions. The native verifier must additionally perform an
/// exact decode and require byte-for-byte canonical re-encoding before core
/// derives [`PrivacyPgcBootstrapProofDigestV1`] for persisted provenance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct PrivacyPgcBootstrapProofBytesV1 {
    /// Exact native proof encoding.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub bytes: Vec<u8>,
}

impl PrivacyPgcBootstrapProofBytesV1 {
    /// Construct proof bytes for subsequent native validation.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Borrow the exact proof bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Validate presence, non-degeneracy, and the fixed Taira byte cap.
    ///
    /// # Errors
    ///
    /// Rejects an empty, all-zero, unrepresentable, or oversized payload.
    pub fn validate(&self) -> Result<(), PrivacyPgcBootstrapProofValidationError> {
        if self.bytes.is_empty() {
            return Err(PrivacyPgcBootstrapProofValidationError::Empty);
        }
        if self.bytes.iter().all(|byte| *byte == 0) {
            return Err(PrivacyPgcBootstrapProofValidationError::AllZero);
        }
        let len = u64::try_from(self.bytes.len())
            .map_err(|_| PrivacyPgcBootstrapProofValidationError::LengthOverflow)?;
        if len > u64::from(TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1) {
            return Err(PrivacyPgcBootstrapProofValidationError::TooLarge {
                bytes: len,
                max: TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,
            });
        }
        Ok(())
    }

    /// Derive the audit digest of these exact proof bytes.
    ///
    /// Callers admitting a proof must invoke this only after the native
    /// verifier has performed exact decode and byte-for-byte canonical
    /// re-encoding. The method repeats structural validation and never accepts
    /// a caller-supplied digest.
    ///
    /// # Errors
    ///
    /// Returns the same failures as [`Self::validate`].
    pub fn digest(
        &self,
    ) -> Result<PrivacyPgcBootstrapProofDigestV1, PrivacyPgcBootstrapProofValidationError> {
        self.validate()?;
        let len = u64::try_from(self.bytes.len())
            .map_err(|_| PrivacyPgcBootstrapProofValidationError::LengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PGC_BOOTSTRAP_PROOF_DIGEST_DOMAIN_V1);
        hasher.update(&len.to_le_bytes());
        hasher.update(&self.bytes);
        Ok(PrivacyPgcBootstrapProofDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}

/// Structural failure for [`PrivacyPgcBootstrapProofBytesV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyPgcBootstrapProofValidationError {
    /// Proof payload is absent.
    #[error("PGC bootstrap proof bytes must not be empty")]
    Empty,
    /// Proof payload is degenerate.
    #[error("PGC bootstrap proof bytes must not be all zero")]
    AllZero,
    /// Proof payload exceeds the fixed first-release cap.
    #[error("PGC bootstrap proof uses {bytes} bytes, exceeding maximum {max}")]
    TooLarge {
        /// Observed byte length.
        bytes: u64,
        /// Fixed maximum byte length.
        max: u32,
    },
    /// Platform collection length cannot be represented canonically.
    #[error("PGC bootstrap proof length exceeds u64")]
    LengthOverflow,
}

/// Governed bootstrap payload for a complete PGC encrypted account table.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyPgcAccountBootstrapV1 {
    /// Exact Anonymous PGC pool namespace.
    pub namespace: PrivacyNamespaceV1,
    /// Declared root, which core must recompute from `accounts`.
    pub initial_root: PrivacyRootV1,
    /// Canonical initial account-state epoch (exactly
    /// [`PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1`]).
    pub initial_epoch: u64,
    /// Exact public aggregate supply encrypted across the initial accounts.
    pub total_supply: u32,
    /// Complete account table in strict public-key order.
    pub accounts: Vec<PrivacyPgcAccountV1>,
}

impl PrivacyPgcAccountBootstrapV1 {
    /// Validate the closed namespace, size, ordering, and nonzero wire values.
    ///
    /// Core must additionally recompute `initial_root` from the canonical
    /// entries under [`PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1`] before
    /// admitting this payload.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyPgcAccountBootstrapValidationError`] for any malformed
    /// bootstrap field.
    pub fn validate(&self) -> Result<(), PrivacyPgcAccountBootstrapValidationError> {
        self.namespace
            .validate()
            .map_err(PrivacyPgcAccountBootstrapValidationError::Namespace)?;
        if self.namespace.protocol_id() != PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
            return Err(PrivacyPgcAccountBootstrapValidationError::WrongProtocol {
                protocol_id: self.namespace.protocol_id(),
            });
        }
        if self.initial_root.is_zero() {
            return Err(PrivacyPgcAccountBootstrapValidationError::ZeroRoot);
        }
        if self.initial_epoch != PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1 {
            return Err(
                PrivacyPgcAccountBootstrapValidationError::NonCanonicalInitialEpoch {
                    epoch: self.initial_epoch,
                },
            );
        }
        if self.total_supply == 0 {
            return Err(PrivacyPgcAccountBootstrapValidationError::ZeroTotalSupply);
        }
        let account_count = u32::try_from(self.accounts.len())
            .map_err(|_| PrivacyPgcAccountBootstrapValidationError::AccountLengthOverflow)?;
        if !ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1.contains(&account_count) {
            return Err(
                PrivacyPgcAccountBootstrapValidationError::InvalidAccountCount {
                    count: account_count,
                },
            );
        }
        for (index, account) in self.accounts.iter().enumerate() {
            let encoded_index = u32::try_from(index)
                .map_err(|_| PrivacyPgcAccountBootstrapValidationError::AccountLengthOverflow)?;
            if account.public_key.is_zero() {
                return Err(PrivacyPgcAccountBootstrapValidationError::ZeroPoint {
                    index: encoded_index,
                    point: PrivacyPgcAccountPointV1::PublicKey,
                });
            }
            if account.encrypted_balance.left.is_zero() {
                return Err(PrivacyPgcAccountBootstrapValidationError::ZeroPoint {
                    index: encoded_index,
                    point: PrivacyPgcAccountPointV1::EncryptedBalanceLeft,
                });
            }
            if account.encrypted_balance.right.is_zero() {
                return Err(PrivacyPgcAccountBootstrapValidationError::ZeroPoint {
                    index: encoded_index,
                    point: PrivacyPgcAccountPointV1::EncryptedBalanceRight,
                });
            }
            if index > 0 && self.accounts[index - 1].public_key >= account.public_key {
                return Err(PrivacyPgcAccountBootstrapValidationError::KeysNotStrictlyIncreasing);
            }
        }
        Ok(())
    }

    /// Hash this bootstrap payload in its distinct provenance domain.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical encoding fails.
    pub fn digest(&self) -> Result<PrivacyPgcAccountBootstrapDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PGC_ACCOUNT_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyPgcAccountBootstrapDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }
}

/// Validation failure for [`PrivacyPgcAccountBootstrapV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyPgcAccountBootstrapValidationError {
    /// Namespace is malformed.
    #[error("PGC bootstrap namespace is invalid: {0}")]
    Namespace(PrivacyNamespaceValidationError),
    /// Namespace belongs to another protocol.
    #[error("PGC bootstrap namespace uses protocol {protocol_id:?}")]
    WrongProtocol {
        /// Unexpected namespace protocol.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// Declared initial root is zero.
    #[error("PGC bootstrap initial root must be non-zero")]
    ZeroRoot,
    /// Declared initial epoch differs from the closed first-release origin.
    #[error("PGC bootstrap initial epoch must be 1, got {epoch}")]
    NonCanonicalInitialEpoch {
        /// Rejected caller-provided epoch.
        epoch: u64,
    },
    /// Declared aggregate supply is zero.
    #[error("PGC bootstrap total supply must be non-zero")]
    ZeroTotalSupply,
    /// Account count is not one of the closed profile sizes.
    #[error("PGC bootstrap account count {count} is not one of 16, 32, or 64")]
    InvalidAccountCount {
        /// Observed account count.
        count: u32,
    },
    /// A P-256 point placeholder is all zero.
    #[error("PGC bootstrap account {index} point {point:?} must be non-zero")]
    ZeroPoint {
        /// Zero-based account index.
        index: u32,
        /// Invalid point role.
        point: PrivacyPgcAccountPointV1,
    },
    /// Account public keys are duplicated or not canonically sorted.
    #[error("PGC bootstrap account public keys must be strictly increasing")]
    KeysNotStrictlyIncreasing,
    /// Platform collection length cannot be represented canonically.
    #[error("PGC bootstrap account length exceeds u32")]
    AccountLengthOverflow,
}

/// Field within [`PrivacyConsensusLimitsV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyLimitFieldV1 {
    /// Actions per transaction.
    ActionsPerTransaction,
    /// Actions per block.
    ActionsPerBlock,
    /// Proof bytes per action.
    ProofBytesPerAction,
    /// Encoded action bytes.
    ActionBytes,
    /// Privacy bytes per transaction.
    PrivacyBytesPerTransaction,
    /// Privacy bytes per block.
    PrivacyBytesPerBlock,
    /// Public statement and encrypted-output bytes per transaction.
    StatementAndEncryptedOutputBytesPerTransaction,
    /// Nullifiers per action.
    NullifiersPerAction,
    /// Commitments per action.
    CommitmentsPerAction,
    /// Retained recent roots.
    RetainedRootCount,
}

/// Consensus-enforced privacy resource limits.
///
/// The first release permits governance to lower these values but not exceed
/// the Taira hard ceilings. Raising a ceiling requires an explicit data-model
/// release so old validators cannot silently admit a larger resource surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyConsensusLimitsV1 {
    /// Maximum privacy actions in one transaction.
    pub max_actions_per_transaction: u32,
    /// Maximum privacy actions in one block.
    pub max_actions_per_block: u32,
    /// Maximum proof payload bytes in one action.
    pub max_proof_bytes_per_action: u32,
    /// Maximum encoded bytes in one action.
    pub max_action_bytes: u32,
    /// Maximum privacy bytes in one transaction.
    pub max_privacy_bytes_per_transaction: u32,
    /// Maximum privacy bytes in one block.
    pub max_privacy_bytes_per_block: u32,
    /// Maximum public-input and encrypted-output bytes in one transaction.
    pub max_statement_and_encrypted_output_bytes_per_transaction: u32,
    /// Maximum nullifiers emitted by one action.
    pub max_nullifiers_per_action: u32,
    /// Maximum commitments emitted by one action.
    pub max_commitments_per_action: u32,
    /// Number of recent commitment roots retained for proof admission.
    pub retained_root_count: u32,
}

impl PrivacyConsensusLimitsV1 {
    /// Return the approved first-release Taira profile.
    #[must_use]
    pub const fn taira_default() -> Self {
        Self {
            max_actions_per_transaction: TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1,
            max_actions_per_block: TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1,
            max_proof_bytes_per_action: TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
            max_action_bytes: TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
            max_privacy_bytes_per_transaction: TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1,
            max_privacy_bytes_per_block: TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1,
            max_statement_and_encrypted_output_bytes_per_transaction:
                TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1,
            max_nullifiers_per_action: TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1,
            max_commitments_per_action: TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1,
            retained_root_count: TAIRA_PRIVACY_RETAINED_ROOT_COUNT_V1,
        }
    }

    /// Validate non-zero, hard-ceiling, and cross-field ordering invariants.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyConsensusLimitsValidationError`] for the first invalid
    /// field or relationship in deterministic field order.
    pub fn validate(&self) -> Result<(), PrivacyConsensusLimitsValidationError> {
        let fields = [
            (
                PrivacyLimitFieldV1::ActionsPerTransaction,
                self.max_actions_per_transaction,
                TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::ActionsPerBlock,
                self.max_actions_per_block,
                TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1,
            ),
            (
                PrivacyLimitFieldV1::ProofBytesPerAction,
                self.max_proof_bytes_per_action,
                TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::ActionBytes,
                self.max_action_bytes,
                TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
                self.max_privacy_bytes_per_transaction,
                TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerBlock,
                self.max_privacy_bytes_per_block,
                TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1,
            ),
            (
                PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
                self.max_statement_and_encrypted_output_bytes_per_transaction,
                TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::NullifiersPerAction,
                self.max_nullifiers_per_action,
                TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::CommitmentsPerAction,
                self.max_commitments_per_action,
                TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1,
            ),
            (
                PrivacyLimitFieldV1::RetainedRootCount,
                self.retained_root_count,
                TAIRA_PRIVACY_RETAINED_ROOT_COUNT_V1,
            ),
        ];

        for (field, value, hard_max) in fields {
            if value == 0 {
                return Err(PrivacyConsensusLimitsValidationError::Zero { field });
            }
            if value > hard_max {
                return Err(PrivacyConsensusLimitsValidationError::ExceedsHardMaximum {
                    field,
                    value,
                    hard_max,
                });
            }
        }

        validate_limit_order(
            PrivacyLimitFieldV1::ActionsPerTransaction,
            self.max_actions_per_transaction,
            PrivacyLimitFieldV1::ActionsPerBlock,
            self.max_actions_per_block,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::ProofBytesPerAction,
            self.max_proof_bytes_per_action,
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
            PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
            self.max_privacy_bytes_per_transaction,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
            self.max_privacy_bytes_per_transaction,
            PrivacyLimitFieldV1::PrivacyBytesPerBlock,
            self.max_privacy_bytes_per_block,
        )?;
        validate_limit_order(
            PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
            self.max_statement_and_encrypted_output_bytes_per_transaction,
            PrivacyLimitFieldV1::ActionBytes,
            self.max_action_bytes,
        )?;
        Ok(())
    }

    /// Validate `next` as a strict component-wise tightening of this policy.
    ///
    /// # Errors
    ///
    /// Rejects invalid profiles, any increased component, and a no-op update.
    pub fn validate_tightening_to(
        &self,
        next: &Self,
    ) -> Result<(), PrivacyConsensusLimitsTighteningErrorV1> {
        self.validate()
            .map_err(PrivacyConsensusLimitsTighteningErrorV1::InvalidCurrent)?;
        next.validate()
            .map_err(PrivacyConsensusLimitsTighteningErrorV1::InvalidNext)?;

        let fields = [
            (
                PrivacyLimitFieldV1::ActionsPerTransaction,
                self.max_actions_per_transaction,
                next.max_actions_per_transaction,
            ),
            (
                PrivacyLimitFieldV1::ActionsPerBlock,
                self.max_actions_per_block,
                next.max_actions_per_block,
            ),
            (
                PrivacyLimitFieldV1::ProofBytesPerAction,
                self.max_proof_bytes_per_action,
                next.max_proof_bytes_per_action,
            ),
            (
                PrivacyLimitFieldV1::ActionBytes,
                self.max_action_bytes,
                next.max_action_bytes,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
                self.max_privacy_bytes_per_transaction,
                next.max_privacy_bytes_per_transaction,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerBlock,
                self.max_privacy_bytes_per_block,
                next.max_privacy_bytes_per_block,
            ),
            (
                PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
                self.max_statement_and_encrypted_output_bytes_per_transaction,
                next.max_statement_and_encrypted_output_bytes_per_transaction,
            ),
            (
                PrivacyLimitFieldV1::NullifiersPerAction,
                self.max_nullifiers_per_action,
                next.max_nullifiers_per_action,
            ),
            (
                PrivacyLimitFieldV1::CommitmentsPerAction,
                self.max_commitments_per_action,
                next.max_commitments_per_action,
            ),
            (
                PrivacyLimitFieldV1::RetainedRootCount,
                self.retained_root_count,
                next.retained_root_count,
            ),
        ];
        for (field, current, candidate) in fields {
            if candidate > current {
                return Err(PrivacyConsensusLimitsTighteningErrorV1::Increase {
                    field,
                    current,
                    candidate,
                });
            }
        }
        if self == next {
            return Err(PrivacyConsensusLimitsTighteningErrorV1::NoChange);
        }
        Ok(())
    }
}

impl Default for PrivacyConsensusLimitsV1 {
    fn default() -> Self {
        Self::taira_default()
    }
}

fn validate_limit_order(
    smaller_field: PrivacyLimitFieldV1,
    smaller_value: u32,
    larger_field: PrivacyLimitFieldV1,
    larger_value: u32,
) -> Result<(), PrivacyConsensusLimitsValidationError> {
    if smaller_value > larger_value {
        return Err(PrivacyConsensusLimitsValidationError::InconsistentOrder {
            smaller_field,
            smaller_value,
            larger_field,
            larger_value,
        });
    }
    Ok(())
}

/// Validation failure for [`PrivacyConsensusLimitsV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyConsensusLimitsValidationError {
    /// A consensus limit is zero.
    #[error("privacy limit {field:?} must be non-zero")]
    Zero {
        /// Invalid field.
        field: PrivacyLimitFieldV1,
    },
    /// A consensus limit exceeds its first-release hard maximum.
    #[error("privacy limit {field:?} value {value} exceeds hard maximum {hard_max}")]
    ExceedsHardMaximum {
        /// Invalid field.
        field: PrivacyLimitFieldV1,
        /// Configured value.
        value: u32,
        /// First-release hard maximum.
        hard_max: u32,
    },
    /// A smaller-scope resource limit exceeds its containing scope.
    #[error(
        "privacy limit {smaller_field:?} value {smaller_value} exceeds {larger_field:?} value {larger_value}"
    )]
    InconsistentOrder {
        /// Field that must not exceed the containing field.
        smaller_field: PrivacyLimitFieldV1,
        /// Value of the smaller-scope field.
        smaller_value: u32,
        /// Containing field.
        larger_field: PrivacyLimitFieldV1,
        /// Value of the containing field.
        larger_value: u32,
    },
}

/// Validation failure for a component-wise consensus-policy tightening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyConsensusLimitsTighteningErrorV1 {
    /// The currently persisted policy is malformed.
    #[error("current privacy consensus limits are invalid: {0}")]
    InvalidCurrent(PrivacyConsensusLimitsValidationError),
    /// The proposed successor policy is malformed.
    #[error("next privacy consensus limits are invalid: {0}")]
    InvalidNext(PrivacyConsensusLimitsValidationError),
    /// A purported tightening increases one component.
    #[error(
        "privacy limit {field:?} cannot increase from {current} to {candidate} in a tightening"
    )]
    Increase {
        /// Increased component.
        field: PrivacyLimitFieldV1,
        /// Current component value.
        current: u32,
        /// Rejected successor value.
        candidate: u32,
    },
    /// A tightening must change at least one component.
    #[error("privacy consensus policy tightening is a no-op")]
    NoChange,
}

/// Scheduled successor for the singleton chain-wide privacy policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyConsensusPolicyTighteningV1 {
    /// Exact block which admitted this schedule.
    pub scheduled_at_height: u64,
    /// Exact incoming block at whose start the successor becomes effective.
    pub effective_at_height: u64,
    /// Complete component-wise-lower successor policy.
    pub next_limits: PrivacyConsensusLimitsV1,
}

impl PrivacyConsensusPolicyTighteningV1 {
    /// Validate schedule timing and component-wise monotonicity.
    ///
    /// # Errors
    ///
    /// Rejects zero/overflowing heights, insufficient notice, an invalid
    /// successor, any increase, or a no-op.
    pub fn validate_against(
        &self,
        current_limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyPolicyValidationErrorV1> {
        validate_privacy_policy_schedule_heights_v1(
            self.scheduled_at_height,
            self.effective_at_height,
        )?;
        current_limits
            .validate_tightening_to(&self.next_limits)
            .map_err(PrivacyPolicyValidationErrorV1::ConsensusTightening)
    }
}

/// Singleton chain-wide privacy admission policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyConsensusPolicyV1 {
    /// Limits effective for the current committed state.
    pub current_limits: PrivacyConsensusLimitsV1,
    /// At most one delayed, component-wise tightening.
    pub pending_tightening: Option<PrivacyConsensusPolicyTighteningV1>,
}

impl PrivacyConsensusPolicyV1 {
    /// Construct the first-release Taira policy with no pending change.
    #[must_use]
    pub const fn taira_default() -> Self {
        Self {
            current_limits: PrivacyConsensusLimitsV1::taira_default(),
            pending_tightening: None,
        }
    }

    /// Validate the complete persisted policy independent of chain height.
    ///
    /// # Errors
    ///
    /// Rejects invalid current limits or a malformed pending tightening.
    pub fn validate(&self) -> Result<(), PrivacyPolicyValidationErrorV1> {
        self.current_limits
            .validate()
            .map_err(PrivacyPolicyValidationErrorV1::InvalidCurrentLimits)?;
        if let Some(pending) = self.pending_tightening {
            pending.validate_against(&self.current_limits)?;
        }
        Ok(())
    }

    /// Validate a restored policy against the latest committed block height.
    ///
    /// A pending transition at height `E` is valid in a snapshot committed at
    /// `E - 1`, and invalid in a snapshot already committed at `E`.
    ///
    /// # Errors
    ///
    /// Rejects an intrinsically invalid policy or a missed/due transition.
    pub fn validate_at_committed_height(
        &self,
        committed_height: u64,
    ) -> Result<(), PrivacyPolicyValidationErrorV1> {
        self.validate()?;
        if let Some(pending) = self.pending_tightening {
            if pending.scheduled_at_height > committed_height {
                return Err(
                    PrivacyPolicyValidationErrorV1::PendingScheduledAfterCommitted {
                        scheduled_at_height: pending.scheduled_at_height,
                        committed_height,
                    },
                );
            }
            if pending.effective_at_height <= committed_height {
                return Err(PrivacyPolicyValidationErrorV1::PendingNotFuture {
                    effective_at_height: pending.effective_at_height,
                    committed_height,
                });
            }
        }
        Ok(())
    }

    /// Root-retention cap enforced while admitting new roots.
    ///
    /// During the notice window new histories must already satisfy the pending
    /// lower limit so the effective-height transition is deterministic.
    #[must_use]
    pub const fn admission_retained_root_count(&self) -> u32 {
        match self.pending_tightening {
            Some(pending)
                if pending.next_limits.retained_root_count
                    < self.current_limits.retained_root_count =>
            {
                pending.next_limits.retained_root_count
            }
            _ => self.current_limits.retained_root_count,
        }
    }
}

impl Default for PrivacyConsensusPolicyV1 {
    fn default() -> Self {
        Self::taira_default()
    }
}

/// Validation failure for a singleton privacy-policy value or schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyPolicyValidationErrorV1 {
    /// Current limits are malformed.
    #[error("current privacy policy limits are invalid: {0}")]
    InvalidCurrentLimits(PrivacyConsensusLimitsValidationError),
    /// A scheduled consensus tightening is invalid.
    #[error("privacy consensus limits tightening is invalid: {0}")]
    ConsensusTightening(PrivacyConsensusLimitsTighteningErrorV1),
    /// Schedule admission height must be a real block height.
    #[error("privacy policy scheduled-at height must be non-zero")]
    ZeroScheduledHeight,
    /// Effective height must be strictly later than schedule admission.
    #[error(
        "privacy policy effective height {effective_at_height} must be later than scheduled height {scheduled_at_height}"
    )]
    EffectiveNotLater {
        /// Exact admission height.
        scheduled_at_height: u64,
        /// Rejected effective height.
        effective_at_height: u64,
    },
    /// The schedule does not provide the consensus minimum notice.
    #[error(
        "privacy policy effective height {effective_at_height} is earlier than minimum {earliest_effective_height}"
    )]
    LeadTimeTooShort {
        /// Rejected effective height.
        effective_at_height: u64,
        /// Earliest admissible effective height.
        earliest_effective_height: u64,
    },
    /// Adding the minimum notice overflows the height domain.
    #[error("privacy policy schedule height overflow")]
    HeightOverflow,
    /// A restored schedule claims admission after the snapshot it inhabits.
    #[error(
        "privacy policy scheduled-at height {scheduled_at_height} is after committed height {committed_height}"
    )]
    PendingScheduledAfterCommitted {
        /// Persisted admission height.
        scheduled_at_height: u64,
        /// Latest committed height.
        committed_height: u64,
    },
    /// A restored state retained a schedule which is already due or missed.
    #[error(
        "privacy policy effective height {effective_at_height} is not after committed height {committed_height}"
    )]
    PendingNotFuture {
        /// Persisted effective height.
        effective_at_height: u64,
        /// Latest committed height.
        committed_height: u64,
    },
}

fn validate_privacy_policy_schedule_heights_v1(
    scheduled_at_height: u64,
    effective_at_height: u64,
) -> Result<(), PrivacyPolicyValidationErrorV1> {
    if scheduled_at_height == 0 {
        return Err(PrivacyPolicyValidationErrorV1::ZeroScheduledHeight);
    }
    if effective_at_height <= scheduled_at_height {
        return Err(PrivacyPolicyValidationErrorV1::EffectiveNotLater {
            scheduled_at_height,
            effective_at_height,
        });
    }
    let earliest_effective_height = scheduled_at_height
        .checked_add(MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1)
        .ok_or(PrivacyPolicyValidationErrorV1::HeightOverflow)?;
    if effective_at_height < earliest_effective_height {
        return Err(PrivacyPolicyValidationErrorV1::LeadTimeTooShort {
            effective_at_height,
            earliest_effective_height,
        });
    }
    Ok(())
}

/// Proposed lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyProposedLifecycleV1 {
    /// Height at which the proposal became canonical.
    pub proposed_at_height: u64,
    /// Scheduled first active height.
    pub activate_at_height: u64,
}

/// Active lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyActiveLifecycleV1 {
    /// Height at which the proposal became canonical.
    pub proposed_at_height: u64,
    /// Height at which the protocol was first activated.
    pub activated_at_height: u64,
    /// Height at which the current active interval began.
    pub state_since_height: u64,
}

/// Suspended lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacySuspendedLifecycleV1 {
    /// Height at which the proposal became canonical.
    pub proposed_at_height: u64,
    /// Height at which the protocol was first activated.
    pub activated_at_height: u64,
    /// Height at which the current suspension began.
    pub state_since_height: u64,
}

/// Retired lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyRetiredLifecycleV1 {
    /// Height at which the proposal became canonical.
    pub proposed_at_height: u64,
    /// First activation height, or `None` if retired before activation.
    pub activated_at_height: Option<u64>,
    /// Height at which retirement became effective.
    pub state_since_height: u64,
}

/// Governed lifecycle of a protocol activation record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "record", deny_unknown_fields)
)]
pub enum PrivacyProtocolLifecycleV1 {
    /// Governance approved a future activation height.
    #[cfg_attr(feature = "json", norito(rename = "proposed"))]
    Proposed(PrivacyProposedLifecycleV1),
    /// The protocol is currently active.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active(PrivacyActiveLifecycleV1),
    /// The protocol is temporarily fail-closed.
    #[cfg_attr(feature = "json", norito(rename = "suspended"))]
    Suspended(PrivacySuspendedLifecycleV1),
    /// The protocol is permanently unavailable.
    #[cfg_attr(feature = "json", norito(rename = "retired"))]
    Retired(PrivacyRetiredLifecycleV1),
}

impl PrivacyProtocolLifecycleV1 {
    /// Validate internal height ordering.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyLifecycleValidationError`] when a transition height is
    /// zero, equal to, or earlier than the height that must precede it.
    pub fn validate(&self) -> Result<(), PrivacyLifecycleValidationError> {
        match *self {
            Self::Proposed(state) => validate_strictly_later(
                PrivacyLifecycleHeightFieldV1::Proposed,
                state.proposed_at_height,
                PrivacyLifecycleHeightFieldV1::Activated,
                state.activate_at_height,
            ),
            Self::Active(state) => {
                validate_strictly_later(
                    PrivacyLifecycleHeightFieldV1::Proposed,
                    state.proposed_at_height,
                    PrivacyLifecycleHeightFieldV1::Activated,
                    state.activated_at_height,
                )?;
                if state.state_since_height < state.activated_at_height {
                    return Err(PrivacyLifecycleValidationError::HeightOrder {
                        earlier_field: PrivacyLifecycleHeightFieldV1::Activated,
                        earlier_height: state.activated_at_height,
                        later_field: PrivacyLifecycleHeightFieldV1::StateSince,
                        later_height: state.state_since_height,
                    });
                }
                Ok(())
            }
            Self::Suspended(state) => {
                validate_strictly_later(
                    PrivacyLifecycleHeightFieldV1::Proposed,
                    state.proposed_at_height,
                    PrivacyLifecycleHeightFieldV1::Activated,
                    state.activated_at_height,
                )?;
                validate_strictly_later(
                    PrivacyLifecycleHeightFieldV1::Activated,
                    state.activated_at_height,
                    PrivacyLifecycleHeightFieldV1::StateSince,
                    state.state_since_height,
                )
            }
            Self::Retired(state) => {
                if let Some(activated_at_height) = state.activated_at_height {
                    validate_strictly_later(
                        PrivacyLifecycleHeightFieldV1::Proposed,
                        state.proposed_at_height,
                        PrivacyLifecycleHeightFieldV1::Activated,
                        activated_at_height,
                    )?;
                    validate_strictly_later(
                        PrivacyLifecycleHeightFieldV1::Activated,
                        activated_at_height,
                        PrivacyLifecycleHeightFieldV1::StateSince,
                        state.state_since_height,
                    )
                } else {
                    validate_strictly_later(
                        PrivacyLifecycleHeightFieldV1::Proposed,
                        state.proposed_at_height,
                        PrivacyLifecycleHeightFieldV1::StateSince,
                        state.state_since_height,
                    )
                }
            }
        }
    }

    /// Return `true` only for the active lifecycle state.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active(_))
    }

    /// Return whether `next` is a valid forward lifecycle transition.
    #[must_use]
    pub fn can_transition_to(&self, next: &Self) -> bool {
        self.validate_transition_to(next).is_ok()
    }

    /// Validate a forward lifecycle transition and its immutable history.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyLifecycleTransitionError`] for invalid states,
    /// unsupported edges, mismatched proposal/activation history, or a
    /// non-increasing transition height.
    pub fn validate_transition_to(
        &self,
        next: &Self,
    ) -> Result<(), PrivacyLifecycleTransitionError> {
        self.validate()
            .map_err(PrivacyLifecycleTransitionError::CurrentState)?;
        next.validate()
            .map_err(PrivacyLifecycleTransitionError::NextState)?;

        match (*self, *next) {
            (Self::Proposed(current), Self::Active(next))
                if current.proposed_at_height == next.proposed_at_height
                    && current.activate_at_height == next.activated_at_height
                    && next.activated_at_height == next.state_since_height =>
            {
                Ok(())
            }
            (Self::Proposed(current), Self::Retired(next))
                if current.proposed_at_height == next.proposed_at_height
                    && next.activated_at_height.is_none()
                    && next.state_since_height <= current.activate_at_height =>
            {
                Ok(())
            }
            (Self::Active(current), Self::Suspended(next))
                if current.proposed_at_height == next.proposed_at_height
                    && current.activated_at_height == next.activated_at_height
                    && next.state_since_height > current.state_since_height =>
            {
                Ok(())
            }
            (Self::Active(current), Self::Retired(next))
                if current.proposed_at_height == next.proposed_at_height
                    && next.activated_at_height == Some(current.activated_at_height)
                    && next.state_since_height > current.state_since_height =>
            {
                Ok(())
            }
            (Self::Suspended(current), Self::Active(next))
                if current.proposed_at_height == next.proposed_at_height
                    && current.activated_at_height == next.activated_at_height
                    && next.state_since_height > current.state_since_height =>
            {
                Ok(())
            }
            (Self::Suspended(current), Self::Retired(next))
                if current.proposed_at_height == next.proposed_at_height
                    && next.activated_at_height == Some(current.activated_at_height)
                    && next.state_since_height > current.state_since_height =>
            {
                Ok(())
            }
            (Self::Retired(_), _) => Err(PrivacyLifecycleTransitionError::RetiredIsTerminal),
            _ => Err(PrivacyLifecycleTransitionError::InvalidTransition),
        }
    }
}

fn validate_strictly_later(
    earlier_field: PrivacyLifecycleHeightFieldV1,
    earlier_height: u64,
    later_field: PrivacyLifecycleHeightFieldV1,
    later_height: u64,
) -> Result<(), PrivacyLifecycleValidationError> {
    if earlier_height == 0 {
        return Err(PrivacyLifecycleValidationError::ZeroHeight {
            field: earlier_field,
        });
    }
    if later_height == 0 {
        return Err(PrivacyLifecycleValidationError::ZeroHeight { field: later_field });
    }
    if later_height <= earlier_height {
        return Err(PrivacyLifecycleValidationError::HeightOrder {
            earlier_field,
            earlier_height,
            later_field,
            later_height,
        });
    }
    Ok(())
}

/// Height field within a privacy lifecycle record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyLifecycleHeightFieldV1 {
    /// Proposal height.
    Proposed,
    /// First activation height.
    Activated,
    /// Height at which the current state began.
    StateSince,
}

/// Validation failure for one [`PrivacyProtocolLifecycleV1`] value.
#[allow(variant_size_differences)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyLifecycleValidationError {
    /// A lifecycle height is zero.
    #[error("privacy lifecycle height {field:?} must be non-zero")]
    ZeroHeight {
        /// Invalid height field.
        field: PrivacyLifecycleHeightFieldV1,
    },
    /// A later lifecycle height is not strictly later.
    #[error(
        "privacy lifecycle {later_field:?} height {later_height} must be later than {earlier_field:?} height {earlier_height}"
    )]
    HeightOrder {
        /// Earlier height field.
        earlier_field: PrivacyLifecycleHeightFieldV1,
        /// Earlier height value.
        earlier_height: u64,
        /// Later height field.
        later_field: PrivacyLifecycleHeightFieldV1,
        /// Later height value.
        later_height: u64,
    },
}

/// Validation failure for a lifecycle state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyLifecycleTransitionError {
    /// The current state is internally invalid.
    #[error("current privacy lifecycle state is invalid: {0}")]
    CurrentState(PrivacyLifecycleValidationError),
    /// The proposed next state is internally invalid.
    #[error("next privacy lifecycle state is invalid: {0}")]
    NextState(PrivacyLifecycleValidationError),
    /// The lifecycle edge or immutable history is invalid.
    #[error("privacy lifecycle transition is invalid")]
    InvalidTransition,
    /// A retired protocol cannot transition again.
    #[error("retired privacy protocol lifecycle is terminal")]
    RetiredIsTerminal,
}

/// Assurance classification for a first-release privacy activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "assurance", content = "value", deny_unknown_fields)
)]
pub enum PrivacyAssuranceV1 {
    /// Testnet-only experimental; not security-audited and not a production-readiness claim.
    #[cfg_attr(feature = "json", norito(rename = "experimental"))]
    Experimental,
}

/// Activation-specific Anonymous PGC policy limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AnonymousPgcActivationLimitsV1 {
    /// Maximum anonymity-set size `n` for this activation.
    pub max_anonymity_set_size: u32,
    /// Maximum intended recipient count `k` for this activation.
    pub max_recipient_count: u32,
}

/// Activation-specific VeRange aggregation policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct VeRangeActivationLimitsV1 {
    /// Maximum aggregation count `T` admitted by this activation.
    pub max_aggregation_count: u32,
}

/// Activation-specific ZK-AMS admission and provisioning policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ZkAmsActivationLimitsV1 {
    /// Maximum ordered admission anchors in one batch settlement.
    pub max_batch_size: u32,
    /// Maximum admitted seed-key ring size in one provisioning action.
    pub max_ring_size: u32,
}

/// Activation-specific Jindo batched univariate-opening policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct JindoActivationLimitsV1 {
    /// Maximum polynomial commitments per statement.
    pub max_polynomial_count: u32,
}

/// Activation-specific Orchard action policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct OrchardActivationLimitsV1 {
    /// Maximum one-to-one spend/output actions per statement.
    pub max_action_count: u32,
}

/// Activation-specific FCMP++ transfer policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct FcmpActivationLimitsV1 {
    /// Maximum consumed outputs per transfer.
    pub max_input_count: u32,
    /// Maximum new outputs per transfer.
    pub max_output_count: u32,
}

/// Activation-specific native IVM private-note policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct IvmPrivateNoteActivationLimitsV1 {
    /// Maximum consumed notes per action.
    pub max_input_count: u32,
    /// Maximum new notes per action.
    pub max_output_count: u32,
}

/// Activation-specific PQ-MASP policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PqMaspActivationLimitsV1 {
    /// Maximum consumed notes per action.
    pub max_input_count: u32,
    /// Maximum new notes per action.
    pub max_output_count: u32,
}

/// Protocol-specific governed limits carried by an activation record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "protocol", content = "limits", deny_unknown_fields)
)]
pub enum PrivacyProtocolActivationLimitsV1 {
    /// ZK-ACE has no additional first-release count limits.
    #[cfg_attr(feature = "json", norito(rename = "zk-ace-pq-authorization-v0"))]
    ZkAcePqAuthorizationV0,
    /// Anonymous PGC receiver policy.
    #[cfg_attr(feature = "json", norito(rename = "anonymous-pgc-k-out-of-n-v1"))]
    AnonymousPgcKOutOfNV1(AnonymousPgcActivationLimitsV1),
    /// VeRange aggregation policy.
    #[cfg_attr(feature = "json", norito(rename = "verange-transparent-range-v1"))]
    VeRangeTransparentRangeV1(VeRangeActivationLimitsV1),
    /// ZK-AMS batch-admission and account-provisioning policy.
    #[cfg_attr(feature = "json", norito(rename = "iroha-zk-ams-v1"))]
    IrohaZkAmsV1(ZkAmsActivationLimitsV1),
    /// Vega has no additional first-release count limits.
    #[cfg_attr(feature = "json", norito(rename = "vega-existing-credential-zk-v0"))]
    VegaExistingCredentialZkV0,
    /// X.509 has fixed first-release limits encoded by its statement validator.
    #[cfg_attr(feature = "json", norito(rename = "iroha-zk-x509-stark-p256-v0"))]
    IrohaZkX509StarkP256V0,
    /// Jindo batched opening policy.
    #[cfg_attr(
        feature = "json",
        norito(rename = "iroha-jindo-polynomial-commitment-v0")
    )]
    IrohaJindoPolynomialCommitmentV0(JindoActivationLimitsV1),
    /// Lantern anonymous credentials have a fixed first-release parameter profile.
    #[cfg_attr(feature = "json", norito(rename = "iroha-bootle-lantern-anoncred-v1"))]
    IrohaBootleLanternAnoncredV1,
    /// Orchard one-to-one action policy.
    #[cfg_attr(feature = "json", norito(rename = "orchard-halo2-actions-v1"))]
    OrchardHalo2ActionsV1(OrchardActivationLimitsV1),
    /// FCMP++ input/output policy.
    #[cfg_attr(feature = "json", norito(rename = "monero-fcmp-plus-plus-v1"))]
    MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1),
    /// Native private-note input/output policy.
    #[cfg_attr(feature = "json", norito(rename = "iroha-ivm-private-note-stark-v1"))]
    IrohaIvmPrivateNoteStarkV1(IvmPrivateNoteActivationLimitsV1),
    /// PQ-MASP input/output policy.
    #[cfg_attr(feature = "json", norito(rename = "pq-masp-stark-v0"))]
    PqMaspStarkV0(PqMaspActivationLimitsV1),
}

impl PrivacyProtocolActivationLimitsV1 {
    /// Exact protocol to which these activation-specific limits apply.
    #[must_use]
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV0 => PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            Self::AnonymousPgcKOutOfNV1(_) => PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            Self::VeRangeTransparentRangeV1(_) => PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            Self::IrohaZkAmsV1(_) => PrivacyProtocolIdV1::IrohaZkAmsV1,
            Self::VegaExistingCredentialZkV0 => PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            Self::IrohaZkX509StarkP256V0 => PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            Self::IrohaJindoPolynomialCommitmentV0(_) => {
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0
            }
            Self::IrohaBootleLanternAnoncredV1 => PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
            Self::OrchardHalo2ActionsV1(_) => PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            Self::PqMaspStarkV0(_) => PrivacyProtocolIdV1::PqMaspStarkV0,
        }
    }

    /// Validate activation-specific values against first-release hard ceilings.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyProtocolActivationLimitsValidationError`] for zero or
    /// over-ceiling configuration values.
    pub fn validate(&self) -> Result<(), PrivacyProtocolActivationLimitsValidationError> {
        match *self {
            Self::AnonymousPgcKOutOfNV1(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::AnonymousPgcAnonymitySetSize,
                    limits.max_anonymity_set_size,
                    ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1,
                )?;
                if !ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1.contains(&limits.max_anonymity_set_size) {
                    return Err(
                        PrivacyProtocolActivationLimitsValidationError::InvalidPgcAnonymitySetSize {
                            size: limits.max_anonymity_set_size,
                        },
                    );
                }
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::AnonymousPgcRecipientCount,
                    limits.max_recipient_count,
                    ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
                )
            }
            Self::VeRangeTransparentRangeV1(limits) => validate_profile_limit(
                PrivacyActivationLimitFieldV1::VeRangeAggregationCount,
                limits.max_aggregation_count,
                VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1,
            ),
            Self::IrohaZkAmsV1(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::ZkAmsBatchSize,
                    limits.max_batch_size,
                    ZK_AMS_MAX_BATCH_SIZE_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::ZkAmsRingSize,
                    limits.max_ring_size,
                    ZK_AMS_MAX_RING_SIZE_V1,
                )?;
                if !ZK_AMS_RING_SIZES_V1.contains(&limits.max_ring_size) {
                    return Err(
                        PrivacyProtocolActivationLimitsValidationError::InvalidZkAmsRingSize {
                            size: limits.max_ring_size,
                        },
                    );
                }
                Ok(())
            }
            Self::IrohaJindoPolynomialCommitmentV0(limits) => validate_profile_limit(
                PrivacyActivationLimitFieldV1::JindoPolynomialCount,
                limits.max_polynomial_count,
                IROHA_JINDO_MAX_POLYNOMIALS_V1,
            ),
            Self::OrchardHalo2ActionsV1(limits) => validate_profile_limit(
                PrivacyActivationLimitFieldV1::OrchardActionCount,
                limits.max_action_count,
                ORCHARD_MAX_ACTIONS_V1,
            ),
            Self::MoneroFcmpPlusPlusV1(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::FcmpInputCount,
                    limits.max_input_count,
                    FCMP_MAX_INPUTS_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::FcmpOutputCount,
                    limits.max_output_count,
                    FCMP_MAX_OUTPUTS_V1,
                )
            }
            Self::IrohaIvmPrivateNoteStarkV1(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteInputCount,
                    limits.max_input_count,
                    IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteOutputCount,
                    limits.max_output_count,
                    IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
                )
            }
            Self::PqMaspStarkV0(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::PqMaspInputCount,
                    limits.max_input_count,
                    PQ_MASP_MAX_INPUTS_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::PqMaspOutputCount,
                    limits.max_output_count,
                    PQ_MASP_MAX_OUTPUTS_V1,
                )
            }
            _ => Ok(()),
        }
    }

    /// Validate this governed protocol policy against a compiled ceiling.
    ///
    /// Both values first undergo their intrinsic nonzero, hard-maximum, and
    /// closed-set validation. The protocol variants must then match exactly,
    /// and every governed component must be less than or equal to its compiled
    /// counterpart.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyProtocolActivationLimitsValidationError`] for an
    /// intrinsically invalid value or ceiling, a protocol-variant mismatch, or
    /// a component exceeding its configured ceiling.
    pub fn validate_with_ceiling(
        &self,
        ceiling: &Self,
    ) -> Result<(), PrivacyProtocolActivationLimitsValidationError> {
        self.validate()?;
        ceiling.validate()?;
        match (*self, *ceiling) {
            (Self::ZkAcePqAuthorizationV0, Self::ZkAcePqAuthorizationV0)
            | (Self::VegaExistingCredentialZkV0, Self::VegaExistingCredentialZkV0)
            | (Self::IrohaZkX509StarkP256V0, Self::IrohaZkX509StarkP256V0)
            | (Self::IrohaBootleLanternAnoncredV1, Self::IrohaBootleLanternAnoncredV1) => Ok(()),
            (Self::AnonymousPgcKOutOfNV1(value), Self::AnonymousPgcKOutOfNV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::AnonymousPgcAnonymitySetSize,
                    value.max_anonymity_set_size,
                    max.max_anonymity_set_size,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::AnonymousPgcRecipientCount,
                    value.max_recipient_count,
                    max.max_recipient_count,
                )
            }
            (Self::VeRangeTransparentRangeV1(value), Self::VeRangeTransparentRangeV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::VeRangeAggregationCount,
                    value.max_aggregation_count,
                    max.max_aggregation_count,
                )
            }
            (Self::IrohaZkAmsV1(value), Self::IrohaZkAmsV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::ZkAmsBatchSize,
                    value.max_batch_size,
                    max.max_batch_size,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::ZkAmsRingSize,
                    value.max_ring_size,
                    max.max_ring_size,
                )
            }
            (
                Self::IrohaJindoPolynomialCommitmentV0(value),
                Self::IrohaJindoPolynomialCommitmentV0(max),
            ) => validate_profile_limit_ceiling(
                PrivacyActivationLimitFieldV1::JindoPolynomialCount,
                value.max_polynomial_count,
                max.max_polynomial_count,
            ),
            (Self::OrchardHalo2ActionsV1(value), Self::OrchardHalo2ActionsV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::OrchardActionCount,
                    value.max_action_count,
                    max.max_action_count,
                )
            }
            (Self::MoneroFcmpPlusPlusV1(value), Self::MoneroFcmpPlusPlusV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::FcmpInputCount,
                    value.max_input_count,
                    max.max_input_count,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::FcmpOutputCount,
                    value.max_output_count,
                    max.max_output_count,
                )
            }
            (Self::IrohaIvmPrivateNoteStarkV1(value), Self::IrohaIvmPrivateNoteStarkV1(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteInputCount,
                    value.max_input_count,
                    max.max_input_count,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteOutputCount,
                    value.max_output_count,
                    max.max_output_count,
                )
            }
            (Self::PqMaspStarkV0(value), Self::PqMaspStarkV0(max)) => {
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::PqMaspInputCount,
                    value.max_input_count,
                    max.max_input_count,
                )?;
                validate_profile_limit_ceiling(
                    PrivacyActivationLimitFieldV1::PqMaspOutputCount,
                    value.max_output_count,
                    max.max_output_count,
                )
            }
            _ => Err(
                PrivacyProtocolActivationLimitsValidationError::ProtocolMismatch {
                    actual: self.protocol_id(),
                    ceiling: ceiling.protocol_id(),
                },
            ),
        }
    }
}

fn validate_profile_limit_ceiling(
    field: PrivacyActivationLimitFieldV1,
    value: u32,
    ceiling: u32,
) -> Result<(), PrivacyProtocolActivationLimitsValidationError> {
    if value > ceiling {
        return Err(
            PrivacyProtocolActivationLimitsValidationError::ExceedsConfiguredCeiling {
                field,
                value,
                ceiling,
            },
        );
    }
    Ok(())
}

fn validate_profile_limit(
    field: PrivacyActivationLimitFieldV1,
    value: u32,
    hard_max: u32,
) -> Result<(), PrivacyProtocolActivationLimitsValidationError> {
    if value == 0 {
        return Err(PrivacyProtocolActivationLimitsValidationError::Zero { field });
    }
    if value > hard_max {
        return Err(
            PrivacyProtocolActivationLimitsValidationError::ExceedsHardMaximum {
                field,
                value,
                hard_max,
            },
        );
    }
    Ok(())
}

/// Activation-specific limit field selected by validation diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyActivationLimitFieldV1 {
    /// Anonymous PGC anonymity-set size.
    AnonymousPgcAnonymitySetSize,
    /// Anonymous PGC intended recipient count.
    AnonymousPgcRecipientCount,
    /// VeRange aggregation count.
    VeRangeAggregationCount,
    /// ZK-AMS batch size.
    ZkAmsBatchSize,
    /// ZK-AMS admitted seed-key ring size.
    ZkAmsRingSize,
    /// Jindo polynomial count.
    JindoPolynomialCount,
    /// Orchard one-to-one action count.
    OrchardActionCount,
    /// FCMP++ input count.
    FcmpInputCount,
    /// FCMP++ output count.
    FcmpOutputCount,
    /// Native IVM private-note input count.
    IvmPrivateNoteInputCount,
    /// Native IVM private-note output count.
    IvmPrivateNoteOutputCount,
    /// PQ-MASP input count.
    PqMaspInputCount,
    /// PQ-MASP output count.
    PqMaspOutputCount,
}

/// Validation failure for protocol-specific activation limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyProtocolActivationLimitsValidationError {
    /// One activation-specific limit is zero.
    #[error("privacy activation limit {field:?} must be non-zero")]
    Zero {
        /// Invalid field.
        field: PrivacyActivationLimitFieldV1,
    },
    /// One activation-specific limit exceeds its first-release hard maximum.
    #[error("privacy activation limit {field:?} value {value} exceeds hard maximum {hard_max}")]
    ExceedsHardMaximum {
        /// Invalid field.
        field: PrivacyActivationLimitFieldV1,
        /// Configured value.
        value: u32,
        /// First-release hard maximum.
        hard_max: u32,
    },
    /// Activation limits and their configured ceiling target different protocols.
    #[error(
        "privacy activation limit protocol {actual:?} differs from ceiling protocol {ceiling:?}"
    )]
    ProtocolMismatch {
        /// Governed protocol variant.
        actual: PrivacyProtocolIdV1,
        /// Compiled-ceiling protocol variant.
        ceiling: PrivacyProtocolIdV1,
    },
    /// One activation-specific limit exceeds a valid configured ceiling.
    #[error(
        "privacy activation limit {field:?} value {value} exceeds configured ceiling {ceiling}"
    )]
    ExceedsConfiguredCeiling {
        /// Invalid field.
        field: PrivacyActivationLimitFieldV1,
        /// Governed value.
        value: u32,
        /// Component-wise ceiling.
        ceiling: u32,
    },
    /// Anonymous PGC activation size is not one of the closed set sizes.
    #[error("Anonymous PGC anonymity-set size {size} is not one of 16, 32, or 64")]
    InvalidPgcAnonymitySetSize {
        /// Invalid configured size.
        size: u32,
    },
    /// ZK-AMS activation ring size is not one of the closed profile sizes.
    #[error("ZK-AMS ring size {size} is not one of 16, 32, or 64")]
    InvalidZkAmsRingSize {
        /// Invalid configured size.
        size: u32,
    },
}

/// Scheduled component-wise tightening for one protocol activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyProtocolLimitsTighteningV1 {
    /// Exact block which admitted this schedule.
    pub scheduled_at_height: u64,
    /// Exact incoming block at whose start the successor becomes effective.
    pub effective_at_height: u64,
    /// Complete protocol-tagged successor limit set.
    pub next_limits: PrivacyProtocolActivationLimitsV1,
}

impl PrivacyProtocolLimitsTighteningV1 {
    /// Validate schedule timing and strict component-wise monotonicity.
    ///
    /// # Errors
    ///
    /// Rejects insufficient notice, a protocol mismatch, invalid limits, an
    /// increase, or a no-op.
    pub fn validate_against(
        &self,
        current_limits: &PrivacyProtocolActivationLimitsV1,
    ) -> Result<(), PrivacyProtocolLimitsTighteningValidationErrorV1> {
        validate_privacy_policy_schedule_heights_v1(
            self.scheduled_at_height,
            self.effective_at_height,
        )
        .map_err(PrivacyProtocolLimitsTighteningValidationErrorV1::Schedule)?;
        self.next_limits
            .validate_with_ceiling(current_limits)
            .map_err(PrivacyProtocolLimitsTighteningValidationErrorV1::Limits)?;
        if self.next_limits == *current_limits {
            return Err(PrivacyProtocolLimitsTighteningValidationErrorV1::NoChange);
        }
        Ok(())
    }
}

/// Validation failure for a scheduled protocol-specific tightening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyProtocolLimitsTighteningValidationErrorV1 {
    /// Scheduled/effective heights violate the chain-wide notice rule.
    #[error("privacy protocol-limit schedule is invalid: {0}")]
    Schedule(PrivacyPolicyValidationErrorV1),
    /// Successor limits are invalid, mismatched, or increase a component.
    #[error("privacy protocol-limit tightening is invalid: {0}")]
    Limits(PrivacyProtocolActivationLimitsValidationError),
    /// A tightening must change at least one component.
    #[error("privacy protocol-limit tightening is a no-op")]
    NoChange,
}

/// Governed activation record for one exact privacy protocol implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyProtocolActivationRecordV1 {
    /// Exact protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact proof-system profile.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Exact native engine identity.
    pub engine_id: PrivacyEngineIdV1,
    /// Governed public-parameter set identifier.
    pub parameter_id: PrivacyParameterIdV1,
    /// Governed public-parameter digest.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Governed verifier artifact digest.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Governed public-statement schema digest.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Governed native engine-manifest digest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Current governed lifecycle.
    pub lifecycle: PrivacyProtocolLifecycleV1,
    /// Protocol-specific governed count limits.
    pub protocol_limits: PrivacyProtocolActivationLimitsV1,
    /// At most one delayed, component-wise protocol-limit tightening.
    pub pending_protocol_limits_tightening: Option<PrivacyProtocolLimitsTighteningV1>,
    /// Testnet assurance classification.
    pub assurance: PrivacyAssuranceV1,
}

impl PrivacyProtocolActivationRecordV1 {
    /// Validate exact protocol mappings, non-zero digests, lifecycle, and limits.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyActivationValidationError`] on the first invalid
    /// binding in deterministic field order.
    pub fn validate(&self) -> Result<(), PrivacyActivationValidationError> {
        let expected_proof_system = self.protocol_id.expected_proof_system();
        if self.proof_system_id != expected_proof_system {
            return Err(PrivacyActivationValidationError::ProofSystemMismatch {
                protocol_id: self.protocol_id,
                expected: expected_proof_system,
                actual: self.proof_system_id,
            });
        }
        let expected_engine = self.protocol_id.expected_engine();
        if self.engine_id != expected_engine {
            return Err(PrivacyActivationValidationError::EngineMismatch {
                protocol_id: self.protocol_id,
                expected: expected_engine,
                actual: self.engine_id,
            });
        }
        if self.parameter_id.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroParameterId);
        }
        if self.parameter_digest.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroParameterDigest);
        }
        if self.verifier_digest.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroVerifierDigest);
        }
        if self.statement_schema_digest.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroStatementSchemaDigest);
        }
        if self.engine_manifest_digest.is_zero() {
            return Err(PrivacyActivationValidationError::ZeroEngineManifestDigest);
        }
        let limits_protocol = self.protocol_limits.protocol_id();
        if limits_protocol != self.protocol_id {
            return Err(PrivacyActivationValidationError::ProtocolLimitsMismatch {
                protocol_id: self.protocol_id,
                limits_protocol,
            });
        }
        self.protocol_limits
            .validate()
            .map_err(PrivacyActivationValidationError::ProtocolLimits)?;
        if let Some(pending) = self.pending_protocol_limits_tightening {
            pending
                .validate_against(&self.protocol_limits)
                .map_err(PrivacyActivationValidationError::PendingProtocolLimits)?;
        }
        self.lifecycle
            .validate()
            .map_err(PrivacyActivationValidationError::Lifecycle)
    }
}

/// Validation failure for [`PrivacyProtocolActivationRecordV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyActivationValidationError {
    /// Protocol and proof-system identities do not match.
    #[error("privacy protocol {protocol_id:?} requires proof system {expected:?}, got {actual:?}")]
    ProofSystemMismatch {
        /// Protocol being activated.
        protocol_id: PrivacyProtocolIdV1,
        /// Required proof system.
        expected: PrivacyProofSystemIdV1,
        /// Supplied proof system.
        actual: PrivacyProofSystemIdV1,
    },
    /// Protocol and native engine identities do not match.
    #[error("privacy protocol {protocol_id:?} requires engine {expected:?}, got {actual:?}")]
    EngineMismatch {
        /// Protocol being activated.
        protocol_id: PrivacyProtocolIdV1,
        /// Required native engine.
        expected: PrivacyEngineIdV1,
        /// Supplied native engine.
        actual: PrivacyEngineIdV1,
    },
    /// Governed parameter-set identifier is zero.
    #[error("privacy activation parameter id must be non-zero")]
    ZeroParameterId,
    /// Governed parameter digest is zero.
    #[error("privacy activation parameter digest must be non-zero")]
    ZeroParameterDigest,
    /// Governed verifier digest is zero.
    #[error("privacy activation verifier digest must be non-zero")]
    ZeroVerifierDigest,
    /// Governed statement-schema digest is zero.
    #[error("privacy activation statement-schema digest must be non-zero")]
    ZeroStatementSchemaDigest,
    /// Governed engine-manifest digest is zero.
    #[error("privacy activation engine-manifest digest must be non-zero")]
    ZeroEngineManifestDigest,
    /// Protocol-specific limits are tagged for another protocol.
    #[error(
        "privacy activation protocol {protocol_id:?} differs from protocol-limit tag {limits_protocol:?}"
    )]
    ProtocolLimitsMismatch {
        /// Activated protocol.
        protocol_id: PrivacyProtocolIdV1,
        /// Protocol encoded by the activation-specific limits.
        limits_protocol: PrivacyProtocolIdV1,
    },
    /// Protocol-specific activation limits are invalid.
    #[error("privacy activation protocol limits are invalid: {0}")]
    ProtocolLimits(PrivacyProtocolActivationLimitsValidationError),
    /// A pending protocol-specific tightening is invalid.
    #[error("privacy activation pending protocol limits are invalid: {0}")]
    PendingProtocolLimits(PrivacyProtocolLimitsTighteningValidationErrorV1),
    /// Lifecycle is invalid.
    #[error("privacy activation lifecycle is invalid: {0}")]
    Lifecycle(PrivacyLifecycleValidationError),
}

/// Exact public capability-snapshot wire version.
pub const PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1: u32 = 1;

/// Exact locally compiled bindings exposed by the public privacy snapshot.
///
/// This is a wire-model counterpart of the core-only compiled profile. It
/// deliberately contains no lifecycle or readiness boolean: governance state
/// is carried separately by [`PrivacyCapabilityRowV1::activation`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyCompiledProfileSnapshotV1 {
    /// Closed protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Closed proof-system identity.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Closed native-engine identity.
    pub engine_id: PrivacyEngineIdV1,
    /// Deterministic identifier of the compiled parameter set.
    pub parameter_id: PrivacyParameterIdV1,
    /// Digest of the exact compiled parameters.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Digest of the exact verifier relation and proof wire.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Digest of the exact public-statement schema.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Digest of the complete compiled engine manifest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Exact protocol-specific limits compiled into the verifier.
    pub protocol_limits: PrivacyProtocolActivationLimitsV1,
}

impl PrivacyCompiledProfileSnapshotV1 {
    /// Validate the closed protocol mappings and every fixed binding.
    ///
    /// # Errors
    ///
    /// Returns a deterministic error for the first mismatched identity, zero
    /// binding, protocol-tag mismatch, or invalid limit.
    pub fn validate(&self) -> Result<(), PrivacyCompiledProfileSnapshotValidationErrorV1> {
        let expected_proof_system = self.protocol_id.expected_proof_system();
        if self.proof_system_id != expected_proof_system {
            return Err(
                PrivacyCompiledProfileSnapshotValidationErrorV1::ProofSystemMismatch {
                    protocol_id: self.protocol_id,
                    expected: expected_proof_system,
                    actual: self.proof_system_id,
                },
            );
        }
        let expected_engine = self.protocol_id.expected_engine();
        if self.engine_id != expected_engine {
            return Err(
                PrivacyCompiledProfileSnapshotValidationErrorV1::EngineMismatch {
                    protocol_id: self.protocol_id,
                    expected: expected_engine,
                    actual: self.engine_id,
                },
            );
        }
        if self.parameter_id.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroParameterId);
        }
        if self.parameter_digest.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroParameterDigest);
        }
        if self.verifier_digest.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroVerifierDigest);
        }
        if self.statement_schema_digest.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroStatementSchemaDigest);
        }
        if self.engine_manifest_digest.is_zero() {
            return Err(PrivacyCompiledProfileSnapshotValidationErrorV1::ZeroEngineManifestDigest);
        }
        let limits_protocol = self.protocol_limits.protocol_id();
        if limits_protocol != self.protocol_id {
            return Err(
                PrivacyCompiledProfileSnapshotValidationErrorV1::ProtocolLimitsMismatch {
                    protocol_id: self.protocol_id,
                    limits_protocol,
                },
            );
        }
        self.protocol_limits
            .validate()
            .map_err(PrivacyCompiledProfileSnapshotValidationErrorV1::ProtocolLimits)
    }
}

/// Validation failure for [`PrivacyCompiledProfileSnapshotV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyCompiledProfileSnapshotValidationErrorV1 {
    /// Protocol and proof-system identities differ.
    #[error(
        "compiled privacy protocol {protocol_id:?} requires proof system {expected:?}, got {actual:?}"
    )]
    ProofSystemMismatch {
        /// Protocol in the compiled profile.
        protocol_id: PrivacyProtocolIdV1,
        /// Required proof system.
        expected: PrivacyProofSystemIdV1,
        /// Rejected proof system.
        actual: PrivacyProofSystemIdV1,
    },
    /// Protocol and native-engine identities differ.
    #[error(
        "compiled privacy protocol {protocol_id:?} requires engine {expected:?}, got {actual:?}"
    )]
    EngineMismatch {
        /// Protocol in the compiled profile.
        protocol_id: PrivacyProtocolIdV1,
        /// Required engine.
        expected: PrivacyEngineIdV1,
        /// Rejected engine.
        actual: PrivacyEngineIdV1,
    },
    /// Compiled parameter-set identifier is zero.
    #[error("compiled privacy parameter id must be non-zero")]
    ZeroParameterId,
    /// Compiled parameter digest is zero.
    #[error("compiled privacy parameter digest must be non-zero")]
    ZeroParameterDigest,
    /// Compiled verifier digest is zero.
    #[error("compiled privacy verifier digest must be non-zero")]
    ZeroVerifierDigest,
    /// Compiled statement-schema digest is zero.
    #[error("compiled privacy statement-schema digest must be non-zero")]
    ZeroStatementSchemaDigest,
    /// Compiled engine-manifest digest is zero.
    #[error("compiled privacy engine-manifest digest must be non-zero")]
    ZeroEngineManifestDigest,
    /// Compiled limits are tagged for another protocol.
    #[error(
        "compiled privacy protocol {protocol_id:?} differs from protocol-limit tag {limits_protocol:?}"
    )]
    ProtocolLimitsMismatch {
        /// Compiled protocol.
        protocol_id: PrivacyProtocolIdV1,
        /// Protocol encoded by the compiled limit variant.
        limits_protocol: PrivacyProtocolIdV1,
    },
    /// Compiled protocol-specific limits are malformed.
    #[error("compiled privacy protocol limits are invalid: {0}")]
    ProtocolLimits(PrivacyProtocolActivationLimitsValidationError),
}

/// Typed failure canonicalizing a compiled public-statement schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "schema_error", content = "detail", deny_unknown_fields)
)]
pub enum PrivacyCompiledStatementSchemaErrorV1 {
    /// Two types reused one stable identifier for incompatible shapes.
    #[cfg_attr(feature = "json", norito(rename = "conflicting-stable-type-id"))]
    ConflictingStableTypeId,
    /// A schema referenced a type absent from the canonical map.
    #[cfg_attr(feature = "json", norito(rename = "missing-type-reference"))]
    MissingTypeReference,
}

/// Typed reason why one closed protocol has no executable compiled profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "reason", content = "detail", deny_unknown_fields)
)]
pub enum PrivacyCompiledProfileUnavailableReasonV1 {
    /// This binary contains no complete end-to-end engine for the protocol.
    #[cfg_attr(feature = "json", norito(rename = "engine-unavailable"))]
    EngineUnavailable,
    /// Deterministic transparent parameter initialization failed.
    #[cfg_attr(feature = "json", norito(rename = "profile-initialization-failed"))]
    ProfileInitializationFailed,
    /// The locally generated statement schema was ambiguous or incomplete.
    #[cfg_attr(feature = "json", norito(rename = "statement-schema-invalid"))]
    StatementSchemaInvalid(PrivacyCompiledStatementSchemaErrorV1),
}

/// Closed result of obtaining one locally compiled privacy profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "value", deny_unknown_fields)
)]
pub enum PrivacyCompiledProfileResultV1 {
    /// The exact native profile is executable in this binary.
    #[cfg_attr(feature = "json", norito(rename = "available"))]
    Available(PrivacyCompiledProfileSnapshotV1),
    /// The protocol remains explicitly unavailable and fail-closed.
    #[cfg_attr(feature = "json", norito(rename = "unavailable"))]
    Unavailable(PrivacyCompiledProfileUnavailableReasonV1),
}

/// One protocol row in the canonical public capability snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyCapabilityRowV1 {
    /// Closed protocol identity for this row.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact local compiled-profile result.
    pub compiled_profile: PrivacyCompiledProfileResultV1,
    /// Exact committed governance record, if registered.
    pub activation: Option<PrivacyProtocolActivationRecordV1>,
}

impl PrivacyCapabilityRowV1 {
    /// Validate a row against its committed snapshot height.
    ///
    /// # Errors
    ///
    /// Rejects embedded identity mismatches, malformed compiled profiles,
    /// activation without an executable engine, activation/profile binding
    /// drift, and lifecycle or policy heights inconsistent with the snapshot.
    pub fn validate_at_committed_height(
        &self,
        committed_height: u64,
    ) -> Result<(), PrivacyCapabilityRowValidationErrorV1> {
        let profile = match self.compiled_profile {
            PrivacyCompiledProfileResultV1::Available(profile) => {
                profile
                    .validate()
                    .map_err(PrivacyCapabilityRowValidationErrorV1::CompiledProfile)?;
                if profile.protocol_id != self.protocol_id {
                    return Err(
                        PrivacyCapabilityRowValidationErrorV1::CompiledProfileProtocolMismatch {
                            row_protocol: self.protocol_id,
                            profile_protocol: profile.protocol_id,
                        },
                    );
                }
                Some(profile)
            }
            PrivacyCompiledProfileResultV1::Unavailable(_) => None,
        };

        let Some(activation) = self.activation else {
            return Ok(());
        };
        let Some(profile) = profile else {
            return Err(
                PrivacyCapabilityRowValidationErrorV1::UnavailableActivation {
                    protocol_id: self.protocol_id,
                },
            );
        };
        activation
            .validate()
            .map_err(PrivacyCapabilityRowValidationErrorV1::Activation)?;
        if activation.protocol_id != self.protocol_id {
            return Err(
                PrivacyCapabilityRowValidationErrorV1::ActivationProtocolMismatch {
                    row_protocol: self.protocol_id,
                    activation_protocol: activation.protocol_id,
                },
            );
        }
        validate_privacy_capability_activation_profile_v1(&activation, &profile)?;
        validate_privacy_capability_activation_height_v1(&activation, committed_height)
    }
}

fn validate_privacy_capability_activation_profile_v1(
    activation: &PrivacyProtocolActivationRecordV1,
    profile: &PrivacyCompiledProfileSnapshotV1,
) -> Result<(), PrivacyCapabilityRowValidationErrorV1> {
    if activation.proof_system_id != profile.proof_system_id {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::ProofSystem,
            },
        );
    }
    if activation.engine_id != profile.engine_id {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::Engine,
            },
        );
    }
    if activation.parameter_id != profile.parameter_id {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::ParameterId,
            },
        );
    }
    if activation.parameter_digest != profile.parameter_digest {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::ParameterDigest,
            },
        );
    }
    if activation.verifier_digest != profile.verifier_digest {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::VerifierDigest,
            },
        );
    }
    if activation.statement_schema_digest != profile.statement_schema_digest {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::StatementSchemaDigest,
            },
        );
    }
    if activation.engine_manifest_digest != profile.engine_manifest_digest {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                field: PrivacyCapabilityBindingFieldV1::EngineManifestDigest,
            },
        );
    }
    activation
        .protocol_limits
        .validate_with_ceiling(&profile.protocol_limits)
        .map_err(PrivacyCapabilityRowValidationErrorV1::ActivationProtocolLimits)
}

fn validate_privacy_capability_activation_height_v1(
    activation: &PrivacyProtocolActivationRecordV1,
    committed_height: u64,
) -> Result<(), PrivacyCapabilityRowValidationErrorV1> {
    let (proposed_at_height, activated_at_height, state_since_height) = match activation.lifecycle {
        PrivacyProtocolLifecycleV1::Proposed(state) => {
            if state.activate_at_height <= committed_height {
                return Err(
                    PrivacyCapabilityRowValidationErrorV1::UnpromotedDueActivation {
                        activate_at_height: state.activate_at_height,
                        committed_height,
                    },
                );
            }
            (state.proposed_at_height, None, None)
        }
        PrivacyProtocolLifecycleV1::Active(state) => (
            state.proposed_at_height,
            Some(state.activated_at_height),
            Some(state.state_since_height),
        ),
        PrivacyProtocolLifecycleV1::Suspended(state) => (
            state.proposed_at_height,
            Some(state.activated_at_height),
            Some(state.state_since_height),
        ),
        PrivacyProtocolLifecycleV1::Retired(state) => (
            state.proposed_at_height,
            state.activated_at_height,
            Some(state.state_since_height),
        ),
    };
    if proposed_at_height > committed_height {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ProposalAfterCommitted {
                proposed_at_height,
                committed_height,
            },
        );
    }
    if let Some(activated_at_height) = activated_at_height
        && activated_at_height > committed_height
    {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::ActivationAfterCommitted {
                activated_at_height,
                committed_height,
            },
        );
    }
    if let Some(state_since_height) = state_since_height
        && state_since_height > committed_height
    {
        return Err(
            PrivacyCapabilityRowValidationErrorV1::LifecycleStateAfterCommitted {
                state_since_height,
                committed_height,
            },
        );
    }
    if let Some(pending) = activation.pending_protocol_limits_tightening {
        if pending.scheduled_at_height > committed_height {
            return Err(
                PrivacyCapabilityRowValidationErrorV1::ProtocolLimitsScheduledAfterCommitted {
                    scheduled_at_height: pending.scheduled_at_height,
                    committed_height,
                },
            );
        }
        if pending.effective_at_height <= committed_height {
            return Err(
                PrivacyCapabilityRowValidationErrorV1::ProtocolLimitsNotFuture {
                    effective_at_height: pending.effective_at_height,
                    committed_height,
                },
            );
        }
    }
    Ok(())
}

/// Immutable binding selected when comparing activation and compiled profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyCapabilityBindingFieldV1 {
    /// Proof-system identity.
    ProofSystem,
    /// Native-engine identity.
    Engine,
    /// Parameter-set identifier.
    ParameterId,
    /// Parameter-set digest.
    ParameterDigest,
    /// Verifier digest.
    VerifierDigest,
    /// Statement-schema digest.
    StatementSchemaDigest,
    /// Engine-manifest digest.
    EngineManifestDigest,
}

/// Validation failure for one [`PrivacyCapabilityRowV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyCapabilityRowValidationErrorV1 {
    /// Locally compiled profile is malformed.
    #[error("privacy capability compiled profile is invalid: {0}")]
    CompiledProfile(PrivacyCompiledProfileSnapshotValidationErrorV1),
    /// Row and compiled-profile identities differ.
    #[error(
        "privacy capability row protocol {row_protocol:?} differs from compiled profile {profile_protocol:?}"
    )]
    CompiledProfileProtocolMismatch {
        /// Row identity.
        row_protocol: PrivacyProtocolIdV1,
        /// Embedded profile identity.
        profile_protocol: PrivacyProtocolIdV1,
    },
    /// A governance activation exists for an unavailable local engine.
    #[error("unavailable privacy protocol {protocol_id:?} cannot have an activation")]
    UnavailableActivation {
        /// Unavailable protocol.
        protocol_id: PrivacyProtocolIdV1,
    },
    /// Governed activation is malformed.
    #[error("privacy capability activation is invalid: {0}")]
    Activation(PrivacyActivationValidationError),
    /// Row and governed activation identities differ.
    #[error(
        "privacy capability row protocol {row_protocol:?} differs from activation {activation_protocol:?}"
    )]
    ActivationProtocolMismatch {
        /// Row identity.
        row_protocol: PrivacyProtocolIdV1,
        /// Embedded activation identity.
        activation_protocol: PrivacyProtocolIdV1,
    },
    /// An immutable governed binding differs from the compiled profile.
    #[error("privacy activation differs from compiled profile at {field:?}")]
    ActivationProfileMismatch {
        /// Mismatched immutable field.
        field: PrivacyCapabilityBindingFieldV1,
    },
    /// Governed protocol limits exceed the compiled profile.
    #[error("privacy activation limits differ from compiled profile: {0}")]
    ActivationProtocolLimits(PrivacyProtocolActivationLimitsValidationError),
    /// Proposal admission is later than the snapshot that contains it.
    #[error(
        "privacy proposal height {proposed_at_height} is after committed height {committed_height}"
    )]
    ProposalAfterCommitted {
        /// Persisted proposal height.
        proposed_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// A due proposal remained unpromoted in committed state.
    #[error(
        "privacy activation at height {activate_at_height} remained proposed at committed height {committed_height}"
    )]
    UnpromotedDueActivation {
        /// Scheduled activation height.
        activate_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// First activation is later than the committed snapshot.
    #[error(
        "privacy activation height {activated_at_height} is after committed height {committed_height}"
    )]
    ActivationAfterCommitted {
        /// Claimed first activation height.
        activated_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// Current lifecycle interval begins after the committed snapshot.
    #[error(
        "privacy lifecycle state height {state_since_height} is after committed height {committed_height}"
    )]
    LifecycleStateAfterCommitted {
        /// Claimed current-state start height.
        state_since_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// Protocol-limit schedule claims admission after the snapshot.
    #[error(
        "privacy protocol-limit schedule height {scheduled_at_height} is after committed height {committed_height}"
    )]
    ProtocolLimitsScheduledAfterCommitted {
        /// Claimed admission height.
        scheduled_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
    /// Protocol-limit schedule was retained after its exact effective height.
    #[error(
        "privacy protocol-limit effective height {effective_at_height} is not after committed height {committed_height}"
    )]
    ProtocolLimitsNotFuture {
        /// Scheduled effective height.
        effective_at_height: u64,
        /// Snapshot height.
        committed_height: u64,
    },
}

/// Authoritative committed privacy capability snapshot.
///
/// `protocols` must contain exactly [`PrivacyProtocolIdV1::ALL`] in Norito
/// discriminant order. The ordering rule makes missing, duplicate, and
/// reordered rows fail closed without accepting aliases.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyCapabilitySnapshotV1 {
    /// Exact snapshot schema version.
    pub version: u32,
    /// Height of the committed state from which this snapshot was read.
    pub committed_height: u64,
    /// Authoritative singleton chain-wide privacy policy.
    pub consensus_policy: PrivacyConsensusPolicyV1,
    /// Exactly twelve protocol rows in canonical discriminant order.
    pub protocols: Vec<PrivacyCapabilityRowV1>,
}

impl PrivacyCapabilitySnapshotV1 {
    /// Validate the complete public snapshot and all embedded state.
    ///
    /// # Errors
    ///
    /// Rejects an unknown version, invalid singleton policy, any row-count or
    /// ordering drift, or an invalid protocol row.
    pub fn validate(&self) -> Result<(), PrivacyCapabilitySnapshotValidationErrorV1> {
        if self.version != PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1 {
            return Err(PrivacyCapabilitySnapshotValidationErrorV1::Version {
                expected: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
                actual: self.version,
            });
        }
        self.consensus_policy
            .validate_at_committed_height(self.committed_height)
            .map_err(PrivacyCapabilitySnapshotValidationErrorV1::ConsensusPolicy)?;
        if self.protocols.len() != PrivacyProtocolIdV1::COUNT {
            return Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolCount {
                expected: PrivacyProtocolIdV1::COUNT,
                actual: self.protocols.len(),
            });
        }
        for (index, (row, expected)) in self
            .protocols
            .iter()
            .zip(PrivacyProtocolIdV1::ALL)
            .enumerate()
        {
            if row.protocol_id != expected {
                return Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolOrder {
                    index,
                    expected,
                    actual: row.protocol_id,
                });
            }
            row.validate_at_committed_height(self.committed_height)
                .map_err(
                    |source| PrivacyCapabilitySnapshotValidationErrorV1::ProtocolRow {
                        protocol_id: expected,
                        source,
                    },
                )?;
        }
        Ok(())
    }
}

/// Exact native bridge ABI required by the first-release privacy SDK surface.
pub const PRIVACY_BRIDGE_ABI_VERSION_V1: u32 = 21;
/// Maximum accepted size of one canonical privacy capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1: usize = 256 * 1024;
/// Maximum elements accepted in any sequence while decoding a capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_SEQUENCE_ELEMENTS_V1: usize = PrivacyProtocolIdV1::COUNT;
/// Maximum cumulative elements accepted while decoding a capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_TOTAL_ELEMENTS_V1: usize = PrivacyProtocolIdV1::COUNT;
/// Maximum bytes accepted in one length-delimited capability-archive field.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_FIELD_BYTES_V1: usize = 128 * 1024;
/// Maximum cumulative allocation permitted while decoding a capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_TOTAL_ALLOCATION_BYTES_V1: usize = 256 * 1024;
/// Maximum data-dependent nesting depth permitted in a capability archive.
pub const PRIVACY_CAPABILITY_ARCHIVE_MAX_NESTING_DEPTH_V1: usize = 32;

/// Stable result codes returned by every native privacy capability validator.
///
/// These numeric discriminants are part of ABI 21. SDKs must accept only
/// [`Self::Valid`]; every other value is a fail-closed rejection.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyCapabilityArchiveValidationStatusV1 {
    /// The archive is the exact canonical typed snapshot and is semantically valid.
    Valid = 0,
    /// A native ABI caller supplied a null archive pointer.
    NullPointer = 1,
    /// The archive contains no bytes.
    Empty = 2,
    /// The archive exceeds [`PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1`].
    ArchiveTooLarge = 3,
    /// Norito rejected a declared sequence, field, allocation, or nesting budget.
    DecodeResourceLimit = 4,
    /// The archive schema is not exactly [`PrivacyCapabilitySnapshotV1`].
    SchemaMismatch = 5,
    /// The bytes are not the one canonical uncompressed V1 encoding.
    NonCanonical = 6,
    /// The archive is malformed, truncated, or fails its checksum.
    MalformedArchive = 7,
    /// The typed snapshot violates its closed first-release semantic invariants.
    InvalidSnapshot = 8,
}

impl PrivacyCapabilityArchiveValidationStatusV1 {
    /// Return the stable ABI-21 integer representation.
    #[must_use]
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// Return whether the archive was accepted.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}

/// Validate one untrusted canonical typed privacy capability archive.
///
/// Admission first applies the fixed 256 KiB byte ceiling, then decodes with
/// tight per-sequence, cumulative-element, allocation, and nesting budgets.
/// The canonical decoder enforces the exact
/// [`PrivacyCapabilitySnapshotV1`] schema and byte-for-byte canonical
/// re-encoding. Finally [`PrivacyCapabilitySnapshotV1::validate`] enforces the
/// exact twelve rows in [`PrivacyProtocolIdV1::ALL`] order and all profile,
/// activation, and policy bindings.
#[must_use]
pub fn validate_privacy_capability_archive_v1(
    archive: &[u8],
) -> PrivacyCapabilityArchiveValidationStatusV1 {
    use PrivacyCapabilityArchiveValidationStatusV1 as Status;

    if archive.is_empty() {
        return Status::Empty;
    }
    if archive.len() > PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1 {
        return Status::ArchiveTooLarge;
    }

    let limits = norito::DecodeLimits::new(
        PRIVACY_CAPABILITY_ARCHIVE_MAX_SEQUENCE_ELEMENTS_V1,
        PRIVACY_CAPABILITY_ARCHIVE_MAX_FIELD_BYTES_V1,
        PRIVACY_CAPABILITY_ARCHIVE_MAX_TOTAL_ELEMENTS_V1,
        PRIVACY_CAPABILITY_ARCHIVE_MAX_TOTAL_ALLOCATION_BYTES_V1,
        PRIVACY_CAPABILITY_ARCHIVE_MAX_NESTING_DEPTH_V1,
    );
    let snapshot = match norito::decode_canonical_with_limits::<PrivacyCapabilitySnapshotV1>(
        archive, limits,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) if error.is_decode_resource_limit() => return Status::DecodeResourceLimit,
        Err(norito::Error::SchemaMismatch) => return Status::SchemaMismatch,
        Err(
            norito::Error::NonCanonicalEncoding
            | norito::Error::DecodeFlagsMismatch { .. }
            | norito::Error::UnsupportedCompression { .. },
        ) => return Status::NonCanonical,
        Err(_) => return Status::MalformedArchive,
    };
    if snapshot.validate().is_err() {
        return Status::InvalidSnapshot;
    }
    Status::Valid
}

/// Validation failure for [`PrivacyCapabilitySnapshotV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyCapabilitySnapshotValidationErrorV1 {
    /// Snapshot wire version is not the exact first-release version.
    #[error("privacy capability snapshot version {actual} differs from required {expected}")]
    Version {
        /// Required version.
        expected: u32,
        /// Rejected version.
        actual: u32,
    },
    /// Singleton policy is invalid at the committed height.
    #[error("privacy capability consensus policy is invalid: {0}")]
    ConsensusPolicy(PrivacyPolicyValidationErrorV1),
    /// Protocol row count differs from the closed registry.
    #[error("privacy capability snapshot has {actual} rows; expected {expected}")]
    ProtocolCount {
        /// Closed first-release row count.
        expected: usize,
        /// Rejected row count.
        actual: usize,
    },
    /// A row is missing, duplicated, or reordered.
    #[error(
        "privacy capability row {index} is {actual:?}; expected canonical protocol {expected:?}"
    )]
    ProtocolOrder {
        /// Zero-based row index.
        index: usize,
        /// Required protocol at this index.
        expected: PrivacyProtocolIdV1,
        /// Rejected protocol at this index.
        actual: PrivacyProtocolIdV1,
    },
    /// One canonical row is invalid.
    #[error("privacy capability row {protocol_id:?} is invalid: {source}")]
    ProtocolRow {
        /// Protocol selected by row order.
        protocol_id: PrivacyProtocolIdV1,
        /// Exact row validation failure.
        source: PrivacyCapabilityRowValidationErrorV1,
    },
}

/// Closed Anonymous PGC anonymity-set sizes in the first release.
pub const ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1: [u32; 3] = [16, 32, 64];
/// Maximum Anonymous PGC anonymity-set size in the first release.
pub const ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1: u32 = 64;
/// Maximum Anonymous PGC intended recipients in the first release.
pub const ANONYMOUS_PGC_MAX_RECIPIENTS_V1: u32 = 8;
/// Hard maximum VeRange aggregation count in the first release.
pub const VERANGE_HARD_MAX_AGGREGATION_COUNT_V1: u32 = 64;
/// Effective VeRange aggregation ceiling under the Taira global commitment cap.
pub const VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1: u32 =
    if VERANGE_HARD_MAX_AGGREGATION_COUNT_V1 < TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1 {
        VERANGE_HARD_MAX_AGGREGATION_COUNT_V1
    } else {
        TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
    };
/// Maximum ordered anchors in one first-release ZK-AMS batch settlement.
pub const ZK_AMS_MAX_BATCH_SIZE_V1: u32 = 8;
/// Closed admitted seed-key ring sizes in the first release.
pub const ZK_AMS_RING_SIZES_V1: [u32; 3] = [16, 32, 64];
/// Maximum admitted seed-key ring size in the first release.
pub const ZK_AMS_MAX_RING_SIZE_V1: u32 = 64;
/// Maximum polynomials in one Jindo batched univariate-opening statement.
pub const IROHA_JINDO_MAX_POLYNOMIALS_V1: u32 = 4;
/// Exact canonical byte width of one Jindo coefficient-field element.
pub const IROHA_JINDO_FIELD_ELEMENT_BYTES_V1: usize = 32;
/// Exact fixed-profile Jindo outer-commitment rank.
pub const IROHA_JINDO_OUTER_COMMITMENT_RANK_V1: usize = 13;
/// Exact fixed-profile Jindo application-ring degree.
pub const IROHA_JINDO_RING_DEGREE_V1: usize = 256;
/// Exact signed coefficient width in the public rounded commitment wire.
pub const IROHA_JINDO_COMMITMENT_COEFFICIENT_BYTES_V1: usize = 4;
/// Exact canonical byte width of one fixed-profile Jindo lattice commitment.
pub const IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1: usize = IROHA_JINDO_OUTER_COMMITMENT_RANK_V1
    * IROHA_JINDO_RING_DEGREE_V1
    * IROHA_JINDO_COMMITMENT_COEFFICIENT_BYTES_V1;
/// Minimum canonical rounded outer-commitment coefficient.
///
/// This is the arithmetic-floor quotient of the smallest balanced residue
/// modulo the fixed 95-bit outer modulus by `2^65`.
pub const IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1: i32 = -268_435_457;
/// Maximum canonical rounded outer-commitment coefficient.
///
/// The one-value asymmetry relative to the minimum is required by the odd
/// outer modulus and arithmetic-floor rounding; accepting a wider symmetric
/// interval would admit encodings the commitment algorithm cannot produce.
pub const IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1: i32 = 268_435_456;
/// Exact direct 64-bit attribute count in the Bootle/Lantern credential profile.
pub const BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1: usize = 8;
/// Exact byte width of one direct Bootle/Lantern attribute.
pub const BOOTLE_LANTERN_ATTRIBUTE_BYTES_V1: usize = 8;
/// Degree of every polynomial in the Bootle/Lantern application ring.
pub const BOOTLE_LANTERN_RING_DEGREE_V1: usize = 64;
/// Application-ring modulus used by the fixed Bootle/Lantern profile.
pub const BOOTLE_LANTERN_APPLICATION_MODULUS_V1: u16 = 12_289;
/// Rows and columns in the issuer's canonical public matrix `B`.
pub const BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1: usize = 8;
/// Maximum selectively disclosed attributes in one Bootle/Lantern statement.
pub const BOOTLE_LANTERN_MAX_DISCLOSED_ATTRIBUTES_V1: u32 = 8;
/// Maximum governed allowed public values for one required attribute.
pub const BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1: u32 = 32;
/// Maximum authoritative Bootle/Lantern issuer-policy lineages in committed state.
pub const BOOTLE_LANTERN_MAX_ISSUER_POLICIES_V1: usize = 4_096;
/// Maximum immutable Vega issuer revisions retained across all lineages.
pub const VEGA_MAX_ISSUER_RECORDS_V1: usize = 4_096;
/// Maximum immutable revisions retained for one Vega issuer lineage.
pub const VEGA_MAX_ISSUER_RECORD_REVISIONS_PER_LINEAGE_V1: usize = 64;
/// Canonical origin epoch for a Vega issuer-key/policy lineage.
pub const VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1: u64 = 1;
/// Maximum admitted X.509 chain depth, including the leaf certificate.
pub const ZK_X509_MAX_CHAIN_DEPTH_V1: u8 = 3;
/// Minimum admitted X.509 chain depth, including leaf and terminal root.
pub const ZK_X509_MIN_CHAIN_DEPTH_V1: u8 = 2;
/// Maximum accepted lag from signed CRL `thisUpdate` to the presentation-window end.
pub const ZK_X509_MAX_CRL_AGE_SECONDS_V1: u64 = 300;
/// Maximum public presentation window covered by one X.509 proof.
///
/// The proof establishes certificate and signed-CRL validity for the complete
/// window; consensus then checks the unpredictable inclusion timestamp lies
/// inside it. Keeping this equal to the CRL-age ceiling preserves the strict
/// five-minute freshness profile.
pub const ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1: u64 = ZK_X509_MAX_CRL_AGE_SECONDS_V1;
/// Maximum DER bytes for one X.509 certificate in the canonical proof topology.
///
/// This is deliberately identical to the native witness codec, RFC 5280 AIR,
/// and fixed-capacity SHA-256 schedule. The first release has no larger
/// API-only tier that could be admitted but not proved.
pub const ZK_X509_MAX_CERTIFICATE_BYTES_V1: u32 = 4 * 1024;
/// Maximum combined DER bytes for an admitted X.509 chain.
pub const ZK_X509_MAX_CHAIN_BYTES_V1: u32 = ZK_X509_MAX_CERTIFICATE_BYTES_V1 * 3;
/// Closed number of selectively disclosable X.509 subject attributes.
pub const ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1: usize = 4;
/// Closed number of extended-key-usage purposes in the first-release profile.
pub const ZK_X509_MAX_EXTENDED_KEY_USAGES_V1: usize = 3;
/// Maximum immutable trust-anchor revisions retained across all lineages.
pub const ZK_X509_MAX_TRUST_ANCHOR_RECORDS_V1: usize = 4_096;
/// Maximum immutable certificate-policy revisions retained across all lineages.
pub const ZK_X509_MAX_CERTIFICATE_POLICY_RECORDS_V1: usize = 4_096;
/// Maximum current issuer-scoped signed-CRL lineages in world state.
///
/// Unlike rare trust-anchor and policy governance, CRLs rotate frequently.
/// Consensus therefore stores one self-chained current record per policy
/// lineage instead of retaining an eventually terminal fixed revision count.
pub const ZK_X509_MAX_CRL_LINEAGES_V1: usize = 4_096;
/// Maximum immutable revisions retained for one trust-anchor or policy lineage.
pub const ZK_X509_MAX_RECORD_REVISIONS_PER_LINEAGE_V1: usize = 64;
/// Canonical origin epoch for an X.509 trust-anchor or certificate-policy lineage.
pub const ZK_X509_INITIAL_RECORD_EPOCH_V1: u64 = 1;
/// Maximum Orchard actions in one first-release bundle.
pub const ORCHARD_MAX_ACTIONS_V1: u32 = 2;
/// Exact Orchard V3 encrypted-note ciphertext width.
pub const ORCHARD_ENCRYPTED_NOTE_BYTES_V1: usize = 580;
/// Exact Orchard V3 outgoing ciphertext width.
pub const ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1: usize = 80;
/// Largest public Orchard value balance representable by the pinned native API.
pub const ORCHARD_MAX_VALUE_BALANCE_V1: u128 = i64::MAX as u128;
/// Maximum FCMP++ consumed outputs in one first-release transfer.
pub const FCMP_MAX_INPUTS_V1: u32 = 2;
/// Maximum FCMP++ new outputs in one first-release transfer.
pub const FCMP_MAX_OUTPUTS_V1: u32 = 4;
/// Maximum native IVM private-note inputs in one first-release action.
pub const IVM_PRIVATE_NOTE_MAX_INPUTS_V1: u32 = 2;
/// Maximum native IVM private-note outputs in one first-release action.
pub const IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1: u32 = 2;
/// Maximum PQ-MASP inputs in one first-release action.
pub const PQ_MASP_MAX_INPUTS_V1: u32 = 2;
/// Maximum PQ-MASP outputs in one first-release action.
pub const PQ_MASP_MAX_OUTPUTS_V1: u32 = 2;
/// Maximum genesis commitments in one typed proof-managed pool bootstrap.
pub const PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1: usize = 4_096;
/// Maximum UTF-8 byte length admitted for a privacy transcript chain id.
pub const PRIVACY_MAX_CHAIN_ID_BYTES_V1: u32 = 255;

/// Explicit chain and governed-artifact binding shared by every statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyStatementContextV1 {
    /// Exact chain identifier.
    pub chain_id: ChainId,
    /// Zero-based privacy action index within the transaction.
    pub action_index: u32,
    /// Digest of the canonical transaction projection with derived privacy
    /// digests zeroed and the typed proof payload empty.
    pub transaction_intent_digest: PrivacyTransactionIntentDigestV1,
    /// Exact governed parameter-set identifier.
    pub parameter_id: PrivacyParameterIdV1,
    /// Digest of the governed parameter set.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Digest of the exact verifier artifact.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Digest of this protocol's public-statement schema.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Digest of the pinned experimental native engine manifest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
}

impl PrivacyStatementContextV1 {
    /// Validate transcript context and non-zero governed artifact bindings.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyStatementValidationError`] for an invalid chain id,
    /// action index, or fixed artifact binding.
    pub fn validate(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyStatementValidationError> {
        let chain_id_bytes = u32::try_from(self.chain_id.as_str().len())
            .map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)?;
        if chain_id_bytes == 0 || chain_id_bytes > PRIVACY_MAX_CHAIN_ID_BYTES_V1 {
            return Err(PrivacyStatementValidationError::InvalidChainIdLength {
                bytes: chain_id_bytes,
                max: PRIVACY_MAX_CHAIN_ID_BYTES_V1,
            });
        }
        if self.action_index >= limits.max_actions_per_transaction {
            return Err(PrivacyStatementValidationError::ActionIndexOutOfBounds {
                index: self.action_index,
                max_actions: limits.max_actions_per_transaction,
            });
        }
        if self.transaction_intent_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroTransactionIntentDigest);
        }
        if self.parameter_id.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroParameterId);
        }
        if self.parameter_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroParameterDigest);
        }
        if self.verifier_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroVerifierDigest);
        }
        if self.statement_schema_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroStatementSchemaDigest);
        }
        if self.engine_manifest_digest.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroEngineManifestDigest);
        }
        Ok(())
    }
}

/// Typed encrypted output emitted by a private transfer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyEncryptedOutputV1 {
    /// Cryptographic recipient identity.
    pub recipient: PrivacyRecipientIdV1,
    /// Protocol-defined identifier for the ephemeral encryption material.
    ///
    /// Diffie-Hellman profiles use a public key here. The fixed PQ-MASP
    /// profile uses the domain-separated digest of its ML-KEM-768
    /// encapsulation ciphertext; the full 1,088-byte encapsulation remains in
    /// `ciphertext`.
    pub ephemeral_public_key: PrivacyEncryptionKeyV1,
    /// Commitment to the plaintext output.
    pub commitment: PrivacyCommitmentV1,
    /// Canonical authenticated ciphertext bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub ciphertext: Vec<u8>,
}

/// Closed lifecycle of one authoritative ZK-ACE authorization policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "state", content = "value", deny_unknown_fields)]
pub enum PrivacyZkAcePolicyLifecycleV1 {
    /// The policy can authorize a matching proof action.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active,
    /// The policy was irreversibly revoked.
    #[cfg_attr(feature = "json", norito(rename = "revoked"))]
    Revoked,
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(schema_name = "iroha.privacy.zk-ace.policy-digest-material.v1")]
struct PrivacyZkAcePolicyDigestMaterialV1 {
    policy_id: PrivacyPolicyIdV1,
    identity_commitment: PrivacyCommitmentV1,
    policy_digest: PrivacyPolicyDigestV1,
    authorization_epoch: u64,
    asset_definition_id: AssetDefinitionId,
    source_allowlist: Vec<AccountId>,
    lifecycle: PrivacyZkAcePolicyLifecycleV1,
}

/// Complete authoritative policy selected by a ZK-ACE authorization statement.
///
/// `record_digest` commits every preceding field. The allowlist is stored in
/// strict account-id order so snapshots, governance instructions, and proof
/// preflight all have exactly one canonical representation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAcePolicyRecordV1 {
    /// Stable lookup key for this policy lineage.
    pub policy_id: PrivacyPolicyIdV1,
    /// Exact identity commitment authorized by the current policy epoch.
    pub identity_commitment: PrivacyCommitmentV1,
    /// Digest of the governed authorization policy.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Strictly increasing governance epoch.
    pub authorization_epoch: u64,
    /// Exact transparent asset definition authorized by this policy.
    pub asset_definition_id: AssetDefinitionId,
    /// Strictly sorted, unique, non-empty set of authorized source accounts.
    pub source_allowlist: Vec<AccountId>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyZkAcePolicyLifecycleV1,
    /// Digest of every authoritative field above.
    pub record_digest: PrivacyZkAcePolicyRecordDigestV1,
}

impl PrivacyZkAcePolicyRecordV1 {
    /// Construct one canonical self-digested policy record.
    ///
    /// # Errors
    ///
    /// Rejects a zero identifier, commitment, digest, or epoch; an empty,
    /// oversized, unsorted, or duplicate allowlist; or a digest encoding
    /// failure.
    pub fn new(
        policy_id: PrivacyPolicyIdV1,
        identity_commitment: PrivacyCommitmentV1,
        policy_digest: PrivacyPolicyDigestV1,
        authorization_epoch: u64,
        asset_definition_id: AssetDefinitionId,
        source_allowlist: Vec<AccountId>,
        lifecycle: PrivacyZkAcePolicyLifecycleV1,
    ) -> Result<Self, PrivacyZkAcePolicyRecordValidationErrorV1> {
        let mut record = Self {
            policy_id,
            identity_commitment,
            policy_digest,
            authorization_epoch,
            asset_definition_id,
            source_allowlist,
            lifecycle,
            record_digest: PrivacyZkAcePolicyRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest()?;
        if record.record_digest.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate an initial policy registration.
    ///
    /// # Errors
    ///
    /// Requires a valid active record at the canonical origin epoch.
    pub fn validate_initial(&self) -> Result<(), PrivacyZkAcePolicyRecordValidationErrorV1> {
        self.validate()?;
        if self.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::InitialPolicyNotActive);
        }
        if self.authorization_epoch != PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1 {
            return Err(
                PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalInitialEpoch {
                    actual: self.authorization_epoch,
                },
            );
        }
        Ok(())
    }

    /// Validate this record, including its canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed authoritative field or self-digest mismatch.
    pub fn validate(&self) -> Result<(), PrivacyZkAcePolicyRecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest()? != self.record_digest {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the canonical digest of every authoritative record field.
    ///
    /// # Errors
    ///
    /// Returns an encoding error when the canonical digest material cannot be
    /// serialized.
    pub fn compute_record_digest(
        &self,
    ) -> Result<PrivacyZkAcePolicyRecordDigestV1, PrivacyZkAcePolicyRecordValidationErrorV1> {
        let material = PrivacyZkAcePolicyDigestMaterialV1 {
            policy_id: self.policy_id,
            identity_commitment: self.identity_commitment,
            policy_digest: self.policy_digest,
            authorization_epoch: self.authorization_epoch,
            asset_definition_id: self.asset_definition_id.clone(),
            source_allowlist: self.source_allowlist.clone(),
            lifecycle: self.lifecycle,
        };
        let encoded = norito::encode_canonical(&material)
            .map_err(|_| PrivacyZkAcePolicyRecordValidationErrorV1::EncodingFailure)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(ZK_ACE_POLICY_RECORD_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyZkAcePolicyRecordDigestV1::new(
            *hasher.finalize().as_bytes(),
        ))
    }

    fn validate_contents(&self) -> Result<(), PrivacyZkAcePolicyRecordValidationErrorV1> {
        if self.policy_id.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroPolicyId);
        }
        if self.identity_commitment.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroIdentityCommitment);
        }
        if self.policy_digest.is_zero() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroPolicyDigest);
        }
        if self.authorization_epoch == 0 {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroAuthorizationEpoch);
        }
        if self.source_allowlist.is_empty() {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::EmptySourceAllowlist);
        }
        if self.source_allowlist.len() > PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1 {
            return Err(
                PrivacyZkAcePolicyRecordValidationErrorV1::SourceAllowlistTooLarge {
                    actual: self.source_allowlist.len(),
                    max: PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1,
                },
            );
        }
        if self
            .source_allowlist
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalSourceAllowlist);
        }
        Ok(())
    }
}

/// Failure while validating one authoritative ZK-ACE policy record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkAcePolicyRecordValidationErrorV1 {
    /// The lookup identifier is all zero.
    #[error("ZK-ACE policy id must be non-zero")]
    ZeroPolicyId,
    /// The identity commitment is all zero.
    #[error("ZK-ACE identity commitment must be non-zero")]
    ZeroIdentityCommitment,
    /// The governed policy digest is all zero.
    #[error("ZK-ACE policy digest must be non-zero")]
    ZeroPolicyDigest,
    /// Epoch zero is not a valid governed policy state.
    #[error("ZK-ACE authorization epoch must be non-zero")]
    ZeroAuthorizationEpoch,
    /// Registration must begin at the canonical origin epoch.
    #[error(
        "initial ZK-ACE authorization epoch must be {PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1}, got {actual}"
    )]
    NonCanonicalInitialEpoch {
        /// Rejected epoch.
        actual: u64,
    },
    /// Registration cannot create an already-revoked policy.
    #[error("initial ZK-ACE policy must be active")]
    InitialPolicyNotActive,
    /// A policy must authorize at least one source account.
    #[error("ZK-ACE source allowlist must be non-empty")]
    EmptySourceAllowlist,
    /// The first-release fixed account bound was exceeded.
    #[error("ZK-ACE source allowlist has {actual} entries; maximum is {max}")]
    SourceAllowlistTooLarge {
        /// Rejected entry count.
        actual: usize,
        /// Fixed first-release maximum.
        max: usize,
    },
    /// The allowlist is not in strict unique account-id order.
    #[error("ZK-ACE source allowlist must be strictly sorted and unique")]
    NonCanonicalSourceAllowlist,
    /// Canonical encoding of the self-digest material failed.
    #[error("ZK-ACE policy record digest material could not be encoded")]
    EncodingFailure,
    /// A decoded record supplied an all-zero self-digest.
    #[error("ZK-ACE policy record self-digest must be non-zero")]
    ZeroRecordDigest,
    /// Recomputing the complete record produced a different digest.
    #[error("ZK-ACE policy record self-digest mismatch")]
    RecordDigestMismatch,
}

/// Failure while validating a canonical ZK-ACE governance transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkAcePolicyTransitionValidationErrorV1 {
    /// The persisted current record is malformed.
    #[error("current ZK-ACE policy record is invalid: {0}")]
    InvalidCurrent(PrivacyZkAcePolicyRecordValidationErrorV1),
    /// The proposed successor record is malformed.
    #[error("successor ZK-ACE policy record is invalid: {0}")]
    InvalidSuccessor(PrivacyZkAcePolicyRecordValidationErrorV1),
    /// A revoked policy cannot transition again.
    #[error("current ZK-ACE policy is not active")]
    CurrentNotActive,
    /// A rotation must retain the stable policy identifier.
    #[error("ZK-ACE transition changed policy id")]
    PolicyIdMismatch,
    /// An epoch cannot advance past `u64::MAX`.
    #[error("ZK-ACE authorization epoch overflow")]
    EpochOverflow,
    /// The successor did not advance exactly one canonical epoch.
    #[error("ZK-ACE successor epoch must be {expected}, got {actual}")]
    NonCanonicalSuccessorEpoch {
        /// Required successor epoch.
        expected: u64,
        /// Rejected successor epoch.
        actual: u64,
    },
    /// A rotation successor must remain active.
    #[error("ZK-ACE rotation successor must be active")]
    RotationSuccessorNotActive,
    /// A rotation must actually replace the identity commitment.
    #[error("ZK-ACE rotation requires a distinct identity commitment")]
    IdentityCommitmentUnchanged,
    /// A revocation successor must be revoked.
    #[error("ZK-ACE revocation successor must be revoked")]
    RevocationSuccessorNotRevoked,
    /// Revocation may change only lifecycle, epoch, and the resulting self-digest.
    #[error("ZK-ACE revocation changed immutable policy contents")]
    RevocationContentsChanged,
}

/// Validate an active-to-active canonical ZK-ACE rotation.
///
/// # Errors
///
/// Rejects malformed records, stale or skipped epochs, policy-id changes, and
/// no-op identity rotations.
pub fn validate_zk_ace_policy_rotation_v1(
    current: &PrivacyZkAcePolicyRecordV1,
    successor: &PrivacyZkAcePolicyRecordV1,
) -> Result<(), PrivacyZkAcePolicyTransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidSuccessor)?;
    if current.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::CurrentNotActive);
    }
    if successor.policy_id != current.policy_id {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::PolicyIdMismatch);
    }
    let expected = current
        .authorization_epoch
        .checked_add(1)
        .ok_or(PrivacyZkAcePolicyTransitionValidationErrorV1::EpochOverflow)?;
    if successor.authorization_epoch != expected {
        return Err(
            PrivacyZkAcePolicyTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                expected,
                actual: successor.authorization_epoch,
            },
        );
    }
    if successor.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    if successor.identity_commitment == current.identity_commitment {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::IdentityCommitmentUnchanged);
    }
    Ok(())
}

/// Validate an irreversible canonical ZK-ACE revocation.
///
/// # Errors
///
/// Rejects malformed records, stale or skipped epochs, and any mutation other
/// than lifecycle, epoch, and the corresponding self-digest.
pub fn validate_zk_ace_policy_revocation_v1(
    current: &PrivacyZkAcePolicyRecordV1,
    successor: &PrivacyZkAcePolicyRecordV1,
) -> Result<(), PrivacyZkAcePolicyTransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidSuccessor)?;
    if current.lifecycle != PrivacyZkAcePolicyLifecycleV1::Active {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::CurrentNotActive);
    }
    if successor.policy_id != current.policy_id {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::PolicyIdMismatch);
    }
    let expected = current
        .authorization_epoch
        .checked_add(1)
        .ok_or(PrivacyZkAcePolicyTransitionValidationErrorV1::EpochOverflow)?;
    if successor.authorization_epoch != expected {
        return Err(
            PrivacyZkAcePolicyTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                expected,
                actual: successor.authorization_epoch,
            },
        );
    }
    if successor.lifecycle != PrivacyZkAcePolicyLifecycleV1::Revoked {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.identity_commitment != current.identity_commitment
        || successor.policy_digest != current.policy_digest
        || successor.asset_definition_id != current.asset_definition_id
        || successor.source_allowlist != current.source_allowlist
    {
        return Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// ZK-ACE authorization statement for a public asset transfer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.zk-ace.authorization-statement.v1")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ZkAcePqAuthorizationStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Identity commitment authorized by the policy.
    pub identity_commitment: PrivacyCommitmentV1,
    /// Exact authorization policy identifier.
    pub policy_id: PrivacyPolicyIdV1,
    /// Digest of the authorization policy.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Public source account.
    pub source: AccountId,
    /// Public destination account.
    pub destination: AccountId,
    /// Public transferred asset definition.
    pub asset_definition_id: AssetDefinitionId,
    /// Atomic transfer amount.
    pub amount: u128,
    /// Ledger epoch used by authorization policy checks.
    pub authorization_epoch: u64,
    /// Per-action replay nullifier.
    pub replay_nullifier: PrivacyNullifierV1,
}

/// Anonymous PGC k-out-of-n private payment statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct AnonymousPgcKOutOfNStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset transferred by the confidential payment.
    pub asset_definition_id: AssetDefinitionId,
    /// Anonymous-account pool namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Current encrypted PGC account-state root.
    pub account_state_root: PrivacyRootV1,
    /// Epoch at which `account_state_root` was canonical.
    pub account_state_root_epoch: u64,
    /// Resulting encrypted PGC account-state root.
    pub next_account_state_root: PrivacyRootV1,
    /// Successor epoch committed with `next_account_state_root`.
    pub next_account_state_root_epoch: u64,
    /// Ordered anonymity-set public keys `(pk_0, …, pk_{n-1})`.
    pub anonymity_set_public_keys: Vec<PrivacyP256PointV1>,
    /// Ordered transfer ciphertexts `(C_0, …, C_{n-1})`, matching the keys.
    pub transfer_ciphertexts: Vec<PrivacyP256CiphertextV1>,
    /// Number `k` of intended positive-value recipients.
    pub recipient_count: u32,
}

/// Bit width admitted by the Iroha VeRange Type-1 profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "bits", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVeRangeBitLengthV1 {
    /// 32-bit unsigned range.
    Bits32,
    /// 64-bit unsigned range.
    Bits64,
}

impl PrivacyVeRangeBitLengthV1 {
    /// Return the exact numeric bit width.
    #[must_use]
    pub const fn bits(self) -> u16 {
        match self {
            Self::Bits32 => 32,
            Self::Bits64 => 64,
        }
    }
}

/// Iroha Type-1 P-256/SHA-256 unsigned range-proof statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct VeRangeTransparentRangeStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset whose atomic values are committed.
    pub asset_definition_id: AssetDefinitionId,
    /// Policy selecting the commitment domain and admitted bit width.
    pub policy_id: PrivacyPolicyIdV1,
    /// Value commitments proved in this aggregate.
    pub value_commitments: Vec<PrivacyP256PointV1>,
    /// Closed first-release range `[0, 2^N)` proved for each committed value.
    pub bit_length: PrivacyVeRangeBitLengthV1,
    /// Number of aggregated value commitments.
    pub aggregation_count: u32,
}

/// Exact wire version of [`PrivacyZkAmsPersonhoodCredentialV1`].
pub const ZK_AMS_PHC_VERSION_V1: u8 = 1;
/// Exact byte width of the closed canonical PHC payload.
pub const ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1: usize = 161;
/// The only initial epoch admitted by the ZK-AMS registry bootstrap.
pub const ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1: u64 = 1;
/// Exact issuer-policy record preimage width.
pub const ZK_AMS_ISSUER_POLICY_RECORD_PAYLOAD_BYTES_V1: usize = 129;
/// Exact registry-snapshot record preimage width.
pub const ZK_AMS_REGISTRY_RECORD_PAYLOAD_BYTES_V1: usize = 200;
/// Exact registry-bootstrap provenance preimage width.
pub const ZK_AMS_REGISTRY_BOOTSTRAP_PAYLOAD_BYTES_V1: usize = 201;

/// Canonical governed origin for one ZK-AMS admitted-identity registry.
///
/// This is the only first-release instruction payload that may initialize an
/// `AccountRegistry` root. It fixes the issuer key, admission policy, registry
/// namespace, and exact nonzero origin root in one atomic governance action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsRegistryBootstrapV1 {
    /// Credential issuer authorized to sign canonical PHCs.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Admitted-identity registry initialized by this record.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Exact governed admission policy.
    pub policy_id: PrivacyPolicyIdV1,
    /// Canonical compressed SEC1 P-256 issuer verification key.
    pub issuer_public_key: PrivacyP256PointV1,
    /// Digest of the complete governed admission policy.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Nonzero origin of the proof-managed admitted-identity registry.
    pub initial_registry_root: PrivacyRootV1,
    /// Closed origin epoch; exactly
    /// [`ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1`].
    pub initial_registry_epoch: u64,
}

impl PrivacyZkAmsRegistryBootstrapV1 {
    /// Derive the sole protocol-scoped namespace governed by this bootstrap.
    #[must_use]
    pub const fn namespace(self) -> PrivacyNamespaceV1 {
        PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaZkAmsV1,
            PrivacyNamespaceScopeV1::IssuerRegistryPolicy(PrivacyIssuerRegistryPolicyNamespaceV1 {
                issuer_id: self.issuer_id,
                registry_id: self.registry_id,
                policy_id: self.policy_id,
            }),
        )
    }

    /// Validate every closed nonzero field and the exact origin epoch.
    ///
    /// Core additionally parses `issuer_public_key` as a canonical,
    /// non-identity P-256 point before persistence.
    pub fn validate(&self) -> Result<(), PrivacyZkAmsRegistryBootstrapValidationError> {
        if self.issuer_id.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroIssuerId);
        }
        if self.registry_id.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroRegistryId);
        }
        if self.policy_id.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroPolicyId);
        }
        if self.issuer_public_key.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroIssuerPublicKey);
        }
        if self.policy_digest.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroPolicyDigest);
        }
        if self.initial_registry_root.is_zero() {
            return Err(PrivacyZkAmsRegistryBootstrapValidationError::ZeroInitialRoot);
        }
        if self.initial_registry_epoch != ZK_AMS_REGISTRY_BOOTSTRAP_INITIAL_EPOCH_V1 {
            return Err(
                PrivacyZkAmsRegistryBootstrapValidationError::NonCanonicalInitialEpoch {
                    epoch: self.initial_registry_epoch,
                },
            );
        }
        self.namespace()
            .validate()
            .map_err(|_| PrivacyZkAmsRegistryBootstrapValidationError::InvalidNamespace)
    }

    /// Derive the authoritative issuer-key/policy record digest.
    #[must_use]
    pub fn issuer_policy_record_digest(self) -> PrivacyZkAmsIssuerPolicyRecordDigestV1 {
        zk_ams_issuer_policy_record_digest_v1(
            self.issuer_id,
            self.policy_id,
            self.issuer_public_key,
            self.policy_digest,
        )
    }

    /// Derive the authoritative origin registry-snapshot record digest.
    #[must_use]
    pub fn registry_record_digest(self) -> PrivacyZkAmsRegistryRecordDigestV1 {
        zk_ams_registry_record_digest_v1(
            self.issuer_id,
            self.registry_id,
            self.policy_id,
            self.issuer_policy_record_digest(),
            self.policy_digest,
            self.initial_registry_root,
            self.initial_registry_epoch,
        )
    }

    /// Hash the exact fixed bootstrap fields in their provenance domain.
    #[must_use]
    pub fn digest(self) -> PrivacyZkAmsRegistryBootstrapDigestV1 {
        let mut payload = [0_u8; ZK_AMS_REGISTRY_BOOTSTRAP_PAYLOAD_BYTES_V1];
        payload[0..32].copy_from_slice(self.issuer_id.as_bytes());
        payload[32..64].copy_from_slice(self.registry_id.as_bytes());
        payload[64..96].copy_from_slice(self.policy_id.as_bytes());
        payload[96..129].copy_from_slice(self.issuer_public_key.as_bytes());
        payload[129..161].copy_from_slice(self.policy_digest.as_bytes());
        payload[161..193].copy_from_slice(self.initial_registry_root.as_bytes());
        payload[193..201].copy_from_slice(&self.initial_registry_epoch.to_be_bytes());
        let mut hasher = Sha256::new();
        hasher.update(ZK_AMS_REGISTRY_BOOTSTRAP_DIGEST_DOMAIN_V1);
        hasher.update(
            u64::try_from(payload.len())
                .expect("fixed ZK-AMS bootstrap payload length fits u64")
                .to_le_bytes(),
        );
        hasher.update(payload);
        PrivacyZkAmsRegistryBootstrapDigestV1::new(hasher.finalize().into())
    }
}

/// Derive one exact authoritative ZK-AMS issuer-key/policy record digest.
#[must_use]
pub fn zk_ams_issuer_policy_record_digest_v1(
    issuer_id: PrivacyIssuerIdV1,
    policy_id: PrivacyPolicyIdV1,
    issuer_public_key: PrivacyP256PointV1,
    policy_digest: PrivacyPolicyDigestV1,
) -> PrivacyZkAmsIssuerPolicyRecordDigestV1 {
    let mut payload = [0_u8; ZK_AMS_ISSUER_POLICY_RECORD_PAYLOAD_BYTES_V1];
    payload[0..32].copy_from_slice(issuer_id.as_bytes());
    payload[32..64].copy_from_slice(policy_id.as_bytes());
    payload[64..97].copy_from_slice(issuer_public_key.as_bytes());
    payload[97..129].copy_from_slice(policy_digest.as_bytes());
    let mut hasher = Sha256::new();
    hasher.update(ZK_AMS_ISSUER_POLICY_RECORD_DIGEST_DOMAIN_V1);
    hasher.update(
        u64::try_from(payload.len())
            .expect("fixed ZK-AMS issuer-policy payload length fits u64")
            .to_le_bytes(),
    );
    hasher.update(payload);
    PrivacyZkAmsIssuerPolicyRecordDigestV1::new(hasher.finalize().into())
}

/// Derive one exact authoritative ZK-AMS registry-snapshot record digest.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn zk_ams_registry_record_digest_v1(
    issuer_id: PrivacyIssuerIdV1,
    registry_id: PrivacyZkAmsRegistryIdV1,
    policy_id: PrivacyPolicyIdV1,
    issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    policy_digest: PrivacyPolicyDigestV1,
    registry_root: PrivacyRootV1,
    registry_epoch: u64,
) -> PrivacyZkAmsRegistryRecordDigestV1 {
    let mut payload = [0_u8; ZK_AMS_REGISTRY_RECORD_PAYLOAD_BYTES_V1];
    payload[0..32].copy_from_slice(issuer_id.as_bytes());
    payload[32..64].copy_from_slice(registry_id.as_bytes());
    payload[64..96].copy_from_slice(policy_id.as_bytes());
    payload[96..128].copy_from_slice(issuer_policy_record_digest.as_bytes());
    payload[128..160].copy_from_slice(policy_digest.as_bytes());
    payload[160..192].copy_from_slice(registry_root.as_bytes());
    payload[192..200].copy_from_slice(&registry_epoch.to_be_bytes());
    let mut hasher = Sha256::new();
    hasher.update(ZK_AMS_REGISTRY_RECORD_DIGEST_DOMAIN_V1);
    hasher.update(
        u64::try_from(payload.len())
            .expect("fixed ZK-AMS registry-record payload length fits u64")
            .to_le_bytes(),
    );
    hasher.update(payload);
    PrivacyZkAmsRegistryRecordDigestV1::new(hasher.finalize().into())
}

/// Structural failure for [`PrivacyZkAmsRegistryBootstrapV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkAmsRegistryBootstrapValidationError {
    /// Issuer id is the zero sentinel.
    #[error("ZK-AMS registry bootstrap issuer id must be nonzero")]
    ZeroIssuerId,
    /// Registry id is the zero sentinel.
    #[error("ZK-AMS registry bootstrap registry id must be nonzero")]
    ZeroRegistryId,
    /// Policy id is the zero sentinel.
    #[error("ZK-AMS registry bootstrap policy id must be nonzero")]
    ZeroPolicyId,
    /// Issuer public key is the all-zero sentinel.
    #[error("ZK-AMS registry bootstrap issuer public key must be nonzero")]
    ZeroIssuerPublicKey,
    /// Policy digest is the zero sentinel.
    #[error("ZK-AMS registry bootstrap policy digest must be nonzero")]
    ZeroPolicyDigest,
    /// Initial registry root is the zero sentinel.
    #[error("ZK-AMS registry bootstrap root must be nonzero")]
    ZeroInitialRoot,
    /// Initial epoch differs from the only closed first-release origin.
    #[error("ZK-AMS registry bootstrap initial epoch must be 1, got {epoch}")]
    NonCanonicalInitialEpoch {
        /// Rejected caller-provided epoch.
        epoch: u64,
    },
    /// Derived namespace is invalid.
    #[error("ZK-AMS registry bootstrap namespace is invalid")]
    InvalidNamespace,
}

/// Fixed typed Personhood Credential admitted by the Iroha ZK-AMS profile.
///
/// The issuer authenticates the domain-separated SHA-256 digest of the exact
/// canonical Norito encoding. The holder proves possession of the Ristretto
/// seed secret over the same digest in the composed admission proof. No
/// variable-length or free-form field is admitted by this first-release type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsPersonhoodCredentialV1 {
    /// Closed credential wire version; must equal one.
    pub version: u8,
    /// Governed issuer that authenticated the credential.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Governed policy under which the credential was issued.
    pub policy_id: PrivacyPolicyIdV1,
    /// Hidden commitment to the issuer-validated personhood subject.
    pub subject_commitment: PrivacyZkAmsSubjectCommitmentV1,
    /// Ristretto seed key later used for anonymous provisioning.
    pub seed_public_key: PrivacyZkAmsSeedPublicKeyV1,
    /// Issuer-selected uniqueness nonce.
    pub credential_nonce: PrivacyZkAmsCredentialNonceV1,
}

impl PrivacyZkAmsPersonhoodCredentialV1 {
    /// Return the exact fixed typed-Norito payload signed by the issuer.
    ///
    /// The payload is closed to
    /// `version || issuer_id || policy_id || subject_commitment ||
    /// seed_public_key || credential_nonce`. Every field has a fixed width, so
    /// no optional, offset, or length table can introduce an alternative
    /// preimage.
    #[must_use]
    pub fn canonical_payload(&self) -> PrivacyZkAmsPhcCanonicalPayloadV1 {
        let mut payload = [0_u8; ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1];
        payload[0] = self.version;
        payload[1..33].copy_from_slice(self.issuer_id.as_bytes());
        payload[33..65].copy_from_slice(self.policy_id.as_bytes());
        payload[65..97].copy_from_slice(self.subject_commitment.as_bytes());
        payload[97..129].copy_from_slice(self.seed_public_key.as_bytes());
        payload[129..161].copy_from_slice(self.credential_nonce.as_bytes());
        PrivacyZkAmsPhcCanonicalPayloadV1(payload)
    }

    /// Hash the exact typed credential payload with domain and length framing.
    #[must_use]
    pub fn digest(&self) -> PrivacyZkAmsPhcHashV1 {
        let payload = self.canonical_payload();
        let mut hasher = Sha256::new();
        hasher.update(ZK_AMS_PHC_HASH_DOMAIN_V1);
        hasher.update(
            u64::try_from(payload.as_bytes().len())
                .expect("Norito output length fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(payload.as_bytes());
        PrivacyZkAmsPhcHashV1::new(hasher.finalize().into())
    }
}

/// Exact fixed typed-Norito preimage of a ZK-AMS Personhood Credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PrivacyZkAmsPhcCanonicalPayloadV1(
    /// Exact closed credential payload.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub [u8; ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1],
);

impl PrivacyZkAmsPhcCanonicalPayloadV1 {
    /// Borrow the exact canonical payload.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ZK_AMS_PHC_CANONICAL_PAYLOAD_BYTES_V1] {
        &self.0
    }
}

/// One ordered public admission anchor from ZK-AMS batch input `X`.
///
/// The order of these pairs is part of the Fiat-Shamir transcript certified
/// by the batch proof. Validation therefore preserves caller order and rejects
/// duplicate credential hashes or seed public keys without sorting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsAdmissionAnchorV1 {
    /// Hash of the canonical Personhood Credential.
    pub phc_hash: PrivacyZkAmsPhcHashV1,
    /// Seed public key later used for anonymous account provisioning.
    pub seed_public_key: PrivacyZkAmsSeedPublicKeyV1,
}

/// Setup-free Iroha instantiation of ZK-AMS batch settlement.
///
/// The native proof recursively folds one fixed credential relation for every
/// ordered anchor and proves the final relaxed instance with a freshly masked
/// Relaxed Spartan proof. Intermediate accumulator and cross-term commitments
/// are already canonical proof sections; duplicating caller-selected digests
/// in the public statement would be circular and is deliberately forbidden.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsBatchAdmissionV1 {
    /// Current admitted-identity registry root.
    pub account_registry_root: PrivacyRootV1,
    /// Epoch at which `account_registry_root` is canonical.
    pub account_registry_root_epoch: u64,
    /// Resulting registry root after atomically recording all ordered anchors.
    pub next_account_registry_root: PrivacyRootV1,
    /// Exact successor epoch committed with `next_account_registry_root`.
    pub next_account_registry_root_epoch: u64,
    /// Ordered `{hash_PHC, pk_seed}` batch input `X`.
    pub anchors: Vec<PrivacyZkAmsAdmissionAnchorV1>,
}

/// ZK-AMS Phase-V anonymous account-provisioning public input.
///
/// The native suite verifies MLSAGS over Ristretto255 with a SHA3-512
/// transcript and hash-to-group operation. Every ring key must be present in
/// the referenced admitted-identity registry. The account id is the signed
/// message binding, and the key image is the one-time replay nullifier.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkAmsProvisionAccountV1 {
    /// Canonical admitted-identity registry root used for ring membership.
    pub account_registry_root: PrivacyRootV1,
    /// Epoch at which `account_registry_root` is canonical.
    pub account_registry_root_epoch: u64,
    /// Strictly increasing canonical ring of admitted seed public keys.
    pub admitted_seed_key_ring: Vec<PrivacyZkAmsSeedPublicKeyV1>,
    /// Fresh Iroha account/address bound by the MLSAGS signature.
    pub account_id: AccountId,
    /// Deterministic MLSAGS key image recorded as the provisioning nullifier.
    pub key_image: PrivacyZkAmsKeyImageV1,
}

/// Closed ZK-AMS chain action.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "action", content = "value", deny_unknown_fields)
)]
pub enum PrivacyZkAmsActionV1 {
    /// Settle one recursively accumulated admission batch.
    BatchAdmission(PrivacyZkAmsBatchAdmissionV1),
    /// Provision one anonymous account from an admitted seed-key ring.
    ProvisionAccount(PrivacyZkAmsProvisionAccountV1),
}

/// Native ZK-AMS batch-admission and anonymous-provisioning statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct IrohaZkAmsStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Credential issuer governing the common admission relation.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Canonical compressed P-256 issuer key copied from authoritative state.
    ///
    /// Consensus must match this value to `issuer_policy_record_digest`; it is
    /// a transcript input, not a caller-selected trust anchor.
    pub issuer_public_key: PrivacyP256PointV1,
    /// Digest of the authoritative issuer/policy/key record.
    pub issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1,
    /// Admitted-identity and provisioning registry.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Digest of the authoritative registry snapshot referenced by the action.
    pub registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1,
    /// Admission policy identifier.
    pub policy_id: PrivacyPolicyIdV1,
    /// Digest of the exact governed admission policy.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Exact batch-settlement or account-provisioning action.
    pub action: PrivacyZkAmsActionV1,
}

/// Credential document family admitted by the Vega first-release profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "document", content = "value", deny_unknown_fields)
)]
pub enum PrivacyCredentialDocumentTypeV1 {
    /// ISO/IEC 18013-5 `org.iso.18013.5.1.mDL` document.
    Iso18013_5Mdl,
}

/// Closed ISO/IEC 18013-5 namespace admitted by the Vega mDL-age profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "namespace", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVegaMdlNamespaceV1 {
    /// The standard mDL namespace `org.iso.18013.5.1`.
    OrgIso18013_5_1,
}

/// Closed digest algorithm used throughout the Vega mDL-age circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "digest", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVegaMdlDigestAlgorithmV1 {
    /// SHA-256 for issuer authentication, signed-item digests, and `H_dev`.
    Sha256,
}

/// Closed COSE signature algorithm used by issuer and device authentication.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "signature", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVegaMdlSignatureAlgorithmV1 {
    /// COSE algorithm `-7`: ECDSA over P-256 with SHA-256 (`ES256`).
    CoseSign1Es256,
}

/// Forward-only lifecycle of one immutable Vega issuer governance lineage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "value", deny_unknown_fields)
)]
pub enum PrivacyVegaIssuerRecordLifecycleV1 {
    /// Credentials authenticated by this exact issuer revision may be verified.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active,
    /// The issuer lineage is terminal and cannot be reactivated.
    #[cfg_attr(feature = "json", norito(rename = "revoked"))]
    Revoked,
}

/// One immutable authoritative Vega mDL issuer-key and algorithm-policy revision.
///
/// Revisions form a bounded append-only self-digested lineage. A proof
/// statement must bind the exact current active revision, including its P-256
/// key, so a submitter cannot manufacture a self-issued credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyVegaIssuerRecordV1 {
    /// Stable credential-issuer identity selecting this lineage.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Strictly increasing immutable revision epoch.
    pub record_epoch: u64,
    /// Exact canonical compressed P-256 issuer verification key.
    pub issuer_public_key: PrivacyP256PointV1,
    /// Exact credential document family admitted by this issuer.
    pub document_type: PrivacyCredentialDocumentTypeV1,
    /// Exact mDL namespace admitted by this issuer.
    pub namespace: PrivacyVegaMdlNamespaceV1,
    /// Exact digest algorithm admitted by this issuer.
    pub digest_algorithm: PrivacyVegaMdlDigestAlgorithmV1,
    /// Exact issuer-authentication algorithm admitted by this issuer.
    pub issuer_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
    /// Exact device-authentication algorithm admitted by this issuer.
    pub device_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
    /// Exact predecessor revision digest, absent only at epoch one.
    pub previous_record_digest: Option<PrivacyVegaIssuerRecordDigestV1>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyVegaIssuerRecordLifecycleV1,
    /// Self-digest of every authoritative field above.
    pub record_digest: PrivacyVegaIssuerRecordDigestV1,
}

impl PrivacyVegaIssuerRecordV1 {
    /// Construct one canonical self-digested Vega issuer revision.
    ///
    /// # Errors
    ///
    /// Rejects zero identities or epochs, a malformed compressed-key shape,
    /// or a non-canonical predecessor shape.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issuer_id: PrivacyIssuerIdV1,
        record_epoch: u64,
        issuer_public_key: PrivacyP256PointV1,
        document_type: PrivacyCredentialDocumentTypeV1,
        namespace: PrivacyVegaMdlNamespaceV1,
        digest_algorithm: PrivacyVegaMdlDigestAlgorithmV1,
        issuer_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
        device_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
        previous_record_digest: Option<PrivacyVegaIssuerRecordDigestV1>,
        lifecycle: PrivacyVegaIssuerRecordLifecycleV1,
    ) -> Result<Self, PrivacyVegaIssuerRecordValidationErrorV1> {
        let mut record = Self {
            issuer_id,
            record_epoch,
            issuer_public_key,
            document_type,
            namespace,
            digest_algorithm,
            issuer_authentication_algorithm,
            device_authentication_algorithm,
            previous_record_digest,
            lifecycle,
            record_digest: PrivacyVegaIssuerRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest();
        if record.record_digest.is_zero() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate a canonical active epoch-one registration.
    ///
    /// # Errors
    ///
    /// Rejects a malformed self-digest, non-origin epoch, predecessor, or
    /// terminal origin.
    pub fn validate_initial(&self) -> Result<(), PrivacyVegaIssuerRecordValidationErrorV1> {
        self.validate()?;
        if self.record_epoch != VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1 {
            return Err(
                PrivacyVegaIssuerRecordValidationErrorV1::NonCanonicalInitialEpoch {
                    actual: self.record_epoch,
                },
            );
        }
        if self.previous_record_digest.is_some() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::OriginHasPredecessor);
        }
        if self.lifecycle != PrivacyVegaIssuerRecordLifecycleV1::Active {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::InitialRecordNotActive);
        }
        Ok(())
    }

    /// Validate all fields and the complete canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed or tampered revision.
    pub fn validate(&self) -> Result<(), PrivacyVegaIssuerRecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest() != self.record_digest {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the domain-separated canonical self-digest.
    #[must_use]
    pub fn compute_record_digest(&self) -> PrivacyVegaIssuerRecordDigestV1 {
        let version = VEGA_ISSUER_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
        let record_epoch = self.record_epoch.to_be_bytes();
        let document_type = [match self.document_type {
            PrivacyCredentialDocumentTypeV1::Iso18013_5Mdl => 0,
        }];
        let namespace = [match self.namespace {
            PrivacyVegaMdlNamespaceV1::OrgIso18013_5_1 => 0,
        }];
        let digest_algorithm = [match self.digest_algorithm {
            PrivacyVegaMdlDigestAlgorithmV1::Sha256 => 0,
        }];
        let issuer_authentication_algorithm = [match self.issuer_authentication_algorithm {
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256 => 0,
        }];
        let device_authentication_algorithm = [match self.device_authentication_algorithm {
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256 => 0,
        }];
        let predecessor = privacy_vega_issuer_predecessor_frame_v1(self.previous_record_digest);
        let lifecycle = [match self.lifecycle {
            PrivacyVegaIssuerRecordLifecycleV1::Active => 0,
            PrivacyVegaIssuerRecordLifecycleV1::Revoked => 1,
        }];
        PrivacyVegaIssuerRecordDigestV1::new(privacy_vega_issuer_sha256_frame_v1(&[
            &version,
            self.issuer_id.as_bytes(),
            &record_epoch,
            self.issuer_public_key.as_bytes(),
            &document_type,
            &namespace,
            &digest_algorithm,
            &issuer_authentication_algorithm,
            &device_authentication_algorithm,
            &predecessor,
            &lifecycle,
        ]))
    }

    fn validate_contents(&self) -> Result<(), PrivacyVegaIssuerRecordValidationErrorV1> {
        if self.issuer_id.is_zero() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroIssuerId);
        }
        if self.record_epoch == 0 {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroRecordEpoch);
        }
        if self.issuer_public_key.is_zero() {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroIssuerPublicKey);
        }
        if !matches!(self.issuer_public_key.as_bytes()[0], 0x02 | 0x03) {
            return Err(PrivacyVegaIssuerRecordValidationErrorV1::InvalidIssuerPublicKeyEncoding);
        }
        match (self.record_epoch, self.previous_record_digest) {
            (VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1, None) => {}
            (VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1, Some(_)) => {
                return Err(PrivacyVegaIssuerRecordValidationErrorV1::OriginHasPredecessor);
            }
            (_, None) => {
                return Err(PrivacyVegaIssuerRecordValidationErrorV1::SuccessorMissingPredecessor);
            }
            (_, Some(digest)) if digest.is_zero() => {
                return Err(PrivacyVegaIssuerRecordValidationErrorV1::ZeroPreviousRecordDigest);
            }
            (_, Some(_)) => {}
        }
        Ok(())
    }
}

fn privacy_vega_issuer_sha256_frame_v1(fields: &[&[u8]]) -> [u8; 32] {
    let domain_len = u16::try_from(VEGA_ISSUER_RECORD_DIGEST_DOMAIN_V1.len())
        .expect("fixed Vega issuer-record digest domain fits u16");
    let field_count =
        u16::try_from(fields.len()).expect("fixed Vega issuer-record field count fits u16");
    let mut hash = Sha256::new();
    hash.update(VEGA_ISSUER_RECORD_HASH_FRAME_DOMAIN_V1);
    hash.update(domain_len.to_be_bytes());
    hash.update(VEGA_ISSUER_RECORD_DIGEST_DOMAIN_V1);
    hash.update(field_count.to_be_bytes());
    for field in fields {
        let field_len =
            u64::try_from(field.len()).expect("slice length fits u64 on supported targets");
        hash.update(field_len.to_be_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

fn privacy_vega_issuer_predecessor_frame_v1(
    previous: Option<PrivacyVegaIssuerRecordDigestV1>,
) -> [u8; 33] {
    let mut frame = [0_u8; 33];
    if let Some(digest) = previous {
        frame[0] = 1;
        frame[1..].copy_from_slice(digest.as_bytes());
    }
    frame
}

/// Failure while validating one immutable Vega issuer governance revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyVegaIssuerRecordValidationErrorV1 {
    /// The stable issuer identifier is all zero.
    #[error("Vega issuer id must be non-zero")]
    ZeroIssuerId,
    /// Epoch zero is never authoritative.
    #[error("Vega issuer-record epoch must be non-zero")]
    ZeroRecordEpoch,
    /// Registration must begin at canonical epoch one.
    #[error(
        "initial Vega issuer-record epoch must be {VEGA_INITIAL_ISSUER_RECORD_EPOCH_V1}, got {actual}"
    )]
    NonCanonicalInitialEpoch {
        /// Rejected epoch.
        actual: u64,
    },
    /// An origin revision cannot claim a predecessor.
    #[error("Vega epoch-one issuer record must not carry a predecessor")]
    OriginHasPredecessor,
    /// Every non-origin revision must bind its exact predecessor.
    #[error("Vega successor issuer record must carry a predecessor digest")]
    SuccessorMissingPredecessor,
    /// A predecessor digest cannot be the all-zero sentinel.
    #[error("Vega predecessor issuer-record digest must be non-zero")]
    ZeroPreviousRecordDigest,
    /// The issuer public key cannot be all zero.
    #[error("Vega issuer public key must be non-zero")]
    ZeroIssuerPublicKey,
    /// The wire key must at least have the canonical compressed SEC1 shape.
    #[error("Vega issuer public key must use compressed SEC1 encoding")]
    InvalidIssuerPublicKeyEncoding,
    /// Registration cannot create a terminal lineage.
    #[error("initial Vega issuer record must be active")]
    InitialRecordNotActive,
    /// A decoded record supplied an all-zero self-digest.
    #[error("Vega issuer-record self-digest must be non-zero")]
    ZeroRecordDigest,
    /// Recomputing every authoritative field produced a different digest.
    #[error("Vega issuer-record self-digest mismatch")]
    RecordDigestMismatch,
}

/// Failure while validating an append-only Vega issuer transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyVegaIssuerTransitionValidationErrorV1 {
    /// The persisted current revision is malformed.
    #[error("current Vega issuer record is invalid: {0}")]
    InvalidCurrent(PrivacyVegaIssuerRecordValidationErrorV1),
    /// The proposed successor revision is malformed.
    #[error("successor Vega issuer record is invalid: {0}")]
    InvalidSuccessor(PrivacyVegaIssuerRecordValidationErrorV1),
    /// A terminal lineage cannot advance.
    #[error("current Vega issuer record is not active")]
    CurrentNotActive,
    /// A transition changed its stable issuer identity.
    #[error("Vega issuer transition changed issuer id")]
    IssuerIdMismatch,
    /// An epoch cannot advance past `u64::MAX`.
    #[error("Vega issuer-record epoch overflow")]
    EpochOverflow,
    /// The successor did not advance exactly one epoch.
    #[error("Vega successor epoch must be {expected}, got {actual}")]
    NonCanonicalSuccessorEpoch {
        /// Required successor epoch.
        expected: u64,
        /// Rejected successor epoch.
        actual: u64,
    },
    /// The successor did not bind the exact current revision.
    #[error("Vega successor predecessor digest does not match the current revision")]
    PredecessorDigestMismatch,
    /// A rotation successor must remain active.
    #[error("Vega issuer rotation successor must be active")]
    RotationSuccessorNotActive,
    /// A rotation must alter the issuer key or admitted algorithm policy.
    #[error("Vega issuer rotation must change governed contents")]
    RotationContentsUnchanged,
    /// A revocation successor must be terminal.
    #[error("Vega issuer revocation successor must be revoked")]
    RevocationSuccessorNotRevoked,
    /// Revocation cannot silently change the trusted key or policy.
    #[error("Vega issuer revocation changed immutable governed contents")]
    RevocationContentsChanged,
}

fn validate_vega_issuer_transition_common_v1(
    current: &PrivacyVegaIssuerRecordV1,
    successor: &PrivacyVegaIssuerRecordV1,
) -> Result<(), PrivacyVegaIssuerTransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyVegaIssuerTransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyVegaIssuerTransitionValidationErrorV1::InvalidSuccessor)?;
    if current.lifecycle != PrivacyVegaIssuerRecordLifecycleV1::Active {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::CurrentNotActive);
    }
    if successor.issuer_id != current.issuer_id {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::IssuerIdMismatch);
    }
    let expected = current
        .record_epoch
        .checked_add(1)
        .ok_or(PrivacyVegaIssuerTransitionValidationErrorV1::EpochOverflow)?;
    if successor.record_epoch != expected {
        return Err(
            PrivacyVegaIssuerTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                expected,
                actual: successor.record_epoch,
            },
        );
    }
    if successor.previous_record_digest != Some(current.record_digest) {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::PredecessorDigestMismatch);
    }
    Ok(())
}

/// Validate an active-to-active Vega issuer key or policy rotation.
///
/// # Errors
///
/// Rejects malformed records, identity changes, stale/skipped epochs,
/// predecessor substitution, terminal successors, and no-op rotations.
pub fn validate_vega_issuer_rotation_v1(
    current: &PrivacyVegaIssuerRecordV1,
    successor: &PrivacyVegaIssuerRecordV1,
) -> Result<(), PrivacyVegaIssuerTransitionValidationErrorV1> {
    validate_vega_issuer_transition_common_v1(current, successor)?;
    if successor.lifecycle != PrivacyVegaIssuerRecordLifecycleV1::Active {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    if successor.issuer_public_key == current.issuer_public_key
        && successor.document_type == current.document_type
        && successor.namespace == current.namespace
        && successor.digest_algorithm == current.digest_algorithm
        && successor.issuer_authentication_algorithm == current.issuer_authentication_algorithm
        && successor.device_authentication_algorithm == current.device_authentication_algorithm
    {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::RotationContentsUnchanged);
    }
    Ok(())
}

/// Validate an irreversible Vega issuer revocation.
///
/// # Errors
///
/// Rejects malformed records, identity changes, stale/skipped epochs,
/// predecessor substitution, nonterminal successors, or key/policy changes.
pub fn validate_vega_issuer_revocation_v1(
    current: &PrivacyVegaIssuerRecordV1,
    successor: &PrivacyVegaIssuerRecordV1,
) -> Result<(), PrivacyVegaIssuerTransitionValidationErrorV1> {
    validate_vega_issuer_transition_common_v1(current, successor)?;
    if successor.lifecycle != PrivacyVegaIssuerRecordLifecycleV1::Revoked {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.issuer_public_key != current.issuer_public_key
        || successor.document_type != current.document_type
        || successor.namespace != current.namespace
        || successor.digest_algorithm != current.digest_algorithm
        || successor.issuer_authentication_algorithm != current.issuer_authentication_algorithm
        || successor.device_authentication_algorithm != current.device_authentication_algorithm
    {
        return Err(PrivacyVegaIssuerTransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// Gregorian UTC calendar date used as Vega Figure 9 public input `(Y, M, D)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyVegaMdlDateV1 {
    /// Four-digit UTC year.
    pub year: u16,
    /// One-based UTC month.
    pub month: u8,
    /// One-based UTC day of month.
    pub day: u8,
}

/// Vega Figure 9 ISO/IEC 18013-5 mDL-age public statement.
///
/// The native circuit exposes only the paper's public inputs `Q_I`, `H_dev`,
/// `(Y, M, D)`, and `tau`. The exact document bytes, decoded MSO payload,
/// issuer and device signatures, device public key, validity interval,
/// birth-date `IssuerSignedItemBytes`, and every lookup hint are private
/// engine witness values.
///
/// `issuer_id`, `issuer_record_epoch`, and `issuer_record_digest` select the
/// exact active governance revision whose key and algorithm policy must match
/// this statement before native verification. The proof has no ledger effect.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct VegaExistingCredentialStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Stable governed issuer identity selecting one authoritative lineage.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Exact current authoritative issuer revision epoch.
    pub issuer_record_epoch: u64,
    /// Exact self-digest of the authoritative issuer revision.
    pub issuer_record_digest: PrivacyVegaIssuerRecordDigestV1,
    /// Exact supported credential document family and `docType`.
    pub document_type: PrivacyCredentialDocumentTypeV1,
    /// Exact namespace containing the `birth_date` signed item.
    pub namespace: PrivacyVegaMdlNamespaceV1,
    /// Exact digest algorithm constrained by the circuit.
    pub digest_algorithm: PrivacyVegaMdlDigestAlgorithmV1,
    /// Exact issuer COSE authentication algorithm constrained by the circuit.
    pub issuer_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
    /// Exact device COSE authentication algorithm constrained by the circuit.
    pub device_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1,
    /// Public P-256 issuer key `Q_I`, copied exactly from authoritative state.
    pub issuer_public_key: PrivacyP256PointV1,
    /// Public device-authentication digest `H_dev`.
    ///
    /// The native engine recomputes this value from the canonical consensus
    /// frame containing chain, genesis, action, all governed artifact
    /// bindings, `Q_I`, date, threshold, challenge, and session digest before
    /// performing any proof verification.
    pub device_authentication_digest: PrivacyVegaDeviceAuthenticationDigestV1,
    /// Public trusted UTC presentation date `(Y, M, D)`.
    ///
    /// Admission additionally requires exact equality with the UTC date
    /// derived from the canonical block timestamp.
    pub presentation_date: PrivacyVegaMdlDateV1,
    /// Public minimum age threshold `tau`, in completed Gregorian years.
    pub minimum_age_years: u8,
    /// Fresh reader challenge incorporated into the `H_dev` consensus frame.
    pub reader_challenge: PrivacyChallengeV1,
    /// Digest of the canonical ISO 18013-5 session transcript incorporated
    /// into the `H_dev` consensus frame.
    pub session_transcript_digest: PrivacySessionTranscriptDigestV1,
}

/// X.509 key-usage bits admitted by the first-release certificate profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyX509KeyUsageV1 {
    /// RFC 5280 digital-signature bit.
    pub digital_signature: bool,
    /// RFC 5280 content-commitment bit.
    pub content_commitment: bool,
    /// RFC 5280 key-encipherment bit.
    pub key_encipherment: bool,
    /// RFC 5280 key-agreement bit.
    pub key_agreement: bool,
}

/// Exact extended-key-usage purpose required from an admitted certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "purpose", content = "value", deny_unknown_fields)
)]
pub enum PrivacyX509ExtendedKeyUsageV1 {
    /// TLS-style client authentication.
    ClientAuthentication,
    /// Digital document signing.
    DocumentSigning,
    /// Wallet or digital-identity authentication.
    WalletIdentity,
}

/// Closed lifecycle of one immutable X.509 governance-record lineage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "value", deny_unknown_fields)
)]
pub enum PrivacyZkX509RecordLifecycleV1 {
    /// The trust-anchor or certificate-policy revision is authoritative.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active,
    /// The lineage was irreversibly revoked.
    #[cfg_attr(feature = "json", norito(rename = "revoked"))]
    Revoked,
}

/// One immutable authoritative revision of an RFC 5280 P-256/SHA-256 trust store.
///
/// Revisions form an append-only self-digested chain. `trust_store_digest`
/// commits the complete canonically ordered trust-anchor artifact; individual
/// CA identity remains private behind the governed CA-membership root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkX509TrustAnchorRecordV1 {
    /// Stable lookup key for this trust-store lineage.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Strictly increasing immutable revision epoch.
    pub record_epoch: u64,
    /// Digest of the exact ordered P-256/SHA-256 trust-store artifact.
    pub trust_store_digest: PrivacyX509TrustStoreDigestV1,
    /// Exact CA-membership root derived from that complete trust store.
    pub ca_membership_root: PrivacyRootV1,
    /// Canonical epoch of `ca_membership_root`.
    ///
    /// An active record requires this to equal `record_epoch`. A terminal
    /// revocation preserves the preceding active root epoch and does not
    /// manufacture a new CA-membership root.
    pub ca_membership_root_epoch: u64,
    /// Exact predecessor revision digest, absent only at epoch one.
    pub previous_record_digest: Option<PrivacyZkX509TrustAnchorRecordDigestV1>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyZkX509RecordLifecycleV1,
    /// Self-digest of every authoritative field above.
    pub record_digest: PrivacyZkX509TrustAnchorRecordDigestV1,
}

impl PrivacyZkX509TrustAnchorRecordV1 {
    /// Construct one canonical self-digested trust-anchor revision.
    ///
    /// # Errors
    ///
    /// Rejects zero fields, a root epoch inconsistent with the lifecycle, a
    /// non-canonical predecessor shape, or an invalid lifecycle/root binding.
    pub fn new(
        trust_anchor_id: PrivacyIssuerIdV1,
        record_epoch: u64,
        trust_store_digest: PrivacyX509TrustStoreDigestV1,
        ca_membership_root: PrivacyRootV1,
        ca_membership_root_epoch: u64,
        previous_record_digest: Option<PrivacyZkX509TrustAnchorRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> Result<Self, PrivacyZkX509RecordValidationErrorV1> {
        let mut record = Self {
            trust_anchor_id,
            record_epoch,
            trust_store_digest,
            ca_membership_root,
            ca_membership_root_epoch,
            previous_record_digest,
            lifecycle,
            record_digest: PrivacyZkX509TrustAnchorRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest()?;
        if record.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate a canonical active epoch-one registration.
    ///
    /// # Errors
    ///
    /// Rejects a malformed self-digest, non-origin epoch, predecessor, or
    /// revoked origin.
    pub fn validate_initial(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate()?;
        validate_zk_x509_initial_revision(
            self.record_epoch,
            self.previous_record_digest.is_some(),
            self.lifecycle,
        )
    }

    /// Validate all fields and the complete canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed or tampered revision.
    pub fn validate(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest()? != self.record_digest {
            return Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the domain-separated self-digest.
    ///
    /// # Errors
    ///
    /// Uses the explicit SHA-256 field frame shared with the proof system.
    pub fn compute_record_digest(
        &self,
    ) -> Result<PrivacyZkX509TrustAnchorRecordDigestV1, PrivacyZkX509RecordValidationErrorV1> {
        let version = ZK_X509_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
        let record_epoch = self.record_epoch.to_be_bytes();
        let ca_membership_root_epoch = self.ca_membership_root_epoch.to_be_bytes();
        let predecessor = privacy_zk_x509_predecessor_frame_v1(self.previous_record_digest);
        let lifecycle = privacy_zk_x509_lifecycle_frame_v1(self.lifecycle);
        Ok(PrivacyZkX509TrustAnchorRecordDigestV1::new(
            privacy_zk_x509_sha256_frame_v1(
                ZK_X509_TRUST_ANCHOR_RECORD_DIGEST_DOMAIN_V1,
                &[
                    &version,
                    self.trust_anchor_id.as_bytes(),
                    &record_epoch,
                    self.trust_store_digest.as_bytes(),
                    self.ca_membership_root.as_bytes(),
                    &ca_membership_root_epoch,
                    &predecessor,
                    &lifecycle,
                ],
            ),
        ))
    }

    fn validate_contents(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        if self.trust_anchor_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroTrustAnchorId);
        }
        if self.trust_store_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroTrustStoreDigest);
        }
        if self.ca_membership_root.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroCaMembershipRoot);
        }
        if self.ca_membership_root_epoch == 0 {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroCaMembershipRootEpoch);
        }
        match self.lifecycle {
            PrivacyZkX509RecordLifecycleV1::Active
                if self.ca_membership_root_epoch != self.record_epoch =>
            {
                return Err(
                    PrivacyZkX509RecordValidationErrorV1::CaMembershipRootEpochMismatch {
                        record_epoch: self.record_epoch,
                        root_epoch: self.ca_membership_root_epoch,
                    },
                );
            }
            PrivacyZkX509RecordLifecycleV1::Revoked
                if self.ca_membership_root_epoch >= self.record_epoch =>
            {
                return Err(
                    PrivacyZkX509RecordValidationErrorV1::RevokedCaMembershipRootEpochNotHistorical {
                        record_epoch: self.record_epoch,
                        root_epoch: self.ca_membership_root_epoch,
                    },
                );
            }
            PrivacyZkX509RecordLifecycleV1::Active | PrivacyZkX509RecordLifecycleV1::Revoked => {}
        }
        validate_zk_x509_revision_shape(self.record_epoch, self.previous_record_digest)
    }
}

/// One immutable authoritative X.509 certificate-policy revision.
///
/// The policy fixes every public predicate selected outside the certificate
/// witness. In particular, a statement must disclose exactly the governed
/// ordered index set rather than a prover-chosen subset or superset.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkX509CertificatePolicyRecordV1 {
    /// Exact trust-store lineage to which this policy belongs.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Stable policy lookup key inside the trust-store namespace.
    pub policy_id: PrivacyPolicyIdV1,
    /// Strictly increasing immutable revision epoch.
    pub record_epoch: u64,
    /// Digest of the exact governed RFC 5280 certificate-policy artifact.
    pub policy_digest: PrivacyPolicyDigestV1,
    /// Required RFC 5280 leaf key-usage bits.
    pub required_key_usage: PrivacyX509KeyUsageV1,
    /// Required extended-key usages in strict enum order.
    pub required_extended_key_usages: Vec<PrivacyX509ExtendedKeyUsageV1>,
    /// Exact required selective-disclosure indices in strict numeric order.
    pub required_disclosed_attribute_indices: Vec<u8>,
    /// Exact predecessor revision digest, absent only at epoch one.
    pub previous_record_digest: Option<PrivacyZkX509CertificatePolicyRecordDigestV1>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyZkX509RecordLifecycleV1,
    /// Self-digest of every authoritative field above.
    pub record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1,
}

impl PrivacyZkX509CertificatePolicyRecordV1 {
    /// Construct one canonical self-digested certificate-policy revision.
    ///
    /// # Errors
    ///
    /// Rejects zero fields, unsupported key usage, oversized or unordered
    /// policy lists or a non-canonical predecessor shape.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trust_anchor_id: PrivacyIssuerIdV1,
        policy_id: PrivacyPolicyIdV1,
        record_epoch: u64,
        policy_digest: PrivacyPolicyDigestV1,
        required_key_usage: PrivacyX509KeyUsageV1,
        required_extended_key_usages: Vec<PrivacyX509ExtendedKeyUsageV1>,
        required_disclosed_attribute_indices: Vec<u8>,
        previous_record_digest: Option<PrivacyZkX509CertificatePolicyRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> Result<Self, PrivacyZkX509RecordValidationErrorV1> {
        let mut record = Self {
            trust_anchor_id,
            policy_id,
            record_epoch,
            policy_digest,
            required_key_usage,
            required_extended_key_usages,
            required_disclosed_attribute_indices,
            previous_record_digest,
            lifecycle,
            record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest()?;
        if record.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate a canonical active epoch-one registration.
    ///
    /// # Errors
    ///
    /// Rejects a malformed self-digest, non-origin epoch, predecessor, or
    /// revoked origin.
    pub fn validate_initial(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate()?;
        validate_zk_x509_initial_revision(
            self.record_epoch,
            self.previous_record_digest.is_some(),
            self.lifecycle,
        )
    }

    /// Validate all fields and the complete canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed or tampered revision.
    pub fn validate(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest()? != self.record_digest {
            return Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the domain-separated self-digest.
    ///
    /// # Errors
    ///
    /// Uses the explicit SHA-256 field frame shared with the proof system.
    pub fn compute_record_digest(
        &self,
    ) -> Result<PrivacyZkX509CertificatePolicyRecordDigestV1, PrivacyZkX509RecordValidationErrorV1>
    {
        validate_zk_x509_key_usage(self.required_key_usage)?;
        validate_zk_x509_extended_key_usages(&self.required_extended_key_usages)?;
        validate_zk_x509_disclosure_indices(&self.required_disclosed_attribute_indices)?;
        let version = ZK_X509_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
        let record_epoch = self.record_epoch.to_be_bytes();
        let key_usage = [privacy_zk_x509_key_usage_mask_v1(self.required_key_usage)];
        let mut extended_key_usages =
            Vec::with_capacity(1 + self.required_extended_key_usages.len());
        extended_key_usages.push(
            u8::try_from(self.required_extended_key_usages.len())
                .expect("validated X.509 EKU count fits u8"),
        );
        extended_key_usages.extend(
            self.required_extended_key_usages
                .iter()
                .copied()
                .map(privacy_zk_x509_extended_key_usage_code_v1),
        );
        let mut disclosed_attributes =
            Vec::with_capacity(1 + self.required_disclosed_attribute_indices.len());
        disclosed_attributes.push(
            u8::try_from(self.required_disclosed_attribute_indices.len())
                .expect("validated X.509 disclosure count fits u8"),
        );
        disclosed_attributes.extend_from_slice(&self.required_disclosed_attribute_indices);
        let predecessor = privacy_zk_x509_predecessor_frame_v1(self.previous_record_digest);
        let lifecycle = privacy_zk_x509_lifecycle_frame_v1(self.lifecycle);
        Ok(PrivacyZkX509CertificatePolicyRecordDigestV1::new(
            privacy_zk_x509_sha256_frame_v1(
                ZK_X509_CERTIFICATE_POLICY_RECORD_DIGEST_DOMAIN_V1,
                &[
                    &version,
                    self.trust_anchor_id.as_bytes(),
                    self.policy_id.as_bytes(),
                    &record_epoch,
                    self.policy_digest.as_bytes(),
                    &key_usage,
                    &extended_key_usages,
                    &disclosed_attributes,
                    &predecessor,
                    &lifecycle,
                ],
            ),
        ))
    }

    fn validate_contents(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        if self.trust_anchor_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroTrustAnchorId);
        }
        if self.policy_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroPolicyId);
        }
        if self.policy_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroPolicyDigest);
        }
        validate_zk_x509_key_usage(self.required_key_usage)?;
        validate_zk_x509_extended_key_usages(&self.required_extended_key_usages)?;
        validate_zk_x509_disclosure_indices(&self.required_disclosed_attribute_indices)?;
        validate_zk_x509_revision_shape(self.record_epoch, self.previous_record_digest)
    }
}

/// One immutable authoritative revision of an issuer-scoped signed CRL.
///
/// The exact signed DER digest, signing-key digest, and validity window are one
/// self-digested governance object. The proof parses that complete, signed CRL
/// and checks the leaf serial against every active entry; there is deliberately
/// no second revocation accumulator whose contents could diverge from the CRL.
/// The first release assigns exactly one leaf certificate issuer and its
/// complete, non-partitioned CRL to each certificate-policy lineage;
/// revocation checks the leaf certificate only.
/// Multiple intermediates require distinct policy lineages. Consensus keeps
/// only the current self-chained record while historical transitions remain
/// committed by blocks. Revocation is terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkX509CrlRecordV1 {
    /// Exact trust-store lineage whose certificate chain selects this CRL.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Exact certificate-policy lineage whose leaf revocation is represented.
    pub certificate_policy_id: PrivacyPolicyIdV1,
    /// Strictly increasing immutable revision epoch.
    pub record_epoch: u64,
    /// Required RFC 5280 CRLNumber, monotonically increasing in this lineage.
    pub crl_number: u64,
    /// SHA-256 digest of the complete exact signed DER CRL.
    pub crl_der_digest: PrivacyX509CrlDerDigestV1,
    /// SHA-256 digest of the exact issuer SPKI that signs the CRL.
    pub issuer_spki_digest: PrivacyX509CrlIssuerSpkiDigestV1,
    /// Signed CRL `thisUpdate` as Unix seconds, inclusive.
    pub this_update_unix_seconds: u64,
    /// Signed CRL `nextUpdate` as Unix seconds, exclusive.
    pub next_update_unix_seconds: u64,
    /// Exact predecessor revision digest, absent only at epoch one.
    pub previous_record_digest: Option<PrivacyZkX509CrlRecordDigestV1>,
    /// Active or irreversibly revoked lifecycle.
    pub lifecycle: PrivacyZkX509RecordLifecycleV1,
    /// Self-digest of every authoritative field above.
    pub record_digest: PrivacyZkX509CrlRecordDigestV1,
}

impl PrivacyZkX509CrlRecordV1 {
    /// Construct one canonical self-digested signed-CRL revision.
    ///
    /// # Errors
    ///
    /// Rejects zero identities or digests, an invalid signed validity window,
    /// or a non-canonical predecessor shape.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        trust_anchor_id: PrivacyIssuerIdV1,
        certificate_policy_id: PrivacyPolicyIdV1,
        record_epoch: u64,
        crl_number: u64,
        crl_der_digest: PrivacyX509CrlDerDigestV1,
        issuer_spki_digest: PrivacyX509CrlIssuerSpkiDigestV1,
        this_update_unix_seconds: u64,
        next_update_unix_seconds: u64,
        previous_record_digest: Option<PrivacyZkX509CrlRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> Result<Self, PrivacyZkX509RecordValidationErrorV1> {
        let mut record = Self {
            trust_anchor_id,
            certificate_policy_id,
            record_epoch,
            crl_number,
            crl_der_digest,
            issuer_spki_digest,
            this_update_unix_seconds,
            next_update_unix_seconds,
            previous_record_digest,
            lifecycle,
            record_digest: PrivacyZkX509CrlRecordDigestV1::new([0; 32]),
        };
        record.validate_contents()?;
        record.record_digest = record.compute_record_digest()?;
        if record.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        Ok(record)
    }

    /// Validate a canonical active epoch-one registration.
    ///
    /// # Errors
    ///
    /// Rejects a malformed self-digest, non-origin epoch, predecessor, or
    /// revoked origin.
    pub fn validate_initial(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate()?;
        validate_zk_x509_initial_revision(
            self.record_epoch,
            self.previous_record_digest.is_some(),
            self.lifecycle,
        )
    }

    /// Validate all fields and the complete canonical self-digest.
    ///
    /// # Errors
    ///
    /// Rejects any malformed or tampered revision.
    pub fn validate(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        self.validate_contents()?;
        if self.record_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest);
        }
        if self.compute_record_digest()? != self.record_digest {
            return Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Recompute the domain-separated self-digest.
    ///
    /// # Errors
    ///
    /// Uses the explicit SHA-256 field frame shared with the proof system.
    pub fn compute_record_digest(
        &self,
    ) -> Result<PrivacyZkX509CrlRecordDigestV1, PrivacyZkX509RecordValidationErrorV1> {
        let version = ZK_X509_GOVERNANCE_RECORD_VERSION_V1.to_be_bytes();
        let record_epoch = self.record_epoch.to_be_bytes();
        let crl_number = self.crl_number.to_be_bytes();
        let this_update = self.this_update_unix_seconds.to_be_bytes();
        let next_update = self.next_update_unix_seconds.to_be_bytes();
        let predecessor = privacy_zk_x509_predecessor_frame_v1(self.previous_record_digest);
        let lifecycle = privacy_zk_x509_lifecycle_frame_v1(self.lifecycle);
        Ok(PrivacyZkX509CrlRecordDigestV1::new(
            privacy_zk_x509_sha256_frame_v1(
                ZK_X509_CRL_RECORD_DIGEST_DOMAIN_V1,
                &[
                    &version,
                    self.trust_anchor_id.as_bytes(),
                    self.certificate_policy_id.as_bytes(),
                    &record_epoch,
                    &crl_number,
                    self.crl_der_digest.as_bytes(),
                    self.issuer_spki_digest.as_bytes(),
                    &this_update,
                    &next_update,
                    &predecessor,
                    &lifecycle,
                ],
            ),
        ))
    }

    fn validate_contents(&self) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
        if self.trust_anchor_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroTrustAnchorId);
        }
        if self.certificate_policy_id.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroPolicyId);
        }
        if self.crl_der_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroCrlDerDigest);
        }
        if self.issuer_spki_digest.is_zero() {
            return Err(PrivacyZkX509RecordValidationErrorV1::ZeroCrlIssuerSpkiDigest);
        }
        if self.next_update_unix_seconds <= self.this_update_unix_seconds {
            return Err(PrivacyZkX509RecordValidationErrorV1::InvalidCrlValidityWindow);
        }
        validate_zk_x509_revision_shape(self.record_epoch, self.previous_record_digest)
    }
}

fn privacy_zk_x509_sha256_frame_v1(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let domain_len = u16::try_from(domain.len()).expect("fixed X.509 digest domain fits u16");
    let field_count = u16::try_from(fields.len()).expect("fixed X.509 digest field count fits u16");
    let mut hash = Sha256::new();
    hash.update(ZK_X509_HASH_FRAME_DOMAIN_V1);
    hash.update(domain_len.to_be_bytes());
    hash.update(domain);
    hash.update(field_count.to_be_bytes());
    for field in fields {
        let field_len =
            u64::try_from(field.len()).expect("slice length fits u64 on supported targets");
        hash.update(field_len.to_be_bytes());
        hash.update(field);
    }
    hash.finalize().into()
}

fn privacy_zk_x509_predecessor_frame_v1<D: PrivacyDigestValueV1>(previous: Option<D>) -> [u8; 33] {
    let mut frame = [0_u8; 33];
    if let Some(digest) = previous {
        frame[0] = 1;
        frame[1..].copy_from_slice(&digest.bytes());
    }
    frame
}

const fn privacy_zk_x509_lifecycle_frame_v1(lifecycle: PrivacyZkX509RecordLifecycleV1) -> [u8; 1] {
    [match lifecycle {
        PrivacyZkX509RecordLifecycleV1::Active => 0,
        PrivacyZkX509RecordLifecycleV1::Revoked => 1,
    }]
}

fn privacy_zk_x509_key_usage_mask_v1(key_usage: PrivacyX509KeyUsageV1) -> u8 {
    u8::from(key_usage.digital_signature)
        | (u8::from(key_usage.content_commitment) << 1)
        | (u8::from(key_usage.key_encipherment) << 2)
        | (u8::from(key_usage.key_agreement) << 3)
}

const fn privacy_zk_x509_extended_key_usage_code_v1(usage: PrivacyX509ExtendedKeyUsageV1) -> u8 {
    match usage {
        PrivacyX509ExtendedKeyUsageV1::ClientAuthentication => 0,
        PrivacyX509ExtendedKeyUsageV1::DocumentSigning => 1,
        PrivacyX509ExtendedKeyUsageV1::WalletIdentity => 2,
    }
}

fn validate_zk_x509_revision_shape<D: PrivacyDigestValueV1>(
    record_epoch: u64,
    previous_record_digest: Option<D>,
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if record_epoch == 0 {
        return Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordEpoch);
    }
    match (record_epoch, previous_record_digest) {
        (ZK_X509_INITIAL_RECORD_EPOCH_V1, None) => Ok(()),
        (ZK_X509_INITIAL_RECORD_EPOCH_V1, Some(_)) => {
            Err(PrivacyZkX509RecordValidationErrorV1::OriginHasPredecessor)
        }
        (_, None) => Err(PrivacyZkX509RecordValidationErrorV1::SuccessorMissingPredecessor),
        (_, Some(digest)) if digest.is_zero() => {
            Err(PrivacyZkX509RecordValidationErrorV1::ZeroPreviousRecordDigest)
        }
        (_, Some(_)) => Ok(()),
    }
}

trait PrivacyDigestValueV1: Copy {
    fn is_zero(self) -> bool;
    fn bytes(self) -> [u8; 32];
}

impl PrivacyDigestValueV1 for PrivacyZkX509TrustAnchorRecordDigestV1 {
    fn is_zero(self) -> bool {
        PrivacyZkX509TrustAnchorRecordDigestV1::is_zero(&self)
    }

    fn bytes(self) -> [u8; 32] {
        *self.as_bytes()
    }
}

impl PrivacyDigestValueV1 for PrivacyZkX509CertificatePolicyRecordDigestV1 {
    fn is_zero(self) -> bool {
        PrivacyZkX509CertificatePolicyRecordDigestV1::is_zero(&self)
    }

    fn bytes(self) -> [u8; 32] {
        *self.as_bytes()
    }
}

impl PrivacyDigestValueV1 for PrivacyZkX509CrlRecordDigestV1 {
    fn is_zero(self) -> bool {
        PrivacyZkX509CrlRecordDigestV1::is_zero(&self)
    }

    fn bytes(self) -> [u8; 32] {
        *self.as_bytes()
    }
}

fn validate_zk_x509_initial_revision(
    record_epoch: u64,
    has_previous: bool,
    lifecycle: PrivacyZkX509RecordLifecycleV1,
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if record_epoch != ZK_X509_INITIAL_RECORD_EPOCH_V1 {
        return Err(
            PrivacyZkX509RecordValidationErrorV1::NonCanonicalInitialEpoch {
                actual: record_epoch,
            },
        );
    }
    if has_previous {
        return Err(PrivacyZkX509RecordValidationErrorV1::OriginHasPredecessor);
    }
    if lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509RecordValidationErrorV1::InitialRecordNotActive);
    }
    Ok(())
}

fn validate_zk_x509_key_usage(
    key_usage: PrivacyX509KeyUsageV1,
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if !key_usage.digital_signature {
        return Err(PrivacyZkX509RecordValidationErrorV1::InvalidKeyUsage);
    }
    Ok(())
}

fn validate_zk_x509_extended_key_usages(
    usages: &[PrivacyX509ExtendedKeyUsageV1],
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if usages.is_empty() {
        return Err(PrivacyZkX509RecordValidationErrorV1::MissingExtendedKeyUsage);
    }
    if usages.len() > ZK_X509_MAX_EXTENDED_KEY_USAGES_V1 {
        return Err(
            PrivacyZkX509RecordValidationErrorV1::TooManyExtendedKeyUsages {
                actual: usages.len(),
                max: ZK_X509_MAX_EXTENDED_KEY_USAGES_V1,
            },
        );
    }
    if usages.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(PrivacyZkX509RecordValidationErrorV1::ExtendedKeyUsagesNotStrictlyIncreasing);
    }
    Ok(())
}

fn validate_zk_x509_disclosure_indices(
    indices: &[u8],
) -> Result<(), PrivacyZkX509RecordValidationErrorV1> {
    if indices.len() > ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1 {
        return Err(
            PrivacyZkX509RecordValidationErrorV1::TooManyDisclosedAttributes {
                actual: indices.len(),
                max: ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1,
            },
        );
    }
    for &index in indices {
        if usize::from(index) >= ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1 {
            return Err(
                PrivacyZkX509RecordValidationErrorV1::UnsupportedDisclosedAttributeIndex { index },
            );
        }
    }
    if indices.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            PrivacyZkX509RecordValidationErrorV1::DisclosedAttributeIndicesNotStrictlyIncreasing,
        );
    }
    Ok(())
}

/// Failure while validating one immutable X.509 governance revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkX509RecordValidationErrorV1 {
    /// The trust-store identifier is all zero.
    #[error("X.509 trust-anchor id must be non-zero")]
    ZeroTrustAnchorId,
    /// The certificate-policy identifier is all zero.
    #[error("X.509 certificate-policy id must be non-zero")]
    ZeroPolicyId,
    /// The canonical trust-store digest is all zero.
    #[error("X.509 trust-store digest must be non-zero")]
    ZeroTrustStoreDigest,
    /// The CA-membership root derived from the trust store is all zero.
    #[error("X.509 CA-membership root must be non-zero")]
    ZeroCaMembershipRoot,
    /// A CA-membership root epoch is zero.
    #[error("X.509 CA-membership root epoch must be non-zero")]
    ZeroCaMembershipRootEpoch,
    /// The first-release trust-anchor record and CA-root epochs differ.
    #[error(
        "X.509 CA-membership root epoch {root_epoch} must equal trust-anchor record epoch {record_epoch}"
    )]
    CaMembershipRootEpochMismatch {
        /// Immutable trust-anchor record epoch.
        record_epoch: u64,
        /// Rejected CA-membership root epoch.
        root_epoch: u64,
    },
    /// A terminal trust-anchor record did not preserve a historical active root.
    #[error(
        "revoked X.509 CA-membership root epoch {root_epoch} must precede record epoch {record_epoch}"
    )]
    RevokedCaMembershipRootEpochNotHistorical {
        /// Terminal trust-anchor record epoch.
        record_epoch: u64,
        /// Rejected retained active-root epoch.
        root_epoch: u64,
    },
    /// The canonical certificate-policy digest is all zero.
    #[error("X.509 certificate-policy digest must be non-zero")]
    ZeroPolicyDigest,
    /// The SHA-256 digest of the exact signed DER CRL is all zero.
    #[error("X.509 signed DER CRL digest must be non-zero")]
    ZeroCrlDerDigest,
    /// The SHA-256 digest of the CRL issuer SPKI is all zero.
    #[error("X.509 CRL issuer SPKI digest must be non-zero")]
    ZeroCrlIssuerSpkiDigest,
    /// The signed CRL validity window is zero, empty, or reversed.
    #[error("X.509 CRL validity window must satisfy thisUpdate < nextUpdate")]
    InvalidCrlValidityWindow,
    /// Epoch zero is never authoritative.
    #[error("X.509 governance-record epoch must be non-zero")]
    ZeroRecordEpoch,
    /// Registration must begin at canonical epoch one.
    #[error(
        "initial X.509 governance-record epoch must be {ZK_X509_INITIAL_RECORD_EPOCH_V1}, got {actual}"
    )]
    NonCanonicalInitialEpoch {
        /// Rejected epoch.
        actual: u64,
    },
    /// An origin revision cannot claim a predecessor.
    #[error("X.509 epoch-one governance record must not carry a predecessor")]
    OriginHasPredecessor,
    /// Every non-origin revision must bind its exact predecessor.
    #[error("X.509 successor governance record must carry a predecessor digest")]
    SuccessorMissingPredecessor,
    /// A predecessor digest cannot be the all-zero sentinel.
    #[error("X.509 predecessor record digest must be non-zero")]
    ZeroPreviousRecordDigest,
    /// Registration cannot create a terminal lineage.
    #[error("initial X.509 governance record must be active")]
    InitialRecordNotActive,
    /// The fixed certificate relation requires digital-signature usage.
    #[error("X.509 policy must require digital-signature key usage")]
    InvalidKeyUsage,
    /// At least one closed extended-key usage must be governed.
    #[error("X.509 policy must require at least one extended-key usage")]
    MissingExtendedKeyUsage,
    /// More EKUs were supplied than the closed profile supports.
    #[error("X.509 policy has {actual} extended-key usages; maximum is {max}")]
    TooManyExtendedKeyUsages {
        /// Rejected length.
        actual: usize,
        /// Closed maximum.
        max: usize,
    },
    /// EKUs were duplicated or reordered.
    #[error("X.509 extended-key usages must be strictly increasing")]
    ExtendedKeyUsagesNotStrictlyIncreasing,
    /// More disclosures were supplied than the closed profile supports.
    #[error("X.509 policy has {actual} disclosed attributes; maximum is {max}")]
    TooManyDisclosedAttributes {
        /// Rejected length.
        actual: usize,
        /// Closed maximum.
        max: usize,
    },
    /// An attribute index is outside the closed C/O/OU/CN set.
    #[error("X.509 disclosed attribute index {index} is unsupported")]
    UnsupportedDisclosedAttributeIndex {
        /// Rejected index.
        index: u8,
    },
    /// Disclosure indices were duplicated or reordered.
    #[error("X.509 disclosed attribute indices must be strictly increasing")]
    DisclosedAttributeIndicesNotStrictlyIncreasing,
    /// A decoded record supplied an all-zero self-digest.
    #[error("X.509 governance-record self-digest must be non-zero")]
    ZeroRecordDigest,
    /// Recomputing every authoritative field produced a different digest.
    #[error("X.509 governance-record self-digest mismatch")]
    RecordDigestMismatch,
}

/// Failure while validating an append-only X.509 governance transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyZkX509TransitionValidationErrorV1 {
    /// The persisted current revision is malformed.
    #[error("current X.509 governance record is invalid: {0}")]
    InvalidCurrent(PrivacyZkX509RecordValidationErrorV1),
    /// The proposed successor revision is malformed.
    #[error("successor X.509 governance record is invalid: {0}")]
    InvalidSuccessor(PrivacyZkX509RecordValidationErrorV1),
    /// A terminal lineage cannot advance.
    #[error("current X.509 governance record is not active")]
    CurrentNotActive,
    /// A trust-anchor transition changed its stable identity.
    #[error("X.509 trust-anchor transition changed trust-anchor id")]
    TrustAnchorIdMismatch,
    /// A trust-store digest rotated without its derived CA-membership root.
    #[error("X.509 trust-store digest changed without changing its CA-membership root")]
    TrustStoreDigestChangedWithoutCaMembershipRoot,
    /// A CA-membership root rotated without its source trust-store digest.
    #[error("X.509 CA-membership root changed without changing its trust-store digest")]
    CaMembershipRootChangedWithoutTrustStoreDigest,
    /// A policy transition changed its stable identity.
    #[error("X.509 certificate-policy transition changed policy id")]
    PolicyIdMismatch,
    /// A signed-CRL lineage changed the issuer public key.
    #[error("X.509 CRL transition changed issuer SPKI digest")]
    CrlIssuerSpkiDigestMismatch,
    /// An epoch cannot advance past `u64::MAX`.
    #[error("X.509 governance-record epoch overflow")]
    EpochOverflow,
    /// The successor did not advance exactly one epoch.
    #[error("X.509 successor epoch must be {expected}, got {actual}")]
    NonCanonicalSuccessorEpoch {
        /// Required successor epoch.
        expected: u64,
        /// Rejected successor epoch.
        actual: u64,
    },
    /// The successor did not bind the exact current revision.
    #[error("X.509 successor predecessor digest does not match the current revision")]
    PredecessorDigestMismatch,
    /// A rotation successor must remain active.
    #[error("X.509 rotation successor must be active")]
    RotationSuccessorNotActive,
    /// A rotation must alter at least one governed substantive field.
    #[error("X.509 rotation must change governed contents")]
    RotationContentsUnchanged,
    /// A signed-CRL successor did not advance `thisUpdate`.
    #[error("X.509 CRL successor thisUpdate must strictly increase")]
    CrlThisUpdateNotIncreasing,
    /// A signed-CRL successor did not advance CRLNumber.
    #[error("X.509 CRL successor CRLNumber must strictly increase")]
    CrlNumberNotIncreasing,
    /// A revocation successor must be terminal.
    #[error("X.509 revocation successor must be revoked")]
    RevocationSuccessorNotRevoked,
    /// Revocation changed substantive governed contents.
    #[error("X.509 revocation changed immutable governed contents")]
    RevocationContentsChanged,
}

fn validate_zk_x509_transition_common<D: Copy + PartialEq>(
    current_epoch: u64,
    current_digest: D,
    current_lifecycle: PrivacyZkX509RecordLifecycleV1,
    successor_epoch: u64,
    successor_previous_digest: Option<D>,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    if current_lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CurrentNotActive);
    }
    let expected = current_epoch
        .checked_add(1)
        .ok_or(PrivacyZkX509TransitionValidationErrorV1::EpochOverflow)?;
    if successor_epoch != expected {
        return Err(
            PrivacyZkX509TransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                expected,
                actual: successor_epoch,
            },
        );
    }
    if successor_previous_digest != Some(current_digest) {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PredecessorDigestMismatch);
    }
    Ok(())
}

/// Validate an active-to-active trust-store rotation.
///
/// # Errors
///
/// Rejects malformed records, identity changes, stale/skipped epochs,
/// predecessor substitution, terminal successors, no-op rotations, and a
/// trust-store digest or CA-membership root changed without the other.
pub fn validate_zk_x509_trust_anchor_rotation_v1(
    current: &PrivacyZkX509TrustAnchorRecordV1,
    successor: &PrivacyZkX509TrustAnchorRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    let trust_store_changed = successor.trust_store_digest != current.trust_store_digest;
    let ca_membership_root_changed = successor.ca_membership_root != current.ca_membership_root;
    match (trust_store_changed, ca_membership_root_changed) {
        (false, false) => {
            return Err(PrivacyZkX509TransitionValidationErrorV1::RotationContentsUnchanged);
        }
        (true, false) => {
            return Err(
                PrivacyZkX509TransitionValidationErrorV1::TrustStoreDigestChangedWithoutCaMembershipRoot,
            );
        }
        (false, true) => {
            return Err(
                PrivacyZkX509TransitionValidationErrorV1::CaMembershipRootChangedWithoutTrustStoreDigest,
            );
        }
        (true, true) => {}
    }
    Ok(())
}

/// Validate an irreversible trust-store revocation.
///
/// # Errors
///
/// Rejects malformed records, identity changes, stale/skipped epochs,
/// predecessor substitution, nonterminal successors, or trust-store/root
/// changes.
pub fn validate_zk_x509_trust_anchor_revocation_v1(
    current: &PrivacyZkX509TrustAnchorRecordV1,
    successor: &PrivacyZkX509TrustAnchorRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Revoked {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.trust_store_digest != current.trust_store_digest
        || successor.ca_membership_root != current.ca_membership_root
        || successor.ca_membership_root_epoch != current.ca_membership_root_epoch
    {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// Validate an active-to-active certificate-policy rotation.
///
/// # Errors
///
/// Rejects malformed records, namespace changes, stale/skipped epochs,
/// predecessor substitution, terminal successors, and no-op rotations.
pub fn validate_zk_x509_certificate_policy_rotation_v1(
    current: &PrivacyZkX509CertificatePolicyRecordV1,
    successor: &PrivacyZkX509CertificatePolicyRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    if successor.policy_id != current.policy_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PolicyIdMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    if successor.policy_digest == current.policy_digest
        && successor.required_key_usage == current.required_key_usage
        && successor.required_extended_key_usages == current.required_extended_key_usages
        && successor.required_disclosed_attribute_indices
            == current.required_disclosed_attribute_indices
    {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationContentsUnchanged);
    }
    Ok(())
}

/// Validate an irreversible certificate-policy revocation.
///
/// # Errors
///
/// Rejects malformed records, namespace changes, stale/skipped epochs,
/// predecessor substitution, nonterminal successors, or policy changes.
pub fn validate_zk_x509_certificate_policy_revocation_v1(
    current: &PrivacyZkX509CertificatePolicyRecordV1,
    successor: &PrivacyZkX509CertificatePolicyRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    if successor.policy_id != current.policy_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PolicyIdMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Revoked {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.policy_digest != current.policy_digest
        || successor.required_key_usage != current.required_key_usage
        || successor.required_extended_key_usages != current.required_extended_key_usages
        || successor.required_disclosed_attribute_indices
            != current.required_disclosed_attribute_indices
    {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// Validate an active-to-active signed-CRL rotation.
///
/// # Errors
///
/// Rejects malformed records, namespace or issuer-key changes,
/// stale/skipped epochs, predecessor substitution, non-increasing
/// `thisUpdate`, terminal successors, and no-op rotations.
pub fn validate_zk_x509_crl_rotation_v1(
    current: &PrivacyZkX509CrlRecordV1,
    successor: &PrivacyZkX509CrlRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    if successor.certificate_policy_id != current.certificate_policy_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PolicyIdMismatch);
    }
    if successor.issuer_spki_digest != current.issuer_spki_digest {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CrlIssuerSpkiDigestMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Active {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationSuccessorNotActive);
    }
    if successor.this_update_unix_seconds <= current.this_update_unix_seconds {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CrlThisUpdateNotIncreasing);
    }
    if successor.crl_number <= current.crl_number {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CrlNumberNotIncreasing);
    }
    if successor.crl_der_digest == current.crl_der_digest {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RotationContentsUnchanged);
    }
    Ok(())
}

/// Validate an irreversible signed-CRL lineage revocation.
///
/// # Errors
///
/// Rejects malformed records, namespace or issuer-key changes,
/// stale/skipped epochs, predecessor substitution, nonterminal successors,
/// or any change to the signed CRL or validity window.
pub fn validate_zk_x509_crl_revocation_v1(
    current: &PrivacyZkX509CrlRecordV1,
    successor: &PrivacyZkX509CrlRecordV1,
) -> Result<(), PrivacyZkX509TransitionValidationErrorV1> {
    current
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidCurrent)?;
    successor
        .validate()
        .map_err(PrivacyZkX509TransitionValidationErrorV1::InvalidSuccessor)?;
    if successor.trust_anchor_id != current.trust_anchor_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::TrustAnchorIdMismatch);
    }
    if successor.certificate_policy_id != current.certificate_policy_id {
        return Err(PrivacyZkX509TransitionValidationErrorV1::PolicyIdMismatch);
    }
    if successor.issuer_spki_digest != current.issuer_spki_digest {
        return Err(PrivacyZkX509TransitionValidationErrorV1::CrlIssuerSpkiDigestMismatch);
    }
    validate_zk_x509_transition_common(
        current.record_epoch,
        current.record_digest,
        current.lifecycle,
        successor.record_epoch,
        successor.previous_record_digest,
    )?;
    if successor.lifecycle != PrivacyZkX509RecordLifecycleV1::Revoked {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationSuccessorNotRevoked);
    }
    if successor.crl_number != current.crl_number
        || successor.crl_der_digest != current.crl_der_digest
        || successor.this_update_unix_seconds != current.this_update_unix_seconds
        || successor.next_update_unix_seconds != current.next_update_unix_seconds
    {
        return Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged);
    }
    Ok(())
}

/// One public selectively disclosed X.509 subject attribute.
///
/// Indices use the paper's closed order: `0=C`, `1=O`, `2=OU`, `3=CN`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyZkX509DisclosedAttributeV1 {
    /// Closed attribute index.
    pub index: u8,
    /// Public digest of the privately salted canonical attribute value.
    pub attribute_digest: PrivacyAttributeDigestV1,
}

/// Native X.509 credential-predicate STARK statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct IrohaZkX509StarkP256StatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Trust-anchor issuer identifier.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Governed certificate-policy identifier.
    pub certificate_policy_id: PrivacyPolicyIdV1,
    /// Exact immutable trust-anchor revision selected by the statement.
    pub trust_anchor_record_digest: PrivacyZkX509TrustAnchorRecordDigestV1,
    /// Exact trust-anchor revision epoch selected by the statement.
    pub trust_anchor_record_epoch: u64,
    /// Exact immutable certificate-policy revision selected by the statement.
    pub certificate_policy_record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1,
    /// Exact certificate-policy revision epoch selected by the statement.
    pub certificate_policy_record_epoch: u64,
    /// Exact immutable signed-CRL revision selected by the statement.
    pub crl_record_digest: PrivacyZkX509CrlRecordDigestV1,
    /// Exact signed-CRL revision epoch selected by the statement.
    pub crl_record_epoch: u64,
    /// Governance-scoped digest of the leaf certificate subject public key.
    ///
    /// Parent-chain keys and the private chain depth are deliberately excluded:
    /// the proof binds those through path validation, signatures, and the
    /// governed trust anchor.
    pub subject_public_key_digest: PrivacyCertificateKeyDigestV1,
    /// CA trust-store membership root authenticating the terminal certificate.
    pub ca_membership_root: PrivacyRootV1,
    /// Epoch at which the CA membership root was canonical.
    pub ca_membership_root_epoch: u64,
    /// Required RFC 5280 key usages.
    pub key_usage: PrivacyX509KeyUsageV1,
    /// Required extended-key-usage purposes, sorted in enum order.
    pub extended_key_usages: Vec<PrivacyX509ExtendedKeyUsageV1>,
    /// Canonical public selective disclosures in strict attribute-index order.
    pub disclosed_attributes: Vec<PrivacyZkX509DisclosedAttributeV1>,
    /// Earliest consensus timestamp at which this presentation may execute.
    ///
    /// The private certificate chain and signed CRL must cover the complete
    /// presentation window, so no exact certificate dates are disclosed.
    pub presentation_not_before_unix_seconds: u64,
    /// Latest consensus timestamp at which this presentation may execute.
    ///
    /// This bound is inclusive; the private CRL `nextUpdate` remains exclusive.
    pub presentation_not_after_unix_seconds: u64,
    /// Public wallet account to which the certificate showing is bound.
    pub wallet_account: AccountId,
    /// Wallet challenge preventing cross-account or cross-session replay.
    pub wallet_challenge: PrivacyChallengeV1,
    /// Nullifier derived from the certificate serial and policy.
    pub certificate_nullifier: PrivacyNullifierV1,
}

/// Canonical little-endian element of the fixed Jindo coefficient field.
///
/// The compiled modulus is `60272^16 + 1`. Fixed width at the type boundary
/// eliminates ambiguous byte order, truncation, and alternate field regimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct PrivacyJindoFieldElementV1 {
    /// Exact canonical little-endian residue.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub encoding: [u8; IROHA_JINDO_FIELD_ELEMENT_BYTES_V1],
}

impl PrivacyJindoFieldElementV1 {
    /// Construct a fixed-width field-element encoding.
    #[must_use]
    pub const fn new(encoding: [u8; IROHA_JINDO_FIELD_ELEMENT_BYTES_V1]) -> Self {
        Self { encoding }
    }

    /// Borrow the exact field-element bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; IROHA_JINDO_FIELD_ELEMENT_BYTES_V1] {
        &self.encoding
    }
}

/// Canonical public outer commitment in the fixed Jindo lattice profile.
///
/// The byte string contains 13 × 256 signed little-endian `i32`
/// coefficients. Native verification additionally enforces the compiled
/// rounded-coefficient bound.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct PrivacyJindoLatticeCommitmentV1 {
    /// Exact governed lattice-commitment encoding.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub encoding: Vec<u8>,
}

impl PrivacyJindoLatticeCommitmentV1 {
    /// Construct a fixed-profile lattice-commitment encoding.
    #[must_use]
    pub fn new(encoding: Vec<u8>) -> Self {
        Self { encoding }
    }

    /// Borrow the exact lattice-commitment bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.encoding
    }
}

/// Native Jindo batched univariate lattice polynomial-opening statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct IrohaJindoPolynomialCommitmentStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Public commitments to degree-bounded univariate polynomials.
    pub polynomial_commitments: Vec<PrivacyJindoLatticeCommitmentV1>,
    /// One common univariate evaluation point.
    pub evaluation_point: PrivacyJindoFieldElementV1,
    /// Claimed values in exact polynomial-commitment order.
    pub claimed_evaluations: Vec<PrivacyJindoFieldElementV1>,
}

/// One direct 64-bit attribute in the fixed Bootle/Lantern credential profile.
///
/// Bits are interpreted little-endian and become the 64 binary coefficients
/// of exactly one application-ring polynomial. This is deliberately not an
/// arbitrary byte string or a digest-preimage claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct BootleLanternAttributeValueV1(
    /// Exact little-endian 64-bit attribute encoding.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub [u8; BOOTLE_LANTERN_ATTRIBUTE_BYTES_V1],
);

impl BootleLanternAttributeValueV1 {
    /// Construct one direct attribute value.
    #[must_use]
    pub const fn new(bytes: [u8; BOOTLE_LANTERN_ATTRIBUTE_BYTES_V1]) -> Self {
        Self(bytes)
    }

    /// Borrow the exact direct attribute bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; BOOTLE_LANTERN_ATTRIBUTE_BYTES_V1] {
        &self.0
    }
}

/// One polynomial in `Z_12289[X]/(X^64 + 1)`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct BootleLanternPolynomialV1 {
    /// Exactly 64 canonical coefficients, each strictly below 12,289.
    pub coefficients: Vec<u16>,
}

/// Canonical issuer verification matrix `B` in the application ring.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct BootleLanternIssuerPublicMatrixV1 {
    /// Exactly 64 polynomials in row-major 8-by-8 order.
    pub entries: Vec<BootleLanternPolynomialV1>,
}

/// Governed allowed values for one required public attribute.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct BootleLanternAllowedAttributeValuesV1 {
    /// Strictly increasing values; empty means any disclosed value is allowed.
    pub values: Vec<BootleLanternAttributeValueV1>,
}

/// Forward-only lifecycle of one authoritative Bootle/Lantern issuer policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "state", content = "value", deny_unknown_fields)
)]
pub enum BootleLanternIssuerPolicyLifecycleV1 {
    /// Presentations selecting the exact current record may be admitted.
    #[cfg_attr(feature = "json", norito(rename = "active"))]
    Active,
    /// Terminal state; the lineage cannot be rotated or reactivated.
    #[cfg_attr(feature = "json", norito(rename = "revoked"))]
    Revoked,
}

/// Committed issuer key and selective-disclosure policy trusted by verification.
///
/// The proof submitter supplies only the record identity and digest in the
/// statement. Core resolves this complete record from committed state.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct BootleLanternIssuerPolicyV1 {
    /// Credential issuer governed by this record.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Stable policy identity within the issuer namespace.
    pub policy_id: PrivacyPolicyIdV1,
    /// Monotonically increasing policy/key epoch.
    pub epoch: u64,
    /// Forward-only active or terminal-revoked lifecycle.
    pub lifecycle: BootleLanternIssuerPolicyLifecycleV1,
    /// Exact issuer parameter artifact identity.
    pub issuer_parameter_id: PrivacyParameterIdV1,
    /// Digest of the exact issuer parameter artifact.
    pub issuer_parameter_digest: PrivacyParameterDigestV1,
    /// Canonical issuer verification matrix `B`.
    pub issuer_public_matrix: BootleLanternIssuerPublicMatrixV1,
    /// Bitmap of attributes that every presentation must disclose.
    pub required_disclosure_bitmap: u8,
    /// Per-attribute allowed public values in fixed attribute order.
    pub allowed_values: Vec<BootleLanternAllowedAttributeValuesV1>,
    /// Digest of this record with this field normalized to zero.
    pub record_digest: PrivacyBootleLanternIssuerPolicyDigestV1,
}

impl BootleLanternIssuerPolicyV1 {
    /// Compute the exact digest of the issuer verification matrix selected by
    /// `issuer_parameter_id`.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if canonical encoding of the matrix
    /// unexpectedly fails.
    pub fn computed_issuer_parameter_digest(
        &self,
    ) -> Result<PrivacyParameterDigestV1, norito::Error> {
        let encoded = norito::to_bytes(&self.issuer_public_matrix)?;
        let mut hasher = Sha256::new();
        hasher.update(BOOTLE_LANTERN_ISSUER_PARAMETER_DIGEST_DOMAIN_V1);
        hasher.update(self.issuer_parameter_id.as_bytes());
        hasher.update(
            u64::try_from(encoded.len())
                .expect("Norito output length fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(encoded);
        Ok(PrivacyParameterDigestV1::new(hasher.finalize().into()))
    }

    /// Compute the canonical record digest with `record_digest` normalized to zero.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if canonical encoding of the normalized record
    /// unexpectedly fails.
    pub fn computed_record_digest(
        &self,
    ) -> Result<PrivacyBootleLanternIssuerPolicyDigestV1, norito::Error> {
        let mut normalized = self.clone();
        normalized.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        let encoded = norito::to_bytes(&normalized)?;
        let mut hasher = Sha256::new();
        hasher.update(BOOTLE_LANTERN_ISSUER_POLICY_DIGEST_DOMAIN_V1);
        hasher.update(
            u64::try_from(encoded.len())
                .expect("Norito output length fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(encoded);
        Ok(PrivacyBootleLanternIssuerPolicyDigestV1::new(
            hasher.finalize().into(),
        ))
    }

    /// Validate canonical issuer key, disclosure rules, and self-authenticating digest.
    ///
    /// This intrinsic check does not make the record trusted. Core must resolve
    /// it from committed state and separately match its issuer parameter
    /// artifact before native verification.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic structural or digest failure.
    pub fn validate(&self) -> Result<(), BootleLanternIssuerPolicyValidationErrorV1> {
        if self.issuer_id.is_zero() {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::ZeroIssuerId);
        }
        if self.policy_id.is_zero() {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::ZeroPolicyId);
        }
        if self.epoch == 0 {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::ZeroEpoch);
        }
        if self.issuer_parameter_id.is_zero() {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::ZeroIssuerParameterId);
        }
        if self.issuer_parameter_digest.is_zero() {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::ZeroIssuerParameterDigest);
        }

        let mut matrix_is_zero = true;
        let matrix_entries = self.issuer_public_matrix.entries.len();
        let expected_matrix_entries =
            BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1 * BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1;
        if matrix_entries != expected_matrix_entries {
            return Err(
                BootleLanternIssuerPolicyValidationErrorV1::InvalidIssuerMatrixEntryCount {
                    count: u32::try_from(matrix_entries).map_err(|_| {
                        BootleLanternIssuerPolicyValidationErrorV1::IssuerMatrixEntryCountOverflow
                    })?,
                    expected: u32::try_from(expected_matrix_entries)
                        .expect("fixed matrix entry count fits u32"),
                },
            );
        }
        for (entry_index, polynomial) in self.issuer_public_matrix.entries.iter().enumerate() {
            if polynomial.coefficients.len() != BOOTLE_LANTERN_RING_DEGREE_V1 {
                return Err(
                    BootleLanternIssuerPolicyValidationErrorV1::InvalidPolynomialCoefficientCount {
                        polynomial: u8::try_from(entry_index)
                            .expect("fixed matrix entry index fits u8"),
                        count: u32::try_from(polynomial.coefficients.len()).map_err(|_| {
                            BootleLanternIssuerPolicyValidationErrorV1::
                                    PolynomialCoefficientCountOverflow
                        })?,
                        expected: u32::try_from(BOOTLE_LANTERN_RING_DEGREE_V1)
                            .expect("fixed ring degree fits u32"),
                    },
                );
            }
            for (coefficient_index, coefficient) in
                polynomial.coefficients.iter().copied().enumerate()
            {
                if coefficient >= BOOTLE_LANTERN_APPLICATION_MODULUS_V1 {
                    return Err(
                        BootleLanternIssuerPolicyValidationErrorV1::NonCanonicalMatrixCoefficient {
                            row: u8::try_from(
                                entry_index / BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1,
                            )
                            .expect("fixed matrix row fits u8"),
                            column: u8::try_from(
                                entry_index % BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1,
                            )
                            .expect("fixed matrix column fits u8"),
                            coefficient: u8::try_from(coefficient_index)
                                .expect("fixed ring coefficient fits u8"),
                            value: coefficient,
                        },
                    );
                }
                matrix_is_zero &= coefficient == 0;
            }
        }
        if matrix_is_zero {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::AllZeroIssuerMatrix);
        }
        let expected_issuer_parameter_digest =
            self.computed_issuer_parameter_digest().map_err(|_| {
                BootleLanternIssuerPolicyValidationErrorV1::IssuerParameterEncodingFailure
            })?;
        if self.issuer_parameter_digest != expected_issuer_parameter_digest {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::IssuerParameterDigestMismatch);
        }

        if self.allowed_values.len() != BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1 {
            return Err(
                BootleLanternIssuerPolicyValidationErrorV1::InvalidAllowedValueRuleCount {
                    count: u32::try_from(self.allowed_values.len()).map_err(|_| {
                        BootleLanternIssuerPolicyValidationErrorV1::AllowedValueRuleCountOverflow
                    })?,
                    expected: u32::try_from(BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1)
                        .expect("fixed attribute count fits u32"),
                },
            );
        }
        for (index, allowed) in self.allowed_values.iter().enumerate() {
            let count = u32::try_from(allowed.values.len()).map_err(|_| {
                BootleLanternIssuerPolicyValidationErrorV1::AllowedValueCountOverflow
            })?;
            if count > BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1 {
                return Err(
                    BootleLanternIssuerPolicyValidationErrorV1::TooManyAllowedValues {
                        index: u8::try_from(index).expect("fixed attribute index fits u8"),
                        count,
                        max: BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1,
                    },
                );
            }
            let required = self.required_disclosure_bitmap & (1_u8 << index) != 0;
            if !required && !allowed.values.is_empty() {
                return Err(
                    BootleLanternIssuerPolicyValidationErrorV1::AllowedValuesForOptionalAttribute {
                        index: u8::try_from(index).expect("fixed attribute index fits u8"),
                    },
                );
            }
            if allowed.values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(
                    BootleLanternIssuerPolicyValidationErrorV1::
                        AllowedValuesNotStrictlyIncreasing {
                            index: u8::try_from(index).expect("fixed attribute index fits u8"),
                        },
                );
            }
        }

        if self.record_digest.is_zero() {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::ZeroRecordDigest);
        }
        let expected = self
            .computed_record_digest()
            .map_err(|_| BootleLanternIssuerPolicyValidationErrorV1::EncodingFailure)?;
        if self.record_digest != expected {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::RecordDigestMismatch);
        }
        Ok(())
    }

    /// Validate a first record for a newly created issuer-policy key.
    ///
    /// # Errors
    ///
    /// Returns an intrinsic record failure or rejects any initial epoch other
    /// than one.
    pub fn validate_initial(&self) -> Result<(), BootleLanternIssuerPolicyValidationErrorV1> {
        self.validate()?;
        if self.epoch != 1 {
            return Err(
                BootleLanternIssuerPolicyValidationErrorV1::InvalidInitialEpoch {
                    epoch: self.epoch,
                },
            );
        }
        if self.lifecycle != BootleLanternIssuerPolicyLifecycleV1::Active {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::InitialPolicyMustBeActive);
        }
        Ok(())
    }

    /// Validate an atomic replacement of one committed issuer-policy record.
    ///
    /// # Errors
    ///
    /// Rejects namespace changes, a non-increasing epoch, an unchanged
    /// rotation, or any intrinsically invalid successor.
    pub fn validate_successor(
        &self,
        previous: &Self,
    ) -> Result<(), BootleLanternIssuerPolicyValidationErrorV1> {
        previous.validate()?;
        self.validate()?;
        if previous.lifecycle == BootleLanternIssuerPolicyLifecycleV1::Revoked {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::PolicyAlreadyRevoked);
        }
        if self.issuer_id != previous.issuer_id {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::IssuerIdChanged);
        }
        if self.policy_id != previous.policy_id {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::PolicyIdChanged);
        }
        let expected_epoch = previous
            .epoch
            .checked_add(1)
            .ok_or(BootleLanternIssuerPolicyValidationErrorV1::PolicyEpochOverflow)?;
        if self.epoch != expected_epoch {
            return Err(
                BootleLanternIssuerPolicyValidationErrorV1::NonConsecutiveEpoch {
                    previous: previous.epoch,
                    next: self.epoch,
                    expected: expected_epoch,
                },
            );
        }
        if self.issuer_parameter_id == previous.issuer_parameter_id
            && self.issuer_parameter_digest == previous.issuer_parameter_digest
            && self.issuer_public_matrix == previous.issuer_public_matrix
            && self.required_disclosure_bitmap == previous.required_disclosure_bitmap
            && self.allowed_values == previous.allowed_values
            && self.lifecycle == previous.lifecycle
        {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::UnchangedRotation);
        }
        Ok(())
    }

    /// Validate an active-to-active exact successor.
    ///
    /// # Errors
    ///
    /// Returns the first generic successor failure or rejects a successor that
    /// does not remain active.
    pub fn validate_rotation_successor(
        &self,
        previous: &Self,
    ) -> Result<(), BootleLanternIssuerPolicyValidationErrorV1> {
        self.validate_successor(previous)?;
        if self.lifecycle != BootleLanternIssuerPolicyLifecycleV1::Active {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::RotationMustRemainActive);
        }
        Ok(())
    }

    /// Validate an active-to-terminal-revoked exact successor.
    ///
    /// # Errors
    ///
    /// Returns the first generic successor failure or rejects a successor that
    /// is not terminal-revoked.
    pub fn validate_revocation_successor(
        &self,
        previous: &Self,
    ) -> Result<(), BootleLanternIssuerPolicyValidationErrorV1> {
        self.validate_successor(previous)?;
        if self.lifecycle != BootleLanternIssuerPolicyLifecycleV1::Revoked {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::RevocationMustBeRevoked);
        }
        if self.issuer_parameter_id != previous.issuer_parameter_id
            || self.issuer_parameter_digest != previous.issuer_parameter_digest
            || self.issuer_public_matrix != previous.issuer_public_matrix
            || self.required_disclosure_bitmap != previous.required_disclosure_bitmap
            || self.allowed_values != previous.allowed_values
        {
            return Err(BootleLanternIssuerPolicyValidationErrorV1::RevocationMustPreservePolicy);
        }
        Ok(())
    }
}

/// Structural failure for a committed Bootle/Lantern issuer-policy record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BootleLanternIssuerPolicyValidationErrorV1 {
    /// Issuer identifier is zero.
    #[error("Bootle/Lantern issuer id must be non-zero")]
    ZeroIssuerId,
    /// Policy identifier is zero.
    #[error("Bootle/Lantern policy id must be non-zero")]
    ZeroPolicyId,
    /// Record epoch is zero.
    #[error("Bootle/Lantern issuer-policy epoch must be non-zero")]
    ZeroEpoch,
    /// Issuer parameter identifier is zero.
    #[error("Bootle/Lantern issuer parameter id must be non-zero")]
    ZeroIssuerParameterId,
    /// Issuer parameter digest is zero.
    #[error("Bootle/Lantern issuer parameter digest must be non-zero")]
    ZeroIssuerParameterDigest,
    /// Matrix entry count overflowed its canonical diagnostic.
    #[error("Bootle/Lantern issuer matrix entry count overflow")]
    IssuerMatrixEntryCountOverflow,
    /// Matrix does not contain exactly 64 row-major polynomials.
    #[error("Bootle/Lantern issuer matrix has {count} entries; expected {expected}")]
    InvalidIssuerMatrixEntryCount {
        /// Observed entry count.
        count: u32,
        /// Fixed entry count.
        expected: u32,
    },
    /// Polynomial coefficient count overflowed its canonical diagnostic.
    #[error("Bootle/Lantern polynomial coefficient count overflow")]
    PolynomialCoefficientCountOverflow,
    /// One matrix polynomial does not contain exactly 64 coefficients.
    #[error(
        "Bootle/Lantern issuer matrix polynomial {polynomial} has {count} coefficients; expected {expected}"
    )]
    InvalidPolynomialCoefficientCount {
        /// Row-major matrix-polynomial index.
        polynomial: u8,
        /// Observed coefficient count.
        count: u32,
        /// Fixed coefficient count.
        expected: u32,
    },
    /// One issuer matrix coefficient is not a canonical residue.
    #[error(
        "Bootle/Lantern issuer matrix coefficient B[{row}][{column}][{coefficient}]={value} is not below 12289"
    )]
    NonCanonicalMatrixCoefficient {
        /// Matrix row.
        row: u8,
        /// Matrix column.
        column: u8,
        /// Polynomial coefficient.
        coefficient: u8,
        /// Rejected residue.
        value: u16,
    },
    /// Issuer matrix is the all-zero sentinel.
    #[error("Bootle/Lantern issuer matrix must not be all zero")]
    AllZeroIssuerMatrix,
    /// Canonical encoding of the issuer verification matrix failed.
    #[error("Bootle/Lantern issuer parameter encoding failed")]
    IssuerParameterEncodingFailure,
    /// The declared issuer parameter digest does not authenticate the matrix.
    #[error("Bootle/Lantern issuer parameter digest does not match the verification matrix")]
    IssuerParameterDigestMismatch,
    /// An allowed-value vector length overflowed its canonical count.
    #[error("Bootle/Lantern allowed-value count overflow")]
    AllowedValueCountOverflow,
    /// Attribute-rule vector length overflowed its canonical diagnostic.
    #[error("Bootle/Lantern allowed-value rule count overflow")]
    AllowedValueRuleCountOverflow,
    /// Policy does not contain exactly eight attribute-rule entries.
    #[error("Bootle/Lantern policy has {count} attribute rules; expected {expected}")]
    InvalidAllowedValueRuleCount {
        /// Observed rule count.
        count: u32,
        /// Fixed rule count.
        expected: u32,
    },
    /// One attribute allows too many governed public values.
    #[error("Bootle/Lantern attribute {index} has {count} allowed values, exceeding maximum {max}")]
    TooManyAllowedValues {
        /// Attribute index.
        index: u8,
        /// Observed value count.
        count: u32,
        /// Fixed maximum.
        max: u32,
    },
    /// A non-required attribute carries an unenforceable allowed-value policy.
    #[error("Bootle/Lantern optional attribute {index} must not carry allowed values")]
    AllowedValuesForOptionalAttribute {
        /// Attribute index.
        index: u8,
    },
    /// Allowed values contain a duplicate or are out of order.
    #[error("Bootle/Lantern attribute {index} allowed values must be strictly increasing")]
    AllowedValuesNotStrictlyIncreasing {
        /// Attribute index.
        index: u8,
    },
    /// Record digest is zero.
    #[error("Bootle/Lantern issuer-policy record digest must be non-zero")]
    ZeroRecordDigest,
    /// Canonical normalized record encoding failed.
    #[error("Bootle/Lantern issuer-policy record encoding failed")]
    EncodingFailure,
    /// Record digest does not match the canonical record contents.
    #[error("Bootle/Lantern issuer-policy record digest mismatch")]
    RecordDigestMismatch,
    /// A newly created issuer-policy key did not start at epoch one.
    #[error("Bootle/Lantern initial issuer-policy epoch {epoch} must equal one")]
    InvalidInitialEpoch {
        /// Rejected initial epoch.
        epoch: u64,
    },
    /// A newly registered lineage was not active.
    #[error("Bootle/Lantern initial issuer policy must be active")]
    InitialPolicyMustBeActive,
    /// A terminal-revoked lineage was used as a successor parent.
    #[error("Bootle/Lantern issuer policy is already terminal-revoked")]
    PolicyAlreadyRevoked,
    /// The current epoch cannot be advanced.
    #[error("Bootle/Lantern issuer-policy epoch overflow")]
    PolicyEpochOverflow,
    /// A rotation changed the issuer namespace.
    #[error("Bootle/Lantern issuer-policy rotation must not change issuer id")]
    IssuerIdChanged,
    /// A rotation changed the policy namespace.
    #[error("Bootle/Lantern issuer-policy rotation must not change policy id")]
    PolicyIdChanged,
    /// A replacement epoch was not exactly the next epoch.
    #[error(
        "Bootle/Lantern issuer-policy epoch must advance exactly once: previous {previous}, next {next}, expected {expected}"
    )]
    NonConsecutiveEpoch {
        /// Current committed epoch.
        previous: u64,
        /// Proposed successor epoch.
        next: u64,
        /// Only accepted successor epoch.
        expected: u64,
    },
    /// A replacement changed only epoch and digest.
    #[error("Bootle/Lantern issuer-policy rotation must change key, parameters, or policy rules")]
    UnchangedRotation,
    /// A rotation attempted to revoke the lineage.
    #[error("Bootle/Lantern issuer-policy rotation successor must remain active")]
    RotationMustRemainActive,
    /// A revocation failed to enter the terminal state.
    #[error("Bootle/Lantern issuer-policy revocation successor must be revoked")]
    RevocationMustBeRevoked,
    /// A revocation attempted to rotate key material or disclosure rules.
    #[error("Bootle/Lantern issuer-policy revocation must preserve key material and policy rules")]
    RevocationMustPreservePolicy,
}

/// One canonical Bootle/Lantern selective-disclosure entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct BootleLanternDisclosedAttributeV1 {
    /// Zero-based index in the fixed eight-attribute credential.
    pub index: u8,
    /// Direct public 64-bit attribute value.
    pub value: BootleLanternAttributeValueV1,
}

/// Native Bootle Lantern/LNP22 module-lattice anonymous-credential statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct IrohaBootleLanternAnoncredStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Anonymous-credential issuer identifier.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Selective-disclosure policy identifier.
    pub policy_id: PrivacyPolicyIdV1,
    /// Exact current committed issuer-policy epoch.
    pub issuer_policy_epoch: u64,
    /// Digest of the complete committed issuer-policy record.
    pub issuer_policy_record_digest: PrivacyBootleLanternIssuerPolicyDigestV1,
    /// Exact issuer parameter-set identifier.
    pub issuer_parameter_id: PrivacyParameterIdV1,
    /// Digest of the issuer parameter set.
    pub issuer_parameter_digest: PrivacyParameterDigestV1,
    /// Strictly increasing direct selectively disclosed attributes.
    pub disclosures: Vec<BootleLanternDisclosedAttributeV1>,
}

/// Direction of a public value balance relative to a private pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "direction", content = "value", deny_unknown_fields)
)]
pub enum PrivacyValueBalanceDirectionV1 {
    /// No public value enters or leaves the pool.
    Balanced,
    /// Public value enters the private pool.
    IntoPool,
    /// Private value leaves the pool.
    OutOfPool,
}

/// Signed public value balance represented without JSON-ambiguous `i128`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyValueBalanceV1 {
    /// Direction relative to the private pool.
    pub direction: PrivacyValueBalanceDirectionV1,
    /// Absolute atomic amount.
    pub amount: u128,
}

impl PrivacyValueBalanceV1 {
    /// Construct a zero public value balance.
    #[must_use]
    pub const fn balanced() -> Self {
        Self {
            direction: PrivacyValueBalanceDirectionV1::Balanced,
            amount: 0,
        }
    }

    /// Validate direction and magnitude consistency.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyStatementValidationError::InvalidValueBalance`] when a
    /// balanced value is non-zero or a directional value is zero.
    pub fn validate(&self) -> Result<(), PrivacyStatementValidationError> {
        let valid = match self.direction {
            PrivacyValueBalanceDirectionV1::Balanced => self.amount == 0,
            PrivacyValueBalanceDirectionV1::IntoPool
            | PrivacyValueBalanceDirectionV1::OutOfPool => self.amount != 0,
        };
        if !valid {
            return Err(PrivacyStatementValidationError::InvalidValueBalance {
                direction: self.direction,
                amount: self.amount,
            });
        }
        Ok(())
    }
}

/// Exact public data for one Orchard V3 action.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyOrchardActionV1 {
    /// Canonical Pallas-base nullifier encoding.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub nullifier: [u8; 32],
    /// Canonical non-identity randomized RedPallas verification key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub randomized_key: [u8; 32],
    /// Canonical extracted note commitment.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub note_commitment: [u8; 32],
    /// Canonical non-identity ephemeral Pallas public key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub ephemeral_key: [u8; 32],
    /// Exact 580-byte Orchard encrypted-note ciphertext.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub encrypted_note: Vec<u8>,
    /// Exact 80-byte Orchard outgoing ciphertext.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub outgoing_ciphertext: Vec<u8>,
    /// Canonical Pallas value commitment.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub value_commitment: [u8; 32],
}

/// Orchard Halo2 private action-bundle statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct OrchardHalo2ActionsStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset represented by the Orchard action.
    pub asset_definition_id: AssetDefinitionId,
    /// Orchard pool namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Admitted note-commitment tree anchor.
    pub anchor: PrivacyRootV1,
    /// Epoch at which `anchor` was canonical.
    pub anchor_epoch: u64,
    /// Non-empty ordered Orchard actions.
    ///
    /// The node derives the successor frontier and root by appending these
    /// note commitments to its authoritative pool frontier. A caller-selected
    /// successor root is intentionally unrepresentable.
    pub actions: Vec<PrivacyOrchardActionV1>,
    /// Public value balance in atomic units.
    pub value_balance: PrivacyValueBalanceV1,
    /// Last block height at which the action is valid.
    pub expiry_height: u64,
}

/// Monero FCMP++ full-chain-membership transfer statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct MoneroFcmpPlusPlusStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset transferred by the private action.
    pub asset_definition_id: AssetDefinitionId,
    /// FCMP++ output-set namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Canonical typed full-output-set root.
    pub output_set_root: PrivacyFcmpTreeRootV1,
    /// Epoch at which the output-set root was canonical.
    pub root_epoch: u64,
    /// Complete public inputs for each hidden consumed output.
    pub inputs: Vec<PrivacyFcmpInputPublicV1>,
    /// Complete new output tuples in canonical append order.
    ///
    /// Validators derive the successor typed root and epoch from these tuples
    /// and the authoritative mixed-radix frontier. A caller-selected successor
    /// is intentionally unrepresentable.
    pub outputs: Vec<PrivacyFcmpOutputTupleV1>,
    /// Encrypted new outputs, aligned one-to-one with `outputs`.
    pub encrypted_outputs: Vec<PrivacyFcmpEncryptedOutputV1>,
}

/// Native IVM private-note execution statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct IrohaIvmPrivateNoteStarkStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset manipulated by the private program.
    pub asset_definition_id: AssetDefinitionId,
    /// Private-note pool namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact private IVM program identifier.
    pub program_id: PrivacyProgramIdV1,
    /// Digest of the exact canonical private-program action and public inputs.
    pub action_digest: PrivacyActionDigestV1,
    /// Canonical private-note state root.
    pub state_root: PrivacyRootV1,
    /// Epoch at which `state_root` was canonical.
    pub root_epoch: u64,
    /// Consumed note nullifiers.
    pub nullifiers: Vec<PrivacyNullifierV1>,
    /// New note commitments in canonical append order.
    ///
    /// Validators derive the successor program-state root and epoch from these
    /// commitments and the authoritative compact frontier. A caller-selected
    /// successor is intentionally unrepresentable.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Encrypted new notes, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Public value balance in atomic units.
    pub value_balance: PrivacyValueBalanceV1,
    /// Ledger epoch bound into private program execution.
    pub execution_epoch: u64,
}

impl IrohaIvmPrivateNoteStarkStatementV1 {
    /// Compute the action digest with its self-authenticating field normalized
    /// to zero.
    ///
    /// # Errors
    ///
    /// Returns a Norito error if canonical statement encoding unexpectedly
    /// fails.
    pub fn computed_action_digest(&self) -> Result<PrivacyActionDigestV1, norito::Error> {
        let mut normalized = self.clone();
        normalized.action_digest = PrivacyActionDigestV1::new([0; 32]);
        let encoded = norito::to_bytes(&normalized)?;
        let mut hasher = Sha256::new();
        hasher.update(IVM_PRIVATE_NOTE_ACTION_DIGEST_DOMAIN_V1);
        hasher.update(
            u64::try_from(encoded.len())
                .expect("Norito output length fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(encoded);
        Ok(PrivacyActionDigestV1::new(hasher.finalize().into()))
    }
}

/// Post-quantum authorization profile required by PQ-MASP v0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "authorization", content = "value", deny_unknown_fields)
)]
pub enum PrivacyPqAuthorizationProfileV1 {
    /// ML-DSA-65 transaction authorization.
    MlDsa65,
}

/// Post-quantum note-encryption profile required by PQ-MASP v0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "encryption", content = "value", deny_unknown_fields)
)]
pub enum PrivacyPqNoteEncryptionProfileV1 {
    /// ML-KEM-768 key establishment with XChaCha20-Poly1305 payload encryption.
    MlKem768XChaCha20Poly1305,
}

/// Post-quantum MASP STARK transfer statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PqMaspStarkStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset transferred by the private action.
    pub asset_definition_id: AssetDefinitionId,
    /// PQ-MASP pool namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Admitted note-commitment tree anchor.
    pub anchor: PrivacyRootV1,
    /// Epoch at which `anchor` was canonical.
    pub anchor_epoch: u64,
    /// Consumed-note nullifiers.
    pub nullifiers: Vec<PrivacyNullifierV1>,
    /// New note commitments in canonical append order.
    ///
    /// Validators derive the successor anchor and epoch from these
    /// commitments and the authoritative compact frontier. A caller-selected
    /// successor is intentionally unrepresentable.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// ML-KEM-derived encrypted output notes, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Required transaction-authorization profile.
    pub authorization_profile: PrivacyPqAuthorizationProfileV1,
    /// Digest of the authorized ML-DSA key.
    pub authorization_key_digest: PrivacyAuthorizationKeyDigestV1,
    /// Required note-encryption profile.
    pub note_encryption_profile: PrivacyPqNoteEncryptionProfileV1,
    /// Digest of the ordered `(recipient key, ML-KEM encapsulation)` pairs for
    /// every encrypted output.
    pub note_encryption_key_digest: PrivacyNoteEncryptionKeyDigestV1,
    /// Ledger epoch bound into authorization.
    pub authorization_epoch: u64,
}

/// Protocol-typed canonical privacy statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.statement.v1")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "protocol", content = "statement", deny_unknown_fields)
)]
pub enum PrivacyStatementV1 {
    /// ZK-ACE post-quantum authorization statement.
    ZkAcePqAuthorizationV0(ZkAcePqAuthorizationStatementV1),
    /// Anonymous PGC k-out-of-n payment statement.
    AnonymousPgcKOutOfNV1(AnonymousPgcKOutOfNStatementV1),
    /// VeRange transparent range statement.
    VeRangeTransparentRangeV1(VeRangeTransparentRangeStatementV1),
    /// Native Iroha ZK-AMS admission/provisioning statement.
    IrohaZkAmsV1(IrohaZkAmsStatementV1),
    /// Vega existing-credential predicate statement.
    VegaExistingCredentialZkV0(VegaExistingCredentialStatementV1),
    /// Native Iroha P-256 X.509 predicate STARK statement.
    IrohaZkX509StarkP256V0(IrohaZkX509StarkP256StatementV1),
    /// Native Iroha Jindo batched univariate lattice polynomial-commitment statement.
    IrohaJindoPolynomialCommitmentV0(IrohaJindoPolynomialCommitmentStatementV1),
    /// Native Bootle Lantern/LNP22 anonymous-credential statement.
    IrohaBootleLanternAnoncredV1(IrohaBootleLanternAnoncredStatementV1),
    /// Orchard Halo2 action statement.
    OrchardHalo2ActionsV1(OrchardHalo2ActionsStatementV1),
    /// Monero FCMP++ membership statement.
    MoneroFcmpPlusPlusV1(MoneroFcmpPlusPlusStatementV1),
    /// Native IVM private-note STARK statement.
    IrohaIvmPrivateNoteStarkV1(IrohaIvmPrivateNoteStarkStatementV1),
    /// Post-quantum MASP STARK statement.
    PqMaspStarkV0(PqMaspStarkStatementV1),
}

impl PrivacyStatementV1 {
    /// Exact protocol carried by this statement variant.
    #[must_use]
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV0(_) => PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            Self::AnonymousPgcKOutOfNV1(_) => PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            Self::VeRangeTransparentRangeV1(_) => PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            Self::IrohaZkAmsV1(_) => PrivacyProtocolIdV1::IrohaZkAmsV1,
            Self::VegaExistingCredentialZkV0(_) => PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            Self::IrohaZkX509StarkP256V0(_) => PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            Self::IrohaJindoPolynomialCommitmentV0(_) => {
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0
            }
            Self::IrohaBootleLanternAnoncredV1(_) => {
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
            }
            Self::OrchardHalo2ActionsV1(_) => PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            Self::PqMaspStarkV0(_) => PrivacyProtocolIdV1::PqMaspStarkV0,
        }
    }

    /// Borrow the explicit shared context inside this protocol statement.
    #[must_use]
    pub const fn context(&self) -> &PrivacyStatementContextV1 {
        match self {
            Self::ZkAcePqAuthorizationV0(statement) => &statement.context,
            Self::AnonymousPgcKOutOfNV1(statement) => &statement.context,
            Self::VeRangeTransparentRangeV1(statement) => &statement.context,
            Self::IrohaZkAmsV1(statement) => &statement.context,
            Self::VegaExistingCredentialZkV0(statement) => &statement.context,
            Self::IrohaZkX509StarkP256V0(statement) => &statement.context,
            Self::IrohaJindoPolynomialCommitmentV0(statement) => &statement.context,
            Self::IrohaBootleLanternAnoncredV1(statement) => &statement.context,
            Self::OrchardHalo2ActionsV1(statement) => &statement.context,
            Self::MoneroFcmpPlusPlusV1(statement) => &statement.context,
            Self::IrohaIvmPrivateNoteStarkV1(statement) => &statement.context,
            Self::PqMaspStarkV0(statement) => &statement.context,
        }
    }

    /// Mutably borrow the explicit shared context inside this protocol statement.
    ///
    /// Transaction-intent normalization uses this single exhaustive boundary
    /// instead of duplicating protocol-specific statement matches.
    #[must_use]
    pub const fn context_mut(&mut self) -> &mut PrivacyStatementContextV1 {
        match self {
            Self::ZkAcePqAuthorizationV0(statement) => &mut statement.context,
            Self::AnonymousPgcKOutOfNV1(statement) => &mut statement.context,
            Self::VeRangeTransparentRangeV1(statement) => &mut statement.context,
            Self::IrohaZkAmsV1(statement) => &mut statement.context,
            Self::VegaExistingCredentialZkV0(statement) => &mut statement.context,
            Self::IrohaZkX509StarkP256V0(statement) => &mut statement.context,
            Self::IrohaJindoPolynomialCommitmentV0(statement) => &mut statement.context,
            Self::IrohaBootleLanternAnoncredV1(statement) => &mut statement.context,
            Self::OrchardHalo2ActionsV1(statement) => &mut statement.context,
            Self::MoneroFcmpPlusPlusV1(statement) => &mut statement.context,
            Self::IrohaIvmPrivateNoteStarkV1(statement) => &mut statement.context,
            Self::PqMaspStarkV0(statement) => &mut statement.context,
        }
    }

    /// Hash this complete protocol-tagged statement using canonical Norito bytes.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical statement encoding fails.
    pub fn digest(&self) -> Result<PrivacyStatementDigestV1, norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_STATEMENT_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(encoded.len())
                .expect("Norito output length always fits u64 on supported targets")
                .to_le_bytes(),
        );
        hasher.update(&encoded);
        Ok(PrivacyStatementDigestV1::new(*hasher.finalize().as_bytes()))
    }

    /// Validate the exact protocol statement and consensus resource bounds.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyStatementValidationError`] for any invalid explicit
    /// field, count, epoch, state item, encrypted output, scalar, or bound.
    pub fn validate(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyStatementValidationError> {
        limits
            .validate()
            .map_err(PrivacyStatementValidationError::InvalidLimits)?;
        self.context().validate(limits)?;
        match self {
            Self::ZkAcePqAuthorizationV0(statement) => validate_zk_ace(statement)?,
            Self::AnonymousPgcKOutOfNV1(statement) => validate_anonymous_pgc(statement, limits)?,
            Self::VeRangeTransparentRangeV1(statement) => validate_verange(statement, limits)?,
            Self::IrohaZkAmsV1(statement) => validate_zk_ams(statement)?,
            Self::VegaExistingCredentialZkV0(statement) => validate_vega(statement)?,
            Self::IrohaZkX509StarkP256V0(statement) => validate_zk_x509(statement)?,
            Self::IrohaJindoPolynomialCommitmentV0(statement) => validate_jindo(statement, limits)?,
            Self::IrohaBootleLanternAnoncredV1(statement) => validate_bootle_lantern(statement)?,
            Self::OrchardHalo2ActionsV1(statement) => validate_orchard(statement, limits)?,
            Self::MoneroFcmpPlusPlusV1(statement) => validate_fcmp(statement, limits)?,
            Self::IrohaIvmPrivateNoteStarkV1(statement) => {
                validate_ivm_private_note(statement, limits)?
            }
            Self::PqMaspStarkV0(statement) => validate_pq_masp(statement, limits)?,
        }
        let encoded =
            norito::to_bytes(self).map_err(|_| PrivacyStatementValidationError::EncodingFailure)?;
        let bytes = u64::try_from(encoded.len())
            .map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)?;
        if bytes > u64::from(limits.max_statement_and_encrypted_output_bytes_per_transaction) {
            return Err(
                PrivacyStatementValidationError::StatementAndEncryptedOutputsTooLarge {
                    bytes,
                    max: limits.max_statement_and_encrypted_output_bytes_per_transaction,
                },
            );
        }
        Ok(())
    }
}

fn validate_zk_ace(
    statement: &ZkAcePqAuthorizationStatementV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_commitment(statement.identity_commitment, 0)?;
    require_nonzero_id(statement.policy_id.is_zero(), PrivacyTypedFieldV1::PolicyId)?;
    require_nonzero_id(
        statement.policy_digest.is_zero(),
        PrivacyTypedFieldV1::PolicyDigest,
    )?;
    if statement.amount == 0 {
        return Err(PrivacyStatementValidationError::ZeroAmount);
    }
    require_epoch(
        statement.authorization_epoch,
        PrivacyEpochFieldV1::Authorization,
    )?;
    require_nullifier(statement.replay_nullifier, 0)
}

fn validate_anonymous_pgc(
    statement: &AnonymousPgcKOutOfNStatementV1,
    _limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    require_nonzero_id(
        statement.account_state_root.is_zero(),
        PrivacyTypedFieldV1::Root,
    )?;
    require_epoch(
        statement.account_state_root_epoch,
        PrivacyEpochFieldV1::Root,
    )?;
    validate_next_root_transition(
        statement.account_state_root,
        statement.account_state_root_epoch,
        statement.next_account_state_root,
        statement.next_account_state_root_epoch,
        PrivacyRootTransitionFieldV1::PgcAccountState,
    )?;
    let anonymity_set_size = u32_len(statement.anonymity_set_public_keys.len())?;
    if !ANONYMOUS_PGC_ANONYMITY_SET_SIZES_V1.contains(&anonymity_set_size) {
        return Err(
            PrivacyStatementValidationError::InvalidPgcAnonymitySetSize {
                size: anonymity_set_size,
            },
        );
    }
    let ciphertext_count = u32_len(statement.transfer_ciphertexts.len())?;
    if ciphertext_count != anonymity_set_size {
        return Err(
            PrivacyStatementValidationError::PgcPublicMemoCountMismatch {
                public_keys: anonymity_set_size,
                ciphertexts: ciphertext_count,
            },
        );
    }
    for (index, key) in statement
        .anonymity_set_public_keys
        .iter()
        .copied()
        .enumerate()
    {
        if key.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroP256Point {
                index: u32_index(index)?,
            });
        }
        if index > 0 && statement.anonymity_set_public_keys[index - 1] >= key {
            return Err(PrivacyStatementValidationError::PgcAnonymitySetNotStrictlyIncreasing);
        }
    }
    for (index, ciphertext) in statement.transfer_ciphertexts.iter().enumerate() {
        let index = u32_index(index)?;
        if ciphertext.left.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroP256CiphertextPoint {
                index,
                component: PrivacyP256CiphertextComponentV1::Left,
            });
        }
        if ciphertext.right.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroP256CiphertextPoint {
                index,
                component: PrivacyP256CiphertextComponentV1::Right,
            });
        }
    }
    let max_recipient_count =
        ANONYMOUS_PGC_MAX_RECIPIENTS_V1.min(anonymity_set_size.saturating_sub(1));
    if statement.recipient_count == 0 || statement.recipient_count > max_recipient_count {
        return Err(PrivacyStatementValidationError::InvalidPgcRecipientCount {
            count: statement.recipient_count,
            anonymity_set_size,
            max: max_recipient_count,
        });
    }
    Ok(())
}

fn validate_verange(
    statement: &VeRangeTransparentRangeStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.policy_id.is_zero(), PrivacyTypedFieldV1::PolicyId)?;
    let aggregation_max =
        VERANGE_HARD_MAX_AGGREGATION_COUNT_V1.min(limits.max_commitments_per_action);
    if statement.aggregation_count == 0 || statement.aggregation_count > aggregation_max {
        return Err(PrivacyStatementValidationError::InvalidAggregationCount {
            count: statement.aggregation_count,
            max: aggregation_max,
        });
    }
    if statement.value_commitments.is_empty() {
        return Err(PrivacyStatementValidationError::MissingCommitment);
    }
    for (index, commitment) in statement.value_commitments.iter().copied().enumerate() {
        if commitment.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroP256Point {
                index: u32_index(index)?,
            });
        }
    }
    if first_duplicate_index(&statement.value_commitments).is_some() {
        return Err(PrivacyStatementValidationError::DuplicateCommitment);
    }
    require_count(
        statement.value_commitments.len(),
        statement.aggregation_count,
        PrivacyCountFieldV1::AggregatedCommitments,
    )
}

fn validate_zk_ams(
    statement: &IrohaZkAmsStatementV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.issuer_id.is_zero(), PrivacyTypedFieldV1::IssuerId)?;
    if statement.issuer_public_key.is_zero() {
        return Err(PrivacyStatementValidationError::ZeroP256Point { index: 0 });
    }
    require_nonzero_id(
        statement.issuer_policy_record_digest.is_zero(),
        PrivacyTypedFieldV1::ZkAmsIssuerPolicyRecordDigest,
    )?;
    require_nonzero_id(
        statement.registry_id.is_zero(),
        PrivacyTypedFieldV1::RegistryId,
    )?;
    require_nonzero_id(
        statement.registry_record_digest.is_zero(),
        PrivacyTypedFieldV1::ZkAmsRegistryRecordDigest,
    )?;
    require_nonzero_id(statement.policy_id.is_zero(), PrivacyTypedFieldV1::PolicyId)?;
    require_nonzero_id(
        statement.policy_digest.is_zero(),
        PrivacyTypedFieldV1::PolicyDigest,
    )?;
    match &statement.action {
        PrivacyZkAmsActionV1::BatchAdmission(batch) => {
            require_nonzero_id(
                batch.account_registry_root.is_zero(),
                PrivacyTypedFieldV1::Root,
            )?;
            require_epoch(batch.account_registry_root_epoch, PrivacyEpochFieldV1::Root)?;
            validate_next_root_transition(
                batch.account_registry_root,
                batch.account_registry_root_epoch,
                batch.next_account_registry_root,
                batch.next_account_registry_root_epoch,
                PrivacyRootTransitionFieldV1::AccountRegistry,
            )?;
            let batch_size = u32_len(batch.anchors.len())?;
            if batch_size == 0 || batch_size > ZK_AMS_MAX_BATCH_SIZE_V1 {
                return Err(PrivacyStatementValidationError::InvalidBatchSize {
                    count: batch_size,
                    max: ZK_AMS_MAX_BATCH_SIZE_V1,
                });
            }
            for (index, anchor) in batch.anchors.iter().enumerate() {
                if anchor.phc_hash.is_zero() {
                    return Err(PrivacyStatementValidationError::ZeroZkAmsPhcHash {
                        index: u32_index(index)?,
                    });
                }
                if anchor.seed_public_key.is_zero() {
                    return Err(PrivacyStatementValidationError::ZeroZkAmsSeedPublicKey {
                        index: u32_index(index)?,
                    });
                }
            }
            for later in 1..batch.anchors.len() {
                if batch.anchors[..later]
                    .iter()
                    .any(|earlier| earlier.phc_hash == batch.anchors[later].phc_hash)
                {
                    return Err(PrivacyStatementValidationError::DuplicateZkAmsPhcHash);
                }
                if batch.anchors[..later]
                    .iter()
                    .any(|earlier| earlier.seed_public_key == batch.anchors[later].seed_public_key)
                {
                    return Err(PrivacyStatementValidationError::DuplicateZkAmsSeedPublicKey);
                }
            }
            Ok(())
        }
        PrivacyZkAmsActionV1::ProvisionAccount(provision) => {
            require_nonzero_id(
                provision.account_registry_root.is_zero(),
                PrivacyTypedFieldV1::Root,
            )?;
            require_epoch(
                provision.account_registry_root_epoch,
                PrivacyEpochFieldV1::Root,
            )?;
            let ring_size = u32_len(provision.admitted_seed_key_ring.len())?;
            if !ZK_AMS_RING_SIZES_V1.contains(&ring_size) {
                return Err(PrivacyStatementValidationError::InvalidZkAmsRingSize {
                    size: ring_size,
                });
            }
            for (index, key) in provision.admitted_seed_key_ring.iter().enumerate() {
                if key.is_zero() {
                    return Err(PrivacyStatementValidationError::ZeroZkAmsSeedPublicKey {
                        index: u32_index(index)?,
                    });
                }
            }
            if provision
                .admitted_seed_key_ring
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            {
                return Err(PrivacyStatementValidationError::ZkAmsSeedKeyRingNotStrictlyIncreasing);
            }
            if provision.key_image.is_zero() {
                return Err(PrivacyStatementValidationError::ZeroZkAmsKeyImage);
            }
            Ok(())
        }
    }
}

fn validate_vega(
    statement: &VegaExistingCredentialStatementV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.issuer_id.is_zero(), PrivacyTypedFieldV1::IssuerId)?;
    require_epoch(
        statement.issuer_record_epoch,
        PrivacyEpochFieldV1::VegaIssuerRecord,
    )?;
    require_nonzero_id(
        statement.issuer_record_digest.is_zero(),
        PrivacyTypedFieldV1::VegaIssuerRecordDigest,
    )?;
    if statement.issuer_public_key.is_zero() {
        return Err(PrivacyStatementValidationError::ZeroP256Point { index: 0 });
    }
    require_nonzero_id(
        statement.device_authentication_digest.is_zero(),
        PrivacyTypedFieldV1::VegaDeviceAuthenticationDigest,
    )?;
    require_nonzero_id(
        statement.reader_challenge.is_zero(),
        PrivacyTypedFieldV1::ReaderChallenge,
    )?;
    require_nonzero_id(
        statement.session_transcript_digest.is_zero(),
        PrivacyTypedFieldV1::SessionTranscriptDigest,
    )?;
    validate_vega_presentation_date(statement.presentation_date)?;
    if !(VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1..=VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1)
        .contains(&statement.minimum_age_years)
    {
        return Err(PrivacyStatementValidationError::InvalidVegaAgeThreshold {
            years: statement.minimum_age_years,
            min: VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1,
            max: VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1,
        });
    }
    Ok(())
}

fn validate_vega_presentation_date(
    date: PrivacyVegaMdlDateV1,
) -> Result<(), PrivacyStatementValidationError> {
    if !(VEGA_MDL_MIN_PRESENTATION_YEAR_V1..=VEGA_MDL_MAX_PRESENTATION_YEAR_V1).contains(&date.year)
    {
        return Err(
            PrivacyStatementValidationError::InvalidVegaPresentationYear {
                year: date.year,
                min: VEGA_MDL_MIN_PRESENTATION_YEAR_V1,
                max: VEGA_MDL_MAX_PRESENTATION_YEAR_V1,
            },
        );
    }
    let max_day = vega_gregorian_days_in_month(date.year, date.month).ok_or(
        PrivacyStatementValidationError::InvalidVegaPresentationDate {
            year: date.year,
            month: date.month,
            day: date.day,
        },
    )?;
    if date.day == 0 || date.day > max_day {
        return Err(
            PrivacyStatementValidationError::InvalidVegaPresentationDate {
                year: date.year,
                month: date.month,
                day: date.day,
            },
        );
    }
    Ok(())
}

fn vega_gregorian_days_in_month(year: u16, month: u8) -> Option<u8> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if vega_is_gregorian_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

fn vega_is_gregorian_leap_year(year: u16) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn validate_zk_x509(
    statement: &IrohaZkX509StarkP256StatementV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(
        statement.trust_anchor_id.is_zero(),
        PrivacyTypedFieldV1::IssuerId,
    )?;
    require_nonzero_id(
        statement.certificate_policy_id.is_zero(),
        PrivacyTypedFieldV1::PolicyId,
    )?;
    require_nonzero_id(
        statement.trust_anchor_record_digest.is_zero(),
        PrivacyTypedFieldV1::X509TrustAnchorRecordDigest,
    )?;
    require_epoch(
        statement.trust_anchor_record_epoch,
        PrivacyEpochFieldV1::X509TrustAnchorRecord,
    )?;
    require_nonzero_id(
        statement.certificate_policy_record_digest.is_zero(),
        PrivacyTypedFieldV1::X509CertificatePolicyRecordDigest,
    )?;
    require_epoch(
        statement.certificate_policy_record_epoch,
        PrivacyEpochFieldV1::X509CertificatePolicyRecord,
    )?;
    require_nonzero_id(
        statement.crl_record_digest.is_zero(),
        PrivacyTypedFieldV1::X509CrlRecordDigest,
    )?;
    require_epoch(
        statement.crl_record_epoch,
        PrivacyEpochFieldV1::X509CrlRecord,
    )?;
    require_nonzero_id(
        statement.subject_public_key_digest.is_zero(),
        PrivacyTypedFieldV1::CertificateKeyDigest,
    )?;
    require_nonzero_id(
        statement.ca_membership_root.is_zero(),
        PrivacyTypedFieldV1::Root,
    )?;
    require_epoch(
        statement.ca_membership_root_epoch,
        PrivacyEpochFieldV1::CertificateAuthorityMembership,
    )?;
    if !statement.key_usage.digital_signature {
        return Err(PrivacyStatementValidationError::InvalidX509KeyUsage);
    }
    if statement.extended_key_usages.is_empty() {
        return Err(PrivacyStatementValidationError::MissingX509ExtendedKeyUsage);
    }
    if statement.extended_key_usages.len() > ZK_X509_MAX_EXTENDED_KEY_USAGES_V1 {
        return Err(
            PrivacyStatementValidationError::TooManyX509ExtendedKeyUsages {
                actual: statement.extended_key_usages.len(),
                max: ZK_X509_MAX_EXTENDED_KEY_USAGES_V1,
            },
        );
    }
    for index in 1..statement.extended_key_usages.len() {
        if statement.extended_key_usages[index - 1] >= statement.extended_key_usages[index] {
            return Err(
                PrivacyStatementValidationError::X509ExtendedKeyUsagesNotStrictlyIncreasing,
            );
        }
    }
    if statement.disclosed_attributes.len() > ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1 {
        return Err(
            PrivacyStatementValidationError::TooManyX509DisclosedAttributes {
                actual: statement.disclosed_attributes.len(),
                max: ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1,
            },
        );
    }
    for (position, disclosed) in statement.disclosed_attributes.iter().enumerate() {
        if usize::from(disclosed.index) >= ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1 {
            return Err(
                PrivacyStatementValidationError::UnsupportedX509DisclosedAttributeIndex {
                    index: disclosed.index,
                },
            );
        }
        if disclosed.attribute_digest.is_zero() {
            return Err(
                PrivacyStatementValidationError::ZeroX509DisclosedAttributeDigest {
                    index: disclosed.index,
                },
            );
        }
        if position > 0 && statement.disclosed_attributes[position - 1].index >= disclosed.index {
            return Err(
                PrivacyStatementValidationError::X509DisclosedAttributesNotStrictlyIncreasing,
            );
        }
    }
    validate_zk_x509_presentation_window(
        statement.presentation_not_before_unix_seconds,
        statement.presentation_not_after_unix_seconds,
    )?;
    require_nonzero_id(
        statement.wallet_challenge.is_zero(),
        PrivacyTypedFieldV1::ReaderChallenge,
    )?;
    require_nullifier(statement.certificate_nullifier, 0)
}

fn validate_zk_x509_presentation_window(
    start: u64,
    end: u64,
) -> Result<(), PrivacyStatementValidationError> {
    if end <= start
        || end
            .checked_sub(start)
            .is_none_or(|seconds| seconds > ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1)
    {
        return Err(
            PrivacyStatementValidationError::InvalidX509PresentationWindow {
                start,
                end,
                max_seconds: ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1,
            },
        );
    }
    Ok(())
}

fn validate_jindo(
    statement: &IrohaJindoPolynomialCommitmentStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    let polynomial_count = u32_len(statement.polynomial_commitments.len())?;
    let polynomial_max = IROHA_JINDO_MAX_POLYNOMIALS_V1.min(limits.max_commitments_per_action);
    if polynomial_count == 0 || polynomial_count > polynomial_max {
        return Err(PrivacyStatementValidationError::InvalidBatchSize {
            count: polynomial_count,
            max: polynomial_max,
        });
    }
    for (index, commitment) in statement.polynomial_commitments.iter().enumerate() {
        let bytes = u32_len(commitment.encoding.len())?;
        if commitment.encoding.len() != IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1 {
            return Err(
                PrivacyStatementValidationError::InvalidJindoLatticeCommitmentSize {
                    index: u32_index(index)?,
                    bytes,
                    expected: u32::try_from(IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1)
                        .expect("fixed Jindo commitment width fits u32"),
                },
            );
        }
        if commitment.encoding.iter().all(|byte| *byte == 0) {
            return Err(
                PrivacyStatementValidationError::AllZeroJindoLatticeCommitment {
                    index: u32_index(index)?,
                },
            );
        }
        for (coefficient_index, bytes) in commitment.encoding.chunks_exact(4).enumerate() {
            let coefficient = i32::from_le_bytes(
                bytes
                    .try_into()
                    .expect("Jindo commitment width is a multiple of four"),
            );
            if !(IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1
                ..=IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1)
                .contains(&coefficient)
            {
                return Err(
                    PrivacyStatementValidationError::JindoCommitmentCoefficientOutOfRange {
                        commitment_index: u32_index(index)?,
                        coefficient_index: u32_index(coefficient_index)?,
                        value: coefficient,
                        min: IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1,
                        max: IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1,
                    },
                );
            }
        }
    }
    if first_duplicate_index(&statement.polynomial_commitments).is_some() {
        return Err(PrivacyStatementValidationError::DuplicateJindoLatticeCommitment);
    }
    require_count(
        statement.claimed_evaluations.len(),
        polynomial_count,
        PrivacyCountFieldV1::JindoClaimedEvaluations,
    )?;
    if !is_canonical_jindo_field_element(&statement.evaluation_point) {
        return Err(PrivacyStatementValidationError::NonCanonicalJindoEvaluationPoint);
    }
    for (index, claimed_evaluation) in statement.claimed_evaluations.iter().enumerate() {
        if !is_canonical_jindo_field_element(claimed_evaluation) {
            return Err(
                PrivacyStatementValidationError::NonCanonicalJindoClaimedEvaluation {
                    index: u32_index(index)?,
                },
            );
        }
    }
    Ok(())
}

const IROHA_JINDO_FIELD_MODULUS_LE_V1: [u8; IROHA_JINDO_FIELD_ELEMENT_BYTES_V1] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x81, 0x32, 0x37, 0x8c, 0xdc, 0x30, 0x96, 0x8e,
    0x55, 0x65, 0xfb, 0xe6, 0xd9, 0x43, 0x56, 0xd6, 0xc2, 0xaf, 0x62, 0x6b, 0x99, 0x45, 0x0d, 0x43,
];

fn is_canonical_jindo_field_element(element: &PrivacyJindoFieldElementV1) -> bool {
    for index in (0..IROHA_JINDO_FIELD_ELEMENT_BYTES_V1).rev() {
        if element.encoding[index] != IROHA_JINDO_FIELD_MODULUS_LE_V1[index] {
            return element.encoding[index] < IROHA_JINDO_FIELD_MODULUS_LE_V1[index];
        }
    }
    false
}

fn validate_bootle_lantern(
    statement: &IrohaBootleLanternAnoncredStatementV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.issuer_id.is_zero(), PrivacyTypedFieldV1::IssuerId)?;
    require_nonzero_id(statement.policy_id.is_zero(), PrivacyTypedFieldV1::PolicyId)?;
    require_epoch(
        statement.issuer_policy_epoch,
        PrivacyEpochFieldV1::IssuerPolicy,
    )?;
    require_nonzero_id(
        statement.issuer_policy_record_digest.is_zero(),
        PrivacyTypedFieldV1::IssuerPolicyRecordDigest,
    )?;
    require_nonzero_id(
        statement.issuer_parameter_id.is_zero(),
        PrivacyTypedFieldV1::IssuerParameterId,
    )?;
    require_nonzero_id(
        statement.issuer_parameter_digest.is_zero(),
        PrivacyTypedFieldV1::IssuerParameterDigest,
    )?;
    let disclosed_count = u32::try_from(statement.disclosures.len())
        .map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)?;
    if disclosed_count > BOOTLE_LANTERN_MAX_DISCLOSED_ATTRIBUTES_V1 {
        return Err(
            PrivacyStatementValidationError::TooManyBootleLanternDisclosures {
                count: disclosed_count,
                max: BOOTLE_LANTERN_MAX_DISCLOSED_ATTRIBUTES_V1,
            },
        );
    }
    let mut previous = None;
    for disclosure in &statement.disclosures {
        if usize::from(disclosure.index) >= BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1 {
            return Err(
                PrivacyStatementValidationError::BootleLanternDisclosureIndexOutOfBounds {
                    index: disclosure.index,
                },
            );
        }
        if previous.is_some_and(|value| disclosure.index <= value) {
            return Err(
                PrivacyStatementValidationError::BootleLanternDisclosuresNotStrictlyIncreasing,
            );
        }
        previous = Some(disclosure.index);
    }
    Ok(())
}

fn validate_orchard(
    statement: &OrchardHalo2ActionsStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    require_nonzero_id(statement.anchor.is_zero(), PrivacyTypedFieldV1::Root)?;
    require_epoch(statement.anchor_epoch, PrivacyEpochFieldV1::Root)?;
    require_epoch(statement.expiry_height, PrivacyEpochFieldV1::ExpiryHeight)?;
    statement.value_balance.validate()?;
    if statement.value_balance.amount > ORCHARD_MAX_VALUE_BALANCE_V1 {
        return Err(
            PrivacyStatementValidationError::OrchardValueBalanceOutOfRange {
                amount: statement.value_balance.amount,
                max: ORCHARD_MAX_VALUE_BALANCE_V1,
            },
        );
    }
    let max = ORCHARD_MAX_ACTIONS_V1
        .min(limits.max_nullifiers_per_action)
        .min(limits.max_commitments_per_action);
    let count = u32_len(statement.actions.len())?;
    if count == 0 || count > max {
        return Err(PrivacyStatementValidationError::InvalidOrchardActionCount { count, max });
    }
    for (index, action) in statement.actions.iter().enumerate() {
        let index = u32_index(index)?;
        let encrypted_note_bytes = u32_len(action.encrypted_note.len())?;
        if action.encrypted_note.len() != ORCHARD_ENCRYPTED_NOTE_BYTES_V1 {
            return Err(
                PrivacyStatementValidationError::InvalidOrchardEncryptedNoteSize {
                    index,
                    bytes: encrypted_note_bytes,
                    expected: u32::try_from(ORCHARD_ENCRYPTED_NOTE_BYTES_V1)
                        .expect("compiled Orchard ciphertext width fits u32"),
                },
            );
        }
        let outgoing_ciphertext_bytes = u32_len(action.outgoing_ciphertext.len())?;
        if action.outgoing_ciphertext.len() != ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1 {
            return Err(
                PrivacyStatementValidationError::InvalidOrchardOutgoingCiphertextSize {
                    index,
                    bytes: outgoing_ciphertext_bytes,
                    expected: u32::try_from(ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1)
                        .expect("compiled Orchard ciphertext width fits u32"),
                },
            );
        }
        if statement.actions[..usize::try_from(index).expect("u32 index fits usize")]
            .iter()
            .any(|earlier| earlier.nullifier == action.nullifier)
        {
            return Err(PrivacyStatementValidationError::DuplicateOrchardNullifier { index });
        }
        if statement.actions[..usize::try_from(index).expect("u32 index fits usize")]
            .iter()
            .any(|earlier| earlier.note_commitment == action.note_commitment)
        {
            return Err(PrivacyStatementValidationError::DuplicateOrchardNoteCommitment { index });
        }
    }
    Ok(())
}

fn validate_fcmp(
    statement: &MoneroFcmpPlusPlusStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    statement
        .output_set_root
        .validate()
        .map_err(PrivacyStatementValidationError::InvalidFcmpTreeRoot)?;
    require_epoch(statement.root_epoch, PrivacyEpochFieldV1::Root)?;
    let max_inputs = FCMP_MAX_INPUTS_V1
        .min(limits.max_commitments_per_action)
        .min(limits.max_nullifiers_per_action);
    let input_count = u32_len(statement.inputs.len())?;
    if input_count == 0 || input_count > max_inputs {
        return Err(PrivacyStatementValidationError::InvalidFcmpInputCount {
            count: input_count,
            max: max_inputs,
        });
    }
    for (index, input) in statement.inputs.iter().copied().enumerate() {
        let canonical_index = u32_index(index)?;
        input.validate_nonzero().map_err(|source| {
            PrivacyStatementValidationError::InvalidFcmpInput {
                index: canonical_index,
                source,
            }
        })?;
        if statement.inputs[..index]
            .iter()
            .any(|earlier| earlier.key_image == input.key_image)
        {
            return Err(PrivacyStatementValidationError::DuplicateFcmpKeyImage {
                index: canonical_index,
            });
        }
        if statement.inputs[..index]
            .iter()
            .any(|earlier| earlier.pseudo_out == input.pseudo_out)
        {
            return Err(PrivacyStatementValidationError::DuplicateFcmpPseudoOut {
                index: canonical_index,
            });
        }
    }

    let max_outputs = FCMP_MAX_OUTPUTS_V1.min(limits.max_commitments_per_action);
    let output_count = u32_len(statement.outputs.len())?;
    if output_count == 0 || output_count > max_outputs {
        return Err(PrivacyStatementValidationError::InvalidFcmpOutputCount {
            count: output_count,
            max: max_outputs,
        });
    }
    for (index, output) in statement.outputs.iter().copied().enumerate() {
        let canonical_index = u32_index(index)?;
        output.validate_nonzero().map_err(|source| {
            PrivacyStatementValidationError::InvalidFcmpOutput {
                index: canonical_index,
                source,
            }
        })?;
        let output_id = output.output_id();
        if statement.outputs[..index]
            .iter()
            .copied()
            .any(|earlier| earlier.output_id() == output_id)
        {
            return Err(PrivacyStatementValidationError::DuplicateFcmpOutputId {
                index: canonical_index,
            });
        }
    }
    validate_fcmp_encrypted_outputs(
        &statement.encrypted_outputs,
        &statement.outputs,
        max_outputs,
    )
}

fn validate_fcmp_encrypted_outputs(
    encrypted_outputs: &[PrivacyFcmpEncryptedOutputV1],
    outputs: &[PrivacyFcmpOutputTupleV1],
    max: u32,
) -> Result<(), PrivacyStatementValidationError> {
    if encrypted_outputs.is_empty() {
        return Err(PrivacyStatementValidationError::MissingEncryptedOutput);
    }
    let count = u32_len(encrypted_outputs.len())?;
    if count > max {
        return Err(PrivacyStatementValidationError::TooManyEncryptedOutputs { count, max });
    }
    if encrypted_outputs.len() != outputs.len() {
        return Err(
            PrivacyStatementValidationError::FcmpEncryptedOutputCountMismatch {
                encrypted_outputs: count,
                outputs: u32_len(outputs.len())?,
            },
        );
    }
    for (index, (encrypted_output, output)) in encrypted_outputs.iter().zip(outputs).enumerate() {
        let index = u32_index(index)?;
        if encrypted_output.recipient.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroEncryptedOutputRecipient { index });
        }
        if encrypted_output.ephemeral_public_key.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroEncryptedOutputEphemeralKey { index });
        }
        if encrypted_output.output_id != output.output_id() {
            return Err(PrivacyStatementValidationError::FcmpEncryptedOutputIdMismatch { index });
        }
        if encrypted_output.ciphertext.len() != PRIVACY_FCMP_ENCRYPTED_OUTPUT_BYTES_V1
            || encrypted_output.ciphertext.get(..4)
                != Some(PRIVACY_FCMP_ENCRYPTED_OUTPUT_MAGIC_V1.as_slice())
            || encrypted_output.ciphertext[4..4 + PRIVACY_FCMP_ENCRYPTED_OUTPUT_NONCE_BYTES_V1]
                .iter()
                .all(|byte| *byte == 0)
            || encrypted_output.ciphertext[4 + PRIVACY_FCMP_ENCRYPTED_OUTPUT_NONCE_BYTES_V1..]
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(PrivacyStatementValidationError::InvalidFcmpEncryptedOutputCodec { index });
        }
    }
    Ok(())
}

fn validate_ivm_private_note(
    statement: &IrohaIvmPrivateNoteStarkStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    require_nonzero_id(
        statement.program_id.is_zero(),
        PrivacyTypedFieldV1::ProgramId,
    )?;
    require_nonzero_id(
        statement.action_digest.is_zero(),
        PrivacyTypedFieldV1::ActionDigest,
    )?;
    let computed_action_digest = statement
        .computed_action_digest()
        .map_err(|_| PrivacyStatementValidationError::ActionDigestEncodingFailed)?;
    if statement.action_digest != computed_action_digest {
        return Err(PrivacyStatementValidationError::ActionDigestMismatch);
    }
    require_nonzero_id(statement.state_root.is_zero(), PrivacyTypedFieldV1::Root)?;
    require_epoch(statement.root_epoch, PrivacyEpochFieldV1::Root)?;
    require_epoch(statement.execution_epoch, PrivacyEpochFieldV1::Execution)?;
    if statement.execution_epoch != statement.root_epoch {
        return Err(PrivacyStatementValidationError::EpochBindingMismatch {
            field: PrivacyEpochFieldV1::Execution,
            root_epoch: statement.root_epoch,
            bound_epoch: statement.execution_epoch,
        });
    }
    statement.value_balance.validate()?;
    validate_nullifiers_with_max(
        &statement.nullifiers,
        true,
        IVM_PRIVATE_NOTE_MAX_INPUTS_V1.min(limits.max_nullifiers_per_action),
    )?;
    validate_commitments_with_max(
        &statement.output_commitments,
        true,
        IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1.min(limits.max_commitments_per_action),
    )?;
    validate_encrypted_outputs(
        &statement.encrypted_outputs,
        &statement.output_commitments,
        true,
        limits,
    )?;
    validate_ivm_private_encrypted_outputs(&statement.encrypted_outputs)
}

fn validate_pq_masp(
    statement: &PqMaspStarkStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    require_nonzero_id(statement.anchor.is_zero(), PrivacyTypedFieldV1::Root)?;
    require_epoch(statement.anchor_epoch, PrivacyEpochFieldV1::Root)?;
    require_epoch(
        statement.authorization_epoch,
        PrivacyEpochFieldV1::Authorization,
    )?;
    if statement.authorization_epoch != statement.anchor_epoch {
        return Err(PrivacyStatementValidationError::EpochBindingMismatch {
            field: PrivacyEpochFieldV1::Authorization,
            root_epoch: statement.anchor_epoch,
            bound_epoch: statement.authorization_epoch,
        });
    }
    require_nonzero_id(
        statement.authorization_key_digest.is_zero(),
        PrivacyTypedFieldV1::AuthorizationKeyDigest,
    )?;
    require_nonzero_id(
        statement.note_encryption_key_digest.is_zero(),
        PrivacyTypedFieldV1::NoteEncryptionKeyDigest,
    )?;
    validate_nullifiers_with_max(
        &statement.nullifiers,
        true,
        PQ_MASP_MAX_INPUTS_V1.min(limits.max_nullifiers_per_action),
    )?;
    validate_commitments_with_max(
        &statement.output_commitments,
        true,
        PQ_MASP_MAX_OUTPUTS_V1.min(limits.max_commitments_per_action),
    )?;
    validate_encrypted_outputs(
        &statement.encrypted_outputs,
        &statement.output_commitments,
        true,
        limits,
    )
}

fn validate_nullifiers_with_max(
    values: &[PrivacyNullifierV1],
    require_nonempty: bool,
    max: u32,
) -> Result<(), PrivacyStatementValidationError> {
    if require_nonempty && values.is_empty() {
        return Err(PrivacyStatementValidationError::MissingNullifier);
    }
    let count = u32_len(values.len())?;
    if count > max {
        return Err(PrivacyStatementValidationError::TooManyNullifiers { count, max });
    }
    for (index, value) in values.iter().copied().enumerate() {
        require_nullifier(value, u32_index(index)?)?;
    }
    if first_duplicate_index(values).is_some() {
        return Err(PrivacyStatementValidationError::DuplicateNullifier);
    }
    Ok(())
}

fn validate_commitments_with_max(
    values: &[PrivacyCommitmentV1],
    require_nonempty: bool,
    max: u32,
) -> Result<(), PrivacyStatementValidationError> {
    if require_nonempty && values.is_empty() {
        return Err(PrivacyStatementValidationError::MissingCommitment);
    }
    let count = u32_len(values.len())?;
    if count > max {
        return Err(PrivacyStatementValidationError::TooManyCommitments { count, max });
    }
    for (index, value) in values.iter().copied().enumerate() {
        require_commitment(value, u32_index(index)?)?;
    }
    if first_duplicate_index(values).is_some() {
        return Err(PrivacyStatementValidationError::DuplicateCommitment);
    }
    Ok(())
}

fn validate_encrypted_outputs(
    outputs: &[PrivacyEncryptedOutputV1],
    commitments: &[PrivacyCommitmentV1],
    require_nonempty: bool,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    if require_nonempty && outputs.is_empty() {
        return Err(PrivacyStatementValidationError::MissingEncryptedOutput);
    }
    let count = u32_len(outputs.len())?;
    if count > limits.max_commitments_per_action {
        return Err(PrivacyStatementValidationError::TooManyEncryptedOutputs {
            count,
            max: limits.max_commitments_per_action,
        });
    }
    if outputs.len() != commitments.len() {
        return Err(
            PrivacyStatementValidationError::EncryptedOutputCommitmentCountMismatch {
                outputs: count,
                commitments: u32_len(commitments.len())?,
            },
        );
    }
    for (index, (output, expected_commitment)) in outputs.iter().zip(commitments).enumerate() {
        let index = u32_index(index)?;
        if output.recipient.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroEncryptedOutputRecipient { index });
        }
        if output.ephemeral_public_key.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroEncryptedOutputEphemeralKey { index });
        }
        if output.commitment.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroCommitment { index });
        }
        if output.commitment != *expected_commitment {
            return Err(
                PrivacyStatementValidationError::EncryptedOutputCommitmentMismatch { index },
            );
        }
        if output.ciphertext.is_empty() {
            return Err(PrivacyStatementValidationError::EmptyEncryptedOutput { index });
        }
        if output.ciphertext.iter().all(|byte| *byte == 0) {
            return Err(PrivacyStatementValidationError::AllZeroEncryptedOutput { index });
        }
    }
    Ok(())
}

fn validate_ivm_private_encrypted_outputs(
    outputs: &[PrivacyEncryptedOutputV1],
) -> Result<(), PrivacyStatementValidationError> {
    for (index, output) in outputs.iter().enumerate() {
        let nonce_end = 4 + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1;
        if output.ciphertext.len() != PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1
            || output.ciphertext.get(..4)
                != Some(PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1.as_slice())
            || output.ciphertext[4..nonce_end]
                .iter()
                .all(|byte| *byte == 0)
            || output.ciphertext[nonce_end..].iter().all(|byte| *byte == 0)
        {
            return Err(
                PrivacyStatementValidationError::InvalidIvmPrivateEncryptedOutputCodec {
                    index: u32_index(index)?,
                },
            );
        }
    }
    Ok(())
}

fn require_nonzero_id(
    is_zero: bool,
    field: PrivacyTypedFieldV1,
) -> Result<(), PrivacyStatementValidationError> {
    if is_zero {
        return Err(PrivacyStatementValidationError::ZeroTypedField { field });
    }
    Ok(())
}

fn require_epoch(
    epoch: u64,
    field: PrivacyEpochFieldV1,
) -> Result<(), PrivacyStatementValidationError> {
    if epoch == 0 {
        return Err(PrivacyStatementValidationError::ZeroEpoch { field });
    }
    Ok(())
}

fn validate_next_root_transition(
    current_root: PrivacyRootV1,
    current_epoch: u64,
    next_root: PrivacyRootV1,
    next_epoch: u64,
    field: PrivacyRootTransitionFieldV1,
) -> Result<(), PrivacyStatementValidationError> {
    if next_root.is_zero() {
        return Err(PrivacyStatementValidationError::ZeroNextRoot { field });
    }
    if next_root == current_root {
        return Err(PrivacyStatementValidationError::UnchangedRootTransition { field });
    }
    if current_epoch.checked_add(1) != Some(next_epoch) {
        return Err(PrivacyStatementValidationError::InvalidNextRootEpoch {
            field,
            current_epoch,
            next_epoch,
        });
    }
    Ok(())
}

fn require_nullifier(
    value: PrivacyNullifierV1,
    index: u32,
) -> Result<(), PrivacyStatementValidationError> {
    if value.is_zero() {
        return Err(PrivacyStatementValidationError::ZeroNullifier { index });
    }
    Ok(())
}

fn require_commitment(
    value: PrivacyCommitmentV1,
    index: u32,
) -> Result<(), PrivacyStatementValidationError> {
    if value.is_zero() {
        return Err(PrivacyStatementValidationError::ZeroCommitment { index });
    }
    Ok(())
}

fn require_count(
    actual: usize,
    declared: u32,
    field: PrivacyCountFieldV1,
) -> Result<(), PrivacyStatementValidationError> {
    let actual = u32_len(actual)?;
    if actual != declared {
        return Err(PrivacyStatementValidationError::DeclaredCountMismatch {
            field,
            declared,
            actual,
        });
    }
    Ok(())
}

fn u32_len(len: usize) -> Result<u32, PrivacyStatementValidationError> {
    u32::try_from(len).map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)
}

fn u32_index(index: usize) -> Result<u32, PrivacyStatementValidationError> {
    u32_len(index)
}

fn first_duplicate_index<T: PartialEq>(values: &[T]) -> Option<usize> {
    for later in 1..values.len() {
        if values[..later].contains(&values[later]) {
            return Some(later);
        }
    }
    None
}

impl PrivacyProtocolActivationLimitsV1 {
    /// Validate a statement against activation-specific governed count limits.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyActivationStatementLimitsError`] if the statement uses
    /// another protocol or exceeds an activation-specific bound.
    pub fn validate_statement(
        &self,
        statement: &PrivacyStatementV1,
    ) -> Result<(), PrivacyActivationStatementLimitsError> {
        if self.protocol_id() != statement.protocol_id() {
            return Err(PrivacyActivationStatementLimitsError::ProtocolMismatch {
                limits_protocol: self.protocol_id(),
                statement_protocol: statement.protocol_id(),
            });
        }
        match (self, statement) {
            (
                Self::AnonymousPgcKOutOfNV1(limits),
                PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement),
            ) => {
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::AnonymousPgcAnonymitySetSize,
                    statement.anonymity_set_public_keys.len(),
                    limits.max_anonymity_set_size,
                )?;
                validate_activation_statement_count(
                    PrivacyActivationLimitFieldV1::AnonymousPgcRecipientCount,
                    statement.recipient_count,
                    limits.max_recipient_count,
                )
            }
            (
                Self::VeRangeTransparentRangeV1(limits),
                PrivacyStatementV1::VeRangeTransparentRangeV1(statement),
            ) => validate_activation_statement_count(
                PrivacyActivationLimitFieldV1::VeRangeAggregationCount,
                statement.aggregation_count,
                limits.max_aggregation_count,
            ),
            (Self::IrohaZkAmsV1(limits), PrivacyStatementV1::IrohaZkAmsV1(statement)) => {
                match &statement.action {
                    PrivacyZkAmsActionV1::BatchAdmission(batch) => {
                        validate_activation_statement_len(
                            PrivacyActivationLimitFieldV1::ZkAmsBatchSize,
                            batch.anchors.len(),
                            limits.max_batch_size,
                        )
                    }
                    PrivacyZkAmsActionV1::ProvisionAccount(provision) => {
                        validate_activation_statement_len(
                            PrivacyActivationLimitFieldV1::ZkAmsRingSize,
                            provision.admitted_seed_key_ring.len(),
                            limits.max_ring_size,
                        )
                    }
                }
            }
            (
                Self::IrohaJindoPolynomialCommitmentV0(limits),
                PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement),
            ) => validate_activation_statement_count(
                PrivacyActivationLimitFieldV1::JindoPolynomialCount,
                u32::try_from(statement.polynomial_commitments.len()).unwrap_or(u32::MAX),
                limits.max_polynomial_count,
            ),
            (
                Self::OrchardHalo2ActionsV1(limits),
                PrivacyStatementV1::OrchardHalo2ActionsV1(statement),
            ) => validate_activation_statement_len(
                PrivacyActivationLimitFieldV1::OrchardActionCount,
                statement.actions.len(),
                limits.max_action_count,
            ),
            (
                Self::MoneroFcmpPlusPlusV1(limits),
                PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement),
            ) => {
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::FcmpInputCount,
                    statement.inputs.len(),
                    limits.max_input_count,
                )?;
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::FcmpOutputCount,
                    statement.outputs.len(),
                    limits.max_output_count,
                )
            }
            (
                Self::IrohaIvmPrivateNoteStarkV1(limits),
                PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement),
            ) => {
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteInputCount,
                    statement.nullifiers.len(),
                    limits.max_input_count,
                )?;
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::IvmPrivateNoteOutputCount,
                    statement.output_commitments.len(),
                    limits.max_output_count,
                )
            }
            (Self::PqMaspStarkV0(limits), PrivacyStatementV1::PqMaspStarkV0(statement)) => {
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::PqMaspInputCount,
                    statement.nullifiers.len(),
                    limits.max_input_count,
                )?;
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::PqMaspOutputCount,
                    statement.output_commitments.len(),
                    limits.max_output_count,
                )
            }
            _ => Ok(()),
        }
    }
}

fn validate_activation_statement_count(
    field: PrivacyActivationLimitFieldV1,
    count: u32,
    max: u32,
) -> Result<(), PrivacyActivationStatementLimitsError> {
    if count > max {
        return Err(PrivacyActivationStatementLimitsError::CountExceeds { field, count, max });
    }
    Ok(())
}

fn validate_activation_statement_len(
    field: PrivacyActivationLimitFieldV1,
    count: usize,
    max: u32,
) -> Result<(), PrivacyActivationStatementLimitsError> {
    if count > max as usize {
        return Err(PrivacyActivationStatementLimitsError::CountExceeds {
            field,
            count: u32::try_from(count).unwrap_or(u32::MAX),
            max,
        });
    }
    Ok(())
}

/// Statement failure against activation-specific governed limits.
#[allow(variant_size_differences)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivacyActivationStatementLimitsError {
    /// Statement and activation-limit protocol tags differ.
    #[error(
        "privacy activation-limit protocol {limits_protocol:?} differs from statement protocol {statement_protocol:?}"
    )]
    ProtocolMismatch {
        /// Protocol encoded by the limits.
        limits_protocol: PrivacyProtocolIdV1,
        /// Protocol encoded by the statement.
        statement_protocol: PrivacyProtocolIdV1,
    },
    /// A statement count exceeds the activation-specific maximum.
    #[error("privacy statement count {field:?} value {count} exceeds active maximum {max}")]
    CountExceeds {
        /// Count field.
        field: PrivacyActivationLimitFieldV1,
        /// Statement count.
        count: u32,
        /// Active governed maximum.
        max: u32,
    },
}

/// Validated raw proof payload for a protocol-specific proof variant.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct PrivacyProofBytesV1 {
    /// Exact native proof encoding.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub bytes: Vec<u8>,
}

impl PrivacyProofBytesV1 {
    /// Construct a proof payload for subsequent validation.
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Borrow the exact native proof bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Validate proof presence, non-degeneracy, and the configured byte bound.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyProofValidationError`] if the configured limits are
    /// invalid or the proof is empty, all zero, or too large.
    pub fn validate(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyProofValidationError> {
        limits
            .validate()
            .map_err(PrivacyProofValidationError::InvalidLimits)?;
        if self.bytes.is_empty() {
            return Err(PrivacyProofValidationError::Empty);
        }
        if self.bytes.iter().all(|byte| *byte == 0) {
            return Err(PrivacyProofValidationError::AllZero);
        }
        let len = u64::try_from(self.bytes.len())
            .map_err(|_| PrivacyProofValidationError::LengthOverflow)?;
        if len > u64::from(limits.max_proof_bytes_per_action) {
            return Err(PrivacyProofValidationError::TooLarge {
                bytes: len,
                max: limits.max_proof_bytes_per_action,
            });
        }
        Ok(())
    }
}

/// Action-typed native ZK-AMS proof.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "action", content = "proof", deny_unknown_fields)
)]
pub enum IrohaZkAmsProofV1 {
    /// Setup-free masked Relaxed Spartan batch-admission proof.
    MaskedRelaxedSpartanBatchAdmission(PrivacyProofBytesV1),
    /// Canonical one-layer MLSAGS/LSAG Ristretto255 provisioning signature.
    Ristretto255LsagProvisionAccount(PrivacyProofBytesV1),
}

impl IrohaZkAmsProofV1 {
    /// Borrow the exact native proof or signature bytes.
    #[must_use]
    pub const fn bytes(&self) -> &PrivacyProofBytesV1 {
        match self {
            Self::MaskedRelaxedSpartanBatchAdmission(bytes)
            | Self::Ristretto255LsagProvisionAccount(bytes) => bytes,
        }
    }

    /// Mutably borrow the exact native proof or signature bytes.
    #[must_use]
    pub const fn bytes_mut(&mut self) -> &mut PrivacyProofBytesV1 {
        match self {
            Self::MaskedRelaxedSpartanBatchAdmission(bytes)
            | Self::Ristretto255LsagProvisionAccount(bytes) => bytes,
        }
    }

    /// Return whether this proof variant matches a typed public action.
    #[must_use]
    pub const fn matches_action(&self, action: &PrivacyZkAmsActionV1) -> bool {
        matches!(
            (self, action),
            (
                Self::MaskedRelaxedSpartanBatchAdmission(_),
                PrivacyZkAmsActionV1::BatchAdmission(_)
            ) | (
                Self::Ristretto255LsagProvisionAccount(_),
                PrivacyZkAmsActionV1::ProvisionAccount(_)
            )
        )
    }
}

/// Protocol-typed native proof payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.proof.v1")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "protocol", content = "proof", deny_unknown_fields)
)]
pub enum PrivacyProofV1 {
    /// ZK-ACE post-quantum authorization proof.
    ZkAcePqAuthorizationV0(PrivacyProofBytesV1),
    /// Anonymous PGC k-out-of-n payment proof.
    AnonymousPgcKOutOfNV1(PrivacyProofBytesV1),
    /// VeRange transparent range proof.
    VeRangeTransparentRangeV1(PrivacyProofBytesV1),
    /// Native Iroha ZK-AMS admission or provisioning proof.
    IrohaZkAmsV1(IrohaZkAmsProofV1),
    /// Vega existing-credential predicate proof.
    VegaExistingCredentialZkV0(PrivacyProofBytesV1),
    /// Native Iroha P-256 X.509 predicate STARK proof.
    IrohaZkX509StarkP256V0(PrivacyProofBytesV1),
    /// Native Iroha Jindo batched univariate lattice polynomial-commitment proof.
    IrohaJindoPolynomialCommitmentV0(PrivacyProofBytesV1),
    /// Native Bootle Lantern/LNP22 anonymous-credential proof.
    IrohaBootleLanternAnoncredV1(PrivacyProofBytesV1),
    /// Orchard Halo2 action proof.
    OrchardHalo2ActionsV1(PrivacyProofBytesV1),
    /// Monero FCMP++ membership proof.
    MoneroFcmpPlusPlusV1(PrivacyProofBytesV1),
    /// Native IVM private-note STARK proof.
    IrohaIvmPrivateNoteStarkV1(PrivacyProofBytesV1),
    /// Post-quantum MASP STARK proof.
    PqMaspStarkV0(PrivacyProofBytesV1),
}

impl PrivacyProofV1 {
    /// Exact protocol carried by this proof variant.
    #[must_use]
    pub const fn protocol_id(&self) -> PrivacyProtocolIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV0(_) => PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            Self::AnonymousPgcKOutOfNV1(_) => PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            Self::VeRangeTransparentRangeV1(_) => PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
            Self::IrohaZkAmsV1(_) => PrivacyProtocolIdV1::IrohaZkAmsV1,
            Self::VegaExistingCredentialZkV0(_) => PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            Self::IrohaZkX509StarkP256V0(_) => PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            Self::IrohaJindoPolynomialCommitmentV0(_) => {
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0
            }
            Self::IrohaBootleLanternAnoncredV1(_) => {
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1
            }
            Self::OrchardHalo2ActionsV1(_) => PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            Self::MoneroFcmpPlusPlusV1(_) => PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            Self::IrohaIvmPrivateNoteStarkV1(_) => PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            Self::PqMaspStarkV0(_) => PrivacyProtocolIdV1::PqMaspStarkV0,
        }
    }

    /// Borrow the protocol-specific native proof payload.
    #[must_use]
    pub const fn bytes(&self) -> &PrivacyProofBytesV1 {
        match self {
            Self::ZkAcePqAuthorizationV0(bytes)
            | Self::AnonymousPgcKOutOfNV1(bytes)
            | Self::VeRangeTransparentRangeV1(bytes)
            | Self::VegaExistingCredentialZkV0(bytes)
            | Self::IrohaZkX509StarkP256V0(bytes)
            | Self::IrohaJindoPolynomialCommitmentV0(bytes)
            | Self::IrohaBootleLanternAnoncredV1(bytes)
            | Self::OrchardHalo2ActionsV1(bytes)
            | Self::MoneroFcmpPlusPlusV1(bytes)
            | Self::IrohaIvmPrivateNoteStarkV1(bytes)
            | Self::PqMaspStarkV0(bytes) => bytes,
            Self::IrohaZkAmsV1(proof) => proof.bytes(),
        }
    }

    /// Mutably borrow the protocol-specific native proof payload.
    ///
    /// Transaction-intent normalization uses this exhaustive accessor to
    /// empty the sole typed proof byte vector without protocol-shape drift.
    #[must_use]
    pub const fn bytes_mut(&mut self) -> &mut PrivacyProofBytesV1 {
        match self {
            Self::ZkAcePqAuthorizationV0(bytes)
            | Self::AnonymousPgcKOutOfNV1(bytes)
            | Self::VeRangeTransparentRangeV1(bytes)
            | Self::VegaExistingCredentialZkV0(bytes)
            | Self::IrohaZkX509StarkP256V0(bytes)
            | Self::IrohaJindoPolynomialCommitmentV0(bytes)
            | Self::IrohaBootleLanternAnoncredV1(bytes)
            | Self::OrchardHalo2ActionsV1(bytes)
            | Self::MoneroFcmpPlusPlusV1(bytes)
            | Self::IrohaIvmPrivateNoteStarkV1(bytes)
            | Self::PqMaspStarkV0(bytes) => bytes,
            Self::IrohaZkAmsV1(proof) => proof.bytes_mut(),
        }
    }
}

/// Fixed typed field used by statement validation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyTypedFieldV1 {
    /// Privacy pool identifier.
    PoolId,
    /// ZK-AMS admitted-identity registry identifier.
    RegistryId,
    /// Governed policy identifier.
    PolicyId,
    /// Governed policy digest.
    PolicyDigest,
    /// Credential issuer identifier.
    IssuerId,
    /// Commitment or accumulator root.
    Root,
    /// Credential revocation root.
    RevocationRoot,
    /// Issuer parameter-set identifier.
    IssuerParameterId,
    /// Issuer parameter-set digest.
    IssuerParameterDigest,
    /// Digest of the committed Bootle/Lantern issuer-policy record.
    IssuerPolicyRecordDigest,
    /// Certificate subject-key digest.
    CertificateKeyDigest,
    /// Exact immutable X.509 trust-anchor record digest.
    X509TrustAnchorRecordDigest,
    /// Exact immutable X.509 certificate-policy record digest.
    X509CertificatePolicyRecordDigest,
    /// Exact immutable X.509 signed-CRL record digest.
    X509CrlRecordDigest,
    /// Exact immutable Vega issuer-key/policy record digest.
    VegaIssuerRecordDigest,
    /// Public Vega Figure 9 device-authentication digest `H_dev`.
    VegaDeviceAuthenticationDigest,
    /// ISO 18013-5 reader challenge.
    ReaderChallenge,
    /// ISO 18013-5 session transcript digest.
    SessionTranscriptDigest,
    /// Private IVM program identifier.
    ProgramId,
    /// Private IVM action digest.
    ActionDigest,
    /// Post-quantum authorization-key digest.
    AuthorizationKeyDigest,
    /// Post-quantum note-encryption-key digest.
    NoteEncryptionKeyDigest,
    /// Digest of the authoritative ZK-AMS issuer-policy record.
    ZkAmsIssuerPolicyRecordDigest,
    /// Digest of the authoritative ZK-AMS registry record.
    ZkAmsRegistryRecordDigest,
}

/// Epoch or height field used by statement validation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyEpochFieldV1 {
    /// Commitment-root epoch.
    Root,
    /// Immutable Vega issuer-key/policy revision epoch.
    VegaIssuerRecord,
    /// X.509 certificate-authority membership epoch.
    CertificateAuthorityMembership,
    /// Immutable X.509 trust-anchor revision epoch.
    X509TrustAnchorRecord,
    /// Immutable X.509 certificate-policy revision epoch.
    X509CertificatePolicyRecord,
    /// Immutable X.509 signed-CRL revision epoch.
    X509CrlRecord,
    /// Revocation-state epoch.
    Revocation,
    /// Committed issuer-policy record epoch.
    IssuerPolicy,
    /// Authorization epoch.
    Authorization,
    /// Private-program execution epoch.
    Execution,
    /// Credential validity start epoch.
    ValidityStart,
    /// Credential validity end epoch.
    ValidityEnd,
    /// Credential presentation or validation epoch.
    Presentation,
    /// Transaction expiry block height.
    ExpiryHeight,
}

/// Proof-managed root transition selected by validation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyRootTransitionFieldV1 {
    /// Mutable encrypted PGC account table.
    PgcAccountState,
    /// ZK-AMS admitted-identity registry.
    AccountRegistry,
    /// FCMP++ complete output set.
    OutputSet,
}

/// Twisted-ElGamal ciphertext component selected by validation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyP256CiphertextComponentV1 {
    /// Left ciphertext point `C_L`.
    Left,
    /// Right ciphertext point `C_R`.
    Right,
}

/// Declared protocol count field used by validation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyCountFieldV1 {
    /// VeRange aggregated commitments.
    AggregatedCommitments,
    /// Jindo claimed evaluations in polynomial-commitment order.
    JindoClaimedEvaluations,
}

/// Validation failure for a protocol-specific [`PrivacyStatementV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyStatementValidationError {
    /// Supplied consensus limits are invalid.
    #[error("privacy statement limits are invalid: {0}")]
    InvalidLimits(PrivacyConsensusLimitsValidationError),
    /// Chain id is empty or exceeds the native transcript bound.
    #[error("privacy statement chain id uses {bytes} UTF-8 bytes; expected 1..={max}")]
    InvalidChainIdLength {
        /// Observed UTF-8 byte length.
        bytes: u32,
        /// Maximum admitted UTF-8 byte length.
        max: u32,
    },
    /// Action index cannot occur under the transaction action limit.
    #[error("privacy statement action index {index} is outside 0..{max_actions}")]
    ActionIndexOutOfBounds {
        /// Encoded zero-based action index.
        index: u32,
        /// Maximum action count in one transaction.
        max_actions: u32,
    },
    /// Canonical transaction-intent projection digest is zero.
    #[error("privacy statement transaction-intent digest must be non-zero")]
    ZeroTransactionIntentDigest,
    /// Governed parameter-set identifier is zero.
    #[error("privacy statement parameter id must be non-zero")]
    ZeroParameterId,
    /// Governed parameter digest is zero.
    #[error("privacy statement parameter digest must be non-zero")]
    ZeroParameterDigest,
    /// Governed verifier digest is zero.
    #[error("privacy statement verifier digest must be non-zero")]
    ZeroVerifierDigest,
    /// Governed statement-schema digest is zero.
    #[error("privacy statement schema digest must be non-zero")]
    ZeroStatementSchemaDigest,
    /// Pinned engine-manifest digest is zero.
    #[error("privacy statement engine-manifest digest must be non-zero")]
    ZeroEngineManifestDigest,
    /// A protocol-specific fixed field is zero.
    #[error("privacy statement field {field:?} must be non-zero")]
    ZeroTypedField {
        /// Invalid field.
        field: PrivacyTypedFieldV1,
    },
    /// The private-IVM action projection could not be canonically encoded.
    #[error("private IVM action digest projection failed canonical encoding")]
    ActionDigestEncodingFailed,
    /// The supplied private-IVM action digest does not authenticate the exact
    /// canonical action projection.
    #[error("private IVM action digest does not match the canonical action projection")]
    ActionDigestMismatch,
    /// A protocol epoch or height is zero.
    #[error("privacy statement epoch or height {field:?} must be non-zero")]
    ZeroEpoch {
        /// Invalid epoch or height field.
        field: PrivacyEpochFieldV1,
    },
    /// A protocol-specific execution or authorization epoch differs from the
    /// authoritative root epoch consumed by the same action.
    #[error("privacy statement {field:?} epoch {bound_epoch} differs from root epoch {root_epoch}")]
    EpochBindingMismatch {
        /// Execution or authorization binding selected by the statement.
        field: PrivacyEpochFieldV1,
        /// Epoch of the authoritative root consumed by the action.
        root_epoch: u64,
        /// Observed execution or authorization epoch.
        bound_epoch: u64,
    },
    /// A proof-managed successor root is zero.
    #[error("privacy root transition {field:?} has a zero successor root")]
    ZeroNextRoot {
        /// Transition selected by the statement.
        field: PrivacyRootTransitionFieldV1,
    },
    /// A proof-managed successor root equals the current root.
    #[error("privacy root transition {field:?} must change the root")]
    UnchangedRootTransition {
        /// Transition selected by the statement.
        field: PrivacyRootTransitionFieldV1,
    },
    /// A proof-managed root epoch does not advance by exactly one.
    #[error(
        "privacy root transition {field:?} from epoch {current_epoch} has invalid successor epoch {next_epoch}"
    )]
    InvalidNextRootEpoch {
        /// Transition selected by the statement.
        field: PrivacyRootTransitionFieldV1,
        /// Current canonical epoch.
        current_epoch: u64,
        /// Claimed successor epoch.
        next_epoch: u64,
    },
    /// A transparent transfer amount is zero.
    #[error("privacy statement transfer amount must be non-zero")]
    ZeroAmount,
    /// Public value-balance direction and magnitude are inconsistent.
    #[error("privacy value balance {direction:?} has invalid amount {amount}")]
    InvalidValueBalance {
        /// Declared pool-relative direction.
        direction: PrivacyValueBalanceDirectionV1,
        /// Absolute atomic amount.
        amount: u128,
    },
    /// A required nullifier vector is empty.
    #[error("privacy statement requires at least one nullifier")]
    MissingNullifier,
    /// A required commitment vector is empty.
    #[error("privacy statement requires at least one commitment")]
    MissingCommitment,
    /// A required encrypted-output vector is empty.
    #[error("privacy statement requires at least one encrypted output")]
    MissingEncryptedOutput,
    /// One encrypted output is empty.
    #[error("privacy statement encrypted output {index} must not be empty")]
    EmptyEncryptedOutput {
        /// Zero-based output index.
        index: u32,
    },
    /// An encrypted output has a zero recipient identity.
    #[error("privacy statement encrypted output {index} has a zero recipient")]
    ZeroEncryptedOutputRecipient {
        /// Zero-based output index.
        index: u32,
    },
    /// An encrypted output has a zero ephemeral public key.
    #[error("privacy statement encrypted output {index} has a zero ephemeral key")]
    ZeroEncryptedOutputEphemeralKey {
        /// Zero-based output index.
        index: u32,
    },
    /// Encrypted-output and commitment vector lengths differ.
    #[error(
        "privacy statement encrypted output count {outputs} differs from commitment count {commitments}"
    )]
    EncryptedOutputCommitmentCountMismatch {
        /// Encrypted-output count.
        outputs: u32,
        /// Commitment count.
        commitments: u32,
    },
    /// An encrypted output carries a different commitment than its ordered public commitment.
    #[error("privacy statement encrypted output {index} commitment mismatch")]
    EncryptedOutputCommitmentMismatch {
        /// Zero-based output index.
        index: u32,
    },
    /// Encrypted-output count exceeds consensus limits.
    #[error("privacy statement encrypted output count {count} exceeds maximum {max}")]
    TooManyEncryptedOutputs {
        /// Observed encrypted-output count.
        count: u32,
        /// Configured maximum.
        max: u32,
    },
    /// One encrypted output is all zero.
    #[error("privacy statement encrypted output {index} must not be all zero")]
    AllZeroEncryptedOutput {
        /// Zero-based output index.
        index: u32,
    },
    /// An FCMP++ encrypted output does not use the sole fixed `IFCE` codec.
    #[error("FCMP++ encrypted output {index} has an invalid canonical codec shape")]
    InvalidFcmpEncryptedOutputCodec {
        /// Zero-based output index.
        index: u32,
    },
    /// A private-IVM encrypted output does not use the sole fixed `IPNE`
    /// codec.
    #[error("private-IVM encrypted output {index} has an invalid canonical codec shape")]
    InvalidIvmPrivateEncryptedOutputCodec {
        /// Zero-based output index.
        index: u32,
    },
    /// Nullifier count exceeds consensus limits.
    #[error("privacy statement nullifier count {count} exceeds maximum {max}")]
    TooManyNullifiers {
        /// Observed nullifier count.
        count: u32,
        /// Configured maximum.
        max: u32,
    },
    /// A nullifier is zero.
    #[error("privacy statement nullifier {index} must be non-zero")]
    ZeroNullifier {
        /// Zero-based nullifier index.
        index: u32,
    },
    /// Two nullifiers are equal.
    #[error("privacy statement contains a duplicate nullifier")]
    DuplicateNullifier,
    /// Commitment count exceeds consensus limits.
    #[error("privacy statement commitment count {count} exceeds maximum {max}")]
    TooManyCommitments {
        /// Observed commitment count.
        count: u32,
        /// Configured maximum.
        max: u32,
    },
    /// A commitment is zero.
    #[error("privacy statement commitment {index} must be non-zero")]
    ZeroCommitment {
        /// Zero-based commitment index.
        index: u32,
    },
    /// A compressed P-256 point is all zero.
    #[error("privacy statement P-256 point {index} must be non-zero")]
    ZeroP256Point {
        /// Zero-based point index.
        index: u32,
    },
    /// One twisted-ElGamal ciphertext point is all zero.
    #[error("privacy statement P-256 ciphertext {index} component {component:?} must be non-zero")]
    ZeroP256CiphertextPoint {
        /// Zero-based ciphertext index.
        index: u32,
        /// Invalid ciphertext component.
        component: PrivacyP256CiphertextComponentV1,
    },
    /// Two commitments are equal.
    #[error("privacy statement contains a duplicate commitment")]
    DuplicateCommitment,
    /// Anonymous PGC anonymity-set size is not one of the closed profile sizes.
    #[error("Anonymous PGC anonymity-set size {size} is not one of 16, 32, or 64")]
    InvalidPgcAnonymitySetSize {
        /// Observed anonymity-set size.
        size: u32,
    },
    /// Anonymous PGC public-key and transfer-ciphertext counts differ.
    #[error(
        "Anonymous PGC public-key count {public_keys} differs from ciphertext count {ciphertexts}"
    )]
    PgcPublicMemoCountMismatch {
        /// Ordered public-key count.
        public_keys: u32,
        /// Ordered transfer-ciphertext count.
        ciphertexts: u32,
    },
    /// Anonymous PGC public keys are duplicated or not canonically ordered.
    #[error("Anonymous PGC public keys must be strictly increasing")]
    PgcAnonymitySetNotStrictlyIncreasing,
    /// Anonymous PGC intended recipient count is outside the approved profile.
    #[error(
        "Anonymous PGC recipient count {count} is outside 1..={max} for anonymity-set size {anonymity_set_size}"
    )]
    InvalidPgcRecipientCount {
        /// Observed intended recipient count.
        count: u32,
        /// Derived anonymity-set size.
        anonymity_set_size: u32,
        /// Maximum admitted recipient count.
        max: u32,
    },
    /// VeRange aggregation count is outside the approved profile.
    #[error("VeRange aggregation count {count} is outside 1..={max}")]
    InvalidAggregationCount {
        /// Observed aggregation count.
        count: u32,
        /// Approved maximum.
        max: u32,
    },
    /// A declared protocol vector count differs from the encoded vector.
    #[error("privacy statement count {field:?} declares {declared}, encoded vector has {actual}")]
    DeclaredCountMismatch {
        /// Count field.
        field: PrivacyCountFieldV1,
        /// Declared count.
        declared: u32,
        /// Encoded vector count.
        actual: u32,
    },
    /// A batched protocol count is outside its approved profile.
    #[error("privacy statement batch size {count} is outside 1..={max}")]
    InvalidBatchSize {
        /// Observed batch count.
        count: u32,
        /// Approved maximum.
        max: u32,
    },
    /// A ZK-AMS admission anchor has a zero PHC hash.
    #[error("ZK-AMS admission anchor {index} has a zero PHC hash")]
    ZeroZkAmsPhcHash {
        /// Zero-based anchor index.
        index: u32,
    },
    /// A ZK-AMS admission anchor or provisioning ring has a zero seed key.
    #[error("ZK-AMS seed public key {index} must be non-zero")]
    ZeroZkAmsSeedPublicKey {
        /// Zero-based anchor or ring index.
        index: u32,
    },
    /// Two ZK-AMS batch anchors carry the same PHC hash.
    #[error("ZK-AMS batch contains a duplicate PHC hash")]
    DuplicateZkAmsPhcHash,
    /// Two ZK-AMS batch anchors carry the same seed public key.
    #[error("ZK-AMS batch contains a duplicate seed public key")]
    DuplicateZkAmsSeedPublicKey,
    /// A ZK-AMS provisioning ring size is outside the closed profile.
    #[error("ZK-AMS provisioning ring size {size} is not one of 16, 32, or 64")]
    InvalidZkAmsRingSize {
        /// Observed ring size.
        size: u32,
    },
    /// A ZK-AMS seed-key ring is duplicated or non-canonical.
    #[error("ZK-AMS seed public-key ring must be strictly increasing")]
    ZkAmsSeedKeyRingNotStrictlyIncreasing,
    /// A ZK-AMS MLSAGS key image is the zero sentinel.
    #[error("ZK-AMS MLSAGS key image must be non-zero")]
    ZeroZkAmsKeyImage,
    /// Vega public presentation year is outside the trusted UTC domain.
    #[error("Vega presentation year {year} is outside {min}..={max}")]
    InvalidVegaPresentationYear {
        /// Observed public UTC year.
        year: u16,
        /// Lowest admitted UTC year.
        min: u16,
        /// Highest admitted UTC year.
        max: u16,
    },
    /// Vega public presentation date is not a Gregorian calendar date.
    #[error("Vega presentation date {year:04}-{month:02}-{day:02} is invalid")]
    InvalidVegaPresentationDate {
        /// Observed public UTC year.
        year: u16,
        /// Observed one-based UTC month.
        month: u8,
        /// Observed one-based UTC day.
        day: u8,
    },
    /// Vega public minimum-age threshold is outside the closed policy domain.
    #[error("Vega minimum-age threshold {years} is outside {min}..={max}")]
    InvalidVegaAgeThreshold {
        /// Observed threshold in completed years.
        years: u8,
        /// Lowest admitted threshold.
        min: u8,
        /// Highest admitted threshold.
        max: u8,
    },
    /// X.509 key usage does not authorize a signature.
    #[error("X.509 statement requires the digitalSignature key-usage bit")]
    InvalidX509KeyUsage,
    /// X.509 extended-key-usage vector is empty.
    #[error("X.509 statement requires at least one extended key usage")]
    MissingX509ExtendedKeyUsage,
    /// X.509 extended-key-usage vector exceeds the closed profile.
    #[error("X.509 statement has {actual} extended key usages; maximum is {max}")]
    TooManyX509ExtendedKeyUsages {
        /// Rejected count.
        actual: usize,
        /// Closed maximum.
        max: usize,
    },
    /// X.509 extended-key-usage values contain duplicates or are out of order.
    #[error("X.509 extended key usages must be strictly increasing")]
    X509ExtendedKeyUsagesNotStrictlyIncreasing,
    /// X.509 selective-disclosure vector exceeds the closed profile.
    #[error("X.509 statement has {actual} disclosed attributes; maximum is {max}")]
    TooManyX509DisclosedAttributes {
        /// Rejected count.
        actual: usize,
        /// Closed maximum.
        max: usize,
    },
    /// A selective-disclosure index is outside the closed C/O/OU/CN set.
    #[error("X.509 disclosed attribute index {index} is unsupported")]
    UnsupportedX509DisclosedAttributeIndex {
        /// Rejected index.
        index: u8,
    },
    /// A public selective-disclosure digest is the all-zero sentinel.
    #[error("X.509 disclosed attribute {index} digest must be non-zero")]
    ZeroX509DisclosedAttributeDigest {
        /// Attribute index.
        index: u8,
    },
    /// Selective disclosures contain duplicate or reordered indices.
    #[error("X.509 disclosed attributes must be strictly increasing by index")]
    X509DisclosedAttributesNotStrictlyIncreasing,
    /// The public presentation window is empty, reversed, or too wide.
    #[error(
        "X.509 presentation window [{start}, {end}] must be non-empty and no wider than {max_seconds} seconds"
    )]
    InvalidX509PresentationWindow {
        /// Inclusive presentation start.
        start: u64,
        /// Inclusive presentation end.
        end: u64,
        /// Closed first-release width ceiling.
        max_seconds: u64,
    },
    /// The common Jindo evaluation point is not the canonical residue in `[0, p)`.
    #[error("Jindo evaluation point is not a canonical coefficient-field element")]
    NonCanonicalJindoEvaluationPoint,
    /// A claimed Jindo evaluation is not the canonical residue in `[0, p)`.
    #[error("Jindo claimed evaluation {index} is not a canonical coefficient-field element")]
    NonCanonicalJindoClaimedEvaluation {
        /// Zero-based claimed-evaluation index.
        index: u32,
    },
    /// A Jindo lattice commitment has the wrong fixed-profile width.
    #[error("Jindo lattice commitment {index} uses {bytes} bytes; expected exactly {expected}")]
    InvalidJindoLatticeCommitmentSize {
        /// Zero-based commitment index.
        index: u32,
        /// Observed byte width.
        bytes: u32,
        /// Exact fixed-profile byte width.
        expected: u32,
    },
    /// A Jindo lattice commitment is the all-zero sentinel.
    #[error("Jindo lattice commitment {index} must not be all zero")]
    AllZeroJindoLatticeCommitment {
        /// Zero-based polynomial-commitment index.
        index: u32,
    },
    /// Two Jindo lattice commitments are identical.
    #[error("Jindo polynomial commitments must be distinct")]
    DuplicateJindoLatticeCommitment,
    /// A rounded public Jindo commitment coefficient is outside the fixed bound.
    #[error(
        "Jindo commitment {commitment_index} coefficient {coefficient_index} is {value}; expected {min}..={max}"
    )]
    JindoCommitmentCoefficientOutOfRange {
        /// Zero-based commitment index.
        commitment_index: u32,
        /// Zero-based coefficient index in row-major order.
        coefficient_index: u32,
        /// Decoded signed little-endian coefficient.
        value: i32,
        /// Inclusive fixed lower bound.
        min: i32,
        /// Inclusive fixed upper bound.
        max: i32,
    },
    /// Bootle/Lantern disclosed attribute count exceeds its fixed profile.
    #[error("Bootle/Lantern disclosed attribute count {count} exceeds {max}")]
    TooManyBootleLanternDisclosures {
        /// Observed disclosure count.
        count: u32,
        /// Approved maximum.
        max: u32,
    },
    /// A Bootle/Lantern disclosure index is outside the fixed eight attributes.
    #[error("Bootle/Lantern disclosed attribute index {index} is outside 0..8")]
    BootleLanternDisclosureIndexOutOfBounds {
        /// Invalid disclosed index.
        index: u8,
    },
    /// Bootle/Lantern disclosure indices contain a duplicate or are out of order.
    #[error("Bootle/Lantern disclosed attribute indices must be strictly increasing")]
    BootleLanternDisclosuresNotStrictlyIncreasing,
    /// Orchard action count is empty or exceeds the compiled/consensus bound.
    #[error("Orchard action count {count} is outside 1..={max}")]
    InvalidOrchardActionCount {
        /// Observed action count.
        count: u32,
        /// Effective compiled and governed maximum.
        max: u32,
    },
    /// An Orchard encrypted-note ciphertext does not have the exact V3 width.
    #[error(
        "Orchard action {index} encrypted-note ciphertext uses {bytes} bytes; expected exactly {expected}"
    )]
    InvalidOrchardEncryptedNoteSize {
        /// Zero-based ordered action index.
        index: u32,
        /// Observed byte width.
        bytes: u32,
        /// Exact required byte width.
        expected: u32,
    },
    /// An Orchard outgoing ciphertext does not have the exact V3 width.
    #[error(
        "Orchard action {index} outgoing ciphertext uses {bytes} bytes; expected exactly {expected}"
    )]
    InvalidOrchardOutgoingCiphertextSize {
        /// Zero-based ordered action index.
        index: u32,
        /// Observed byte width.
        bytes: u32,
        /// Exact required byte width.
        expected: u32,
    },
    /// Two Orchard actions use the same nullifier.
    #[error("Orchard action {index} duplicates an earlier nullifier")]
    DuplicateOrchardNullifier {
        /// Zero-based duplicate action index.
        index: u32,
    },
    /// Two Orchard actions use the same note commitment.
    #[error("Orchard action {index} duplicates an earlier note commitment")]
    DuplicateOrchardNoteCommitment {
        /// Zero-based duplicate action index.
        index: u32,
    },
    /// Orchard value balance is outside the signed native API range.
    #[error("Orchard value balance magnitude {amount} exceeds {max}")]
    OrchardValueBalanceOutOfRange {
        /// Observed absolute magnitude.
        amount: u128,
        /// Exact inclusive maximum.
        max: u128,
    },
    /// The typed FCMP++ root has an invalid layer count or zero point.
    #[error("FCMP++ output-set root is structurally invalid: {0}")]
    InvalidFcmpTreeRoot(PrivacyFcmpTreeRootValidationErrorV1),
    /// FCMP++ public-input count is empty or exceeds the effective profile.
    #[error("FCMP++ input count {count} is outside 1..={max}")]
    InvalidFcmpInputCount {
        /// Observed input count.
        count: u32,
        /// Effective first-release maximum.
        max: u32,
    },
    /// One FCMP++ public input contains a structurally invalid point.
    #[error("FCMP++ public input {index} is invalid: {source}")]
    InvalidFcmpInput {
        /// Zero-based input index.
        index: u32,
        /// Exact structural input failure.
        source: PrivacyFcmpInputValidationErrorV1,
    },
    /// Two FCMP++ public inputs use the same key image.
    #[error("FCMP++ public input {index} duplicates an earlier key image")]
    DuplicateFcmpKeyImage {
        /// Zero-based duplicate input index.
        index: u32,
    },
    /// Two FCMP++ public inputs use the same pseudo output.
    #[error("FCMP++ public input {index} duplicates an earlier pseudo output")]
    DuplicateFcmpPseudoOut {
        /// Zero-based duplicate input index.
        index: u32,
    },
    /// FCMP++ output count is empty or exceeds the effective profile.
    #[error("FCMP++ output count {count} is outside 1..={max}")]
    InvalidFcmpOutputCount {
        /// Observed output count.
        count: u32,
        /// Effective first-release maximum.
        max: u32,
    },
    /// One FCMP++ output tuple contains a structurally invalid point.
    #[error("FCMP++ output tuple {index} is invalid: {source}")]
    InvalidFcmpOutput {
        /// Zero-based output index.
        index: u32,
        /// Exact structural tuple failure.
        source: PrivacyFcmpOutputTupleValidationErrorV1,
    },
    /// Two FCMP++ outputs derive the same ledger identifier.
    #[error("FCMP++ output tuple {index} duplicates an earlier output id")]
    DuplicateFcmpOutputId {
        /// Zero-based duplicate output index.
        index: u32,
    },
    /// FCMP++ encrypted-output and tuple counts differ.
    #[error(
        "FCMP++ encrypted output count {encrypted_outputs} differs from output tuple count {outputs}"
    )]
    FcmpEncryptedOutputCountMismatch {
        /// Encrypted-output count.
        encrypted_outputs: u32,
        /// Public output-tuple count.
        outputs: u32,
    },
    /// An FCMP++ encrypted output identifies a different ordered tuple.
    #[error("FCMP++ encrypted output {index} output id mismatch")]
    FcmpEncryptedOutputIdMismatch {
        /// Zero-based output index.
        index: u32,
    },
    /// Public statement and encrypted outputs exceed the transaction budget.
    #[error("privacy statement and encrypted outputs use {bytes} bytes, exceeding maximum {max}")]
    StatementAndEncryptedOutputsTooLarge {
        /// Observed payload bytes.
        bytes: u64,
        /// Configured maximum.
        max: u32,
    },
    /// Canonical statement encoding failed.
    #[error("privacy statement canonical encoding failed")]
    EncodingFailure,
    /// A platform collection length could not be represented canonically.
    #[error("privacy statement payload length overflow")]
    PayloadLengthOverflow,
}

/// Validation failure for [`PrivacyProofBytesV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyProofValidationError {
    /// Supplied consensus limits are invalid.
    #[error("privacy proof limits are invalid: {0}")]
    InvalidLimits(PrivacyConsensusLimitsValidationError),
    /// Proof payload is empty.
    #[error("privacy proof bytes must not be empty")]
    Empty,
    /// Proof payload is all zero.
    #[error("privacy proof bytes must not be all zero")]
    AllZero,
    /// Proof payload exceeds consensus limits.
    #[error("privacy proof uses {bytes} bytes, exceeding maximum {max}")]
    TooLarge {
        /// Observed proof bytes.
        bytes: u64,
        /// Configured maximum.
        max: u32,
    },
    /// A platform collection length could not be represented canonically.
    #[error("privacy proof length overflow")]
    LengthOverflow,
}

/// Complete protocol-bound privacy proof admission envelope.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[norito(schema_name = "iroha.privacy.proof-envelope.v1")]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct PrivacyProofEnvelopeV1 {
    /// Exact protocol identity.
    pub protocol_id: PrivacyProtocolIdV1,
    /// Exact proof-system profile.
    pub proof_system_id: PrivacyProofSystemIdV1,
    /// Exact native verifier engine.
    pub engine_id: PrivacyEngineIdV1,
    /// Governed public-parameter set identifier.
    pub parameter_id: PrivacyParameterIdV1,
    /// Governed public-parameter digest.
    pub parameter_digest: PrivacyParameterDigestV1,
    /// Governed verifier artifact digest.
    pub verifier_digest: PrivacyVerifierDigestV1,
    /// Governed public-statement schema digest.
    pub statement_schema_digest: PrivacyStatementSchemaDigestV1,
    /// Governed native engine-manifest digest.
    pub engine_manifest_digest: PrivacyEngineManifestDigestV1,
    /// Digest of the complete protocol-tagged statement.
    pub statement_digest: PrivacyStatementDigestV1,
    /// Protocol-typed public statement.
    pub statement: PrivacyStatementV1,
    /// Protocol-typed proof bytes.
    pub proof: PrivacyProofV1,
}

impl PrivacyProofEnvelopeV1 {
    /// Validate internal protocol bindings and resource limits.
    ///
    /// This validates only the envelope's intrinsic shape. Chain admission must
    /// additionally call [`Self::validate_against_activation`] to bind the
    /// envelope to the active governed artifacts and current block height.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyProofEnvelopeValidationError`] for any mismatch,
    /// degenerate digest or payload, statement digest tampering, encoding
    /// failure, or resource-bound violation.
    pub fn validate_with_limits(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyProofEnvelopeValidationError> {
        limits
            .validate()
            .map_err(PrivacyProofEnvelopeValidationError::InvalidLimits)?;
        let expected_proof_system = self.protocol_id.expected_proof_system();
        if self.proof_system_id != expected_proof_system {
            return Err(PrivacyProofEnvelopeValidationError::ProofSystemMismatch {
                expected: expected_proof_system,
                actual: self.proof_system_id,
            });
        }
        let expected_engine = self.protocol_id.expected_engine();
        if self.engine_id != expected_engine {
            return Err(PrivacyProofEnvelopeValidationError::EngineMismatch {
                expected: expected_engine,
                actual: self.engine_id,
            });
        }
        let statement_protocol = self.statement.protocol_id();
        if statement_protocol != self.protocol_id {
            return Err(
                PrivacyProofEnvelopeValidationError::StatementProtocolMismatch {
                    envelope: self.protocol_id,
                    statement: statement_protocol,
                },
            );
        }
        let proof_protocol = self.proof.protocol_id();
        if proof_protocol != self.protocol_id {
            return Err(PrivacyProofEnvelopeValidationError::ProofProtocolMismatch {
                envelope: self.protocol_id,
                proof: proof_protocol,
            });
        }
        if let (PrivacyStatementV1::IrohaZkAmsV1(statement), PrivacyProofV1::IrohaZkAmsV1(proof)) =
            (&self.statement, &self.proof)
            && !proof.matches_action(&statement.action)
        {
            return Err(PrivacyProofEnvelopeValidationError::ZkAmsActionProofMismatch);
        }
        if self.parameter_id.is_zero() {
            return Err(PrivacyProofEnvelopeValidationError::ZeroParameterId);
        }
        if self.parameter_digest.is_zero() {
            return Err(PrivacyProofEnvelopeValidationError::ZeroParameterDigest);
        }
        if self.verifier_digest.is_zero() {
            return Err(PrivacyProofEnvelopeValidationError::ZeroVerifierDigest);
        }
        if self.statement_schema_digest.is_zero() {
            return Err(PrivacyProofEnvelopeValidationError::ZeroStatementSchemaDigest);
        }
        if self.engine_manifest_digest.is_zero() {
            return Err(PrivacyProofEnvelopeValidationError::ZeroEngineManifestDigest);
        }
        if self.statement_digest.is_zero() {
            return Err(PrivacyProofEnvelopeValidationError::ZeroStatementDigest);
        }
        let context = self.statement.context();
        if context.parameter_id != self.parameter_id {
            return Err(PrivacyProofEnvelopeValidationError::StatementParameterIdMismatch);
        }
        if context.parameter_digest != self.parameter_digest {
            return Err(PrivacyProofEnvelopeValidationError::StatementParameterDigestMismatch);
        }
        if context.verifier_digest != self.verifier_digest {
            return Err(PrivacyProofEnvelopeValidationError::StatementVerifierDigestMismatch);
        }
        if context.statement_schema_digest != self.statement_schema_digest {
            return Err(PrivacyProofEnvelopeValidationError::StatementSchemaDigestMismatch);
        }
        if context.engine_manifest_digest != self.engine_manifest_digest {
            return Err(PrivacyProofEnvelopeValidationError::StatementEngineManifestDigestMismatch);
        }
        self.statement
            .validate(limits)
            .map_err(PrivacyProofEnvelopeValidationError::Statement)?;
        self.proof
            .bytes()
            .validate(limits)
            .map_err(PrivacyProofEnvelopeValidationError::Proof)?;

        let computed_statement_digest = self
            .statement
            .digest()
            .map_err(|_| PrivacyProofEnvelopeValidationError::EncodingFailure)?;
        if computed_statement_digest != self.statement_digest {
            return Err(
                PrivacyProofEnvelopeValidationError::StatementDigestMismatch {
                    expected: computed_statement_digest,
                    actual: self.statement_digest,
                },
            );
        }

        let encoded = norito::to_bytes(self)
            .map_err(|_| PrivacyProofEnvelopeValidationError::EncodingFailure)?;
        let encoded_len = u64::try_from(encoded.len())
            .map_err(|_| PrivacyProofEnvelopeValidationError::EncodedLengthOverflow)?;
        if encoded_len > u64::from(limits.max_action_bytes) {
            return Err(PrivacyProofEnvelopeValidationError::ActionTooLarge {
                bytes: encoded_len,
                max: limits.max_action_bytes,
            });
        }
        if encoded_len > u64::from(limits.max_privacy_bytes_per_transaction) {
            return Err(
                PrivacyProofEnvelopeValidationError::TransactionPrivacyPayloadTooLarge {
                    bytes: encoded_len,
                    max: limits.max_privacy_bytes_per_transaction,
                },
            );
        }
        Ok(())
    }

    /// Validate this envelope against an active governed protocol record.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyProofEnvelopeValidationError`] if the activation record
    /// is invalid or inactive at `current_height`, any governed identity or
    /// digest differs, or intrinsic envelope validation fails.
    pub fn validate_against_activation(
        &self,
        activation: &PrivacyProtocolActivationRecordV1,
        consensus_limits: &PrivacyConsensusLimitsV1,
        current_height: u64,
    ) -> Result<(), PrivacyProofEnvelopeValidationError> {
        activation
            .validate()
            .map_err(PrivacyProofEnvelopeValidationError::InvalidActivation)?;
        if activation.protocol_id != self.protocol_id {
            return Err(
                PrivacyProofEnvelopeValidationError::ActivationProtocolMismatch {
                    activation: activation.protocol_id,
                    envelope: self.protocol_id,
                },
            );
        }
        let PrivacyProtocolLifecycleV1::Active(active_state) = activation.lifecycle else {
            return Err(PrivacyProofEnvelopeValidationError::ActivationNotActive);
        };
        if current_height < active_state.state_since_height {
            return Err(
                PrivacyProofEnvelopeValidationError::ActivationNotEffective {
                    current_height,
                    effective_height: active_state.state_since_height,
                },
            );
        }
        if activation.proof_system_id != self.proof_system_id {
            return Err(
                PrivacyProofEnvelopeValidationError::ActivationProofSystemMismatch {
                    activation: activation.proof_system_id,
                    envelope: self.proof_system_id,
                },
            );
        }
        if activation.engine_id != self.engine_id {
            return Err(
                PrivacyProofEnvelopeValidationError::ActivationEngineMismatch {
                    activation: activation.engine_id,
                    envelope: self.engine_id,
                },
            );
        }
        if activation.parameter_id != self.parameter_id {
            return Err(PrivacyProofEnvelopeValidationError::ActivationParameterIdMismatch);
        }
        if activation.parameter_digest != self.parameter_digest {
            return Err(PrivacyProofEnvelopeValidationError::ActivationParameterDigestMismatch);
        }
        if activation.verifier_digest != self.verifier_digest {
            return Err(PrivacyProofEnvelopeValidationError::ActivationVerifierDigestMismatch);
        }
        if activation.statement_schema_digest != self.statement_schema_digest {
            return Err(
                PrivacyProofEnvelopeValidationError::ActivationStatementSchemaDigestMismatch,
            );
        }
        if activation.engine_manifest_digest != self.engine_manifest_digest {
            return Err(
                PrivacyProofEnvelopeValidationError::ActivationEngineManifestDigestMismatch,
            );
        }
        activation
            .protocol_limits
            .validate_statement(&self.statement)
            .map_err(PrivacyProofEnvelopeValidationError::ActivationStatementLimits)?;
        self.validate_with_limits(consensus_limits)
    }
}

/// Validation failure for [`PrivacyProofEnvelopeV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyProofEnvelopeValidationError {
    /// Supplied consensus limits are invalid.
    #[error("privacy envelope limits are invalid: {0}")]
    InvalidLimits(PrivacyConsensusLimitsValidationError),
    /// Protocol and proof-system identities differ.
    #[error("privacy envelope requires proof system {expected:?}, got {actual:?}")]
    ProofSystemMismatch {
        /// Required proof system.
        expected: PrivacyProofSystemIdV1,
        /// Supplied proof system.
        actual: PrivacyProofSystemIdV1,
    },
    /// Protocol and native engine identities differ.
    #[error("privacy envelope requires engine {expected:?}, got {actual:?}")]
    EngineMismatch {
        /// Required native engine.
        expected: PrivacyEngineIdV1,
        /// Supplied native engine.
        actual: PrivacyEngineIdV1,
    },
    /// Envelope and statement protocol variants differ.
    #[error("privacy envelope protocol {envelope:?} differs from statement protocol {statement:?}")]
    StatementProtocolMismatch {
        /// Envelope protocol.
        envelope: PrivacyProtocolIdV1,
        /// Statement variant protocol.
        statement: PrivacyProtocolIdV1,
    },
    /// Envelope and proof protocol variants differ.
    #[error("privacy envelope protocol {envelope:?} differs from proof protocol {proof:?}")]
    ProofProtocolMismatch {
        /// Envelope protocol.
        envelope: PrivacyProtocolIdV1,
        /// Proof variant protocol.
        proof: PrivacyProtocolIdV1,
    },
    /// ZK-AMS statement action and typed proof action differ.
    #[error("ZK-AMS statement action and proof variant differ")]
    ZkAmsActionProofMismatch,
    /// Parameter-set identifier is zero.
    #[error("privacy envelope parameter id must be non-zero")]
    ZeroParameterId,
    /// Parameter digest is zero.
    #[error("privacy envelope parameter digest must be non-zero")]
    ZeroParameterDigest,
    /// Verifier digest is zero.
    #[error("privacy envelope verifier digest must be non-zero")]
    ZeroVerifierDigest,
    /// Statement-schema digest is zero.
    #[error("privacy envelope statement-schema digest must be non-zero")]
    ZeroStatementSchemaDigest,
    /// Engine-manifest digest is zero.
    #[error("privacy envelope engine-manifest digest must be non-zero")]
    ZeroEngineManifestDigest,
    /// Statement digest is zero.
    #[error("privacy envelope statement digest must be non-zero")]
    ZeroStatementDigest,
    /// Statement and envelope parameter-set identifiers differ.
    #[error("privacy statement parameter id differs from envelope")]
    StatementParameterIdMismatch,
    /// Statement and envelope parameter digests differ.
    #[error("privacy statement parameter digest differs from envelope")]
    StatementParameterDigestMismatch,
    /// Statement and envelope verifier digests differ.
    #[error("privacy statement verifier digest differs from envelope")]
    StatementVerifierDigestMismatch,
    /// Statement and envelope schema digests differ.
    #[error("privacy statement schema digest differs from envelope")]
    StatementSchemaDigestMismatch,
    /// Statement and envelope engine-manifest digests differ.
    #[error("privacy statement engine-manifest digest differs from envelope")]
    StatementEngineManifestDigestMismatch,
    /// Statement validation failed.
    #[error("privacy envelope statement is invalid: {0}")]
    Statement(PrivacyStatementValidationError),
    /// Proof payload validation failed.
    #[error("privacy envelope proof is invalid: {0}")]
    Proof(PrivacyProofValidationError),
    /// Canonical statement digest does not match the envelope.
    #[error("privacy envelope statement digest mismatch")]
    StatementDigestMismatch {
        /// Digest recomputed from the canonical statement.
        expected: PrivacyStatementDigestV1,
        /// Digest carried by the envelope.
        actual: PrivacyStatementDigestV1,
    },
    /// Canonical Norito encoding failed.
    #[error("privacy envelope canonical encoding failed")]
    EncodingFailure,
    /// Encoded length cannot be represented canonically.
    #[error("privacy envelope encoded length overflow")]
    EncodedLengthOverflow,
    /// Encoded action exceeds its consensus bound.
    #[error("privacy envelope action uses {bytes} bytes, exceeding maximum {max}")]
    ActionTooLarge {
        /// Observed encoded bytes.
        bytes: u64,
        /// Configured maximum.
        max: u32,
    },
    /// Encoded action exceeds the transaction privacy budget.
    #[error("privacy envelope transaction payload uses {bytes} bytes, exceeding maximum {max}")]
    TransactionPrivacyPayloadTooLarge {
        /// Observed encoded bytes.
        bytes: u64,
        /// Configured maximum.
        max: u32,
    },
    /// Governed activation record is internally invalid.
    #[error("privacy activation record is invalid: {0}")]
    InvalidActivation(PrivacyActivationValidationError),
    /// Activation and envelope protocols differ.
    #[error(
        "privacy activation protocol {activation:?} differs from envelope protocol {envelope:?}"
    )]
    ActivationProtocolMismatch {
        /// Governed protocol.
        activation: PrivacyProtocolIdV1,
        /// Envelope protocol.
        envelope: PrivacyProtocolIdV1,
    },
    /// Governed activation is not active.
    #[error("privacy protocol activation is not active")]
    ActivationNotActive,
    /// The active state is not yet effective at the current height.
    #[error(
        "privacy activation effective height {effective_height} is later than current height {current_height}"
    )]
    ActivationNotEffective {
        /// Current block height.
        current_height: u64,
        /// First height of the current active interval.
        effective_height: u64,
    },
    /// Governed and envelope proof systems differ.
    #[error(
        "privacy activation proof system {activation:?} differs from envelope proof system {envelope:?}"
    )]
    ActivationProofSystemMismatch {
        /// Governed proof system.
        activation: PrivacyProofSystemIdV1,
        /// Envelope proof system.
        envelope: PrivacyProofSystemIdV1,
    },
    /// Governed and envelope native engines differ.
    #[error("privacy activation engine {activation:?} differs from envelope engine {envelope:?}")]
    ActivationEngineMismatch {
        /// Governed native engine.
        activation: PrivacyEngineIdV1,
        /// Envelope native engine.
        envelope: PrivacyEngineIdV1,
    },
    /// Governed and envelope parameter-set identifiers differ.
    #[error("privacy activation parameter id differs from envelope")]
    ActivationParameterIdMismatch,
    /// Governed and envelope parameter digests differ.
    #[error("privacy activation parameter digest differs from envelope")]
    ActivationParameterDigestMismatch,
    /// Governed and envelope verifier digests differ.
    #[error("privacy activation verifier digest differs from envelope")]
    ActivationVerifierDigestMismatch,
    /// Governed and envelope statement-schema digests differ.
    #[error("privacy activation statement-schema digest differs from envelope")]
    ActivationStatementSchemaDigestMismatch,
    /// Governed and envelope engine-manifest digests differ.
    #[error("privacy activation engine-manifest digest differs from envelope")]
    ActivationEngineManifestDigestMismatch,
    /// Statement exceeds activation-specific governed protocol limits.
    #[error("privacy statement violates active protocol limits: {0}")]
    ActivationStatementLimits(PrivacyActivationStatementLimitsError),
}

#[cfg(any(test, feature = "test-fixtures"))]
mod exact12_fixture {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{domain::DomainId, name::Name};

    const _: [(); 12] = [(); PrivacyProtocolIdV1::COUNT];

    pub(super) fn raw(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    pub(super) fn p256_point(seed: u8) -> PrivacyP256PointV1 {
        let mut bytes = [seed; 33];
        bytes[0] = 0x02;
        PrivacyP256PointV1::new(bytes)
    }

    pub(super) fn p256_ciphertext(seed: u8) -> PrivacyP256CiphertextV1 {
        PrivacyP256CiphertextV1 {
            left: p256_point(seed),
            right: p256_point(seed.wrapping_add(64)),
        }
    }

    pub(super) fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair");
        AccountId::new(key_pair.public_key().clone())
    }

    pub(super) fn asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("asset").expect("asset name"),
        )
    }

    pub(super) fn context() -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            chain_id: "privacy-test-chain".parse().expect("chain id"),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new(raw(6)),
            parameter_id: PrivacyParameterIdV1::new(raw(1)),
            parameter_digest: PrivacyParameterDigestV1::new(raw(2)),
            verifier_digest: PrivacyVerifierDigestV1::new(raw(3)),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(raw(4)),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(raw(5)),
        }
    }

    pub(super) fn commitment(seed: u8) -> PrivacyCommitmentV1 {
        PrivacyCommitmentV1::new(raw(seed))
    }

    pub(super) fn zk_ace_allowlist() -> Vec<AccountId> {
        let mut allowlist = vec![account(13), account(14), account(15)];
        allowlist.sort_unstable();
        allowlist
    }

    pub(super) fn zk_ace_policy(
        epoch: u64,
        identity_seed: u8,
        lifecycle: PrivacyZkAcePolicyLifecycleV1,
    ) -> PrivacyZkAcePolicyRecordV1 {
        PrivacyZkAcePolicyRecordV1::new(
            PrivacyPolicyIdV1::new(raw(10)),
            commitment(identity_seed),
            PrivacyPolicyDigestV1::new(raw(12)),
            epoch,
            asset_definition_id(),
            zk_ace_allowlist(),
            lifecycle,
        )
        .expect("canonical ZK-ACE policy fixture")
    }

    pub(super) fn redigest_zk_ace_policy(record: &mut PrivacyZkAcePolicyRecordV1) {
        record.record_digest = PrivacyZkAcePolicyRecordDigestV1::new([0; 32]);
        record.record_digest = record
            .compute_record_digest()
            .expect("canonical ZK-ACE policy digest material");
    }

    pub(super) fn zk_x509_trust_anchor(
        epoch: u64,
        trust_store_seed: u8,
        previous_record_digest: Option<PrivacyZkX509TrustAnchorRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> PrivacyZkX509TrustAnchorRecordV1 {
        PrivacyZkX509TrustAnchorRecordV1::new(
            PrivacyIssuerIdV1::new(raw(61)),
            epoch,
            PrivacyX509TrustStoreDigestV1::new(raw(trust_store_seed)),
            PrivacyRootV1::new(raw(trust_store_seed.wrapping_add(1))),
            if lifecycle == PrivacyZkX509RecordLifecycleV1::Revoked {
                epoch.saturating_sub(1)
            } else {
                epoch
            },
            previous_record_digest,
            lifecycle,
        )
        .expect("canonical X.509 trust-anchor fixture")
    }

    pub(super) fn zk_x509_certificate_policy(
        epoch: u64,
        policy_seed: u8,
        disclosures: Vec<u8>,
        previous_record_digest: Option<PrivacyZkX509CertificatePolicyRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> PrivacyZkX509CertificatePolicyRecordV1 {
        PrivacyZkX509CertificatePolicyRecordV1::new(
            PrivacyIssuerIdV1::new(raw(61)),
            PrivacyPolicyIdV1::new(raw(62)),
            epoch,
            PrivacyPolicyDigestV1::new(raw(policy_seed)),
            PrivacyX509KeyUsageV1 {
                digital_signature: true,
                content_commitment: false,
                key_encipherment: false,
                key_agreement: false,
            },
            vec![
                PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
            ],
            disclosures,
            previous_record_digest,
            lifecycle,
        )
        .expect("canonical X.509 certificate-policy fixture")
    }

    pub(super) fn zk_x509_crl(
        epoch: u64,
        crl_der_seed: u8,
        this_update_unix_seconds: u64,
        _revoked_root_seed: u8,
        previous_record_digest: Option<PrivacyZkX509CrlRecordDigestV1>,
        lifecycle: PrivacyZkX509RecordLifecycleV1,
    ) -> PrivacyZkX509CrlRecordV1 {
        PrivacyZkX509CrlRecordV1::new(
            PrivacyIssuerIdV1::new(raw(61)),
            PrivacyPolicyIdV1::new(raw(62)),
            epoch,
            epoch,
            PrivacyX509CrlDerDigestV1::new(raw(crl_der_seed)),
            PrivacyX509CrlIssuerSpkiDigestV1::new(raw(74)),
            this_update_unix_seconds,
            this_update_unix_seconds + 300,
            previous_record_digest,
            lifecycle,
        )
        .expect("canonical X.509 signed-CRL fixture")
    }

    pub(super) fn nullifier(seed: u8) -> PrivacyNullifierV1 {
        PrivacyNullifierV1::new(raw(seed))
    }

    pub(super) fn zk_ams_seed_key(seed: u8) -> PrivacyZkAmsSeedPublicKeyV1 {
        PrivacyZkAmsSeedPublicKeyV1::new(raw(seed))
    }

    pub(super) fn zk_ams_anchor(seed: u8) -> PrivacyZkAmsAdmissionAnchorV1 {
        PrivacyZkAmsAdmissionAnchorV1 {
            phc_hash: PrivacyZkAmsPhcHashV1::new(raw(seed)),
            seed_public_key: zk_ams_seed_key(seed.wrapping_add(32)),
        }
    }

    pub(super) fn zk_ams_provision_statement(ring_size: u8) -> PrivacyStatementV1 {
        PrivacyStatementV1::IrohaZkAmsV1(IrohaZkAmsStatementV1 {
            context: context(),
            issuer_id: PrivacyIssuerIdV1::new(raw(40)),
            issuer_public_key: p256_point(42),
            issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1::new(raw(43)),
            registry_id: PrivacyZkAmsRegistryIdV1::new(raw(41)),
            registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1::new(raw(44)),
            policy_id: PrivacyPolicyIdV1::new(raw(45)),
            policy_digest: PrivacyPolicyDigestV1::new(raw(46)),
            action: PrivacyZkAmsActionV1::ProvisionAccount(PrivacyZkAmsProvisionAccountV1 {
                account_registry_root: PrivacyRootV1::new(raw(144)),
                account_registry_root_epoch: 10,
                admitted_seed_key_ring: (1..=ring_size).map(zk_ams_seed_key).collect(),
                account_id: account(200),
                key_image: PrivacyZkAmsKeyImageV1::new(raw(201)),
            }),
        })
    }

    pub(super) fn jindo_field(seed: u8) -> PrivacyJindoFieldElementV1 {
        let mut encoding = [0; IROHA_JINDO_FIELD_ELEMENT_BYTES_V1];
        encoding[0] = seed;
        PrivacyJindoFieldElementV1::new(encoding)
    }

    pub(super) fn jindo_commitment(seed: u8) -> PrivacyJindoLatticeCommitmentV1 {
        let mut encoding = vec![0; IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1];
        encoding[..4].copy_from_slice(&i32::from(seed).to_le_bytes());
        PrivacyJindoLatticeCommitmentV1::new(encoding)
    }

    pub(super) fn encrypted_output(
        commitment_seed: u8,
        recipient_seed: u8,
    ) -> PrivacyEncryptedOutputV1 {
        let mut ciphertext = vec![0xA5; PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1];
        ciphertext[..4].copy_from_slice(&PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1);
        ciphertext[4] = recipient_seed;
        PrivacyEncryptedOutputV1 {
            recipient: PrivacyRecipientIdV1::new(raw(recipient_seed)),
            ephemeral_public_key: PrivacyEncryptionKeyV1::new(raw(recipient_seed.wrapping_add(1))),
            commitment: commitment(commitment_seed),
            ciphertext,
        }
    }

    pub(super) fn fcmp_output(seed: u8) -> PrivacyFcmpOutputTupleV1 {
        PrivacyFcmpOutputTupleV1 {
            output_key: raw(seed),
            linking_tag_generator: raw(seed.wrapping_add(1)),
            amount_commitment: raw(seed.wrapping_add(2)),
        }
    }

    pub(super) fn sorted_fcmp_outputs(seeds: &[u8]) -> Vec<PrivacyFcmpOutputTupleV1> {
        let mut outputs = seeds.iter().copied().map(fcmp_output).collect::<Vec<_>>();
        outputs.sort_unstable_by_key(|output| output.output_id());
        outputs
    }

    pub(super) fn fcmp_input(seed: u8) -> PrivacyFcmpInputPublicV1 {
        PrivacyFcmpInputPublicV1 {
            output_key_tilde: raw(seed),
            linking_tag_generator_tilde: raw(seed.wrapping_add(1)),
            rerandomization_commitment: raw(seed.wrapping_add(2)),
            pseudo_out: raw(seed.wrapping_add(3)),
            key_image: PrivacyFcmpKeyImageV1::new(raw(seed.wrapping_add(4))),
        }
    }

    pub(super) fn fcmp_encrypted_output(
        output: PrivacyFcmpOutputTupleV1,
        recipient_seed: u8,
    ) -> PrivacyFcmpEncryptedOutputV1 {
        let mut ciphertext = vec![0xA5; PRIVACY_FCMP_ENCRYPTED_OUTPUT_BYTES_V1];
        ciphertext[..4].copy_from_slice(&PRIVACY_FCMP_ENCRYPTED_OUTPUT_MAGIC_V1);
        ciphertext[4] = recipient_seed;
        PrivacyFcmpEncryptedOutputV1 {
            recipient: PrivacyRecipientIdV1::new(raw(recipient_seed)),
            ephemeral_public_key: PrivacyEncryptionKeyV1::new(raw(recipient_seed.wrapping_add(1))),
            output_id: output.output_id(),
            ciphertext,
        }
    }

    pub(super) fn orchard_action(seed: u8) -> PrivacyOrchardActionV1 {
        PrivacyOrchardActionV1 {
            nullifier: raw(seed),
            randomized_key: raw(seed.wrapping_add(1)),
            note_commitment: raw(seed.wrapping_add(2)),
            ephemeral_key: raw(seed.wrapping_add(3)),
            encrypted_note: vec![seed.wrapping_add(4); ORCHARD_ENCRYPTED_NOTE_BYTES_V1],
            outgoing_ciphertext: vec![seed.wrapping_add(5); ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1],
            value_commitment: raw(seed.wrapping_add(6)),
        }
    }

    pub(super) fn bootle_lantern_policy() -> BootleLanternIssuerPolicyV1 {
        let entries = (0..BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1
            * BOOTLE_LANTERN_ISSUER_MATRIX_DIMENSION_V1)
            .map(|entry| BootleLanternPolynomialV1 {
                coefficients: (0..BOOTLE_LANTERN_RING_DEGREE_V1)
                    .map(|coefficient| {
                        u16::try_from((entry * 67 + coefficient + 1) % 12_288)
                            .expect("test residue fits u16")
                    })
                    .collect(),
            })
            .collect();
        let allowed_values = (0..BOOTLE_LANTERN_ATTRIBUTE_COUNT_V1)
            .map(|index| BootleLanternAllowedAttributeValuesV1 {
                values: if index == 1 {
                    vec![
                        BootleLanternAttributeValueV1::new([1; 8]),
                        BootleLanternAttributeValueV1::new([2; 8]),
                    ]
                } else {
                    Vec::new()
                },
            })
            .collect();
        let mut record = BootleLanternIssuerPolicyV1 {
            issuer_id: PrivacyIssuerIdV1::new(raw(171)),
            policy_id: PrivacyPolicyIdV1::new(raw(172)),
            epoch: 1,
            lifecycle: BootleLanternIssuerPolicyLifecycleV1::Active,
            issuer_parameter_id: PrivacyParameterIdV1::new(raw(173)),
            issuer_parameter_digest: PrivacyParameterDigestV1::new([0; 32]),
            issuer_public_matrix: BootleLanternIssuerPublicMatrixV1 { entries },
            required_disclosure_bitmap: 0b0001_0010,
            allowed_values,
            record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]),
        };
        redigest_bootle_lantern_policy(&mut record);
        record
    }

    pub(super) fn redigest_bootle_lantern_policy(record: &mut BootleLanternIssuerPolicyV1) {
        record.issuer_parameter_digest = record
            .computed_issuer_parameter_digest()
            .expect("test issuer-parameter digest");
        record.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        record.record_digest = record
            .computed_record_digest()
            .expect("test issuer-policy digest");
    }

    pub(super) fn sample_statements() -> Vec<PrivacyStatementV1> {
        let asset = asset_definition_id();
        vec![
            PrivacyStatementV1::ZkAcePqAuthorizationV0(ZkAcePqAuthorizationStatementV1 {
                context: context(),
                identity_commitment: commitment(10),
                policy_id: PrivacyPolicyIdV1::new(raw(11)),
                policy_digest: PrivacyPolicyDigestV1::new(raw(12)),
                source: account(13),
                destination: account(14),
                asset_definition_id: asset.clone(),
                amount: 1_000,
                authorization_epoch: 7,
                replay_nullifier: nullifier(15),
            }),
            PrivacyStatementV1::AnonymousPgcKOutOfNV1(AnonymousPgcKOutOfNStatementV1 {
                context: context(),
                asset_definition_id: asset.clone(),
                pool_id: PrivacyPoolIdV1::new(raw(20)),
                account_state_root: PrivacyRootV1::new(raw(21)),
                account_state_root_epoch: 8,
                next_account_state_root: PrivacyRootV1::new(raw(22)),
                next_account_state_root_epoch: 9,
                anonymity_set_public_keys: (1..=16).map(p256_point).collect(),
                transfer_ciphertexts: (1..=16).map(p256_ciphertext).collect(),
                recipient_count: 2,
            }),
            PrivacyStatementV1::VeRangeTransparentRangeV1(VeRangeTransparentRangeStatementV1 {
                context: context(),
                asset_definition_id: asset.clone(),
                policy_id: PrivacyPolicyIdV1::new(raw(35)),
                value_commitments: vec![p256_point(36), p256_point(37)],
                bit_length: PrivacyVeRangeBitLengthV1::Bits32,
                aggregation_count: 2,
            }),
            PrivacyStatementV1::IrohaZkAmsV1(IrohaZkAmsStatementV1 {
                context: context(),
                issuer_id: PrivacyIssuerIdV1::new(raw(40)),
                issuer_public_key: p256_point(42),
                issuer_policy_record_digest: PrivacyZkAmsIssuerPolicyRecordDigestV1::new(raw(43)),
                registry_id: PrivacyZkAmsRegistryIdV1::new(raw(41)),
                registry_record_digest: PrivacyZkAmsRegistryRecordDigestV1::new(raw(44)),
                policy_id: PrivacyPolicyIdV1::new(raw(45)),
                policy_digest: PrivacyPolicyDigestV1::new(raw(46)),
                action: PrivacyZkAmsActionV1::BatchAdmission(PrivacyZkAmsBatchAdmissionV1 {
                    account_registry_root: PrivacyRootV1::new(raw(144)),
                    account_registry_root_epoch: 10,
                    next_account_registry_root: PrivacyRootV1::new(raw(145)),
                    next_account_registry_root_epoch: 11,
                    anchors: vec![zk_ams_anchor(44), zk_ams_anchor(45)],
                }),
            }),
            PrivacyStatementV1::VegaExistingCredentialZkV0(VegaExistingCredentialStatementV1 {
                context: context(),
                issuer_id: PrivacyIssuerIdV1::new(raw(49)),
                issuer_record_epoch: 3,
                issuer_record_digest: PrivacyVegaIssuerRecordDigestV1::new(raw(51)),
                document_type: PrivacyCredentialDocumentTypeV1::Iso18013_5Mdl,
                namespace: PrivacyVegaMdlNamespaceV1::OrgIso18013_5_1,
                digest_algorithm: PrivacyVegaMdlDigestAlgorithmV1::Sha256,
                issuer_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
                device_authentication_algorithm: PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
                issuer_public_key: p256_point(50),
                device_authentication_digest: PrivacyVegaDeviceAuthenticationDigestV1::new(raw(57)),
                presentation_date: PrivacyVegaMdlDateV1 {
                    year: 2_026,
                    month: 7,
                    day: 26,
                },
                minimum_age_years: 18,
                reader_challenge: PrivacyChallengeV1::new(raw(58)),
                session_transcript_digest: PrivacySessionTranscriptDigestV1::new(raw(59)),
            }),
            PrivacyStatementV1::IrohaZkX509StarkP256V0(IrohaZkX509StarkP256StatementV1 {
                context: context(),
                trust_anchor_id: PrivacyIssuerIdV1::new(raw(61)),
                certificate_policy_id: PrivacyPolicyIdV1::new(raw(62)),
                trust_anchor_record_digest: PrivacyZkX509TrustAnchorRecordDigestV1::new(raw(69)),
                trust_anchor_record_epoch: 3,
                certificate_policy_record_digest: PrivacyZkX509CertificatePolicyRecordDigestV1::new(
                    raw(70),
                ),
                certificate_policy_record_epoch: 4,
                crl_record_digest: PrivacyZkX509CrlRecordDigestV1::new(raw(73)),
                crl_record_epoch: 5,
                subject_public_key_digest: PrivacyCertificateKeyDigestV1::new(raw(63)),
                ca_membership_root: PrivacyRootV1::new(raw(64)),
                ca_membership_root_epoch: 10,
                key_usage: PrivacyX509KeyUsageV1 {
                    digital_signature: true,
                    content_commitment: false,
                    key_encipherment: false,
                    key_agreement: false,
                },
                extended_key_usages: vec![
                    PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                    PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
                ],
                disclosed_attributes: vec![
                    PrivacyZkX509DisclosedAttributeV1 {
                        index: 0,
                        attribute_digest: PrivacyAttributeDigestV1::new(raw(71)),
                    },
                    PrivacyZkX509DisclosedAttributeV1 {
                        index: 3,
                        attribute_digest: PrivacyAttributeDigestV1::new(raw(72)),
                    },
                ],
                presentation_not_before_unix_seconds: 1_400,
                presentation_not_after_unix_seconds: 1_600,
                wallet_account: account(66),
                wallet_challenge: PrivacyChallengeV1::new(raw(67)),
                certificate_nullifier: nullifier(68),
            }),
            PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(
                IrohaJindoPolynomialCommitmentStatementV1 {
                    context: context(),
                    polynomial_commitments: vec![jindo_commitment(70), jindo_commitment(71)],
                    evaluation_point: jindo_field(1),
                    claimed_evaluations: vec![jindo_field(4), jindo_field(5)],
                },
            ),
            PrivacyStatementV1::IrohaBootleLanternAnoncredV1(
                IrohaBootleLanternAnoncredStatementV1 {
                    context: context(),
                    issuer_id: PrivacyIssuerIdV1::new(raw(72)),
                    policy_id: PrivacyPolicyIdV1::new(raw(73)),
                    issuer_policy_epoch: 12,
                    issuer_policy_record_digest: PrivacyBootleLanternIssuerPolicyDigestV1::new(
                        raw(76),
                    ),
                    issuer_parameter_id: PrivacyParameterIdV1::new(raw(74)),
                    issuer_parameter_digest: PrivacyParameterDigestV1::new(raw(75)),
                    disclosures: vec![
                        BootleLanternDisclosedAttributeV1 {
                            index: 1,
                            value: BootleLanternAttributeValueV1::new([0; 8]),
                        },
                        BootleLanternDisclosedAttributeV1 {
                            index: 4,
                            value: BootleLanternAttributeValueV1::new([u8::MAX; 8]),
                        },
                    ],
                },
            ),
            PrivacyStatementV1::OrchardHalo2ActionsV1(OrchardHalo2ActionsStatementV1 {
                context: context(),
                asset_definition_id: asset.clone(),
                pool_id: PrivacyPoolIdV1::new(raw(81)),
                anchor: PrivacyRootV1::new(raw(82)),
                anchor_epoch: 13,
                actions: vec![orchard_action(83)],
                value_balance: PrivacyValueBalanceV1::balanced(),
                expiry_height: 10_000,
            }),
            PrivacyStatementV1::MoneroFcmpPlusPlusV1({
                let output = fcmp_output(91);
                MoneroFcmpPlusPlusStatementV1 {
                    context: context(),
                    asset_definition_id: asset.clone(),
                    pool_id: PrivacyPoolIdV1::new(raw(87)),
                    output_set_root: PrivacyFcmpTreeRootV1 {
                        layers: 1,
                        point: raw(88),
                    },
                    root_epoch: 14,
                    inputs: vec![fcmp_input(89)],
                    outputs: vec![output],
                    encrypted_outputs: vec![fcmp_encrypted_output(output, 92)],
                }
            }),
            PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1({
                let mut statement = IrohaIvmPrivateNoteStarkStatementV1 {
                    context: context(),
                    asset_definition_id: asset.clone(),
                    pool_id: PrivacyPoolIdV1::new(raw(94)),
                    program_id: PrivacyProgramIdV1::new(raw(95)),
                    action_digest: PrivacyActionDigestV1::new([0; 32]),
                    state_root: PrivacyRootV1::new(raw(96)),
                    root_epoch: 15,
                    nullifiers: vec![nullifier(97)],
                    output_commitments: vec![commitment(98)],
                    encrypted_outputs: vec![encrypted_output(98, 99)],
                    value_balance: PrivacyValueBalanceV1::balanced(),
                    execution_epoch: 15,
                };
                statement.action_digest = statement
                    .computed_action_digest()
                    .expect("compute private-IVM fixture action digest");
                statement
            }),
            PrivacyStatementV1::PqMaspStarkV0(PqMaspStarkStatementV1 {
                context: context(),
                asset_definition_id: asset,
                pool_id: PrivacyPoolIdV1::new(raw(101)),
                anchor: PrivacyRootV1::new(raw(102)),
                anchor_epoch: 17,
                nullifiers: vec![nullifier(103)],
                output_commitments: vec![commitment(104)],
                encrypted_outputs: vec![encrypted_output(104, 105)],
                authorization_profile: PrivacyPqAuthorizationProfileV1::MlDsa65,
                authorization_key_digest: PrivacyAuthorizationKeyDigestV1::new(raw(107)),
                note_encryption_profile:
                    PrivacyPqNoteEncryptionProfileV1::MlKem768XChaCha20Poly1305,
                note_encryption_key_digest: PrivacyNoteEncryptionKeyDigestV1::new(raw(108)),
                authorization_epoch: 17,
            }),
        ]
    }

    pub(super) fn statement_for(protocol: PrivacyProtocolIdV1) -> PrivacyStatementV1 {
        sample_statements()
            .into_iter()
            .find(|statement| statement.protocol_id() == protocol)
            .expect("sample statement for every protocol")
    }

    pub(super) fn proof_for(protocol: PrivacyProtocolIdV1) -> PrivacyProofV1 {
        let bytes = PrivacyProofBytesV1::new(vec![0xA5, 0x5A, 1]);
        match protocol {
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => {
                PrivacyProofV1::ZkAcePqAuthorizationV0(bytes)
            }
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => {
                PrivacyProofV1::AnonymousPgcKOutOfNV1(bytes)
            }
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
                PrivacyProofV1::VeRangeTransparentRangeV1(bytes)
            }
            PrivacyProtocolIdV1::IrohaZkAmsV1 => PrivacyProofV1::IrohaZkAmsV1(
                IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(bytes),
            ),
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0 => {
                PrivacyProofV1::VegaExistingCredentialZkV0(bytes)
            }
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => {
                PrivacyProofV1::IrohaZkX509StarkP256V0(bytes)
            }
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 => {
                PrivacyProofV1::IrohaJindoPolynomialCommitmentV0(bytes)
            }
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
                PrivacyProofV1::IrohaBootleLanternAnoncredV1(bytes)
            }
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
                PrivacyProofV1::OrchardHalo2ActionsV1(bytes)
            }
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
                PrivacyProofV1::MoneroFcmpPlusPlusV1(bytes)
            }
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyProofV1::IrohaIvmPrivateNoteStarkV1(bytes)
            }
            PrivacyProtocolIdV1::PqMaspStarkV0 => PrivacyProofV1::PqMaspStarkV0(bytes),
        }
    }

    pub(super) fn statement_variant_name(statement: &PrivacyStatementV1) -> &'static str {
        match statement {
            PrivacyStatementV1::ZkAcePqAuthorizationV0(_) => "ZkAcePqAuthorizationV0",
            PrivacyStatementV1::AnonymousPgcKOutOfNV1(_) => "AnonymousPgcKOutOfNV1",
            PrivacyStatementV1::VeRangeTransparentRangeV1(_) => "VeRangeTransparentRangeV1",
            PrivacyStatementV1::IrohaZkAmsV1(_) => "IrohaZkAmsV1",
            PrivacyStatementV1::VegaExistingCredentialZkV0(_) => "VegaExistingCredentialZkV0",
            PrivacyStatementV1::IrohaZkX509StarkP256V0(_) => "IrohaZkX509StarkP256V0",
            PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(_) => {
                "IrohaJindoPolynomialCommitmentV0"
            }
            PrivacyStatementV1::IrohaBootleLanternAnoncredV1(_) => "IrohaBootleLanternAnoncredV1",
            PrivacyStatementV1::OrchardHalo2ActionsV1(_) => "OrchardHalo2ActionsV1",
            PrivacyStatementV1::MoneroFcmpPlusPlusV1(_) => "MoneroFcmpPlusPlusV1",
            PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(_) => "IrohaIvmPrivateNoteStarkV1",
            PrivacyStatementV1::PqMaspStarkV0(_) => "PqMaspStarkV0",
        }
    }

    pub(super) fn proof_variant_name(proof: &PrivacyProofV1) -> &'static str {
        match proof {
            PrivacyProofV1::ZkAcePqAuthorizationV0(_) => "ZkAcePqAuthorizationV0",
            PrivacyProofV1::AnonymousPgcKOutOfNV1(_) => "AnonymousPgcKOutOfNV1",
            PrivacyProofV1::VeRangeTransparentRangeV1(_) => "VeRangeTransparentRangeV1",
            PrivacyProofV1::IrohaZkAmsV1(_) => "IrohaZkAmsV1",
            PrivacyProofV1::VegaExistingCredentialZkV0(_) => "VegaExistingCredentialZkV0",
            PrivacyProofV1::IrohaZkX509StarkP256V0(_) => "IrohaZkX509StarkP256V0",
            PrivacyProofV1::IrohaJindoPolynomialCommitmentV0(_) => {
                "IrohaJindoPolynomialCommitmentV0"
            }
            PrivacyProofV1::IrohaBootleLanternAnoncredV1(_) => "IrohaBootleLanternAnoncredV1",
            PrivacyProofV1::OrchardHalo2ActionsV1(_) => "OrchardHalo2ActionsV1",
            PrivacyProofV1::MoneroFcmpPlusPlusV1(_) => "MoneroFcmpPlusPlusV1",
            PrivacyProofV1::IrohaIvmPrivateNoteStarkV1(_) => "IrohaIvmPrivateNoteStarkV1",
            PrivacyProofV1::PqMaspStarkV0(_) => "PqMaspStarkV0",
        }
    }

    fn try_envelope(
        statement: PrivacyStatementV1,
    ) -> Result<PrivacyProofEnvelopeV1, norito::Error> {
        let protocol_id = statement.protocol_id();
        let context = statement.context().clone();
        let statement_digest = statement.digest()?;
        let proof = match &statement {
            PrivacyStatementV1::IrohaZkAmsV1(IrohaZkAmsStatementV1 {
                action: PrivacyZkAmsActionV1::ProvisionAccount(_),
                ..
            }) => {
                PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::Ristretto255LsagProvisionAccount(
                    PrivacyProofBytesV1::new(vec![0xA5, 0x5A, 1]),
                ))
            }
            _ => proof_for(protocol_id),
        };
        Ok(PrivacyProofEnvelopeV1 {
            protocol_id,
            proof_system_id: protocol_id.expected_proof_system(),
            engine_id: protocol_id.expected_engine(),
            parameter_id: context.parameter_id,
            parameter_digest: context.parameter_digest,
            verifier_digest: context.verifier_digest,
            statement_schema_digest: context.statement_schema_digest,
            engine_manifest_digest: context.engine_manifest_digest,
            statement_digest,
            statement,
            proof,
        })
    }

    pub(super) fn envelope(statement: PrivacyStatementV1) -> PrivacyProofEnvelopeV1 {
        try_envelope(statement).expect("fixture statement has a canonical digest")
    }

    /// One compiled semantic row of the canonical exact-12 cross-SDK fixture.
    ///
    /// This test-fixture-only type is derived from actual typed statements,
    /// proof variants, and canonical Norito envelope bytes. It never parses the
    /// checked-in TSV and carries no embedded digest constants.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct PrivacyExact12TypedEnvelopeRowV1 {
        /// Closed protocol identity in canonical discriminant order.
        pub protocol_id: PrivacyProtocolIdV1,
        /// Actual typed statement variant selected for the sample.
        pub statement_variant: &'static str,
        /// Actual typed proof variant selected for the sample envelope.
        pub proof_variant: &'static str,
        /// Digest recomputed from the actual typed sample statement.
        pub statement_digest: [u8; 32],
        /// SHA-256 of the actual canonical Norito proof envelope.
        pub envelope_sha256: [u8; 32],
    }

    /// Failure to derive the fixed exact-12 semantic fixture from current
    /// typed values and canonical Norito serialization.
    #[derive(Debug, Error)]
    pub enum PrivacyExact12FixtureErrorV1 {
        /// A typed statement digest or proof-envelope encoding failed.
        #[error("exact12 typed fixture canonical encoding failed: {0}")]
        CanonicalEncoding(#[from] norito::Error),
        /// The fixture constructor no longer produced the closed registry size.
        #[error("exact12 typed fixture produced {actual} rows instead of 12")]
        RowCount {
            /// Number of rows constructed from the current typed fixtures.
            actual: usize,
        },
    }

    /// Recompute all 12 canonical cross-SDK semantic rows from current Rust
    /// types and canonical Norito serialization.
    ///
    /// The helper is available only to tests and builds that explicitly enable
    /// `iroha_data_model/test-fixtures`; ordinary validator builds do not
    /// compile this fixture surface.
    ///
    /// # Errors
    ///
    /// Returns an error if a typed statement or envelope cannot be encoded
    /// canonically, or if the fixture constructor does not produce exactly 12
    /// rows.
    pub fn privacy_exact12_typed_envelope_rows_v1()
    -> Result<[PrivacyExact12TypedEnvelopeRowV1; 12], PrivacyExact12FixtureErrorV1> {
        let rows = sample_statements()
            .into_iter()
            .map(|statement| {
                let protocol_id = statement.protocol_id();
                let proof_envelope = try_envelope(statement)?;
                let canonical_envelope = norito::encode_canonical(&proof_envelope)?;
                Ok(PrivacyExact12TypedEnvelopeRowV1 {
                    protocol_id,
                    statement_variant: statement_variant_name(&proof_envelope.statement),
                    proof_variant: proof_variant_name(&proof_envelope.proof),
                    statement_digest: *proof_envelope.statement_digest.as_bytes(),
                    envelope_sha256: Sha256::digest(canonical_envelope).into(),
                })
            })
            .collect::<Result<Vec<_>, norito::Error>>()?;
        rows.try_into()
            .map_err(|rows: Vec<PrivacyExact12TypedEnvelopeRowV1>| {
                PrivacyExact12FixtureErrorV1::RowCount { actual: rows.len() }
            })
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use hex_literal::hex;

    use crate::{domain::DomainId, name::Name};

    use super::{
        exact12_fixture::{
            account, asset_definition_id, bootle_lantern_policy, commitment, context,
            encrypted_output, envelope, fcmp_input, fcmp_output, jindo_commitment, jindo_field,
            nullifier, orchard_action, p256_ciphertext, p256_point, proof_for, proof_variant_name,
            raw, redigest_bootle_lantern_policy, redigest_zk_ace_policy, sample_statements,
            sorted_fcmp_outputs, statement_for, statement_variant_name, zk_ace_allowlist,
            zk_ace_policy, zk_ams_anchor, zk_ams_provision_statement, zk_ams_seed_key,
            zk_x509_certificate_policy, zk_x509_crl, zk_x509_trust_anchor,
        },
        *,
    };

    fn pgc_accounts(count: u8) -> Vec<PrivacyPgcAccountV1> {
        (1..=count)
            .map(|seed| PrivacyPgcAccountV1 {
                public_key: p256_point(seed),
                encrypted_balance: p256_ciphertext(seed),
            })
            .collect()
    }

    fn pgc_bootstrap() -> PrivacyPgcAccountBootstrapV1 {
        let statement = statement_for(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1);
        PrivacyPgcAccountBootstrapV1 {
            namespace: PrivacyNamespaceV1::from_statement(&statement),
            initial_root: PrivacyRootV1::new(raw(201)),
            initial_epoch: 1,
            total_supply: 160,
            accounts: pgc_accounts(16),
        }
    }

    #[derive(Clone, Copy)]
    enum RootCorruption {
        ZeroSuccessor,
        Unchanged,
        SkippedEpoch,
        EpochOverflow,
    }

    fn corrupt_root_transition(statement: &mut PrivacyStatementV1, corruption: RootCorruption) {
        macro_rules! corrupt {
            ($current:expr, $epoch:expr, $next:expr, $next_epoch:expr) => {
                match corruption {
                    RootCorruption::ZeroSuccessor => $next = PrivacyRootV1::new([0; 32]),
                    RootCorruption::Unchanged => $next = $current,
                    RootCorruption::SkippedEpoch => {
                        $next_epoch = $epoch.checked_add(2).expect("fixture epoch has room")
                    }
                    RootCorruption::EpochOverflow => {
                        $epoch = u64::MAX;
                        $next_epoch = 0;
                    }
                }
            };
        }
        match statement {
            PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement) => corrupt!(
                statement.account_state_root,
                statement.account_state_root_epoch,
                statement.next_account_state_root,
                statement.next_account_state_root_epoch
            ),
            PrivacyStatementV1::IrohaZkAmsV1(statement) => {
                let PrivacyZkAmsActionV1::BatchAdmission(batch) = &mut statement.action else {
                    panic!("ZK-AMS provisioning does not manage a root transition")
                };
                corrupt!(
                    batch.account_registry_root,
                    batch.account_registry_root_epoch,
                    batch.next_account_registry_root,
                    batch.next_account_registry_root_epoch
                )
            }
            PrivacyStatementV1::OrchardHalo2ActionsV1(_) => {
                panic!("Orchard successor roots are derived from the authoritative node frontier")
            }
            PrivacyStatementV1::MoneroFcmpPlusPlusV1(_)
            | PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(_)
            | PrivacyStatementV1::PqMaspStarkV0(_) => {
                panic!("FCMP++ and private-note successor roots are validator-derived")
            }
            _ => panic!("protocol does not manage a root transition"),
        }
    }

    fn protocol_limits(protocol: PrivacyProtocolIdV1) -> PrivacyProtocolActivationLimitsV1 {
        match protocol {
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0 => {
                PrivacyProtocolActivationLimitsV1::ZkAcePqAuthorizationV0
            }
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 => {
                PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                    AnonymousPgcActivationLimitsV1 {
                        max_anonymity_set_size: ANONYMOUS_PGC_MAX_ANONYMITY_SET_SIZE_V1,
                        max_recipient_count: ANONYMOUS_PGC_MAX_RECIPIENTS_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1 => {
                PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
                    VeRangeActivationLimitsV1 {
                        max_aggregation_count: VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::IrohaZkAmsV1 => {
                PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                    max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1,
                    max_ring_size: ZK_AMS_MAX_RING_SIZE_V1,
                })
            }
            PrivacyProtocolIdV1::VegaExistingCredentialZkV0 => {
                PrivacyProtocolActivationLimitsV1::VegaExistingCredentialZkV0
            }
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0 => {
                PrivacyProtocolActivationLimitsV1::IrohaZkX509StarkP256V0
            }
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0 => {
                PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                    JindoActivationLimitsV1 {
                        max_polynomial_count: IROHA_JINDO_MAX_POLYNOMIALS_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1 => {
                PrivacyProtocolActivationLimitsV1::IrohaBootleLanternAnoncredV1
            }
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1 => {
                PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(
                    OrchardActivationLimitsV1 {
                        max_action_count: ORCHARD_MAX_ACTIONS_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1 => {
                PrivacyProtocolActivationLimitsV1::MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1 {
                    max_input_count: FCMP_MAX_INPUTS_V1,
                    max_output_count: FCMP_MAX_OUTPUTS_V1,
                })
            }
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(
                    IvmPrivateNoteActivationLimitsV1 {
                        max_input_count: IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                        max_output_count: IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::PqMaspStarkV0 => {
                PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(PqMaspActivationLimitsV1 {
                    max_input_count: PQ_MASP_MAX_INPUTS_V1,
                    max_output_count: PQ_MASP_MAX_OUTPUTS_V1,
                })
            }
        }
    }

    fn assert_stable_schema_wire<T>(value: &T, schema_name: &str, expected_schema_hash: [u8; 16])
    where
        T: norito::NoritoSerialize
            + for<'de> norito::NoritoDeserialize<'de>
            + PartialEq
            + core::fmt::Debug
            + 'static,
    {
        let derived_schema_hash = norito::core::schema_hash_for_name(schema_name);
        assert_eq!(
            derived_schema_hash, expected_schema_hash,
            "permanent schema-name KAT changed for {schema_name}"
        );
        assert_eq!(
            <T as norito::NoritoSerialize>::schema_hash(),
            expected_schema_hash
        );
        assert_eq!(
            <T as norito::NoritoDeserialize<'static>>::schema_hash(),
            expected_schema_hash
        );

        let legacy_type_name_hash = norito::core::type_name_schema_hash::<T>();
        assert_ne!(
            legacy_type_name_hash, expected_schema_hash,
            "permanent public schema must not fall back to the Rust type name"
        );

        let canonical = norito::encode_canonical(value).expect("encode permanent-schema frame");
        assert_eq!(
            &canonical[6..22],
            expected_schema_hash.as_slice(),
            "canonical frame header must carry the permanent schema identity"
        );
        assert_eq!(
            norito::decode_canonical::<T>(&canonical).expect("decode permanent-schema frame"),
            *value
        );

        let mut legacy_header = canonical.clone();
        legacy_header[6..22].copy_from_slice(&legacy_type_name_hash);
        assert!(
            matches!(
                norito::decode_canonical::<T>(&legacy_header),
                Err(norito::Error::SchemaMismatch)
            ),
            "the pre-release Rust type-name wire must fail closed"
        );

        let mut forged_header = canonical;
        forged_header[6] ^= 0x80;
        assert!(
            matches!(
                norito::decode_canonical::<T>(&forged_header),
                Err(norito::Error::SchemaMismatch)
            ),
            "an unknown schema identity must fail before payload decoding"
        );
    }

    #[test]
    fn first_release_privacy_schema_names_and_old_headers_are_frozen() {
        let statement = statement_for(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0);
        let PrivacyStatementV1::ZkAcePqAuthorizationV0(authorization_statement) = &statement else {
            unreachable!("ZK-ACE fixture must use the typed authorization statement")
        };
        let authorization_statement = authorization_statement.clone();
        let public_inputs =
            crate::zk::ZkAcePrivacyPublicInputsV1::new(authorization_statement.clone(), raw(0xD1));
        let proof = proof_for(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0);
        let proof_envelope = envelope(statement.clone());
        let policy = zk_ace_policy(
            PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
            11,
            PrivacyZkAcePolicyLifecycleV1::Active,
        );
        let policy_material = PrivacyZkAcePolicyDigestMaterialV1 {
            policy_id: policy.policy_id,
            identity_commitment: policy.identity_commitment,
            policy_digest: policy.policy_digest,
            authorization_epoch: policy.authorization_epoch,
            asset_definition_id: policy.asset_definition_id,
            source_allowlist: policy.source_allowlist,
            lifecycle: policy.lifecycle,
        };

        assert_stable_schema_wire(
            &authorization_statement,
            ZK_ACE_AUTHORIZATION_STATEMENT_SCHEMA_NAME_V1,
            hex!("4acf679326b17350dcb57f4ea7ac20a1"),
        );
        assert_stable_schema_wire(
            &statement,
            PRIVACY_STATEMENT_SCHEMA_NAME_V1,
            hex!("7966b2f6ebc8c1ff1a1eb8ac458657af"),
        );
        assert_stable_schema_wire(
            &public_inputs,
            crate::zk::ZK_ACE_PRIVACY_PUBLIC_INPUTS_SCHEMA_NAME_V1,
            hex!("0f16958a9641702815d6cddee3aeb8aa"),
        );
        assert_stable_schema_wire(
            &policy_material,
            ZK_ACE_POLICY_DIGEST_MATERIAL_SCHEMA_NAME_V1,
            hex!("5f08d7306e9c76b183a60175fa514966"),
        );
        assert_stable_schema_wire(
            &proof,
            PRIVACY_PROOF_SCHEMA_NAME_V1,
            hex!("8335f36ecd62f6cc59715441a3496a27"),
        );
        assert_stable_schema_wire(
            &proof_envelope,
            PRIVACY_PROOF_ENVELOPE_SCHEMA_NAME_V1,
            hex!("3956178024ddd2abae83d3a5b59827fb"),
        );
    }

    fn activation(envelope: &PrivacyProofEnvelopeV1) -> PrivacyProtocolActivationRecordV1 {
        PrivacyProtocolActivationRecordV1 {
            protocol_id: envelope.protocol_id,
            proof_system_id: envelope.proof_system_id,
            engine_id: envelope.engine_id,
            parameter_id: envelope.parameter_id,
            parameter_digest: envelope.parameter_digest,
            verifier_digest: envelope.verifier_digest,
            statement_schema_digest: envelope.statement_schema_digest,
            engine_manifest_digest: envelope.engine_manifest_digest,
            lifecycle: PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
                proposed_at_height: 1,
                activated_at_height: 2,
                state_since_height: 2,
            }),
            protocol_limits: protocol_limits(envelope.protocol_id),
            pending_protocol_limits_tightening: None,
            assurance: PrivacyAssuranceV1::Experimental,
        }
    }

    fn compiled_profile_snapshot(
        activation: &PrivacyProtocolActivationRecordV1,
    ) -> PrivacyCompiledProfileSnapshotV1 {
        PrivacyCompiledProfileSnapshotV1 {
            protocol_id: activation.protocol_id,
            proof_system_id: activation.proof_system_id,
            engine_id: activation.engine_id,
            parameter_id: activation.parameter_id,
            parameter_digest: activation.parameter_digest,
            verifier_digest: activation.verifier_digest,
            statement_schema_digest: activation.statement_schema_digest,
            engine_manifest_digest: activation.engine_manifest_digest,
            protocol_limits: activation.protocol_limits,
        }
    }

    fn capability_snapshot() -> PrivacyCapabilitySnapshotV1 {
        let pgc_activation = activation(&envelope(statement_for(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        )));
        let pgc_profile = compiled_profile_snapshot(&pgc_activation);
        PrivacyCapabilitySnapshotV1 {
            version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
            committed_height: 2,
            consensus_policy: PrivacyConsensusPolicyV1::taira_default(),
            protocols: PrivacyProtocolIdV1::ALL
                .into_iter()
                .map(|protocol_id| {
                    if protocol_id == PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1 {
                        PrivacyCapabilityRowV1 {
                            protocol_id,
                            compiled_profile: PrivacyCompiledProfileResultV1::Available(
                                pgc_profile,
                            ),
                            activation: Some(pgc_activation),
                        }
                    } else {
                        PrivacyCapabilityRowV1 {
                            protocol_id,
                            compiled_profile: PrivacyCompiledProfileResultV1::Unavailable(
                                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                            ),
                            activation: None,
                        }
                    }
                })
                .collect(),
        }
    }

    #[test]
    fn protocol_ids_keep_closed_norito_discriminants() {
        assert_eq!(PrivacyProtocolIdV1::ALL.len(), PrivacyProtocolIdV1::COUNT);
        for (expected, protocol) in PrivacyProtocolIdV1::ALL.into_iter().enumerate() {
            let encoded = protocol.encode();
            assert_eq!(encoded, u32::try_from(expected).unwrap().to_le_bytes());
            assert_eq!(
                PrivacyProtocolIdV1::decode(&mut encoded.as_slice()).expect("decode protocol"),
                protocol
            );
        }
        for unknown in [PrivacyProtocolIdV1::COUNT as u32, 99, u32::MAX] {
            assert!(
                PrivacyProtocolIdV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err(),
                "unknown protocol discriminant {unknown} must fail"
            );
        }
    }

    #[test]
    fn protocol_ids_have_unique_exact_external_labels() {
        let expected = [
            (
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
                "zk-ace-pq-authorization-v0",
            ),
            (
                PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
                "anonymous-pgc-k-out-of-n-v1",
            ),
            (
                PrivacyProtocolIdV1::VeRangeTransparentRangeV1,
                "verange-transparent-range-v1",
            ),
            (PrivacyProtocolIdV1::IrohaZkAmsV1, "iroha-zk-ams-v1"),
            (
                PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
                "vega-existing-credential-zk-v0",
            ),
            (
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                "iroha-zk-x509-stark-p256-v0",
            ),
            (
                PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
                "iroha-jindo-polynomial-commitment-v0",
            ),
            (
                PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
                "iroha-bootle-lantern-anoncred-v1",
            ),
            (
                PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
                "orchard-halo2-actions-v1",
            ),
            (
                PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
                "monero-fcmp-plus-plus-v1",
            ),
            (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                "iroha-ivm-private-note-stark-v1",
            ),
            (PrivacyProtocolIdV1::PqMaspStarkV0, "pq-masp-stark-v0"),
        ];
        assert_eq!(expected.len(), PrivacyProtocolIdV1::COUNT);

        for (index, (protocol, label)) in expected.into_iter().enumerate() {
            assert_eq!(PrivacyProtocolIdV1::ALL[index], protocol);
            assert_eq!(protocol.canonical_label(), label);
            assert_eq!(
                PrivacyProtocolIdV1::from_canonical_label(label),
                Some(protocol)
            );
            assert!(
                PrivacyProtocolIdV1::ALL[..index]
                    .iter()
                    .all(|prior| prior.canonical_label() != label),
                "duplicate privacy protocol label {label}"
            );
        }
    }

    #[test]
    fn active_and_retired_protocol_labels_share_one_exact_reservation_namespace() {
        let mut reserved = std::collections::BTreeSet::new();
        for protocol in PrivacyProtocolIdV1::ALL {
            let label = protocol.canonical_label();
            assert!(
                privacy_protocol_label_is_reserved_v1(label),
                "active label {label} must be reserved"
            );
            assert!(reserved.insert(label), "duplicate active label {label}");
        }
        for label in PRIVACY_RETIRED_PROTOCOL_LABELS_V1 {
            assert!(
                PrivacyProtocolIdV1::from_canonical_label(label).is_none(),
                "retired label {label} must not become active"
            );
            assert!(
                privacy_protocol_label_is_reserved_v1(label),
                "retired label {label} must remain reserved"
            );
            assert!(
                reserved.insert(label),
                "retired label {label} overlaps another reservation"
            );
        }
        assert_eq!(
            reserved.len(),
            PrivacyProtocolIdV1::COUNT + PRIVACY_RETIRED_PROTOCOL_LABELS_V1.len()
        );

        for label in reserved {
            for near_miss in [
                format!("generic-{label}"),
                format!("{label}-generic"),
                format!(" {label}"),
                format!("{label} "),
                label.to_ascii_uppercase(),
            ] {
                assert!(
                    !privacy_protocol_label_is_reserved_v1(&near_miss),
                    "near-miss label {near_miss:?} must not alias {label:?}"
                );
            }
        }
    }

    #[test]
    fn protocol_id_parser_rejects_aliases_retired_ids_and_noncanonical_text() {
        for label in [
            "",
            " ",
            " iroha-zk-ams-v1",
            "iroha-zk-ams-v1 ",
            "IROHA-ZK-AMS-V1",
            "zk-ams-recursive-admission-v0",
            "zk-x509-onchain-identity-v0",
            "jindo-lattice-pcs-zk-v0",
            "sis-hints-anoncred-pq-v0",
            "sis-with-hints",
            "iroha-bootle-genisis-ac-stark-v0",
            "miden-stark-note-v1",
            "pq-masp-stark-fri-v1",
            "zkat-policy-private-auth-v1",
            "silent-threshold-anoncred-v0",
            "penumbra-masp-v1",
            "aztec-private-rollup-v1",
            "iroha-zk-ams-v1\0",
            "iroha-zk-\u{200b}ams-v1",
            "iroha\u{ff0f}zk-ams-v1",
            "iroh\u{0430}-zk-ams-v1",
        ] {
            assert!(
                PrivacyProtocolIdV1::from_canonical_label(label).is_none(),
                "non-canonical protocol label {label:?} must fail"
            );
        }
    }

    #[test]
    fn privacy_public_json_labels_are_exact_and_roundtrip() {
        for protocol in PrivacyProtocolIdV1::ALL {
            let expected = format!(
                "{{\"protocol\":\"{}\",\"value\":null}}",
                protocol.canonical_label()
            );
            assert_eq!(
                norito::json::to_json(&protocol).expect("serialize protocol id"),
                expected
            );
            assert_eq!(
                norito::json::from_json::<PrivacyProtocolIdV1>(&expected)
                    .expect("deserialize protocol id"),
                protocol
            );

            let limits = protocol_limits(protocol);
            let limits_json = norito::json::to_json(&limits).expect("serialize protocol limits");
            assert!(
                limits_json.starts_with(&format!(
                    "{{\"protocol\":\"{}\",\"limits\":",
                    protocol.canonical_label()
                )),
                "unexpected activation-limit label: {limits_json}"
            );
            assert_eq!(
                norito::json::from_json::<PrivacyProtocolActivationLimitsV1>(&limits_json)
                    .expect("deserialize protocol limits"),
                limits
            );
        }

        let proof_systems = [
            (
                PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
                "stark-fri-sha256-goldilocks",
            ),
            (
                PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512,
                "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
            ),
            (
                PrivacyProofSystemIdV1::AnonymousPgcP256,
                "anonymous-pgc-p256",
            ),
            (
                PrivacyProofSystemIdV1::IrohaVeRangeP256,
                "iroha-verange-p256",
            ),
            (
                PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256,
                "vega-neutron-nova-spartan-hyrax-t256",
            ),
            (
                PrivacyProofSystemIdV1::JindoPolynomialCommitment,
                "jindo-polynomial-commitment",
            ),
            (PrivacyProofSystemIdV1::Halo2IpaPasta, "halo2-ipa-pasta"),
            (
                PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs,
                "fcmp-plus-plus-curve-tree-bulletproofs",
            ),
            (
                PrivacyProofSystemIdV1::LanternLnp22ModuleLinearNorm,
                "lantern-lnp22-module-linear-norm",
            ),
        ];
        for (value, label) in proof_systems {
            let expected = format!("{{\"proof_system\":\"{label}\",\"value\":null}}");
            assert_eq!(
                norito::json::to_json(&value).expect("serialize proof-system id"),
                expected
            );
            assert_eq!(
                norito::json::from_json::<PrivacyProofSystemIdV1>(&expected)
                    .expect("deserialize proof-system id"),
                value
            );
        }

        let engines = [
            (
                PrivacyEngineIdV1::NativeGoldilocksStarkFri,
                "native-goldilocks-stark-fri",
            ),
            (
                PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255,
                "native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
            ),
            (
                PrivacyEngineIdV1::NativeAnonymousPgcP256,
                "native-anonymous-pgc-p256",
            ),
            (PrivacyEngineIdV1::NativeVeRangeP256, "native-verange-p256"),
            (PrivacyEngineIdV1::NativeVega, "native-vega"),
            (PrivacyEngineIdV1::NativeJindo, "native-jindo"),
            (
                PrivacyEngineIdV1::NativeHalo2Orchard,
                "native-halo2-orchard",
            ),
            (
                PrivacyEngineIdV1::NativeFcmpPlusPlus,
                "native-fcmp-plus-plus",
            ),
            (
                PrivacyEngineIdV1::NativeLanternLnp22,
                "native-lantern-lnp22",
            ),
        ];
        for (value, label) in engines {
            let expected = format!("{{\"engine\":\"{label}\",\"value\":null}}");
            assert_eq!(
                norito::json::to_json(&value).expect("serialize engine id"),
                expected
            );
            assert_eq!(
                norito::json::from_json::<PrivacyEngineIdV1>(&expected)
                    .expect("deserialize engine id"),
                value
            );
        }

        let unavailable = [
            (
                PrivacyCompiledProfileUnavailableReasonV1::EngineUnavailable,
                "{\"reason\":\"engine-unavailable\",\"detail\":null}",
            ),
            (
                PrivacyCompiledProfileUnavailableReasonV1::ProfileInitializationFailed,
                "{\"reason\":\"profile-initialization-failed\",\"detail\":null}",
            ),
            (
                PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(
                    PrivacyCompiledStatementSchemaErrorV1::ConflictingStableTypeId,
                ),
                "{\"reason\":\"statement-schema-invalid\",\"detail\":{\"schema_error\":\"conflicting-stable-type-id\",\"detail\":null}}",
            ),
            (
                PrivacyCompiledProfileUnavailableReasonV1::StatementSchemaInvalid(
                    PrivacyCompiledStatementSchemaErrorV1::MissingTypeReference,
                ),
                "{\"reason\":\"statement-schema-invalid\",\"detail\":{\"schema_error\":\"missing-type-reference\",\"detail\":null}}",
            ),
        ];
        for (value, expected) in unavailable {
            assert_eq!(
                norito::json::to_json(&value).expect("serialize unavailable reason"),
                expected
            );
            assert_eq!(
                norito::json::from_json::<PrivacyCompiledProfileUnavailableReasonV1>(expected)
                    .expect("deserialize unavailable reason"),
                value
            );
        }

        assert_eq!(
            norito::json::to_json(&PrivacyAssuranceV1::Experimental).expect("serialize assurance"),
            "{\"assurance\":\"experimental\",\"value\":null}"
        );
        let lifecycle = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 2,
        });
        assert_eq!(
            norito::json::to_json(&lifecycle).expect("serialize lifecycle"),
            "{\"state\":\"active\",\"record\":{\"proposed_at_height\":1,\"activated_at_height\":2,\"state_since_height\":2}}"
        );
    }

    #[test]
    fn privacy_public_json_rejects_aliases_case_whitespace_confusables_and_unknown_fields() {
        for hostile in [
            "AnonymousPgcKOutOfNV1",
            "anonymous-pgc-k-out-of-n",
            "anonymous-pgc-k-out-of-n-v0",
            "ANONYMOUS-PGC-K-OUT-OF-N-V1",
            " anonymous-pgc-k-out-of-n-v1",
            "anonymous-pgc-k-out-of-n-v1 ",
            "anonymous\u{2010}pgc-k-out-of-n-v1",
            "anonym\u{043e}us-pgc-k-out-of-n-v1",
            "iroha-bootle-genisis-ac-stark-v0",
            "unknown",
        ] {
            let json = format!("{{\"protocol\":\"{hostile}\",\"value\":null}}");
            assert!(
                norito::json::from_json::<PrivacyProtocolIdV1>(&json).is_err(),
                "hostile protocol JSON {json} must fail"
            );
        }
        for hostile in [
            "{\"protocol\":\"anonymous-pgc-k-out-of-n-v1\",\"value\":null,\"extra\":1}",
            "{\"protocol\":\"anonymous-pgc-k-out-of-n-v1\",\"protocol\":\"anonymous-pgc-k-out-of-n-v1\",\"value\":null}",
            "{\"proof_system\":\"AnonymousPgcP256\",\"value\":null}",
            "{\"proof_system\":\"anonymous-pgc-p256 \",\"value\":null}",
            "{\"proof_system\":\"stark-fri-poseidon2-goldilocks\",\"value\":null}",
            "{\"engine\":\"NativeAnonymousPgcP256\",\"value\":null}",
            "{\"engine\":\"native-anonymous-pgc-p25\u{ff16}\",\"value\":null}",
            "{\"reason\":\"EngineUnavailable\",\"detail\":null}",
            "{\"reason\":\"engine-unavailable\",\"detail\":null,\"extra\":false}",
            "{\"reason\":\"statement-schema-invalid\",\"detail\":{\"schema_error\":\"MissingTypeReference\",\"detail\":null}}",
            "{\"assurance\":\"production\",\"value\":null}",
            "{\"assurance\":\"Experimental\",\"value\":null}",
        ] {
            let rejected = norito::json::from_json::<PrivacyProtocolIdV1>(hostile).is_err()
                && norito::json::from_json::<PrivacyProofSystemIdV1>(hostile).is_err()
                && norito::json::from_json::<PrivacyEngineIdV1>(hostile).is_err()
                && norito::json::from_json::<PrivacyCompiledProfileUnavailableReasonV1>(hostile)
                    .is_err()
                && norito::json::from_json::<PrivacyAssuranceV1>(hostile).is_err();
            assert!(rejected, "hostile closed-enum JSON {hostile} must fail");
        }
    }

    #[test]
    fn capability_snapshot_roundtrips_and_rejects_structural_adversaries() {
        let snapshot = capability_snapshot();
        snapshot.validate().expect("valid capability snapshot");

        let archive = norito::to_bytes(&snapshot).expect("encode snapshot");
        let decoded: PrivacyCapabilitySnapshotV1 =
            norito::decode_from_bytes(&archive).expect("decode snapshot");
        assert_eq!(decoded, snapshot);
        decoded.validate().expect("validate decoded snapshot");

        let canonical = norito::json::to_json(&snapshot).expect("serialize snapshot JSON");
        let decoded_json: PrivacyCapabilitySnapshotV1 =
            norito::json::from_json(&canonical).expect("decode snapshot JSON");
        assert_eq!(decoded_json, snapshot);
        decoded_json.validate().expect("validate JSON snapshot");

        let unknown = canonical.replacen('{', "{\"unknown\":true,", 1);
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&unknown).is_err(),
            "unknown top-level field must fail"
        );
        let duplicate = canonical.replacen('{', "{\"version\":1,", 1);
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&duplicate).is_err(),
            "duplicate top-level field must fail"
        );

        let assurance_alias = canonical.replacen(
            "\"assurance\":\"experimental\"",
            "\"assurance\":\"production\"",
            1,
        );
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&assurance_alias).is_err(),
            "non-Experimental assurance must fail"
        );

        let pgc_profile = match snapshot.protocols[1].compiled_profile {
            PrivacyCompiledProfileResultV1::Available(profile) => profile,
            PrivacyCompiledProfileResultV1::Unavailable(_) => unreachable!("PGC fixture available"),
        };
        let parameter_json =
            norito::json::to_json(&pgc_profile.parameter_id).expect("serialize fixed bytes");
        let malformed_fixed_bytes = canonical.replacen(
            &format!("\"parameter_id\":{parameter_json}"),
            "\"parameter_id\":[1]",
            1,
        );
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&malformed_fixed_bytes).is_err(),
            "wrong-length fixed bytes must fail"
        );
        let out_of_range_fixed_bytes = canonical.replacen(
            &format!("\"parameter_id\":{parameter_json}"),
            "\"parameter_id\":[256,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]",
            1,
        );
        assert!(
            norito::json::from_json::<PrivacyCapabilitySnapshotV1>(&out_of_range_fixed_bytes)
                .is_err(),
            "out-of-range fixed byte must fail"
        );

        let mut missing = snapshot.clone();
        missing.protocols.pop();
        assert!(matches!(
            missing.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolCount { .. })
        ));

        let mut duplicate_row = snapshot.clone();
        duplicate_row.protocols[2] = duplicate_row.protocols[1];
        assert!(matches!(
            duplicate_row.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolOrder { .. })
        ));

        let mut reordered = snapshot.clone();
        reordered.protocols.swap(0, 1);
        assert!(matches!(
            reordered.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolOrder { .. })
        ));

        let mut embedded_id_mismatch = snapshot.clone();
        embedded_id_mismatch.protocols[2].compiled_profile =
            PrivacyCompiledProfileResultV1::Available(pgc_profile);
        assert!(matches!(
            embedded_id_mismatch.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolRow {
                source: PrivacyCapabilityRowValidationErrorV1::CompiledProfileProtocolMismatch { .. },
                ..
            })
        ));

        let mut activation_profile_mismatch = snapshot.clone();
        activation_profile_mismatch.protocols[1]
            .activation
            .as_mut()
            .expect("PGC activation")
            .parameter_digest = PrivacyParameterDigestV1::new(raw(250));
        assert!(matches!(
            activation_profile_mismatch.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolRow {
                source: PrivacyCapabilityRowValidationErrorV1::ActivationProfileMismatch {
                    field: PrivacyCapabilityBindingFieldV1::ParameterDigest,
                },
                ..
            })
        ));

        let mut unavailable_activation = snapshot;
        unavailable_activation.protocols[2].activation = Some(activation(&envelope(
            statement_for(PrivacyProtocolIdV1::VeRangeTransparentRangeV1),
        )));
        assert!(matches!(
            unavailable_activation.validate(),
            Err(PrivacyCapabilitySnapshotValidationErrorV1::ProtocolRow {
                source: PrivacyCapabilityRowValidationErrorV1::UnavailableActivation { .. },
                ..
            })
        ));
    }

    #[test]
    fn canonical_capability_archive_validator_is_bounded_typed_and_fail_closed() {
        use PrivacyCapabilityArchiveValidationStatusV1 as Status;

        assert_eq!(PRIVACY_BRIDGE_ABI_VERSION_V1, 21);
        assert_eq!(PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1, 256 * 1024);
        assert_eq!(Status::Valid.code(), 0);
        assert_eq!(Status::NullPointer.code(), 1);
        assert_eq!(Status::Empty.code(), 2);
        assert_eq!(Status::ArchiveTooLarge.code(), 3);
        assert_eq!(Status::DecodeResourceLimit.code(), 4);
        assert_eq!(Status::SchemaMismatch.code(), 5);
        assert_eq!(Status::NonCanonical.code(), 6);
        assert_eq!(Status::MalformedArchive.code(), 7);
        assert_eq!(Status::InvalidSnapshot.code(), 8);

        let snapshot = capability_snapshot();
        let archive = norito::encode_canonical(&snapshot).expect("canonical capability archive");
        assert_eq!(
            validate_privacy_capability_archive_v1(&archive),
            Status::Valid
        );

        assert_eq!(validate_privacy_capability_archive_v1(&[]), Status::Empty);
        assert_eq!(
            validate_privacy_capability_archive_v1(&vec![
                0;
                PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1
                    + 1
            ]),
            Status::ArchiveTooLarge
        );
        assert_eq!(
            validate_privacy_capability_archive_v1(&archive[..archive.len() - 1]),
            Status::MalformedArchive
        );

        let mut wrong_schema = archive.clone();
        wrong_schema[6] ^= 0x80;
        assert_eq!(
            validate_privacy_capability_archive_v1(&wrong_schema),
            Status::SchemaMismatch
        );

        // Preserve a valid CRC over a one-byte payload while substituting the
        // expected snapshot schema. Header-only validation used to accept this
        // exact adversary; the typed decoder must reject it.
        let mut one_byte_fake = norito::encode_canonical(&0_u8).expect("canonical one-byte value");
        one_byte_fake[6..22].copy_from_slice(&archive[6..22]);
        assert_eq!(
            validate_privacy_capability_archive_v1(&one_byte_fake),
            Status::MalformedArchive
        );

        let mut reordered = snapshot.clone();
        reordered.protocols.swap(0, 1);
        let reordered =
            norito::encode_canonical(&reordered).expect("canonical reordered snapshot bytes");
        assert_eq!(
            validate_privacy_capability_archive_v1(&reordered),
            Status::InvalidSnapshot
        );

        let mut profile_mutation = snapshot.clone();
        let PrivacyCompiledProfileResultV1::Available(profile) =
            &mut profile_mutation.protocols[1].compiled_profile
        else {
            panic!("PGC fixture must have a compiled profile");
        };
        profile.parameter_digest = PrivacyParameterDigestV1::new([0; 32]);
        let profile_mutation =
            norito::encode_canonical(&profile_mutation).expect("canonical invalid-profile bytes");
        assert_eq!(
            validate_privacy_capability_archive_v1(&profile_mutation),
            Status::InvalidSnapshot
        );

        let mut activation_mutation = snapshot.clone();
        activation_mutation.protocols[1]
            .activation
            .as_mut()
            .expect("PGC fixture activation")
            .parameter_digest = PrivacyParameterDigestV1::new(raw(250));
        let activation_mutation = norito::encode_canonical(&activation_mutation)
            .expect("canonical activation-mismatch bytes");
        assert_eq!(
            validate_privacy_capability_archive_v1(&activation_mutation),
            Status::InvalidSnapshot
        );

        let mut excessive_rows = snapshot;
        excessive_rows.protocols.push(excessive_rows.protocols[0]);
        let excessive_rows =
            norito::encode_canonical(&excessive_rows).expect("canonical excessive-row bytes");
        assert_eq!(
            validate_privacy_capability_archive_v1(&excessive_rows),
            Status::DecodeResourceLimit
        );
    }

    #[test]
    fn all_protocol_mappings_and_typed_variants_are_exact() {
        let statements = sample_statements();
        assert_eq!(statements.len(), PrivacyProtocolIdV1::COUNT);
        for (protocol, statement) in PrivacyProtocolIdV1::ALL.into_iter().zip(statements) {
            assert_eq!(statement.protocol_id(), protocol);
            let proof = proof_for(protocol);
            assert_eq!(proof.protocol_id(), protocol);
            assert_eq!(
                statement_variant_name(&statement),
                protocol.canonical_typed_variant_label()
            );
            assert_eq!(
                proof_variant_name(&proof),
                protocol.canonical_typed_variant_label()
            );
            assert_eq!(
                protocol_limits(protocol).protocol_id(),
                protocol,
                "activation limits must carry the same closed protocol tag"
            );
        }
        assert_eq!(
            PrivacyProtocolIdV1::VeRangeTransparentRangeV1.expected_proof_system(),
            PrivacyProofSystemIdV1::IrohaVeRangeP256
        );
        assert_eq!(
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0.expected_proof_system(),
            PrivacyProofSystemIdV1::JindoPolynomialCommitment
        );
        assert_eq!(
            PrivacyProtocolIdV1::IrohaZkAmsV1.expected_proof_system(),
            PrivacyProofSystemIdV1::ZkAmsMaskedRelaxedSpartanT256Ristretto255Sha3_512
        );
        assert_eq!(
            PrivacyProtocolIdV1::IrohaZkAmsV1.expected_engine(),
            PrivacyEngineIdV1::NativeZkAmsMaskedRelaxedSpartanT256Ristretto255
        );
        for protocol in [
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
        ] {
            assert_eq!(
                protocol.expected_proof_system(),
                PrivacyProofSystemIdV1::StarkFriSha256Goldilocks,
                "{protocol:?} must identify the SHA-256 transcript/Merkle STARK"
            );
        }
    }

    #[test]
    #[ignore = "explicit regeneration helper for fixtures/privacy/exact12_v1.tsv"]
    fn emit_exact12_typed_envelope_fixture_rows() {
        for row in privacy_exact12_typed_envelope_rows_v1().expect("compiled exact12 semantics") {
            println!(
                "typed-envelope\t{}\t{}\t{}\t{}\t{}",
                row.protocol_id.canonical_label(),
                row.statement_variant,
                row.proof_variant,
                hex::encode(row.statement_digest),
                hex::encode(row.envelope_sha256)
            );
        }
    }

    #[test]
    fn exact12_cross_sdk_matrix_binds_registry_routes_and_typed_envelopes() {
        let matrix = include_str!("../../../fixtures/privacy/exact12_v1.tsv");
        assert!(matrix.ends_with('\n'), "matrix must end with one LF");
        assert!(!matrix.contains('\r'), "matrix must use canonical LF lines");
        assert!(
            matrix
                .strip_suffix('\n')
                .expect("terminal LF")
                .lines()
                .all(|line| !line.is_empty()),
            "matrix must not contain empty rows"
        );

        let mut matrix_version = None;
        let mut registry_sha256 = None;
        let mut protocols = Vec::new();
        let mut typed_envelopes = Vec::new();
        let mut retired = Vec::new();
        for (line_index, line) in matrix.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields = line.split('\t').collect::<Vec<_>>();
            match fields.as_slice() {
                ["matrix-version", version] => {
                    assert!(
                        matrix_version.replace(*version).is_none(),
                        "duplicate version"
                    );
                }
                ["registry-sha256", digest] => {
                    assert!(
                        registry_sha256.replace(*digest).is_none(),
                        "duplicate registry digest"
                    );
                }
                ["protocol", index, label, statement_variant, proof_variant] => {
                    protocols.push((
                        index.parse::<usize>().expect("decimal protocol index"),
                        *label,
                        *statement_variant,
                        *proof_variant,
                    ));
                }
                [
                    "typed-envelope",
                    label,
                    statement_variant,
                    proof_variant,
                    statement_digest,
                    envelope_sha256,
                ] => typed_envelopes.push((
                    *label,
                    *statement_variant,
                    *proof_variant,
                    *statement_digest,
                    *envelope_sha256,
                )),
                ["retired", label] => retired.push(*label),
                _ => panic!("malformed exact12 matrix row {}", line_index + 1),
            }
        }

        assert_eq!(matrix_version, Some("1"));
        assert_eq!(protocols.len(), PrivacyProtocolIdV1::COUNT);
        assert_eq!(typed_envelopes.len(), PrivacyProtocolIdV1::COUNT);
        assert!(!retired.is_empty());
        let semantic_rows =
            privacy_exact12_typed_envelope_rows_v1().expect("compiled exact12 semantics");
        assert_eq!(semantic_rows.len(), PrivacyProtocolIdV1::COUNT);
        let mut registry_preimage = String::new();
        let mut unique_labels = std::collections::BTreeSet::new();
        for (expected_index, (protocol, semantic)) in PrivacyProtocolIdV1::ALL
            .into_iter()
            .zip(&semantic_rows)
            .enumerate()
        {
            let (index, label, expected_statement_variant, expected_proof_variant) =
                protocols[expected_index];
            assert_eq!(index, expected_index);
            assert_eq!(label, protocol.canonical_label());
            assert_eq!(semantic.protocol_id, protocol);
            assert_eq!(
                PrivacyProtocolIdV1::from_canonical_label(label),
                Some(protocol)
            );
            assert!(unique_labels.insert(label), "duplicate protocol label");
            assert_eq!(semantic.statement_variant, expected_statement_variant);
            assert_eq!(semantic.proof_variant, expected_proof_variant);
            registry_preimage.push_str(label);
            registry_preimage.push('\n');
        }
        assert_eq!(
            hex::encode(Sha256::digest(registry_preimage.as_bytes())),
            registry_sha256.expect("registry digest")
        );

        for (semantic, row) in semantic_rows.iter().zip(typed_envelopes) {
            let (
                label,
                expected_statement_variant,
                expected_proof_variant,
                expected_statement_digest,
                expected_envelope_sha256,
            ) = row;
            assert_eq!(
                PrivacyProtocolIdV1::from_canonical_label(label),
                Some(semantic.protocol_id)
            );
            assert_eq!(semantic.statement_variant, expected_statement_variant);
            assert_eq!(semantic.proof_variant, expected_proof_variant);
            for expected_digest in [expected_statement_digest, expected_envelope_sha256] {
                assert_eq!(expected_digest.len(), 64);
                assert!(
                    expected_digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
                    "typed envelope digest must be canonical lowercase hex"
                );
                assert!(
                    expected_digest.bytes().any(|byte| byte != b'0'),
                    "typed envelope digest must not be the zero placeholder"
                );
            }
            assert_eq!(
                hex::encode(semantic.statement_digest),
                expected_statement_digest
            );
            assert_eq!(
                hex::encode(semantic.envelope_sha256),
                expected_envelope_sha256
            );
        }

        let mut unique_retired = std::collections::BTreeSet::new();
        for label in retired {
            assert!(unique_retired.insert(label), "duplicate retired label");
            assert!(
                PrivacyProtocolIdV1::from_canonical_label(label).is_none(),
                "retired label {label:?} must remain unrepresentable"
            );
        }
    }

    #[test]
    fn exact12_compiled_semantics_are_closed_unique_and_context_bound() {
        let rows = privacy_exact12_typed_envelope_rows_v1().expect("compiled exact12 semantics");
        assert_eq!(rows.len(), PrivacyProtocolIdV1::COUNT);
        assert_eq!(
            rows.iter().map(|row| row.protocol_id).collect::<Vec<_>>(),
            PrivacyProtocolIdV1::ALL.to_vec()
        );
        assert!(rows.iter().all(|row| {
            row.statement_variant == row.protocol_id.canonical_typed_variant_label()
                && row.proof_variant == row.protocol_id.canonical_typed_variant_label()
                && row.statement_digest != [0; 32]
                && row.envelope_sha256 != [0; 32]
        }));
        assert_eq!(
            rows.iter()
                .map(|row| row.statement_digest)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            PrivacyProtocolIdV1::COUNT
        );
        assert_eq!(
            rows.iter()
                .map(|row| row.envelope_sha256)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            PrivacyProtocolIdV1::COUNT
        );

        let mut mutated = statement_for(PrivacyProtocolIdV1::ZkAcePqAuthorizationV0);
        let PrivacyStatementV1::ZkAcePqAuthorizationV0(statement) = &mut mutated else {
            unreachable!("closed first row is ZK-ACE")
        };
        statement.context.action_index = 1;
        let mutated_statement_digest = mutated.digest().expect("mutated statement digest");
        let mutated_envelope_sha256: [u8; 32] = Sha256::digest(
            norito::encode_canonical(&envelope(mutated)).expect("mutated canonical envelope"),
        )
        .into();
        assert_ne!(
            *mutated_statement_digest.as_bytes(),
            rows[0].statement_digest
        );
        assert_ne!(mutated_envelope_sha256, rows[0].envelope_sha256);
    }

    #[test]
    fn fixed_digest_types_are_exact_and_nonzero_checked() {
        macro_rules! check_type {
            ($type:ident, $seed:expr) => {{
                let value = $type::new(raw($seed));
                assert_eq!(value.as_bytes(), &raw($seed));
                assert_eq!(value.into_bytes(), raw($seed));
                assert!(!value.is_zero());
                assert!($type::new([0; 32]).is_zero());
                let encoded = value.encode();
                // Bare Norito encodes a fixed byte-array field as one
                // canonical compact-width prefix followed by exactly 32
                // bytes. No variable-length payload is admitted.
                assert_eq!(encoded.len(), 33);
                assert_eq!(
                    $type::decode(&mut encoded.as_slice()).expect("decode fixed value"),
                    value
                );
            }};
        }
        check_type!(PrivacyParameterIdV1, 1);
        check_type!(PrivacyParameterDigestV1, 2);
        check_type!(PrivacyVerifierDigestV1, 3);
        check_type!(PrivacyStatementSchemaDigestV1, 4);
        check_type!(PrivacyEngineManifestDigestV1, 5);
        check_type!(PrivacyStatementDigestV1, 6);
        check_type!(PrivacyTransactionIntentDigestV1, 17);
        check_type!(PrivacyOrchardPoolBootstrapDigestV1, 20);
        check_type!(PrivacyProofManagedPoolBootstrapDigestV1, 21);
        check_type!(PrivacyFcmpOutputIdV1, 22);
        check_type!(PrivacyFcmpKeyImageV1, 23);
        check_type!(PrivacyBootleLanternIssuerPolicyDigestV1, 18);
        check_type!(PrivacyNullifierV1, 7);
        check_type!(PrivacyCommitmentV1, 8);
        check_type!(PrivacyPoolIdV1, 9);
        check_type!(PrivacyZkAmsRegistryIdV1, 10);
        check_type!(PrivacyPolicyIdV1, 10);
        check_type!(PrivacyRootV1, 11);
        check_type!(PrivacyChallengeV1, 12);
        check_type!(PrivacyZkAmsPhcHashV1, 13);
        check_type!(PrivacyZkAmsSubjectCommitmentV1, 14);
        check_type!(PrivacyZkAmsCredentialNonceV1, 15);
        check_type!(PrivacyZkAmsIssuerPolicyRecordDigestV1, 16);
        check_type!(PrivacyZkAmsRegistryRecordDigestV1, 19);
    }

    #[test]
    fn all_statements_and_envelopes_roundtrip_and_validate() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        for statement in sample_statements() {
            statement.validate(&limits).expect("valid typed statement");
            let statement_bytes = norito::to_bytes(&statement).expect("frame statement");
            let decoded_statement: PrivacyStatementV1 =
                norito::decode_from_bytes(&statement_bytes).expect("decode statement");
            assert_eq!(decoded_statement, statement);
            assert_eq!(
                decoded_statement.digest().expect("decoded digest"),
                statement.digest().expect("original digest")
            );

            let envelope = envelope(statement);
            envelope
                .validate_with_limits(&limits)
                .expect("valid intrinsic envelope");
            let activation = activation(&envelope);
            activation.validate().expect("valid activation");
            envelope
                .validate_against_activation(&activation, &limits, 2)
                .expect("valid active envelope");

            let bytes = norito::to_bytes(&envelope).expect("frame envelope");
            let decoded: PrivacyProofEnvelopeV1 =
                norito::decode_from_bytes(&bytes).expect("decode envelope");
            assert_eq!(decoded, envelope);
        }
    }

    #[test]
    fn normalization_accessors_cover_every_statement_and_nested_proof_variant() {
        for mut statement in sample_statements() {
            let replacement = PrivacyTransactionIntentDigestV1::new(raw(231));
            statement.context_mut().transaction_intent_digest = replacement;
            assert_eq!(statement.context().transaction_intent_digest, replacement);
        }

        let mut batch =
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(
                PrivacyProofBytesV1::new(vec![1, 2, 3]),
            ));
        batch.bytes_mut().bytes.clear();
        assert!(batch.bytes().as_bytes().is_empty());

        let mut provisioning =
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::Ristretto255LsagProvisionAccount(
                PrivacyProofBytesV1::new(vec![4, 5, 6]),
            ));
        provisioning.bytes_mut().bytes.clear();
        assert!(provisioning.bytes().as_bytes().is_empty());

        for mut proof in sample_statements()
            .into_iter()
            .map(envelope)
            .map(|envelope| envelope.proof)
        {
            proof.bytes_mut().bytes.clear();
            assert!(proof.bytes().as_bytes().is_empty());
        }
    }

    #[test]
    fn zk_ams_envelope_requires_the_proof_variant_for_its_exact_action() {
        let limits = PrivacyConsensusLimitsV1::taira_default();

        let batch_statement = statement_for(PrivacyProtocolIdV1::IrohaZkAmsV1);
        let mut batch_envelope = envelope(batch_statement);
        batch_envelope
            .validate_with_limits(&limits)
            .expect("batch masked Relaxed Spartan proof variant");
        batch_envelope.proof = PrivacyProofV1::IrohaZkAmsV1(
            IrohaZkAmsProofV1::Ristretto255LsagProvisionAccount(PrivacyProofBytesV1::new(vec![1])),
        );
        assert!(matches!(
            batch_envelope.validate_with_limits(&limits),
            Err(PrivacyProofEnvelopeValidationError::ZkAmsActionProofMismatch)
        ));

        let provision_statement = zk_ams_provision_statement(16);
        let mut provision_envelope = envelope(provision_statement);
        provision_envelope
            .validate_with_limits(&limits)
            .expect("provisioning LSAG proof variant");
        provision_envelope.proof =
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(
                PrivacyProofBytesV1::new(vec![1]),
            ));
        assert!(matches!(
            provision_envelope.validate_with_limits(&limits),
            Err(PrivacyProofEnvelopeValidationError::ZkAmsActionProofMismatch)
        ));
    }

    #[test]
    fn p256_wire_types_are_exact_width_and_closed() {
        let point = p256_point(9);
        let encoded = point.encode();
        assert_eq!(encoded.len(), 34);
        assert_eq!(
            PrivacyP256PointV1::decode(&mut encoded.as_slice()).expect("decode exact point"),
            point
        );
        assert!(PrivacyP256PointV1::decode(&mut [0x02; 32].as_slice()).is_err());
        assert!(PrivacyP256PointV1::decode(&mut [0x02; 34].as_slice()).is_err());

        let ciphertext = p256_ciphertext(10);
        let bytes = norito::to_bytes(&ciphertext).expect("frame ciphertext");
        let decoded: PrivacyP256CiphertextV1 =
            norito::decode_from_bytes(&bytes).expect("decode ciphertext");
        assert_eq!(decoded, ciphertext);

        for unknown in [2_u32, 3, u32::MAX] {
            assert!(
                PrivacyVeRangeBitLengthV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err()
            );
        }
    }

    #[test]
    fn zk_ams_ristretto_wire_types_and_action_tags_are_closed() {
        let seed_key = zk_ams_seed_key(9);
        let encoded = seed_key.encode();
        assert_eq!(encoded.len(), 33);
        assert_eq!(
            PrivacyZkAmsSeedPublicKeyV1::decode(&mut encoded.as_slice())
                .expect("decode exact seed key"),
            seed_key
        );
        assert!(PrivacyZkAmsSeedPublicKeyV1::decode(&mut [9; 31].as_slice()).is_err());
        assert!(PrivacyZkAmsSeedPublicKeyV1::decode(&mut [9; 33].as_slice()).is_err());

        let key_image = PrivacyZkAmsKeyImageV1::new(raw(10));
        assert_eq!(key_image.encode().len(), 33);
        assert!(PrivacyZkAmsKeyImageV1::decode(&mut key_image.encode().as_slice()).is_ok());

        for unknown in [2_u32, 3, u32::MAX] {
            assert!(PrivacyZkAmsActionV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
            assert!(IrohaZkAmsProofV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
        }
    }

    #[test]
    fn context_rejects_unusable_chain_ids_and_action_indexes() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let mut value = context();
        value.chain_id = ChainId::from("");
        assert!(matches!(
            value.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidChainIdLength { bytes: 0, .. })
        ));

        value = context();
        value.chain_id = ChainId::from("x".repeat(255));
        value.validate(&limits).expect("255-byte chain id");
        value.chain_id = ChainId::from("x".repeat(256));
        assert!(matches!(
            value.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidChainIdLength { bytes: 256, .. })
        ));

        value = context();
        value.action_index = 1;
        assert!(matches!(
            value.validate(&limits),
            Err(PrivacyStatementValidationError::ActionIndexOutOfBounds {
                index: 1,
                max_actions: 1
            })
        ));

        value = context();
        value.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new([0; 32]);
        assert_eq!(
            value.validate(&limits),
            Err(PrivacyStatementValidationError::ZeroTransactionIntentDigest)
        );
    }

    #[test]
    fn privacy_context_statement_proof_and_envelope_json_are_closed() {
        let context = context();
        let context_json = norito::json::to_json(&context).expect("encode privacy context JSON");
        let context_prefix = context_json
            .strip_suffix('}')
            .expect("privacy context JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyStatementContextV1>(&format!(
                "{context_prefix},\"legacy_context\":true}}"
            ))
            .is_err()
        );

        let envelope = envelope(statement_for(
            PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1,
        ));
        let envelope_json = norito::json::to_json(&envelope).expect("encode privacy envelope JSON");
        let decoded: PrivacyProofEnvelopeV1 =
            norito::json::from_json(&envelope_json).expect("decode canonical envelope JSON");
        assert_eq!(decoded, envelope);
        let envelope_prefix = envelope_json
            .strip_suffix('}')
            .expect("privacy envelope JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyProofEnvelopeV1>(&format!(
                "{envelope_prefix},\"legacy_envelope\":true}}"
            ))
            .is_err()
        );

        let statement_json =
            norito::json::to_json(&envelope.statement).expect("encode typed statement JSON");
        let statement_prefix = statement_json
            .strip_suffix('}')
            .expect("typed statement JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyStatementV1>(&format!(
                "{statement_prefix},\"legacy_statement\":true}}"
            ))
            .is_err()
        );

        let proof_json = norito::json::to_json(&envelope.proof).expect("encode typed proof JSON");
        let proof_prefix = proof_json
            .strip_suffix('}')
            .expect("typed proof JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyProofV1>(&format!(
                "{proof_prefix},\"legacy_proof\":true}}"
            ))
            .is_err()
        );
    }

    #[test]
    fn first_release_statements_reject_nested_unknown_json_fields() {
        for protocol_id in PrivacyProtocolIdV1::ALL {
            let statement = statement_for(protocol_id);
            let canonical =
                norito::json::to_json(&statement).expect("encode private-transfer statement JSON");
            let nested_prefix = canonical
                .strip_suffix("}}")
                .expect("tagged statement ends with nested and outer objects");
            let hostile = format!("{nested_prefix},\"legacy_transfer\":true}}}}");
            assert!(
                norito::json::from_json::<PrivacyStatementV1>(&hostile).is_err(),
                "nested unknown field must fail for {protocol_id:?}"
            );
        }
        for (protocol_id, removed_field) in [
            (PrivacyProtocolIdV1::ZkAcePqAuthorizationV0, "fee"),
            (PrivacyProtocolIdV1::OrchardHalo2ActionsV1, "fee"),
            (PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1, "fee"),
            (PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1, "fee"),
            (PrivacyProtocolIdV1::PqMaspStarkV0, "fee"),
            (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                "next_state_root",
            ),
            (
                PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
                "next_state_root_epoch",
            ),
            (PrivacyProtocolIdV1::PqMaspStarkV0, "next_anchor"),
            (PrivacyProtocolIdV1::PqMaspStarkV0, "next_anchor_epoch"),
        ] {
            let canonical = norito::json::to_json(&statement_for(protocol_id))
                .expect("encode validator-derived-successor statement");
            let nested_prefix = canonical
                .strip_suffix("}}")
                .expect("tagged statement ends with nested and outer objects");
            let hostile = format!("{nested_prefix},\"{removed_field}\":null}}}}");
            assert!(
                norito::json::from_json::<PrivacyStatementV1>(&hostile).is_err(),
                "removed caller-selected field `{removed_field}` must not decode"
            );
        }

        let authorization = norito::json::to_json(&PrivacyPqAuthorizationProfileV1::MlDsa65)
            .expect("encode PQ authorization profile");
        let authorization_prefix = authorization
            .strip_suffix('}')
            .expect("PQ authorization profile is an object");
        assert!(
            norito::json::from_json::<PrivacyPqAuthorizationProfileV1>(&format!(
                "{authorization_prefix},\"legacy\":null}}"
            ))
            .is_err()
        );

        let encryption =
            norito::json::to_json(&PrivacyPqNoteEncryptionProfileV1::MlKem768XChaCha20Poly1305)
                .expect("encode PQ note-encryption profile");
        let encryption_prefix = encryption
            .strip_suffix('}')
            .expect("PQ note-encryption profile is an object");
        assert!(
            norito::json::from_json::<PrivacyPqNoteEncryptionProfileV1>(&format!(
                "{encryption_prefix},\"legacy\":null}}"
            ))
            .is_err()
        );

        let encrypted_note =
            norito::json::to_json(&encrypted_output(31, 32)).expect("encode encrypted note");
        let encrypted_note_prefix = encrypted_note
            .strip_suffix('}')
            .expect("encrypted note is an object");
        assert!(
            norito::json::from_json::<PrivacyEncryptedOutputV1>(&format!(
                "{encrypted_note_prefix},\"legacy_ciphertext\":null}}"
            ))
            .is_err(),
            "nested private-note encrypted output must reject unknown fields"
        );

        let output = fcmp_output(33);
        let fcmp_encrypted_output = PrivacyFcmpEncryptedOutputV1 {
            recipient: PrivacyRecipientIdV1::new(raw(34)),
            ephemeral_public_key: PrivacyEncryptionKeyV1::new(raw(35)),
            output_id: output.output_id(),
            ciphertext: vec![0xA5],
        };
        let fcmp_encrypted_output_json =
            norito::json::to_json(&fcmp_encrypted_output).expect("encode FCMP++ encrypted output");
        let fcmp_encrypted_output_prefix = fcmp_encrypted_output_json
            .strip_suffix('}')
            .expect("FCMP++ encrypted output is an object");
        assert!(
            norito::json::from_json::<PrivacyFcmpEncryptedOutputV1>(&format!(
                "{fcmp_encrypted_output_prefix},\"legacy_output_id\":null}}"
            ))
            .is_err(),
            "nested FCMP++ encrypted output must reject unknown fields"
        );
    }

    #[test]
    fn taira_consensus_limits_reject_zero_overflow_and_inconsistent_profiles() {
        let defaults = PrivacyConsensusLimitsV1::taira_default();
        defaults.validate().expect("Taira defaults");
        assert_eq!(
            defaults.max_actions_per_transaction,
            TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1
        );
        assert_eq!(defaults.max_proof_bytes_per_action, 9 * 1024 * 1024);
        assert_eq!(defaults.max_action_bytes, 9 * 1024 * 1024);
        assert_eq!(defaults.max_privacy_bytes_per_transaction, 9 * 1024 * 1024);
        assert_eq!(defaults.max_privacy_bytes_per_block, 18 * 1024 * 1024);
        assert_eq!(
            defaults.max_commitments_per_action,
            TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
        );

        let invalid = [
            {
                let mut value = defaults;
                value.max_actions_per_transaction = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_actions_per_block = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_proof_bytes_per_action = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_action_bytes = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_privacy_bytes_per_transaction = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_privacy_bytes_per_block = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_statement_and_encrypted_output_bytes_per_transaction = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_nullifiers_per_action = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_commitments_per_action = 0;
                value
            },
            {
                let mut value = defaults;
                value.retained_root_count = 0;
                value
            },
            {
                let mut value = defaults;
                value.max_commitments_per_action = TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1 + 1;
                value
            },
            {
                let mut value = defaults;
                value.max_action_bytes = defaults.max_proof_bytes_per_action - 1;
                value
            },
        ];
        for value in invalid {
            assert!(
                value.validate().is_err(),
                "mutated limits must fail: {value:?}"
            );
        }

        let hard_maximum_mutations: [(PrivacyLimitFieldV1, u32, fn(&mut PrivacyConsensusLimitsV1));
            4] = [
            (
                PrivacyLimitFieldV1::ProofBytesPerAction,
                TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
                |value| value.max_proof_bytes_per_action += 1,
            ),
            (
                PrivacyLimitFieldV1::ActionBytes,
                TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
                |value| value.max_action_bytes += 1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerTransaction,
                TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1,
                |value| value.max_privacy_bytes_per_transaction += 1,
            ),
            (
                PrivacyLimitFieldV1::PrivacyBytesPerBlock,
                TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1,
                |value| value.max_privacy_bytes_per_block += 1,
            ),
        ];
        for (field, hard_max, mutate) in hard_maximum_mutations {
            let mut value = defaults;
            mutate(&mut value);
            assert_eq!(
                value.validate(),
                Err(PrivacyConsensusLimitsValidationError::ExceedsHardMaximum {
                    field,
                    value: hard_max + 1,
                    hard_max,
                })
            );
        }
    }

    #[test]
    fn privacy_proof_payload_admits_exact_nine_mib_and_rejects_cap_plus_one() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let maximum =
            usize::try_from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1).expect("cap fits usize");
        let mut proof = PrivacyProofBytesV1::new(vec![0xA5; maximum]);
        proof.validate(&limits).expect("exact 9 MiB proof payload");

        proof.bytes.push(0x5A);
        assert_eq!(
            proof.validate(&limits),
            Err(PrivacyProofValidationError::TooLarge {
                bytes: u64::from(TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1) + 1,
                max: TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1,
            })
        );
    }

    #[test]
    fn consensus_limit_tightening_is_strict_and_rejects_every_component_increase() {
        let current = PrivacyConsensusLimitsV1 {
            max_actions_per_transaction: 1,
            max_actions_per_block: 1,
            max_proof_bytes_per_action: 1_024,
            max_action_bytes: 2_048,
            max_privacy_bytes_per_transaction: 4_096,
            max_privacy_bytes_per_block: 8_192,
            max_statement_and_encrypted_output_bytes_per_transaction: 1_024,
            max_nullifiers_per_action: 4,
            max_commitments_per_action: 4,
            retained_root_count: 100,
        };
        current.validate().expect("lower valid current profile");

        assert!(matches!(
            current.validate_tightening_to(&current),
            Err(PrivacyConsensusLimitsTighteningErrorV1::NoChange)
        ));
        let mut strict = current;
        strict.retained_root_count -= 1;
        current
            .validate_tightening_to(&strict)
            .expect("one component may be lowered");

        let mutations: [(PrivacyLimitFieldV1, fn(&mut PrivacyConsensusLimitsV1)); 10] = [
            (PrivacyLimitFieldV1::ActionsPerTransaction, |value| {
                value.max_actions_per_transaction += 1
            }),
            (PrivacyLimitFieldV1::ActionsPerBlock, |value| {
                value.max_actions_per_block += 1;
            }),
            (PrivacyLimitFieldV1::ProofBytesPerAction, |value| {
                value.max_proof_bytes_per_action += 1;
            }),
            (PrivacyLimitFieldV1::ActionBytes, |value| {
                value.max_action_bytes += 1;
            }),
            (PrivacyLimitFieldV1::PrivacyBytesPerTransaction, |value| {
                value.max_privacy_bytes_per_transaction += 1
            }),
            (PrivacyLimitFieldV1::PrivacyBytesPerBlock, |value| {
                value.max_privacy_bytes_per_block += 1;
            }),
            (
                PrivacyLimitFieldV1::StatementAndEncryptedOutputBytesPerTransaction,
                |value| value.max_statement_and_encrypted_output_bytes_per_transaction += 1,
            ),
            (PrivacyLimitFieldV1::NullifiersPerAction, |value| {
                value.max_nullifiers_per_action += 1;
            }),
            (PrivacyLimitFieldV1::CommitmentsPerAction, |value| {
                value.max_commitments_per_action += 1;
            }),
            (PrivacyLimitFieldV1::RetainedRootCount, |value| {
                value.retained_root_count += 1;
            }),
        ];
        for (field, mutate) in mutations {
            let mut candidate = current;
            mutate(&mut candidate);
            let error = current
                .validate_tightening_to(&candidate)
                .expect_err("an increased component must fail closed");
            if field == PrivacyLimitFieldV1::ActionsPerTransaction {
                assert!(matches!(
                    error,
                    PrivacyConsensusLimitsTighteningErrorV1::InvalidNext(
                        PrivacyConsensusLimitsValidationError::ExceedsHardMaximum {
                            field: PrivacyLimitFieldV1::ActionsPerTransaction,
                            ..
                        }
                    )
                ));
            } else {
                assert!(matches!(
                    error,
                    PrivacyConsensusLimitsTighteningErrorV1::Increase {
                        field: actual,
                        ..
                    } if actual == field
                ));
            }
        }

        let mut mixed = strict;
        mixed.max_actions_per_block += 1;
        assert!(matches!(
            current.validate_tightening_to(&mixed),
            Err(PrivacyConsensusLimitsTighteningErrorV1::Increase {
                field: PrivacyLimitFieldV1::ActionsPerBlock,
                ..
            })
        ));
    }

    #[test]
    fn consensus_policy_schedule_enforces_exact_notice_and_snapshot_boundaries() {
        let current_limits = PrivacyConsensusLimitsV1::taira_default();
        let mut next_limits = current_limits;
        next_limits.max_actions_per_block -= 1;
        next_limits.retained_root_count -= 1;
        let valid = PrivacyConsensusPolicyTighteningV1 {
            scheduled_at_height: 100,
            effective_at_height: 100 + MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1,
            next_limits,
        };
        valid
            .validate_against(&current_limits)
            .expect("exact +300 schedule");

        for invalid in [
            PrivacyConsensusPolicyTighteningV1 {
                scheduled_at_height: 0,
                ..valid
            },
            PrivacyConsensusPolicyTighteningV1 {
                effective_at_height: 99,
                ..valid
            },
            PrivacyConsensusPolicyTighteningV1 {
                effective_at_height: 100,
                ..valid
            },
            PrivacyConsensusPolicyTighteningV1 {
                effective_at_height: valid.effective_at_height - 1,
                ..valid
            },
            PrivacyConsensusPolicyTighteningV1 {
                scheduled_at_height: u64::MAX - 100,
                effective_at_height: u64::MAX,
                ..valid
            },
        ] {
            assert!(
                invalid.validate_against(&current_limits).is_err(),
                "invalid schedule must reject: {invalid:?}"
            );
        }

        let policy = PrivacyConsensusPolicyV1 {
            current_limits,
            pending_tightening: Some(valid),
        };
        assert!(matches!(
            policy.validate_at_committed_height(99),
            Err(
                PrivacyPolicyValidationErrorV1::PendingScheduledAfterCommitted {
                    scheduled_at_height: 100,
                    committed_height: 99
                }
            )
        ));
        policy
            .validate_at_committed_height(100)
            .expect("schedule exists in its admitting committed block");
        policy
            .validate_at_committed_height(valid.effective_at_height - 1)
            .expect("effective E remains pending in committed E-1");
        assert!(matches!(
            policy.validate_at_committed_height(valid.effective_at_height),
            Err(PrivacyPolicyValidationErrorV1::PendingNotFuture {
                effective_at_height,
                committed_height
            }) if effective_at_height == valid.effective_at_height
                && committed_height == valid.effective_at_height
        ));
        assert_eq!(
            policy.admission_retained_root_count(),
            next_limits.retained_root_count
        );
    }

    #[test]
    fn protocol_limit_schedule_rejects_bad_timing_mismatch_increase_and_noop() {
        let current = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 8,
            },
        );
        let next = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 7,
            },
        );
        let valid = PrivacyProtocolLimitsTighteningV1 {
            scheduled_at_height: 25,
            effective_at_height: 25 + MIN_PRIVACY_POLICY_DELAY_BLOCKS_V1,
            next_limits: next,
        };
        valid
            .validate_against(&current)
            .expect("exact delayed protocol tightening");

        assert!(matches!(
            PrivacyProtocolLimitsTighteningV1 {
                next_limits: current,
                ..valid
            }
            .validate_against(&current),
            Err(PrivacyProtocolLimitsTighteningValidationErrorV1::NoChange)
        ));
        assert!(matches!(
            PrivacyProtocolLimitsTighteningV1 {
                effective_at_height: valid.effective_at_height - 1,
                ..valid
            }
            .validate_against(&current),
            Err(PrivacyProtocolLimitsTighteningValidationErrorV1::Schedule(
                PrivacyPolicyValidationErrorV1::LeadTimeTooShort { .. }
            ))
        ));
        assert!(matches!(
            PrivacyProtocolLimitsTighteningV1 {
                next_limits: PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(
                    OrchardActivationLimitsV1 {
                        max_action_count: 1,
                    }
                ),
                ..valid
            }
            .validate_against(&current),
            Err(PrivacyProtocolLimitsTighteningValidationErrorV1::Limits(
                PrivacyProtocolActivationLimitsValidationError::ProtocolMismatch { .. }
            ))
        ));

        let lower_current = next;
        assert!(matches!(
            PrivacyProtocolLimitsTighteningV1 {
                next_limits: current,
                ..valid
            }
            .validate_against(&lower_current),
            Err(PrivacyProtocolLimitsTighteningValidationErrorV1::Limits(
                PrivacyProtocolActivationLimitsValidationError::ExceedsConfiguredCeiling {
                    field: PrivacyActivationLimitFieldV1::VeRangeAggregationCount,
                    value: 8,
                    ceiling: 7
                }
            ))
        ));
    }

    #[test]
    fn namespaces_root_roles_and_publications_are_closed_and_typed() {
        for statement in sample_statements() {
            let namespace = PrivacyNamespaceV1::from_statement(&statement);
            namespace.validate().expect("derived namespace");
            assert_eq!(namespace.protocol_id(), statement.protocol_id());
        }

        let incompatible = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            PrivacyNamespaceScopeV1::Pool(PrivacyPoolNamespaceV1 {
                pool_id: PrivacyPoolIdV1::new(raw(1)),
            }),
        );
        assert!(matches!(
            incompatible.validate(),
            Err(PrivacyNamespaceValidationError::IncompatibleScope { .. })
        ));
        let zero = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            PrivacyNamespaceScopeV1::Policy(PrivacyPolicyNamespaceV1 {
                policy_id: PrivacyPolicyIdV1::new([0; 32]),
            }),
        );
        assert!(matches!(
            zero.validate(),
            Err(PrivacyNamespaceValidationError::ZeroComponent { .. })
        ));

        let x509_trust_anchor_namespace = PrivacyNamespaceV1::new(
            PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            PrivacyNamespaceScopeV1::TrustAnchor(PrivacyTrustAnchorNamespaceV1 {
                trust_anchor_id: PrivacyIssuerIdV1::new(raw(61)),
            }),
        );
        x509_trust_anchor_namespace
            .validate()
            .expect("X.509 trust-anchor namespace");
        let encoded =
            norito::to_bytes(&x509_trust_anchor_namespace).expect("encode trust-anchor namespace");
        let decoded: PrivacyNamespaceV1 =
            norito::decode_from_bytes(&encoded).expect("decode trust-anchor namespace");
        assert_eq!(decoded, x509_trust_anchor_namespace);
        let json = norito::json::to_json(&x509_trust_anchor_namespace)
            .expect("encode trust-anchor namespace JSON");
        let decoded_json: PrivacyNamespaceV1 =
            norito::json::from_json(&json).expect("decode trust-anchor namespace JSON");
        assert_eq!(decoded_json, x509_trust_anchor_namespace);

        let x509_statement = statement_for(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0);
        let x509_policy_namespace = PrivacyNamespaceV1::from_statement(&x509_statement);
        assert!(matches!(
            x509_policy_namespace.scope(),
            PrivacyNamespaceScopeV1::TrustAnchorPolicy(_)
        ));
        let ca_publication = PrivacyRootPublicationV1::new(
            x509_trust_anchor_namespace,
            PrivacyRootRoleV1::CertificateAuthorityMembership,
            1,
            PrivacyRootV1::new(raw(170)),
        )
        .expect("CA root uses the trust-anchor-wide namespace");
        ca_publication
            .validate()
            .expect("canonical CA root publication");
        assert!(matches!(
            PrivacyRootPublicationV1::new(
                x509_policy_namespace,
                PrivacyRootRoleV1::CertificateAuthorityMembership,
                1,
                PrivacyRootV1::new(raw(172)),
            ),
            Err(
                PrivacyRootPublicationValidationError::IncompatibleNamespaceScope {
                    role: PrivacyRootRoleV1::CertificateAuthorityMembership,
                    ..
                }
            )
        ));
        assert!(matches!(
            PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
                PrivacyNamespaceScopeV1::TrustAnchor(PrivacyTrustAnchorNamespaceV1 {
                    trust_anchor_id: PrivacyIssuerIdV1::new(raw(61)),
                }),
            )
            .validate(),
            Err(PrivacyNamespaceValidationError::IncompatibleScope {
                protocol_id: PrivacyProtocolIdV1::ZkAcePqAuthorizationV0,
            })
        ));
        assert_eq!(
            PrivacyNamespaceV1::new(
                PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                PrivacyNamespaceScopeV1::TrustAnchor(PrivacyTrustAnchorNamespaceV1 {
                    trust_anchor_id: PrivacyIssuerIdV1::new([0; 32]),
                }),
            )
            .validate(),
            Err(PrivacyNamespaceValidationError::ZeroComponent {
                component: PrivacyNamespaceComponentV1::Issuer,
            })
        );

        for role in [
            PrivacyRootRoleV1::PgcAccountState,
            PrivacyRootRoleV1::AccountRegistry,
            PrivacyRootRoleV1::NoteCommitmentAnchor,
            PrivacyRootRoleV1::OutputSet,
            PrivacyRootRoleV1::ProgramState,
        ] {
            assert_eq!(role.management(), PrivacyRootManagementV1::ProofManaged);
        }
        for role in [
            PrivacyRootRoleV1::Revocation,
            PrivacyRootRoleV1::CertificateAuthorityMembership,
        ] {
            assert_eq!(
                role.management(),
                PrivacyRootManagementV1::GovernanceManaged
            );
        }

        let pgc = statement_for(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1);
        let namespace = PrivacyNamespaceV1::from_statement(&pgc);
        let publication = PrivacyRootPublicationV1::new(
            namespace,
            PrivacyRootRoleV1::PgcAccountState,
            1,
            PrivacyRootV1::new(raw(200)),
        )
        .expect("valid root publication");
        publication.validate().expect("valid publication");
        let bytes = norito::to_bytes(&publication).expect("frame publication");
        let decoded: PrivacyRootPublicationV1 =
            norito::decode_from_bytes(&bytes).expect("decode publication");
        assert_eq!(decoded, publication);
        assert!(!publication.digest().expect("publication digest").is_zero());

        let mut invalid = publication;
        invalid.epoch = 0;
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyRootPublicationValidationError::ZeroEpoch)
        ));
        invalid = publication;
        invalid.root = PrivacyRootV1::new([0; 32]);
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyRootPublicationValidationError::ZeroRoot)
        ));
        invalid = publication;
        invalid.role = PrivacyRootRoleV1::Revocation;
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyRootPublicationValidationError::IncompatibleRole { .. })
        ));
    }

    #[test]
    fn pgc_bootstrap_is_canonical_bounded_and_has_distinct_provenance() {
        let bootstrap = pgc_bootstrap();
        bootstrap.validate().expect("valid PGC bootstrap");
        let bytes = norito::to_bytes(&bootstrap).expect("frame bootstrap");
        let decoded: PrivacyPgcAccountBootstrapV1 =
            norito::decode_from_bytes(&bytes).expect("decode bootstrap");
        assert_eq!(decoded, bootstrap);
        let digest = bootstrap.digest().expect("bootstrap digest");
        assert!(!digest.is_zero());

        let publication = PrivacyRootPublicationV1::new(
            bootstrap.namespace,
            PrivacyRootRoleV1::PgcAccountState,
            bootstrap.initial_epoch,
            bootstrap.initial_root,
        )
        .expect("bootstrap publication");
        assert_ne!(
            digest.as_bytes(),
            publication.digest().expect("publication digest").as_bytes(),
            "bootstrap and root-publication provenance domains must differ"
        );

        let mut invalid = bootstrap.clone();
        invalid.initial_root = PrivacyRootV1::new([0; 32]);
        assert!(invalid.validate().is_err());
        for epoch in [0, 2, u64::MAX] {
            invalid = bootstrap.clone();
            invalid.initial_epoch = epoch;
            assert!(matches!(
                invalid.validate(),
                Err(
                    PrivacyPgcAccountBootstrapValidationError::NonCanonicalInitialEpoch {
                        epoch: rejected,
                    }
                ) if rejected == epoch
            ));
        }
        invalid = bootstrap.clone();
        invalid.total_supply = 0;
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyPgcAccountBootstrapValidationError::ZeroTotalSupply)
        ));
        invalid = bootstrap.clone();
        invalid.total_supply = u32::MAX;
        invalid
            .validate()
            .expect("the inclusive u32 supply boundary is canonical");
        invalid = bootstrap.clone();
        invalid.accounts.pop();
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyPgcAccountBootstrapValidationError::InvalidAccountCount { count: 15 })
        ));
        invalid = bootstrap.clone();
        invalid.accounts.swap(0, 1);
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyPgcAccountBootstrapValidationError::KeysNotStrictlyIncreasing)
        ));
        invalid = bootstrap.clone();
        invalid.accounts[1].public_key = invalid.accounts[0].public_key;
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyPgcAccountBootstrapValidationError::KeysNotStrictlyIncreasing)
        ));
        invalid = bootstrap.clone();
        invalid.accounts[0].encrypted_balance.right = PrivacyP256PointV1::new([0; 33]);
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyPgcAccountBootstrapValidationError::ZeroPoint {
                point: PrivacyPgcAccountPointV1::EncryptedBalanceRight,
                ..
            })
        ));
    }

    #[test]
    fn orchard_pool_bootstrap_has_one_node_derived_origin_and_distinct_provenance() {
        let bootstrap = PrivacyOrchardPoolBootstrapV1::new(
            PrivacyPoolIdV1::new(raw(210)),
            asset_definition_id(),
            account(211),
        )
        .expect("canonical Orchard pool bootstrap");
        bootstrap.validate().expect("valid Orchard pool bootstrap");
        assert_eq!(
            bootstrap.namespace().protocol_id(),
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1
        );
        let encoded = norito::to_bytes(&bootstrap).expect("frame Orchard bootstrap");
        let decoded: PrivacyOrchardPoolBootstrapV1 =
            norito::decode_from_bytes(&encoded).expect("decode Orchard bootstrap");
        assert_eq!(decoded, bootstrap);

        let digest = bootstrap.digest().expect("digest Orchard bootstrap");
        assert!(!digest.is_zero());
        let mut changed_asset = bootstrap.clone();
        changed_asset.asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("other").expect("asset name"),
        );
        assert_ne!(
            changed_asset.digest().expect("digest changed asset"),
            digest,
            "the immutable public bridge asset must be provenance-bound"
        );
        let mut changed_reserve = bootstrap.clone();
        changed_reserve.reserve_account = account(212);
        assert_ne!(
            changed_reserve.digest().expect("digest changed reserve"),
            digest,
            "the immutable reserve account must be provenance-bound"
        );
        assert_eq!(
            PrivacyOrchardPoolBootstrapV1::new(
                PrivacyPoolIdV1::new([0; 32]),
                asset_definition_id(),
                account(211),
            ),
            Err(PrivacyOrchardPoolBootstrapValidationErrorV1::ZeroPoolId)
        );
    }

    #[test]
    fn proof_managed_pool_bootstraps_are_closed_bounded_and_self_authenticating() {
        let variants = [
            PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(PrivacyFcmpPoolBootstrapV1 {
                pool_id: PrivacyPoolIdV1::new(raw(213)),
                asset_definition_id: asset_definition_id(),
                initial_outputs: sorted_fcmp_outputs(&[1, 2]),
            }),
            PrivacyProofManagedPoolBootstrapV1::IrohaIvmPrivateNoteStarkV1(
                PrivacyIvmPrivateNotePoolBootstrapV1 {
                    pool_id: PrivacyPoolIdV1::new(raw(214)),
                    asset_definition_id: asset_definition_id(),
                    reserve_account: account(215),
                    program_id: PrivacyProgramIdV1::new(raw(216)),
                    initial_note_commitments: vec![commitment(3), commitment(4)],
                },
            ),
            PrivacyProofManagedPoolBootstrapV1::PqMaspStarkV0(PrivacyPqMaspPoolBootstrapV1 {
                pool_id: PrivacyPoolIdV1::new(raw(217)),
                asset_definition_id: asset_definition_id(),
                initial_note_commitments: vec![commitment(5), commitment(6)],
            }),
        ];
        for bootstrap in variants {
            bootstrap
                .validate()
                .expect("canonical typed pool bootstrap");
            assert_eq!(bootstrap.namespace().protocol_id(), bootstrap.protocol_id());
            assert!(
                bootstrap
                    .root_role()
                    .is_compatible_with_namespace(bootstrap.namespace())
            );
            let digest = bootstrap.digest().expect("digest typed pool bootstrap");
            assert!(!digest.is_zero());
            let encoded = norito::to_bytes(&bootstrap).expect("encode typed pool bootstrap");
            let decoded: PrivacyProofManagedPoolBootstrapV1 =
                norito::decode_from_bytes(&encoded).expect("decode typed pool bootstrap");
            assert_eq!(decoded, bootstrap);
            let json = norito::json::to_json(&bootstrap).expect("encode pool-bootstrap JSON");
            let decoded_json: PrivacyProofManagedPoolBootstrapV1 =
                norito::json::from_json(&json).expect("decode pool-bootstrap JSON");
            assert_eq!(decoded_json, bootstrap);
            let nested_prefix = json
                .strip_suffix("}}")
                .expect("tagged bootstrap ends with nested and outer objects");
            assert!(
                norito::json::from_json::<PrivacyProofManagedPoolBootstrapV1>(&format!(
                    "{nested_prefix},\"legacy_root\":true}}}}"
                ))
                .is_err()
            );
        }

        let mut invalid =
            PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(PrivacyFcmpPoolBootstrapV1 {
                pool_id: PrivacyPoolIdV1::new(raw(218)),
                asset_definition_id: asset_definition_id(),
                initial_outputs: Vec::new(),
            });
        assert_eq!(
            invalid.validate(),
            Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::EmptyInitialFcmpOutputs)
        );
        let PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(fcmp) = &mut invalid else {
            unreachable!()
        };
        fcmp.initial_outputs = vec![fcmp_output(7), fcmp_output(7)];
        assert!(matches!(
            invalid.validate(),
            Err(
                PrivacyProofManagedPoolBootstrapValidationErrorV1::InitialFcmpOutputIdsNotStrictlyIncreasing {
                    index: 1
                }
            )
        ));
        let PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(fcmp) = &mut invalid else {
            unreachable!()
        };
        fcmp.initial_outputs = vec![
            PrivacyFcmpOutputTupleV1 {
                output_key: [0; 32],
                linking_tag_generator: raw(8),
                amount_commitment: raw(9),
            },
            fcmp_output(10),
        ];
        assert!(matches!(
            invalid.validate(),
            Err(
                PrivacyProofManagedPoolBootstrapValidationErrorV1::InvalidInitialFcmpOutput {
                    index: 0,
                    source: PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                        component: PrivacyFcmpOutputComponentV1::OutputKey
                    }
                }
            )
        ));
        let PrivacyProofManagedPoolBootstrapV1::MoneroFcmpPlusPlusV1(fcmp) = &mut invalid else {
            unreachable!()
        };
        fcmp.initial_outputs = vec![fcmp_output(9); PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1 + 1];
        assert!(matches!(
            invalid.validate(),
            Err(
                PrivacyProofManagedPoolBootstrapValidationErrorV1::TooManyInitialFcmpOutputs {
                    count,
                    max: PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1
                }
            ) if count == PRIVACY_MAX_INITIAL_POOL_COMMITMENTS_V1 + 1
        ));

        let invalid_program = PrivacyProofManagedPoolBootstrapV1::IrohaIvmPrivateNoteStarkV1(
            PrivacyIvmPrivateNotePoolBootstrapV1 {
                pool_id: PrivacyPoolIdV1::new(raw(219)),
                asset_definition_id: asset_definition_id(),
                reserve_account: account(220),
                program_id: PrivacyProgramIdV1::new([0; 32]),
                initial_note_commitments: vec![commitment(10)],
            },
        );
        assert_eq!(
            invalid_program.validate(),
            Err(PrivacyProofManagedPoolBootstrapValidationErrorV1::ZeroProgramId)
        );
    }

    #[test]
    fn fcmp_output_and_typed_root_domains_match_known_answers() {
        let output = PrivacyFcmpOutputTupleV1 {
            output_key: raw(1),
            linking_tag_generator: raw(2),
            amount_commitment: raw(3),
        };
        assert_eq!(
            output.output_id().into_bytes(),
            hex!("5a67b729f611b60999fabfdb9028dc2e6aec85c1e63422e82ceb0d48bef6d824")
        );
        let root = PrivacyFcmpTreeRootV1 {
            layers: 1,
            point: raw(4),
        };
        assert_eq!(
            root.history_commitment().into_bytes(),
            hex!("d78fe90fc23fc58cf72a5eeaa6c9b145a47319c1ca5ef32eba9d30d359734906")
        );
        assert_ne!(
            PrivacyFcmpTreeRootV1 {
                layers: 2,
                point: raw(4),
            }
            .history_commitment(),
            root.history_commitment(),
            "layer parity is part of the shared root-history commitment"
        );
    }

    #[test]
    fn pgc_bootstrap_proof_bytes_enforce_exact_cap_and_distinct_digest() {
        let max = usize::try_from(TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1)
            .expect("compiled proof cap fits usize");
        let at_cap = PrivacyPgcBootstrapProofBytesV1::new(vec![0xA5; max]);
        at_cap.validate().expect("exact byte cap is admitted");
        let digest = at_cap.digest().expect("digest proof at exact cap");
        assert!(!digest.is_zero());

        let mut changed = at_cap.clone();
        changed.bytes[max - 1] ^= 1;
        assert_ne!(
            changed.digest().expect("digest changed proof"),
            digest,
            "proof provenance must distinguish a one-byte mutation"
        );
        assert!(matches!(
            PrivacyPgcBootstrapProofBytesV1::new(Vec::new()).validate(),
            Err(PrivacyPgcBootstrapProofValidationError::Empty)
        ));
        assert!(matches!(
            PrivacyPgcBootstrapProofBytesV1::new(vec![0; 32]).validate(),
            Err(PrivacyPgcBootstrapProofValidationError::AllZero)
        ));
        assert!(matches!(
            PrivacyPgcBootstrapProofBytesV1::new(vec![1; max + 1]).validate(),
            Err(PrivacyPgcBootstrapProofValidationError::TooLarge {
                bytes,
                max: TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1,
            }) if bytes == u64::from(TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1) + 1
        ));
    }

    #[test]
    fn every_caller_declared_root_transition_requires_a_distinct_exact_successor() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let protocols = [
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyProtocolIdV1::IrohaZkAmsV1,
        ];
        for protocol in protocols {
            for corruption in [
                RootCorruption::ZeroSuccessor,
                RootCorruption::Unchanged,
                RootCorruption::SkippedEpoch,
                RootCorruption::EpochOverflow,
            ] {
                let mut statement = statement_for(protocol);
                corrupt_root_transition(&mut statement, corruption);
                assert!(
                    statement.validate(&limits).is_err(),
                    "{protocol:?} accepted a malformed root transition"
                );
            }
        }
    }

    #[test]
    fn pgc_public_memo_rejects_noncanonical_sizes_order_and_ciphertexts() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let base = statement_for(PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1);

        let mutate = |f: fn(&mut AnonymousPgcKOutOfNStatementV1)| {
            let mut value = base.clone();
            let PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement) = &mut value else {
                unreachable!()
            };
            f(statement);
            value.validate(&limits)
        };

        assert!(matches!(
            mutate(|statement| {
                statement.anonymity_set_public_keys.pop();
                statement.transfer_ciphertexts.pop();
            }),
            Err(PrivacyStatementValidationError::InvalidPgcAnonymitySetSize { size: 15 })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.transfer_ciphertexts.pop();
            }),
            Err(
                PrivacyStatementValidationError::PgcPublicMemoCountMismatch {
                    public_keys: 16,
                    ciphertexts: 15
                }
            )
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.anonymity_set_public_keys[1] = statement.anonymity_set_public_keys[0];
            }),
            Err(PrivacyStatementValidationError::PgcAnonymitySetNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.anonymity_set_public_keys.swap(0, 1);
            }),
            Err(PrivacyStatementValidationError::PgcAnonymitySetNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.anonymity_set_public_keys[0] = PrivacyP256PointV1::new([0; 33]);
            }),
            Err(PrivacyStatementValidationError::ZeroP256Point { index: 0 })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.transfer_ciphertexts[0].left = PrivacyP256PointV1::new([0; 33]);
            }),
            Err(PrivacyStatementValidationError::ZeroP256CiphertextPoint {
                index: 0,
                component: PrivacyP256CiphertextComponentV1::Left
            })
        ));
        assert!(matches!(
            mutate(|statement| statement.recipient_count = 0),
            Err(PrivacyStatementValidationError::InvalidPgcRecipientCount { count: 0, .. })
        ));
        assert!(matches!(
            mutate(|statement| statement.recipient_count = 9),
            Err(PrivacyStatementValidationError::InvalidPgcRecipientCount {
                count: 9,
                max: 8,
                ..
            })
        ));

        let mut thirty_two = base;
        let PrivacyStatementV1::AnonymousPgcKOutOfNV1(statement) = &mut thirty_two else {
            unreachable!()
        };
        statement.anonymity_set_public_keys = (1..=32).map(p256_point).collect();
        statement.transfer_ciphertexts = (1..=32).map(p256_ciphertext).collect();
        thirty_two.validate(&limits).expect("closed n=32 profile");
        let governed = PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
            AnonymousPgcActivationLimitsV1 {
                max_anonymity_set_size: 16,
                max_recipient_count: 8,
            },
        );
        assert!(matches!(
            governed.validate_statement(&thirty_two),
            Err(PrivacyActivationStatementLimitsError::CountExceeds {
                field: PrivacyActivationLimitFieldV1::AnonymousPgcAnonymitySetSize,
                count: 32,
                max: 16
            })
        ));
    }

    #[test]
    fn verange_uses_only_closed_unsigned_ranges_and_effective_taira_cap() {
        assert_eq!(
            VERANGE_TAIRA_MAX_AGGREGATION_COUNT_V1,
            TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1
        );
        assert_eq!(PrivacyVeRangeBitLengthV1::Bits32.bits(), 32);
        assert_eq!(PrivacyVeRangeBitLengthV1::Bits64.bits(), 64);

        let limits = PrivacyConsensusLimitsV1::taira_default();
        let base = statement_for(PrivacyProtocolIdV1::VeRangeTransparentRangeV1);
        let mutate = |f: fn(&mut VeRangeTransparentRangeStatementV1)| {
            let mut value = base.clone();
            let PrivacyStatementV1::VeRangeTransparentRangeV1(statement) = &mut value else {
                unreachable!()
            };
            f(statement);
            value.validate(&limits)
        };

        assert!(matches!(
            mutate(|statement| {
                statement.value_commitments.clear();
                statement.aggregation_count = 0;
            }),
            Err(PrivacyStatementValidationError::InvalidAggregationCount { count: 0, max: 8 })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.value_commitments = (1..=9).map(p256_point).collect();
                statement.aggregation_count = 9;
            }),
            Err(PrivacyStatementValidationError::InvalidAggregationCount { count: 9, max: 8 })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.value_commitments[1] = statement.value_commitments[0];
            }),
            Err(PrivacyStatementValidationError::DuplicateCommitment)
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.value_commitments[0] = PrivacyP256PointV1::new([0; 33]);
            }),
            Err(PrivacyStatementValidationError::ZeroP256Point { index: 0 })
        ));
        let invalid_activation = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 9,
            },
        );
        assert!(matches!(
            invalid_activation.validate(),
            Err(
                PrivacyProtocolActivationLimitsValidationError::ExceedsHardMaximum {
                    field: PrivacyActivationLimitFieldV1::VeRangeAggregationCount,
                    value: 9,
                    hard_max: 8
                }
            )
        ));
    }

    #[test]
    fn protocol_activation_profiles_reject_zero_and_over_ceiling_values() {
        for protocol in PrivacyProtocolIdV1::ALL {
            protocol_limits(protocol)
                .validate()
                .expect("default protocol profile");
        }
        let invalid = [
            PrivacyProtocolActivationLimitsV1::AnonymousPgcKOutOfNV1(
                AnonymousPgcActivationLimitsV1 {
                    max_anonymity_set_size: 15,
                    max_recipient_count: 8,
                },
            ),
            PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                max_batch_size: 0,
                max_ring_size: ZK_AMS_MAX_RING_SIZE_V1,
            }),
            PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
                max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1,
                max_ring_size: 15,
            }),
            PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
                JindoActivationLimitsV1 {
                    max_polynomial_count: IROHA_JINDO_MAX_POLYNOMIALS_V1 + 1,
                },
            ),
            PrivacyProtocolActivationLimitsV1::OrchardHalo2ActionsV1(OrchardActivationLimitsV1 {
                max_action_count: 0,
            }),
            PrivacyProtocolActivationLimitsV1::MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1 {
                max_input_count: FCMP_MAX_INPUTS_V1 + 1,
                max_output_count: FCMP_MAX_OUTPUTS_V1,
            }),
            PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(
                IvmPrivateNoteActivationLimitsV1 {
                    max_input_count: IVM_PRIVATE_NOTE_MAX_INPUTS_V1,
                    max_output_count: 0,
                },
            ),
            PrivacyProtocolActivationLimitsV1::PqMaspStarkV0(PqMaspActivationLimitsV1 {
                max_input_count: PQ_MASP_MAX_INPUTS_V1,
                max_output_count: PQ_MASP_MAX_OUTPUTS_V1 + 1,
            }),
        ];
        for value in invalid {
            assert!(value.validate().is_err(), "invalid activation: {value:?}");
        }
    }

    #[test]
    fn zk_ams_batch_rejects_malformed_or_duplicate_anchors() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let base = statement_for(PrivacyProtocolIdV1::IrohaZkAmsV1);
        let mutate = |f: fn(&mut PrivacyZkAmsBatchAdmissionV1)| {
            let mut value = base.clone();
            let PrivacyStatementV1::IrohaZkAmsV1(statement) = &mut value else {
                unreachable!()
            };
            let PrivacyZkAmsActionV1::BatchAdmission(batch) = &mut statement.action else {
                unreachable!()
            };
            f(batch);
            value.validate(&limits)
        };
        assert!(matches!(
            mutate(|batch| batch.anchors = (1..=9).map(zk_ams_anchor).collect()),
            Err(PrivacyStatementValidationError::InvalidBatchSize { count: 9, max: 8 })
        ));
        assert!(matches!(
            mutate(|batch| batch.anchors.clear()),
            Err(PrivacyStatementValidationError::InvalidBatchSize { count: 0, max: 8 })
        ));
        assert!(matches!(
            mutate(|batch| batch.anchors[0].phc_hash = PrivacyZkAmsPhcHashV1::new([0; 32])),
            Err(PrivacyStatementValidationError::ZeroZkAmsPhcHash { index: 0 })
        ));
        assert!(matches!(
            mutate(|batch| {
                batch.anchors[1].phc_hash = batch.anchors[0].phc_hash;
            }),
            Err(PrivacyStatementValidationError::DuplicateZkAmsPhcHash)
        ));
        assert!(matches!(
            mutate(|batch| {
                batch.anchors[1].seed_public_key = batch.anchors[0].seed_public_key;
            }),
            Err(PrivacyStatementValidationError::DuplicateZkAmsSeedPublicKey)
        ));
    }

    #[test]
    fn zk_ams_provisioning_enforces_closed_canonical_ring_and_key_image() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        for size in [16, 32, 64] {
            zk_ams_provision_statement(size)
                .validate(&limits)
                .expect("closed ZK-AMS ring size");
        }

        let mutate = |f: fn(&mut PrivacyZkAmsProvisionAccountV1)| {
            let mut value = zk_ams_provision_statement(16);
            let PrivacyStatementV1::IrohaZkAmsV1(statement) = &mut value else {
                unreachable!()
            };
            let PrivacyZkAmsActionV1::ProvisionAccount(provision) = &mut statement.action else {
                unreachable!()
            };
            f(provision);
            value.validate(&limits)
        };
        assert!(matches!(
            mutate(|provision| {
                provision.admitted_seed_key_ring.pop();
            }),
            Err(PrivacyStatementValidationError::InvalidZkAmsRingSize { size: 15 })
        ));
        assert!(matches!(
            mutate(|provision| provision.admitted_seed_key_ring.swap(0, 1)),
            Err(PrivacyStatementValidationError::ZkAmsSeedKeyRingNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate(|provision| {
                provision.admitted_seed_key_ring[1] = provision.admitted_seed_key_ring[0];
            }),
            Err(PrivacyStatementValidationError::ZkAmsSeedKeyRingNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate(|provision| {
                provision.admitted_seed_key_ring[0] = PrivacyZkAmsSeedPublicKeyV1::new([0; 32]);
            }),
            Err(PrivacyStatementValidationError::ZeroZkAmsSeedPublicKey { index: 0 })
        ));
        assert!(matches!(
            mutate(|provision| provision.key_image = PrivacyZkAmsKeyImageV1::new([0; 32])),
            Err(PrivacyStatementValidationError::ZeroZkAmsKeyImage)
        ));

        let governed = PrivacyProtocolActivationLimitsV1::IrohaZkAmsV1(ZkAmsActivationLimitsV1 {
            max_batch_size: ZK_AMS_MAX_BATCH_SIZE_V1,
            max_ring_size: 16,
        });
        assert!(matches!(
            governed.validate_statement(&zk_ams_provision_statement(32)),
            Err(PrivacyActivationStatementLimitsError::CountExceeds {
                field: PrivacyActivationLimitFieldV1::ZkAmsRingSize,
                count: 32,
                max: 16
            })
        ));
    }

    #[test]
    fn jindo_univariate_profile_rejects_noncanonical_and_out_of_bound_values() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let base = statement_for(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0);
        let mutate = |f: fn(&mut IrohaJindoPolynomialCommitmentStatementV1)| {
            let mut value = base.clone();
            let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut value else {
                unreachable!()
            };
            f(statement);
            value.validate(&limits)
        };

        mutate(|statement| {
            statement.evaluation_point = PrivacyJindoFieldElementV1::new([0; 32]);
            statement.claimed_evaluations[0] = PrivacyJindoFieldElementV1::new([0; 32]);
        })
        .expect("zero is a canonical Jindo field element");

        mutate(|statement| {
            let mut value = IROHA_JINDO_FIELD_MODULUS_LE_V1;
            value[0] -= 1;
            statement.evaluation_point = PrivacyJindoFieldElementV1::new(value);
        })
        .expect("p - 1 is the largest canonical Jindo field element");

        assert!(matches!(
            mutate(|statement| {
                statement.evaluation_point =
                    PrivacyJindoFieldElementV1::new(IROHA_JINDO_FIELD_MODULUS_LE_V1);
            }),
            Err(PrivacyStatementValidationError::NonCanonicalJindoEvaluationPoint)
        ));
        assert!(matches!(
            mutate(|statement| {
                let mut modulus_plus_one = IROHA_JINDO_FIELD_MODULUS_LE_V1;
                modulus_plus_one[0] += 1;
                statement.claimed_evaluations[1] =
                    PrivacyJindoFieldElementV1::new(modulus_plus_one);
            }),
            Err(PrivacyStatementValidationError::NonCanonicalJindoClaimedEvaluation { index: 1 })
        ));

        assert!(matches!(
            mutate(|statement| {
                statement.claimed_evaluations.pop();
            }),
            Err(PrivacyStatementValidationError::DeclaredCountMismatch {
                field: PrivacyCountFieldV1::JindoClaimedEvaluations,
                declared: 2,
                actual: 1
            })
        ));
        assert!(matches!(
            mutate(|statement| statement.polynomial_commitments.clear()),
            Err(PrivacyStatementValidationError::InvalidBatchSize { count: 0, max: 4 })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.polynomial_commitments = (1..=5).map(jindo_commitment).collect();
                statement.claimed_evaluations = (1..=5).map(jindo_field).collect();
            }),
            Err(PrivacyStatementValidationError::InvalidBatchSize { count: 5, max: 4 })
        ));

        assert!(matches!(
            mutate(|statement| {
                statement.polynomial_commitments[0].encoding.pop();
            }),
            Err(
                PrivacyStatementValidationError::InvalidJindoLatticeCommitmentSize {
                    index: 0,
                    bytes,
                    expected
                }
            ) if bytes == u32::try_from(IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1 - 1).unwrap()
                && expected == u32::try_from(IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1).unwrap()
        ));
        assert!(matches!(
            mutate(|statement| statement.polynomial_commitments[0].encoding.push(0)),
            Err(
                PrivacyStatementValidationError::InvalidJindoLatticeCommitmentSize {
                    index: 0,
                    bytes,
                    expected
                }
            ) if bytes == u32::try_from(IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1 + 1).unwrap()
                && expected == u32::try_from(IROHA_JINDO_LATTICE_COMMITMENT_BYTES_V1).unwrap()
        ));
        assert!(matches!(
            mutate(|statement| statement.polynomial_commitments[0].encoding.fill(0)),
            Err(PrivacyStatementValidationError::AllZeroJindoLatticeCommitment { index: 0 })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.polynomial_commitments[1] = statement.polynomial_commitments[0].clone();
            }),
            Err(PrivacyStatementValidationError::DuplicateJindoLatticeCommitment)
        ));

        for boundary in [
            IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1,
            IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1,
        ] {
            let mut value = base.clone();
            let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut value else {
                unreachable!()
            };
            statement.polynomial_commitments[0].encoding[..4]
                .copy_from_slice(&boundary.to_le_bytes());
            value
                .validate(&limits)
                .expect("inclusive Jindo rounded-coefficient boundary");
        }

        for outside in [
            i64::from(IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1) + 1,
            i64::from(IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1) - 1,
        ] {
            let mut value = base.clone();
            let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut value else {
                unreachable!()
            };
            statement.polynomial_commitments[0].encoding[..4].copy_from_slice(
                &i32::try_from(outside)
                    .expect("adversarial Jindo coefficient fits i32")
                    .to_le_bytes(),
            );
            assert!(matches!(
                value.validate(&limits),
                Err(
                    PrivacyStatementValidationError::JindoCommitmentCoefficientOutOfRange {
                        commitment_index: 0,
                        coefficient_index: 0,
                        value: observed,
                        min: IROHA_JINDO_MIN_ROUNDED_COMMITMENT_COEFFICIENT_V1,
                        max: IROHA_JINDO_MAX_ROUNDED_COMMITMENT_COEFFICIENT_V1
                    }
                ) if i64::from(observed) == outside
            ));
        }

        let governed = PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
            JindoActivationLimitsV1 {
                max_polynomial_count: 1,
            },
        );
        assert!(matches!(
            governed.validate_statement(&base),
            Err(PrivacyActivationStatementLimitsError::CountExceeds {
                field: PrivacyActivationLimitFieldV1::JindoPolynomialCount,
                count: 2,
                max: 1
            })
        ));
    }

    #[test]
    fn vega_figure9_public_inputs_are_closed_and_non_degenerate() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let vega = statement_for(PrivacyProtocolIdV1::VegaExistingCredentialZkV0);
        let mutate_vega = |f: fn(&mut VegaExistingCredentialStatementV1)| {
            let mut value = vega.clone();
            let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) = &mut value else {
                unreachable!()
            };
            f(statement);
            value.validate(&limits)
        };
        assert!(matches!(
            mutate_vega(|statement| statement.issuer_id = PrivacyIssuerIdV1::new([0; 32])),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::IssuerId
            })
        ));
        assert!(matches!(
            mutate_vega(|statement| statement.issuer_record_epoch = 0),
            Err(PrivacyStatementValidationError::ZeroEpoch {
                field: PrivacyEpochFieldV1::VegaIssuerRecord
            })
        ));
        assert!(matches!(
            mutate_vega(|statement| {
                statement.issuer_record_digest = PrivacyVegaIssuerRecordDigestV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::VegaIssuerRecordDigest
            })
        ));
        assert!(matches!(
            mutate_vega(|statement| statement.issuer_public_key = PrivacyP256PointV1::new([0; 33])),
            Err(PrivacyStatementValidationError::ZeroP256Point { index: 0 })
        ));
        assert!(matches!(
            mutate_vega(|statement| {
                statement.device_authentication_digest =
                    PrivacyVegaDeviceAuthenticationDigestV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::VegaDeviceAuthenticationDigest
            })
        ));
        assert!(matches!(
            mutate_vega(|statement| {
                statement.reader_challenge = PrivacyChallengeV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::ReaderChallenge
            })
        ));
        assert!(matches!(
            mutate_vega(|statement| {
                statement.session_transcript_digest = PrivacySessionTranscriptDigestV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::SessionTranscriptDigest
            })
        ));
        for years in [0, VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1.saturating_add(1)] {
            let mut value = vega.clone();
            let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) = &mut value else {
                unreachable!()
            };
            statement.minimum_age_years = years;
            assert!(matches!(
                value.validate(&limits),
                Err(PrivacyStatementValidationError::InvalidVegaAgeThreshold { .. })
            ));
        }
        assert_eq!(
            PrivacyNamespaceV1::from_statement(&vega).scope(),
            PrivacyNamespaceScopeV1::Parameter(PrivacyParameterNamespaceV1 {
                parameter_id: context().parameter_id
            })
        );
    }

    #[test]
    fn vega_issuer_records_are_self_digested_forward_only_and_policy_closed() {
        let issuer_id = PrivacyIssuerIdV1::new(raw(0x91));
        let origin = PrivacyVegaIssuerRecordV1::new(
            issuer_id,
            1,
            p256_point(0x92),
            PrivacyCredentialDocumentTypeV1::Iso18013_5Mdl,
            PrivacyVegaMdlNamespaceV1::OrgIso18013_5_1,
            PrivacyVegaMdlDigestAlgorithmV1::Sha256,
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
            PrivacyVegaMdlSignatureAlgorithmV1::CoseSign1Es256,
            None,
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        )
        .expect("canonical Vega issuer origin");
        origin.validate_initial().expect("valid issuer origin");

        let mut tampered = origin;
        tampered.issuer_public_key = p256_point(0x93);
        assert_eq!(
            tampered.validate(),
            Err(PrivacyVegaIssuerRecordValidationErrorV1::RecordDigestMismatch)
        );

        let rotation = PrivacyVegaIssuerRecordV1::new(
            issuer_id,
            2,
            p256_point(0x93),
            origin.document_type,
            origin.namespace,
            origin.digest_algorithm,
            origin.issuer_authentication_algorithm,
            origin.device_authentication_algorithm,
            Some(origin.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        )
        .expect("canonical issuer rotation");
        validate_vega_issuer_rotation_v1(&origin, &rotation)
            .expect("one-step key rotation is valid");

        let no_op = PrivacyVegaIssuerRecordV1::new(
            issuer_id,
            2,
            origin.issuer_public_key,
            origin.document_type,
            origin.namespace,
            origin.digest_algorithm,
            origin.issuer_authentication_algorithm,
            origin.device_authentication_algorithm,
            Some(origin.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        )
        .expect("intrinsically valid no-op successor");
        assert_eq!(
            validate_vega_issuer_rotation_v1(&origin, &no_op),
            Err(PrivacyVegaIssuerTransitionValidationErrorV1::RotationContentsUnchanged)
        );

        let wrong_predecessor = PrivacyVegaIssuerRecordV1::new(
            issuer_id,
            2,
            p256_point(0x93),
            origin.document_type,
            origin.namespace,
            origin.digest_algorithm,
            origin.issuer_authentication_algorithm,
            origin.device_authentication_algorithm,
            Some(PrivacyVegaIssuerRecordDigestV1::new(raw(0x94))),
            PrivacyVegaIssuerRecordLifecycleV1::Active,
        )
        .expect("intrinsically valid predecessor substitution");
        assert_eq!(
            validate_vega_issuer_rotation_v1(&origin, &wrong_predecessor),
            Err(PrivacyVegaIssuerTransitionValidationErrorV1::PredecessorDigestMismatch)
        );

        let revocation = PrivacyVegaIssuerRecordV1::new(
            issuer_id,
            3,
            rotation.issuer_public_key,
            rotation.document_type,
            rotation.namespace,
            rotation.digest_algorithm,
            rotation.issuer_authentication_algorithm,
            rotation.device_authentication_algorithm,
            Some(rotation.record_digest),
            PrivacyVegaIssuerRecordLifecycleV1::Revoked,
        )
        .expect("canonical issuer revocation");
        validate_vega_issuer_revocation_v1(&rotation, &revocation)
            .expect("exact terminal successor");
        assert_eq!(
            validate_vega_issuer_rotation_v1(&revocation, &rotation),
            Err(PrivacyVegaIssuerTransitionValidationErrorV1::CurrentNotActive)
        );

        let encoded = norito::json::to_json(&origin).expect("encode Vega issuer record");
        let unknown_algorithm = encoded.replacen("Sha256", "Sha512", 1);
        assert_ne!(
            unknown_algorithm, encoded,
            "fixture contains digest algorithm"
        );
        assert!(
            norito::json::from_json::<PrivacyVegaIssuerRecordV1>(&unknown_algorithm).is_err(),
            "unreleased algorithm-policy variants must reject"
        );
        let legacy_field = encoded.replacen(
            "\"record_epoch\":1",
            "\"record_epoch\":1,\"legacy_key_id\":\"forbidden\"",
            1,
        );
        assert_ne!(legacy_field, encoded, "fixture contains record epoch");
        assert!(
            norito::json::from_json::<PrivacyVegaIssuerRecordV1>(&legacy_field).is_err(),
            "unknown legacy fields must reject"
        );
    }

    #[test]
    fn vega_presentation_date_is_strict_proleptic_gregorian() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let vega = statement_for(PrivacyProtocolIdV1::VegaExistingCredentialZkV0);
        let mutate_date = |date: PrivacyVegaMdlDateV1| {
            let mut value = vega.clone();
            let PrivacyStatementV1::VegaExistingCredentialZkV0(statement) = &mut value else {
                unreachable!()
            };
            statement.presentation_date = date;
            value.validate(&limits)
        };

        for year in [
            VEGA_MDL_MIN_PRESENTATION_YEAR_V1 - 1,
            VEGA_MDL_MAX_PRESENTATION_YEAR_V1 + 1,
        ] {
            assert!(matches!(
                mutate_date(PrivacyVegaMdlDateV1 {
                    year,
                    month: 1,
                    day: 1
                }),
                Err(PrivacyStatementValidationError::InvalidVegaPresentationYear { .. })
            ));
        }
        for date in [
            PrivacyVegaMdlDateV1 {
                year: 2_026,
                month: 0,
                day: 1,
            },
            PrivacyVegaMdlDateV1 {
                year: 2_026,
                month: 13,
                day: 1,
            },
            PrivacyVegaMdlDateV1 {
                year: 2_026,
                month: 4,
                day: 31,
            },
            PrivacyVegaMdlDateV1 {
                year: 2_026,
                month: 2,
                day: 29,
            },
            PrivacyVegaMdlDateV1 {
                year: 2_000,
                month: 2,
                day: 30,
            },
            PrivacyVegaMdlDateV1 {
                year: 2_026,
                month: 1,
                day: 0,
            },
        ] {
            assert!(matches!(
                mutate_date(date),
                Err(PrivacyStatementValidationError::InvalidVegaPresentationDate { .. })
            ));
        }
        assert!(
            mutate_date(PrivacyVegaMdlDateV1 {
                year: 2_000,
                month: 2,
                day: 29,
            })
            .is_ok()
        );
        assert!(
            mutate_date(PrivacyVegaMdlDateV1 {
                year: VEGA_MDL_MAX_PRESENTATION_YEAR_V1,
                month: 12,
                day: 31,
            })
            .is_ok(),
            "the maximum presentation date has a possible later four-digit expiry"
        );
    }

    #[test]
    fn vega_release_constants_match_the_one_compiled_figure9_shape() {
        assert_eq!(VEGA_MDL_ISSUER_AUTHENTICATION_SIG_STRUCTURE_BYTES_V1, 368);
        assert_eq!(VEGA_MDL_MSO_PAYLOAD_BYTES_V1, 348);
        assert_eq!(VEGA_MDL_BIRTH_DATE_ISSUER_SIGNED_ITEM_BYTES_V1, 92);
        assert_eq!(VEGA_MDL_BIRTH_RANDOM_BYTES_V1, 16);
        assert_eq!(VEGA_MDL_FULL_DATE_TEXT_BYTES_V1, 10);
        assert_eq!(VEGA_MDL_RFC3339_UTC_SECONDS_TEXT_BYTES_V1, 20);
        assert_eq!(VEGA_MDL_MIN_PRESENTATION_YEAR_V1, 1_970);
        assert_eq!(VEGA_MDL_MAX_PRESENTATION_YEAR_V1, 9_998);
        assert_eq!(VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1, 1);
        assert_eq!(VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1, 150);
    }

    #[test]
    fn x509_rejects_stale_roots_invalid_usage_and_invalid_presentation_windows() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let x509 = statement_for(PrivacyProtocolIdV1::IrohaZkX509StarkP256V0);
        let mutate_x509 = |f: fn(&mut IrohaZkX509StarkP256StatementV1)| {
            let mut value = x509.clone();
            let PrivacyStatementV1::IrohaZkX509StarkP256V0(statement) = &mut value else {
                unreachable!()
            };
            f(statement);
            value.validate(&limits)
        };
        assert!(mutate_x509(|statement| statement.ca_membership_root_epoch = 0).is_err());
        assert!(matches!(
            mutate_x509(|statement| statement.trust_anchor_record_epoch = 0),
            Err(PrivacyStatementValidationError::ZeroEpoch {
                field: PrivacyEpochFieldV1::X509TrustAnchorRecord
            })
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.trust_anchor_record_digest =
                    PrivacyZkX509TrustAnchorRecordDigestV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::X509TrustAnchorRecordDigest
            })
        ));
        assert!(matches!(
            mutate_x509(|statement| statement.certificate_policy_record_epoch = 0),
            Err(PrivacyStatementValidationError::ZeroEpoch {
                field: PrivacyEpochFieldV1::X509CertificatePolicyRecord
            })
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.certificate_policy_record_digest =
                    PrivacyZkX509CertificatePolicyRecordDigestV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::X509CertificatePolicyRecordDigest
            })
        ));
        assert!(matches!(
            mutate_x509(|statement| statement.crl_record_epoch = 0),
            Err(PrivacyStatementValidationError::ZeroEpoch {
                field: PrivacyEpochFieldV1::X509CrlRecord
            })
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.crl_record_digest = PrivacyZkX509CrlRecordDigestV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::X509CrlRecordDigest
            })
        ));
        assert!(matches!(
            mutate_x509(|statement| statement.key_usage.digital_signature = false),
            Err(PrivacyStatementValidationError::InvalidX509KeyUsage)
        ));
        assert!(matches!(
            mutate_x509(|statement| statement.extended_key_usages.clear()),
            Err(PrivacyStatementValidationError::MissingX509ExtendedKeyUsage)
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.extended_key_usages = vec![
                    PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                    PrivacyX509ExtendedKeyUsageV1::DocumentSigning,
                    PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
                    PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
                ]
            }),
            Err(
                PrivacyStatementValidationError::TooManyX509ExtendedKeyUsages {
                    actual: 4,
                    max: ZK_X509_MAX_EXTENDED_KEY_USAGES_V1
                }
            )
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.extended_key_usages = vec![
                    PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                    PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                ]
            }),
            Err(PrivacyStatementValidationError::X509ExtendedKeyUsagesNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.disclosed_attributes[1].index = statement.disclosed_attributes[0].index
            }),
            Err(PrivacyStatementValidationError::X509DisclosedAttributesNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate_x509(|statement| statement.disclosed_attributes[1].index = 4),
            Err(
                PrivacyStatementValidationError::UnsupportedX509DisclosedAttributeIndex {
                    index: 4
                }
            )
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.disclosed_attributes[0].attribute_digest =
                    PrivacyAttributeDigestV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroX509DisclosedAttributeDigest { index: 0 })
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.disclosed_attributes = (0_u8..5)
                    .map(|index| PrivacyZkX509DisclosedAttributeV1 {
                        index,
                        attribute_digest: PrivacyAttributeDigestV1::new(raw(index + 1)),
                    })
                    .collect()
            }),
            Err(
                PrivacyStatementValidationError::TooManyX509DisclosedAttributes {
                    actual: 5,
                    max: ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1
                }
            )
        ));
        mutate_x509(|statement| {
            statement.presentation_not_after_unix_seconds = statement
                .presentation_not_before_unix_seconds
                + ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1;
        })
        .expect("the exact presentation-window ceiling is admitted");
        let window_mutations: [fn(&mut IrohaZkX509StarkP256StatementV1); 3] = [
            |statement: &mut IrohaZkX509StarkP256StatementV1| {
                statement.presentation_not_after_unix_seconds =
                    statement.presentation_not_before_unix_seconds;
            },
            |statement: &mut IrohaZkX509StarkP256StatementV1| {
                statement.presentation_not_after_unix_seconds =
                    statement.presentation_not_before_unix_seconds - 1;
            },
            |statement: &mut IrohaZkX509StarkP256StatementV1| {
                statement.presentation_not_after_unix_seconds = statement
                    .presentation_not_before_unix_seconds
                    + ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1
                    + 1;
            },
        ];
        for mutation in window_mutations {
            assert!(matches!(
                mutate_x509(mutation),
                Err(PrivacyStatementValidationError::InvalidX509PresentationWindow { .. })
            ));
        }
    }

    #[test]
    fn zk_x509_governance_sha256_frames_match_known_answers() {
        // A small, canonical DER SEQUENCE containing INTEGER 42 and NULL.
        let exact_crl_der = hex!("300502012a0500");
        // RFC 5480 id-ecPublicKey/P-256 SPKI for the SEC 2 generator.
        let exact_issuer_spki_der = hex!(
            "3059301306072a8648ce3d020106082a8648ce3d03010703420004\
             6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296\
             4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        );
        let crl_der_digest = PrivacyX509CrlDerDigestV1::digest_exact_der(&exact_crl_der);
        let issuer_spki_digest =
            PrivacyX509CrlIssuerSpkiDigestV1::digest_exact_der(&exact_issuer_spki_der);
        assert_eq!(
            crl_der_digest.as_bytes(),
            &hex!("bfa3e6225fbdc178b8595c06d8fb7ac8c48bcbf22370733501284b73fbba7e98")
        );
        assert_eq!(
            issuer_spki_digest.as_bytes(),
            &hex!("f7e1ecd75dd0aee92a81c2e8cfbb22cdee73ba8700b64349f6331d989d2b4400")
        );
        assert_ne!(
            crl_der_digest.as_bytes(),
            PrivacyX509CrlDerDigestV1::digest_exact_der(&exact_issuer_spki_der).as_bytes(),
            "the exact byte sequence must remain bound to its digest input"
        );
        assert_ne!(
            crl_der_digest.as_bytes(),
            PrivacyX509CrlIssuerSpkiDigestV1::digest_exact_der(&exact_crl_der).as_bytes(),
            "identical bytes under distinct digest domains must not collide"
        );

        let record = PrivacyZkX509CrlRecordV1::new(
            PrivacyIssuerIdV1::new([0x11; 32]),
            PrivacyPolicyIdV1::new([0x22; 32]),
            1,
            42,
            crl_der_digest,
            issuer_spki_digest,
            1_700_000_000,
            1_700_000_300,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("known-answer CRL record is canonical");
        assert_eq!(
            record.record_digest.as_bytes(),
            &hex!("d9cc3938a2fb3b8407f17c9e71ce926c627c144c1bf6c6a89a5fa2b73176c64d")
        );

        let trust_anchor = PrivacyZkX509TrustAnchorRecordV1::new(
            PrivacyIssuerIdV1::new([0x11; 32]),
            1,
            PrivacyX509TrustStoreDigestV1::new([0x22; 32]),
            PrivacyRootV1::new([0x33; 32]),
            1,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("known-answer trust-anchor record is canonical");
        assert_eq!(
            trust_anchor.record_digest.as_bytes(),
            &hex!("e4a0cf77fc1f0acefeeb98e62c74f718a2aa44e6471f7d1ee4d8b9022743e429")
        );

        let policy = PrivacyZkX509CertificatePolicyRecordV1::new(
            PrivacyIssuerIdV1::new([0x11; 32]),
            PrivacyPolicyIdV1::new([0x22; 32]),
            1,
            PrivacyPolicyDigestV1::new([0x33; 32]),
            PrivacyX509KeyUsageV1 {
                digital_signature: true,
                content_commitment: false,
                key_encipherment: false,
                key_agreement: false,
            },
            vec![
                PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
            ],
            vec![0, 3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        )
        .expect("known-answer certificate-policy record is canonical");
        assert_eq!(
            policy.record_digest.as_bytes(),
            &hex!("9a1b485c0566abe3130bf29cacb9e2108adb6372174f8578021019a2f58c8ab0")
        );
    }

    #[test]
    fn zk_x509_governance_records_are_self_digested_strict_and_roundtrip() {
        let trust_anchor = zk_x509_trust_anchor(
            ZK_X509_INITIAL_RECORD_EPOCH_V1,
            80,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        trust_anchor
            .validate_initial()
            .expect("canonical trust-anchor origin");
        assert_eq!(
            trust_anchor
                .compute_record_digest()
                .expect("recompute trust-anchor digest"),
            trust_anchor.record_digest
        );
        let encoded = norito::to_bytes(&trust_anchor).expect("encode trust-anchor");
        let decoded: PrivacyZkX509TrustAnchorRecordV1 =
            norito::decode_from_bytes(&encoded).expect("decode trust-anchor");
        assert_eq!(decoded, trust_anchor);
        decoded
            .validate_initial()
            .expect("decoded trust-anchor validates");
        let json = norito::json::to_json(&trust_anchor).expect("encode trust-anchor JSON");
        let decoded_json: PrivacyZkX509TrustAnchorRecordV1 =
            norito::json::from_json(&json).expect("decode trust-anchor JSON");
        assert_eq!(decoded_json, trust_anchor);
        let object_prefix = json
            .strip_suffix('}')
            .expect("trust-anchor JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyZkX509TrustAnchorRecordV1>(&format!(
                "{object_prefix},\"legacy_anchor\":true}}"
            ))
            .is_err()
        );

        let certificate_policy = zk_x509_certificate_policy(
            ZK_X509_INITIAL_RECORD_EPOCH_V1,
            81,
            vec![0, 3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        certificate_policy
            .validate_initial()
            .expect("canonical certificate-policy origin");
        assert_eq!(
            certificate_policy
                .compute_record_digest()
                .expect("recompute certificate-policy digest"),
            certificate_policy.record_digest
        );
        let encoded = norito::to_bytes(&certificate_policy).expect("encode certificate policy");
        let decoded: PrivacyZkX509CertificatePolicyRecordV1 =
            norito::decode_from_bytes(&encoded).expect("decode certificate policy");
        assert_eq!(decoded, certificate_policy);
        decoded
            .validate_initial()
            .expect("decoded certificate policy validates");
        let json =
            norito::json::to_json(&certificate_policy).expect("encode certificate-policy JSON");
        let decoded_json: PrivacyZkX509CertificatePolicyRecordV1 =
            norito::json::from_json(&json).expect("decode certificate-policy JSON");
        assert_eq!(decoded_json, certificate_policy);
        let object_prefix = json
            .strip_suffix('}')
            .expect("certificate-policy JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyZkX509CertificatePolicyRecordV1>(&format!(
                "{object_prefix},\"legacy_policy\":true}}"
            ))
            .is_err()
        );

        let mut anchor_tamperings = Vec::new();
        let mut tampered = trust_anchor;
        tampered.trust_anchor_id = PrivacyIssuerIdV1::new(raw(82));
        anchor_tamperings.push(tampered);
        let mut tampered = trust_anchor;
        tampered.trust_store_digest = PrivacyX509TrustStoreDigestV1::new(raw(83));
        anchor_tamperings.push(tampered);
        let mut tampered = trust_anchor;
        tampered.ca_membership_root = PrivacyRootV1::new(raw(84));
        anchor_tamperings.push(tampered);
        for tampered in anchor_tamperings {
            assert_eq!(
                tampered.validate(),
                Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch)
            );
        }
        let mut terminal_origin = trust_anchor;
        terminal_origin.lifecycle = PrivacyZkX509RecordLifecycleV1::Revoked;
        assert_eq!(
            terminal_origin.validate(),
            Err(
                PrivacyZkX509RecordValidationErrorV1::RevokedCaMembershipRootEpochNotHistorical {
                    record_epoch: 1,
                    root_epoch: 1,
                }
            )
        );
        let mut mismatched_root_epoch = trust_anchor;
        mismatched_root_epoch.ca_membership_root_epoch = trust_anchor.record_epoch + 1;
        assert_eq!(
            mismatched_root_epoch.validate(),
            Err(
                PrivacyZkX509RecordValidationErrorV1::CaMembershipRootEpochMismatch {
                    record_epoch: trust_anchor.record_epoch,
                    root_epoch: trust_anchor.record_epoch + 1,
                }
            )
        );
        let mut zero_digest = trust_anchor;
        zero_digest.record_digest = PrivacyZkX509TrustAnchorRecordDigestV1::new([0; 32]);
        assert_eq!(
            zero_digest.validate(),
            Err(PrivacyZkX509RecordValidationErrorV1::ZeroRecordDigest)
        );
        assert_eq!(
            PrivacyZkX509TrustAnchorRecordV1::new(
                trust_anchor.trust_anchor_id,
                1,
                trust_anchor.trust_store_digest,
                PrivacyRootV1::new([0; 32]),
                1,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            ),
            Err(PrivacyZkX509RecordValidationErrorV1::ZeroCaMembershipRoot)
        );
        assert_eq!(
            PrivacyZkX509TrustAnchorRecordV1::new(
                trust_anchor.trust_anchor_id,
                1,
                trust_anchor.trust_store_digest,
                trust_anchor.ca_membership_root,
                0,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            ),
            Err(PrivacyZkX509RecordValidationErrorV1::ZeroCaMembershipRootEpoch)
        );
        assert_eq!(
            PrivacyZkX509TrustAnchorRecordV1::new(
                trust_anchor.trust_anchor_id,
                1,
                trust_anchor.trust_store_digest,
                trust_anchor.ca_membership_root,
                2,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            ),
            Err(
                PrivacyZkX509RecordValidationErrorV1::CaMembershipRootEpochMismatch {
                    record_epoch: 1,
                    root_epoch: 2,
                }
            )
        );
        assert_eq!(
            PrivacyZkX509TrustAnchorRecordV1::new(
                trust_anchor.trust_anchor_id,
                2,
                trust_anchor.trust_store_digest,
                trust_anchor.ca_membership_root,
                2,
                Some(trust_anchor.record_digest),
                PrivacyZkX509RecordLifecycleV1::Revoked,
            ),
            Err(
                PrivacyZkX509RecordValidationErrorV1::RevokedCaMembershipRootEpochNotHistorical {
                    record_epoch: 2,
                    root_epoch: 2,
                }
            )
        );

        let mut policy_tamperings = Vec::new();
        let mut tampered = certificate_policy.clone();
        tampered.policy_digest = PrivacyPolicyDigestV1::new(raw(84));
        policy_tamperings.push(tampered);
        let mut tampered = certificate_policy.clone();
        tampered.required_key_usage.key_agreement = true;
        policy_tamperings.push(tampered);
        let mut tampered = certificate_policy.clone();
        tampered.required_extended_key_usages.remove(0);
        policy_tamperings.push(tampered);
        let mut tampered = certificate_policy;
        tampered.required_disclosed_attribute_indices = vec![0, 2];
        policy_tamperings.push(tampered);
        for tampered in policy_tamperings {
            assert_eq!(
                tampered.validate(),
                Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch)
            );
        }
    }

    #[test]
    fn zk_x509_signed_crl_records_are_canonical_bound_and_fail_closed() {
        let origin = zk_x509_crl(
            ZK_X509_INITIAL_RECORD_EPOCH_V1,
            100,
            1_000,
            101,
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        origin
            .validate_initial()
            .expect("canonical signed-CRL origin");
        assert_eq!(
            origin
                .compute_record_digest()
                .expect("recompute signed-CRL digest"),
            origin.record_digest
        );

        let encoded = norito::to_bytes(&origin).expect("encode signed-CRL record");
        let decoded: PrivacyZkX509CrlRecordV1 =
            norito::decode_from_bytes(&encoded).expect("decode signed-CRL record");
        assert_eq!(decoded, origin);
        decoded
            .validate_initial()
            .expect("decoded signed-CRL record validates");
        let json = norito::json::to_json(&origin).expect("encode signed-CRL JSON");
        let decoded_json: PrivacyZkX509CrlRecordV1 =
            norito::json::from_json(&json).expect("decode signed-CRL JSON");
        assert_eq!(decoded_json, origin);
        let object_prefix = json
            .strip_suffix('}')
            .expect("signed-CRL JSON is an object");
        assert!(
            norito::json::from_json::<PrivacyZkX509CrlRecordV1>(&format!(
                "{object_prefix},\"legacy_crl\":true}}"
            ))
            .is_err()
        );

        let construct = |trust_anchor_id,
                         certificate_policy_id,
                         record_epoch,
                         crl_number,
                         crl_der_digest,
                         issuer_spki_digest,
                         this_update,
                         next_update,
                         previous_record_digest,
                         lifecycle| {
            PrivacyZkX509CrlRecordV1::new(
                trust_anchor_id,
                certificate_policy_id,
                record_epoch,
                crl_number,
                crl_der_digest,
                issuer_spki_digest,
                this_update,
                next_update,
                previous_record_digest,
                lifecycle,
            )
        };
        assert_eq!(
            construct(
                origin.trust_anchor_id,
                origin.certificate_policy_id,
                1,
                1,
                PrivacyX509CrlDerDigestV1::new([0; 32]),
                origin.issuer_spki_digest,
                1_000,
                1_300,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            ),
            Err(PrivacyZkX509RecordValidationErrorV1::ZeroCrlDerDigest)
        );
        assert_eq!(
            construct(
                origin.trust_anchor_id,
                origin.certificate_policy_id,
                1,
                1,
                origin.crl_der_digest,
                PrivacyX509CrlIssuerSpkiDigestV1::new([0; 32]),
                1_000,
                1_300,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            ),
            Err(PrivacyZkX509RecordValidationErrorV1::ZeroCrlIssuerSpkiDigest)
        );
        for (this_update, next_update) in [(1_000, 1_000), (1_001, 1_000)] {
            assert_eq!(
                construct(
                    origin.trust_anchor_id,
                    origin.certificate_policy_id,
                    1,
                    1,
                    origin.crl_der_digest,
                    origin.issuer_spki_digest,
                    this_update,
                    next_update,
                    None,
                    PrivacyZkX509RecordLifecycleV1::Active,
                ),
                Err(PrivacyZkX509RecordValidationErrorV1::InvalidCrlValidityWindow)
            );
        }
        let mut tampered = origin;
        tampered.next_update_unix_seconds += 1;
        assert_eq!(
            tampered.validate(),
            Err(PrivacyZkX509RecordValidationErrorV1::RecordDigestMismatch)
        );

        let rotation = zk_x509_crl(
            2,
            102,
            1_200,
            103,
            Some(origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        validate_zk_x509_crl_rotation_v1(&origin, &rotation)
            .expect("canonical signed-CRL rotation");
        let mut rotation_revocation = zk_x509_crl(
            3,
            102,
            1_200,
            103,
            Some(rotation.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        rotation_revocation.crl_number = rotation.crl_number;
        rotation_revocation.record_digest = rotation_revocation
            .compute_record_digest()
            .expect("canonical post-rotation CRL revocation digest");
        validate_zk_x509_crl_revocation_v1(&rotation, &rotation_revocation)
            .expect("revocation preserves the most recent complete signed CRL");

        let stale_update = zk_x509_crl(
            2,
            102,
            1_000,
            103,
            Some(origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        assert_eq!(
            validate_zk_x509_crl_rotation_v1(&origin, &stale_update),
            Err(PrivacyZkX509TransitionValidationErrorV1::CrlThisUpdateNotIncreasing)
        );
        let mut stale_number = rotation;
        stale_number.crl_number = origin.crl_number;
        stale_number.record_digest = stale_number
            .compute_record_digest()
            .expect("stale CRLNumber digest");
        assert_eq!(
            validate_zk_x509_crl_rotation_v1(&origin, &stale_number),
            Err(PrivacyZkX509TransitionValidationErrorV1::CrlNumberNotIncreasing)
        );
        let same_der = zk_x509_crl(
            2,
            100,
            1_200,
            103,
            Some(origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        assert_eq!(
            validate_zk_x509_crl_rotation_v1(&origin, &same_der),
            Err(PrivacyZkX509TransitionValidationErrorV1::RotationContentsUnchanged)
        );

        let mut issuer_substitution = rotation;
        issuer_substitution.issuer_spki_digest = PrivacyX509CrlIssuerSpkiDigestV1::new(raw(104));
        issuer_substitution.record_digest = issuer_substitution
            .compute_record_digest()
            .expect("issuer-substitution digest");
        assert_eq!(
            validate_zk_x509_crl_rotation_v1(&origin, &issuer_substitution),
            Err(PrivacyZkX509TransitionValidationErrorV1::CrlIssuerSpkiDigestMismatch)
        );

        let mut revoked = zk_x509_crl(
            2,
            100,
            1_000,
            101,
            Some(origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        revoked.crl_number = origin.crl_number;
        revoked.record_digest = revoked
            .compute_record_digest()
            .expect("canonical signed-CRL revocation digest");
        validate_zk_x509_crl_revocation_v1(&origin, &revoked)
            .expect("canonical signed-CRL lineage revocation");
        let after_terminal = zk_x509_crl(
            3,
            105,
            1_400,
            106,
            Some(revoked.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        assert_eq!(
            validate_zk_x509_crl_rotation_v1(&revoked, &after_terminal),
            Err(PrivacyZkX509TransitionValidationErrorV1::CurrentNotActive)
        );
        let mut mutated_revocation = zk_x509_crl(
            2,
            100,
            1_000,
            107,
            Some(origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        mutated_revocation.crl_number = origin.crl_number;
        mutated_revocation.record_digest = mutated_revocation
            .compute_record_digest()
            .expect("mutated signed-CRL revocation digest");
        assert_eq!(
            validate_zk_x509_crl_revocation_v1(&origin, &mutated_revocation),
            Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged)
        );
    }

    #[test]
    fn zk_x509_policy_caps_ordering_and_transition_adversaries_fail_closed() {
        let key_usage = PrivacyX509KeyUsageV1 {
            digital_signature: true,
            content_commitment: false,
            key_encipherment: false,
            key_agreement: false,
        };
        let construct_policy = |extended_key_usages, disclosures| {
            PrivacyZkX509CertificatePolicyRecordV1::new(
                PrivacyIssuerIdV1::new(raw(61)),
                PrivacyPolicyIdV1::new(raw(62)),
                1,
                PrivacyPolicyDigestV1::new(raw(90)),
                key_usage,
                extended_key_usages,
                disclosures,
                None,
                PrivacyZkX509RecordLifecycleV1::Active,
            )
        };
        construct_policy(
            vec![
                PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                PrivacyX509ExtendedKeyUsageV1::DocumentSigning,
                PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
            ],
            vec![0, 1, 2, 3],
        )
        .expect("exact EKU and disclosure caps are valid");
        assert!(matches!(
            construct_policy(
                vec![
                    PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                    PrivacyX509ExtendedKeyUsageV1::DocumentSigning,
                    PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
                    PrivacyX509ExtendedKeyUsageV1::WalletIdentity,
                ],
                vec![]
            ),
            Err(
                PrivacyZkX509RecordValidationErrorV1::TooManyExtendedKeyUsages {
                    actual: 4,
                    max: ZK_X509_MAX_EXTENDED_KEY_USAGES_V1
                }
            )
        ));
        assert!(matches!(
            construct_policy(
                vec![
                    PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                    PrivacyX509ExtendedKeyUsageV1::ClientAuthentication,
                ],
                vec![]
            ),
            Err(PrivacyZkX509RecordValidationErrorV1::ExtendedKeyUsagesNotStrictlyIncreasing)
        ));
        assert!(matches!(
            construct_policy(
                vec![PrivacyX509ExtendedKeyUsageV1::ClientAuthentication],
                vec![0, 1, 2, 3, 4]
            ),
            Err(
                PrivacyZkX509RecordValidationErrorV1::TooManyDisclosedAttributes {
                    actual: 5,
                    max: ZK_X509_MAX_DISCLOSED_ATTRIBUTES_V1
                }
            )
        ));
        assert!(matches!(
            construct_policy(
                vec![PrivacyX509ExtendedKeyUsageV1::ClientAuthentication],
                vec![0, 2, 2]
            ),
            Err(
                PrivacyZkX509RecordValidationErrorV1::DisclosedAttributeIndicesNotStrictlyIncreasing
            )
        ));

        let anchor_origin =
            zk_x509_trust_anchor(1, 91, None, PrivacyZkX509RecordLifecycleV1::Active);
        let anchor_rotation = zk_x509_trust_anchor(
            2,
            92,
            Some(anchor_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        validate_zk_x509_trust_anchor_rotation_v1(&anchor_origin, &anchor_rotation)
            .expect("canonical trust-anchor rotation");
        let mut digest_only_rotation = anchor_rotation;
        digest_only_rotation.ca_membership_root = anchor_origin.ca_membership_root;
        digest_only_rotation.record_digest = digest_only_rotation
            .compute_record_digest()
            .expect("digest-only trust-anchor rotation digest");
        assert_eq!(
            validate_zk_x509_trust_anchor_rotation_v1(&anchor_origin, &digest_only_rotation),
            Err(
                PrivacyZkX509TransitionValidationErrorV1::TrustStoreDigestChangedWithoutCaMembershipRoot
            )
        );
        let mut root_only_rotation = anchor_rotation;
        root_only_rotation.trust_store_digest = anchor_origin.trust_store_digest;
        root_only_rotation.record_digest = root_only_rotation
            .compute_record_digest()
            .expect("root-only trust-anchor rotation digest");
        assert_eq!(
            validate_zk_x509_trust_anchor_rotation_v1(&anchor_origin, &root_only_rotation),
            Err(
                PrivacyZkX509TransitionValidationErrorV1::CaMembershipRootChangedWithoutTrustStoreDigest
            )
        );
        let rotation_revocation = zk_x509_trust_anchor(
            3,
            92,
            Some(anchor_rotation.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        validate_zk_x509_trust_anchor_revocation_v1(&anchor_rotation, &rotation_revocation)
            .expect("trust-anchor revocation preserves the latest active CA-root epoch");
        let mut substituted_historical_root_epoch = rotation_revocation;
        substituted_historical_root_epoch.ca_membership_root_epoch =
            anchor_origin.ca_membership_root_epoch;
        substituted_historical_root_epoch.record_digest = substituted_historical_root_epoch
            .compute_record_digest()
            .expect("historical CA-root epoch substitution digest");
        assert_eq!(
            validate_zk_x509_trust_anchor_revocation_v1(
                &anchor_rotation,
                &substituted_historical_root_epoch,
            ),
            Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged)
        );
        let anchor_noop = zk_x509_trust_anchor(
            2,
            91,
            Some(anchor_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        assert_eq!(
            validate_zk_x509_trust_anchor_rotation_v1(&anchor_origin, &anchor_noop),
            Err(PrivacyZkX509TransitionValidationErrorV1::RotationContentsUnchanged)
        );
        let anchor_skipped = zk_x509_trust_anchor(
            3,
            92,
            Some(anchor_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        assert!(matches!(
            validate_zk_x509_trust_anchor_rotation_v1(&anchor_origin, &anchor_skipped),
            Err(
                PrivacyZkX509TransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                    expected: 2,
                    actual: 3
                }
            )
        ));
        let anchor_revoked = zk_x509_trust_anchor(
            2,
            91,
            Some(anchor_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        validate_zk_x509_trust_anchor_revocation_v1(&anchor_origin, &anchor_revoked)
            .expect("canonical trust-anchor revocation");
        let after_terminal = zk_x509_trust_anchor(
            3,
            93,
            Some(anchor_revoked.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        assert_eq!(
            validate_zk_x509_trust_anchor_rotation_v1(&anchor_revoked, &after_terminal),
            Err(PrivacyZkX509TransitionValidationErrorV1::CurrentNotActive)
        );

        let policy_origin = zk_x509_certificate_policy(
            1,
            94,
            vec![0, 3],
            None,
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        let policy_rotation = zk_x509_certificate_policy(
            2,
            95,
            vec![0, 2, 3],
            Some(policy_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Active,
        );
        validate_zk_x509_certificate_policy_rotation_v1(&policy_origin, &policy_rotation)
            .expect("canonical policy rotation");
        let policy_revoked = zk_x509_certificate_policy(
            2,
            94,
            vec![0, 3],
            Some(policy_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        validate_zk_x509_certificate_policy_revocation_v1(&policy_origin, &policy_revoked)
            .expect("canonical policy revocation");
        let mutated_revocation = zk_x509_certificate_policy(
            2,
            96,
            vec![0, 3],
            Some(policy_origin.record_digest),
            PrivacyZkX509RecordLifecycleV1::Revoked,
        );
        assert_eq!(
            validate_zk_x509_certificate_policy_revocation_v1(&policy_origin, &mutated_revocation),
            Err(PrivacyZkX509TransitionValidationErrorV1::RevocationContentsChanged)
        );
    }

    #[test]
    fn bootle_lantern_disclosures_are_fixed_direct_and_canonically_ordered() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let base = statement_for(PrivacyProtocolIdV1::IrohaBootleLanternAnoncredV1);
        let mutate = |f: fn(&mut IrohaBootleLanternAnoncredStatementV1)| {
            let mut value = base.clone();
            let PrivacyStatementV1::IrohaBootleLanternAnoncredV1(statement) = &mut value else {
                unreachable!()
            };
            f(statement);
            value.validate(&limits)
        };
        assert!(matches!(
            mutate(|statement| statement.issuer_policy_epoch = 0),
            Err(PrivacyStatementValidationError::ZeroEpoch {
                field: PrivacyEpochFieldV1::IssuerPolicy
            })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.issuer_policy_record_digest =
                    PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32])
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::IssuerPolicyRecordDigest
            })
        ));
        assert!(matches!(
            mutate(|statement| statement.disclosures.swap(0, 1)),
            Err(PrivacyStatementValidationError::BootleLanternDisclosuresNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate(|statement| statement.disclosures[1].index = 8),
            Err(
                PrivacyStatementValidationError::BootleLanternDisclosureIndexOutOfBounds {
                    index: 8
                }
            )
        ));
        assert!(matches!(
            mutate(|statement| statement.disclosures[1].index = statement.disclosures[0].index),
            Err(PrivacyStatementValidationError::BootleLanternDisclosuresNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.disclosures = (0_u8..=8)
                    .map(|index| BootleLanternDisclosedAttributeV1 {
                        index,
                        value: BootleLanternAttributeValueV1::new([index; 8]),
                    })
                    .collect()
            }),
            Err(
                PrivacyStatementValidationError::TooManyBootleLanternDisclosures {
                    count: 9,
                    max: 8
                }
            )
        ));

        let mut all_boundaries = base;
        let PrivacyStatementV1::IrohaBootleLanternAnoncredV1(statement) = &mut all_boundaries
        else {
            unreachable!()
        };
        statement.disclosures = (0_u8..8)
            .map(|index| BootleLanternDisclosedAttributeV1 {
                index,
                value: BootleLanternAttributeValueV1::new(if index.is_multiple_of(2) {
                    [0; 8]
                } else {
                    [u8::MAX; 8]
                }),
            })
            .collect();
        all_boundaries
            .validate(&limits)
            .expect("all eight direct zero/maximum values are canonical");
    }

    #[test]
    fn zk_ace_policy_record_is_canonical_self_digested_and_roundtrips() {
        let record = zk_ace_policy(
            PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
            11,
            PrivacyZkAcePolicyLifecycleV1::Active,
        );
        record.validate_initial().expect("canonical initial policy");
        assert_eq!(
            record
                .compute_record_digest()
                .expect("recompute canonical policy digest"),
            record.record_digest
        );

        let encoded = norito::to_bytes(&record).expect("encode ZK-ACE policy");
        let decoded: PrivacyZkAcePolicyRecordV1 =
            norito::decode_from_bytes(&encoded).expect("decode ZK-ACE policy");
        assert_eq!(decoded, record);
        decoded
            .validate_initial()
            .expect("decoded policy validates");

        let json = norito::json::to_json(&record).expect("encode ZK-ACE policy JSON");
        let decoded_json: PrivacyZkAcePolicyRecordV1 =
            norito::json::from_json(&json).expect("decode ZK-ACE policy JSON");
        assert_eq!(decoded_json, record);
        decoded_json
            .validate_initial()
            .expect("JSON-decoded policy validates");
        let object_prefix = json
            .strip_suffix('}')
            .expect("policy JSON is a top-level object");
        let unknown_field = format!("{object_prefix},\"unexpected_policy_alias\":true}}");
        assert!(
            norito::json::from_json::<PrivacyZkAcePolicyRecordV1>(&unknown_field).is_err(),
            "unknown JSON fields must not create an alternate first-release policy encoding"
        );

        let mut zero_digest = record.clone();
        zero_digest.record_digest = PrivacyZkAcePolicyRecordDigestV1::new([0; 32]);
        assert_eq!(
            zero_digest.validate(),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroRecordDigest)
        );

        let mut tamperings = Vec::new();
        let mut tampered = record.clone();
        tampered.policy_id = PrivacyPolicyIdV1::new(raw(90));
        tamperings.push(tampered);
        let mut tampered = record.clone();
        tampered.identity_commitment = commitment(91);
        tamperings.push(tampered);
        let mut tampered = record.clone();
        tampered.policy_digest = PrivacyPolicyDigestV1::new(raw(92));
        tamperings.push(tampered);
        let mut tampered = record.clone();
        tampered.authorization_epoch = 2;
        tamperings.push(tampered);
        let mut tampered = record.clone();
        tampered.asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("other_asset").expect("asset name"),
        );
        tamperings.push(tampered);
        let mut tampered = record.clone();
        tampered.source_allowlist.push(account(99));
        tampered.source_allowlist.sort_unstable();
        tamperings.push(tampered);
        let mut tampered = record;
        tampered.lifecycle = PrivacyZkAcePolicyLifecycleV1::Revoked;
        tamperings.push(tampered);
        for tampered in tamperings {
            assert_eq!(
                tampered.validate(),
                Err(PrivacyZkAcePolicyRecordValidationErrorV1::RecordDigestMismatch)
            );
        }
    }

    #[test]
    fn zk_ace_policy_registration_rejects_every_noncanonical_boundary() {
        let policy_id = PrivacyPolicyIdV1::new(raw(10));
        let identity = commitment(11);
        let digest = PrivacyPolicyDigestV1::new(raw(12));
        let asset = asset_definition_id();
        let allowlist = zk_ace_allowlist();
        let construct = |policy_id,
                         identity_commitment,
                         policy_digest,
                         authorization_epoch,
                         source_allowlist,
                         lifecycle| {
            PrivacyZkAcePolicyRecordV1::new(
                policy_id,
                identity_commitment,
                policy_digest,
                authorization_epoch,
                asset.clone(),
                source_allowlist,
                lifecycle,
            )
        };

        assert_eq!(
            construct(
                PrivacyPolicyIdV1::new([0; 32]),
                identity,
                digest,
                1,
                allowlist.clone(),
                PrivacyZkAcePolicyLifecycleV1::Active,
            ),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroPolicyId)
        );
        assert_eq!(
            construct(
                policy_id,
                PrivacyCommitmentV1::new([0; 32]),
                digest,
                1,
                allowlist.clone(),
                PrivacyZkAcePolicyLifecycleV1::Active,
            ),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroIdentityCommitment)
        );
        assert_eq!(
            construct(
                policy_id,
                identity,
                PrivacyPolicyDigestV1::new([0; 32]),
                1,
                allowlist.clone(),
                PrivacyZkAcePolicyLifecycleV1::Active,
            ),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroPolicyDigest)
        );
        assert_eq!(
            construct(
                policy_id,
                identity,
                digest,
                0,
                allowlist.clone(),
                PrivacyZkAcePolicyLifecycleV1::Active,
            ),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::ZeroAuthorizationEpoch)
        );
        assert_eq!(
            construct(
                policy_id,
                identity,
                digest,
                1,
                Vec::new(),
                PrivacyZkAcePolicyLifecycleV1::Active,
            ),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::EmptySourceAllowlist)
        );

        let over_limit = vec![account(20); PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1 + 1];
        assert_eq!(
            construct(
                policy_id,
                identity,
                digest,
                1,
                over_limit,
                PrivacyZkAcePolicyLifecycleV1::Active,
            ),
            Err(
                PrivacyZkAcePolicyRecordValidationErrorV1::SourceAllowlistTooLarge {
                    actual: PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1 + 1,
                    max: PRIVACY_ZK_ACE_MAX_SOURCE_ACCOUNTS_V1,
                }
            )
        );

        let mut reversed = allowlist.clone();
        reversed.reverse();
        assert_eq!(
            construct(
                policy_id,
                identity,
                digest,
                1,
                reversed,
                PrivacyZkAcePolicyLifecycleV1::Active,
            ),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalSourceAllowlist)
        );
        let duplicate = vec![allowlist[0].clone(), allowlist[0].clone()];
        assert_eq!(
            construct(
                policy_id,
                identity,
                digest,
                1,
                duplicate,
                PrivacyZkAcePolicyLifecycleV1::Active,
            ),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalSourceAllowlist)
        );

        let noncanonical_epoch = zk_ace_policy(2, 11, PrivacyZkAcePolicyLifecycleV1::Active);
        assert_eq!(
            noncanonical_epoch.validate_initial(),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::NonCanonicalInitialEpoch { actual: 2 })
        );
        let initially_revoked = zk_ace_policy(
            PRIVACY_ZK_ACE_POLICY_INITIAL_EPOCH_V1,
            11,
            PrivacyZkAcePolicyLifecycleV1::Revoked,
        );
        assert_eq!(
            initially_revoked.validate_initial(),
            Err(PrivacyZkAcePolicyRecordValidationErrorV1::InitialPolicyNotActive)
        );
    }

    #[test]
    fn zk_ace_rotation_rejects_replays_skips_noops_and_terminal_policies() {
        let current = zk_ace_policy(1, 11, PrivacyZkAcePolicyLifecycleV1::Active);
        let successor = zk_ace_policy(2, 21, PrivacyZkAcePolicyLifecycleV1::Active);
        validate_zk_ace_policy_rotation_v1(&current, &successor)
            .expect("canonical one-epoch identity rotation");

        let mut invalid_current = current.clone();
        invalid_current.record_digest = PrivacyZkAcePolicyRecordDigestV1::new(raw(90));
        assert_eq!(
            validate_zk_ace_policy_rotation_v1(&invalid_current, &successor),
            Err(
                PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidCurrent(
                    PrivacyZkAcePolicyRecordValidationErrorV1::RecordDigestMismatch
                )
            )
        );
        let mut invalid_successor = successor.clone();
        invalid_successor.record_digest = PrivacyZkAcePolicyRecordDigestV1::new(raw(91));
        assert_eq!(
            validate_zk_ace_policy_rotation_v1(&current, &invalid_successor),
            Err(
                PrivacyZkAcePolicyTransitionValidationErrorV1::InvalidSuccessor(
                    PrivacyZkAcePolicyRecordValidationErrorV1::RecordDigestMismatch
                )
            )
        );

        let mut different_policy = successor.clone();
        different_policy.policy_id = PrivacyPolicyIdV1::new(raw(92));
        redigest_zk_ace_policy(&mut different_policy);
        assert_eq!(
            validate_zk_ace_policy_rotation_v1(&current, &different_policy),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::PolicyIdMismatch)
        );

        for epoch in [1, 3] {
            let candidate = zk_ace_policy(epoch, 21, PrivacyZkAcePolicyLifecycleV1::Active);
            assert!(matches!(
                validate_zk_ace_policy_rotation_v1(&current, &candidate),
                Err(
                    PrivacyZkAcePolicyTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                        expected: 2,
                        actual
                    }
                ) if actual == epoch
            ));
        }

        let revoked_successor = zk_ace_policy(2, 21, PrivacyZkAcePolicyLifecycleV1::Revoked);
        assert_eq!(
            validate_zk_ace_policy_rotation_v1(&current, &revoked_successor),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RotationSuccessorNotActive)
        );
        let no_op = zk_ace_policy(2, 11, PrivacyZkAcePolicyLifecycleV1::Active);
        assert_eq!(
            validate_zk_ace_policy_rotation_v1(&current, &no_op),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::IdentityCommitmentUnchanged)
        );

        let revoked_current = zk_ace_policy(1, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
        assert_eq!(
            validate_zk_ace_policy_rotation_v1(&revoked_current, &successor),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::CurrentNotActive)
        );
        let max_epoch = zk_ace_policy(u64::MAX, 11, PrivacyZkAcePolicyLifecycleV1::Active);
        let max_successor = zk_ace_policy(u64::MAX, 21, PrivacyZkAcePolicyLifecycleV1::Active);
        assert_eq!(
            validate_zk_ace_policy_rotation_v1(&max_epoch, &max_successor),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::EpochOverflow)
        );
    }

    #[test]
    fn zk_ace_revocation_is_one_step_terminal_and_content_preserving() {
        let current = zk_ace_policy(1, 11, PrivacyZkAcePolicyLifecycleV1::Active);
        let successor = zk_ace_policy(2, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
        validate_zk_ace_policy_revocation_v1(&current, &successor)
            .expect("canonical one-epoch revocation");

        let active_successor = zk_ace_policy(2, 11, PrivacyZkAcePolicyLifecycleV1::Active);
        assert_eq!(
            validate_zk_ace_policy_revocation_v1(&current, &active_successor),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RevocationSuccessorNotRevoked)
        );

        let mut mutations = Vec::new();
        let mut changed_identity = successor.clone();
        changed_identity.identity_commitment = commitment(21);
        redigest_zk_ace_policy(&mut changed_identity);
        mutations.push(changed_identity);
        let mut changed_policy_digest = successor.clone();
        changed_policy_digest.policy_digest = PrivacyPolicyDigestV1::new(raw(22));
        redigest_zk_ace_policy(&mut changed_policy_digest);
        mutations.push(changed_policy_digest);
        let mut changed_asset = successor.clone();
        changed_asset.asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("other_asset").expect("asset name"),
        );
        redigest_zk_ace_policy(&mut changed_asset);
        mutations.push(changed_asset);
        let mut changed_allowlist = successor.clone();
        changed_allowlist.source_allowlist.push(account(99));
        changed_allowlist.source_allowlist.sort_unstable();
        redigest_zk_ace_policy(&mut changed_allowlist);
        mutations.push(changed_allowlist);
        for mutation in mutations {
            assert_eq!(
                validate_zk_ace_policy_revocation_v1(&current, &mutation),
                Err(PrivacyZkAcePolicyTransitionValidationErrorV1::RevocationContentsChanged)
            );
        }

        let mut different_policy = successor.clone();
        different_policy.policy_id = PrivacyPolicyIdV1::new(raw(92));
        redigest_zk_ace_policy(&mut different_policy);
        assert_eq!(
            validate_zk_ace_policy_revocation_v1(&current, &different_policy),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::PolicyIdMismatch)
        );
        for epoch in [1, 3] {
            let candidate = zk_ace_policy(epoch, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
            assert!(matches!(
                validate_zk_ace_policy_revocation_v1(&current, &candidate),
                Err(
                    PrivacyZkAcePolicyTransitionValidationErrorV1::NonCanonicalSuccessorEpoch {
                        expected: 2,
                        actual
                    }
                ) if actual == epoch
            ));
        }
        let revoked_current = zk_ace_policy(1, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
        assert_eq!(
            validate_zk_ace_policy_revocation_v1(&revoked_current, &successor),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::CurrentNotActive)
        );
        let max_epoch = zk_ace_policy(u64::MAX, 11, PrivacyZkAcePolicyLifecycleV1::Active);
        let max_successor = zk_ace_policy(u64::MAX, 11, PrivacyZkAcePolicyLifecycleV1::Revoked);
        assert_eq!(
            validate_zk_ace_policy_revocation_v1(&max_epoch, &max_successor),
            Err(PrivacyZkAcePolicyTransitionValidationErrorV1::EpochOverflow)
        );
    }

    #[test]
    fn bootle_lantern_issuer_policy_is_canonical_bounded_and_forward_only() {
        let record = bootle_lantern_policy();
        record.validate_initial().expect("canonical initial record");
        assert_eq!(
            record
                .computed_record_digest()
                .expect("canonical record digest"),
            record.record_digest
        );
        let encoded = norito::to_bytes(&record).expect("encode policy");
        let decoded: BootleLanternIssuerPolicyV1 =
            norito::decode_from_bytes(&encoded).expect("decode policy");
        assert_eq!(decoded, record);
        let json = norito::json::to_json(&record).expect("encode policy JSON");
        let decoded_json: BootleLanternIssuerPolicyV1 =
            norito::json::from_json(&json).expect("decode policy JSON");
        assert_eq!(decoded_json, record);
        let object_prefix = json
            .strip_suffix('}')
            .expect("policy JSON is a top-level object");
        let unknown_field = format!("{object_prefix},\"legacy_policy_alias\":true}}");
        assert!(
            norito::json::from_json::<BootleLanternIssuerPolicyV1>(&unknown_field).is_err(),
            "unknown JSON fields must not create an alternate first-release policy encoding"
        );

        let mut invalid = record.clone();
        invalid.issuer_public_matrix.entries.pop();
        assert!(matches!(
            invalid.validate(),
            Err(
                BootleLanternIssuerPolicyValidationErrorV1::InvalidIssuerMatrixEntryCount {
                    count: 63,
                    expected: 64
                }
            )
        ));
        invalid = record.clone();
        invalid.issuer_public_matrix.entries[0].coefficients.pop();
        assert!(matches!(
            invalid.validate(),
            Err(
                BootleLanternIssuerPolicyValidationErrorV1::InvalidPolynomialCoefficientCount {
                    polynomial: 0,
                    count: 63,
                    expected: 64
                }
            )
        ));
        invalid = record.clone();
        invalid.issuer_public_matrix.entries[0].coefficients[0] =
            BOOTLE_LANTERN_APPLICATION_MODULUS_V1;
        assert!(matches!(
            invalid.validate(),
            Err(
                BootleLanternIssuerPolicyValidationErrorV1::NonCanonicalMatrixCoefficient {
                    row: 0,
                    column: 0,
                    coefficient: 0,
                    value: BOOTLE_LANTERN_APPLICATION_MODULUS_V1
                }
            )
        ));
        invalid = record.clone();
        for polynomial in &mut invalid.issuer_public_matrix.entries {
            polynomial.coefficients.fill(0);
        }
        assert_eq!(
            invalid.validate(),
            Err(BootleLanternIssuerPolicyValidationErrorV1::AllZeroIssuerMatrix)
        );
        invalid = record.clone();
        invalid.issuer_parameter_digest.0[0] ^= 1;
        assert_eq!(
            invalid.validate(),
            Err(BootleLanternIssuerPolicyValidationErrorV1::IssuerParameterDigestMismatch)
        );
        invalid = record.clone();
        invalid.allowed_values.pop();
        assert!(matches!(
            invalid.validate(),
            Err(
                BootleLanternIssuerPolicyValidationErrorV1::InvalidAllowedValueRuleCount {
                    count: 7,
                    expected: 8
                }
            )
        ));
        invalid = record.clone();
        invalid.allowed_values[0]
            .values
            .push(BootleLanternAttributeValueV1::new([1; 8]));
        assert_eq!(
            invalid.validate(),
            Err(
                BootleLanternIssuerPolicyValidationErrorV1::AllowedValuesForOptionalAttribute {
                    index: 0
                }
            )
        );
        invalid = record.clone();
        invalid.allowed_values[1].values = vec![
            BootleLanternAttributeValueV1::new([2; 8]),
            BootleLanternAttributeValueV1::new([2; 8]),
        ];
        assert_eq!(
            invalid.validate(),
            Err(
                BootleLanternIssuerPolicyValidationErrorV1::AllowedValuesNotStrictlyIncreasing {
                    index: 1
                }
            )
        );
        invalid = record.clone();
        invalid.allowed_values[1].values =
            vec![
                BootleLanternAttributeValueV1::new([3; 8]);
                usize::try_from(BOOTLE_LANTERN_MAX_ALLOWED_VALUES_PER_ATTRIBUTE_V1 + 1)
                    .expect("test bound fits usize")
            ];
        assert!(matches!(
            invalid.validate(),
            Err(
                BootleLanternIssuerPolicyValidationErrorV1::TooManyAllowedValues {
                    index: 1,
                    count: 33,
                    max: 32
                }
            )
        ));
        invalid = record.clone();
        invalid.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new([0; 32]);
        assert_eq!(
            invalid.validate(),
            Err(BootleLanternIssuerPolicyValidationErrorV1::ZeroRecordDigest)
        );
        invalid = record.clone();
        invalid.record_digest = PrivacyBootleLanternIssuerPolicyDigestV1::new(raw(199));
        assert_eq!(
            invalid.validate(),
            Err(BootleLanternIssuerPolicyValidationErrorV1::RecordDigestMismatch)
        );

        let mut successor = record.clone();
        successor.epoch = 2;
        successor.required_disclosure_bitmap |= 1;
        redigest_bootle_lantern_policy(&mut successor);
        successor
            .validate_successor(&record)
            .expect("strict policy rotation");

        let mut non_consecutive = successor.clone();
        non_consecutive.epoch = 3;
        redigest_bootle_lantern_policy(&mut non_consecutive);
        assert!(matches!(
            non_consecutive.validate_successor(&record),
            Err(
                BootleLanternIssuerPolicyValidationErrorV1::NonConsecutiveEpoch {
                    previous: 1,
                    next: 3,
                    expected: 2
                }
            )
        ));
        let mut unchanged = record.clone();
        unchanged.epoch = 2;
        redigest_bootle_lantern_policy(&mut unchanged);
        assert_eq!(
            unchanged.validate_successor(&record),
            Err(BootleLanternIssuerPolicyValidationErrorV1::UnchangedRotation)
        );
        let mut revoked = record.clone();
        revoked.epoch = 2;
        revoked.lifecycle = BootleLanternIssuerPolicyLifecycleV1::Revoked;
        redigest_bootle_lantern_policy(&mut revoked);
        revoked
            .validate_revocation_successor(&record)
            .expect("exact terminal revocation");
        let mut revocation_with_rotation = revoked.clone();
        revocation_with_rotation.required_disclosure_bitmap ^= 1;
        redigest_bootle_lantern_policy(&mut revocation_with_rotation);
        assert_eq!(
            revocation_with_rotation.validate_revocation_successor(&record),
            Err(BootleLanternIssuerPolicyValidationErrorV1::RevocationMustPreservePolicy)
        );
        assert_eq!(
            revoked.validate_successor(&revoked),
            Err(BootleLanternIssuerPolicyValidationErrorV1::PolicyAlreadyRevoked)
        );

        let mut wrong_initial_epoch = record.clone();
        wrong_initial_epoch.epoch = 2;
        redigest_bootle_lantern_policy(&mut wrong_initial_epoch);
        assert_eq!(
            wrong_initial_epoch.validate_initial(),
            Err(BootleLanternIssuerPolicyValidationErrorV1::InvalidInitialEpoch { epoch: 2 })
        );
        let mut revoked_initial = record;
        revoked_initial.lifecycle = BootleLanternIssuerPolicyLifecycleV1::Revoked;
        redigest_bootle_lantern_policy(&mut revoked_initial);
        assert_eq!(
            revoked_initial.validate_initial(),
            Err(BootleLanternIssuerPolicyValidationErrorV1::InitialPolicyMustBeActive)
        );
    }

    #[test]
    fn private_transfer_shapes_enforce_hard_caps_and_ordered_ciphertexts() {
        let limits = PrivacyConsensusLimitsV1::taira_default();

        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.actions.clear();
        assert!(matches!(
            orchard.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidOrchardActionCount { count: 0, max: 2 })
        ));

        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.actions = vec![
            orchard_action(110),
            orchard_action(120),
            orchard_action(130),
        ];
        assert!(matches!(
            orchard.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidOrchardActionCount { count: 3, max: 2 })
        ));

        for malformed_len in [
            ORCHARD_ENCRYPTED_NOTE_BYTES_V1 - 1,
            ORCHARD_ENCRYPTED_NOTE_BYTES_V1 + 1,
        ] {
            let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
            let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
                unreachable!()
            };
            statement.actions[0]
                .encrypted_note
                .resize(malformed_len, 0xA5);
            assert!(matches!(
                orchard.validate(&limits),
                Err(
                    PrivacyStatementValidationError::InvalidOrchardEncryptedNoteSize {
                        index: 0,
                        ..
                    }
                )
            ));
        }
        for malformed_len in [
            ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1 - 1,
            ORCHARD_OUTGOING_CIPHERTEXT_BYTES_V1 + 1,
        ] {
            let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
            let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
                unreachable!()
            };
            statement.actions[0]
                .outgoing_ciphertext
                .resize(malformed_len, 0xA5);
            assert!(matches!(
                orchard.validate(&limits),
                Err(
                    PrivacyStatementValidationError::InvalidOrchardOutgoingCiphertextSize {
                        index: 0,
                        ..
                    }
                )
            ));
        }

        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.actions.push(orchard_action(120));
        statement.actions[1].nullifier = statement.actions[0].nullifier;
        assert_eq!(
            orchard.validate(&limits),
            Err(PrivacyStatementValidationError::DuplicateOrchardNullifier { index: 1 })
        );

        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.actions.push(orchard_action(120));
        statement.actions[1].note_commitment = statement.actions[0].note_commitment;
        assert_eq!(
            orchard.validate(&limits),
            Err(PrivacyStatementValidationError::DuplicateOrchardNoteCommitment { index: 1 })
        );

        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.value_balance = PrivacyValueBalanceV1 {
            direction: PrivacyValueBalanceDirectionV1::OutOfPool,
            amount: ORCHARD_MAX_VALUE_BALANCE_V1 + 1,
        };
        assert_eq!(
            orchard.validate(&limits),
            Err(
                PrivacyStatementValidationError::OrchardValueBalanceOutOfRange {
                    amount: ORCHARD_MAX_VALUE_BALANCE_V1 + 1,
                    max: ORCHARD_MAX_VALUE_BALANCE_V1,
                }
            )
        );

        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.actions.push(orchard_action(120));
        statement.actions[0].nullifier = [0; 32];
        statement.actions[0].note_commitment = [0; 32];
        orchard
            .validate(&limits)
            .expect("zero is a canonical Pallas field encoding, not a schema sentinel");

        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.inputs = vec![fcmp_input(121), fcmp_input(122), fcmp_input(123)];
        assert!(matches!(
            fcmp.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidFcmpInputCount {
                count: 3,
                max: FCMP_MAX_INPUTS_V1
            })
        ));

        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.output_set_root.layers = 0;
        assert!(matches!(
            fcmp.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidFcmpTreeRoot(
                PrivacyFcmpTreeRootValidationErrorV1::InvalidLayerCount { layers: 0, .. }
            ))
        ));

        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.inputs[0].rerandomization_commitment = [0; 32];
        assert_eq!(
            fcmp.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidFcmpInput {
                index: 0,
                source: PrivacyFcmpInputValidationErrorV1::ZeroComponent {
                    component: PrivacyFcmpInputComponentV1::RerandomizationCommitment,
                },
            })
        );

        for duplicate_key_image in [true, false] {
            let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
            let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
                unreachable!()
            };
            statement.inputs.push(fcmp_input(141));
            if duplicate_key_image {
                statement.inputs[1].key_image = statement.inputs[0].key_image;
                assert_eq!(
                    fcmp.validate(&limits),
                    Err(PrivacyStatementValidationError::DuplicateFcmpKeyImage { index: 1 })
                );
            } else {
                statement.inputs[1].pseudo_out = statement.inputs[0].pseudo_out;
                assert_eq!(
                    fcmp.validate(&limits),
                    Err(PrivacyStatementValidationError::DuplicateFcmpPseudoOut { index: 1 })
                );
            }
        }

        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.outputs[0].amount_commitment = [0; 32];
        assert_eq!(
            fcmp.validate(&limits),
            Err(PrivacyStatementValidationError::InvalidFcmpOutput {
                index: 0,
                source: PrivacyFcmpOutputTupleValidationErrorV1::ZeroComponent {
                    component: PrivacyFcmpOutputComponentV1::AmountCommitment,
                },
            })
        );

        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.outputs.push(statement.outputs[0]);
        statement
            .encrypted_outputs
            .push(statement.encrypted_outputs[0].clone());
        assert_eq!(
            fcmp.validate(&limits),
            Err(PrivacyStatementValidationError::DuplicateFcmpOutputId { index: 1 })
        );

        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.encrypted_outputs[0].output_id = fcmp_output(222).output_id();
        assert_eq!(
            fcmp.validate(&limits),
            Err(PrivacyStatementValidationError::FcmpEncryptedOutputIdMismatch { index: 0 })
        );

        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.encrypted_outputs.clear();
        assert_eq!(
            fcmp.validate(&limits),
            Err(PrivacyStatementValidationError::MissingEncryptedOutput)
        );

        let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
            unreachable!()
        };
        statement.action_digest = PrivacyActionDigestV1::new([0; 32]);
        assert_eq!(
            ivm.validate(&limits),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::ActionDigest,
            })
        );

        let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
            unreachable!()
        };
        statement.action_digest.0[0] ^= 1;
        assert_eq!(
            ivm.validate(&limits),
            Err(PrivacyStatementValidationError::ActionDigestMismatch)
        );

        let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
            unreachable!()
        };
        statement.execution_epoch += 1;
        statement.action_digest = statement
            .computed_action_digest()
            .expect("recompute mismatched-epoch action digest");
        assert_eq!(
            ivm.validate(&limits),
            Err(PrivacyStatementValidationError::EpochBindingMismatch {
                field: PrivacyEpochFieldV1::Execution,
                root_epoch: 15,
                bound_epoch: 16,
            })
        );

        let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
            unreachable!()
        };
        statement.nullifiers = vec![nullifier(127), nullifier(128), nullifier(129)];
        statement.action_digest = statement
            .computed_action_digest()
            .expect("recompute oversized-input action digest");
        assert!(matches!(
            ivm.validate(&limits),
            Err(PrivacyStatementValidationError::TooManyNullifiers {
                count: 3,
                max: IVM_PRIVATE_NOTE_MAX_INPUTS_V1
            })
        ));

        let mut pq = statement_for(PrivacyProtocolIdV1::PqMaspStarkV0);
        let PrivacyStatementV1::PqMaspStarkV0(statement) = &mut pq else {
            unreachable!()
        };
        statement.output_commitments = vec![commitment(130), commitment(131), commitment(132)];
        assert!(matches!(
            pq.validate(&limits),
            Err(PrivacyStatementValidationError::TooManyCommitments {
                count: 3,
                max: PQ_MASP_MAX_OUTPUTS_V1
            })
        ));

        let mut pq = statement_for(PrivacyProtocolIdV1::PqMaspStarkV0);
        let PrivacyStatementV1::PqMaspStarkV0(statement) = &mut pq else {
            unreachable!()
        };
        statement.authorization_epoch += 1;
        assert_eq!(
            pq.validate(&limits),
            Err(PrivacyStatementValidationError::EpochBindingMismatch {
                field: PrivacyEpochFieldV1::Authorization,
                root_epoch: 17,
                bound_epoch: 18,
            })
        );

        let mut malformed = statement_for(PrivacyProtocolIdV1::PqMaspStarkV0);
        let PrivacyStatementV1::PqMaspStarkV0(statement) = &mut malformed else {
            unreachable!()
        };
        statement.encrypted_outputs[0].recipient = PrivacyRecipientIdV1::new([0; 32]);
        assert!(matches!(
            malformed.validate(&limits),
            Err(PrivacyStatementValidationError::ZeroEncryptedOutputRecipient { index: 0 })
        ));
    }

    #[test]
    fn private_ivm_encrypted_output_codec_is_exact_and_fail_closed() {
        assert_eq!(PRIVACY_IVM_PRIVATE_NOTE_PLAINTEXT_BYTES_V1, 180);
        assert_eq!(PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1, 224);

        let canonical = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &canonical else {
            unreachable!()
        };
        let ciphertext = statement.encrypted_outputs[0].ciphertext.clone();
        assert_eq!(
            ciphertext.len(),
            PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_BYTES_V1
        );
        assert_eq!(
            ciphertext.get(..4),
            Some(PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_MAGIC_V1.as_slice())
        );

        let mut malformed = Vec::new();
        let mut truncated = ciphertext.clone();
        truncated.pop();
        malformed.push(truncated);
        let mut suffixed = ciphertext.clone();
        suffixed.push(0);
        malformed.push(suffixed);
        let mut wrong_magic = ciphertext.clone();
        wrong_magic[0] ^= 1;
        malformed.push(wrong_magic);
        let mut zero_nonce = ciphertext.clone();
        zero_nonce[4..4 + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1].fill(0);
        malformed.push(zero_nonce);
        let mut zero_payload = ciphertext;
        zero_payload[4 + PRIVACY_IVM_PRIVATE_ENCRYPTED_OUTPUT_NONCE_BYTES_V1..].fill(0);
        malformed.push(zero_payload);

        for ciphertext in malformed {
            let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
            let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
                unreachable!()
            };
            statement.encrypted_outputs[0].ciphertext = ciphertext;
            statement.action_digest = statement
                .computed_action_digest()
                .expect("recompute malformed-codec action digest");
            assert_eq!(
                ivm.validate(&PrivacyConsensusLimitsV1::taira_default()),
                Err(
                    PrivacyStatementValidationError::InvalidIvmPrivateEncryptedOutputCodec {
                        index: 0,
                    }
                )
            );
        }
    }

    #[test]
    fn lifecycle_edges_preserve_history_and_retirement_is_terminal() {
        let proposed = PrivacyProtocolLifecycleV1::Proposed(PrivacyProposedLifecycleV1 {
            proposed_at_height: 1,
            activate_at_height: 3,
        });
        let active = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 3,
            state_since_height: 3,
        });
        let suspended = PrivacyProtocolLifecycleV1::Suspended(PrivacySuspendedLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 3,
            state_since_height: 4,
        });
        let resumed = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 3,
            state_since_height: 5,
        });
        let retired = PrivacyProtocolLifecycleV1::Retired(PrivacyRetiredLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: Some(3),
            state_since_height: 6,
        });
        proposed
            .validate_transition_to(&active)
            .expect("proposal activates");
        active
            .validate_transition_to(&suspended)
            .expect("active suspends");
        suspended
            .validate_transition_to(&resumed)
            .expect("suspension resumes");
        resumed
            .validate_transition_to(&retired)
            .expect("active retires");
        assert!(retired.validate_transition_to(&active).is_err());

        let invalid = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 3,
            activated_at_height: 3,
            state_since_height: 3,
        });
        assert!(invalid.validate().is_err());
        let rewritten_history =
            PrivacyProtocolLifecycleV1::Suspended(PrivacySuspendedLifecycleV1 {
                proposed_at_height: 2,
                activated_at_height: 3,
                state_since_height: 4,
            });
        assert!(active.validate_transition_to(&rewritten_history).is_err());
    }

    #[test]
    fn activation_effective_height_uses_active_lifecycle_payload() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let envelope = envelope(statement_for(
            PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
        ));
        let mut activation = activation(&envelope);
        activation.lifecycle = PrivacyProtocolLifecycleV1::Active(PrivacyActiveLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 5,
        });

        assert_eq!(
            envelope.validate_against_activation(&activation, &limits, 4),
            Err(
                PrivacyProofEnvelopeValidationError::ActivationNotEffective {
                    current_height: 4,
                    effective_height: 5,
                }
            )
        );
        assert_eq!(
            envelope.validate_against_activation(&activation, &limits, 5),
            Ok(())
        );
    }

    #[test]
    fn envelopes_fail_closed_on_every_binding_and_resource_mutation() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let statement = statement_for(PrivacyProtocolIdV1::VeRangeTransparentRangeV1);
        let base = envelope(statement);
        base.validate_with_limits(&limits).expect("valid envelope");

        let mut invalid = base.clone();
        invalid.protocol_id = PrivacyProtocolIdV1::ZkAcePqAuthorizationV0;
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.proof_system_id = PrivacyProofSystemIdV1::StarkFriSha256Goldilocks;
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.engine_id = PrivacyEngineIdV1::NativeJindo;
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.parameter_id = PrivacyParameterIdV1::new(raw(220));
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.parameter_digest = PrivacyParameterDigestV1::new([0; 32]);
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.verifier_digest = PrivacyVerifierDigestV1::new([0; 32]);
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.statement_schema_digest = PrivacyStatementSchemaDigestV1::new([0; 32]);
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.engine_manifest_digest = PrivacyEngineManifestDigestV1::new([0; 32]);
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.statement_digest = PrivacyStatementDigestV1::new(raw(221));
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.proof = PrivacyProofV1::ZkAcePqAuthorizationV0(PrivacyProofBytesV1::new(vec![1]));
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.proof =
            PrivacyProofV1::VeRangeTransparentRangeV1(PrivacyProofBytesV1::new(Vec::new()));
        assert!(invalid.validate_with_limits(&limits).is_err());
        invalid = base.clone();
        invalid.proof =
            PrivacyProofV1::VeRangeTransparentRangeV1(PrivacyProofBytesV1::new(vec![0; 3]));
        assert!(invalid.validate_with_limits(&limits).is_err());

        let mut proof_limited = limits;
        proof_limited.max_proof_bytes_per_action = 2;
        proof_limited.validate().expect("lower proof limit");
        assert!(base.validate_with_limits(&proof_limited).is_err());

        let mut governed = activation(&base);
        base.validate_against_activation(&governed, &limits, 2)
            .expect("active matching activation");
        assert!(
            base.validate_against_activation(&governed, &limits, 1)
                .is_err()
        );
        governed.parameter_digest = PrivacyParameterDigestV1::new(raw(222));
        assert!(
            base.validate_against_activation(&governed, &limits, 2)
                .is_err()
        );
        governed = activation(&base);
        governed.lifecycle = PrivacyProtocolLifecycleV1::Suspended(PrivacySuspendedLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 3,
        });
        assert!(
            base.validate_against_activation(&governed, &limits, 3)
                .is_err()
        );
        governed = activation(&base);
        governed.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 1,
            },
        );
        assert!(
            base.validate_against_activation(&governed, &limits, 2)
                .is_err()
        );

        let framed = norito::to_bytes(&base).expect("frame envelope");
        let mut truncated = framed.clone();
        truncated.pop();
        assert!(norito::decode_from_bytes::<PrivacyProofEnvelopeV1>(&truncated).is_err());
        let mut trailing = framed;
        trailing.push(0);
        assert!(norito::decode_from_bytes::<PrivacyProofEnvelopeV1>(&trailing).is_err());

        for unknown in [99_u32, u32::MAX] {
            assert!(PrivacyProofSystemIdV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
            assert!(PrivacyEngineIdV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
            assert!(PrivacyStatementV1::decode(&mut unknown.to_le_bytes().as_slice()).is_err());
        }
    }
}
