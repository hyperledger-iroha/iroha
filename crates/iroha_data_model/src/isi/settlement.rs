use std::collections::BTreeMap;

use derive_more::{Constructor, Display, FromStr};
use getset::{CopyGetters, Getters};
use iroha_crypto::HashOf;
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use iroha_primitives::numeric::Quantity;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::derive::{JsonDeserialize, JsonSerialize};

pub use self::model::SettlementId;
use super::*;
use crate::{
    Name,
    block::BlockHeader,
    metadata::Metadata,
    nexus::DataSpaceId,
    prelude::{AccountId, AssetDefinitionId},
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
    pub quantity: Quantity,
    /// Account sending the asset.
    pub from: AccountId,
    /// Account receiving the asset.
    pub to: AccountId,
    /// Optional metadata (e.g., ISIN, clearing reference).
    pub metadata: Metadata,
}

/// Immutable routing and pricing policy for one native FX corridor.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct FxCorridorPolicy {
    /// Stable policy identifier referenced by settlement instructions.
    pub policy_id: Name,
    /// Monotonic policy revision used to bind signed settlement intent.
    pub revision: u64,
    /// Dataspace holding the source currency balance.
    pub source_dataspace: DataSpaceId,
    /// Fixed account funding the source-currency leg.
    pub source_account: AccountId,
    /// Source-currency asset definition.
    pub source_asset_definition_id: AssetDefinitionId,
    /// Fixed sink receiving the source currency.
    pub source_sink: AccountId,
    /// Dataspace holding the destination reserve.
    pub destination_dataspace: DataSpaceId,
    /// Fixed reserve funding destination-currency payouts.
    pub destination_reserve: AccountId,
    /// Destination-currency asset definition.
    pub destination_asset_definition_id: AssetDefinitionId,
    /// Exact destination/source rate numerator.
    pub rate_numerator: u64,
    /// Exact destination/source rate denominator.
    pub rate_denominator: u64,
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
        if self.source_account == self.source_sink {
            return Some("FX corridor source account and sink must be distinct");
        }
        if self.source_asset_definition_id == self.destination_asset_definition_id {
            return Some("FX corridor assets must be distinct");
        }
        if self.rate_numerator == 0 || self.rate_denominator == 0 {
            return Some("FX corridor rate terms must be non-zero");
        }
        None
    }
}

/// Complete governed set of native FX corridor policies.
#[derive(Debug, Clone, Default, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
pub struct FxCorridorPolicyRegistry {
    /// Policies keyed by their stable identifier.
    pub policies: BTreeMap<Name, FxCorridorPolicy>,
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
        /// Source-currency quantity collected from the fixed policy account.
        pub source_amount: Quantity,
    }
}

impl SetFxCorridorPolicy {
    /// Stable wire identifier used by tooling that reports the concrete operation.
    pub const WIRE_ID: &'static str = "iroha.settlement.fx_corridor.policy.set";
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
    #[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
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
    /// Policy-backed native cross-dataspace FX conversion.
    FxCorridor,
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
    /// Source-currency collection leg in an FX corridor settlement.
    FxSource,
    /// Destination-currency payout leg in an FX corridor settlement.
    FxDestination,
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

/// Immutable pricing and route context retained for a successful native FX settlement.
///
/// The two generic leg snapshots retain the actual balance movements. This record additionally
/// binds those legs to the exact governed policy revision and rate that the signer approved.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct FxCorridorSettlementDetails {
    /// Stable corridor policy identifier.
    pub policy_id: Name,
    /// Exact policy revision used for execution.
    pub policy_revision: u64,
    /// Source private dataspace.
    pub source_dataspace: DataSpaceId,
    /// Destination private dataspace.
    pub destination_dataspace: DataSpaceId,
    /// Exact destination/source rate numerator.
    pub rate_numerator: u64,
    /// Exact destination/source rate denominator.
    pub rate_denominator: u64,
    /// Account that supplied source currency.
    pub source_account: AccountId,
    /// Account that received source currency.
    pub source_sink: AccountId,
    /// Account that supplied destination currency.
    pub destination_reserve: AccountId,
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
    /// Exact governed FX context, present only for native FX corridor settlements.
    pub fx_corridor: Option<FxCorridorSettlementDetails>,
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
impl crate::seal::Instruction for SetFxCorridorPolicy {}
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
        /// Execute one policy-backed native FX settlement.
        SettleFxCorridor(SettleFxCorridor),
    }
}

impl_into_box! {
    DvpIsi | PvpIsi | SetFxCorridorPolicy | SettleFxCorridor => SettlementInstructionBox
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

impl_settlement_decode_from_slice!(SettleFxCorridor {
    policy_id: Name,
    expected_policy_revision: u64,
    source_asset_definition_id: AssetDefinitionId,
    destination_asset_definition_id: AssetDefinitionId,
    settlement_id: SettlementId,
    recipient: AccountId,
    source_amount: Quantity,
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
            3 => Self::SettleFxCorridor(super::decode_aos_slice_field::<SettleFxCorridor>(
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
    }

    #[derive(Encode)]
    struct ForgedFxCorridorSettlementDetails {
        policy_id: Name,
        policy_revision: u64,
        source_dataspace: DataSpaceId,
        destination_dataspace: DataSpaceId,
        rate_numerator: u64,
        rate_denominator: u64,
        source_account: AccountId,
        source_sink: AccountId,
        destination_reserve: AccountId,
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
        AssetDefinitionId::new(
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
            source_dataspace: DataSpaceId::new(10),
            source_account: account(ALICE_SIGNATORY, "cbuae"),
            source_asset_definition_id: asset("cbuae", "aed"),
            source_sink: account(BOB_SIGNATORY, "cbuae"),
            destination_dataspace: DataSpaceId::new(12),
            destination_reserve: account(ALICE_SIGNATORY, "sbp"),
            destination_asset_definition_id: asset("sbp", "pkr"),
            rate_numerator: 76,
            rate_denominator: 1,
            enabled: true,
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
    fn settlement_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(dvp_instruction());
        assert_slice_roundtrip(pvp_instruction());
        assert_slice_roundtrip(SetFxCorridorPolicy {
            policy: fx_policy(),
        });
        assert_slice_roundtrip(fx_settlement_instruction());
        assert_slice_roundtrip(SettlementInstructionBox::Dvp(dvp_instruction()));
        assert_slice_roundtrip(SettlementInstructionBox::Pvp(pvp_instruction()));
        assert_slice_roundtrip(SettlementInstructionBox::SetFxCorridorPolicy(
            SetFxCorridorPolicy {
                policy: fx_policy(),
            },
        ));
        assert_slice_roundtrip(SettlementInstructionBox::SettleFxCorridor(
            fx_settlement_instruction(),
        ));
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
        let decoded: SetFxCorridorPolicy =
            norito::json::from_str(&encoded).expect("deserialize FX policy instruction");
        assert_eq!(decoded, policy);

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
            rate_numerator: policy.rate_numerator,
            rate_denominator: policy.rate_denominator,
            source_account: policy.source_account,
            source_sink: policy.source_sink,
            destination_reserve: policy.destination_reserve,
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
            SettlementInstructionBox::SettleFxCorridor(fx_settlement_instruction()),
        ] {
            assert_registry_decodes(&registry, SettlementInstructionBox::WIRE_ID, value);
        }
        for name in [
            std::any::type_name::<DvpIsi>(),
            std::any::type_name::<PvpIsi>(),
            std::any::type_name::<SetFxCorridorPolicy>(),
            std::any::type_name::<SettleFxCorridor>(),
            DvpIsi::WIRE_ID,
            PvpIsi::WIRE_ID,
            SetFxCorridorPolicy::WIRE_ID,
            SettleFxCorridor::WIRE_ID,
        ] {
            assert!(!registry.contains(name), "{name} must remain boxed-only");
        }
    }
}
