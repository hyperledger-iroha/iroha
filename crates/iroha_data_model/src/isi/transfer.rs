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

impl<'a, S, O, D> norito::core::DecodeFromSlice<'a> for Transfer<S, O, D>
where
    S: Identifiable,
    S::Id: for<'de> norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
    O: for<'de> norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
    D: Identifiable,
    D::Id: for<'de> norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
    Self: norito::codec::Decode,
{
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let source = super::decode_aos_canonical_field::<S::Id>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let object = super::decode_aos_canonical_field::<O>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let destination = super::decode_aos_canonical_field::<D::Id>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                source,
                object,
                destination,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for TransferBox {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
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
            0 => Self::Domain(super::decode_aos_slice_field::<
                Transfer<Account, DomainId, Account>,
            >(
                super::read_aos_field(bytes, &mut offset, flags)?, flags
            )?),
            1 => Self::AssetDefinition(super::decode_aos_slice_field::<
                Transfer<Account, AssetDefinitionId, Account>,
            >(
                super::read_aos_field(bytes, &mut offset, flags)?, flags
            )?),
            2 => Self::Asset(super::decode_aos_slice_field::<
                Transfer<Asset, Numeric, Account>,
            >(
                super::read_aos_field(bytes, &mut offset, flags)?, flags
            )?),
            3 => Self::Nft(super::decode_aos_slice_field::<
                Transfer<Account, NftId, Account>,
            >(
                super::read_aos_field(bytes, &mut offset, flags)?, flags
            )?),
            _ => {
                return Err(norito::core::Error::Message(format!(
                    "invalid TransferBox tag {tag}"
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

impl<'a> norito::core::DecodeFromSlice<'a> for TransferAssetBatchEntry {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let from = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let to = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let asset_definition = super::decode_aos_canonical_field::<AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let amount = super::decode_aos_canonical_field::<Numeric>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                from,
                to,
                asset_definition,
                amount,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for TransferAssetBatch {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let entries = super::decode_aos_slice_field::<Vec<TransferAssetBatchEntry>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { entries }, offset))
    }
}

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

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "rose".parse().expect("asset name"),
        )
    }

    #[test]
    fn transfer_decode_from_slice_roundtrips_asset_numeric() {
        let from = account(0x11);
        let to = account(0x22);
        let asset_id = AssetId::of(asset_definition(), from);
        let transfer = Transfer::asset_numeric(asset_id, 7_u32, to);
        let bytes = transfer.encode();

        let (decoded, used) =
            Transfer::<Asset, Numeric, Account>::decode_from_slice(&bytes).expect("decode");

        assert_eq!(used, bytes.len());
        assert_eq!(decoded, transfer);
    }

    #[test]
    fn transfer_box_registry_decodes_stable_id_from_misaligned_payload() {
        let from = account(0x33);
        let to = account(0x44);
        let asset_id = AssetId::of(asset_definition(), from);
        let transfer_box = TransferBox::Asset(Transfer::asset_numeric(asset_id, 11_u32, to));
        let (payload, flags) = norito::codec::encode_with_header_flags(&transfer_box);
        let framed = norito::core::frame_bare_with_header_flags::<TransferBox>(&payload, flags)
            .expect("frame transfer box");
        let mut misaligned = Vec::with_capacity(framed.len() + 1);
        misaligned.push(0xAA);
        misaligned.extend_from_slice(&framed);
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<TransferBox>(TransferBox::WIRE_ID);

        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            TransferBox::WIRE_ID,
            &misaligned[1..],
        )
        .expect("registered transfer box")
        .expect("decode transfer box");

        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn transfer_asset_batch_decode_from_slice_roundtrips() {
        let from = account(0x55);
        let to = account(0x66);
        let definition = asset_definition();
        let batch = TransferAssetBatch::new(vec![
            TransferAssetBatchEntry::new(from.clone(), to.clone(), definition.clone(), 1_u32),
            TransferAssetBatchEntry::new(to, from, definition, 2_u32),
        ]);
        let bytes = batch.encode();

        let (decoded, used) = TransferAssetBatch::decode_from_slice(&bytes).expect("decode batch");

        assert_eq!(used, bytes.len());
        assert_eq!(decoded, batch);
    }

    #[test]
    fn transfer_asset_batch_registry_decodes_stable_id() {
        let from = account(0x77);
        let to = account(0x88);
        let batch = TransferAssetBatch::new(vec![TransferAssetBatchEntry::new(
            from,
            to,
            asset_definition(),
            3_u32,
        )]);
        let (payload, flags) = norito::codec::encode_with_header_flags(&batch);
        let framed =
            norito::core::frame_bare_with_header_flags::<TransferAssetBatch>(&payload, flags)
                .expect("frame batch");
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<TransferAssetBatch>(TransferAssetBatch::WIRE_ID);

        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            TransferAssetBatch::WIRE_ID,
            &framed,
        )
        .expect("registered transfer batch")
        .expect("decode transfer batch");

        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }
}
