//! Shared ISO 20022 OpenAPI parameter builders.

use norito::json::Value;

use super::{operator_signature_header_parameters, string_query_param};

pub(super) fn operator_parameters(mut parameters: Vec<Value>) -> Vec<Value> {
    parameters.extend(operator_signature_header_parameters());
    parameters
}

pub(super) fn profile_selection_parameters() -> Vec<Value> {
    operator_parameters(vec![string_query_param(
        "profile",
        "Optional ISO bridge rail profile bound into the canonical operator signature.",
    )])
}
