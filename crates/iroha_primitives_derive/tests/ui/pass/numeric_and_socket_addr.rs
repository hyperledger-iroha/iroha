use iroha_primitives_derive::{numeric, socket_addr};

fn main() {
    let price = numeric!(12.34);
    let port = 1337;

    let _v4 = socket_addr!(127.0.0.1:port);
    let _v6 = socket_addr!([2001:db8::1]:8080);

    let _ = (price, _v4, _v6);
}
