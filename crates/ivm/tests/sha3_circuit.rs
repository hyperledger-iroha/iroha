#![cfg(feature = "ivm_zk_tests")]
use ivm::halo2::Sha3BlockCircuit;

fn sha3_compress_ref(state: [u64; 25], block: &[u8; 136]) -> [u64; 25] {
    let mut st = state;
    for i in 0..17 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&block[i * 8..i * 8 + 8]);
        let lane = u64::from_le_bytes(bytes);
        st[i] ^= lane;
    }
    tiny_keccak::keccakf(&mut st);
    st
}

#[test]
fn test_sha3block_known_vector() {
    let block = [0u8; 136];
    let state = [0u64; 25];
    let expected = sha3_compress_ref(state, &block);
    let circuit = Sha3BlockCircuit {
        state,
        block,
        result: expected,
    };
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_sha3block_bad_witness() {
    let block = [0u8; 136];
    let state = [0u64; 25];
    let mut expected = sha3_compress_ref(state, &block);
    expected[0] ^= 1;
    let circuit = Sha3BlockCircuit {
        state,
        block,
        result: expected,
    };
    assert!(circuit.verify().is_err());
}
