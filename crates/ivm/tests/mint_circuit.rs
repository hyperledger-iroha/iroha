#![cfg(feature = "ivm_zk_tests")]
use ivm::{
    halo2::{ECPoint, MintCircuit, MintPublic, MintWitness},
    pedersen_commit_truncated, poseidon,
};

fn poseidon_hash(inputs: &[u64]) -> u64 {
    if inputs.is_empty() {
        return 0;
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

fn expected_public(w: &MintWitness) -> MintPublic {
    let coin_commitment = poseidon_hash(&[
        w.recipient.x,
        w.recipient.y,
        w.value,
        w.token_id,
        w.serial,
        w.hook_id,
        w.hook_data,
    ]);
    let value_commitment = ECPoint {
        x: pedersen_commit_truncated(w.value, w.value_blind),
        y: 0,
    };
    let token_commitment = poseidon_hash(&[w.token_id, w.token_blind]);
    MintPublic {
        coin_commitment,
        value_commitment,
        token_commitment,
    }
}

#[test]
fn test_mint_circuit_verify_ok() {
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
        coin_commitment: 0, // placeholder will be computed below
        value_commitment: ECPoint { x: 0, y: 0 },
        token_commitment: 0,
    };
    let mut circuit = MintCircuit::new(witness, public);
    // compute expected public values
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
        poseidon_hash(&[circuit.witness.token_id, circuit.witness.token_blind]);
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_mint_circuit_bad_commitment() {
    let witness = MintWitness {
        recipient: ECPoint { x: 1, y: 2 },
        value: 3,
        token_id: 4,
        serial: 5,
        hook_id: 0,
        hook_data: 0,
        value_blind: 6,
        token_blind: 7,
    };
    let public = MintPublic {
        coin_commitment: 123, // incorrect
        value_commitment: ECPoint { x: 0, y: 0 },
        token_commitment: 0,
    };
    let circuit = MintCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}

#[test]
fn test_mint_circuit_bad_value_commitment() {
    let witness = MintWitness {
        recipient: ECPoint { x: 8, y: 9 },
        value: 4,
        token_id: 6,
        serial: 10,
        hook_id: 0,
        hook_data: 0,
        value_blind: 2,
        token_blind: 5,
    };
    let mut public = expected_public(&witness);
    public.value_commitment.x = public.value_commitment.x.wrapping_add(1);
    let circuit = MintCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}

#[test]
fn test_mint_circuit_bad_token_commitment() {
    let witness = MintWitness {
        recipient: ECPoint { x: 3, y: 4 },
        value: 9,
        token_id: 2,
        serial: 14,
        hook_id: 0,
        hook_data: 0,
        value_blind: 1,
        token_blind: 7,
    };
    let mut public = expected_public(&witness);
    public.token_commitment = public.token_commitment.wrapping_add(1);
    let circuit = MintCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}
