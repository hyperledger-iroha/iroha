use std::fmt::Display;

use iroha_primitives::numeric::Numeric;
#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize};

use super::*;

isi! {
    /// Generic instruction for a transfer of an object from the identifiable source to the identifiable destination.
    pub struct Transfer<S: Identifiable, O, D: Identifiable> {
        /// Source object `Id`.
        pub source: S::Id,
        /// Object which should be transferred.
        pub object: O,
        /// Destination object `Id`.
        pub destination: D::Id,
    }
}

impl Transfer<Account, DomainId, Account> {
    /// Constructs a new [`Transfer`] for a [`Domain`].
    pub fn domain(from: AccountId, domain_id: DomainId, to: AccountId) -> Self {
        Self {
            source: from,
            object: domain_id,
            destination: to,
        }
    }
}

impl Transfer<Account, AssetDefinitionId, Account> {
    /// Constructs a new [`Transfer`] for an [`AssetDefinition`].
    pub fn asset_definition(
        from: AccountId,
        asset_definition_id: AssetDefinitionId,
        to: AccountId,
    ) -> Self {
        Self {
            source: from,
            object: asset_definition_id,
            destination: to,
        }
    }
}

impl Transfer<Asset, Numeric, Account> {
    /// Constructs a new [`Transfer`] for an [`Asset`] of [`Numeric`] type.
    pub fn asset_numeric(asset_id: AssetId, quantity: impl Into<Numeric>, to: AccountId) -> Self {
        Self {
            source: asset_id,
            object: quantity.into(),
            destination: to,
        }
    }
}

impl Transfer<Account, NftId, Account> {
    /// Constructs a new [`Transfer`] for an [`Nft`].
    pub fn nft(from: AccountId, nft_id: NftId, to: AccountId) -> Self {
        Self {
            source: from,
            object: nft_id,
            destination: to,
        }
    }
}

impl_display! {
    Transfer<S, O, D>
    where
        S: Identifiable,
        S::Id: Display,
        O: Display,
        D: Identifiable,
        D::Id: Display,
    =>
    "TRANSFER `{}` FROM `{}` TO `{}`",
    object,
    source,
    destination,
}

impl_into_box! {
    Transfer<Account, DomainId, Account> |
    Transfer<Account, AssetDefinitionId, Account> |
    Transfer<Asset, Numeric, Account> | Transfer<Account, NftId, Account>
=> TransferBox
}

isi_box! {
    /// Enum with all supported [`Transfer`] instructions.
    ///
    /// Dev note: this is an enum that groups concrete `Transfer<_, _, _>`
    /// variants (not a heap box).
    pub enum TransferBox {
        /// Transfer [`Domain`] to another [`Account`].
        Domain(Transfer<Account, DomainId, Account>),
        /// Transfer [`AssetDefinition`] to another [`Account`].
        AssetDefinition(Transfer<Account, AssetDefinitionId, Account>),
        /// Transfer [`Asset`] to another [`Account`].
        Asset(Transfer<Asset, Numeric, Account>),
        /// Transfer [`Nft`] to another [`Account`].
        Nft(Transfer<Account, NftId, Account>),
    }
}

enum_type! {
    pub(crate) enum TransferType {
        Domain,
        AssetDefinition,
        Asset,
        Nft,
    }
}

// Seal implementations
impl crate::seal::Instruction for TransferBox {}
impl crate::seal::Instruction for Transfer<Account, DomainId, Account> {}
impl crate::seal::Instruction for Transfer<Account, AssetDefinitionId, Account> {}
impl crate::seal::Instruction for Transfer<Asset, Numeric, Account> {}
impl crate::seal::Instruction for Transfer<Account, NftId, Account> {}

// Stable wire ID for encoding
impl TransferBox {
    /// Norito wire identifier for boxed transfer instructions.
    pub const WIRE_ID: &'static str = "iroha.transfer";
}

isi! {
    /// Single entry within a [`TransferAssetBatch`] instruction.
    pub struct TransferAssetBatchEntry {
        /// Account sending the asset.
        from: AccountId,
        /// Account receiving the asset.
        to: AccountId,
        /// Asset definition being transferred.
        asset_definition: AssetDefinitionId,
        /// Amount to transfer.
        amount: Numeric,
    }
}

impl TransferAssetBatchEntry {
    /// Construct a new batch entry.
    #[must_use]
    pub fn new(
        from: AccountId,
        to: AccountId,
        asset_definition: AssetDefinitionId,
        amount: impl Into<Numeric>,
    ) -> Self {
        Self {
            from,
            to,
            asset_definition,
            amount: amount.into(),
        }
    }
}

isi! {
    /// Deterministic batch transfer instruction covering multiple `Transfer::asset_numeric` calls.
    pub struct TransferAssetBatch {
        /// Ordered transfer entries executed sequentially.
        entries: Vec<TransferAssetBatchEntry>,
    }
}

impl TransferAssetBatch {
    /// Stable wire identifier for Norito encoding.
    pub const WIRE_ID: &'static str = "iroha.transfer_batch";

    /// Construct a new batch instruction.
    #[must_use]
    pub fn new(entries: Vec<TransferAssetBatchEntry>) -> Self {
        Self { entries }
    }
}

impl crate::seal::Instruction for TransferAssetBatch {}

impl From<TransferAssetBatch> for InstructionBox {
    fn from(instruction: TransferAssetBatch) -> Self {
        InstructionBox(Box::new(instruction))
    }
}

#[cfg(feature = "json")]
impl<S, O, D> FastJsonWrite for Transfer<S, O, D>
where
    S: Identifiable,
    S::Id: JsonSerialize,
    O: JsonSerialize,
    D: Identifiable,
    D::Id: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"source\":");
        JsonSerialize::json_serialize(&self.source, out);
        out.push_str(",\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push_str(",\"destination\":");
        JsonSerialize::json_serialize(&self.destination, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for TransferAssetBatchEntry {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"from\":");
        JsonSerialize::json_serialize(&self.from, out);
        out.push_str(",\"to\":");
        JsonSerialize::json_serialize(&self.to, out);
        out.push_str(",\"asset_definition\":");
        JsonSerialize::json_serialize(&self.asset_definition, out);
        out.push_str(",\"amount\":");
        JsonSerialize::json_serialize(&self.amount, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for TransferAssetBatch {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"entries\":");
        JsonSerialize::json_serialize(&self.entries, out);
        out.push('}');
    }
}
