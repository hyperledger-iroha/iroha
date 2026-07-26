//! Bounded, non-delegable authorization for native asset transfers.

use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, asset::AssetId, smart_contract::ContractAddress};

/// Domain separator used when deriving capability identifiers.
pub const ASSET_TRANSFER_CAPABILITY_ID_DOMAIN_V1: &[u8] = b"iroha:asset-transfer-capability:id:v1:";

/// Consensus limit for a single capability's execution budget.
///
/// A hard upper bound keeps malicious records from representing unbounded authority while
/// allowing long-lived institutional mandates to avoid middleware-side renewal races.
pub const MAX_ASSET_TRANSFER_CAPABILITY_USES_V1: u32 = 1_000_000;

/// Maximum UTF-8 byte length of a contract entrypoint bound into a capability.
pub const MAX_ASSET_TRANSFER_CAPABILITY_ENTRYPOINT_BYTES_V1: usize = 128;

/// Stable identifier of an asset-transfer capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetTransferCapabilityIdV1(pub Hash);

impl AssetTransferCapabilityIdV1 {
    /// Construct an identifier from its consensus hash.
    #[must_use]
    pub const fn new(hash: Hash) -> Self {
        Self(hash)
    }

    /// Return the consensus hash.
    #[must_use]
    pub const fn as_hash(&self) -> &Hash {
        &self.0
    }
}

/// Optional smart-contract execution scope for a capability.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetTransferCapabilityContractScopeV1 {
    /// Exact active contract address.
    pub contract_address: ContractAddress,
    /// Exact code hash activated at registration and execution.
    pub code_hash: Hash,
    /// Exact entrypoint allowed to consume the capability.
    pub entrypoint: String,
}

/// Immutable intent from which a capability id and ledger record are derived.
///
/// `nonce` is selected by the grantor. It permits multiple otherwise-identical authorizations
/// without making the id depend on transaction ordering or wall-clock time.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetTransferCapabilityIntentV1 {
    /// Source owner granting the authority.
    pub grantor: AccountId,
    /// Non-delegable account permitted to consume it.
    pub delegate: AccountId,
    /// Exact source asset, including source account and balance scope.
    pub source: AssetId,
    /// Exact destination account.
    pub destination: AccountId,
    /// Exact amount transferred by each use.
    pub amount_per_use: Quantity,
    /// Exact evidence digest required on every execution.
    pub evidence_digest: Hash,
    /// Inclusive Unix timestamp in milliseconds at which execution becomes valid.
    pub valid_from_ms: u64,
    /// Exclusive Unix timestamp in milliseconds at which execution expires.
    pub expires_at_ms: u64,
    /// Number of authorized executions.
    pub initial_uses: u32,
    /// Optional contract address, code, and entrypoint binding.
    pub contract_scope: Option<AssetTransferCapabilityContractScopeV1>,
    /// Grantor-selected uniqueness value included in the id commitment.
    pub nonce: u64,
}

impl AssetTransferCapabilityIntentV1 {
    /// Derive the domain-separated, consensus-stable identifier for this exact intent.
    #[must_use]
    pub fn id(&self) -> AssetTransferCapabilityIdV1 {
        let encoded = self.encode();
        let mut preimage =
            Vec::with_capacity(ASSET_TRANSFER_CAPABILITY_ID_DOMAIN_V1.len() + encoded.len());
        preimage.extend_from_slice(ASSET_TRANSFER_CAPABILITY_ID_DOMAIN_V1);
        preimage.extend_from_slice(&encoded);
        AssetTransferCapabilityIdV1(Hash::new(preimage))
    }

    /// Validate context-free consensus invariants.
    ///
    /// State-dependent checks (source ownership, account existence, active contract binding,
    /// and duplicate ids) are deliberately performed by Core at registration.
    pub fn validate(&self) -> Result<(), AssetTransferCapabilityValidationErrorV1> {
        if self.source.account() != &self.grantor {
            return Err(AssetTransferCapabilityValidationErrorV1::GrantorIsNotSourceOwner);
        }
        if self.amount_per_use.is_zero() {
            return Err(AssetTransferCapabilityValidationErrorV1::ZeroAmount);
        }
        if self.initial_uses == 0 {
            return Err(AssetTransferCapabilityValidationErrorV1::ZeroUses);
        }
        if self.initial_uses > MAX_ASSET_TRANSFER_CAPABILITY_USES_V1 {
            return Err(AssetTransferCapabilityValidationErrorV1::TooManyUses);
        }
        if self.valid_from_ms >= self.expires_at_ms {
            return Err(AssetTransferCapabilityValidationErrorV1::InvalidTimeWindow);
        }
        if let Some(scope) = &self.contract_scope {
            let entrypoint_len = scope.entrypoint.len();
            if entrypoint_len == 0
                || entrypoint_len > MAX_ASSET_TRANSFER_CAPABILITY_ENTRYPOINT_BYTES_V1
            {
                return Err(AssetTransferCapabilityValidationErrorV1::InvalidEntrypoint);
            }
        }
        Ok(())
    }
}

/// Context-free capability validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetTransferCapabilityValidationErrorV1 {
    /// The source asset is not owned by the declared grantor.
    GrantorIsNotSourceOwner,
    /// Per-use amount is zero.
    ZeroAmount,
    /// Execution budget is zero.
    ZeroUses,
    /// Execution budget exceeds the consensus bound.
    TooManyUses,
    /// `valid_from_ms` is not strictly before `expires_at_ms`.
    InvalidTimeWindow,
    /// A contract-scoped entrypoint is empty or too long.
    InvalidEntrypoint,
}

impl core::fmt::Display for AssetTransferCapabilityValidationErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::GrantorIsNotSourceOwner => "capability grantor must own the exact source asset",
            Self::ZeroAmount => "capability amount per use must be non-zero",
            Self::ZeroUses => "capability initial uses must be non-zero",
            Self::TooManyUses => "capability initial uses exceed the consensus limit",
            Self::InvalidTimeWindow => "capability valid-from timestamp must precede its expiry",
            Self::InvalidEntrypoint => {
                "capability contract entrypoint must be non-empty and within the byte limit"
            }
        })
    }
}

impl std::error::Error for AssetTransferCapabilityValidationErrorV1 {}

/// Lifecycle status of an asset-transfer capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "status", content = "value"))]
#[repr(u8)]
pub enum AssetTransferCapabilityStatusV1 {
    /// The capability may be consumed.
    Active,
    /// The source owner revoked the capability.
    Revoked,
    /// All authorized uses were consumed.
    Consumed,
}

/// Consensus record for an exact bounded asset-transfer authorization.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetTransferCapabilityV1 {
    /// Stable record identifier.
    pub id: AssetTransferCapabilityIdV1,
    /// Source owner that granted the capability.
    pub grantor: AccountId,
    /// Non-delegable account permitted to consume it.
    pub delegate: AccountId,
    /// Exact source asset, including source account and balance scope.
    pub source: AssetId,
    /// Exact destination account.
    pub destination: AccountId,
    /// Exact amount transferred by each use.
    pub amount_per_use: Quantity,
    /// Exact evidence digest required on every execution.
    pub evidence_digest: Hash,
    /// Inclusive Unix timestamp in milliseconds at which execution becomes valid.
    pub valid_from_ms: u64,
    /// Exclusive Unix timestamp in milliseconds at which execution expires.
    pub expires_at_ms: u64,
    /// Initial number of authorized executions.
    pub initial_uses: u32,
    /// Number of executions remaining.
    pub remaining_uses: u32,
    /// Optional contract address, code, and entrypoint binding.
    pub contract_scope: Option<AssetTransferCapabilityContractScopeV1>,
    /// Grantor-selected uniqueness value committed by `id`.
    pub nonce: u64,
    /// Current lifecycle status.
    pub status: AssetTransferCapabilityStatusV1,
    /// Ledger timestamp at registration.
    pub created_at_ms: u64,
    /// Ledger timestamp of the latest lifecycle transition.
    pub updated_at_ms: u64,
}

impl AssetTransferCapabilityV1 {
    /// Materialize the canonical active record for a validated immutable intent.
    #[must_use]
    pub fn from_intent(intent: AssetTransferCapabilityIntentV1, created_at_ms: u64) -> Self {
        let id = intent.id();
        Self {
            id,
            grantor: intent.grantor,
            delegate: intent.delegate,
            source: intent.source,
            destination: intent.destination,
            amount_per_use: intent.amount_per_use,
            evidence_digest: intent.evidence_digest,
            valid_from_ms: intent.valid_from_ms,
            expires_at_ms: intent.expires_at_ms,
            initial_uses: intent.initial_uses,
            remaining_uses: intent.initial_uses,
            contract_scope: intent.contract_scope,
            nonce: intent.nonce,
            status: AssetTransferCapabilityStatusV1::Active,
            created_at_ms,
            updated_at_ms: created_at_ms,
        }
    }

    /// Reconstruct the immutable intent committed by this record.
    #[must_use]
    pub fn intent(&self) -> AssetTransferCapabilityIntentV1 {
        AssetTransferCapabilityIntentV1 {
            grantor: self.grantor.clone(),
            delegate: self.delegate.clone(),
            source: self.source.clone(),
            destination: self.destination.clone(),
            amount_per_use: self.amount_per_use.clone(),
            evidence_digest: self.evidence_digest,
            valid_from_ms: self.valid_from_ms,
            expires_at_ms: self.expires_at_ms,
            initial_uses: self.initial_uses,
            contract_scope: self.contract_scope.clone(),
            nonce: self.nonce,
        }
    }

    /// Validate the record's immutable commitment and mutable lifecycle invariants.
    pub fn validate(&self) -> Result<(), AssetTransferCapabilityRecordErrorV1> {
        self.intent()
            .validate()
            .map_err(AssetTransferCapabilityRecordErrorV1::Intent)?;
        if self.id != self.intent().id() {
            return Err(AssetTransferCapabilityRecordErrorV1::IdentifierMismatch);
        }
        if self.remaining_uses > self.initial_uses {
            return Err(AssetTransferCapabilityRecordErrorV1::RemainingUsesExceedInitial);
        }
        match self.status {
            AssetTransferCapabilityStatusV1::Active if self.remaining_uses == 0 => {
                Err(AssetTransferCapabilityRecordErrorV1::ActiveWithoutUses)
            }
            AssetTransferCapabilityStatusV1::Consumed if self.remaining_uses != 0 => {
                Err(AssetTransferCapabilityRecordErrorV1::ConsumedWithUses)
            }
            _ => Ok(()),
        }
    }
}

/// Capability record validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetTransferCapabilityRecordErrorV1 {
    /// Immutable intent is malformed.
    Intent(AssetTransferCapabilityValidationErrorV1),
    /// Stored id does not commit to the immutable fields and nonce.
    IdentifierMismatch,
    /// Mutable execution budget exceeds its initial bound.
    RemainingUsesExceedInitial,
    /// An active record has no remaining execution budget.
    ActiveWithoutUses,
    /// A consumed record retains execution budget.
    ConsumedWithUses,
}

impl core::fmt::Display for AssetTransferCapabilityRecordErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Intent(error) => write!(formatter, "{error}"),
            Self::IdentifierMismatch => {
                formatter.write_str("capability id does not match its immutable intent")
            }
            Self::RemainingUsesExceedInitial => {
                formatter.write_str("capability remaining uses exceed initial uses")
            }
            Self::ActiveWithoutUses => {
                formatter.write_str("active capability must retain at least one use")
            }
            Self::ConsumedWithUses => {
                formatter.write_str("consumed capability must have zero remaining uses")
            }
        }
    }
}

impl std::error::Error for AssetTransferCapabilityRecordErrorV1 {}

/// Prelude exports for the native capability model.
pub mod prelude {
    pub use super::{
        AssetTransferCapabilityContractScopeV1, AssetTransferCapabilityIdV1,
        AssetTransferCapabilityIntentV1, AssetTransferCapabilityRecordErrorV1,
        AssetTransferCapabilityStatusV1, AssetTransferCapabilityV1,
        AssetTransferCapabilityValidationErrorV1,
        MAX_ASSET_TRANSFER_CAPABILITY_ENTRYPOINT_BYTES_V1, MAX_ASSET_TRANSFER_CAPABILITY_USES_V1,
    };
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{
        asset::{AssetBalanceScope, AssetDefinitionId},
        domain::DomainId,
    };

    fn account(seed: u8) -> AccountId {
        AccountId::new(
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("fixture key")
                .public_key()
                .clone(),
        )
    }

    fn intent(nonce: u64) -> AssetTransferCapabilityIntentV1 {
        let grantor = account(1);
        let definition = AssetDefinitionId::new(
            DomainId::try_new("cbdc", "universal").expect("domain"),
            "ils".parse().expect("name"),
        );
        AssetTransferCapabilityIntentV1 {
            grantor: grantor.clone(),
            delegate: account(2),
            source: AssetId::with_scope(
                definition,
                grantor,
                AssetBalanceScope::Dataspace(crate::nexus::DataSpaceId::UNIVERSAL),
            ),
            destination: account(3),
            amount_per_use: Quantity::from(10_u32),
            evidence_digest: Hash::new(b"court-order"),
            valid_from_ms: 100,
            expires_at_ms: 200,
            initial_uses: 2,
            contract_scope: None,
            nonce,
        }
    }

    #[test]
    fn id_commits_to_exact_intent_and_nonce() {
        let baseline = intent(7);
        let mut changed = baseline.clone();
        changed.destination = account(4);
        assert_ne!(baseline.id(), changed.id());
        assert_ne!(baseline.id(), intent(8).id());
        assert_eq!(baseline.id(), baseline.clone().id());
    }

    #[test]
    fn validation_rejects_unsafe_intent_shapes() {
        let mut value = intent(1);
        value.initial_uses = 0;
        assert_eq!(
            value.validate(),
            Err(AssetTransferCapabilityValidationErrorV1::ZeroUses)
        );

        let mut value = intent(1);
        value.valid_from_ms = value.expires_at_ms;
        assert_eq!(
            value.validate(),
            Err(AssetTransferCapabilityValidationErrorV1::InvalidTimeWindow)
        );

        let mut value = intent(1);
        value.delegate = value.grantor.clone();
        assert!(
            value.validate().is_ok(),
            "self-delegation is a valid exact mandate"
        );
    }

    #[test]
    fn canonical_record_starts_active_with_full_budget() {
        let intent = intent(11);
        let record = AssetTransferCapabilityV1::from_intent(intent.clone(), 120);
        assert_eq!(record.id, intent.id());
        assert_eq!(record.remaining_uses, intent.initial_uses);
        assert_eq!(record.status, AssetTransferCapabilityStatusV1::Active);
        assert_eq!(record.created_at_ms, 120);
        assert_eq!(record.updated_at_ms, 120);
    }
}
