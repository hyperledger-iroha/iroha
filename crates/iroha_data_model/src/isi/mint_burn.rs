use super::*;
use iroha_primitives::numeric::Quantity;
#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize};
use std::fmt::Display;
isi! {
    /// Generic instruction for a mint of an object to the identifiable destination.
    pub struct Mint<O, D: Identifiable> {
        /// Object which should be minted.
        pub object: O,
        /// Destination object [`Identifiable::Id`].
        pub destination: D::Id,
    }
}
impl Mint<Quantity, Asset> {
    /// Constructs a new [`Mint`] for a non-negative [`Asset`] quantity.
    pub fn asset_quantity(object: impl Into<Quantity>, asset_id: AssetId) -> Self {
        Self {
            object: object.into(),
            destination: asset_id,
        }
    }
}
impl Mint<u32, Trigger> {
    /// Constructs a new [`Mint`] for repetition count of [`Trigger`].
    pub fn trigger_repetitions(repetitions: u32, trigger_id: TriggerId) -> Self {
        Self {
            object: repetitions,
            destination: trigger_id,
        }
    }
}
impl_display! {
    Mint<O, D>
    where
        O: Display,
        D: Identifiable,
        D::Id: Display,
    =>
    "MINT `{}` TO `{}`",
    object,
    destination,
}
impl_into_box! {
    Mint<Quantity, Asset> |
    Mint<u32, Trigger>
=> MintBox
}
isi! {
    /// Generic instruction for a burn of an object to the identifiable destination.
    pub struct Burn<O, D: Identifiable> {
        /// Object which should be burned.
        pub object: O,
        /// Destination object [`Identifiable::Id`].
        pub destination: D::Id,
    }
}
impl Burn<Quantity, Asset> {
    /// Constructs a new [`Burn`] for a non-negative [`Asset`] quantity.
    pub fn asset_quantity(object: impl Into<Quantity>, asset_id: AssetId) -> Self {
        Self {
            object: object.into(),
            destination: asset_id,
        }
    }
}
impl Burn<u32, Trigger> {
    /// Constructs a new [`Burn`] for repetition count of [`Trigger`].
    pub fn trigger_repetitions(repetitions: u32, trigger_id: TriggerId) -> Self {
        Self {
            object: repetitions,
            destination: trigger_id,
        }
    }
}
impl_display! {
    Burn<O, D>
    where
        O: Display,
        D: Identifiable,
        D::Id: Display,
    =>
    "BURN `{}` FROM `{}`",
    object,
    destination,
}
impl_into_box! {
    Burn<Quantity, Asset> |
    Burn<u32, Trigger>
=> BurnBox
}
#[cfg(feature = "json")]
impl<O, D> FastJsonWrite for Mint<O, D>
where
    O: JsonSerialize,
    D: Identifiable,
    D::Id: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push_str(",\"destination\":");
        JsonSerialize::json_serialize(&self.destination, out);
        out.push('}');
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        out.begin_container()?;
        out.push_str("{\"object\":")?;
        JsonSerialize::json_serialize_to(&self.object, out)?;
        out.push_str(",\"destination\":")?;
        JsonSerialize::json_serialize_to(&self.destination, out)?;
        out.push('}')?;
        out.end_container();
        Ok(())
    }
}
#[cfg(feature = "json")]
impl<O, D> FastJsonWrite for Burn<O, D>
where
    O: JsonSerialize,
    D: Identifiable,
    D::Id: JsonSerialize,
{
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"object\":");
        JsonSerialize::json_serialize(&self.object, out);
        out.push_str(",\"destination\":");
        JsonSerialize::json_serialize(&self.destination, out);
        out.push('}');
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        out.begin_container()?;
        out.push_str("{\"object\":")?;
        JsonSerialize::json_serialize_to(&self.object, out)?;
        out.push_str(",\"destination\":")?;
        JsonSerialize::json_serialize_to(&self.destination, out)?;
        out.push('}')?;
        out.end_container();
        Ok(())
    }
}
isi_box! {
    /// Enum with all supported [`Mint`] instructions.
    ///
    /// Dev note: "Box" is naming for a grouped enum, not heap allocation.
    pub enum MintBox {
        /// Mint for [`Asset`].
        Asset(Mint<Quantity, Asset>),
        /// Mint [`Trigger`] repetitions.
        TriggerRepetitions(Mint<u32, Trigger>),
    }
}
enum_type! {
    pub(crate) enum MintType {
        Asset,
        TriggerRepetitions,
    }
}
isi_box! {
    /// Enum with all supported [`Burn`] instructions.
    ///
    /// Dev note: this is a tagged union of concrete `Burn<_, _>` variants.
    pub enum BurnBox {
        /// Burn [`Asset`].
        Asset(Burn<Quantity, Asset>),
        /// Burn [`Trigger`] repetitions.
        TriggerRepetitions(Burn<u32, Trigger>),
    }
}
enum_type! {
    pub(crate) enum BurnType {
        Asset,
        TriggerRepetitions,
    }
}
// Seal implementations
impl crate::seal::Instruction for MintBox {}
impl crate::seal::Instruction for BurnBox {}
impl crate::seal::Instruction for Mint<Quantity, Asset> {}
impl crate::seal::Instruction for Mint<u32, Trigger> {}
impl crate::seal::Instruction for Burn<Quantity, Asset> {}
impl crate::seal::Instruction for Burn<u32, Trigger> {}
macro_rules! impl_mint_burn_slice_decode {
    ($ty:ident) => {
        impl<'a, O, D> norito::core::DecodeFromSlice<'a> for $ty<O, D>
        where
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
                        object,
                        destination,
                    },
                    offset,
                ))
            }
        }
    };
}
impl_mint_burn_slice_decode!(Mint);
impl_mint_burn_slice_decode!(Burn);
impl<'a> norito::core::DecodeFromSlice<'a> for MintBox {
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
            0 => Self::Asset(super::decode_aos_slice_field::<Mint<Quantity, Asset>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            1 => Self::TriggerRepetitions(super::decode_aos_slice_field::<Mint<u32, Trigger>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            _ => {
                return Err(norito::core::Error::Message(format!(
                    "invalid MintBox tag {tag}"
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
impl<'a> norito::core::DecodeFromSlice<'a> for BurnBox {
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
            0 => Self::Asset(super::decode_aos_slice_field::<Burn<Quantity, Asset>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            1 => Self::TriggerRepetitions(super::decode_aos_slice_field::<Burn<u32, Trigger>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?),
            _ => {
                return Err(norito::core::Error::Message(format!(
                    "invalid BurnBox tag {tag}"
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
// Stable wire IDs for encoding
impl MintBox {
    /// Norito wire identifier for boxed mint instructions.
    pub const WIRE_ID: &'static str = "iroha.mint";
}
impl BurnBox {
    /// Norito wire identifier for boxed burn instructions.
    pub const WIRE_ID: &'static str = "iroha.burn";
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::numeric::{Numeric, NumericOperationError};
    use norito::core::DecodeFromSlice;
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked mint/burn fixture account keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn asset_id() -> AssetId {
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "rose".parse().expect("asset name"),
        );
        AssetId::of(definition, account(0x41))
    }
    fn trigger_id() -> TriggerId {
        "nightly_tick".parse().expect("trigger id")
    }
    #[cfg(feature = "json")]
    fn assert_exact_json<T: norito::json::JsonSerialize>(value: &T) {
        let legacy = norito::json::to_json(value).expect("serialize legacy JSON");
        assert_eq!(
            norito::json::to_json_bounded(value, legacy.len()).expect("serialize at exact bound"),
            legacy
        );
        assert_eq!(
            norito::json::to_json_bounded(value, legacy.len() - 1),
            Err(norito::json::BoundedJsonError::BodyTooLarge)
        );
    }
    #[cfg(feature = "json")]
    #[test]
    fn mint_and_burn_json_match_legacy_bytes_at_exact_bounds() {
        assert_exact_json(&Mint::asset_quantity(7_u32, asset_id()));
        assert_exact_json(&Burn::asset_quantity(3_u32, asset_id()));
        assert_exact_json(&Mint::trigger_repetitions(5, trigger_id()));
        assert_exact_json(&Burn::trigger_repetitions(2, trigger_id()));
    }
    #[test]
    fn mint_burn_decode_from_slice_roundtrips_asset_quantity() {
        let mint = Mint::asset_quantity(7_u32, asset_id());
        let mint_bytes = mint.encode();
        let (decoded, used) =
            Mint::<Quantity, Asset>::decode_from_slice(&mint_bytes).expect("decode mint");
        assert_eq!(used, mint_bytes.len());
        assert_eq!(decoded, mint);
        let burn = Burn::asset_quantity(3_u32, asset_id());
        let burn_bytes = burn.encode();
        let (decoded, used) =
            Burn::<Quantity, Asset>::decode_from_slice(&burn_bytes).expect("decode burn");
        assert_eq!(used, burn_bytes.len());
        assert_eq!(decoded, burn);
    }
    #[test]
    fn negative_asset_quantity_is_rejected_before_construction_and_during_decode() {
        let negative = Numeric::new(-1_i32, 2);
        assert_eq!(
            Quantity::try_from_numeric(negative.clone()),
            Err(NumericOperationError::NegativeQuantity)
        );
        // A signed decimal has the same generic field layout, but it is not an
        // asset instruction and must not decode through the nominal boundary.
        let forged_mint = Mint::<Numeric, Asset> {
            object: negative.clone(),
            destination: asset_id(),
        };
        assert!(
            Mint::<Quantity, Asset>::decode_from_slice(&forged_mint.encode()).is_err(),
            "negative signed payload must not decode as an asset mint"
        );
        let forged_burn = Burn::<Numeric, Asset> {
            object: negative,
            destination: asset_id(),
        };
        assert!(
            Burn::<Quantity, Asset>::decode_from_slice(&forged_burn.encode()).is_err(),
            "negative signed payload must not decode as an asset burn"
        );
    }
    #[test]
    fn mint_burn_decode_from_slice_roundtrips_trigger_repetitions() {
        let mint = Mint::trigger_repetitions(5, trigger_id());
        let mint_bytes = mint.encode();
        let (decoded, used) =
            Mint::<u32, Trigger>::decode_from_slice(&mint_bytes).expect("decode trigger mint");
        assert_eq!(used, mint_bytes.len());
        assert_eq!(decoded, mint);
        let burn = Burn::trigger_repetitions(2, trigger_id());
        let burn_bytes = burn.encode();
        let (decoded, used) =
            Burn::<u32, Trigger>::decode_from_slice(&burn_bytes).expect("decode trigger burn");
        assert_eq!(used, burn_bytes.len());
        assert_eq!(decoded, burn);
    }
    #[test]
    fn mint_burn_boxes_registry_decode_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<MintBox>(MintBox::WIRE_ID)
            .register_with_id_slice::<BurnBox>(BurnBox::WIRE_ID);
        let mint_cases = [
            MintBox::Asset(Mint::asset_quantity(7_u32, asset_id())),
            MintBox::TriggerRepetitions(Mint::trigger_repetitions(5, trigger_id())),
        ];
        for value in mint_cases {
            let (payload, flags) = norito::codec::encode_with_header_flags(&value);
            let framed = norito::core::frame_bare_with_header_flags::<MintBox>(&payload, flags)
                .expect("frame mint box");
            let decoded =
                crate::isi::InstructionRegistry::decode(&registry, MintBox::WIRE_ID, &framed)
                    .expect("registered mint box")
                    .expect("decode mint box");
            assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
        }
        let burn_cases = [
            BurnBox::Asset(Burn::asset_quantity(3_u32, asset_id())),
            BurnBox::TriggerRepetitions(Burn::trigger_repetitions(2, trigger_id())),
        ];
        for value in burn_cases {
            let (payload, flags) = norito::codec::encode_with_header_flags(&value);
            let framed = norito::core::frame_bare_with_header_flags::<BurnBox>(&payload, flags)
                .expect("frame burn box");
            let decoded =
                crate::isi::InstructionRegistry::decode(&registry, BurnBox::WIRE_ID, &framed)
                    .expect("registered burn box")
                    .expect("decode burn box");
            assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
        }
    }
}
