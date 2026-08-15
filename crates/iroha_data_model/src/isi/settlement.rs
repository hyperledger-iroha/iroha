pub use self::model::SettlementId;
use super::*;
use crate::{
    Name, NetworkId,
    block::BlockHeader,
    metadata::Metadata,
    nexus::DataSpaceId,
    oracle::{FeedConfigVersion, FeedEvent, FeedId, FeedSlot, ObservationValue},
    prelude::{AccountId, AssetDefinitionId},
};
use derive_more::{Constructor, Display, FromStr};
use getset::{CopyGetters, Getters};
use iroha_crypto::{Hash, HashOf, derive_non_signing_ed25519_public_key};
use iroha_data_model_derive::model;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::derive::{JsonDeserialize, JsonSerialize};
use std::collections::{BTreeMap, BTreeSet};
const FX_CORRIDOR_ESCROW_ACCOUNT_DOMAIN_V1: &[u8] = b"iroha:fx-corridor:escrow-account:v1";
#[model]
mod model {
    use super::*;
    /// One-shot identifier consumed when a settlement commits successfully.
    #[derive(
        Debug,
        Display,
        FromStr,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Constructor,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{name}")]
    #[getset(get = "pub")]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct SettlementId {
        /// Logical identifier chosen by upstream trade/collateral workflows.
        pub name: Name,
    }
}
string_id!(SettlementId);
enum_type! {
    pub enum SettlementExecutionOrder {
        DeliveryThenPayment,
        PaymentThenDelivery,
    }
}
enum_type! {
    pub enum SettlementAtomicity {
        AllOrNothing,
        CommitFirstLeg,
        CommitSecondLeg,
    }
}
#[allow(clippy::derivable_impls)]
impl Default for SettlementExecutionOrder {
    fn default() -> Self {
        Self::DeliveryThenPayment
    }
}
#[allow(clippy::derivable_impls)]
impl Default for SettlementAtomicity {
    fn default() -> Self {
        Self::AllOrNothing
    }
}
/// Execution plan covering leg ordering and failure handling.
#[derive(
    Copy, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, CopyGetters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[getset(get_copy = "pub")]
pub struct SettlementPlan {
    /// Ordering of the settlement legs.
    order: SettlementExecutionOrder,
    /// Atomicity policy to apply when executing the settlement.
    atomicity: SettlementAtomicity,
}
impl SettlementPlan {
    /// Construct a settlement plan from the desired order and atomicity policy.
    pub const fn new(order: SettlementExecutionOrder, atomicity: SettlementAtomicity) -> Self {
        Self { order, atomicity }
    }
}
impl Default for SettlementPlan {
    fn default() -> Self {
        Self::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        )
    }
}
/// One leg of a bilateral settlement (asset or payment).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[getset(get = "pub")]
pub struct SettlementLeg {
    /// Asset definition exchanged in this leg.
    pub asset_definition_id: AssetDefinitionId,
    /// Quantity of the asset exchanged.
    pub quantity: Quantity,
    /// Account sending the asset.
    pub from: AccountId,
    /// Account receiving the asset.
    pub to: AccountId,
    /// Optional metadata (e.g., ISIN, clearing reference).
    pub metadata: Metadata,
}
/// Immutable identity of one first-release FX corridor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct FxCorridorId {
    /// Stable governed corridor name.
    pub policy_id: Name,
    /// Private dataspace holding the source currency.
    pub source_dataspace: DataSpaceId,
    /// Source-currency asset definition.
    pub source_asset_definition_id: AssetDefinitionId,
    /// Private dataspace holding the destination reserve.
    pub destination_dataspace: DataSpaceId,
    /// Destination-currency asset definition.
    pub destination_asset_definition_id: AssetDefinitionId,
}
/// Exact retained oracle event selected by a signed FX settlement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct FxCorridorOracleEvidence {
    /// Governed feed that produced the rate.
    pub feed_id: FeedId,
    /// Exact current feed configuration version.
    pub feed_config_version: FeedConfigVersion,
    /// Exact aggregated feed slot.
    pub slot: FeedSlot,
    /// Exact upstream request commitment.
    pub request_hash: Hash,
    /// Typed hash of the complete retained feed event.
    pub event_hash: HashOf<FeedEvent>,
}
/// Deterministic fixed-window usage retained for one FX corridor.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct FxCorridorUsage {
    /// Consensus-time-aligned start of the current velocity window.
    pub window_start_ms: u64,
    /// Successful settlements committed in the current window.
    pub settlements: u64,
    /// Total source currency collected in the current window.
    pub source_amount: Quantity,
    /// Total destination currency released in the current window.
    pub destination_amount: Quantity,
}
/// Immutable routing and pricing policy for one native FX corridor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct FxCorridorPolicy {
    /// Stable policy identifier referenced by settlement instructions.
    pub policy_id: Name,
    /// Monotonic policy revision used to bind signed settlement intent.
    pub revision: u64,
    /// Immutable account that funds the isolated destination reserve and receives source funds.
    pub owner: AccountId,
    /// Dataspace holding the source currency balance.
    pub source_dataspace: DataSpaceId,
    /// Source-currency asset definition.
    pub source_asset_definition_id: AssetDefinitionId,
    /// Dataspace holding the destination reserve.
    pub destination_dataspace: DataSpaceId,
    /// Destination-currency asset definition.
    pub destination_asset_definition_id: AssetDefinitionId,
    /// Exact destination FI alias domains accepted for recipients.
    pub allowed_destination_alias_domains: BTreeSet<crate::domain::DomainId>,
    /// Exact governed oracle feed whose latest event supplies the rate.
    pub oracle_feed_id: FeedId,
    /// Maximum consensus-time age accepted for the retained oracle event.
    pub max_oracle_age_ms: u64,
    /// Maximum source amount admitted by one settlement.
    pub max_source_amount_per_settlement: Quantity,
    /// Maximum destination exposure admitted by one settlement.
    pub max_destination_amount_per_settlement: Quantity,
    /// Deterministic consensus-time velocity window length.
    pub velocity_window_ms: u64,
    /// Maximum successful settlements admitted in one velocity window.
    pub max_settlements_per_window: u64,
    /// Maximum aggregate source amount admitted in one velocity window.
    pub max_source_amount_per_window: Quantity,
    /// Maximum aggregate destination amount admitted in one velocity window.
    pub max_destination_amount_per_window: Quantity,
    /// Whether new settlements may use this policy.
    pub enabled: bool,
}
impl FxCorridorPolicy {
    /// Return the first static policy invariant violation, if any.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.revision == 0 {
            return Some("FX corridor policy revision must be non-zero");
        }
        if self.source_dataspace == DataSpaceId::UNIVERSAL
            || self.destination_dataspace == DataSpaceId::UNIVERSAL
        {
            return Some("FX corridor dataspaces must be private");
        }
        if self.source_dataspace == self.destination_dataspace {
            return Some("FX corridor dataspaces must be distinct");
        }
        if self.source_asset_definition_id == self.destination_asset_definition_id {
            return Some("FX corridor assets must be distinct");
        }
        if self.allowed_destination_alias_domains.is_empty() {
            return Some("FX corridor destination alias domains must not be empty");
        }
        if self.max_oracle_age_ms == 0 {
            return Some("FX corridor oracle maximum age must be non-zero");
        }
        if self.max_source_amount_per_settlement.is_zero()
            || self.max_destination_amount_per_settlement.is_zero()
            || self.max_source_amount_per_window.is_zero()
            || self.max_destination_amount_per_window.is_zero()
        {
            return Some("FX corridor exposure and velocity amount limits must be non-zero");
        }
        if self.max_source_amount_per_settlement > self.max_source_amount_per_window
            || self.max_destination_amount_per_settlement > self.max_destination_amount_per_window
        {
            return Some("FX corridor per-settlement exposure cannot exceed its window limit");
        }
        if self.velocity_window_ms == 0 || self.max_settlements_per_window == 0 {
            return Some("FX corridor velocity window and settlement limit must be non-zero");
        }
        None
    }
    /// Return the immutable corridor identity used for protocol escrow derivation.
    #[must_use]
    pub fn corridor_id(&self) -> FxCorridorId {
        FxCorridorId {
            policy_id: self.policy_id.clone(),
            source_dataspace: self.source_dataspace,
            source_asset_definition_id: self.source_asset_definition_id.clone(),
            destination_dataspace: self.destination_dataspace,
            destination_asset_definition_id: self.destination_asset_definition_id.clone(),
        }
    }
}
/// Derive the non-signable reserve account for one exact FX corridor and asset.
#[must_use]
pub fn fx_corridor_escrow_account_id_v1(
    network_id: &NetworkId,
    corridor_id: &FxCorridorId,
    asset_definition_id: &AssetDefinitionId,
) -> AccountId {
    let corridor_id = corridor_id.encode();
    let asset_definition_id = asset_definition_id.encode();
    AccountId::new(derive_non_signing_ed25519_public_key(
        FX_CORRIDOR_ESCROW_ACCOUNT_DOMAIN_V1,
        &[
            network_id.as_bytes(),
            corridor_id.as_slice(),
            asset_definition_id.as_slice(),
        ],
    ))
}
/// Complete governed set of native FX corridor policies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct FxCorridorPolicyRegistry {
    /// Policies keyed by their stable identifier.
    pub policies: BTreeMap<Name, FxCorridorPolicy>,
    /// Bounded O(1)-per-corridor exposure and velocity counters.
    pub usage: BTreeMap<Name, FxCorridorUsage>,
}
impl FxCorridorPolicyRegistry {
    /// Identifier of the custom parameter carrying all FX corridor policies.
    pub const PARAMETER_ID_STR: &'static str = "iroha:fx_corridor_policies";
    /// Construct the registry custom-parameter identifier.
    #[must_use]
    pub fn parameter_id() -> crate::parameter::CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid FX corridor policy-registry parameter identifier")
    }
    /// Return the policy registered under `policy_id`.
    #[must_use]
    pub fn get(&self, policy_id: &Name) -> Option<&FxCorridorPolicy> {
        self.policies.get(policy_id)
    }
    /// Insert or replace a policy under its embedded identifier.
    pub fn upsert(&mut self, policy: FxCorridorPolicy) {
        self.policies.insert(policy.policy_id.clone(), policy);
    }
    /// Return retained usage for `policy_id`.
    #[must_use]
    pub fn usage(&self, policy_id: &Name) -> Option<&FxCorridorUsage> {
        self.usage.get(policy_id)
    }
    /// Convert the registry into the custom parameter accepted by `SetParameter`.
    #[cfg(feature = "json")]
    #[must_use]
    pub fn into_custom_parameter(self) -> crate::parameter::CustomParameter {
        crate::parameter::CustomParameter::new(
            Self::parameter_id(),
            iroha_primitives::json::Json::new(self),
        )
    }
    /// Decode a matching policy-registry custom parameter.
    ///
    /// # Errors
    ///
    /// Returns a Norito error when a parameter with the registry identifier
    /// does not contain a valid [`FxCorridorPolicyRegistry`].
    #[cfg(feature = "json")]
    pub fn from_custom_parameter(
        custom: &crate::parameter::CustomParameter,
    ) -> Result<Option<Self>, norito::Error> {
        if custom.id() != &Self::parameter_id() {
            return Ok(None);
        }
        Ok(Some(custom.payload().try_into_any_norito::<Self>()?))
    }
}
isi! {
    #[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
    /// Register or replace a native FX corridor policy.
    pub struct SetFxCorridorPolicy {
        /// Complete policy to persist under its stable identifier.
        pub policy: FxCorridorPolicy,
    }
}
isi! {
    #[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
    /// Fund the isolated destination reserve of one exact FX corridor.
    pub struct FundFxCorridorEscrow {
        /// Stable corridor policy identifier.
        pub policy_id: Name,
        /// Exact active policy revision approved by the owner.
        pub expected_policy_revision: u64,
        /// Exact destination asset bound by the signed funding instruction.
        pub destination_asset_definition_id: AssetDefinitionId,
        /// Positive destination-currency quantity to deposit.
        pub amount: Quantity,
    }
}
isi! {
    #[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
    /// Refund an inactive FX corridor reserve to its immutable owner.
    pub struct RefundFxCorridorEscrow {
        /// Stable corridor policy identifier.
        pub policy_id: Name,
        /// Exact inactive policy revision approved by the owner.
        pub expected_policy_revision: u64,
        /// Exact destination asset bound by the signed refund instruction.
        pub destination_asset_definition_id: AssetDefinitionId,
        /// Positive destination-currency quantity to refund.
        pub amount: Quantity,
    }
}
isi! {
    #[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
    /// Atomically settle one policy-backed cross-dataspace FX conversion.
    pub struct SettleFxCorridor {
        /// Stable corridor policy identifier.
        pub policy_id: Name,
        /// Exact policy revision expected by the signer.
        pub expected_policy_revision: u64,
        /// Expected source asset, bound explicitly for admission and fee validation.
        pub source_asset_definition_id: AssetDefinitionId,
        /// Expected destination asset, bound explicitly for admission and fee validation.
        pub destination_asset_definition_id: AssetDefinitionId,
        /// Unique settlement/replay identifier.
        pub settlement_id: SettlementId,
        /// Recipient of the policy-derived destination currency.
        pub recipient: AccountId,
        /// Source-currency quantity collected from the signing account.
        pub source_amount: Quantity,
        /// Exact destination quantity approved by the signer.
        pub expected_destination_amount: Quantity,
        /// Exact retained oracle event approved by the signer.
        pub oracle_evidence: FxCorridorOracleEvidence,
    }
}
impl SetFxCorridorPolicy {
    /// Stable wire identifier used by tooling that reports the concrete operation.
    pub const WIRE_ID: &'static str = "iroha.settlement.fx_corridor.policy.set";
}
impl FundFxCorridorEscrow {
    /// Stable wire identifier used by tooling that reports the concrete operation.
    pub const WIRE_ID: &'static str = "iroha.settlement.fx_corridor.escrow.fund";
}
impl RefundFxCorridorEscrow {
    /// Stable wire identifier used by tooling that reports the concrete operation.
    pub const WIRE_ID: &'static str = "iroha.settlement.fx_corridor.escrow.refund";
}
impl SettleFxCorridor {
    /// Stable wire identifier used by tooling that reports the concrete operation.
    pub const WIRE_ID: &'static str = "iroha.settlement.fx_corridor.settle";
}
impl core::fmt::Display for SetFxCorridorPolicy {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SET FX CORRIDOR POLICY `{}`", self.policy.policy_id)
    }
}
impl core::fmt::Display for FundFxCorridorEscrow {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "FUND FX CORRIDOR ESCROW `{}` REVISION {}",
            self.policy_id, self.expected_policy_revision
        )
    }
}
impl core::fmt::Display for RefundFxCorridorEscrow {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "REFUND FX CORRIDOR ESCROW `{}` REVISION {}",
            self.policy_id, self.expected_policy_revision
        )
    }
}
impl core::fmt::Display for SettleFxCorridor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "SETTLE FX CORRIDOR `{}` REVISION {} AS `{}`",
            self.policy_id, self.expected_policy_revision, self.settlement_id
        )
    }
}
impl SettlementLeg {
    /// Construct a settlement leg without metadata.
    pub fn new(
        asset_definition_id: AssetDefinitionId,
        quantity: impl Into<Quantity>,
        from: AccountId,
        to: AccountId,
    ) -> Self {
        Self {
            asset_definition_id,
            quantity: quantity.into(),
            from,
            to,
            metadata: Metadata::default(),
        }
    }
}
isi! {
    #[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
    /// Delivery-versus-payment settlement instruction requiring exact counterparty consent.
    ///
    /// Core accepts this instruction only with `AllOrNothing` atomicity and a
    /// counterparty-issued permission bound to [`DvpIsi::intent_hash`].
    pub struct DvpIsi {
        /// Stable identifier shared across the delivery lifecycle.
        pub settlement_id: SettlementId,
        /// Asset leg delivered in exchange for the payment leg.
        pub delivery_leg: SettlementLeg,
        /// Payment leg (cash or tokenised currency) completing the exchange.
        pub payment_leg: SettlementLeg,
        /// Execution plan describing leg order and failure handling.
        pub plan: SettlementPlan,
        /// Optional metadata propagated alongside the settlement for downstream reconciliation.
        pub metadata: Metadata,
    }
}
isi! {
    #[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
    /// Payment-versus-payment settlement instruction requiring exact counterparty consent.
    ///
    /// Core accepts this instruction only with `AllOrNothing` atomicity and a
    /// counterparty-issued permission bound to [`PvpIsi::intent_hash`].
    pub struct PvpIsi {
        /// Stable identifier associated with this FX settlement lifecycle.
        pub settlement_id: SettlementId,
        /// Primary currency leg.
        pub primary_leg: SettlementLeg,
        /// Counter currency leg.
        pub counter_leg: SettlementLeg,
        /// Execution plan describing leg order and failure handling.
        pub plan: SettlementPlan,
        /// Optional metadata propagated alongside the settlement for downstream reconciliation.
        pub metadata: Metadata,
    }
}
impl DvpIsi {
    /// Stable wire identifier used by the instruction registry.
    pub const WIRE_ID: &'static str = "iroha.settlement.dvp";
    /// Domain separator for the complete counterparty-authorized `DvP` intent.
    pub const INTENT_HASH_DOMAIN: &'static [u8] = b"iroha:settlement:dvp-intent:v1\0";
    /// Construct a `DvP` instruction enforcing basic invariants.
    pub fn new(
        settlement_id: SettlementId,
        delivery_leg: SettlementLeg,
        payment_leg: SettlementLeg,
        plan: SettlementPlan,
    ) -> Self {
        Self {
            settlement_id,
            delivery_leg,
            payment_leg,
            plan,
            metadata: Metadata::default(),
        }
    }
    /// Return the domain-separated commitment that a counterparty authorizes.
    ///
    /// The commitment covers the complete canonical instruction, including both
    /// legs, execution plan, settlement identifier, and metadata.
    #[must_use]
    pub fn intent_hash(&self) -> Hash {
        let encoded = self.encode();
        Hash::new_from_chunks(&[Self::INTENT_HASH_DOMAIN, encoded.as_slice()])
    }
}
impl PvpIsi {
    /// Stable wire identifier used by the instruction registry.
    pub const WIRE_ID: &'static str = "iroha.settlement.pvp";
    /// Domain separator for the complete counterparty-authorized `PvP` intent.
    pub const INTENT_HASH_DOMAIN: &'static [u8] = b"iroha:settlement:pvp-intent:v1\0";
    /// Construct a `PvP` instruction enforcing basic invariants.
    pub fn new(
        settlement_id: SettlementId,
        primary_leg: SettlementLeg,
        counter_leg: SettlementLeg,
        plan: SettlementPlan,
    ) -> Self {
        Self {
            settlement_id,
            primary_leg,
            counter_leg,
            plan,
            metadata: Metadata::default(),
        }
    }
    /// Return the domain-separated commitment that a counterparty authorizes.
    ///
    /// The commitment covers the complete canonical instruction, including both
    /// legs, execution plan, settlement identifier, and metadata.
    #[must_use]
    pub fn intent_hash(&self) -> Hash {
        let encoded = self.encode();
        Hash::new_from_chunks(&[Self::INTENT_HASH_DOMAIN, encoded.as_slice()])
    }
}
impl core::fmt::Display for DvpIsi {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "DVP `{}` ORDER {:?} ATOMIC {:?}",
            self.settlement_id,
            self.plan.order(),
            self.plan.atomicity(),
        )
    }
}
impl core::fmt::Display for PvpIsi {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "PVP `{}` ORDER {:?} ATOMIC {:?}",
            self.settlement_id,
            self.plan.order(),
            self.plan.atomicity(),
        )
    }
}
/// Settlement kind recorded in a successful receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
#[repr(u8)]
pub enum SettlementKind {
    /// Delivery-versus-payment trade.
    Dvp,
    /// Payment-versus-payment trade.
    Pvp,
    /// Policy-backed native cross-dataspace FX conversion.
    FxCorridor,
}
/// Enumerates the logical role played by a settlement leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
#[repr(u8)]
pub enum SettlementLegRole {
    /// Asset leg in a delivery-versus-payment trade.
    Delivery,
    /// Payment leg in a delivery-versus-payment trade.
    Payment,
    /// Primary leg in a payment-versus-payment trade.
    Primary,
    /// Counter leg in a payment-versus-payment trade.
    Counter,
    /// Source-currency collection leg in an FX corridor settlement.
    FxSource,
    /// Destination-currency payout leg in an FX corridor settlement.
    FxDestination,
}
/// Snapshot of a single leg in a committed settlement receipt.
#[allow(missing_copy_implementations)]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct SettlementLegSnapshot {
    /// Logical role of the leg (delivery, payment, primary, counter).
    pub role: SettlementLegRole,
    /// Original leg payload.
    pub leg: SettlementLeg,
}
/// Immutable pricing and route context retained for a successful native FX settlement.
///
/// The two generic leg snapshots retain the actual balance movements. This record additionally
/// binds those legs to the exact governed policy revision and rate that the signer approved.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct FxCorridorSettlementDetails {
    /// Stable corridor policy identifier.
    pub policy_id: Name,
    /// Exact policy revision used for execution.
    pub policy_revision: u64,
    /// Source private dataspace.
    pub source_dataspace: DataSpaceId,
    /// Destination private dataspace.
    pub destination_dataspace: DataSpaceId,
    /// Immutable corridor owner that received the source currency.
    pub owner: AccountId,
    /// Exact retained oracle event selected by the signed instruction.
    pub oracle_evidence: FxCorridorOracleEvidence,
    /// Consensus-time timestamp attached to the retained oracle event.
    pub oracle_recorded_at_ms: u64,
    /// Exact positive fixed-point destination/source rate from the oracle event.
    pub oracle_rate: ObservationValue,
    /// Account that supplied source currency.
    pub source_account: AccountId,
    /// Non-signable corridor escrow that supplied destination currency.
    pub destination_escrow: AccountId,
    /// Account that received destination currency.
    pub recipient: AccountId,
    /// Source asset definition bound by the signed instruction.
    pub source_asset_definition_id: AssetDefinitionId,
    /// Destination asset definition bound by the signed instruction.
    pub destination_asset_definition_id: AssetDefinitionId,
    /// Source quantity collected.
    pub source_amount: Quantity,
    /// Destination quantity paid out.
    pub destination_amount: Quantity,
}
/// Immutable receipt persisted after one successful settlement.
///
/// The containing world-state map is keyed by the settlement identifier, so the
/// identifier is deliberately not duplicated here. Presence of a receipt means
/// that both legs committed successfully. Failed attempts are observable only
/// through bounded, node-local telemetry and never create consensus state.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(opaque)
)]
pub struct SettlementReceipt {
    /// Settlement kind (`DvP`, `PvP`, or governed FX corridor).
    pub kind: SettlementKind,
    /// Account that authorised the settlement.
    pub authority: AccountId,
    /// Execution plan applied when settling.
    pub plan: SettlementPlan,
    /// Arbitrary metadata supplied alongside the settlement instruction.
    pub metadata: Metadata,
    /// Block height in which the event was recorded.
    pub block_height: u64,
    /// Hash of the enclosing block for traceability.
    pub block_hash: HashOf<BlockHeader>,
    /// Block timestamp (milliseconds since Unix epoch).
    pub executed_at_ms: u64,
    /// The two committed settlement legs.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_array"))]
    pub legs: [SettlementLegSnapshot; 2],
    /// Exact governed FX context, present only for native FX corridor settlements.
    pub fx_corridor: Option<FxCorridorSettlementDetails>,
}
impl crate::seal::Instruction for DvpIsi {}
impl crate::seal::Instruction for PvpIsi {}
impl crate::seal::Instruction for SetFxCorridorPolicy {}
impl crate::seal::Instruction for FundFxCorridorEscrow {}
impl crate::seal::Instruction for RefundFxCorridorEscrow {}
impl crate::seal::Instruction for SettleFxCorridor {}
isi_box! {
    /// Grouping enum for settlement instructions.
    pub enum SettlementInstructionBox {
        /// Delivery-versus-payment settlement.
        Dvp(DvpIsi),
        /// Payment-versus-payment settlement.
        Pvp(PvpIsi),
        /// Register or replace a native FX corridor policy.
        SetFxCorridorPolicy(SetFxCorridorPolicy),
        /// Fund one exact corridor's isolated destination reserve.
        FundFxCorridorEscrow(FundFxCorridorEscrow),
        /// Refund one inactive corridor reserve to its immutable owner.
        RefundFxCorridorEscrow(RefundFxCorridorEscrow),
        /// Execute one policy-backed native FX settlement.
        SettleFxCorridor(SettleFxCorridor),
    }
}
impl_into_box! {
    DvpIsi | PvpIsi | SetFxCorridorPolicy | FundFxCorridorEscrow | RefundFxCorridorEscrow | SettleFxCorridor => SettlementInstructionBox
}
impl crate::seal::Instruction for SettlementInstructionBox {}
impl SettlementInstructionBox {
    /// Stable Norito wire identifier for boxed settlement instructions.
    pub const WIRE_ID: &'static str = "iroha.settlement";
}
fn settlement_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}
macro_rules! impl_settlement_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = settlement_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }
                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}
impl_settlement_decode_from_slice!(DvpIsi {
    settlement_id: SettlementId,
    delivery_leg: SettlementLeg,
    payment_leg: SettlementLeg,
    plan: SettlementPlan,
    metadata: Metadata,
});
impl_settlement_decode_from_slice!(PvpIsi {
    settlement_id: SettlementId,
    primary_leg: SettlementLeg,
    counter_leg: SettlementLeg,
    plan: SettlementPlan,
    metadata: Metadata,
});
impl_settlement_decode_from_slice!(SetFxCorridorPolicy {
    policy: FxCorridorPolicy,
});
impl_settlement_decode_from_slice!(FundFxCorridorEscrow {
    policy_id: Name,
    expected_policy_revision: u64,
    destination_asset_definition_id: AssetDefinitionId,
    amount: Quantity,
});
impl_settlement_decode_from_slice!(RefundFxCorridorEscrow {
    policy_id: Name,
    expected_policy_revision: u64,
    destination_asset_definition_id: AssetDefinitionId,
    amount: Quantity,
});
impl_settlement_decode_from_slice!(SettleFxCorridor {
    policy_id: Name,
    expected_policy_revision: u64,
    source_asset_definition_id: AssetDefinitionId,
    destination_asset_definition_id: AssetDefinitionId,
    settlement_id: SettlementId,
    recipient: AccountId,
    source_amount: Quantity,
    expected_destination_amount: Quantity,
    oracle_evidence: FxCorridorOracleEvidence,
});
impl<'a> norito::core::DecodeFromSlice<'a> for SettlementInstructionBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = settlement_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let tag_bytes = bytes.get(..4).ok_or(norito::core::Error::LengthMismatch)?;
        let tag = u32::from_le_bytes(
            tag_bytes
                .try_into()
                .map_err(|_| norito::core::Error::LengthMismatch)?,
        );
        let mut offset = 4usize;
        let value = match tag {
            0 => Self::Dvp(super::decode_aos_slice_field::<DvpIsi>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            1 => Self::Pvp(super::decode_aos_slice_field::<PvpIsi>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            2 => Self::SetFxCorridorPolicy(super::decode_aos_slice_field::<SetFxCorridorPolicy>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            3 => Self::FundFxCorridorEscrow(super::decode_aos_slice_field::<FundFxCorridorEscrow>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            4 => Self::RefundFxCorridorEscrow(super::decode_aos_slice_field::<
                RefundFxCorridorEscrow,
            >(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            5 => Self::SettleFxCorridor(super::decode_aos_slice_field::<SettleFxCorridor>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            _ => {
                return Err(norito::core::Error::Message(format!(
                    "invalid SettlementInstructionBox tag {tag}"
                )));
            }
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((value, offset))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainId;
    use iroha_primitives::numeric::Numeric;
    use norito::codec::{Decode, Encode};
    use norito::core::DecodeFromSlice;
    #[derive(Encode)]
    struct ForgedSettleFxCorridor {
        policy_id: Name,
        expected_policy_revision: u64,
        source_asset_definition_id: AssetDefinitionId,
        destination_asset_definition_id: AssetDefinitionId,
        settlement_id: SettlementId,
        recipient: AccountId,
        source_amount: Numeric,
        expected_destination_amount: Quantity,
        oracle_evidence: FxCorridorOracleEvidence,
    }
    #[derive(Encode)]
    struct ForgedSettlementLeg {
        asset_definition_id: AssetDefinitionId,
        quantity: Numeric,
        from: AccountId,
        to: AccountId,
        metadata: Metadata,
    }
    #[derive(Encode)]
    struct ForgedFxCorridorSettlementDetails {
        policy_id: Name,
        policy_revision: u64,
        source_dataspace: DataSpaceId,
        destination_dataspace: DataSpaceId,
        owner: AccountId,
        oracle_evidence: FxCorridorOracleEvidence,
        oracle_recorded_at_ms: u64,
        oracle_rate: ObservationValue,
        source_account: AccountId,
        destination_escrow: AccountId,
        recipient: AccountId,
        source_asset_definition_id: AssetDefinitionId,
        destination_asset_definition_id: AssetDefinitionId,
        source_amount: Numeric,
        destination_amount: Numeric,
    }
    const ALICE_SIGNATORY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    const BOB_SIGNATORY: &str =
        "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016";
    fn account(signatory: &str, _domain: &str) -> AccountId {
        AccountId::new(signatory.parse().expect("public key"))
    }
    fn asset(domain: &str, name: &str) -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new(domain, "universal").expect("domain"),
            name.parse().expect("name"),
        )
    }
    fn dvp_instruction() -> DvpIsi {
        let settlement_id: SettlementId = "dvp_trade_1".parse().expect("settlement id");
        let delivery_leg = SettlementLeg::new(
            asset("wonderland", "bond"),
            1_000u32,
            account(ALICE_SIGNATORY, "test"),
            account(BOB_SIGNATORY, "test"),
        );
        let payment_leg = SettlementLeg::new(
            asset("wonderland", "usd"),
            1_005u32,
            account(BOB_SIGNATORY, "test"),
            account(ALICE_SIGNATORY, "test"),
        );
        DvpIsi {
            settlement_id,
            delivery_leg,
            payment_leg,
            plan: SettlementPlan::new(
                SettlementExecutionOrder::DeliveryThenPayment,
                SettlementAtomicity::AllOrNothing,
            ),
            metadata: Metadata::default(),
        }
    }
    fn pvp_instruction() -> PvpIsi {
        let settlement_id: SettlementId = "pvp_fx_1".parse().expect("settlement id");
        let primary_leg = SettlementLeg::new(
            asset("wonderland", "usd"),
            1_000u32,
            account(ALICE_SIGNATORY, "test"),
            account(BOB_SIGNATORY, "test"),
        );
        let counter_leg = SettlementLeg::new(
            asset("wonderland", "eur"),
            920u32,
            account(BOB_SIGNATORY, "test"),
            account(ALICE_SIGNATORY, "test"),
        );
        PvpIsi {
            settlement_id,
            primary_leg,
            counter_leg,
            plan: SettlementPlan::new(
                SettlementExecutionOrder::PaymentThenDelivery,
                SettlementAtomicity::CommitSecondLeg,
            ),
            metadata: Metadata::default(),
        }
    }
    fn fx_policy() -> FxCorridorPolicy {
        FxCorridorPolicy {
            policy_id: "cbuae_aed_sbp_pkr".parse().expect("policy id"),
            revision: 1,
            owner: account(ALICE_SIGNATORY, "cbuae"),
            source_dataspace: DataSpaceId::new(10),
            source_asset_definition_id: asset("cbuae", "aed"),
            destination_dataspace: DataSpaceId::new(12),
            destination_asset_definition_id: asset("sbp", "pkr"),
            allowed_destination_alias_domains: BTreeSet::from([
                DomainId::try_new("hbl", "sbp").expect("HBL domain"),
                DomainId::try_new("ubl", "sbp").expect("UBL domain"),
            ]),
            oracle_feed_id: "aed_pkr".parse().expect("feed id"),
            max_oracle_age_ms: 60_000,
            max_source_amount_per_settlement: Quantity::from(1_000_u32),
            max_destination_amount_per_settlement: Quantity::from(100_000_u32),
            velocity_window_ms: 60_000,
            max_settlements_per_window: 100,
            max_source_amount_per_window: Quantity::from(10_000_u32),
            max_destination_amount_per_window: Quantity::from(1_000_000_u32),
            enabled: true,
        }
    }
    fn fx_oracle_event() -> FeedEvent {
        FeedEvent {
            feed_id: "aed_pkr".parse().expect("feed id"),
            feed_config_version: FeedConfigVersion(1),
            slot: 42,
            request_hash: Hash::new(b"aed-pkr-request"),
            outcome: crate::oracle::FeedEventOutcome::Success(crate::oracle::FeedSuccess {
                value: ObservationValue::new(76, 0),
                entries: Vec::new(),
            }),
        }
    }
    fn fx_oracle_evidence() -> FxCorridorOracleEvidence {
        let event = fx_oracle_event();
        FxCorridorOracleEvidence {
            feed_id: event.feed_id.clone(),
            feed_config_version: event.feed_config_version,
            slot: event.slot,
            request_hash: event.request_hash,
            event_hash: HashOf::new(&event),
        }
    }
    fn fx_settlement_instruction() -> SettleFxCorridor {
        let policy = fx_policy();
        SettleFxCorridor {
            policy_id: policy.policy_id,
            expected_policy_revision: policy.revision,
            source_asset_definition_id: policy.source_asset_definition_id,
            destination_asset_definition_id: policy.destination_asset_definition_id,
            settlement_id: "fx_settlement_1".parse().expect("settlement id"),
            recipient: account(BOB_SIGNATORY, "sbp"),
            source_amount: Quantity::from(10_u32),
            expected_destination_amount: Quantity::from(760_u32),
            oracle_evidence: fx_oracle_evidence(),
        }
    }
    fn fx_funding_instruction() -> FundFxCorridorEscrow {
        let policy = fx_policy();
        FundFxCorridorEscrow {
            policy_id: policy.policy_id,
            expected_policy_revision: policy.revision,
            destination_asset_definition_id: policy.destination_asset_definition_id,
            amount: Quantity::from(10_000_u32),
        }
    }
    fn fx_refund_instruction() -> RefundFxCorridorEscrow {
        let policy = fx_policy();
        RefundFxCorridorEscrow {
            policy_id: policy.policy_id,
            expected_policy_revision: policy.revision,
            destination_asset_definition_id: policy.destination_asset_definition_id,
            amount: Quantity::from(250_u32),
        }
    }
    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }
    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }
    #[test]
    fn dvp_roundtrip_encodes_plan() {
        let settlement_id: SettlementId = "dvp_trade_1".parse().expect("settlement id");
        let delivery_leg = SettlementLeg::new(
            asset("wonderland", "bond"),
            1_000u32,
            account(ALICE_SIGNATORY, "test"),
            account(BOB_SIGNATORY, "test"),
        );
        let payment_leg = SettlementLeg::new(
            asset("wonderland", "usd"),
            1_005u32,
            account(BOB_SIGNATORY, "test"),
            account(ALICE_SIGNATORY, "test"),
        );
        let plan = SettlementPlan::new(
            SettlementExecutionOrder::DeliveryThenPayment,
            SettlementAtomicity::AllOrNothing,
        );
        let instruction = DvpIsi {
            settlement_id: settlement_id.clone(),
            delivery_leg: delivery_leg.clone(),
            payment_leg: payment_leg.clone(),
            plan,
            metadata: Metadata::default(),
        };
        let bytes = instruction.encode();
        let decoded = DvpIsi::decode(&mut bytes.as_slice()).expect("decode");
        assert_eq!(instruction, decoded);
        assert_eq!(decoded.plan().order(), plan.order());
        assert_eq!(decoded.plan().atomicity(), plan.atomicity());
        assert_eq!(
            decoded.delivery_leg().asset_definition_id(),
            delivery_leg.asset_definition_id()
        );
    }
    #[test]
    fn pvp_display_includes_identifier() {
        let settlement_id: SettlementId = "pvp_fx_1".parse().expect("settlement id");
        let primary_leg = SettlementLeg::new(
            asset("wonderland", "usd"),
            1_000u32,
            account(ALICE_SIGNATORY, "test"),
            account(BOB_SIGNATORY, "test"),
        );
        let counter_leg = SettlementLeg::new(
            asset("wonderland", "eur"),
            920u32,
            account(BOB_SIGNATORY, "test"),
            account(ALICE_SIGNATORY, "test"),
        );
        let plan = SettlementPlan::new(
            SettlementExecutionOrder::PaymentThenDelivery,
            SettlementAtomicity::CommitSecondLeg,
        );
        let instruction = PvpIsi {
            settlement_id,
            primary_leg,
            counter_leg,
            plan,
            metadata: Metadata::default(),
        };
        let formatted = format!("{instruction}");
        assert!(formatted.contains("pvp_fx_1"));
        assert!(formatted.contains("PaymentThenDelivery"));
        assert!(formatted.contains("CommitSecondLeg"));
    }
    #[test]
    fn bilateral_intent_hash_binds_complete_instruction() {
        let dvp = dvp_instruction();
        let same = dvp.clone();
        let mut changed = dvp.clone();
        changed.payment_leg.quantity = Quantity::from(1_006_u32);
        assert_eq!(dvp.intent_hash(), same.intent_hash());
        assert_ne!(dvp.intent_hash(), changed.intent_hash());
        assert_ne!(dvp.intent_hash(), pvp_instruction().intent_hash());
    }
    #[test]
    fn settlement_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(dvp_instruction());
        assert_slice_roundtrip(pvp_instruction());
        assert_slice_roundtrip(SetFxCorridorPolicy {
            policy: fx_policy(),
        });
        assert_slice_roundtrip(fx_funding_instruction());
        assert_slice_roundtrip(fx_refund_instruction());
        assert_slice_roundtrip(fx_settlement_instruction());
        assert_slice_roundtrip(SettlementInstructionBox::Dvp(dvp_instruction()));
        assert_slice_roundtrip(SettlementInstructionBox::Pvp(pvp_instruction()));
        assert_slice_roundtrip(SettlementInstructionBox::SetFxCorridorPolicy(
            SetFxCorridorPolicy {
                policy: fx_policy(),
            },
        ));
        assert_slice_roundtrip(SettlementInstructionBox::FundFxCorridorEscrow(
            fx_funding_instruction(),
        ));
        assert_slice_roundtrip(SettlementInstructionBox::RefundFxCorridorEscrow(
            fx_refund_instruction(),
        ));
        assert_slice_roundtrip(SettlementInstructionBox::SettleFxCorridor(
            fx_settlement_instruction(),
        ));
    }
    #[test]
    fn fx_protocol_escrow_binds_exact_network_corridor_and_asset() {
        let policy = fx_policy();
        let first_network = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"fx-network-one")),
        );
        let second_network = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"fx-network-two")),
        );
        let first = fx_corridor_escrow_account_id_v1(
            &first_network,
            &policy.corridor_id(),
            &policy.destination_asset_definition_id,
        );
        let other_network = fx_corridor_escrow_account_id_v1(
            &second_network,
            &policy.corridor_id(),
            &policy.destination_asset_definition_id,
        );
        let other_asset = fx_corridor_escrow_account_id_v1(
            &first_network,
            &policy.corridor_id(),
            &policy.source_asset_definition_id,
        );
        let mut other_corridor = policy.corridor_id();
        other_corridor.policy_id = "other_corridor".parse().expect("corridor id");
        let other_corridor = fx_corridor_escrow_account_id_v1(
            &first_network,
            &other_corridor,
            &policy.destination_asset_definition_id,
        );
        assert_ne!(first, other_network);
        assert_ne!(first, other_asset);
        assert_ne!(first, other_corridor);
    }
    #[cfg(feature = "json")]
    #[test]
    fn settlement_instructions_json_roundtrip() {
        let dvp = dvp_instruction();
        let encoded = norito::json::to_json(&dvp).expect("serialize DvP instruction");
        let decoded: DvpIsi =
            norito::json::from_str(&encoded).expect("deserialize DvP instruction");
        assert_eq!(decoded, dvp);
        let pvp = pvp_instruction();
        let encoded = norito::json::to_json(&pvp).expect("serialize PvP instruction");
        let decoded: PvpIsi =
            norito::json::from_str(&encoded).expect("deserialize PvP instruction");
        assert_eq!(decoded, pvp);
        let policy = SetFxCorridorPolicy {
            policy: fx_policy(),
        };
        let encoded = norito::json::to_json(&policy).expect("serialize FX policy instruction");
        assert!(encoded.contains("\"owner\":"));
        assert!(encoded.contains("\"oracle_feed_id\":[\"aed_pkr\"]"));
        assert!(
            encoded.contains("\"allowed_destination_alias_domains\":[\"hbl.sbp\",\"ubl.sbp\"]")
        );
        let decoded: SetFxCorridorPolicy =
            norito::json::from_str(&encoded).expect("deserialize FX policy instruction");
        assert_eq!(decoded, policy);
        assert!(!encoded.contains("source_sink"));
        assert!(!encoded.contains("destination_reserve"));
        assert!(!encoded.contains("rate_numerator"));
        let settlement = fx_settlement_instruction();
        let encoded =
            norito::json::to_json(&settlement).expect("serialize FX settlement instruction");
        let decoded: SettleFxCorridor =
            norito::json::from_str(&encoded).expect("deserialize FX settlement instruction");
        assert_eq!(decoded, settlement);
    }
    #[test]
    fn fx_receipt_json_requires_canonical_quantity_strings() {
        let policy = fx_policy();
        let details = FxCorridorSettlementDetails {
            policy_id: policy.policy_id,
            policy_revision: policy.revision,
            source_dataspace: policy.source_dataspace,
            destination_dataspace: policy.destination_dataspace,
            owner: policy.owner.clone(),
            oracle_evidence: fx_oracle_evidence(),
            oracle_recorded_at_ms: 1_700_000_000_000,
            oracle_rate: ObservationValue::new(76, 0),
            source_account: account(ALICE_SIGNATORY, "cbuae"),
            destination_escrow: policy.owner,
            recipient: account(BOB_SIGNATORY, "sbp"),
            source_asset_definition_id: policy.source_asset_definition_id,
            destination_asset_definition_id: policy.destination_asset_definition_id,
            source_amount: Quantity::from(10_u32),
            destination_amount: Quantity::from(760_u32),
        };
        let canonical = norito::json::to_json(&details).expect("serialize FX receipt details");
        assert!(canonical.contains("\"source_amount\":\"10\""));
        assert!(canonical.contains("\"destination_amount\":\"760\""));
        let decoded: FxCorridorSettlementDetails =
            norito::json::from_str(&canonical).expect("canonical FX receipt JSON");
        assert_eq!(decoded, details);
        for replacement in ["\"source_amount\":10", "\"source_amount\":\"010\""] {
            let malformed = canonical.replace("\"source_amount\":\"10\"", replacement);
            assert!(
                norito::json::from_str::<FxCorridorSettlementDetails>(&malformed).is_err(),
                "FX receipts must reject non-string and non-canonical quantity encodings",
            );
        }
    }
    #[test]
    fn committed_settlement_receipt_roundtrips_one_fixed_leg_pair() {
        let DvpIsi {
            delivery_leg,
            payment_leg,
            plan,
            ..
        } = dvp_instruction();
        let receipt = SettlementReceipt {
            kind: SettlementKind::Dvp,
            authority: delivery_leg.from().clone(),
            plan,
            metadata: Metadata::default(),
            block_height: 7,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0xA7; Hash::LENGTH],
            )),
            executed_at_ms: 11,
            legs: [
                SettlementLegSnapshot {
                    role: SettlementLegRole::Delivery,
                    leg: delivery_leg,
                },
                SettlementLegSnapshot {
                    role: SettlementLegRole::Payment,
                    leg: payment_leg,
                },
            ],
            fx_corridor: None,
        };
        let bytes = receipt.encode();
        let decoded = SettlementReceipt::decode(&mut bytes.as_slice()).expect("decode receipt");
        assert_eq!(decoded, receipt);
        assert_eq!(decoded.legs.len(), 2);
        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&receipt).expect("serialize receipt");
            assert!(
                !json.contains("\"settlement_id\"") && !json.contains("\"outcome\""),
                "the map key is the sole settlement identity and receipt presence means success"
            );
            let decoded: SettlementReceipt =
                norito::json::from_str(&json).expect("deserialize receipt");
            assert_eq!(decoded, receipt);
            assert_eq!(decoded.legs.len(), 2);
        }
    }
    #[test]
    fn negative_numeric_payloads_cannot_decode_as_settlement_quantities() {
        let encoded = ForgedSettlementLeg {
            asset_definition_id: asset("wonderland", "bond"),
            quantity: Numeric::new(-1_i32, 0),
            from: account(ALICE_SIGNATORY, "test"),
            to: account(BOB_SIGNATORY, "test"),
            metadata: Metadata::default(),
        }
        .encode();
        assert!(
            SettlementLeg::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a settlement leg"
        );
        let policy = fx_policy();
        let recipient = account(BOB_SIGNATORY, "sbp");
        let forged_instruction = ForgedSettleFxCorridor {
            policy_id: policy.policy_id.clone(),
            expected_policy_revision: policy.revision,
            source_asset_definition_id: policy.source_asset_definition_id.clone(),
            destination_asset_definition_id: policy.destination_asset_definition_id.clone(),
            settlement_id: "negative_fx_settlement".parse().expect("settlement id"),
            recipient: recipient.clone(),
            source_amount: Numeric::new(-1_i32, 0),
            expected_destination_amount: Quantity::from(760_u32),
            oracle_evidence: fx_oracle_evidence(),
        };
        assert!(
            SettleFxCorridor::decode_from_slice(&forged_instruction.encode()).is_err(),
            "a negative signed payload must not decode as an FX settlement instruction"
        );
        let forged_receipt = ForgedFxCorridorSettlementDetails {
            policy_id: policy.policy_id,
            policy_revision: policy.revision,
            source_dataspace: policy.source_dataspace,
            destination_dataspace: policy.destination_dataspace,
            owner: policy.owner.clone(),
            oracle_evidence: fx_oracle_evidence(),
            oracle_recorded_at_ms: 1_700_000_000_000,
            oracle_rate: ObservationValue::new(76, 0),
            source_account: policy.owner.clone(),
            destination_escrow: policy.owner,
            recipient,
            source_asset_definition_id: policy.source_asset_definition_id,
            destination_asset_definition_id: policy.destination_asset_definition_id,
            source_amount: Numeric::new(-1_i32, 0),
            destination_amount: Numeric::from(760_u32),
        };
        let encoded = forged_receipt.encode();
        assert!(
            FxCorridorSettlementDetails::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a durable FX settlement receipt"
        );
    }
    #[test]
    fn settlement_registry_decodes_type_names_and_stable_ids() {
        let registry = crate::isi::registry::default();
        assert_registry_decodes(
            &registry,
            std::any::type_name::<SettlementInstructionBox>(),
            SettlementInstructionBox::Dvp(dvp_instruction()),
        );
        for value in [
            SettlementInstructionBox::Dvp(dvp_instruction()),
            SettlementInstructionBox::Pvp(pvp_instruction()),
            SettlementInstructionBox::SetFxCorridorPolicy(SetFxCorridorPolicy {
                policy: fx_policy(),
            }),
            SettlementInstructionBox::FundFxCorridorEscrow(fx_funding_instruction()),
            SettlementInstructionBox::RefundFxCorridorEscrow(fx_refund_instruction()),
            SettlementInstructionBox::SettleFxCorridor(fx_settlement_instruction()),
        ] {
            assert_registry_decodes(&registry, SettlementInstructionBox::WIRE_ID, value);
        }
        for name in [
            std::any::type_name::<DvpIsi>(),
            std::any::type_name::<PvpIsi>(),
            std::any::type_name::<SetFxCorridorPolicy>(),
            std::any::type_name::<FundFxCorridorEscrow>(),
            std::any::type_name::<RefundFxCorridorEscrow>(),
            std::any::type_name::<SettleFxCorridor>(),
            DvpIsi::WIRE_ID,
            PvpIsi::WIRE_ID,
            SetFxCorridorPolicy::WIRE_ID,
            FundFxCorridorEscrow::WIRE_ID,
            RefundFxCorridorEscrow::WIRE_ID,
            SettleFxCorridor::WIRE_ID,
        ] {
            assert!(!registry.contains(name), "{name} must remain boxed-only");
        }
    }
}
