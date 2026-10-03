//! pass: macro-forwarded Option types retain required-key semantics.
use norito::derive::{JsonDeserialize, JsonSerialize};

macro_rules! required_records {
    ($ty:ty) => {
        #[derive(JsonDeserialize, JsonSerialize)]
        struct RequiredMacroStruct {
            #[norito(required)]
            value: $ty,
            optional: $ty,
        }

        #[derive(norito_derive::FastJson)]
        struct FastRequiredMacroStruct {
            #[norito(required)]
            value: $ty,
            optional: $ty,
        }

        #[derive(JsonDeserialize, JsonSerialize)]
        #[norito(tag = "kind", content = "payload")]
        enum RequiredMacroEnum {
            Value {
                #[norito(required)]
                value: $ty,
            },
        }
    };
}

required_records!(::core::option::Option<u32>);

fn main() {}
