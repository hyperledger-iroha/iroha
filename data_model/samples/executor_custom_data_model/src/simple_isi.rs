//! Example of one custom instruction.
//! See `ivm/samples/executor_custom_instructions_simple` for the IVM equivalent.

use std::{borrow::ToOwned, format, string::String, vec::Vec};

use iroha_data_model::{
    asset::AssetDefinitionId,
    isi::{CustomInstruction, InstructionBox},
    prelude::{Json, Numeric},
};
use iroha_schema::IntoSchema;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json,
};

#[derive(Debug, IntoSchema)]
pub enum CustomInstructionBox {
    MintAssetForAllAccounts(MintAssetForAllAccounts),
    // Other custom instructions
}

#[derive(Debug, JsonDeserialize, JsonSerialize, IntoSchema)]
pub struct MintAssetForAllAccounts {
    pub asset_definition: AssetDefinitionId,
    pub quantity: Numeric,
}

impl From<MintAssetForAllAccounts> for CustomInstructionBox {
    fn from(isi: MintAssetForAllAccounts) -> Self {
        Self::MintAssetForAllAccounts(isi)
    }
}

// Do not implement the sealed Instruction trait for custom types; wrap into CustomInstruction instead.

impl From<CustomInstructionBox> for CustomInstruction {
    fn from(isi: CustomInstructionBox) -> Self {
        let payload =
            json::to_value(&isi).expect("INTERNAL BUG: Couldn't serialize custom instruction");

        Self::new(payload)
    }
}

impl From<MintAssetForAllAccounts> for InstructionBox {
    fn from(isi: MintAssetForAllAccounts) -> Self {
        InstructionBox::from(CustomInstruction::from(CustomInstructionBox::from(isi)))
    }
}

impl From<CustomInstructionBox> for InstructionBox {
    fn from(isi: CustomInstructionBox) -> Self {
        InstructionBox::from(CustomInstruction::from(isi))
    }
}

impl TryFrom<&Json> for CustomInstructionBox {
    type Error = json::Error;

    fn try_from(payload: &Json) -> Result<Self, json::Error> {
        json::from_str::<Self>(payload.as_ref())
    }
}

impl json::JsonSerialize for CustomInstructionBox {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            Self::MintAssetForAllAccounts(value) => {
                json::write_json_string("MintAssetForAllAccounts", out);
                out.push(':');
                json::JsonSerialize::json_serialize(value, out);
            }
        }
        out.push('}');
    }
}

impl json::JsonDeserialize for CustomInstructionBox {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let mut visitor = json::MapVisitor::new(parser)?;
        let mut variant: Option<Self> = None;

        while let Some(key) = visitor.next_key()? {
            let key_str = key.as_str();
            match key_str {
                "MintAssetForAllAccounts" => {
                    if variant.is_some() {
                        return Err(json::Error::duplicate_field(key_str.to_owned()));
                    }
                    let value = visitor.parse_value::<MintAssetForAllAccounts>()?;
                    variant = Some(Self::MintAssetForAllAccounts(value));
                }
                _ => {
                    visitor.skip_value()?;
                    return Err(json::Error::unknown_field(key_str.to_owned()));
                }
            }
        }

        visitor.finish()?;

        variant.ok_or_else(|| json::Error::missing_field("CustomInstructionBox"))
    }
}
