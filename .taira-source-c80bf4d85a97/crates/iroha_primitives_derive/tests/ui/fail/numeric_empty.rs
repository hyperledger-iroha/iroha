//! `numeric!` must reject empty invocations.

use iroha_primitives_derive::numeric;

fn main() {
    let _ = numeric!();
}
