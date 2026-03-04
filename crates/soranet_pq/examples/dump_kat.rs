//! Dump deterministic `SoraNet` PQ fixtures for manual inspection.

use soranet_pq::{
    MlDsaSuite, MlKemSuite, generate_mldsa_keypair, generate_mlkem_keypair, sign_mldsa,
};

fn main() {
    dump_mlkem();
    dump_mldsa();
}

fn dump_mlkem() {
    for suite in [
        MlKemSuite::MlKem512,
        MlKemSuite::MlKem768,
        MlKemSuite::MlKem1024,
    ] {
        let keys = generate_mlkem_keypair(suite);
        let (shared, ct) = soranet_pq::encapsulate_mlkem(suite, keys.public_key()).unwrap();
        println!("mlkem suite={suite:?}");
        println!("  pk={}", to_hex(keys.public_key()));
        println!("  sk={}", to_hex(keys.secret_key()));
        println!("  ct={}", to_hex(ct.as_bytes()));
        println!("  ss={}", to_hex(shared.as_bytes()));
    }
}

fn dump_ml_dsa_suite(suite: MlDsaSuite, message: &[u8]) {
    let keys = generate_mldsa_keypair(suite).expect("keypair");
    let sig = sign_mldsa(suite, keys.secret_key(), message).expect("sign");
    println!("mldsa suite={suite:?}");
    println!("  pk={}", to_hex(keys.public_key()));
    println!("  sk={}", to_hex(keys.secret_key()));
    println!("  msg={}", to_hex(message));
    println!("  sig={}", to_hex(sig.as_bytes()));
}

fn dump_mldsa() {
    let message = b"SoraNet PQ fixture";
    dump_ml_dsa_suite(MlDsaSuite::MlDsa44, message);
    dump_ml_dsa_suite(MlDsaSuite::MlDsa65, message);
    dump_ml_dsa_suite(MlDsaSuite::MlDsa87, message);
}

fn to_hex<T: AsRef<[u8]>>(value: T) -> String {
    use std::fmt::Write as _;

    let bytes = value.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut out, "{byte:02x}").expect("write to string");
    }
    out
}
