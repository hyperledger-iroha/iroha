#[cfg(all(test, feature = "app_api"))]
mod tx_query_integration_smoke {
    // Textual chunks remain in this module, preserving every item namespace.
    include!("routing_tx_query_smoke/part_1.rs");
    include!("routing_tx_query_smoke/part_2.rs");
}
