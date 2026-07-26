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

/// Maximum privacy actions admitted in one Taira transaction.
pub const TAIRA_PRIVACY_MAX_ACTIONS_PER_TRANSACTION_V1: u32 = 1;
/// Maximum privacy actions admitted in one Taira block.
pub const TAIRA_PRIVACY_MAX_ACTIONS_PER_BLOCK_V1: u32 = 2;
/// Maximum proof payload bytes admitted for one Taira privacy action.
pub const TAIRA_PRIVACY_MAX_PROOF_BYTES_PER_ACTION_V1: u32 = 8 * 1024 * 1024;
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
    /// Native Iroha ZK-AMS STARK admission protocol v0.
    IrohaZkAmsStarkV0,
    /// Vega proof over an existing credential v0.
    VegaExistingCredentialZkV0,
    /// Native Iroha P-256 X.509 predicate STARK protocol v0.
    IrohaZkX509StarkP256V0,
    /// Native Iroha Jindo multilinear-extension profile over Pallas, dimension 19.
    IrohaJindoMlePallasD19V0,
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
        Self::IrohaZkAmsStarkV0,
        Self::VegaExistingCredentialZkV0,
        Self::IrohaZkX509StarkP256V0,
        Self::IrohaJindoMlePallasD19V0,
        Self::IrohaBootleGenisisAcStarkV0,
        Self::OrchardHalo2ActionsV1,
        Self::MoneroFcmpPlusPlusV1,
        Self::IrohaIvmPrivateNoteStarkV1,
        Self::PqMaspStarkV0,
    ];

    /// Exact proof system required by this protocol.
    #[must_use]
    pub const fn expected_proof_system(self) -> PrivacyProofSystemIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV0 | Self::PqMaspStarkV0 => {
                PrivacyProofSystemIdV1::StarkFriSha256Goldilocks
            }
            Self::IrohaZkAmsStarkV0
            | Self::IrohaZkX509StarkP256V0
            | Self::IrohaBootleGenisisAcStarkV0
            | Self::IrohaIvmPrivateNoteStarkV1 => {
                PrivacyProofSystemIdV1::StarkFriPoseidon2Goldilocks
            }
            Self::AnonymousPgcKOutOfNV1 => PrivacyProofSystemIdV1::AnonymousPgcP256,
            Self::VeRangeTransparentRangeV1 => PrivacyProofSystemIdV1::VeRangeP256,
            Self::VegaExistingCredentialZkV0 => {
                PrivacyProofSystemIdV1::VegaNeutronNovaSpartanHyraxT256
            }
            Self::IrohaJindoMlePallasD19V0 => PrivacyProofSystemIdV1::JindoMlePallasD19,
            Self::OrchardHalo2ActionsV1 => PrivacyProofSystemIdV1::Halo2IpaPasta,
            Self::MoneroFcmpPlusPlusV1 => PrivacyProofSystemIdV1::FcmpPlusPlusCurveTreeBulletproofs,
        }
    }

    /// Exact native verifier engine required by this protocol.
    #[must_use]
    pub const fn expected_engine(self) -> PrivacyEngineIdV1 {
        match self {
            Self::ZkAcePqAuthorizationV0
            | Self::IrohaZkAmsStarkV0
            | Self::IrohaZkX509StarkP256V0
            | Self::IrohaBootleGenisisAcStarkV0
            | Self::IrohaIvmPrivateNoteStarkV1
            | Self::PqMaspStarkV0 => PrivacyEngineIdV1::NativeGoldilocksStarkFri,
            Self::AnonymousPgcKOutOfNV1 => PrivacyEngineIdV1::NativeAnonymousPgcP256,
            Self::VeRangeTransparentRangeV1 => PrivacyEngineIdV1::NativeVeRangeP256,
            Self::VegaExistingCredentialZkV0 => PrivacyEngineIdV1::NativeVega,
            Self::IrohaJindoMlePallasD19V0 => PrivacyEngineIdV1::NativeJindo,
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
    /// Anonymous PGC k-out-of-n proof system over P-256.
    AnonymousPgcP256,
    /// VeRange proof system over P-256.
    VeRangeP256,
    /// Vega Neutron/Nova/Spartan proof system with Hyrax commitments over T256.
    VegaNeutronNovaSpartanHyraxT256,
    /// Jindo multilinear-extension proof system over Pallas at dimension 19.
    JindoMlePallasD19,
    /// Halo2 IPA proof system over the Pasta curve cycle.
    Halo2IpaPasta,
    /// FCMP++ Curve Tree and Bulletproofs proof composition.
    FcmpPlusPlusCurveTreeBulletproofs,
}

/// Native verifier engine implementation selected by a privacy protocol.
///
/// Engine identity binds the audited Rust implementation independently of the
/// mathematical proof-system profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "engine", content = "value"))]
pub enum PrivacyEngineIdV1 {
    /// Native Goldilocks STARK/FRI verifier.
    NativeGoldilocksStarkFri,
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
    /// Digest of the audited native engine manifest.
    PrivacyEngineManifestDigestV1
);
define_privacy_digest!(
    /// Digest of a canonical protocol-specific public statement.
    PrivacyStatementDigestV1
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
    /// Fixed identifier of a credential schema.
    PrivacyCredentialSchemaIdV1
);
define_privacy_digest!(
    /// Fixed identifier of a governed credential predicate.
    PrivacyPredicateIdV1
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
    /// Canonical little-endian Pallas scalar encoding.
    PrivacyPallasScalarV1
);

impl PrivacyPallasScalarV1 {
    /// Return whether the little-endian value is below the Pallas scalar modulus.
    #[must_use]
    pub fn is_canonical(&self) -> bool {
        const PALLAS_SCALAR_MODULUS_LE: [u8; 32] = [
            0x01, 0x00, 0x00, 0x00, 0x21, 0xeb, 0x46, 0x8c, 0xdd, 0xa8, 0x94, 0x09, 0xfc, 0x98,
            0x46, 0x22, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x40,
        ];
        for index in (0..self.0.len()).rev() {
            if self.0[index] < PALLAS_SCALAR_MODULUS_LE[index] {
                return true;
            }
            if self.0[index] > PALLAS_SCALAR_MODULUS_LE[index] {
                return false;
            }
        }
        false
    }
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "assurance", content = "value"))]
pub enum PrivacyAssuranceV1 {
    /// Testnet-only experimental activation pending production audit gates.
    Experimental,
}

/// Governed activation record for one exact privacy protocol implementation.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    /// Lifecycle is invalid.
    #[error("privacy activation lifecycle is invalid: {0}")]
    Lifecycle(PrivacyLifecycleValidationError),
    /// Consensus limits are invalid.
    #[error("privacy activation limits are invalid: {0}")]
    Limits(PrivacyConsensusLimitsValidationError),
}

/// Maximum receiver-set size for Anonymous PGC v1.
pub const ANONYMOUS_PGC_MAX_RECEIVERS_V1: u32 = 8;
/// Maximum VeRange aggregation count in the first release.
pub const VERANGE_MAX_AGGREGATION_COUNT_V1: u32 = 8;
/// Maximum recursively admitted accounts in one ZK-AMS statement.
pub const ZK_AMS_MAX_BATCH_SIZE_V1: u32 = 8;
/// Maximum ZK-AMS recursion depth in the first release.
pub const ZK_AMS_MAX_RECURSION_DEPTH_V1: u16 = 8;
/// Fixed multilinear-extension dimension of the native Jindo profile.
pub const IROHA_JINDO_MLE_DIMENSION_V1: usize = 19;
/// Maximum batched polynomial openings in one native Jindo statement.
pub const IROHA_JINDO_MAX_BATCH_SIZE_V1: u32 = 8;
/// Maximum credential attributes bound by the SIS-with-hints profile.
pub const SIS_WITH_HINTS_MAX_ATTRIBUTE_COUNT_V1: u32 = 64;
/// Maximum selectively disclosed attribute indices in one SIS-with-hints statement.
pub const SIS_WITH_HINTS_MAX_DISCLOSED_ATTRIBUTES_V1: u32 = 8;

/// Explicit chain and governed-artifact binding shared by every statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
}

impl PrivacyStatementContextV1 {
    /// Validate non-zero governed artifact bindings.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyStatementValidationError`] if any fixed artifact
    /// identifier or digest is zero.
    pub fn validate(&self) -> Result<(), PrivacyStatementValidationError> {
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
        Ok(())
    }
}

/// Typed encrypted output emitted by a private transfer.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    /// Admitted anonymous-account accumulator root.
    pub anonymity_root: PrivacyRootV1,
    /// Epoch at which `anonymity_root` was canonical.
    pub root_epoch: u64,
    /// Commitment to the spending anonymous account.
    pub sender_account_commitment: PrivacyCommitmentV1,
    /// Commitment to the complete receiver candidate set.
    pub receiver_set_root: PrivacyRootV1,
    /// Required recipient threshold `k`.
    pub threshold: u32,
    /// Receiver candidate count `n`.
    pub receiver_count: u32,
    /// Commitment to the hidden transfer amount.
    pub amount_commitment: PrivacyCommitmentV1,
    /// Commitment to the fee-conservation term.
    pub fee_commitment: PrivacyCommitmentV1,
    /// Public atomic validation fee.
    pub fee: u128,
    /// Link tag preventing reuse of the sender state.
    pub link_tag: PrivacyNullifierV1,
    /// Commitments to the new receiver states.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Encrypted receiver state, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
}

/// VeRange transparent-bound range-proof statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VeRangeTransparentRangeStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Asset whose atomic values are committed.
    pub asset_definition_id: AssetDefinitionId,
    /// Policy selecting the commitment domain and admitted bounds.
    pub policy_id: PrivacyPolicyIdV1,
    /// Value commitments proved in this aggregate.
    pub value_commitments: Vec<PrivacyCommitmentV1>,
    /// Inclusive public lower bound.
    pub minimum: u128,
    /// Exclusive public upper bound.
    pub maximum: u128,
    /// Bit width of each committed value.
    pub bit_length: u16,
    /// Number of aggregated value commitments.
    pub aggregation_count: u32,
}

/// Native ZK-AMS recursive anonymous-admission statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IrohaZkAmsStarkStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Credential issuer namespace.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Anonymous-account destination pool.
    pub pool_id: PrivacyPoolIdV1,
    /// Canonical issuer credential root.
    pub issuer_root: PrivacyRootV1,
    /// Epoch at which `issuer_root` was canonical.
    pub issuer_epoch: u64,
    /// Admission policy identifier.
    pub policy_id: PrivacyPolicyIdV1,
    /// Admission nullifiers, one per admitted account.
    pub admission_nullifiers: Vec<PrivacyNullifierV1>,
    /// Anonymous account commitments admitted by this batch.
    pub account_commitments: Vec<PrivacyCommitmentV1>,
    /// Declared batch size.
    pub batch_size: u32,
    /// Recursive aggregation depth.
    pub recursion_depth: u16,
}

/// Vega existing-credential predicate statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VegaExistingCredentialStatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Credential issuer identifier.
    pub issuer_id: PrivacyIssuerIdV1,
    /// Exact credential schema identifier.
    pub schema_id: PrivacyCredentialSchemaIdV1,
    /// Governed predicate identifier.
    pub predicate_id: PrivacyPredicateIdV1,
    /// Wallet or identity commitment to which the showing is bound.
    pub subject_binding: PrivacyCommitmentV1,
    /// Canonical issuer credential root.
    pub issuer_root: PrivacyRootV1,
    /// Canonical revocation accumulator root.
    pub revocation_root: PrivacyRootV1,
    /// Credential issuance epoch.
    pub issued_epoch: u64,
    /// Credential expiration epoch, exclusive.
    pub expires_epoch: u64,
    /// Epoch at which the predicate is evaluated.
    pub presentation_epoch: u64,
    /// Unlinkable per-policy presentation nullifier.
    pub presentation_nullifier: PrivacyNullifierV1,
}

/// Native X.509 credential-predicate STARK statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    /// Canonical trust-store root.
    pub trust_store_root: PrivacyRootV1,
    /// Certificate validity start epoch, inclusive.
    pub not_before_epoch: u64,
    /// Certificate validity end epoch, exclusive.
    pub not_after_epoch: u64,
    /// Epoch at which the certificate is validated.
    pub validation_epoch: u64,
    /// Nullifier derived from the certificate serial and policy.
    pub certificate_nullifier: PrivacyNullifierV1,
}

/// Native Jindo batched multilinear-opening statement over Pallas.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IrohaJindoMlePallasD19StatementV1 {
    /// Shared chain and governed-artifact binding.
    pub context: PrivacyStatementContextV1,
    /// Polynomial commitments opened in this batch.
    pub polynomial_commitments: Vec<PrivacyCommitmentV1>,
    /// Fixed 19-coordinate multilinear evaluation point.
    pub evaluation_point: [PrivacyPallasScalarV1; IROHA_JINDO_MLE_DIMENSION_V1],
    /// Claimed evaluations, in polynomial commitment order.
    pub claimed_evaluations: Vec<PrivacyPallasScalarV1>,
    /// Declared batched polynomial count.
    pub polynomial_count: u32,
}

impl IrohaJindoMlePallasD19StatementV1 {
    /// Construct a d=19 Jindo statement and infer its exact batch count.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyStatementValidationError::PayloadLengthOverflow`] if
    /// the commitment count cannot be represented as `u32`.
    pub fn new(
        context: PrivacyStatementContextV1,
        polynomial_commitments: Vec<PrivacyCommitmentV1>,
        evaluation_point: [PrivacyPallasScalarV1; IROHA_JINDO_MLE_DIMENSION_V1],
        claimed_evaluations: Vec<PrivacyPallasScalarV1>,
    ) -> Result<Self, PrivacyStatementValidationError> {
        let polynomial_count = u32::try_from(polynomial_commitments.len())
            .map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)?;
        Ok(Self {
            context,
            polynomial_commitments,
            evaluation_point,
            claimed_evaluations,
            polynomial_count,
        })
    }
}

/// Native Bootle GenISIS SIS-with-hints credential statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    /// Strictly increasing selectively disclosed attribute indices.
    pub disclosed_attribute_indices: Vec<u16>,
}

/// Orchard Halo2 private action statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    /// Spent-note nullifiers.
    pub spend_nullifiers: Vec<PrivacyNullifierV1>,
    /// New note commitments.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Encrypted output notes, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Public signed value balance in atomic units.
    pub value_balance: i128,
    /// Public validation fee in atomic units.
    pub fee: u128,
    /// Last block height at which the action is valid.
    pub expiry_height: u64,
}

/// Monero FCMP++ full-chain-membership transfer statement.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    /// Consumed note nullifiers.
    pub nullifiers: Vec<PrivacyNullifierV1>,
    /// New note commitments.
    pub output_commitments: Vec<PrivacyCommitmentV1>,
    /// Encrypted new notes, in commitment order.
    pub encrypted_outputs: Vec<PrivacyEncryptedOutputV1>,
    /// Public signed value balance in atomic units.
    pub value_balance: i128,
    /// Public validation fee in atomic units.
    pub fee: u128,
    /// Ledger epoch bound into private program execution.
    pub execution_epoch: u64,
}

/// Post-quantum authorization profile required by PQ-MASP v0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    /// Native Iroha ZK-AMS STARK admission statement.
    IrohaZkAmsStarkV0(IrohaZkAmsStarkStatementV1),
    /// Vega existing-credential predicate statement.
    VegaExistingCredentialZkV0(VegaExistingCredentialStatementV1),
    /// Native Iroha P-256 X.509 predicate STARK statement.
    IrohaZkX509StarkP256V0(IrohaZkX509StarkP256StatementV1),
    /// Native Iroha Jindo MLE Pallas d19 statement.
    IrohaJindoMlePallasD19V0(IrohaJindoMlePallasD19StatementV1),
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
            Self::IrohaZkAmsStarkV0(_) => PrivacyProtocolIdV1::IrohaZkAmsStarkV0,
            Self::VegaExistingCredentialZkV0(_) => PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            Self::IrohaZkX509StarkP256V0(_) => PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            Self::IrohaJindoMlePallasD19V0(_) => PrivacyProtocolIdV1::IrohaJindoMlePallasD19V0,
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
            Self::IrohaZkAmsStarkV0(statement) => &statement.context,
            Self::VegaExistingCredentialZkV0(statement) => &statement.context,
            Self::IrohaZkX509StarkP256V0(statement) => &statement.context,
            Self::IrohaJindoMlePallasD19V0(statement) => &statement.context,
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
        self.context().validate()?;
        match self {
            Self::ZkAcePqAuthorizationV0(statement) => validate_zk_ace(statement)?,
            Self::AnonymousPgcKOutOfNV1(statement) => validate_anonymous_pgc(statement, limits)?,
            Self::VeRangeTransparentRangeV1(statement) => validate_verange(statement, limits)?,
            Self::IrohaZkAmsStarkV0(statement) => validate_zk_ams(statement, limits)?,
            Self::VegaExistingCredentialZkV0(statement) => validate_vega(statement)?,
            Self::IrohaZkX509StarkP256V0(statement) => validate_zk_x509(statement)?,
            Self::IrohaJindoMlePallasD19V0(statement) => validate_jindo(statement, limits)?,
            Self::IrohaBootleGenisisAcStarkV0(statement) => validate_sis_hints(statement)?,
            Self::OrchardHalo2ActionsV1(statement) => {
                validate_orchard(statement, limits)?
            }
            Self::MoneroFcmpPlusPlusV1(statement) => validate_fcmp(statement, limits)?,
            Self::IrohaIvmPrivateNoteStarkV1(statement) => {
                validate_ivm_private_note(statement, limits)?
            }
            Self::PqMaspStarkV0(statement) => validate_pq_masp(statement, limits)?,
        }
        let encoded = norito::to_bytes(self)
            .map_err(|_| PrivacyStatementValidationError::EncodingFailure)?;
        let bytes = u64::try_from(encoded.len())
            .map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)?;
        if bytes
            > u64::from(
                limits.max_statement_and_encrypted_output_bytes_per_transaction,
            )
        {
            return Err(
                PrivacyStatementValidationError::StatementAndEncryptedOutputsTooLarge {
                    bytes,
                    max: limits
                        .max_statement_and_encrypted_output_bytes_per_transaction,
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
    require_epoch(statement.authorization_epoch, PrivacyEpochFieldV1::Authorization)?;
    require_nullifier(statement.replay_nullifier, 0)
}

fn validate_anonymous_pgc(
    statement: &AnonymousPgcKOutOfNStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    require_nonzero_id(
        statement.anonymity_root.is_zero(),
        PrivacyTypedFieldV1::Root,
    )?;
    require_epoch(statement.root_epoch, PrivacyEpochFieldV1::Root)?;
    require_commitment(statement.sender_account_commitment, 0)?;
    require_nonzero_id(
        statement.receiver_set_root.is_zero(),
        PrivacyTypedFieldV1::ReceiverSetRoot,
    )?;
    if statement.receiver_count < 2
        || statement.receiver_count > ANONYMOUS_PGC_MAX_RECEIVERS_V1
    {
        return Err(PrivacyStatementValidationError::InvalidReceiverCount {
            count: statement.receiver_count,
            max: ANONYMOUS_PGC_MAX_RECEIVERS_V1,
        });
    }
    if statement.threshold == 0 || statement.threshold > statement.receiver_count {
        return Err(PrivacyStatementValidationError::InvalidThreshold {
            threshold: statement.threshold,
            receiver_count: statement.receiver_count,
        });
    }
    require_commitment(statement.amount_commitment, 1)?;
    require_commitment(statement.fee_commitment, 2)?;
    require_nullifier(statement.link_tag, 0)?;
    validate_commitments(&statement.output_commitments, true, limits)?;
    require_count(
        statement.output_commitments.len(),
        statement.receiver_count,
        PrivacyCountFieldV1::ReceiverOutputs,
    )?;
    validate_encrypted_outputs(
        &statement.encrypted_outputs,
        &statement.output_commitments,
        true,
        limits,
    )
}

fn validate_verange(
    statement: &VeRangeTransparentRangeStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.policy_id.is_zero(), PrivacyTypedFieldV1::PolicyId)?;
    if statement.minimum >= statement.maximum {
        return Err(PrivacyStatementValidationError::InvalidRange {
            minimum: statement.minimum,
            maximum: statement.maximum,
        });
    }
    if !matches!(statement.bit_length, 8 | 16 | 32 | 64 | 128) {
        return Err(PrivacyStatementValidationError::InvalidBitLength {
            bit_length: statement.bit_length,
        });
    }
    if statement.bit_length < 128
        && statement.maximum > (1_u128 << u32::from(statement.bit_length))
    {
        return Err(PrivacyStatementValidationError::RangeExceedsBitLength {
            maximum: statement.maximum,
            bit_length: statement.bit_length,
        });
    }
    if statement.aggregation_count == 0
        || statement.aggregation_count > VERANGE_MAX_AGGREGATION_COUNT_V1
    {
        return Err(PrivacyStatementValidationError::InvalidAggregationCount {
            count: statement.aggregation_count,
            max: VERANGE_MAX_AGGREGATION_COUNT_V1,
        });
    }
    validate_commitments(&statement.value_commitments, true, limits)?;
    require_count(
        statement.value_commitments.len(),
        statement.aggregation_count,
        PrivacyCountFieldV1::AggregatedCommitments,
    )
}

fn validate_zk_ams(
    statement: &IrohaZkAmsStarkStatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.issuer_id.is_zero(), PrivacyTypedFieldV1::IssuerId)?;
    require_nonzero_id(statement.pool_id.is_zero(), PrivacyTypedFieldV1::PoolId)?;
    require_nonzero_id(statement.issuer_root.is_zero(), PrivacyTypedFieldV1::Root)?;
    require_nonzero_id(statement.policy_id.is_zero(), PrivacyTypedFieldV1::PolicyId)?;
    require_epoch(statement.issuer_epoch, PrivacyEpochFieldV1::Issuer)?;
    if statement.batch_size == 0 || statement.batch_size > ZK_AMS_MAX_BATCH_SIZE_V1 {
        return Err(PrivacyStatementValidationError::InvalidBatchSize {
            count: statement.batch_size,
            max: ZK_AMS_MAX_BATCH_SIZE_V1,
        });
    }
    if statement.recursion_depth == 0
        || statement.recursion_depth > ZK_AMS_MAX_RECURSION_DEPTH_V1
    {
        return Err(PrivacyStatementValidationError::InvalidRecursionDepth {
            depth: statement.recursion_depth,
            max: ZK_AMS_MAX_RECURSION_DEPTH_V1,
        });
    }
    validate_nullifiers(&statement.admission_nullifiers, true, limits)?;
    validate_commitments(&statement.account_commitments, true, limits)?;
    require_count(
        statement.admission_nullifiers.len(),
        statement.batch_size,
        PrivacyCountFieldV1::AdmissionNullifiers,
    )?;
    require_count(
        statement.account_commitments.len(),
        statement.batch_size,
        PrivacyCountFieldV1::AdmittedAccounts,
    )
}

fn validate_vega(
    statement: &VegaExistingCredentialStatementV1,
) -> Result<(), PrivacyStatementValidationError> {
    require_nonzero_id(statement.issuer_id.is_zero(), PrivacyTypedFieldV1::IssuerId)?;
    require_nonzero_id(statement.schema_id.is_zero(), PrivacyTypedFieldV1::SchemaId)?;
    require_nonzero_id(
        statement.predicate_id.is_zero(),
        PrivacyTypedFieldV1::PredicateId,
    )?;
    require_commitment(statement.subject_binding, 0)?;
    require_nonzero_id(statement.issuer_root.is_zero(), PrivacyTypedFieldV1::Root)?;
    require_nonzero_id(
        statement.revocation_root.is_zero(),
        PrivacyTypedFieldV1::RevocationRoot,
    )?;
    validate_validity_epochs(
        statement.issued_epoch,
        statement.expires_epoch,
        statement.presentation_epoch,
    )?;
    require_nullifier(statement.presentation_nullifier, 0)
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
        statement.trust_store_root.is_zero(),
        PrivacyTypedFieldV1::Root,
    )?;
    validate_validity_epochs(
        statement.not_before_epoch,
        statement.not_after_epoch,
        statement.validation_epoch,
    )?;
    require_nullifier(statement.certificate_nullifier, 0)
}

fn validate_jindo(
    statement: &IrohaJindoMlePallasD19StatementV1,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    if statement.polynomial_count == 0
        || statement.polynomial_count > IROHA_JINDO_MAX_BATCH_SIZE_V1
    {
        return Err(PrivacyStatementValidationError::InvalidBatchSize {
            count: statement.polynomial_count,
            max: IROHA_JINDO_MAX_BATCH_SIZE_V1,
        });
    }
    validate_commitments(&statement.polynomial_commitments, true, limits)?;
    require_count(
        statement.polynomial_commitments.len(),
        statement.polynomial_count,
        PrivacyCountFieldV1::JindoPolynomialCommitments,
    )?;
    require_count(
        statement.claimed_evaluations.len(),
        statement.polynomial_count,
        PrivacyCountFieldV1::JindoClaimedEvaluations,
    )?;
    for (index, scalar) in statement.evaluation_point.iter().enumerate() {
        if !scalar.is_canonical() {
            return Err(PrivacyStatementValidationError::NonCanonicalPallasScalar {
                field: PrivacyPallasScalarFieldV1::EvaluationPoint,
                index: u32_index(index)?,
            });
        }
    }
    for (index, scalar) in statement.claimed_evaluations.iter().enumerate() {
        if !scalar.is_canonical() {
            return Err(PrivacyStatementValidationError::NonCanonicalPallasScalar {
                field: PrivacyPallasScalarFieldV1::ClaimedEvaluation,
                index: u32_index(index)?,
            });
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
    if statement.attribute_count == 0
        || statement.attribute_count > SIS_WITH_HINTS_MAX_ATTRIBUTE_COUNT_V1
    {
        return Err(PrivacyStatementValidationError::InvalidAttributeCount {
            count: statement.attribute_count,
            max: SIS_WITH_HINTS_MAX_ATTRIBUTE_COUNT_V1,
        });
    }
    let disclosed_count = u32::try_from(statement.disclosed_attribute_indices.len())
        .map_err(|_| PrivacyStatementValidationError::PayloadLengthOverflow)?;
    if disclosed_count > SIS_WITH_HINTS_MAX_DISCLOSED_ATTRIBUTES_V1 {
        return Err(PrivacyStatementValidationError::TooManyDisclosedAttributes {
            count: disclosed_count,
            max: SIS_WITH_HINTS_MAX_DISCLOSED_ATTRIBUTES_V1,
        });
    }
    let mut previous = None;
    for &index in &statement.disclosed_attribute_indices {
        if u32::from(index) >= statement.attribute_count {
            return Err(PrivacyStatementValidationError::DisclosedAttributeOutOfBounds {
                index,
                attribute_count: statement.attribute_count,
            });
        }
        if previous.is_some_and(|value| index <= value) {
            return Err(
                PrivacyStatementValidationError::DisclosedAttributesNotStrictlyIncreasing,
            );
        }
        previous = Some(index);
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
    validate_nullifiers(&statement.spend_nullifiers, true, limits)?;
    validate_commitments(&statement.output_commitments, true, limits)?;
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
    validate_commitments(&statement.input_commitments, true, limits)?;
    validate_nullifiers(&statement.link_tags, true, limits)?;
    if statement.input_commitments.len() != statement.link_tags.len() {
        return Err(PrivacyStatementValidationError::InputLinkTagCountMismatch {
            inputs: u32_len(statement.input_commitments.len())?,
            link_tags: u32_len(statement.link_tags.len())?,
        });
    }
    validate_commitments(&statement.output_commitments, true, limits)?;
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
    require_nonzero_id(statement.program_id.is_zero(), PrivacyTypedFieldV1::ProgramId)?;
    require_nonzero_id(statement.state_root.is_zero(), PrivacyTypedFieldV1::Root)?;
    require_epoch(statement.root_epoch, PrivacyEpochFieldV1::Root)?;
    require_epoch(statement.execution_epoch, PrivacyEpochFieldV1::Execution)?;
    validate_nullifiers(&statement.nullifiers, true, limits)?;
    validate_commitments(&statement.output_commitments, true, limits)?;
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
    validate_nullifiers(&statement.nullifiers, true, limits)?;
    validate_commitments(&statement.output_commitments, true, limits)?;
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

fn validate_nullifiers(
    values: &[PrivacyNullifierV1],
    require_nonempty: bool,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    if require_nonempty && values.is_empty() {
        return Err(PrivacyStatementValidationError::MissingNullifier);
    }
    let count = u32_len(values.len())?;
    if count > limits.max_nullifiers_per_action {
        return Err(PrivacyStatementValidationError::TooManyNullifiers {
            count,
            max: limits.max_nullifiers_per_action,
        });
    }
    for (index, value) in values.iter().copied().enumerate() {
        require_nullifier(value, u32_index(index)?)?;
    }
    if first_duplicate_index(values).is_some() {
        return Err(PrivacyStatementValidationError::DuplicateNullifier);
    }
    Ok(())
}

fn validate_commitments(
    values: &[PrivacyCommitmentV1],
    require_nonempty: bool,
    limits: &PrivacyConsensusLimitsV1,
) -> Result<(), PrivacyStatementValidationError> {
    if require_nonempty && values.is_empty() {
        return Err(PrivacyStatementValidationError::MissingCommitment);
    }
    let count = u32_len(values.len())?;
    if count > limits.max_commitments_per_action {
        return Err(PrivacyStatementValidationError::TooManyCommitments {
            count,
            max: limits.max_commitments_per_action,
        });
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
    for (index, (output, expected_commitment)) in
        outputs.iter().zip(commitments).enumerate()
    {
        let index = u32_index(index)?;
        if output.recipient.is_zero() {
            return Err(PrivacyStatementValidationError::ZeroEncryptedOutputRecipient {
                index,
            });
        }
        if output.ephemeral_public_key.is_zero() {
            return Err(
                PrivacyStatementValidationError::ZeroEncryptedOutputEphemeralKey { index },
            );
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

/// Validated raw proof payload for a protocol-specific proof variant.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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

/// Protocol-typed native proof payload.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
    /// Native Iroha ZK-AMS STARK admission proof.
    IrohaZkAmsStarkV0(PrivacyProofBytesV1),
    /// Vega existing-credential predicate proof.
    VegaExistingCredentialZkV0(PrivacyProofBytesV1),
    /// Native Iroha P-256 X.509 predicate STARK proof.
    IrohaZkX509StarkP256V0(PrivacyProofBytesV1),
    /// Native Iroha Jindo MLE Pallas d19 proof.
    IrohaJindoMlePallasD19V0(PrivacyProofBytesV1),
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
            Self::IrohaZkAmsStarkV0(_) => PrivacyProtocolIdV1::IrohaZkAmsStarkV0,
            Self::VegaExistingCredentialZkV0(_) => PrivacyProtocolIdV1::VegaExistingCredentialZkV0,
            Self::IrohaZkX509StarkP256V0(_) => PrivacyProtocolIdV1::IrohaZkX509StarkP256V0,
            Self::IrohaJindoMlePallasD19V0(_) => PrivacyProtocolIdV1::IrohaJindoMlePallasD19V0,
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
            | Self::IrohaZkAmsStarkV0(bytes)
            | Self::VegaExistingCredentialZkV0(bytes)
            | Self::IrohaZkX509StarkP256V0(bytes)
            | Self::IrohaJindoMlePallasD19V0(bytes)
            | Self::IrohaBootleGenisisAcStarkV0(bytes)
            | Self::OrchardHalo2ActionsV1(bytes)
            | Self::MoneroFcmpPlusPlusV1(bytes)
            | Self::IrohaIvmPrivateNoteStarkV1(bytes)
            | Self::PqMaspStarkV0(bytes) => bytes,
        }
    }
}

/// Validation failure for a [`PrivacyStatementBodyV1`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PrivacyStatementValidationError {
    /// Supplied consensus limits are invalid.
    #[error("privacy statement limits are invalid: {0}")]
    InvalidLimits(PrivacyConsensusLimitsValidationError),
    /// Chain-context digest is zero.
    #[error("privacy statement chain-context digest must be non-zero")]
    ZeroChainContextDigest,
    /// Action digest is zero.
    #[error("privacy statement action digest must be non-zero")]
    ZeroActionDigest,
    /// Public-input payload is empty.
    #[error("privacy statement public inputs must not be empty")]
    EmptyPublicInputs,
    /// Public-input payload is all zero.
    #[error("privacy statement public inputs must not be all zero")]
    AllZeroPublicInputs,
    /// One encrypted output is empty.
    #[error("privacy statement encrypted output {index} must not be empty")]
    EmptyEncryptedOutput {
        /// Zero-based output index.
        index: u32,
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
    /// Two commitments are equal.
    #[error("privacy statement contains a duplicate commitment")]
    DuplicateCommitment,
    /// Public statement and encrypted outputs exceed the transaction budget.
    #[error("privacy statement and encrypted outputs use {bytes} bytes, exceeding maximum {max}")]
    StatementAndEncryptedOutputsTooLarge {
        /// Observed payload bytes.
        bytes: u64,
        /// Configured maximum.
        max: u32,
    },
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
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
        let PrivacyProtocolLifecycleV1::Active {
            state_since_height, ..
        } = activation.lifecycle
        else {
            return Err(PrivacyProofEnvelopeValidationError::ActivationNotActive);
        };
        if current_height < state_since_height {
            return Err(
                PrivacyProofEnvelopeValidationError::ActivationNotEffective {
                    current_height,
                    effective_height: state_since_height,
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
}
