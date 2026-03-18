#![doc = "//! Feature-gated tests for real Poseidon gadget wiring"]
#![allow(unexpected_cfgs)]
#[cfg(feature = "zk-halo2-ipa-poseidon")]
use ivm::halo2::{ECPoint, MintCircuit, MintPublic, MintWitness, verify_merkle_path};
#[cfg(feature = "zk-halo2-ipa-poseidon")]
use ivm::{pedersen_commit_truncated, poseidon, poseidon2};

#[cfg(feature = "zk-halo2-ipa-poseidon")]
fn poseidon_hash(inputs: &[u64]) -> u64 {
    if inputs.is_empty() {
        return 0;
    }
    if inputs.len() == 1 {
        return poseidon2(inputs[0], 0);
    }
    if inputs.len() == 2 {
        return poseidon2(inputs[0], inputs[1]);
    }

    let mut idx = 0usize;
    let mut acc = 0u64;
    let mut first = true;
    while idx < inputs.len() {
        if first {
            let take = (inputs.len() - idx).min(6);
            let mut state = [0u64; 6];
            state[..take].copy_from_slice(&inputs[idx..idx + take]);
            if take < 6 {
                state[take] = inputs.len() as u64;
            }
            acc = poseidon::poseidon6(state);
            idx += take;
            first = false;
        } else {
            let take = (inputs.len() - idx).min(5);
            let mut state = [0u64; 6];
            state[0] = acc;
            if take > 0 {
                state[1..1 + take].copy_from_slice(&inputs[idx..idx + take]);
            }
            if take < 5 {
                state[take + 1] = (inputs.len() - idx) as u64;
            }
            acc = poseidon::poseidon6(state);
            idx += take;
        }
    }
    acc
}

#[cfg(feature = "zk-halo2-ipa-poseidon")]
#[test]
fn test_mint_circuit_with_real_poseidon() {
    let witness = MintWitness {
        recipient: ECPoint { x: 10, y: 20 },
        value: 5,
        token_id: 7,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        value_blind: 3,
        token_blind: 13,
    };

    let public = MintPublic {
        coin_commitment: 0,
        value_commitment: ECPoint { x: 0, y: 0 },
        token_commitment: 0,
    };

    let mut circuit = MintCircuit::new(witness, public);

    // Compute expected commitments using the real Poseidon compressor wiring
    circuit.public.coin_commitment = poseidon_hash(&[
        circuit.witness.recipient.x,
        circuit.witness.recipient.y,
        circuit.witness.value,
        circuit.witness.token_id,
        circuit.witness.serial,
        circuit.witness.hook_id,
        circuit.witness.hook_data,
    ]);
    circuit.public.value_commitment.x =
        pedersen_commit_truncated(circuit.witness.value, circuit.witness.value_blind);
    circuit.public.token_commitment =
        poseidon2(circuit.witness.token_id, circuit.witness.token_blind);

    assert!(circuit.verify().is_ok());
}

#[cfg(feature = "zk-halo2-ipa-poseidon")]
#[test]
fn test_merkle_with_real_poseidon_pairwise() {
    // Construct a small synthetic path and compute the expected root using
    // the pairwise 2-input Poseidon compressor to mirror the gadget path.
    let mut path = [0u64; 32];
    for i in 0..32u32 {
        path[i as usize] = (i as u64).wrapping_mul(13).wrapping_add(9);
    }
    let leaf = 123u64;
    let index = 0b_00101u32;

    // Expected root via explicit pairwise hashing
    let mut current = leaf;
    for (lvl, sib) in path.iter().enumerate() {
        let bit = (index >> lvl) & 1;
        current = if bit == 0 {
            poseidon2(current, *sib)
        } else {
            poseidon2(*sib, current)
        };
    }

    let root_from_gadget = verify_merkle_path(leaf, index, &path);
    assert_eq!(current, root_from_gadget);
}
