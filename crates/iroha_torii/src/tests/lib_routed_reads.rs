#[cfg(all(test, feature = "app_api"))]
mod torii_routed_read_tests {
    // Textual chunks remain in this module, preserving every item namespace.
    include!("lib_routed_reads/part_1.rs");
    include!("lib_routed_reads/part_2.rs");
}
