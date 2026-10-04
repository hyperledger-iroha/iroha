//! Reject daemon mutation features in every non-test library or node build.

#[cfg(all(feature = "mutation-testing", not(test)))]
compile_error!(
    "irohad_lib mutation-testing is test-only; use cargo test -p irohad_lib --lib, never a node or library build"
);
