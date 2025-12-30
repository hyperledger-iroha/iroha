//! Sora Name Service data structures for registrar APIs.

use blake3::Hasher;
use derive_more::Display;
use iroha_crypto::PublicKey;
use iroha_primitives::json::Json;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    account::{AccountAddress, AccountId},
    metadata::Metadata,
    name,
};

/// Unique identifier assigned to a top-level suffix (e.g., `.sora`).
pub type SuffixId = u16;

/// Canonical selector payload for SNS names.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NameSelectorV1 {
    /// Selector encoding version (currently `1`).
    pub version: u8,
    /// Registered suffix identifier.
    pub suffix_id: SuffixId,
    /// Lowercase, NFC-normalised label.
    pub label: String,
}

impl NameSelectorV1 {
    /// Current selector version.
    pub const VERSION: u8 = 1;

    /// Construct a selector by canonicalising the provided label.
    ///
    /// # Errors
    ///
    /// Returns [`NameSelectorError::InvalidLabel`] when the label fails the canonicalization
    /// rules (length, character set, or Unicode normalization).
    pub fn new(suffix_id: SuffixId, label: impl Into<String>) -> Result<Self, NameSelectorError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(NameSelectorError::InvalidLabel(
                "label must not be empty".into(),
            ));
        }
        let canonical = name::canonicalize_domain_label(&label).map_err(|err| {
            NameSelectorError::InvalidLabel(format!(
                "failed to canonicalize label: {}",
                err.reason()
            ))
        })?;
        Ok(Self {
            version: Self::VERSION,
            suffix_id,
            label: canonical,
        })
    }

    /// Compute the deterministic name hash used as the registry key.
    #[must_use]
    pub fn name_hash(&self) -> [u8; 32] {
        let mut hasher = Hasher::new();
        hasher.update(&[self.version]);
        hasher.update(&self.suffix_id.to_be_bytes());
        hasher.update(self.label.as_bytes());
        *hasher.finalize().as_bytes()
    }

    /// Borrow the normalised label.
    #[must_use]
    pub fn normalized_label(&self) -> &str {
        &self.label
    }
}

/// Errors raised while constructing [`NameSelectorV1`].
#[derive(Debug, Display, Error)]
pub enum NameSelectorError {
    /// Provided label failed canonicalization or validation.
    #[display("{_0}")]
    InvalidLabel(String),
}

/// Record describing the canonical ownership state of a SNS name.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NameRecordV1 {
    /// Canonical selector for the registered label.
    pub selector: NameSelectorV1,
    /// Deterministic name hash (blake3 of the selector payload).
    pub name_hash: [u8; 32],
    /// Account that currently controls the registration.
    pub owner: AccountId,
    /// Controller descriptors (accounts, resolver templates, or external payloads).
    pub controllers: Vec<NameControllerV1>,
    /// Lifecycle state (active, grace, frozen, etc.).
    pub status: NameStatus,
    /// Pricing tier identifier returned by the registrar.
    pub pricing_class: u8,
    /// Timestamp (milliseconds since UNIX epoch) when the registration was created.
    pub registered_at_ms: u64,
    /// Timestamp when the paid term ends.
    pub expires_at_ms: u64,
    /// Timestamp when the grace window ends.
    pub grace_expires_at_ms: u64,
    /// Timestamp when the redemption window ends.
    pub redemption_expires_at_ms: u64,
    /// Arbitrary registrar metadata (text records, resolver hints, etc.).
    pub metadata: Metadata,
    /// Optional auction state for premium or Dutch reopen flows.
    pub auction: Option<NameAuctionStateV1>,
}

impl NameRecordV1 {
    /// Construct a new record with the supplied parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        selector: NameSelectorV1,
        owner: AccountId,
        controllers: Vec<NameControllerV1>,
        pricing_class: u8,
        registered_at_ms: u64,
        expires_at_ms: u64,
        grace_expires_at_ms: u64,
        redemption_expires_at_ms: u64,
        metadata: Metadata,
    ) -> Self {
        let name_hash = selector.name_hash();
        Self {
            selector,
            name_hash,
            owner,
            controllers,
            status: NameStatus::Active,
            pricing_class,
            registered_at_ms,
            expires_at_ms,
            grace_expires_at_ms,
            redemption_expires_at_ms,
            metadata,
            auction: None,
        }
    }
}

/// Lifecycle state of a SNS registration.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "detail", no_fast_from_json)
)]
pub enum NameStatus {
    /// Registration is active and fully paid.
    Active,
    /// Grace period after expiry (still serves traffic).
    GracePeriod,
    /// Redemption period after grace expiration (requires penalty payment).
    Redemption,
    /// Registration is frozen until the supplied timestamp.
    Frozen(NameFrozenStateV1),
    /// Registration has been permanently retired.
    Tombstoned(NameTombstoneStateV1),
}

/// Details captured when a registration is frozen.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NameFrozenStateV1 {
    /// Reason recorded by governance/guardian.
    pub reason: String,
    /// Timestamp (ms) when the freeze lifts automatically.
    pub until_ms: u64,
}

/// Details captured when a registration is tombstoned.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NameTombstoneStateV1 {
    /// Reason recorded by governance/guardian.
    pub reason: String,
}

/// Canonical representation of token amounts used by SNS pricing.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TokenValue {
    /// Settlement asset identifier (e.g., `xor#sora`).
    pub asset_id: String,
    /// Amount expressed in the asset's native units.
    pub amount: u128,
}

impl TokenValue {
    /// Construct a new token amount.
    #[must_use]
    pub fn new(asset_id: impl Into<String>, amount: u128) -> Self {
        Self {
            asset_id: asset_id.into(),
            amount,
        }
    }
}

/// Auction metadata recorded for pending or running auctions.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NameAuctionStateV1 {
    /// Auction flavour.
    pub kind: AuctionKind,
    /// Timestamp when bidding opened.
    pub opened_at_ms: u64,
    /// Timestamp when bidding closes.
    pub closes_at_ms: u64,
    /// Minimum acceptable price.
    pub floor_price: TokenValue,
    /// Highest sealed bid hash (commitment reference).
    pub highest_commitment: Option<[u8; 32]>,
    /// Settlement transaction hash once the auction closes.
    pub settlement_tx: Option<Json>,
}

/// Supported auction kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", no_fast_from_json)
)]
pub enum AuctionKind {
    /// 72h commit / 24h reveal sealed-bid auction.
    VickreyCommitReveal,
    /// Dutch reopen (descending price) for expired names.
    DutchReopen,
}

/// Controller descriptor referencing account addresses or resolver templates.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct NameControllerV1 {
    /// Controller classification.
    pub controller_type: ControllerType,
    /// Optional canonical account address bound to the controller.
    pub account_address: Option<AccountAddress>,
    /// Optional resolver template identifier.
    pub resolver_template_id: Option<String>,
    /// Free-form metadata payload describing UX hints.
    pub payload: Metadata,
}

impl NameControllerV1 {
    /// Convenience helper for single-account controllers.
    pub fn account(address: &AccountAddress) -> Self {
        Self {
            controller_type: ControllerType::Account,
            account_address: Some(address.clone()),
            resolver_template_id: None,
            payload: Metadata::default(),
        }
    }
}

/// Supported controller categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "detail", no_fast_from_json)
)]
pub enum ControllerType {
    /// Single account address controller.
    Account,
    /// Multisig controller (see `AccountController::Multisig`).
    Multisig,
    /// Resolver template (pre-baked DNS/SoraFS template).
    ResolverTemplate,
    /// External link / application-defined payload.
    ExternalLink,
}

/// Registrar request for creating a new name.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RegisterNameRequestV1 {
    /// Selector describing the requested label + suffix.
    pub selector: NameSelectorV1,
    /// Owner account.
    pub owner: AccountId,
    /// Initial controller list.
    pub controllers: Vec<NameControllerV1>,
    /// Registration term in years.
    pub term_years: u8,
    /// Optional steward-advertised pricing class identifier.
    pub pricing_class_hint: Option<u8>,
    /// Payment evidence submitted alongside the request.
    pub payment: PaymentProofV1,
    /// Governance evidence (council vote hashes, steward acknowledgement, etc.).
    pub governance: Option<GovernanceHookV1>,
    /// Arbitrary metadata.
    pub metadata: Metadata,
}

/// Response emitted after a successful registration.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RegisterNameResponseV1 {
    /// Canonical name record returned by the registrar.
    pub name_record: NameRecordV1,
}

/// Renewal request body.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RenewNameRequestV1 {
    /// Additional term to purchase (years).
    pub term_years: u8,
    /// Payment evidence (covers base + surcharges).
    pub payment: PaymentProofV1,
}

/// Transfer request body.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TransferNameRequestV1 {
    /// New owner account identifier.
    pub new_owner: AccountId,
    /// Governance evidence proving the transfer was approved.
    pub governance: GovernanceHookV1,
}

/// Controller update request body.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct UpdateControllersRequestV1 {
    /// Replacement controller set.
    pub controllers: Vec<NameControllerV1>,
}

/// Freeze request body (guardian-issued).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FreezeNameRequestV1 {
    /// Reason captured in the freeze log.
    pub reason: String,
    /// Timestamp (ms) when the freeze should auto-expire.
    pub until_ms: u64,
    /// Guardian ticket signature proving the freeze approval (base64-encoded JSON).
    pub guardian_ticket: Json,
}

/// Reserved label assignment body.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReservedAssignmentRequestV1 {
    /// Intended owner account.
    pub owner: AccountId,
    /// Governance evidence referencing the reserved label decision.
    pub governance: GovernanceHookV1,
    /// Additional metadata captured with the assignment.
    pub metadata: Metadata,
}

/// Payment evidence submitted alongside registrar operations.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PaymentProofV1 {
    /// Asset identifier used for settlement (e.g., `xor#sora`).
    pub asset_id: String,
    /// Gross amount paid (base price + surcharges) expressed in the asset's native units.
    pub gross_amount: u64,
    /// Net amount forwarded to the registry (after referral rebates).
    pub net_amount: u64,
    /// Settlement transaction hash (canonical hex string or JSON hash encoding).
    pub settlement_tx: Json,
    /// Account that authorised the payment.
    pub payer: AccountId,
    /// Steward or treasury signature attesting to the settlement (base64-encoded JSON).
    pub signature: Json,
}

/// Governance evidence wrapper.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct GovernanceHookV1 {
    /// Governance proposal identifier.
    pub proposal_id: String,
    /// Council vote hash (hex string/JSON blob).
    pub council_vote_hash: Json,
    /// DAO vote hash (hex string/JSON blob).
    pub dao_vote_hash: Json,
    /// Steward acknowledgement signature.
    pub steward_ack: Json,
    /// Optional guardian clearance signature.
    pub guardian_clearance: Option<Json>,
}

/// Steward-advertised pricing tier definition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PriceTierV1 {
    /// Tier identifier.
    pub tier_id: u8,
    /// RE2-compatible regex describing eligible labels.
    pub label_regex: String,
    /// Base one-year price used for calculations.
    pub base_price: TokenValue,
    /// Auction mode triggered by this tier.
    pub auction_kind: AuctionKind,
    /// Optional Dutch floor when `auction_kind` = Dutch.
    pub dutch_floor: Option<TokenValue>,
    /// Minimum purchase duration allowed by this tier.
    pub min_duration_years: u8,
    /// Maximum purchase duration allowed by this tier.
    pub max_duration_years: u8,
}

/// Reserved label assignment controlled by governance.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ReservedNameV1 {
    /// Canonical lowercase label.
    pub normalized_label: String,
    /// Optional owner assigned by governance.
    pub assigned_to: Option<AccountId>,
    /// Optional release timestamp (ms since epoch).
    pub release_at_ms: Option<u64>,
    /// Notes captured with the reservation.
    pub note: String,
}

/// Basis-point split describing how funds are routed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SuffixFeeSplitV1 {
    /// Treasury share (basis points).
    pub treasury_bps: u16,
    /// Steward share (basis points).
    pub steward_bps: u16,
    /// Referral carve-out ceiling (basis points).
    pub referral_max_bps: u16,
    /// Escrow allocation (basis points).
    pub escrow_bps: u16,
}

/// Policy lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(tag = "status", content = "detail", no_fast_from_json)
)]
pub enum SuffixStatus {
    /// Policy active and available for registration.
    Active,
    /// Temporarily paused (maintenance or governance review).
    Paused,
    /// Permanently revoked.
    Revoked,
}

/// Minimal suffix policy definition consumed by the registrar.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SuffixPolicyV1 {
    /// Assigned identifier.
    pub suffix_id: SuffixId,
    /// Human-readable suffix (without leading dot).
    pub suffix: String,
    /// Steward account responsible for operations.
    pub steward: AccountId,
    /// Current lifecycle status.
    pub status: SuffixStatus,
    /// Minimum term that can be purchased.
    pub min_term_years: u8,
    /// Maximum term that can be purchased.
    pub max_term_years: u8,
    /// Grace window (days).
    pub grace_period_days: u16,
    /// Redemption window (days).
    pub redemption_period_days: u16,
    /// Referral ceiling expressed in basis points.
    pub referral_cap_bps: u16,
    /// Reserved labels enforced by governance.
    pub reserved_labels: Vec<ReservedNameV1>,
    /// Asset required for settlement (e.g., `xor#sora`).
    pub payment_asset_id: String,
    /// Pricing tiers advertised by the steward.
    pub pricing: Vec<PriceTierV1>,
    /// Treasury/steward/referral split.
    pub fee_split: SuffixFeeSplitV1,
    /// Escrow account used for fund distribution.
    pub fund_splitter_account: AccountId,
    /// Policy version (increments on change).
    pub policy_version: u16,
    /// Free-form metadata (KPI covenants, annex references).
    pub metadata: Metadata,
}

impl SuffixPolicyV1 {
    /// Returns the canonical lowercase suffix string.
    #[must_use]
    pub fn suffix_key(&self) -> String {
        self.suffix.to_lowercase()
    }
}

/// Convenience helpers for building example payloads inside docs/tests.
pub mod fixtures {
    use super::*;

    /// Deterministic steward account used in fixtures/tests.
    pub fn steward_account() -> AccountId {
        // Use a known-valid ed25519 point so parsing is stable in tests.
        let key: PublicKey =
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
                .parse()
                .expect("valid steward key literal");
        AccountId::new("sns".parse().expect("domain parses"), key)
    }

    /// Minimal suffix policy covering `.sora` for unit tests and mock environments.
    pub fn default_policy() -> SuffixPolicyV1 {
        let steward = steward_account();
        SuffixPolicyV1 {
            suffix_id: 0x0001,
            suffix: "sora".to_string(),
            steward: steward.clone(),
            status: SuffixStatus::Active,
            min_term_years: 1,
            max_term_years: 5,
            grace_period_days: 30,
            redemption_period_days: 60,
            referral_cap_bps: 500,
            reserved_labels: vec![ReservedNameV1 {
                normalized_label: "treasury".to_string(),
                assigned_to: Some(steward.clone()),
                release_at_ms: None,
                note: "Protocol reserved".to_string(),
            }],
            payment_asset_id: "xor#sora".to_string(),
            pricing: vec![PriceTierV1 {
                tier_id: 0,
                label_regex: "^[a-z0-9]{3,}$".to_string(),
                base_price: TokenValue::new("xor#sora", 120),
                auction_kind: AuctionKind::VickreyCommitReveal,
                dutch_floor: None,
                min_duration_years: 1,
                max_duration_years: 5,
            }],
            fee_split: SuffixFeeSplitV1 {
                treasury_bps: 7000,
                steward_bps: 3000,
                referral_max_bps: 1000,
                escrow_bps: 0,
            },
            fund_splitter_account: steward,
            policy_version: 1,
            metadata: Metadata::default(),
        }
    }
}

/// Re-export commonly used SNS types.
pub mod prelude {
    pub use super::{
        AuctionKind, ControllerType, FreezeNameRequestV1, GovernanceHookV1, NameAuctionStateV1,
        NameControllerV1, NameRecordV1, NameSelectorError, NameSelectorV1, NameStatus,
        PaymentProofV1, PriceTierV1, RegisterNameRequestV1, RegisterNameResponseV1,
        RenewNameRequestV1, ReservedAssignmentRequestV1, ReservedNameV1, SuffixFeeSplitV1,
        SuffixId, SuffixPolicyV1, SuffixStatus, TokenValue, TransferNameRequestV1,
        UpdateControllersRequestV1, fixtures,
    };
}

#[cfg(test)]
mod tests {
    use super::{TokenValue, fixtures};

    #[test]
    fn token_value_new_assigns_fields() {
        let value = TokenValue::new("xor#sora", 120);
        assert_eq!(value.asset_id, "xor#sora");
        assert_eq!(value.amount, 120);
    }

    #[test]
    fn default_policy_exposes_pricing_and_reserved_names() {
        let policy = fixtures::default_policy();
        assert!(!policy.pricing.is_empty(), "expected pricing tiers");
        assert!(
            !policy.reserved_labels.is_empty(),
            "expected reserved labels"
        );
        assert_eq!(policy.status, super::SuffixStatus::Active);
    }
}
