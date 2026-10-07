//! Shared genuine proof builder; contains no registered test cases.
//! Recursive obligation ownership, with genuine Bootstrap state and signed-object
//! composition through sigma, Q, A1, W and A2. Send tests still isolate its frame.

#[path = "../bootstrap.rs"]
mod bootstrap;
#[path = "../bootstrap_objects.rs"]
#[allow(dead_code)] // Shared payer/receiver helpers are consumed by distinct test binaries.
mod bootstrap_objects;
#[path = "../mod.rs"]
mod common;
include!("a_recursive_body.rs");
