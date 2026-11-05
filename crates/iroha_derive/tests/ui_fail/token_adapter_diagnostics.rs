//! Token adapters preserve diagnostics from both maintained implementations.
#[iroha_derive::telemetry_future(unexpected)]
struct NotAFunction;

#[derive(iroha_derive::ReadConfig)]
enum NotAConfiguration {
    Item,
}

fn main() {}
