//! Token adapters preserve parser and shape diagnostics for every emitter entry.
#[iroha_data_model_derive::model]
struct NotAModule;

iroha_data_model_derive::model_single! {}

#[derive(iroha_data_model_derive::IdEqOrdHash)]
enum NotAnIdentifiedStruct {}

#[derive(iroha_data_model_derive::HasOrigin)]
struct NotAnOriginEnum;

#[derive(iroha_data_model_derive::EventSet)]
struct NotAnEventEnum;

fn main() {}
