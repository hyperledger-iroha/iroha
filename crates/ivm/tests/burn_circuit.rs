#![cfg(feature = "ivm_zk_tests")]
use ivm::{
    halo2::{
        BurnCircuit, BurnPublic, BurnWitness, ECPoint, compute_nullifier, derive_public_key,
        verify_merkle_path,
    },
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

fn expected_public(w: &BurnWitness) -> BurnPublic {
    let nullifier = compute_nullifier(w.secret_key, w.serial);
    let value_commitment = ECPoint {
        x: pedersen_commit_truncated(w.value, w.value_blind),
        y: 0,
    };
    let token_commitment = poseidon_hash(&[w.token_id, w.token_blind]);
    let pk = derive_public_key(w.secret_key);
    let coin = poseidon_hash(&[
        pk.x,
        pk.y,
        w.value,
        w.token_id,
        w.serial,
        w.hook_id,
        w.hook_data,
    ]);
    let leaf = if w.value == 0 { 0 } else { coin };
    let merkle_root = verify_merkle_path(leaf, w.leaf_index, &w.merkle_path);
    let hook_commitment = poseidon_hash(&[w.hook_data, w.hook_blind]);
    let sig_public = derive_public_key(w.sig_secret);
    BurnPublic {
        nullifier,
        value_commitment,
        token_commitment,
        merkle_root,
        hook_commitment,
        hook_id: w.hook_id,
        sig_public,
    }
}

#[test]
fn test_burn_circuit_verify_ok() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 5,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 0,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let public = BurnPublic {
        nullifier: 0,
        value_commitment: ECPoint { x: 0, y: 0 },
        token_commitment: 0,
        merkle_root: 0,
        hook_commitment: 0,
        hook_id: 0,
        sig_public: ECPoint { x: 0, y: 0 },
    };
    let mut circuit = BurnCircuit::new(witness, public);

    circuit.public.nullifier =
        compute_nullifier(circuit.witness.secret_key, circuit.witness.serial);
    circuit.public.value_commitment = ECPoint {
        x: pedersen_commit_truncated(circuit.witness.value, circuit.witness.value_blind),
        y: 0,
    };
    circuit.public.token_commitment =
        poseidon_hash(&[circuit.witness.token_id, circuit.witness.token_blind]);
    let pk = derive_public_key(circuit.witness.secret_key);
    let coin = poseidon_hash(&[
        pk.x,
        pk.y,
        circuit.witness.value,
        circuit.witness.token_id,
        circuit.witness.serial,
        circuit.witness.hook_id,
        circuit.witness.hook_data,
    ]);
    let leaf = if circuit.witness.value == 0 { 0 } else { coin };
    circuit.public.merkle_root = verify_merkle_path(
        leaf,
        circuit.witness.leaf_index,
        &circuit.witness.merkle_path,
    );
    circuit.public.hook_commitment =
        poseidon_hash(&[circuit.witness.hook_data, circuit.witness.hook_blind]);
    circuit.public.hook_id = circuit.witness.hook_id;
    circuit.public.sig_public = derive_public_key(circuit.witness.sig_secret);

    assert!(circuit.verify().is_ok());
}

#[test]
fn test_burn_circuit_bad_nullifier() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 5,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 0,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let public = BurnPublic {
        nullifier: 123, // incorrect
        value_commitment: ECPoint { x: 0, y: 0 },
        token_commitment: 0,
        merkle_root: 0,
        hook_commitment: 0,
        hook_id: 0,
        sig_public: ECPoint { x: 0, y: 0 },
    };
    let mut circuit = BurnCircuit::new(witness, public);

    // compute correct values for other outputs
    circuit.public.value_commitment = ECPoint {
        x: pedersen_commit_truncated(circuit.witness.value, circuit.witness.value_blind),
        y: 0,
    };
    circuit.public.token_commitment =
        poseidon_hash(&[circuit.witness.token_id, circuit.witness.token_blind]);
    let pk = derive_public_key(circuit.witness.secret_key);
    let coin = poseidon_hash(&[
        pk.x,
        pk.y,
        circuit.witness.value,
        circuit.witness.token_id,
        circuit.witness.serial,
        circuit.witness.hook_id,
        circuit.witness.hook_data,
    ]);
    let leaf = if circuit.witness.value == 0 { 0 } else { coin };
    circuit.public.merkle_root = verify_merkle_path(
        leaf,
        circuit.witness.leaf_index,
        &circuit.witness.merkle_path,
    );
    circuit.public.hook_commitment =
        poseidon_hash(&[circuit.witness.hook_data, circuit.witness.hook_blind]);
    circuit.public.hook_id = circuit.witness.hook_id;
    circuit.public.sig_public = derive_public_key(circuit.witness.sig_secret);

    assert!(circuit.verify().is_err());
}

#[test]
fn test_burn_circuit_bad_signature_key() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 5,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 0,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let public = BurnPublic {
        nullifier: 0,
        value_commitment: ECPoint { x: 0, y: 0 },
        token_commitment: 0,
        merkle_root: 0,
        hook_commitment: 0,
        hook_id: 0,
        sig_public: ECPoint { x: 999, y: 0 }, // incorrect
    };
    let mut circuit = BurnCircuit::new(witness, public);

    circuit.public.nullifier =
        compute_nullifier(circuit.witness.secret_key, circuit.witness.serial);
    circuit.public.value_commitment = ECPoint {
        x: pedersen_commit_truncated(circuit.witness.value, circuit.witness.value_blind),
        y: 0,
    };
    circuit.public.token_commitment =
        poseidon_hash(&[circuit.witness.token_id, circuit.witness.token_blind]);
    let pk = derive_public_key(circuit.witness.secret_key);
    let coin = poseidon_hash(&[
        pk.x,
        pk.y,
        circuit.witness.value,
        circuit.witness.token_id,
        circuit.witness.serial,
        circuit.witness.hook_id,
        circuit.witness.hook_data,
    ]);
    let leaf = if circuit.witness.value == 0 { 0 } else { coin };
    circuit.public.merkle_root = verify_merkle_path(
        leaf,
        circuit.witness.leaf_index,
        &circuit.witness.merkle_path,
    );
    circuit.public.hook_commitment =
        poseidon_hash(&[circuit.witness.hook_data, circuit.witness.hook_blind]);
    circuit.public.hook_id = circuit.witness.hook_id;
    // do not correct sig_public; keep wrong value

    assert!(circuit.verify().is_err());
}

#[test]
fn test_burn_circuit_bad_merkle_root() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 5,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 0,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let mut public = expected_public(&witness);
    public.merkle_root = public.merkle_root.wrapping_add(1);
    let circuit = BurnCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}

#[test]
fn test_burn_circuit_bad_value_commitment() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 5,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 0,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let mut public = expected_public(&witness);
    public.value_commitment.x = public.value_commitment.x.wrapping_add(1);
    let circuit = BurnCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}

#[test]
fn test_burn_circuit_bad_token_commitment() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 5,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 0,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let mut public = expected_public(&witness);
    public.token_commitment = public.token_commitment.wrapping_add(1);
    let circuit = BurnCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}

#[test]
fn test_burn_circuit_bad_hook_commitment() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 5,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 0,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let mut public = expected_public(&witness);
    public.hook_commitment = public.hook_commitment.wrapping_add(1);
    let circuit = BurnCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}

#[test]
fn test_burn_circuit_hook_id_mismatch() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 5,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 0,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let mut public = expected_public(&witness);
    public.hook_id = 1; // wrong ID
    let circuit = BurnCircuit::new(witness, public);
    assert!(circuit.verify().is_err());
}

#[test]
fn test_burn_circuit_zero_value_dummy() {
    let witness = BurnWitness {
        secret_key: 9,
        value: 0,
        token_id: 7,
        value_blind: 3,
        token_blind: 13,
        serial: 11,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 21,
        leaf_index: 5,
        merkle_path: [0u64; 32],
        sig_secret: 4,
    };
    let public = expected_public(&witness);
    let circuit = BurnCircuit::new(witness, public);
    assert!(circuit.verify().is_ok());
}

#[test]
fn test_burn_circuit_large_index_parity() {
    // Exercise large leaf_index with a nontrivial path to ensure bit handling
    // remains correct and deterministic.
    let mut path = [0u64; 32];
    for i in 0..32u32 {
        path[i as usize] = (i as u64).wrapping_mul(13).wrapping_add(29);
    }
    let witness = BurnWitness {
        secret_key: 42,
        value: 7,
        token_id: 9,
        value_blind: 5,
        token_blind: 17,
        serial: 3,
        hook_id: 0,
        hook_data: 0,
        hook_blind: 11,
        leaf_index: u32::MAX, // large index; verifier consumes only 32 bits
        merkle_path: path,
        sig_secret: 8,
    };
    let public = expected_public(&witness);
    let circuit = BurnCircuit::new(witness, public);
    assert!(circuit.verify().is_ok());
}
