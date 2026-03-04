use std::fmt::Display;

use iroha_primitives::numeric::Numeric;
#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize};

use super::*;

isi! {
    /// Generic instruction for a mint of an object to the identifiable destination.
    pub struct Mint<O, D: Identifiable> {
        /// Object which should be minted.
        pub object: O,
        /// Destination object [`Identifiable::Id`].
        pub destination: D::Id,
    }
}

impl Mint<Numeric, Asset> {
    /// Constructs a new [`Mint`] for an [`Asset`] of [`Numeric`] type.
    pub fn asset_numeric(object: impl Into<Numeric>, asset_id: AssetId) -> Self {
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
    Mint<Numeric, Asset> |
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

impl Burn<Numeric, Asset> {
    /// Constructs a new [`Burn`] for an [`Asset`] of [`Numeric`] type.
    pub fn asset_numeric(object: impl Into<Numeric>, asset_id: AssetId) -> Self {
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
    Burn<Numeric, Asset> |
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
}

isi_box! {
    /// Enum with all supported [`Mint`] instructions.
    ///
    /// Dev note: "Box" is naming for a grouped enum, not heap allocation.
    pub enum MintBox {
        /// Mint for [`Asset`].
        Asset(Mint<Numeric, Asset>),
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
        Asset(Burn<Numeric, Asset>),
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
impl crate::seal::Instruction for Mint<Numeric, Asset> {}
impl crate::seal::Instruction for Mint<u32, Trigger> {}
impl crate::seal::Instruction for Burn<Numeric, Asset> {}
impl crate::seal::Instruction for Burn<u32, Trigger> {}

// Stable wire IDs for encoding
impl MintBox {
    /// Norito wire identifier for boxed mint instructions.
    pub const WIRE_ID: &'static str = "iroha.mint";
}
impl BurnBox {
    /// Norito wire identifier for boxed burn instructions.
    pub const WIRE_ID: &'static str = "iroha.burn";
}
