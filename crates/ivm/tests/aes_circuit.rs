#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::{AesDecCircuit, AesEncCircuit};

#[test]
fn test_aesenc_known_vector() {
    let state = hex_literal::hex!("00102030405060708090a0b0c0d0e0f0");
    let mut st = [0u8; 16];
    st.copy_from_slice(&state);
    let rk = hex_literal::hex!("d6aa74fdd2af72fadaa678f1d6ab76fe");
    let mut key = [0u8; 16];
    key.copy_from_slice(&rk);
    let expected = ivm::aesenc(st, key);
    let circuit = AesEncCircuit {
        state: st,
        round_key: key,
        result: expected,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_aesdec_inverts() {
    let state = [0x12u8; 16];
    let rk = [0x34u8; 16];
    let enc = ivm::aesenc(state, rk);
    let dec = ivm::aesdec(enc, rk);
    let circuit = AesDecCircuit {
        state: enc,
        round_key: rk,
        result: dec,
    };
    assert!(circuit.verify().is_ok());
    assert_eq!(dec, state);
}

#[test]
fn test_sbox_values() {
    assert_eq!(ivm::sbox(0x00), 0x63);
    assert_eq!(ivm::sbox(0x53), 0xed);
    assert_eq!(ivm::sbox(0xff), 0x16);
}
