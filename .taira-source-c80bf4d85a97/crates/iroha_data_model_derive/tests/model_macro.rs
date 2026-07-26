//! Tests for the `model` and `model_single` derive macros.

use iroha_data_model_derive::{model, model_single};
use model::PublicItem;

#[model]
mod model {
    /// Public struct exposed by the `model!` macro.
    pub struct PublicItem {
        /// Value used in test assertions.
        pub value: u32,
    }
    struct PrivateItem {
        value: u32,
    }
}

model_single! {
    /// Public struct produced by the `model_single!` helper.
    pub struct SingleItem {
        /// Value used in test assertions.
        pub value: u32,
    }
}

#[test]
fn uses_generated_items() {
    let public_item = PublicItem { value: 10 };
    assert_eq!(public_item.value, 10);

    let single_item = SingleItem { value: 20 };
    assert_eq!(single_item.value, 20);
}
