//! Benchmarks for zero-knowledge operations and instructions in IVM.
use criterion::Criterion;
use ivm::{
    halo2::{
        BurnCircuit, BurnPublic, BurnWitness, ECPoint, MintCircuit, MintPublic, MintWitness,
        compute_nullifier, derive_public_key, verify_merkle_path,
    },
    pedersen_commit_truncated, poseidon2, poseidon6,
};

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
            acc = poseidon6(state);
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
            acc = poseidon6(state);
            idx += take;
        }
    }
    acc
}

fn mint_circuit() -> MintCircuit {
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
    circuit.public.coin_commitment = poseidon_hash(&[
        circuit.witness.recipient.x,
        circuit.witness.recipient.y,
        circuit.witness.value,
        circuit.witness.token_id,
        circuit.witness.serial,
        circuit.witness.hook_id,
        circuit.witness.hook_data,
    ]);
    circuit.public.value_commitment = ECPoint {
        x: pedersen_commit_truncated(circuit.witness.value, circuit.witness.value_blind),
        y: 0,
    };
    circuit.public.token_commitment =
        poseidon_hash(&[circuit.witness.token_id, circuit.witness.token_blind]);
    circuit
}

fn burn_circuit() -> BurnCircuit {
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
    circuit
}

fn bench_mint(c: &mut Criterion) {
    let circuit = mint_circuit();
    c.bench_function("mint_circuit_verify", |b| {
        b.iter(|| {
            circuit.verify().unwrap();
        })
    });
}

fn bench_transfer(c: &mut Criterion) {
    let circuit = burn_circuit();
    c.bench_function("burn_circuit_verify", |b| {
        b.iter(|| {
            circuit.verify().unwrap();
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_mint(&mut c);
    bench_transfer(&mut c);
    c.final_summary();
}
