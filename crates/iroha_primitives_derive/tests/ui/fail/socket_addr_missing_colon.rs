//! Fails when socket address literal misses the port separator.
use iroha_primitives_derive::socket_addr;

fn main() {
    let _ = socket_addr!(127.0.0.1);
}
