//! Emit dual-format account address vectors (ADDR-2a).

use iroha_data_model::account::address::vectors::build_vector_bundle;
use norito::json;

fn main() {
    let bundle = build_vector_bundle();
    let json = json::to_json_pretty(&bundle.to_json_value()).expect("vector bundle must serialize");
    println!("{json}");
}
