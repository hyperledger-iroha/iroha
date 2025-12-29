//! `socket_addr!` should surface parsing errors.

use iroha_primitives_derive::socket_addr;

fn main() {
    let _ = socket_addr!(127.(0.0.1):8080);
}
