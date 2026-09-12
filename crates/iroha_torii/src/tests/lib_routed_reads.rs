#[cfg(all(test, feature = "app_api"))]
mod torii_routed_read_tests {
    // Textual chunks remain in this module, preserving every item namespace.
    use iroha_model_base::topology::DataSpaceId;
    use iroha_model_base::topology::LaneId;
    include!("lib_routed_reads/part_1.rs");
    include!("lib_routed_reads/fanout_memory_bounds.rs");
    include!("lib_routed_reads/skipped_responses.rs");
}
