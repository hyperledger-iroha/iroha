use derive_more::{Constructor, Display, FromStr};
use getset::{CopyGetters, Getters};
use iroha_crypto::HashOf;
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::derive::{JsonDeserialize, JsonSerialize};

pub use self::model::SettlementId;
use super::*;
use crate::{
    Name,
    block::BlockHeader,
    metadata::Metadata,
    prelude::{AccountId, AssetDefinitionId, Numeric},
};

#[model]
mod model {
    use super::*;

    /// Identifier shared across a settlement lifecycle.
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
    pub quantity: Numeric,
    /// Account sending the asset.
    pub from: AccountId,
    /// Account receiving the asset.
    pub to: AccountId,
    /// Optional metadata (e.g., ISIN, clearing reference).
    pub metadata: Metadata,
}

impl SettlementLeg {
    /// Construct a settlement leg without metadata.
    pub fn new(
        asset_definition_id: AssetDefinitionId,
        quantity: Numeric,
        from: AccountId,
        to: AccountId,
    ) -> Self {
        Self {
            asset_definition_id,
            quantity,
            from,
            to,
            metadata: Metadata::default(),
        }
    }
}

isi! {
    /// Delivery-versus-payment settlement instruction ensuring atomic exchange between asset and payment legs.
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
    /// Payment-versus-payment settlement instruction covering cross-currency exchanges.
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
}

impl PvpIsi {
    /// Stable wire identifier used by the instruction registry.
    pub const WIRE_ID: &'static str = "iroha.settlement.pvp";

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

/// Settlement kind used when recording ledger events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[repr(u8)]
pub enum SettlementKind {
    /// Delivery-versus-payment trade.
    Dvp,
    /// Payment-versus-payment trade.
    Pvp,
}

/// Enumerates the logical role played by a settlement leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
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
}

/// Snapshot of a single settlement leg recorded in the ledger.
#[allow(missing_copy_implementations)]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct SettlementLegSnapshot {
    /// Logical role of the leg (delivery, payment, primary, counter).
    pub role: SettlementLegRole,
    /// Original leg payload.
    pub leg: SettlementLeg,
    /// Whether the leg remained committed after atomicity handling.
    pub committed: bool,
}

/// Snapshot of a successful settlement attempt.
#[allow(missing_copy_implementations)]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct SettlementSuccessRecord {
    /// Whether the first leg remained committed after execution.
    pub first_committed: bool,
    /// Whether the second leg remained committed after execution.
    pub second_committed: bool,
    /// Observed FX window in milliseconds (`PvP` only).
    pub fx_window_ms: Option<u64>,
}

/// Snapshot of a failed settlement attempt.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct SettlementFailureRecord {
    /// Stable failure classification.
    pub reason: String,
}

/// Outcome recorded for a settlement attempt.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "detail"))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub enum SettlementOutcomeRecord {
    /// Settlement completed successfully.
    Success(SettlementSuccessRecord),
    /// Settlement failed; legs may have been rolled back.
    Failure(SettlementFailureRecord),
}

impl SettlementOutcomeRecord {
    /// Returns `true` when the outcome represents a successful settlement.
    #[must_use]
    pub const fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }
}

/// Ledger entry persisted for auditing and reconciliation.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct SettlementLedgerEntry {
    /// Unique identifier shared across the settlement lifecycle.
    pub settlement_id: SettlementId,
    /// Settlement kind (`DvP` or `PvP`).
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
    /// Legs captured with their committed state at the end of execution.
    pub legs: Vec<SettlementLegSnapshot>,
    /// Outcome of the settlement execution.
    pub outcome: SettlementOutcomeRecord,
}

/// Auditing trail for a settlement identifier.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct SettlementLedger {
    /// Chronological sequence of settlement attempts tied to the identifier.
    pub entries: Vec<SettlementLedgerEntry>,
}

impl SettlementLedger {
    /// Append a new entry to the ledger.
    pub fn push(&mut self, entry: SettlementLedgerEntry) {
        self.entries.push(entry);
    }

    /// Iterate over ledger entries.
    #[must_use]
    pub fn entries(&self) -> &[SettlementLedgerEntry] {
        &self.entries
    }
}

impl crate::seal::Instruction for DvpIsi {}
impl crate::seal::Instruction for PvpIsi {}

isi_box! {
    /// Grouping enum for settlement instructions.
    pub enum SettlementInstructionBox {
        /// Delivery-versus-payment settlement.
        Dvp(DvpIsi),
        /// Payment-versus-payment settlement.
        Pvp(PvpIsi),
    }
}

impl_into_box! {
    DvpIsi | PvpIsi => SettlementInstructionBox
}

impl crate::seal::Instruction for SettlementInstructionBox {}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE_SIGNATORY: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
    const BOB_SIGNATORY: &str =
        "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016";

    fn account(signatory: &str, _domain: &str) -> AccountId {
        AccountId::new(signatory.parse().expect("public key"))
    }

    fn asset(id: &str) -> AssetDefinitionId {
        id.parse().expect("asset id")
    }

    #[test]
    fn dvp_roundtrip_encodes_plan() {
        let settlement_id: SettlementId = "dvp_trade_1".parse().expect("settlement id");
        let delivery_leg = SettlementLeg::new(
            asset("bond#wonderland"),
            1_000u32.into(),
            account(ALICE_SIGNATORY, "test"),
            account(BOB_SIGNATORY, "test"),
        );
        let payment_leg = SettlementLeg::new(
            asset("usd#wonderland"),
            1_005u32.into(),
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
            asset("usd#wonderland"),
            1_000u32.into(),
            account(ALICE_SIGNATORY, "test"),
            account(BOB_SIGNATORY, "test"),
        );
        let counter_leg = SettlementLeg::new(
            asset("eur#wonderland"),
            920u32.into(),
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
}
