//! Canonical first-release privacy protocol wire types.
//!
//! The types in this module deliberately form a closed protocol surface.
//! Protocol identities, proof systems, and native verifier engines are separate
//! enums, and proof envelopes must bind all three together with governed
//! parameter, verifier, statement-schema, and engine-manifest digests. There
//! are no free-form identifiers, aliases, or fallback proof variants.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{AssetDefinitionId, ChainId, account::AccountId};

/// Domain separator used to hash canonical [`PrivacyStatementV1`] values.
pub const PRIVACY_STATEMENT_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:statement:v1";
/// Domain separator used to hash canonical [`PrivacyRootPublicationV1`] values.
pub const PRIVACY_ROOT_PUBLICATION_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:root-publication:v1";
/// Domain separator used to hash canonical [`PrivacyPgcAccountBootstrapV1`] payloads.
pub const PRIVACY_PGC_ACCOUNT_BOOTSTRAP_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:pgc-account-bootstrap:v1";
/// Domain separator used to hash canonical Anonymous PGC bootstrap proof bytes.
pub const PRIVACY_PGC_BOOTSTRAP_PROOF_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:pgc-bootstrap-proof:v1";
/// Domain separator for core's deterministic PGC account-state root derivation.
pub const PRIVACY_PGC_ACCOUNT_STATE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha:privacy:pgc-account-state-root:v1";

/// Maximum privacy actions admitted in one Taira transaction.
pub const TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1: u32 = 1;
/// Maximum privacy actions admitted in one Taira block.
pub const TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1: u32 = 2;
/// Maximum proof payload bytes admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1: u32 = 8 * 1024 * 1024;
/// Maximum canonical bytes admitted for one Anonymous PGC bootstrap proof.
pub const TAIRA_PRIVACY_MAX_PGC_BOOTSTRAP_PROOF_BYTES_V1: u32 = 4 * 1024 * 1024;
/// The only account-state epoch admitted for an Anonymous PGC bootstrap.
///
/// Successor proofs advance this epoch by exactly one. Keeping the origin
/// fixed prevents governance or a caller from creating ambiguous histories.
pub const PRIVACY_PGC_BOOTSTRAP_INITIAL_EPOCH_V1: u64 = 1;
/// Maximum encoded bytes admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_ACTION_BYTES_V1: u32 = 8 * 1024 * 1024;
/// Maximum privacy bytes admitted in one Taira transaction.
pub const TAIRA_PRIVACY_MAX_BYTES_PER_TRANSACTION_V1: u32 = 8 * 1024 * 1024;
/// Maximum privacy bytes admitted in one Taira block.
pub const TAIRA_PRIVACY_MAX_BYTES_PER_BLOCK_V1: u32 = 16 * 1024 * 1024;
/// Maximum public-statement and encrypted-output payload bytes in one Taira transaction.
pub const TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1: u32 =
    256 * 1024;
/// Maximum nullifiers admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_NULLIFIERS_PER_ACTION_V1: u32 = 8;
/// Maximum commitments admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_COMMITMENTS_PER_ACTION_V1: u32 = 8;
/// Number of recent privacy roots retained by the Taira first-release profile.
pub const TAIRA_PRIVACY_RETAINED_ROOT_COUNT_V1: u32 = 2_048;

/// Canonical first-release privacy protocol identity.
///
/// Variant order is part of the Norito wire contract. New protocols require a
/// new data-model release; unknown discriminants are rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "protocol", content = "value"))]
pub enum PrivacyProtocolIdV1 {
    /// Native ZK-ACE post-quantum authorization protocol v0.
    ZkAcePqAuthorizationV0,
    /// Anonymous PGC k-out-of-n payment protocol v1.
    AnonymousPgcKOutOfNV1,
    /// VeRange transparent range-proof protocol v1.
    VeRangeTransparentRangeV1,
    /// Native Iroha ZK-AMS admission and anonymous-account provisioning suite v1.
    IrohaZkAmsV1,
    /// Vega proof over an existing credential v0.
    VegaExistingCredentialZkV0,
    /// Native Iroha P-256 X.509 predicate STARK protocol v0.
    IrohaZkX509StarkP256V0,
    /// Native Iroha Jindo multilinear lattice polynomial-commitment protocol v0.
    IrohaJindoPolynomialCommitmentV0,
    /// Native Iroha Bootle GenISIS anonymous-credential STARK profile v0.
    IrohaBootleGenisisAcStarkV0,
    /// Orchard Halo2 action protocol v1.
    OrchardHalo2ActionsV1,
    /// Monero FCMP++ full-chain membership protocol v1.
    MoneroFcmpPlusPlusV1,
    /// Native IVM private-note STARK protocol v1.
    IrohaIvmPrivateNoteStarkV1,
    /// Post-quantum MASP STARK protocol v0.
    PqMaspStarkV0,
}

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
        Self::IrohaBootleGenisisAcStarkV0,
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
            Self::IrohaJindoPolynomialCommitmentV0 => {
                "iroha-jindo-polynomial-commitment-v0"
            }
            Self::IrohaBootleGenisisAcStarkV0 => "iroha-bootle-genisis-ac-stark-v0",
            Self::OrchardHalo2ActionsV1 => "orchard-halo2-actions-v1",
            Self::MoneroFcmpPlusPlusV1 => "monero-fcmp-plus-plus-v1",
            Self::IrohaIvmPrivateNoteStarkV1 => "iroha-ivm-private-note-stark-v1",
            Self::PqMaspStarkV0 => "pq-masp-stark-v0",
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
            b"iroha-jindo-polynomial-commitment-v0" => {
                Some(Self::IrohaJindoPolynomialCommitmentV0)
            }
            b"iroha-bootle-genisis-ac-stark-v0" => {
                Some(Self::IrohaBootleGenisisAcStarkV0)
            }
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
            Self::ZkAcePqAuthorizationV0 | Self::PqMaspStarkV0 => {
                PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
            }
            Self::IrohaZkX509StarkP256V0
            | Self::IrohaBootleGenisisAcStarkV0
            | Self::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyProofSystemIdV1::StarkFriPoseidon2Goldilocks
            }
            Self::IrohaZkAmsV1 => {
                PrivacyProofSystemIdV1::ZkAmsTransparentStarkPoseidon2GoldilocksMlsagsRistretto255Sha3_512
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
            | Self::IrohaBootleGenisisAcStarkV0
            | Self::IrohaIvmPrivateNoteStarkV1
            | Self::PqMaspStarkV0 => PrivacyEngineIdV1::NativeGoldilocksStarkFri,
            Self::IrohaZkAmsV1 => PrivacyEngineIdV1::NativeZkAmsTransparentStarkMlsagsRistretto255,
            Self::AnonymousPgcKOutOfNV1 => PrivacyEngineIdV1::NativeAnonymousPgcP256,
            Self::VeRangeTransparentRangeV1 => PrivacyEngineIdV1::NativeVeRangeP256,
            Self::VegaExistingCredentialZkV0 => PrivacyEngineIdV1::NativeVega,
            Self::IrohaJindoPolynomialCommitmentV0 => PrivacyEngineIdV1::NativeJindo,
            Self::OrchardHalo2ActionsV1 => PrivacyEngineIdV1::NativeHalo2Orchard,
            Self::MoneroFcmpPlusPlusV1 => PrivacyEngineIdV1::NativeFcmpPlusPlus,
        }
    }
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
#[cfg_attr(feature = "json", norito(tag = "proof_system", content = "value"))]
pub enum PrivacyProofSystemIdV1 {
    /// STARK/FRI over Goldilocks with SHA-256 transcript and commitments.
    StarkFriSha256Goldilocks,
    /// STARK/FRI over Goldilocks with Poseidon2 transcript and commitments.
    StarkFriPoseidon2Goldilocks,
    /// ZK-AMS transparent STARK batch admission plus Ristretto255 MLSAGS provisioning.
    ///
    /// Batch admission uses Poseidon2/Goldilocks commitment digests and a
    /// transparent STARK/FRI proof. Account provisioning uses MLSAGS over
    /// Ristretto255 with SHA3-512 for the transcript and hash-to-group suite.
    ZkAmsTransparentStarkPoseidon2GoldilocksMlsagsRistretto255Sha3_512,
    /// Anonymous PGC k-out-of-n proof system over P-256.
    AnonymousPgcP256,
    /// Iroha Type-1 VeRange profile over P-256 with SHA-256.
    ///
    /// This profile is distinct from the upstream BN254-and-Keccak reference.
    IrohaVeRangeP256,
    /// Vega Neutron/Nova/Spartan proof system with Hyrax commitments over T256.
    VegaNeutronNovaSpartanHyraxT256,
    /// Jindo multilinear lattice polynomial-commitment proof system.
    JindoPolynomialCommitment,
    /// Halo2 IPA proof system over the Pasta curve cycle.
    Halo2IpaPasta,
    /// FCMP++ Curve Tree and Bulletproofs proof composition.
    FcmpPlusPlusCurveTreeBulletproofs,
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
#[cfg_attr(feature = "json", norito(tag = "engine", content = "value"))]
pub enum PrivacyEngineIdV1 {
    /// Native Goldilocks STARK/FRI verifier.
    NativeGoldilocksStarkFri,
    /// Native ZK-AMS transparent-STARK and Ristretto255-MLSAGS verifier suite.
    NativeZkAmsTransparentStarkMlsagsRistretto255,
    /// Native Anonymous PGC verifier over P-256.
    NativeAnonymousPgcP256,
    /// Native VeRange verifier over P-256.
    NativeVeRangeP256,
    /// Native Vega verifier.
    NativeVega,
    /// Native Jindo verifier.
    NativeJindo,
    /// Native Orchard Halo2 verifier.
    NativeHalo2Orchard,
    /// Native FCMP++ verifier.
    NativeFcmpPlusPlus,
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
    /// Digest of a canonical governance root publication.
    PrivacyRootPublicationDigestV1
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
    /// Digest of a certificate subject public key.
    PrivacyCertificateKeyDigestV1
);
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
    /// Poseidon2/Goldilocks digest of source object `I_acc,N`.
    PrivacyZkAmsAccumulatedInstanceDigestV1
);
define_privacy_digest!(
    /// Poseidon2/Goldilocks digest of source object `Tbar_(N+1)`.
    PrivacyZkAmsPaddingCrossTermDigestV1
);
define_privacy_digest!(
    /// Poseidon2/Goldilocks digest of source object `I_acc,N+1`.
    PrivacyZkAmsFinalFoldedInstanceDigestV1
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
pub struct PrivacyIssuerRegistryPolicyNamespaceV1 {
    /// Exact credential issuer.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Exact admitted-identity registry.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Exact admission policy.
    pub policy_id: PrivacyPolicyIdV1,
}

/// Trust-anchor and certificate-policy namespace payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(feature = "json", norito(tag = "scope", content = "value"))]
pub enum PrivacyNamespaceScopeV1 {
    /// Governed authorization or range policy.
    Policy(PrivacyPolicyNamespaceV1),
    /// Anonymous-account or private-note pool.
    Pool(PrivacyPoolNamespaceV1),
    /// Credential issuer, admitted-identity registry, and admission policy.
    IssuerRegistryPolicy(PrivacyIssuerRegistryPolicyNamespaceV1),
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
            PrivacyStatementV1::IrohaBootleGenisisAcStarkV0(statement) => Self::new(
                PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0,
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
                PrivacyNamespaceScopeV1::TrustAnchorPolicy(_)
            ) | (
                PrivacyProtocolIdV1::VegaExistingCredentialZkV0
                    | PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0,
                PrivacyNamespaceScopeV1::Parameter(_)
            ) | (
                PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0,
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
#[cfg_attr(feature = "json", norito(tag = "management", content = "value"))]
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
#[cfg_attr(feature = "json", norito(tag = "role", content = "value"))]
pub enum PrivacyRootRoleV1 {
    /// Mutable encrypted PGC account table.
    PgcAccountState,
    /// ZK-AMS admitted identities, seed keys, and provisioning records.
    AccountRegistry,
    /// Credential revocation accumulator.
    Revocation,
    /// X.509 CA-membership accumulator.
    CertificateAuthorityMembership,
    /// X.509 CRL non-membership tree.
    CertificateRevocationNonmembership,
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
            Self::Revocation
            | Self::CertificateAuthorityMembership
            | Self::CertificateRevocationNonmembership => {
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
                    PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0,
                    Self::Revocation
                )
                | (
                    PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
                    Self::CertificateAuthorityMembership | Self::CertificateRevocationNonmembership
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
}

/// Governance payload publishing one canonical privacy root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Root epoch is zero.
    #[error("privacy root publication epoch must be non-zero")]
    ZeroEpoch,
    /// Root is zero.
    #[error("privacy root publication root must be non-zero")]
    ZeroRoot,
}

/// One canonical encrypted account in a PGC account-state bootstrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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

/// Proposed lifecycle state fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(feature = "json", norito(tag = "state", content = "record"))]
pub enum PrivacyProtocolLifecycleV1 {
    /// Governance approved a future activation height.
    Proposed(PrivacyProposedLifecycleV1),
    /// The protocol is currently active.
    Active(PrivacyActiveLifecycleV1),
    /// The protocol is temporarily fail-closed.
    Suspended(PrivacySuspendedLifecycleV1),
    /// The protocol is permanently unavailable.
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
#[cfg_attr(feature = "json", norito(tag = "assurance", content = "value"))]
pub enum PrivacyAssuranceV1 {
    /// Testnet-only experimental activation pending production review gates.
    Experimental,
}

/// Activation-specific Anonymous PGC policy limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
pub struct ZkAmsActivationLimitsV1 {
    /// Maximum ordered admission anchors in one batch settlement.
    pub max_batch_size: u32,
    /// Maximum admitted seed-key ring size in one provisioning action.
    pub max_ring_size: u32,
}

/// Activation-specific Jindo multilinear-opening policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct JindoActivationLimitsV1 {
    /// Maximum polynomial commitments per statement.
    pub max_polynomial_count: u32,
    /// Maximum multilinear evaluation queries per statement.
    pub max_evaluation_query_count: u32,
    /// Maximum variables in each multilinear evaluation point.
    pub max_multilinear_variable_count: u32,
}

/// Activation-specific Orchard action policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(feature = "json", norito(tag = "protocol", content = "limits"))]
pub enum PrivacyProtocolActivationLimitsV1 {
    /// ZK-ACE has no additional first-release count limits.
    ZkAcePqAuthorizationV0,
    /// Anonymous PGC receiver policy.
    AnonymousPgcKOutOfNV1(AnonymousPgcActivationLimitsV1),
    /// VeRange aggregation policy.
    VeRangeTransparentRangeV1(VeRangeActivationLimitsV1),
    /// ZK-AMS batch-admission and account-provisioning policy.
    IrohaZkAmsV1(ZkAmsActivationLimitsV1),
    /// Vega has no additional first-release count limits.
    VegaExistingCredentialZkV0,
    /// X.509 has fixed first-release limits encoded by its statement validator.
    IrohaZkX509StarkP256V0,
    /// Jindo batched opening policy.
    IrohaJindoPolynomialCommitmentV0(JindoActivationLimitsV1),
    /// SIS-with-hints has a fixed first-release attribute profile.
    IrohaBootleGenisisAcStarkV0,
    /// Orchard one-to-one action policy.
    OrchardHalo2ActionsV1(OrchardActivationLimitsV1),
    /// FCMP++ input/output policy.
    MoneroFcmpPlusPlusV1(FcmpActivationLimitsV1),
    /// Native private-note input/output policy.
    IrohaIvmPrivateNoteStarkV1(IvmPrivateNoteActivationLimitsV1),
    /// PQ-MASP input/output policy.
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
            Self::IrohaBootleGenisisAcStarkV0 => PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0,
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
            Self::IrohaJindoPolynomialCommitmentV0(limits) => {
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::JindoPolynomialCount,
                    limits.max_polynomial_count,
                    IROHA_JINDO_MAX_POLYNOMIALS_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::JindoEvaluationQueryCount,
                    limits.max_evaluation_query_count,
                    IROHA_JINDO_MAX_EVALUATION_QUERIES_V1,
                )?;
                validate_profile_limit(
                    PrivacyActivationLimitFieldV1::JindoMultilinearVariableCount,
                    limits.max_multilinear_variable_count,
                    IROHA_JINDO_MAX_MULTILINEAR_VARIABLES_V1,
                )
            }
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
    /// Jindo multilinear evaluation-query count.
    JindoEvaluationQueryCount,
    /// Jindo multilinear variable count.
    JindoMultilinearVariableCount,
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

/// Governed activation record for one exact privacy protocol implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Consensus resource limits for this activation.
    pub limits: PrivacyConsensusLimitsV1,
    /// Protocol-specific governed count limits.
    pub protocol_limits: PrivacyProtocolActivationLimitsV1,
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
        self.lifecycle
            .validate()
            .map_err(PrivacyActivationValidationError::Lifecycle)?;
        self.limits
            .validate()
            .map_err(PrivacyActivationValidationError::Limits)
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
    /// Lifecycle is invalid.
    #[error("privacy activation lifecycle is invalid: {0}")]
    Lifecycle(PrivacyLifecycleValidationError),
    /// Consensus limits are invalid.
    #[error("privacy activation limits are invalid: {0}")]
    Limits(PrivacyConsensusLimitsValidationError),
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
/// Maximum polynomials in one Jindo multilinear-opening statement.
pub const IROHA_JINDO_MAX_POLYNOMIALS_V1: u32 = 4;
/// Maximum multilinear evaluation queries in one Jindo statement.
pub const IROHA_JINDO_MAX_EVALUATION_QUERIES_V1: u32 = 8;
/// Maximum variables in one Jindo multilinear polynomial profile.
pub const IROHA_JINDO_MAX_MULTILINEAR_VARIABLES_V1: u32 = 32;
/// Maximum canonical byte width of one governed Jindo field element.
pub const IROHA_JINDO_MAX_FIELD_ELEMENT_BYTES_V1: u16 = 64;
/// Maximum canonical byte width of one governed Jindo lattice commitment.
pub const IROHA_JINDO_MAX_LATTICE_COMMITMENT_BYTES_V1: u32 = 64 * 1024;
/// Exact credential attribute count bound by the SIS-with-hints profile.
pub const SIS_WITH_HINTS_ATTRIBUTE_COUNT_V1: u32 = 8;
/// Maximum encoded bytes in one SIS-with-hints attribute.
pub const SIS_WITH_HINTS_MAX_ATTRIBUTE_BYTES_V1: u32 = 1_024;
/// Maximum selectively disclosed attributes in one SIS-with-hints statement.
pub const SIS_WITH_HINTS_MAX_DISCLOSED_ATTRIBUTES_V1: u32 = 8;
/// Maximum decoded ISO 18013-5 MSO payload bytes admitted by Vega.
///
/// This is the 1,920-byte mDL profile evaluated in Figure 9 of the Vega paper.
/// It is deliberately distinct from the COSE `Sig_structure` bytes hashed for
/// issuer authentication.
pub const VEGA_MDL_MAX_MSO_PAYLOAD_BYTES_V1: u32 = 1_920;
/// Fixed SHA-256 compression-table width for the issuer-authenticated bytes.
pub const VEGA_MDL_ISSUER_AUTH_SHA256_STEP_COUNT_V1: u8 = 32;
/// Maximum canonical COSE `Sig_structure` bytes that fit the fixed table.
///
/// A 32-block SHA-256 table holds 2,048 bytes. Canonical SHA-256 padding needs
/// at least nine bytes, so 2,039 is the exact maximum unpadded message length.
pub const VEGA_MDL_MAX_ISSUER_AUTH_BYTES_V1: u32 = 2_039;
/// Maximum canonical tagged `IssuerSignedItemBytes` for `birth_date`.
pub const VEGA_MDL_MAX_BIRTH_DATE_ITEM_BYTES_V1: u32 = 256;
/// Fixed SHA-256 compression-table width for the birth-date signed item.
pub const VEGA_MDL_BIRTH_DATE_SHA256_STEP_COUNT_V1: u8 = 8;
/// Lowest trusted UTC presentation year admitted by the first release.
pub const VEGA_MDL_MIN_PRESENTATION_YEAR_V1: u16 = 1_970;
/// Highest trusted UTC presentation year admitted by the first release.
pub const VEGA_MDL_MAX_PRESENTATION_YEAR_V1: u16 = 9_999;
/// Lowest non-degenerate public age threshold admitted by the first release.
pub const VEGA_MDL_MIN_AGE_THRESHOLD_YEARS_V1: u8 = 1;
/// Highest public age threshold admitted by the first release.
pub const VEGA_MDL_MAX_AGE_THRESHOLD_YEARS_V1: u8 = 150;
/// Maximum admitted X.509 chain depth, including the leaf certificate.
pub const ZK_X509_MAX_CHAIN_DEPTH_V1: u8 = 3;
/// Maximum DER bytes for one X.509 certificate.
pub const ZK_X509_MAX_CERTIFICATE_BYTES_V1: u32 = 16 * 1024;
/// Maximum combined DER bytes for an admitted X.509 chain.
pub const ZK_X509_MAX_CHAIN_BYTES_V1: u32 = ZK_X509_MAX_CERTIFICATE_BYTES_V1 * 3;
/// Maximum Orchard spends and outputs in one first-release action.
pub const ORCHARD_MAX_ACTIONS_V1: u32 = 2;
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
/// Maximum UTF-8 byte length admitted for a privacy transcript chain id.
pub const PRIVACY_MAX_CHAIN_ID_BYTES_V1: u32 = 255;

/// Explicit chain and governed-artifact binding shared by every statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PrivacyStatementContextV1 {
    /// Exact chain identifier.
    pub chain_id: ChainId,
    /// Zero-based privacy action index within the transaction.
    pub action_index: u32,
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
pub struct PrivacyEncryptedOutputV1 {
    /// Cryptographic recipient identity.
    pub recipient: PrivacyRecipientIdV1,
    /// Ephemeral public encryption key.
    pub ephemeral_public_key: PrivacyEncryptionKeyV1,
    /// Commitment to the plaintext output.
    pub commitment: PrivacyCommitmentV1,
    /// Canonical authenticated ciphertext bytes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub ciphertext: Vec<u8>,
}

/// ZK-ACE authorization statement for a public asset transfer.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Atomic validation fee.
    pub fee: u128,
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
#[cfg_attr(feature = "json", norito(tag = "bits", content = "value"))]
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
pub struct PrivacyZkAmsAdmissionAnchorV1 {
    /// Hash of the canonical Personhood Credential.
    pub phc_hash: PrivacyZkAmsPhcHashV1,
    /// Seed public key later used for anonymous account provisioning.
    pub seed_public_key: PrivacyZkAmsSeedPublicKeyV1,
}

/// Transparent Iroha instantiation of ZK-AMS batch settlement.
///
/// The source relation exposes the complete committed relaxed-R1CS objects
/// `I_acc,N`, `Tbar_(N+1)`, and `I_acc,N+1`. The Iroha transparent-STARK
/// profile carries distinct Poseidon2/Goldilocks digests here; the STARK
/// witness contains the full canonical objects and proves each digest preimage,
/// the folding and public-padding equations, and the admission relation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Digest of materialized accumulated instance `I_acc,N`.
    pub accumulated_instance_digest: PrivacyZkAmsAccumulatedInstanceDigestV1,
    /// Digest of final public padding cross-term `Tbar_(N+1)`.
    pub padding_cross_term_digest: PrivacyZkAmsPaddingCrossTermDigestV1,
    /// Digest of final folded instance `I_acc,N+1`.
    pub final_folded_instance_digest: PrivacyZkAmsFinalFoldedInstanceDigestV1,
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
#[cfg_attr(feature = "json", norito(tag = "action", content = "value"))]
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
pub struct IrohaZkAmsStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Credential issuer governing the common admission relation.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Admitted-identity and provisioning registry.
    pub registry_id: PrivacyZkAmsRegistryIdV1,
    /// Admission policy identifier.
    pub policy_id: PrivacyPolicyIdV1,
    /// Exact batch-settlement or account-provisioning action.
    pub action: PrivacyZkAmsActionV1,
}

/// Credential document family admitted by the Vega first-release profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "document", content = "value"))]
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
#[cfg_attr(feature = "json", norito(tag = "namespace", content = "value"))]
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
#[cfg_attr(feature = "json", norito(tag = "digest", content = "value"))]
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
#[cfg_attr(feature = "json", norito(tag = "signature", content = "value"))]
pub enum PrivacyVegaMdlSignatureAlgorithmV1 {
    /// COSE algorithm `-7`: ECDSA over P-256 with SHA-256 (`ES256`).
    CoseSign1Es256,
}

/// Gregorian UTC calendar date used as Vega Figure 9 public input `(Y, M, D)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
/// `Q_I` being a public proof input establishes only that the hidden
/// credential was authenticated by that key. Issuer accreditation is a
/// downstream policy decision; this reusable proof component has no ledger
/// effect.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VegaExistingCredentialStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
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
    /// Public P-256 issuer key `Q_I`.
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
#[cfg_attr(feature = "json", norito(tag = "purpose", content = "value"))]
pub enum PrivacyX509ExtendedKeyUsageV1 {
    /// TLS-style client authentication.
    ClientAuthentication,
    /// Digital document signing.
    DocumentSigning,
    /// Wallet or digital-identity authentication.
    WalletIdentity,
}

/// Native X.509 credential-predicate STARK statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IrohaZkX509StarkP256StatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Trust-anchor issuer identifier.
    pub trust_anchor_id: PrivacyIssuerIdV1,
    /// Governed certificate-policy identifier.
    pub certificate_policy_id: PrivacyPolicyIdV1,
    /// Digest of the certificate subject public key.
    pub subject_public_key_digest: PrivacyCertificateKeyDigestV1,
    /// CA membership accumulator root for the complete chain.
    pub ca_membership_root: PrivacyRootV1,
    /// Epoch at which the CA membership root was canonical.
    pub ca_membership_root_epoch: u64,
    /// CRL sparse-tree root against which non-membership is proven.
    pub crl_nonmembership_root: PrivacyRootV1,
    /// Epoch at which the CRL sparse-tree root was canonical.
    pub crl_nonmembership_root_epoch: u64,
    /// Required RFC 5280 key usages.
    pub key_usage: PrivacyX509KeyUsageV1,
    /// Required extended-key-usage purposes, sorted in enum order.
    pub extended_key_usages: Vec<PrivacyX509ExtendedKeyUsageV1>,
    /// Certificate validity start as Unix seconds, inclusive.
    pub not_before_unix_seconds: u64,
    /// Certificate validity end as Unix seconds, exclusive.
    pub not_after_unix_seconds: u64,
    /// Validation time as Unix seconds.
    pub validation_unix_seconds: u64,
    /// Number of certificates in the validated chain, including the leaf.
    pub chain_depth: u8,
    /// DER byte length of the leaf certificate.
    pub leaf_certificate_bytes: u32,
    /// Combined DER byte length of the complete chain.
    pub chain_certificate_bytes: u32,
    /// Public wallet account to which the certificate showing is bound.
    pub wallet_account: AccountId,
    /// Wallet challenge preventing cross-account or cross-session replay.
    pub wallet_challenge: PrivacyChallengeV1,
    /// Nullifier derived from the certificate serial and policy.
    pub certificate_nullifier: PrivacyNullifierV1,
}

/// Canonical field-element encoding selected by a governed Jindo regime.
///
/// Jindo supports diverse fields, so the data model intentionally does not
/// claim one universal scalar modulus or byte order. The statement fixes an
/// exact byte width, and the governed parameter set plus native engine enforce
/// canonicality against the selected field.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct PrivacyJindoFieldElementV1 {
    /// Exact governed field-element encoding.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub encoding: Vec<u8>,
}

impl PrivacyJindoFieldElementV1 {
    /// Construct a governed field-element encoding.
    #[must_use]
    pub fn new(encoding: Vec<u8>) -> Self {
        Self { encoding }
    }

    /// Borrow the exact field-element bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.encoding
    }
}

/// Canonical public lattice-commitment encoding selected by a Jindo regime.
///
/// This bounded opaque encoding is deliberate: the revised paper permits
/// flexible lattice parameter regimes whose exact algebraic wire is governed
/// by the pinned parameter and engine manifests.
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
    /// Construct a governed lattice-commitment encoding.
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

/// Public shape of the governed Jindo parameter regime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PrivacyJindoParameterRegimeV1 {
    /// Variables in every committed multilinear polynomial and query point.
    pub multilinear_variable_count: u32,
    /// Exact canonical byte width of one element of the selected field.
    pub field_element_bytes: u16,
    /// Exact canonical byte width of one public lattice commitment.
    pub lattice_commitment_bytes: u32,
    /// Whether the governed Jindo evaluation-hiding profile is enabled.
    pub evaluation_hiding: bool,
}

/// Claimed evaluations at one multilinear Jindo point.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PrivacyJindoEvaluationQueryV1 {
    /// Multilinear point `(r_0, ..., r_{m-1})`.
    pub evaluation_point: Vec<PrivacyJindoFieldElementV1>,
    /// Claimed evaluations, in polynomial-commitment order.
    pub claimed_evaluations: Vec<PrivacyJindoFieldElementV1>,
}

/// Native Jindo multilinear lattice polynomial-opening statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IrohaJindoPolynomialCommitmentStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Explicit public shape of the governed flexible parameter regime.
    pub regime: PrivacyJindoParameterRegimeV1,
    /// Public commitments to multilinear polynomials.
    pub polynomial_commitments: Vec<PrivacyJindoLatticeCommitmentV1>,
    /// Distinct multilinear points and claimed evaluation rows.
    pub evaluation_queries: Vec<PrivacyJindoEvaluationQueryV1>,
}

/// Public representation of one selectively disclosed SIS-with-hints attribute.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "representation", content = "value"))]
pub enum PrivacyDisclosedAttributeValueV1 {
    /// Canonical plaintext attribute encoding.
    Plaintext(Vec<u8>),
    /// Digest-only disclosure under the governed attribute schema.
    Digest(PrivacyAttributeDigestV1),
}

/// One sorted SIS-with-hints selective-disclosure entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PrivacyDisclosedAttributeV1 {
    /// Zero-based index in the fixed eight-attribute credential.
    pub index: u16,
    /// Public plaintext or governed-schema digest.
    pub value: PrivacyDisclosedAttributeValueV1,
}

/// Native Bootle GenISIS SIS-with-hints credential statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IrohaBootleGenisisAcStarkStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Anonymous-credential issuer identifier.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Selective-disclosure policy identifier.
    pub policy_id: PrivacyPolicyIdV1,
    /// Exact issuer parameter-set identifier.
    pub issuer_parameter_id: PrivacyParameterIdV1,
    /// Digest of the issuer parameter set.
    pub issuer_parameter_digest: PrivacyParameterDigestV1,
    /// Commitment to the credential.
    pub credential_commitment: PrivacyCommitmentV1,
    /// Commitment to the SIS hint transcript.
    pub hints_commitment: PrivacyCommitmentV1,
    /// Canonical revocation accumulator root.
    pub revocation_root: PrivacyRootV1,
    /// Epoch at which the revocation root was canonical.
    pub revocation_epoch: u64,
    /// Presentation nullifier.
    pub presentation_nullifier: PrivacyNullifierV1,
    /// Total attributes committed by the credential.
    pub attribute_count: u32,
    /// Strictly increasing selectively disclosed attributes.
    pub disclosures: Vec<PrivacyDisclosedAttributeV1>,
}

/// Direction of a public value balance relative to a private pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "direction", content = "value"))]
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

/// Orchard Halo2 private action statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Resulting note-commitment tree anchor.
    pub next_anchor: PrivacyRootV1,
    /// Successor epoch committed with `next_anchor`.
    pub next_anchor_epoch: u64,
    /// Spent-note nullifiers.
    pub spend_nullifiers: Vec<PrivacyNullifierV1>,
    /// New note commitments.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Encrypted output notes, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Public value balance in atomic units.
    pub value_balance: PrivacyValueBalanceV1,
    /// Public validation fee in atomic units.
    pub fee: u128,
    /// Last block height at which the action is valid.
    pub expiry_height: u64,
}

/// Monero FCMP++ full-chain-membership transfer statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MoneroFcmpPlusPlusStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset transferred by the private action.
    pub asset_definition_id: AssetDefinitionId,
    /// FCMP++ output-set namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Canonical full-output-set root.
    pub output_set_root: PrivacyRootV1,
    /// Epoch at which the output-set root was canonical.
    pub root_epoch: u64,
    /// Resulting complete output-set root.
    pub next_output_set_root: PrivacyRootV1,
    /// Successor epoch committed with `next_output_set_root`.
    pub next_output_set_root_epoch: u64,
    /// Commitments to the consumed outputs.
    pub input_commitments: Vec<PrivacyCommitmentV1>,
    /// Link tags corresponding one-to-one with consumed outputs.
    pub link_tags: Vec<PrivacyNullifierV1>,
    /// Commitments to new outputs.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Encrypted new outputs, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Public validation fee in atomic units.
    pub fee: u128,
}

/// Native IVM private-note execution statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IrohaIvmPrivateNoteStarkStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset manipulated by the private program.
    pub asset_definition_id: AssetDefinitionId,
    /// Private-note pool namespace.
    pub pool_id: PrivacyPoolIdV1,
    /// Exact private IVM program identifier.
    pub program_id: PrivacyProgramIdV1,
    /// Canonical private-note state root.
    pub state_root: PrivacyRootV1,
    /// Epoch at which `state_root` was canonical.
    pub root_epoch: u64,
    /// Resulting private-note program-state root.
    pub next_state_root: PrivacyRootV1,
    /// Successor epoch committed with `next_state_root`.
    pub next_state_root_epoch: u64,
    /// Consumed note nullifiers.
    pub nullifiers: Vec<PrivacyNullifierV1>,
    /// New note commitments.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Encrypted new notes, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Public value balance in atomic units.
    pub value_balance: PrivacyValueBalanceV1,
    /// Public validation fee in atomic units.
    pub fee: u128,
    /// Ledger epoch bound into private program execution.
    pub execution_epoch: u64,
}

/// Post-quantum authorization profile required by PQ-MASP v0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "authorization", content = "value"))]
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
#[cfg_attr(feature = "json", norito(tag = "encryption", content = "value"))]
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
    /// Resulting note-commitment tree anchor.
    pub next_anchor: PrivacyRootV1,
    /// Successor epoch committed with `next_anchor`.
    pub next_anchor_epoch: u64,
    /// Consumed-note nullifiers.
    pub nullifiers: Vec<PrivacyNullifierV1>,
    /// New note commitments.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// ML-KEM-derived encrypted output notes, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Public validation fee in atomic units.
    pub fee: u128,
    /// Required transaction-authorization profile.
    pub authorization_profile: PrivacyPqAuthorizationProfileV1,
    /// Digest of the authorized ML-DSA key.
    pub authorization_key_digest: PrivacyAuthorizationKeyDigestV1,
    /// Required note-encryption profile.
    pub note_encryption_profile: PrivacyPqNoteEncryptionProfileV1,
    /// Digest of the wallet note-encryption key.
    pub note_encryption_key_digest: PrivacyNoteEncryptionKeyDigestV1,
    /// Ledger epoch bound into authorization.
    pub authorization_epoch: u64,
}

/// Protocol-typed canonical privacy statement.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "protocol", content = "statement"))]
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
    /// Native Iroha Jindo multilinear lattice polynomial-commitment statement.
    IrohaJindoPolynomialCommitmentV0(IrohaJindoPolynomialCommitmentStatementV1),
    /// Native Iroha Bootle GenISIS anonymous-credential statement.
    IrohaBootleGenisisAcStarkV0(IrohaBootleGenisisAcStarkStatementV1),
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
            Self::IrohaBootleGenisisAcStarkV0(_) => {
                PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0
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
            Self::IrohaBootleGenisisAcStarkV0(statement) => &statement.context,
            Self::OrchardHalo2ActionsV1(statement) => &statement.context,
            Self::MoneroFcmpPlusPlusV1(statement) => &statement.context,
            Self::IrohaIvmPrivateNoteStarkV1(statement) => &statement.context,
            Self::PqMaspStarkV0(statement) => &statement.context,
        }
    }

    /// Hash this complete protocol-tagged statement using canonical Norito bytes.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if canonical statement encoding fails.
    pub fn digest(&self) -> Result<PrivacyStatementDigestV1, norito::Error> {
        let encoded = norito::to_bytes(self)?;
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
            Self::IrohaBootleGenisisAcStarkV0(statement) => validate_sis_hints(statement)?,
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
    require_nonzero_id(
        statement.registry_id.is_zero(),
        PrivacyTypedFieldV1::RegistryId,
    )?;
    require_nonzero_id(statement.policy_id.is_zero(), PrivacyTypedFieldV1::PolicyId)?;
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
            require_nonzero_id(
                batch.accumulated_instance_digest.is_zero(),
                PrivacyTypedFieldV1::ZkAmsAccumulatedInstanceDigest,
            )?;
            require_nonzero_id(
                batch.padding_cross_term_digest.is_zero(),
                PrivacyTypedFieldV1::ZkAmsPaddingCrossTermDigest,
            )?;
            require_nonzero_id(
                batch.final_folded_instance_digest.is_zero(),
                PrivacyTypedFieldV1::ZkAmsFinalFoldedInstanceDigest,
            )
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
    require_nonzero_id(
        statement.crl_nonmembership_root.is_zero(),
        PrivacyTypedFieldV1::RevocationRoot,
    )?;
    require_epoch(
        statement.crl_nonmembership_root_epoch,
        PrivacyEpochFieldV1::Revocation,
    )?;
    if !statement.key_usage.digital_signature {
        return Err(PrivacyStatementValidationError::InvalidX509KeyUsage);
    }
    if statement.extended_key_usages.is_empty() {
        return Err(PrivacyStatementValidationError::MissingX509ExtendedKeyUsage);
    }
    for index in 1..statement.extended_key_usages.len() {
        if statement.extended_key_usages[index - 1] >= statement.extended_key_usages[index] {
            return Err(
                PrivacyStatementValidationError::X509ExtendedKeyUsagesNotStrictlyIncreasing,
            );
        }
    }
    validate_validity_epochs(
        statement.not_before_unix_seconds,
        statement.not_after_unix_seconds,
        statement.validation_unix_seconds,
    )?;
    if statement.chain_depth == 0 || statement.chain_depth > ZK_X509_MAX_CHAIN_DEPTH_V1 {
        return Err(PrivacyStatementValidationError::InvalidX509ChainDepth {
            depth: statement.chain_depth,
            max: ZK_X509_MAX_CHAIN_DEPTH_V1,
        });
    }
    if statement.leaf_certificate_bytes == 0
        || statement.leaf_certificate_bytes > ZK_X509_MAX_CERTIFICATE_BYTES_V1
    {
        return Err(
            PrivacyStatementValidationError::InvalidX509LeafCertificateSize {
                bytes: statement.leaf_certificate_bytes,
                max: ZK_X509_MAX_CERTIFICATE_BYTES_V1,
            },
        );
    }
    if statement.chain_certificate_bytes < statement.leaf_certificate_bytes
        || statement.chain_certificate_bytes > ZK_X509_MAX_CHAIN_BYTES_V1
    {
        return Err(PrivacyStatementValidationError::InvalidX509ChainSize {
            bytes: statement.chain_certificate_bytes,
            leaf_bytes: statement.leaf_certificate_bytes,
            max: ZK_X509_MAX_CHAIN_BYTES_V1,
        });
    }
    require_nonzero_id(
        statement.wallet_challenge.is_zero(),
        PrivacyTypedFieldV1::ReaderChallenge,
    )?;
    require_nullifier(statement.certificate_nullifier, 0)
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
    let evaluation_query_count = u32_len(statement.evaluation_queries.len())?;
    if evaluation_query_count == 0 || evaluation_query_count > IROHA_JINDO_MAX_EVALUATION_QUERIES_V1
    {
        return Err(
            PrivacyStatementValidationError::InvalidJindoEvaluationQueryCount {
                count: evaluation_query_count,
                max: IROHA_JINDO_MAX_EVALUATION_QUERIES_V1,
            },
        );
    }
    if statement.regime.multilinear_variable_count == 0
        || statement.regime.multilinear_variable_count > IROHA_JINDO_MAX_MULTILINEAR_VARIABLES_V1
    {
        return Err(
            PrivacyStatementValidationError::InvalidJindoMultilinearVariableCount {
                count: statement.regime.multilinear_variable_count,
                max: IROHA_JINDO_MAX_MULTILINEAR_VARIABLES_V1,
            },
        );
    }
    if statement.regime.field_element_bytes == 0
        || statement.regime.field_element_bytes > IROHA_JINDO_MAX_FIELD_ELEMENT_BYTES_V1
    {
        return Err(
            PrivacyStatementValidationError::InvalidJindoFieldElementSize {
                bytes: statement.regime.field_element_bytes,
                max: IROHA_JINDO_MAX_FIELD_ELEMENT_BYTES_V1,
            },
        );
    }
    if statement.regime.lattice_commitment_bytes == 0
        || statement.regime.lattice_commitment_bytes > IROHA_JINDO_MAX_LATTICE_COMMITMENT_BYTES_V1
    {
        return Err(
            PrivacyStatementValidationError::InvalidJindoLatticeCommitmentSize {
                index: None,
                bytes: statement.regime.lattice_commitment_bytes,
                expected: statement.regime.lattice_commitment_bytes,
                max: IROHA_JINDO_MAX_LATTICE_COMMITMENT_BYTES_V1,
            },
        );
    }
    for (index, commitment) in statement.polynomial_commitments.iter().enumerate() {
        let bytes = u32_len(commitment.encoding.len())?;
        if bytes != statement.regime.lattice_commitment_bytes {
            return Err(
                PrivacyStatementValidationError::InvalidJindoLatticeCommitmentSize {
                    index: Some(u32_index(index)?),
                    bytes,
                    expected: statement.regime.lattice_commitment_bytes,
                    max: IROHA_JINDO_MAX_LATTICE_COMMITMENT_BYTES_V1,
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
    }
    if first_duplicate_index(&statement.polynomial_commitments).is_some() {
        return Err(PrivacyStatementValidationError::DuplicateJindoLatticeCommitment);
    }
    let expected_field_bytes = usize::from(statement.regime.field_element_bytes);
    for query in &statement.evaluation_queries {
        require_count(
            query.evaluation_point.len(),
            statement.regime.multilinear_variable_count,
            PrivacyCountFieldV1::JindoEvaluationPointCoordinates,
        )?;
        require_count(
            query.claimed_evaluations.len(),
            polynomial_count,
            PrivacyCountFieldV1::JindoClaimedEvaluations,
        )?;
        for element in query
            .evaluation_point
            .iter()
            .chain(query.claimed_evaluations.iter())
        {
            if element.encoding.len() != expected_field_bytes {
                return Err(
                    PrivacyStatementValidationError::InvalidJindoFieldElementEncoding {
                        bytes: u32_len(element.encoding.len())?,
                        expected: u32::from(statement.regime.field_element_bytes),
                    },
                );
            }
        }
    }
    for later in 1..statement.evaluation_queries.len() {
        if statement.evaluation_queries[..later].iter().any(|earlier| {
            earlier.evaluation_point == statement.evaluation_queries[later].evaluation_point
        }) {
            return Err(PrivacyStatementValidationError::DuplicateJindoEvaluationPoint);
        }
    }
    Ok(())
}

fn validate_sis_hints(
    statement: &IrohaBootleGenisisAcStarkStatementV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.issuer_id.is_zero(), PrivacyTypedFieldV1::IssuerId)?;
    require_nonzero_id(statement.policy_id.is_zero(), PrivacyTypedFieldV1::PolicyId)?;
    require_nonzero_id(
        statement.issuer_parameter_id.is_zero(),
        PrivacyTypedFieldV1::IssuerParameterId,
    )?;
    require_nonzero_id(
        statement.issuer_parameter_digest.is_zero(),
        PrivacyTypedFieldV1::IssuerParameterDigest,
    )?;
    require_commitment(statement.credential_commitment, 0)?;
    require_commitment(statement.hints_commitment, 1)?;
    require_nonzero_id(
        statement.revocation_root.is_zero(),
        PrivacyTypedFieldV1::RevocationRoot,
    )?;
    require_epoch(statement.revocation_epoch, PrivacyEpochFieldV1::Revocation)?;
    require_nullifier(statement.presentation_nullifier, 0)?;
    if statement.attribute_count != SIS_WITH_HINTS_ATTRIBUTE_COUNT_V1 {
        return Err(PrivacyStatementValidationError::InvalidAttributeCount {
            count: statement.attribute_count,
            expected: SIS_WITH_HINTS_ATTRIBUTE_COUNT_V1,
        });
    }
    let disclosed_count = u32::try_from(statement.disclosures.len())
        .map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)?;
    if disclosed_count > SIS_WITH_HINTS_MAX_DISCLOSED_ATTRIBUTES_V1 {
        return Err(
            PrivacyStatementValidationError::TooManyDisclosedAttributes {
                count: disclosed_count,
                max: SIS_WITH_HINTS_MAX_DISCLOSED_ATTRIBUTES_V1,
            },
        );
    }
    let mut previous = None;
    for disclosure in &statement.disclosures {
        if u32::from(disclosure.index) >= statement.attribute_count {
            return Err(
                PrivacyStatementValidationError::DisclosedAttributeOutOfBounds {
                    index: disclosure.index,
                    attribute_count: statement.attribute_count,
                },
            );
        }
        if previous.is_some_and(|value| disclosure.index <= value) {
            return Err(PrivacyStatementValidationError::DisclosedAttributesNotStrictlyIncreasing);
        }
        match &disclosure.value {
            PrivacyDisclosedAttributeValueV1::Plaintext(value) => {
                let bytes = u32_len(value.len())?;
                if bytes == 0
                    || bytes > SIS_WITH_HINTS_MAX_ATTRIBUTE_BYTES_V1
                    || value.iter().all(|byte| *byte == 0)
                {
                    return Err(
                        PrivacyStatementValidationError::InvalidDisclosedAttributeValue {
                            index: disclosure.index,
                            bytes,
                            max: SIS_WITH_HINTS_MAX_ATTRIBUTE_BYTES_V1,
                        },
                    );
                }
            }
            PrivacyDisclosedAttributeValueV1::Digest(digest) => {
                if digest.is_zero() {
                    return Err(
                        PrivacyStatementValidationError::ZeroDisclosedAttributeDigest {
                            index: disclosure.index,
                        },
                    );
                }
            }
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
    validate_next_root_transition(
        statement.anchor,
        statement.anchor_epoch,
        statement.next_anchor,
        statement.next_anchor_epoch,
        PrivacyRootTransitionFieldV1::NoteCommitmentAnchor,
    )?;
    require_epoch(statement.expiry_height, PrivacyEpochFieldV1::ExpiryHeight)?;
    statement.value_balance.validate()?;
    validate_nullifiers_with_max(
        &statement.spend_nullifiers,
        true,
        ORCHARD_MAX_ACTIONS_V1.min(limits.max_nullifiers_per_action),
    )?;
    validate_commitments_with_max(
        &statement.output_commitments,
        true,
        ORCHARD_MAX_ACTIONS_V1.min(limits.max_commitments_per_action),
    )?;
    if statement.spend_nullifiers.len() != statement.output_commitments.len() {
        return Err(
            PrivacyStatementValidationError::OrchardSpendOutputCountMismatch {
                spends: u32_len(statement.spend_nullifiers.len())?,
                outputs: u32_len(statement.output_commitments.len())?,
            },
        );
    }
    validate_encrypted_outputs(
        &statement.encrypted_outputs,
        &statement.output_commitments,
        true,
        limits,
    )
}

fn validate_fcmp(
    statement: &MoneroFcmpPlusPlusStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    require_nonzero_id(
        statement.output_set_root.is_zero(),
        PrivacyTypedFieldV1::Root,
    )?;
    require_epoch(statement.root_epoch, PrivacyEpochFieldV1::Root)?;
    validate_next_root_transition(
        statement.output_set_root,
        statement.root_epoch,
        statement.next_output_set_root,
        statement.next_output_set_root_epoch,
        PrivacyRootTransitionFieldV1::OutputSet,
    )?;
    validate_commitments_with_max(
        &statement.input_commitments,
        true,
        FCMP_MAX_INPUTS_V1.min(limits.max_commitments_per_action),
    )?;
    validate_nullifiers_with_max(
        &statement.link_tags,
        true,
        FCMP_MAX_INPUTS_V1.min(limits.max_nullifiers_per_action),
    )?;
    if statement.input_commitments.len() != statement.link_tags.len() {
        return Err(PrivacyStatementValidationError::InputLinkTagCountMismatch {
            inputs: u32_len(statement.input_commitments.len())?,
            link_tags: u32_len(statement.link_tags.len())?,
        });
    }
    validate_commitments_with_max(
        &statement.output_commitments,
        true,
        FCMP_MAX_OUTPUTS_V1.min(limits.max_commitments_per_action),
    )?;
    validate_encrypted_outputs(
        &statement.encrypted_outputs,
        &statement.output_commitments,
        true,
        limits,
    )
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
    require_nonzero_id(statement.state_root.is_zero(), PrivacyTypedFieldV1::Root)?;
    require_epoch(statement.root_epoch, PrivacyEpochFieldV1::Root)?;
    validate_next_root_transition(
        statement.state_root,
        statement.root_epoch,
        statement.next_state_root,
        statement.next_state_root_epoch,
        PrivacyRootTransitionFieldV1::ProgramState,
    )?;
    require_epoch(statement.execution_epoch, PrivacyEpochFieldV1::Execution)?;
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
    )
}

fn validate_pq_masp(
    statement: &PqMaspStarkStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    require_nonzero_id(statement.anchor.is_zero(), PrivacyTypedFieldV1::Root)?;
    require_epoch(statement.anchor_epoch, PrivacyEpochFieldV1::Root)?;
    validate_next_root_transition(
        statement.anchor,
        statement.anchor_epoch,
        statement.next_anchor,
        statement.next_anchor_epoch,
        PrivacyRootTransitionFieldV1::NoteCommitmentAnchor,
    )?;
    require_epoch(
        statement.authorization_epoch,
        PrivacyEpochFieldV1::Authorization,
    )?;
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

fn validate_validity_epochs(
    start: u64,
    end: u64,
    current: u64,
) -> Result<(), PrivacyStatementValidationError> {
    require_epoch(start, PrivacyEpochFieldV1::ValidityStart)?;
    require_epoch(end, PrivacyEpochFieldV1::ValidityEnd)?;
    require_epoch(current, PrivacyEpochFieldV1::Presentation)?;
    if end <= start || current < start || current >= end {
        return Err(PrivacyStatementValidationError::InvalidValidityWindow {
            start,
            end,
            current,
        });
    }
    Ok(())
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
            ) => {
                validate_activation_statement_count(
                    PrivacyActivationLimitFieldV1::JindoPolynomialCount,
                    u32::try_from(statement.polynomial_commitments.len()).unwrap_or(u32::MAX),
                    limits.max_polynomial_count,
                )?;
                validate_activation_statement_count(
                    PrivacyActivationLimitFieldV1::JindoEvaluationQueryCount,
                    u32::try_from(statement.evaluation_queries.len()).unwrap_or(u32::MAX),
                    limits.max_evaluation_query_count,
                )?;
                validate_activation_statement_count(
                    PrivacyActivationLimitFieldV1::JindoMultilinearVariableCount,
                    statement.regime.multilinear_variable_count,
                    limits.max_multilinear_variable_count,
                )
            }
            (
                Self::OrchardHalo2ActionsV1(limits),
                PrivacyStatementV1::OrchardHalo2ActionsV1(statement),
            ) => {
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::OrchardActionCount,
                    statement.spend_nullifiers.len(),
                    limits.max_action_count,
                )?;
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::OrchardActionCount,
                    statement.output_commitments.len(),
                    limits.max_action_count,
                )
            }
            (
                Self::MoneroFcmpPlusPlusV1(limits),
                PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement),
            ) => {
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::FcmpInputCount,
                    statement.input_commitments.len(),
                    limits.max_input_count,
                )?;
                validate_activation_statement_len(
                    PrivacyActivationLimitFieldV1::FcmpOutputCount,
                    statement.output_commitments.len(),
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
#[cfg_attr(feature = "json", norito(tag = "action", content = "proof"))]
pub enum IrohaZkAmsProofV1 {
    /// Transparent Poseidon2/Goldilocks STARK batch-admission proof.
    TransparentStarkBatchAdmission(PrivacyProofBytesV1),
    /// Canonical Ristretto255 MLSAGS account-provisioning signature.
    Ristretto255MlsagsProvisionAccount(PrivacyProofBytesV1),
}

impl IrohaZkAmsProofV1 {
    /// Borrow the exact native proof or signature bytes.
    #[must_use]
    pub const fn bytes(&self) -> &PrivacyProofBytesV1 {
        match self {
            Self::TransparentStarkBatchAdmission(bytes)
            | Self::Ristretto255MlsagsProvisionAccount(bytes) => bytes,
        }
    }

    /// Return whether this proof variant matches a typed public action.
    #[must_use]
    pub const fn matches_action(&self, action: &PrivacyZkAmsActionV1) -> bool {
        matches!(
            (self, action),
            (
                Self::TransparentStarkBatchAdmission(_),
                PrivacyZkAmsActionV1::BatchAdmission(_)
            ) | (
                Self::Ristretto255MlsagsProvisionAccount(_),
                PrivacyZkAmsActionV1::ProvisionAccount(_)
            )
        )
    }
}

/// Protocol-typed native proof payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "protocol", content = "proof"))]
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
    /// Native Iroha Jindo multilinear lattice polynomial-commitment proof.
    IrohaJindoPolynomialCommitmentV0(PrivacyProofBytesV1),
    /// Native Iroha Bootle GenISIS anonymous-credential proof.
    IrohaBootleGenisisAcStarkV0(PrivacyProofBytesV1),
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
            Self::IrohaBootleGenisisAcStarkV0(_) => {
                PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0
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
            | Self::IrohaBootleGenisisAcStarkV0(bytes)
            | Self::OrchardHalo2ActionsV1(bytes)
            | Self::MoneroFcmpPlusPlusV1(bytes)
            | Self::IrohaIvmPrivateNoteStarkV1(bytes)
            | Self::PqMaspStarkV0(bytes) => bytes,
            Self::IrohaZkAmsV1(proof) => proof.bytes(),
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
    /// Certificate subject-key digest.
    CertificateKeyDigest,
    /// Public Vega Figure 9 device-authentication digest `H_dev`.
    VegaDeviceAuthenticationDigest,
    /// ISO 18013-5 reader challenge.
    ReaderChallenge,
    /// ISO 18013-5 session transcript digest.
    SessionTranscriptDigest,
    /// Private IVM program identifier.
    ProgramId,
    /// Post-quantum authorization-key digest.
    AuthorizationKeyDigest,
    /// Post-quantum note-encryption-key digest.
    NoteEncryptionKeyDigest,
    /// ZK-AMS accumulated relaxed-R1CS instance digest.
    ZkAmsAccumulatedInstanceDigest,
    /// ZK-AMS final padding cross-term digest.
    ZkAmsPaddingCrossTermDigest,
    /// ZK-AMS final folded relaxed-R1CS instance digest.
    ZkAmsFinalFoldedInstanceDigest,
}

/// Epoch or height field used by statement validation diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyEpochFieldV1 {
    /// Commitment-root epoch.
    Root,
    /// X.509 certificate-authority membership epoch.
    CertificateAuthorityMembership,
    /// Revocation-state epoch.
    Revocation,
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
    /// Orchard or PQ-MASP note-commitment anchor.
    NoteCommitmentAnchor,
    /// FCMP++ complete output set.
    OutputSet,
    /// Native IVM private program state.
    ProgramState,
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
    /// Coordinates in one Jindo multilinear evaluation point.
    JindoEvaluationPointCoordinates,
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
    /// A protocol epoch or height is zero.
    #[error("privacy statement epoch or height {field:?} must be non-zero")]
    ZeroEpoch {
        /// Invalid epoch or height field.
        field: PrivacyEpochFieldV1,
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
    /// Credential validity epochs do not contain the presentation epoch.
    #[error(
        "privacy credential validity [{start}, {end}) does not contain current epoch {current}"
    )]
    InvalidValidityWindow {
        /// Inclusive validity start.
        start: u64,
        /// Exclusive validity end.
        end: u64,
        /// Presentation or validation epoch.
        current: u64,
    },
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
    /// X.509 extended-key-usage values contain duplicates or are out of order.
    #[error("X.509 extended key usages must be strictly increasing")]
    X509ExtendedKeyUsagesNotStrictlyIncreasing,
    /// X.509 certificate chain depth is outside the approved profile.
    #[error("X.509 chain depth {depth} is outside 1..={max}")]
    InvalidX509ChainDepth {
        /// Observed chain depth.
        depth: u8,
        /// Approved maximum.
        max: u8,
    },
    /// X.509 leaf certificate byte length is zero or exceeds the approved bound.
    #[error("X.509 leaf certificate size {bytes} is outside 1..={max}")]
    InvalidX509LeafCertificateSize {
        /// Observed DER byte length.
        bytes: u32,
        /// Approved maximum.
        max: u32,
    },
    /// X.509 complete chain byte length is inconsistent or exceeds the approved bound.
    #[error("X.509 chain size {bytes} must be at least leaf size {leaf_bytes} and at most {max}")]
    InvalidX509ChainSize {
        /// Observed combined DER bytes.
        bytes: u32,
        /// Declared leaf DER bytes.
        leaf_bytes: u32,
        /// Approved combined maximum.
        max: u32,
    },
    /// Jindo evaluation-query count is outside its approved profile.
    #[error("Jindo evaluation-query count {count} is outside 1..={max}")]
    InvalidJindoEvaluationQueryCount {
        /// Observed evaluation-query count.
        count: u32,
        /// Approved maximum.
        max: u32,
    },
    /// Jindo multilinear variable count is outside its approved profile.
    #[error("Jindo multilinear variable count {count} is outside 1..={max}")]
    InvalidJindoMultilinearVariableCount {
        /// Observed variable count.
        count: u32,
        /// Approved maximum.
        max: u32,
    },
    /// Jindo governed field-element width is outside its approved profile.
    #[error("Jindo field-element width {bytes} is outside 1..={max}")]
    InvalidJindoFieldElementSize {
        /// Observed byte width.
        bytes: u16,
        /// Approved maximum.
        max: u16,
    },
    /// A Jindo field-element encoding has the wrong governed width.
    #[error("Jindo field-element encoding uses {bytes} bytes; expected {expected}")]
    InvalidJindoFieldElementEncoding {
        /// Observed byte width.
        bytes: u32,
        /// Exact governed byte width.
        expected: u32,
    },
    /// A Jindo lattice commitment has an invalid governed width.
    #[error(
        "Jindo lattice commitment {index:?} uses {bytes} bytes; expected {expected}, profile maximum {max}"
    )]
    InvalidJindoLatticeCommitmentSize {
        /// Commitment index, or `None` when the regime itself is invalid.
        index: Option<u32>,
        /// Observed or declared byte width.
        bytes: u32,
        /// Exact governed byte width.
        expected: u32,
        /// Approved maximum.
        max: u32,
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
    /// Two Jindo openings use the same evaluation point.
    #[error("Jindo evaluation points must be distinct")]
    DuplicateJindoEvaluationPoint,
    /// SIS-with-hints attribute count differs from its fixed profile.
    #[error("SIS-with-hints attribute count {count} must equal {expected}")]
    InvalidAttributeCount {
        /// Observed attribute count.
        count: u32,
        /// Required profile count.
        expected: u32,
    },
    /// SIS-with-hints disclosed attribute count exceeds its approved profile.
    #[error("SIS-with-hints disclosed attribute count {count} exceeds {max}")]
    TooManyDisclosedAttributes {
        /// Observed disclosure count.
        count: u32,
        /// Approved maximum.
        max: u32,
    },
    /// A disclosed attribute index is outside the committed attribute vector.
    #[error(
        "SIS-with-hints disclosed attribute index {index} is outside attribute count {attribute_count}"
    )]
    DisclosedAttributeOutOfBounds {
        /// Invalid disclosed index.
        index: u16,
        /// Committed attribute count.
        attribute_count: u32,
    },
    /// Disclosed attribute indices contain a duplicate or are out of order.
    #[error("SIS-with-hints disclosed attribute indices must be strictly increasing")]
    DisclosedAttributesNotStrictlyIncreasing,
    /// A disclosed plaintext attribute is empty, degenerate, or too large.
    #[error(
        "SIS-with-hints disclosed attribute {index} has invalid length {bytes}; maximum is {max}"
    )]
    InvalidDisclosedAttributeValue {
        /// Attribute index.
        index: u16,
        /// Encoded value bytes.
        bytes: u32,
        /// Approved maximum bytes.
        max: u32,
    },
    /// A digest-only disclosed attribute has a zero digest.
    #[error("SIS-with-hints disclosed attribute {index} has a zero digest")]
    ZeroDisclosedAttributeDigest {
        /// Attribute index.
        index: u16,
    },
    /// Orchard spend and output action counts differ.
    #[error("Orchard spend count {spends} differs from output count {outputs}")]
    OrchardSpendOutputCountMismatch {
        /// Spent-note nullifier count.
        spends: u32,
        /// New-note commitment count.
        outputs: u32,
    },
    /// FCMP++ consumed-output and link-tag counts differ.
    #[error("FCMP++ input count {inputs} differs from link-tag count {link_tags}")]
    InputLinkTagCountMismatch {
        /// Consumed input commitment count.
        inputs: u32,
        /// Link-tag count.
        link_tags: u32,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
        self.validate_with_limits(&activation.limits)
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

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{domain::DomainId, name::Name};

    fn raw(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn p256_point(seed: u8) -> PrivacyP256PointV1 {
        let mut bytes = [seed; 33];
        bytes[0] = 0x02;
        PrivacyP256PointV1::new(bytes)
    }

    fn p256_ciphertext(seed: u8) -> PrivacyP256CiphertextV1 {
        PrivacyP256CiphertextV1 {
            left: p256_point(seed),
            right: p256_point(seed.wrapping_add(64)),
        }
    }

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair");
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_definition_id() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("privacy", "universal").expect("domain"),
            Name::from_str("asset").expect("asset name"),
        )
    }

    fn context() -> PrivacyStatementContextV1 {
        PrivacyStatementContextV1 {
            chain_id: "privacy-test-chain".parse().expect("chain id"),
            action_index: 0,
            parameter_id: PrivacyParameterIdV1::new(raw(1)),
            parameter_digest: PrivacyParameterDigestV1::new(raw(2)),
            verifier_digest: PrivacyVerifierDigestV1::new(raw(3)),
            statement_schema_digest: PrivacyStatementSchemaDigestV1::new(raw(4)),
            engine_manifest_digest: PrivacyEngineManifestDigestV1::new(raw(5)),
        }
    }

    fn commitment(seed: u8) -> PrivacyCommitmentV1 {
        PrivacyCommitmentV1::new(raw(seed))
    }

    fn nullifier(seed: u8) -> PrivacyNullifierV1 {
        PrivacyNullifierV1::new(raw(seed))
    }

    fn zk_ams_seed_key(seed: u8) -> PrivacyZkAmsSeedPublicKeyV1 {
        PrivacyZkAmsSeedPublicKeyV1::new(raw(seed))
    }

    fn zk_ams_anchor(seed: u8) -> PrivacyZkAmsAdmissionAnchorV1 {
        PrivacyZkAmsAdmissionAnchorV1 {
            phc_hash: PrivacyZkAmsPhcHashV1::new(raw(seed)),
            seed_public_key: zk_ams_seed_key(seed.wrapping_add(32)),
        }
    }

    fn zk_ams_provision_statement(ring_size: u8) -> PrivacyStatementV1 {
        PrivacyStatementV1::IrohaZkAmsV1(IrohaZkAmsStatementV1 {
            context: context(),
            issuer_id: PrivacyIssuerIdV1::new(raw(40)),
            registry_id: PrivacyZkAmsRegistryIdV1::new(raw(41)),
            policy_id: PrivacyPolicyIdV1::new(raw(43)),
            action: PrivacyZkAmsActionV1::ProvisionAccount(PrivacyZkAmsProvisionAccountV1 {
                account_registry_root: PrivacyRootV1::new(raw(144)),
                account_registry_root_epoch: 10,
                admitted_seed_key_ring: (1..=ring_size).map(zk_ams_seed_key).collect(),
                account_id: account(200),
                key_image: PrivacyZkAmsKeyImageV1::new(raw(201)),
            }),
        })
    }

    fn jindo_field(seed: u8) -> PrivacyJindoFieldElementV1 {
        PrivacyJindoFieldElementV1::new(vec![seed; 2])
    }

    fn jindo_commitment(seed: u8) -> PrivacyJindoLatticeCommitmentV1 {
        PrivacyJindoLatticeCommitmentV1::new(vec![seed; 4])
    }

    fn encrypted_output(commitment_seed: u8, recipient_seed: u8) -> PrivacyEncryptedOutputV1 {
        PrivacyEncryptedOutputV1 {
            recipient: PrivacyRecipientIdV1::new(raw(recipient_seed)),
            ephemeral_public_key: PrivacyEncryptionKeyV1::new(raw(recipient_seed.wrapping_add(1))),
            commitment: commitment(commitment_seed),
            ciphertext: vec![recipient_seed, commitment_seed, 0xA5],
        }
    }

    fn sample_statements() -> Vec<PrivacyStatementV1> {
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
                fee: 2,
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
                registry_id: PrivacyZkAmsRegistryIdV1::new(raw(41)),
                policy_id: PrivacyPolicyIdV1::new(raw(43)),
                action: PrivacyZkAmsActionV1::BatchAdmission(PrivacyZkAmsBatchAdmissionV1 {
                    account_registry_root: PrivacyRootV1::new(raw(144)),
                    account_registry_root_epoch: 10,
                    next_account_registry_root: PrivacyRootV1::new(raw(145)),
                    next_account_registry_root_epoch: 11,
                    anchors: vec![zk_ams_anchor(44), zk_ams_anchor(45)],
                    accumulated_instance_digest: PrivacyZkAmsAccumulatedInstanceDigestV1::new(raw(
                        46,
                    )),
                    padding_cross_term_digest: PrivacyZkAmsPaddingCrossTermDigestV1::new(raw(47)),
                    final_folded_instance_digest: PrivacyZkAmsFinalFoldedInstanceDigestV1::new(
                        raw(48),
                    ),
                }),
            }),
            PrivacyStatementV1::VegaExistingCredentialZkV0(VegaExistingCredentialStatementV1 {
                context: context(),
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
                subject_public_key_digest: PrivacyCertificateKeyDigestV1::new(raw(63)),
                ca_membership_root: PrivacyRootV1::new(raw(64)),
                ca_membership_root_epoch: 10,
                crl_nonmembership_root: PrivacyRootV1::new(raw(65)),
                crl_nonmembership_root_epoch: 11,
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
                not_before_unix_seconds: 1_000,
                not_after_unix_seconds: 2_000,
                validation_unix_seconds: 1_500,
                chain_depth: 3,
                leaf_certificate_bytes: 2_048,
                chain_certificate_bytes: 6_144,
                wallet_account: account(66),
                wallet_challenge: PrivacyChallengeV1::new(raw(67)),
                certificate_nullifier: nullifier(68),
            }),
            PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(
                IrohaJindoPolynomialCommitmentStatementV1 {
                    context: context(),
                    regime: PrivacyJindoParameterRegimeV1 {
                        multilinear_variable_count: 2,
                        field_element_bytes: 2,
                        lattice_commitment_bytes: 4,
                        evaluation_hiding: true,
                    },
                    polynomial_commitments: vec![jindo_commitment(70), jindo_commitment(71)],
                    evaluation_queries: vec![
                        PrivacyJindoEvaluationQueryV1 {
                            evaluation_point: vec![jindo_field(1), jindo_field(2)],
                            claimed_evaluations: vec![jindo_field(4), jindo_field(5)],
                        },
                        PrivacyJindoEvaluationQueryV1 {
                            evaluation_point: vec![jindo_field(6), jindo_field(7)],
                            claimed_evaluations: vec![jindo_field(8), jindo_field(9)],
                        },
                    ],
                },
            ),
            PrivacyStatementV1::IrohaBootleGenisisAcStarkV0(IrohaBootleGenisisAcStarkStatementV1 {
                context: context(),
                issuer_id: PrivacyIssuerIdV1::new(raw(72)),
                policy_id: PrivacyPolicyIdV1::new(raw(73)),
                issuer_parameter_id: PrivacyParameterIdV1::new(raw(74)),
                issuer_parameter_digest: PrivacyParameterDigestV1::new(raw(75)),
                credential_commitment: commitment(76),
                hints_commitment: commitment(77),
                revocation_root: PrivacyRootV1::new(raw(78)),
                revocation_epoch: 12,
                presentation_nullifier: nullifier(79),
                attribute_count: SIS_WITH_HINTS_ATTRIBUTE_COUNT_V1,
                disclosures: vec![
                    PrivacyDisclosedAttributeV1 {
                        index: 1,
                        value: PrivacyDisclosedAttributeValueV1::Plaintext(vec![1, 2]),
                    },
                    PrivacyDisclosedAttributeV1 {
                        index: 4,
                        value: PrivacyDisclosedAttributeValueV1::Digest(
                            PrivacyAttributeDigestV1::new(raw(80)),
                        ),
                    },
                ],
            }),
            PrivacyStatementV1::OrchardHalo2ActionsV1(OrchardHalo2ActionsStatementV1 {
                context: context(),
                asset_definition_id: asset.clone(),
                pool_id: PrivacyPoolIdV1::new(raw(81)),
                anchor: PrivacyRootV1::new(raw(82)),
                anchor_epoch: 13,
                next_anchor: PrivacyRootV1::new(raw(86)),
                next_anchor_epoch: 14,
                spend_nullifiers: vec![nullifier(83)],
                output_commitments: vec![commitment(84)],
                encrypted_outputs: vec![encrypted_output(84, 85)],
                value_balance: PrivacyValueBalanceV1::balanced(),
                fee: 2,
                expiry_height: 10_000,
            }),
            PrivacyStatementV1::MoneroFcmpPlusPlusV1(MoneroFcmpPlusPlusStatementV1 {
                context: context(),
                asset_definition_id: asset.clone(),
                pool_id: PrivacyPoolIdV1::new(raw(87)),
                output_set_root: PrivacyRootV1::new(raw(88)),
                root_epoch: 14,
                next_output_set_root: PrivacyRootV1::new(raw(93)),
                next_output_set_root_epoch: 15,
                input_commitments: vec![commitment(89)],
                link_tags: vec![nullifier(90)],
                output_commitments: vec![commitment(91)],
                encrypted_outputs: vec![encrypted_output(91, 92)],
                fee: 2,
            }),
            PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(IrohaIvmPrivateNoteStarkStatementV1 {
                context: context(),
                asset_definition_id: asset.clone(),
                pool_id: PrivacyPoolIdV1::new(raw(94)),
                program_id: PrivacyProgramIdV1::new(raw(95)),
                state_root: PrivacyRootV1::new(raw(96)),
                root_epoch: 15,
                next_state_root: PrivacyRootV1::new(raw(100)),
                next_state_root_epoch: 16,
                nullifiers: vec![nullifier(97)],
                output_commitments: vec![commitment(98)],
                encrypted_outputs: vec![encrypted_output(98, 99)],
                value_balance: PrivacyValueBalanceV1::balanced(),
                fee: 2,
                execution_epoch: 16,
            }),
            PrivacyStatementV1::PqMaspStarkV0(PqMaspStarkStatementV1 {
                context: context(),
                asset_definition_id: asset,
                pool_id: PrivacyPoolIdV1::new(raw(101)),
                anchor: PrivacyRootV1::new(raw(102)),
                anchor_epoch: 17,
                next_anchor: PrivacyRootV1::new(raw(106)),
                next_anchor_epoch: 18,
                nullifiers: vec![nullifier(103)],
                output_commitments: vec![commitment(104)],
                encrypted_outputs: vec![encrypted_output(104, 105)],
                fee: 2,
                authorization_profile: PrivacyPqAuthorizationProfileV1::MlDsa65,
                authorization_key_digest: PrivacyAuthorizationKeyDigestV1::new(raw(107)),
                note_encryption_profile:
                    PrivacyPqNoteEncryptionProfileV1::MlKem768XChaCha20Poly1305,
                note_encryption_key_digest: PrivacyNoteEncryptionKeyDigestV1::new(raw(108)),
                authorization_epoch: 18,
            }),
        ]
    }

    fn statement_for(protocol: PrivacyProtocolIdV1) -> PrivacyStatementV1 {
        sample_statements()
            .into_iter()
            .find(|statement| statement.protocol_id() == protocol)
            .expect("sample statement for every protocol")
    }

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
            PrivacyStatementV1::OrchardHalo2ActionsV1(statement) => corrupt!(
                statement.anchor,
                statement.anchor_epoch,
                statement.next_anchor,
                statement.next_anchor_epoch
            ),
            PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) => corrupt!(
                statement.output_set_root,
                statement.root_epoch,
                statement.next_output_set_root,
                statement.next_output_set_root_epoch
            ),
            PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) => corrupt!(
                statement.state_root,
                statement.root_epoch,
                statement.next_state_root,
                statement.next_state_root_epoch
            ),
            PrivacyStatementV1::PqMaspStarkV0(statement) => corrupt!(
                statement.anchor,
                statement.anchor_epoch,
                statement.next_anchor,
                statement.next_anchor_epoch
            ),
            _ => panic!("protocol does not manage a root transition"),
        }
    }

    fn proof_for(protocol: PrivacyProtocolIdV1) -> PrivacyProofV1 {
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
                IrohaZkAmsProofV1::TransparentStarkBatchAdmission(bytes),
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
            PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0 => {
                PrivacyProofV1::IrohaBootleGenisisAcStarkV0(bytes)
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
                        max_evaluation_query_count: IROHA_JINDO_MAX_EVALUATION_QUERIES_V1,
                        max_multilinear_variable_count: IROHA_JINDO_MAX_MULTILINEAR_VARIABLES_V1,
                    },
                )
            }
            PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0 => {
                PrivacyProtocolActivationLimitsV1::IrohaBootleGenisisAcStarkV0
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

    fn envelope(statement: PrivacyStatementV1) -> PrivacyProofEnvelopeV1 {
        let protocol_id = statement.protocol_id();
        let context = statement.context().clone();
        let statement_digest = statement.digest().expect("statement digest");
        let proof = match &statement {
            PrivacyStatementV1::IrohaZkAmsV1(IrohaZkAmsStatementV1 {
                action: PrivacyZkAmsActionV1::ProvisionAccount(_),
                ..
            }) => {
                PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::Ristretto255MlsagsProvisionAccount(
                    PrivacyProofBytesV1::new(vec![0xA5, 0x5A, 1]),
                ))
            }
            _ => proof_for(protocol_id),
        };
        PrivacyProofEnvelopeV1 {
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
        }
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
            limits: PrivacyConsensusLimitsV1::taira_default(),
            protocol_limits: protocol_limits(envelope.protocol_id),
            assurance: PrivacyAssuranceV1::Experimental,
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
                PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0,
                "iroha-bootle-genisis-ac-stark-v0",
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
    fn all_protocol_mappings_and_typed_variants_are_exact() {
        let statements = sample_statements();
        assert_eq!(statements.len(), PrivacyProtocolIdV1::COUNT);
        for (protocol, statement) in PrivacyProtocolIdV1::ALL.into_iter().zip(statements) {
            assert_eq!(statement.protocol_id(), protocol);
            assert_eq!(proof_for(protocol).protocol_id(), protocol);
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
            PrivacyProofSystemIdV1::ZkAmsTransparentStarkPoseidon2GoldilocksMlsagsRistretto255Sha3_512
        );
        assert_eq!(
            PrivacyProtocolIdV1::IrohaZkAmsV1.expected_engine(),
            PrivacyEngineIdV1::NativeZkAmsTransparentStarkMlsagsRistretto255
        );
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
                assert_eq!(encoded.len(), 32);
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
        check_type!(PrivacyNullifierV1, 7);
        check_type!(PrivacyCommitmentV1, 8);
        check_type!(PrivacyPoolIdV1, 9);
        check_type!(PrivacyZkAmsRegistryIdV1, 10);
        check_type!(PrivacyPolicyIdV1, 10);
        check_type!(PrivacyRootV1, 11);
        check_type!(PrivacyChallengeV1, 12);
        check_type!(PrivacyZkAmsPhcHashV1, 13);
        check_type!(PrivacyZkAmsAccumulatedInstanceDigestV1, 14);
        check_type!(PrivacyZkAmsPaddingCrossTermDigestV1, 15);
        check_type!(PrivacyZkAmsFinalFoldedInstanceDigestV1, 16);
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
                .validate_against_activation(&activation, 2)
                .expect("valid active envelope");

            let bytes = norito::to_bytes(&envelope).expect("frame envelope");
            let decoded: PrivacyProofEnvelopeV1 =
                norito::decode_from_bytes(&bytes).expect("decode envelope");
            assert_eq!(decoded, envelope);
        }
    }

    #[test]
    fn zk_ams_envelope_requires_the_proof_variant_for_its_exact_action() {
        let limits = PrivacyConsensusLimitsV1::taira_default();

        let batch_statement = statement_for(PrivacyProtocolIdV1::IrohaZkAmsV1);
        let mut batch_envelope = envelope(batch_statement);
        batch_envelope
            .validate_with_limits(&limits)
            .expect("batch STARK proof variant");
        batch_envelope.proof =
            PrivacyProofV1::IrohaZkAmsV1(IrohaZkAmsProofV1::Ristretto255MlsagsProvisionAccount(
                PrivacyProofBytesV1::new(vec![1]),
            ));
        assert!(matches!(
            batch_envelope.validate_with_limits(&limits),
            Err(PrivacyProofEnvelopeValidationError::ZkAmsActionProofMismatch)
        ));

        let provision_statement = zk_ams_provision_statement(16);
        let mut provision_envelope = envelope(provision_statement);
        provision_envelope
            .validate_with_limits(&limits)
            .expect("provisioning MLSAGS proof variant");
        provision_envelope.proof = PrivacyProofV1::IrohaZkAmsV1(
            IrohaZkAmsProofV1::TransparentStarkBatchAdmission(PrivacyProofBytesV1::new(vec![1])),
        );
        assert!(matches!(
            provision_envelope.validate_with_limits(&limits),
            Err(PrivacyProofEnvelopeValidationError::ZkAmsActionProofMismatch)
        ));
    }

    #[test]
    fn p256_wire_types_are_exact_width_and_closed() {
        let point = p256_point(9);
        let encoded = point.encode();
        assert_eq!(encoded.len(), 33);
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
        assert_eq!(encoded.len(), 32);
        assert_eq!(
            PrivacyZkAmsSeedPublicKeyV1::decode(&mut encoded.as_slice())
                .expect("decode exact seed key"),
            seed_key
        );
        assert!(PrivacyZkAmsSeedPublicKeyV1::decode(&mut [9; 31].as_slice()).is_err());
        assert!(PrivacyZkAmsSeedPublicKeyV1::decode(&mut [9; 33].as_slice()).is_err());

        let key_image = PrivacyZkAmsKeyImageV1::new(raw(10));
        assert_eq!(key_image.encode().len(), 32);
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
    }

    #[test]
    fn taira_consensus_limits_reject_zero_overflow_and_inconsistent_profiles() {
        let defaults = PrivacyConsensusLimitsV1::taira_default();
        defaults.validate().expect("Taira defaults");
        assert_eq!(
            defaults.max_actions_per_transaction,
            TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1
        );
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
            PrivacyRootRoleV1::CertificateRevocationNonmembership,
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

        let mut invalid = publication.clone();
        invalid.epoch = 0;
        assert!(matches!(
            invalid.validate(),
            Err(PrivacyRootPublicationValidationError::ZeroEpoch)
        ));
        invalid = publication.clone();
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
    fn every_proof_managed_root_requires_a_distinct_exact_successor() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let protocols = [
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            PrivacyProtocolIdV1::IrohaZkAmsV1,
            PrivacyProtocolIdV1::OrchardHalo2ActionsV1,
            PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1,
            PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1,
            PrivacyProtocolIdV1::PqMaspStarkV0,
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
                    max_evaluation_query_count: IROHA_JINDO_MAX_EVALUATION_QUERIES_V1,
                    max_multilinear_variable_count: IROHA_JINDO_MAX_MULTILINEAR_VARIABLES_V1,
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
    fn zk_ams_batch_binds_source_objects_and_rejects_malformed_anchors() {
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
        assert!(matches!(
            mutate(|batch| {
                batch.accumulated_instance_digest =
                    PrivacyZkAmsAccumulatedInstanceDigestV1::new([0; 32]);
            }),
            Err(PrivacyStatementValidationError::ZeroTypedField {
                field: PrivacyTypedFieldV1::ZkAmsAccumulatedInstanceDigest
            })
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
    fn jindo_multilinear_profile_rejects_malformed_shapes_and_encodings() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let mut value = statement_for(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0);
        let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut value else {
            unreachable!()
        };
        statement.evaluation_queries[0].evaluation_point[0] =
            PrivacyJindoFieldElementV1::new(vec![0; 2]);
        statement.evaluation_queries[0].claimed_evaluations[0] =
            PrivacyJindoFieldElementV1::new(vec![0; 2]);
        value
            .validate(&limits)
            .expect("zero is a valid governed field element");

        let mut duplicate = value.clone();
        let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut duplicate else {
            unreachable!()
        };
        statement.evaluation_queries[1].evaluation_point =
            statement.evaluation_queries[0].evaluation_point.clone();
        assert!(matches!(
            duplicate.validate(&limits),
            Err(PrivacyStatementValidationError::DuplicateJindoEvaluationPoint)
        ));

        let mut bad_matrix = value.clone();
        let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut bad_matrix
        else {
            unreachable!()
        };
        statement.evaluation_queries[0].claimed_evaluations.pop();
        assert!(matches!(
            bad_matrix.validate(&limits),
            Err(PrivacyStatementValidationError::DeclaredCountMismatch {
                field: PrivacyCountFieldV1::JindoClaimedEvaluations,
                ..
            })
        ));

        let mut bad_point = value.clone();
        let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut bad_point else {
            unreachable!()
        };
        statement.evaluation_queries[0].evaluation_point.pop();
        assert!(matches!(
            bad_point.validate(&limits),
            Err(PrivacyStatementValidationError::DeclaredCountMismatch {
                field: PrivacyCountFieldV1::JindoEvaluationPointCoordinates,
                ..
            })
        ));

        let mut bad_field_width = value.clone();
        let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut bad_field_width
        else {
            unreachable!()
        };
        statement.evaluation_queries[0].evaluation_point[0]
            .encoding
            .push(1);
        assert!(matches!(
            bad_field_width.validate(&limits),
            Err(
                PrivacyStatementValidationError::InvalidJindoFieldElementEncoding {
                    bytes: 3,
                    expected: 2
                }
            )
        ));

        let mut zero_commitment = value.clone();
        let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut zero_commitment
        else {
            unreachable!()
        };
        statement.polynomial_commitments[0].encoding.fill(0);
        assert!(matches!(
            zero_commitment.validate(&limits),
            Err(PrivacyStatementValidationError::AllZeroJindoLatticeCommitment { index: 0 })
        ));

        let mut wrong_commitment_width = value;
        let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) =
            &mut wrong_commitment_width
        else {
            unreachable!()
        };
        statement.polynomial_commitments[0].encoding.push(1);
        assert!(matches!(
            wrong_commitment_width.validate(&limits),
            Err(
                PrivacyStatementValidationError::InvalidJindoLatticeCommitmentSize {
                    index: Some(0),
                    bytes: 5,
                    expected: 4,
                    ..
                }
            )
        ));

        let base = statement_for(PrivacyProtocolIdV1::IrohaJindoPolynomialCommitmentV0);
        let mutate = |f: fn(&mut IrohaJindoPolynomialCommitmentStatementV1)| {
            let mut value = base.clone();
            let PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(statement) = &mut value else {
                unreachable!()
            };
            f(statement);
            value.validate(&limits)
        };
        assert!(matches!(
            mutate(|statement| statement.regime.multilinear_variable_count = 0),
            Err(
                PrivacyStatementValidationError::InvalidJindoMultilinearVariableCount {
                    count: 0,
                    ..
                }
            )
        ));
        assert!(matches!(
            mutate(|statement| statement.regime.field_element_bytes = 0),
            Err(PrivacyStatementValidationError::InvalidJindoFieldElementSize { bytes: 0, .. })
        ));
        assert!(matches!(
            mutate(|statement| statement.regime.lattice_commitment_bytes = 0),
            Err(
                PrivacyStatementValidationError::InvalidJindoLatticeCommitmentSize {
                    index: None,
                    bytes: 0,
                    ..
                }
            )
        ));
        assert!(matches!(
            mutate(|statement| statement.evaluation_queries.clear()),
            Err(PrivacyStatementValidationError::InvalidJindoEvaluationQueryCount { count: 0, .. })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.polynomial_commitments[1] = statement.polynomial_commitments[0].clone();
            }),
            Err(PrivacyStatementValidationError::DuplicateJindoLatticeCommitment)
        ));

        let governed = PrivacyProtocolActivationLimitsV1::IrohaJindoPolynomialCommitmentV0(
            JindoActivationLimitsV1 {
                max_polynomial_count: 1,
                max_evaluation_query_count: 1,
                max_multilinear_variable_count: 1,
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
    }

    #[test]
    fn x509_rejects_stale_roots_invalid_usage_and_invalid_chain_sizes() {
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
        assert!(mutate_x509(|statement| statement.crl_nonmembership_root_epoch = 0).is_err());
        assert!(matches!(
            mutate_x509(|statement| statement.key_usage.digital_signature = false),
            Err(PrivacyStatementValidationError::InvalidX509KeyUsage)
        ));
        assert!(matches!(
            mutate_x509(|statement| statement.extended_key_usages.clear()),
            Err(PrivacyStatementValidationError::MissingX509ExtendedKeyUsage)
        ));
        assert!(matches!(
            mutate_x509(|statement| statement.chain_depth = ZK_X509_MAX_CHAIN_DEPTH_V1 + 1),
            Err(PrivacyStatementValidationError::InvalidX509ChainDepth { .. })
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.leaf_certificate_bytes = ZK_X509_MAX_CERTIFICATE_BYTES_V1 + 1
            }),
            Err(PrivacyStatementValidationError::InvalidX509LeafCertificateSize { .. })
        ));
        assert!(matches!(
            mutate_x509(|statement| {
                statement.chain_certificate_bytes = statement.leaf_certificate_bytes - 1
            }),
            Err(PrivacyStatementValidationError::InvalidX509ChainSize { .. })
        ));
    }

    #[test]
    fn sis_disclosures_are_exact_bounded_and_canonically_ordered() {
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let base = statement_for(PrivacyProtocolIdV1::IrohaBootleGenisisAcStarkV0);
        let mutate = |f: fn(&mut IrohaBootleGenisisAcStarkStatementV1)| {
            let mut value = base.clone();
            let PrivacyStatementV1::IrohaBootleGenisisAcStarkV0(statement) = &mut value else {
                unreachable!()
            };
            f(statement);
            value.validate(&limits)
        };
        assert!(matches!(
            mutate(|statement| statement.attribute_count = 7),
            Err(PrivacyStatementValidationError::InvalidAttributeCount { .. })
        ));
        assert!(matches!(
            mutate(|statement| statement.disclosures.swap(0, 1)),
            Err(PrivacyStatementValidationError::DisclosedAttributesNotStrictlyIncreasing)
        ));
        assert!(matches!(
            mutate(|statement| statement.disclosures[1].index = 8),
            Err(PrivacyStatementValidationError::DisclosedAttributeOutOfBounds { .. })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.disclosures[0].value =
                    PrivacyDisclosedAttributeValueV1::Plaintext(vec![0; 2])
            }),
            Err(PrivacyStatementValidationError::InvalidDisclosedAttributeValue { .. })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.disclosures[0].value = PrivacyDisclosedAttributeValueV1::Plaintext(vec![
                        1;
                        usize::try_from(
                            SIS_WITH_HINTS_MAX_ATTRIBUTE_BYTES_V1 + 1
                        )
                        .expect("bound fits usize")
                    ])
            }),
            Err(PrivacyStatementValidationError::InvalidDisclosedAttributeValue { .. })
        ));
        assert!(matches!(
            mutate(|statement| {
                statement.disclosures[1].value =
                    PrivacyDisclosedAttributeValueV1::Digest(PrivacyAttributeDigestV1::new([0; 32]))
            }),
            Err(PrivacyStatementValidationError::ZeroDisclosedAttributeDigest { .. })
        ));
    }

    #[test]
    fn private_transfer_shapes_enforce_hard_caps_and_ordered_ciphertexts() {
        let limits = PrivacyConsensusLimitsV1::taira_default();

        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.output_commitments.push(commitment(110));
        statement.encrypted_outputs.push(encrypted_output(110, 111));
        assert!(matches!(
            orchard.validate(&limits),
            Err(
                PrivacyStatementValidationError::OrchardSpendOutputCountMismatch {
                    spends: 1,
                    outputs: 2
                }
            )
        ));

        let mut orchard = statement_for(PrivacyProtocolIdV1::OrchardHalo2ActionsV1);
        let PrivacyStatementV1::OrchardHalo2ActionsV1(statement) = &mut orchard else {
            unreachable!()
        };
        statement.spend_nullifiers = vec![nullifier(110), nullifier(111), nullifier(112)];
        statement.output_commitments = vec![commitment(113), commitment(114), commitment(115)];
        statement.encrypted_outputs = vec![
            encrypted_output(113, 116),
            encrypted_output(114, 118),
            encrypted_output(115, 120),
        ];
        assert!(matches!(
            orchard.validate(&limits),
            Err(PrivacyStatementValidationError::TooManyNullifiers {
                count: 3,
                max: ORCHARD_MAX_ACTIONS_V1
            })
        ));

        let mut fcmp = statement_for(PrivacyProtocolIdV1::MoneroFcmpPlusPlusV1);
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) = &mut fcmp else {
            unreachable!()
        };
        statement.input_commitments = vec![commitment(121), commitment(122), commitment(123)];
        statement.link_tags = vec![nullifier(124), nullifier(125), nullifier(126)];
        assert!(matches!(
            fcmp.validate(&limits),
            Err(PrivacyStatementValidationError::TooManyCommitments {
                count: 3,
                max: FCMP_MAX_INPUTS_V1
            })
        ));

        let mut ivm = statement_for(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
        let PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(statement) = &mut ivm else {
            unreachable!()
        };
        statement.nullifiers = vec![nullifier(127), nullifier(128), nullifier(129)];
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
            envelope.validate_against_activation(&activation, 4),
            Err(
                PrivacyProofEnvelopeValidationError::ActivationNotEffective {
                    current_height: 4,
                    effective_height: 5,
                }
            )
        );
        assert_eq!(envelope.validate_against_activation(&activation, 5), Ok(()));
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
        base.validate_against_activation(&governed, 2)
            .expect("active matching activation");
        assert!(base.validate_against_activation(&governed, 1).is_err());
        governed.parameter_digest = PrivacyParameterDigestV1::new(raw(222));
        assert!(base.validate_against_activation(&governed, 2).is_err());
        governed = activation(&base);
        governed.lifecycle = PrivacyProtocolLifecycleV1::Suspended(PrivacySuspendedLifecycleV1 {
            proposed_at_height: 1,
            activated_at_height: 2,
            state_since_height: 3,
        });
        assert!(base.validate_against_activation(&governed, 3).is_err());
        governed = activation(&base);
        governed.protocol_limits = PrivacyProtocolActivationLimitsV1::VeRangeTransparentRangeV1(
            VeRangeActivationLimitsV1 {
                max_aggregation_count: 1,
            },
        );
        assert!(base.validate_against_activation(&governed, 2).is_err());

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
