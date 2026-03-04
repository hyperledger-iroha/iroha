//! Print canonical oracle kits as Norito JSON for CLI/SDK scaffolding.
//!
//! Run with `cargo run -p iroha_data_model --example oracle_kits` to emit the
//! price and social-follow kits in JSON form.

use iroha_data_model::oracle::kits;
use norito::json;

fn main() {
    print_kit("price_xor_usd", &kits::price_xor_usd());
    print_kit("social_follow_twitter", &kits::twitter_follow_binding());
}

fn print_kit(name: &str, kit: &kits::OracleKit) {
    println!("== {name} feed_config ==");
    println!("{}", pretty(&kit.feed_config));
    println!("== {name} connector_request ==");
    println!("{}", pretty(&kit.connector_request));
    if let Some(observation) = kit.observations.first() {
        println!("== {name} observation ==");
        println!("{}", pretty(observation));
    }
    println!("== {name} report ==");
    println!("{}", pretty(&kit.report));
    println!("== {name} feed_event ==");
    println!("{}", pretty(&kit.feed_event));
    println!();
}

fn pretty<T: json::JsonSerialize>(value: &T) -> String {
    let val = json::to_value(value).expect("serialize");
    json::to_string_pretty(&val).expect("render json")
}
