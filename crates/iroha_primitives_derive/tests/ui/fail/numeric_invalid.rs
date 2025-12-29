//! `numeric!` should reject malformed literals.

use iroha_primitives_derive::numeric;

fn main() {
    let _ = numeric!(12.3.4);
}
