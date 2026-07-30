#[cfg(all(test, feature = "app_api"))]
pub(crate) mod tests_runtime_handlers {
    // Textual chunks remain in this module, preserving every item namespace.
    include!("lib_runtime_handlers/part_1.rs");
    include!("lib_runtime_handlers/part_2.rs");
    include!("lib_runtime_handlers/part_3.rs");
    include!("lib_runtime_handlers/part_4.rs");
    include!("lib_runtime_handlers/part_5.rs");
    include!("lib_runtime_handlers/part_6.rs");
    include!("lib_runtime_handlers/part_7.rs");
    include!("lib_runtime_handlers/part_8.rs");
    include!("lib_runtime_handlers/part_9.rs");
}
