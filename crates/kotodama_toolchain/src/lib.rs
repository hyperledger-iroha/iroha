//! VM-backed Kotodama developer toolchain.
//!
//! Hosts the `koto` CLI/LSP, the `dump_program` inspection tool and the
//! in-process Kotodama test runner. It is kept out of `ivm` so compiler and
//! tooling edits never rebuild the VM or the node.

/// VM-backed Kotodama test runner shared by developer tools.
pub mod koto_test_driver;
