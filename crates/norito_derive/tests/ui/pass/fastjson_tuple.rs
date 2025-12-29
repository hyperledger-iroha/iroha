//! pass: FastJsonWrite handles tuple structs.

#[derive(Debug, PartialEq, norito::derive::FastJsonWrite)]
struct Tup(u32, String);

fn main() {
    let tup = Tup(1, "iroha".to_owned());
    let mut out = String::new();
    norito::json::FastJsonWrite::write_json(&tup, &mut out);
    assert_eq!(out, "[1,\"iroha\"]");
}
