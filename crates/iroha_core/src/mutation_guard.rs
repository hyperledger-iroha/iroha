//! Reject Core mutation features in every non-test library or node build.

#[cfg(all(feature = "mutation-testing", not(test)))]
compile_error!(
    "iroha_core mutation-testing is test-only; use cargo test -p iroha_core --lib, never a node or library build"
);
