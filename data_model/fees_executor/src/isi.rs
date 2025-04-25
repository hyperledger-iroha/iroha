use alloc::{format, string::String, vec::Vec};

use iroha_data_model::{
    isi::{CustomInstruction, Instruction, InstructionBox},
    prelude::*,
};
use iroha_schema::IntoSchema;
use serde::{Deserialize, Serialize};

use crate::parameters::*;

#[derive(Debug, Deserialize, Serialize, IntoSchema)]
pub enum FeesInstructionBox {
    SetDefaultFeesAmountsOptions(SetDefaultFeesAmountsOptions),
    SetAccountFeesAmountsOptions(SetAccountFeesAmountsOptions),
}

impl Instruction for FeesInstructionBox {}

impl From<FeesInstructionBox> for CustomInstruction {
    fn from(isi: FeesInstructionBox) -> Self {
        let payload = serde_json::to_value(&isi).expect(concat!(
            "INTERNAL BUG: Couldn't serialize instruction: ",
            stringify!($isi)
        ));
        Self::new(payload)
    }
}

impl From<FeesInstructionBox> for InstructionBox {
    fn from(isi: FeesInstructionBox) -> Self {
        Self::Custom(isi.into())
    }
}

impl TryFrom<&Json> for FeesInstructionBox {
    type Error = serde_json::Error;

    fn try_from(payload: &Json) -> serde_json::Result<Self> {
        serde_json::from_str::<Self>(payload.as_ref())
    }
}

#[derive(Debug, Deserialize, Serialize, IntoSchema)]
pub struct SetDefaultFeesAmountsOptions(pub FeesAmountsOptions);

#[derive(Debug, Deserialize, Serialize, IntoSchema, derive_more::Constructor)]
pub struct SetAccountFeesAmountsOptions {
    pub account: AccountId,
    pub options: FeesAmountsOptions,
}

macro_rules! generate_isi_impls {
    ($isi:ident) => {
        impl From<$isi> for FeesInstructionBox {
            fn from(isi: $isi) -> Self {
                Self::$isi(isi)
            }
        }

        impl Instruction for $isi {}

        impl From<$isi> for InstructionBox {
            fn from(isi: $isi) -> Self {
                Self::Custom(FeesInstructionBox::from(isi).into())
            }
        }
    };
}

generate_isi_impls!(SetDefaultFeesAmountsOptions);
generate_isi_impls!(SetAccountFeesAmountsOptions);
