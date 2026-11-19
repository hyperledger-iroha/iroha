//! Bounded-integer execution STARK: transparent proofs of state-transition traces under
//! verifier-reconstructed public inputs.
//!
//! A step function (for example one game tick) is described with a [`circuit::Builder`]:
//! registers for the carried state, public columns for inputs every verifier rebuilds from public
//! data (player controls, schedules), and bounded-integer gadgets (products, selects, comparisons,
//! divisions by constants, one-hot tables, range checks). The builder yields both the AIR and the
//! witness program, so the step semantics are written once. This is not a general zkVM, and there
//! is no witness privacy: traces are not masked.
//!
//! The statement "`steps` steps take `initial` to `final_state` (through an optional checkpoint)
//! under these public columns" is proved with a DEEP-ALI STARK over Goldilocks with
//! quartic-extension challenges, BLAKE2b-384 Merkle commitments and FRI. [`Params::standard`] sizes
//! its query phase for 129 bits under the proven Johnson-regime bound (129.522 raw bits at the
//! smallest trace, or 128.522 after the per-round margin). The full budget also depends on the
//! circuit shape. Native 384-bit hashes remove the former 126.5-bit hash ceiling; callers enforce
//! the actual circuit's required budget with [`Params::check_security`]. See [`params`].
//!
//! Proving uses a trace with one row per step and proceeds through these stages:
//!
//! 1. witness generation (rows in parallel, cross-checked against the caller's own simulation);
//! 2. main commitment, LogUp challenge, LogUp commitment, composition quotient commitment;
//! 3. one out-of-domain point, DEEP batching of every committed polynomial, FRI, grinding, queries.
//!
//! Application circuits live beside the engine; [`games::race_v1`] is the SORA CARS Stock race.
//!
//! TODO: GPU (Metal/CUDA) LDE and Merkle backends; multi-row step scheduling and separate
//! multi-height tables to shrink wide traces' proofs.

pub mod air;
pub mod circuit;
pub mod field;
pub mod fri;
pub mod games;
pub mod hash;
pub mod merkle;
pub mod ntt;
pub mod params;
pub mod proof;
pub mod prover;
pub mod trace;
pub mod transcript;
pub mod verifier;

pub use circuit::{Builder, Circuit, Expr};
pub use params::{Params, ParamsError};
pub use proof::{Checkpoint, Proof, Statement, max_proof_bytes};
pub use prover::{ProveError, check_trace, circuit_id, prove};
pub use verifier::{VerifyError, verify};

#[cfg(test)]
mod tests {
    use super::*;

    /// A toy "kart" with one register set: position, velocity, energy, crossing flag.
    fn toy_circuit(log_rows: u32) -> Circuit {
        let mut b = Builder::new("toy kart v1", log_rows);
        let throttle = b.public("throttle", -3, 3);
        let wind = b.public("wind", -50, 50);
        let position = b.register("position", -1_000_000, 1_000_000);
        let velocity = b.register("velocity", -400, 400);
        let energy = b.register("energy", 0, 1_000);
        let crossed = b.register("crossed", 0, 1);

        let boost_ok = b.ge(&energy, &Expr::constant(25), "boost ok");
        let push = b.mul(
            &throttle,
            &Expr::constant(10).add(&boost_ok.scale(5)),
            "push",
        );
        let drag = b.div_trunc(&velocity.scale(7), 8, "drag");
        let gust = b.mul(&wind, &velocity, "gust");
        let gust_force = b.div_trunc(&gust, 120, "gust force");
        let raw = drag.add(&push).add(&gust_force);
        let new_velocity = b.clamp(
            &raw,
            &Expr::constant(-400),
            &Expr::constant(400),
            "velocity clamp",
        );
        let new_energy_raw = energy.add(&Expr::constant(3)).sub(&boost_ok.scale(10));
        let new_energy_raw = b.narrow(&new_energy_raw, 0, 1_003, "energy floor");
        let new_energy = b.min(&new_energy_raw, &Expr::constant(1_000), "energy cap");
        let next_position_raw = position.add(&new_velocity);
        let next_position = b.clamp(
            &next_position_raw,
            &Expr::constant(-1_000_000),
            &Expr::constant(1_000_000),
            "position clamp",
        );
        let (cell, _) = b.div_rem_floor(&next_position.offset(1_000_000), 250_000, "cell");
        let hot = b.one_hot(&cell, 0, 8, "cell one-hot");
        let bonus = hot.select_table(&[0, 1, 0, 2, 0, 1, 0, 3, 0]);
        let beyond = b.gt(&next_position, &Expr::constant(500_000), "beyond");
        let new_crossed = b.or(&crossed, &beyond, "crossed");
        b.assert_nonzero_when(&b.active(), &energy.offset(1), "energy never -1");
        let energy_with_bonus = b.min(&new_energy.add(&bonus), &Expr::constant(1_000), "bonus cap");
        b.set_next(&position, &next_position);
        b.set_next(&velocity, &new_velocity);
        b.set_next(&energy, &energy_with_bonus);
        b.set_next(&crossed, &new_crossed);
        b.finish()
    }

    fn div_trunc(a: i64, c: i64) -> i64 {
        a / c
    }

    fn toy_step(state: &[i64], throttle: i64, wind: i64) -> Vec<i64> {
        let (position, velocity, energy, crossed) = (state[0], state[1], state[2], state[3]);
        let boost_ok = i64::from(energy >= 25);
        let push = throttle * (10 + 5 * boost_ok);
        let raw = div_trunc(velocity * 7, 8) + push + div_trunc(wind * velocity, 120);
        let new_velocity = raw.clamp(-400, 400);
        let new_energy = (energy + 3 - 10 * boost_ok).min(1_000);
        let next_position = (position + new_velocity).clamp(-1_000_000, 1_000_000);
        let cell = (next_position + 1_000_000).div_euclid(250_000);
        let bonus = [0, 1, 0, 2, 0, 1, 0, 3, 0][cell as usize];
        let new_crossed = i64::from(crossed == 1 || next_position > 500_000);
        vec![
            next_position,
            new_velocity,
            (new_energy + bonus).min(1_000),
            new_crossed,
        ]
    }

    fn toy_run(steps: usize, rows: usize) -> (Statement, Vec<Vec<i64>>, Vec<Vec<i64>>) {
        let throttle: Vec<i64> = (0..rows)
            .map(|i| {
                if i < steps {
                    [3, 3, 2, -1, 0, 3, -3][i % 7]
                } else {
                    0
                }
            })
            .collect();
        let wind: Vec<i64> = (0..rows)
            .map(|i| ((i64::try_from(i).unwrap() * 37) % 101) - 50)
            .collect();
        let mut states = vec![vec![0, 0, 500, 0]];
        for i in 0..steps {
            let next = toy_step(states.last().unwrap(), throttle[i], wind[i]);
            states.push(next);
        }
        let statement = Statement {
            steps,
            initial: states[0].clone(),
            final_state: states[steps].clone(),
            checkpoint: None,
            binding: b"toy replay".to_vec(),
        };
        (statement, vec![throttle, wind], states)
    }

    fn fast_params() -> Params {
        Params {
            log_blowup: 3,
            queries: 24,
            fri_log_arity: 3,
            fri_log_final_degree: 3,
            grinding_bits: 4,
        }
    }

    #[test]
    fn toy_game_proves_and_verifies() {
        let circuit = toy_circuit(8);
        let (statement, public, states) = toy_run(200, circuit.rows());
        check_trace(&circuit, &statement, &public, &states).unwrap();
        let params = fast_params();
        let proof = prove(&circuit, &params, &statement, &public, &states).unwrap();
        verify(&circuit, &params, &statement, &public, &proof).unwrap();
        let bytes = proof.to_bytes();
        assert_eq!(Proof::from_bytes(&bytes).unwrap(), proof);
        assert!(Proof::from_bytes(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn toy_game_rejects_wrong_statements_and_tampering() {
        let circuit = toy_circuit(8);
        let (statement, public, states) = toy_run(150, circuit.rows());
        let params = fast_params();
        let proof = prove(&circuit, &params, &statement, &public, &states).unwrap();

        let mut wrong_final = statement.clone();
        wrong_final.final_state[0] += 1;
        assert!(verify(&circuit, &params, &wrong_final, &public, &proof).is_err());
        let mut wrong_steps = statement.clone();
        wrong_steps.steps -= 1;
        assert!(verify(&circuit, &params, &wrong_steps, &public, &proof).is_err());
        let mut wrong_input = public.clone();
        wrong_input[0][17] = -wrong_input[0][17] + 1;
        assert!(verify(&circuit, &params, &statement, &wrong_input, &proof).is_err());
        let mut wrong_binding = statement.clone();
        wrong_binding.binding.push(0);
        assert!(verify(&circuit, &params, &wrong_binding, &public, &proof).is_err());

        let mut bad = proof.clone();
        bad.ood_main[3] += Fp4::ONE;
        assert!(verify(&circuit, &params, &statement, &public, &bad).is_err());
        let mut bad = proof.clone();
        bad.main_rows[0][2] += Fp::ONE;
        assert!(verify(&circuit, &params, &statement, &public, &bad).is_err());
        let mut bad = proof.clone();
        bad.quotient_rows[1][0] += Fp::ONE;
        assert!(verify(&circuit, &params, &statement, &public, &bad).is_err());
        let mut bad = proof.clone();
        bad.pow_nonce ^= 1;
        assert!(verify(&circuit, &params, &statement, &public, &bad).is_err());
        let mut bad = proof.clone();
        bad.fri.final_coefficients[0] += Fp4::ONE;
        assert!(verify(&circuit, &params, &statement, &public, &bad).is_err());
    }

    #[test]
    fn all_384_bit_commitments_survive_the_codec_and_bind_verification() {
        let circuit = toy_circuit(8);
        let (statement, public, states) = toy_run(150, circuit.rows());
        let params = fast_params();
        let proof = prove(&circuit, &params, &statement, &public, &states).unwrap();
        verify(&circuit, &params, &statement, &public, &proof).unwrap();
        let last = crate::hash::DIGEST_BYTES - 1;
        assert!(
            last > 31,
            "the suite must not truncate to the retired width"
        );
        let reject = |bad: Proof| {
            let decoded = Proof::from_bytes(&bad.to_bytes()).unwrap();
            assert_eq!(decoded, bad, "all digest bytes survive canonical encoding");
            assert!(verify(&circuit, &params, &statement, &public, &decoded).is_err());
        };
        let mut bad = proof.clone();
        bad.main_root[last] ^= 1;
        reject(bad);
        let mut bad = proof.clone();
        bad.aux_root.as_mut().unwrap()[last] ^= 1;
        reject(bad);
        let mut bad = proof.clone();
        bad.quotient_root[last] ^= 1;
        reject(bad);
        let mut bad = proof.clone();
        bad.fri.roots[0][last] ^= 1;
        reject(bad);
        let mut bad = proof.clone();
        bad.main_siblings[0][last] ^= 1;
        reject(bad);
        let mut bad = proof.clone();
        bad.aux_siblings[0][last] ^= 1;
        reject(bad);
        let mut bad = proof;
        bad.quotient_siblings[0][last] ^= 1;
        reject(bad);
    }

    #[test]
    fn checkpoints_bind_an_intermediate_state() {
        let circuit = toy_circuit(8);
        let (mut statement, public, states) = toy_run(120, circuit.rows());
        let params = fast_params();
        statement.checkpoint = Some(Checkpoint {
            step: 50,
            registers: states[50].clone(),
        });
        let proof = prove(&circuit, &params, &statement, &public, &states).unwrap();
        verify(&circuit, &params, &statement, &public, &proof).unwrap();
        assert!(proof.to_bytes().len() <= max_proof_bytes(&circuit, &params).unwrap());

        // Another state at the same step, the same state at another step, or no checkpoint at
        // all are different statements.
        let mut wrong = statement.clone();
        wrong.checkpoint.as_mut().unwrap().registers[0] += 1;
        assert!(verify(&circuit, &params, &wrong, &public, &proof).is_err());
        assert_eq!(
            prove(&circuit, &params, &wrong, &public, &states),
            Err(ProveError::Boundary)
        );
        let mut moved = statement.clone();
        moved.checkpoint = Some(Checkpoint {
            step: 51,
            registers: states[50].clone(),
        });
        assert!(verify(&circuit, &params, &moved, &public, &proof).is_err());
        let mut none = statement.clone();
        none.checkpoint = None;
        assert!(verify(&circuit, &params, &none, &public, &proof).is_err());
        let mut beyond = statement.clone();
        beyond.checkpoint = Some(Checkpoint {
            step: 121,
            registers: states[120].clone(),
        });
        assert!(verify(&circuit, &params, &beyond, &public, &proof).is_err());
    }

    /// Impossible parameter/domain combinations are errors at both entry points, never panics.
    #[test]
    fn impossible_parameters_are_rejected_before_any_work() {
        use crate::params::ParamsError;
        let circuit = toy_circuit(8);
        let (statement, public, states) = toy_run(100, circuit.rows());
        let valid = prove(&circuit, &fast_params(), &statement, &public, &states).unwrap();
        let cases = [
            // 1,024 queries over a 512-point domain (2^8 rows, blowup 2).
            (
                &circuit,
                Params {
                    log_blowup: 1,
                    queries: 1_024,
                    fri_log_arity: 3,
                    fri_log_final_degree: 3,
                    grinding_bits: 0,
                },
                ParamsError::Queries,
            ),
            // Unsupported values.
            (
                &circuit,
                Params {
                    fri_log_arity: 5,
                    ..fast_params()
                },
                ParamsError::Unsupported,
            ),
        ];
        // 2^9 rows, blowup 2, arity 16, final degree 1: folds would consume 12 bits of a 10-bit domain.
        let tall = toy_circuit(9);
        let folds = Params {
            log_blowup: 1,
            queries: 8,
            fri_log_arity: 4,
            fri_log_final_degree: 0,
            grinding_bits: 0,
        };
        // Degree-5 helpers need a 4n quotient domain, more than blowup 2.
        let mut wide = toy_circuit(8);
        wide.lookups_per_helper = 4;
        let narrow = Params {
            log_blowup: 1,
            queries: 8,
            fri_log_arity: 1,
            fri_log_final_degree: 3,
            grinding_bits: 0,
        };
        // Malformed public circuit dimensions fail before any shift (2^64 rows would overflow).
        let mut huge = toy_circuit(8);
        huge.log_rows = usize::BITS;
        assert_eq!(fast_params().check(&huge), Err(ParamsError::Circuit));
        assert_eq!(
            max_proof_bytes(&huge, &fast_params()),
            Err(ParamsError::Circuit)
        );
        assert_eq!(
            prove(&huge, &fast_params(), &statement, &public, &states),
            Err(ProveError::Parameters(ParamsError::Circuit))
        );
        assert_eq!(
            verify(&huge, &fast_params(), &statement, &public, &valid),
            Err(VerifyError::Parameters(ParamsError::Circuit))
        );
        let blown = Params {
            log_blowup: u32::MAX,
            ..fast_params()
        };
        assert_eq!(
            max_proof_bytes(&circuit, &blown),
            Err(ParamsError::Unsupported)
        );
        for (c, params, expected) in cases.into_iter().chain([
            (&tall, folds, ParamsError::FoldGeometry),
            (&wide, narrow, ParamsError::QuotientBlowup),
        ]) {
            assert_eq!(params.check(c), Err(expected));
            assert_eq!(max_proof_bytes(c, &params), Err(expected));
            let (statement, public, states) = toy_run(100, c.rows());
            assert_eq!(
                prove(c, &params, &statement, &public, &states),
                Err(ProveError::Parameters(expected))
            );
            assert_eq!(
                verify(c, &params, &statement, &public, &valid),
                Err(VerifyError::Parameters(expected))
            );
        }
    }

    /// Multiplicities must be the exact integer lookup counts, not just equal modulo p.
    #[test]
    fn multiplicities_must_be_exact_counts() {
        let circuit = toy_circuit(8);
        let (statement, public, states) = toy_run(150, circuit.rows());
        let params = fast_params();
        let proof = prove(&circuit, &params, &statement, &public, &states).unwrap();
        verify(&circuit, &params, &statement, &public, &proof).unwrap();
        let used = proof
            .multiplicities
            .iter()
            .position(|m| *m > 0)
            .expect("some lookup");
        let unused = proof
            .multiplicities
            .iter()
            .position(|m| *m == 0)
            .expect("some unused table value");
        let mut plus_p = proof.clone();
        plus_p.multiplicities[used] += crate::field::MODULUS;
        let mut mismatch = proof.clone();
        mismatch.multiplicities[unused] += 1;
        let mut overflow = proof.clone();
        overflow.multiplicities[unused] = u64::MAX;
        for (name, bad) in [
            ("count + p", plus_p),
            ("sum mismatch", mismatch),
            ("overflow", overflow),
        ] {
            assert_eq!(
                verify(&circuit, &params, &statement, &public, &bad),
                Err(VerifyError::Multiplicities),
                "{name}"
            );
            // The codec still round-trips it; rejection is the verifier's job.
            assert_eq!(Proof::from_bytes(&bad.to_bytes()).unwrap(), bad);
        }
        // Moving a count keeps the sum but changes the transcript and the LogUp identity.
        let mut moved = proof.clone();
        moved.multiplicities[used] -= 1;
        moved.multiplicities[unused] += 1;
        assert!(verify(&circuit, &params, &statement, &public, &moved).is_err());
    }

    /// Without lookups there is no auxiliary tree, so auxiliary siblings must be absent: unconsumed
    /// data would make proofs malleable and break the size bound.
    #[test]
    fn lookup_free_proofs_carry_no_auxiliary_openings() {
        let mut b = Builder::new("constant register", 8);
        let value = b.register("value", 0, 9);
        b.set_next(&value, &value);
        let circuit = b.finish();
        assert!(circuit.lookups.is_empty());
        let params = fast_params();
        let statement = Statement {
            steps: 10,
            initial: vec![7],
            final_state: vec![7],
            checkpoint: None,
            binding: b"constant".to_vec(),
        };
        let states = vec![vec![7]; 11];
        let proof = prove(&circuit, &params, &statement, &[], &states).unwrap();
        verify(&circuit, &params, &statement, &[], &proof).unwrap();
        let bound = max_proof_bytes(&circuit, &params).unwrap();
        assert!(proof.to_bytes().len() <= bound);
        let mut one = proof.clone();
        one.aux_siblings.push([1; crate::hash::DIGEST_BYTES]);
        assert_eq!(
            verify(&circuit, &params, &statement, &[], &one),
            Err(VerifyError::Shape)
        );
        let mut oversized = proof.clone();
        while oversized.to_bytes().len() <= bound {
            oversized.aux_siblings.push([2; crate::hash::DIGEST_BYTES]);
        }
        assert_eq!(
            verify(&circuit, &params, &statement, &[], &oversized),
            Err(VerifyError::Shape)
        );
        // Multiplicities likewise must be absent without lookups.
        let mut counts = proof;
        counts.multiplicities.push(0);
        assert_eq!(
            verify(&circuit, &params, &statement, &[], &counts),
            Err(VerifyError::Shape)
        );
    }

    #[test]
    fn prover_refuses_a_divergent_simulation() {
        let circuit = toy_circuit(8);
        let (statement, public, mut states) = toy_run(100, circuit.rows());
        states[40][1] += 1;
        assert!(prove(&circuit, &fast_params(), &statement, &public, &states).is_err());
    }

    /// A walk over fixed tables: `sum += a[row mod a.len()] + b[row mod b.len()]`, the keys from checked
    /// remainders of the row index.
    fn table_walk(a: &[i64], b_values: &[i64]) -> Circuit {
        let mut b = Builder::new("table walk v1", 8);
        b.set_lookups_per_helper(2);
        let sum = b.register("sum", -1_000_000, 1_000_000);
        let first = b.fixed_table("first", a);
        let second = b.fixed_table("second", b_values);
        let row = b.step_index();
        let (_, key_a) = b.div_rem_floor(&row, i64::try_from(a.len()).unwrap(), "key a");
        let (_, key_b) = b.div_rem_floor(&row, i64::try_from(b_values.len()).unwrap(), "key b");
        let value_a = b.lookup_table(first, &key_a, "value a");
        let value_b = b.lookup_table(second, &key_b, "value b");
        let next = b.narrow(
            &sum.add(&value_a).add(&value_b),
            -1_000_000,
            1_000_000,
            "sum bound",
        );
        b.set_next(&sum, &next);
        b.finish()
    }

    fn table_walk_run(a: &[i64], b_values: &[i64], steps: usize) -> (Statement, Vec<Vec<i64>>) {
        let mut states = vec![vec![0i64]];
        for row in 0..steps {
            let last = states.last().unwrap()[0];
            states.push(vec![
                last + a[row % a.len()] + b_values[row % b_values.len()],
            ]);
        }
        let statement = Statement {
            steps,
            initial: vec![0],
            final_state: states[steps].clone(),
            checkpoint: None,
            binding: b"table walk".to_vec(),
        };
        (statement, states)
    }

    const WALK_A: [i64; 7] = [3, -7, 12, 0, 5, -7, 9];
    const WALK_B: [i64; 5] = [-30_000, 0, 30_000, 17, -1];

    #[test]
    fn fixed_tables_prove_and_bind_their_rows() {
        let circuit = table_walk(&WALK_A, &WALK_B);
        assert_eq!(circuit.tables.len(), 2);
        assert_eq!(circuit.table_lookups.len(), 2);
        assert_eq!(circuit.table_len(), (1 << 16) + WALK_A.len() + WALK_B.len());
        let (statement, states) = table_walk_run(&WALK_A, &WALK_B, 200);
        check_trace(&circuit, &statement, &[], &states).unwrap();
        let params = fast_params();
        let proof = prove(&circuit, &params, &statement, &[], &states).unwrap();
        verify(&circuit, &params, &statement, &[], &proof).unwrap();
        let bytes = proof.to_bytes();
        assert_eq!(Proof::from_bytes(&bytes).unwrap(), proof);
        assert_eq!(
            Proof::from_bytes_for_circuit(&bytes, &circuit).unwrap(),
            proof
        );
        let shorter_table = table_walk(&WALK_A[1..], &WALK_B);
        assert_eq!(
            Proof::from_bytes_for_circuit(&bytes, &shorter_table),
            Err(proof::DecodeError::Length)
        );
        assert!(bytes.len() <= max_proof_bytes(&circuit, &params).unwrap());
        // Every row reads one row of each table, padding included.
        let fixed = &proof.multiplicities[1 << 16..];
        assert_eq!(fixed.iter().sum::<u64>(), 2 * circuit.rows() as u64);
        assert!(fixed.iter().all(|count| *count > 0));

        // The table contents are part of the circuit: change one value (and the run that reads it) and
        // the old proof no longer verifies.
        let mut changed = WALK_A;
        changed[2] += 1;
        let other = table_walk(&changed, &WALK_B);
        assert_ne!(circuit_id(&other), circuit_id(&circuit));
        assert!(verify(&other, &params, &statement, &[], &proof).is_err());
        let (other_statement, _) = table_walk_run(&changed, &WALK_B, 200);
        assert!(verify(&other, &params, &other_statement, &[], &proof).is_err());
        // A statement that adds up a different table is not proved by this proof.
        assert!(verify(&circuit, &params, &other_statement, &[], &proof).is_err());
    }

    #[test]
    fn fixed_table_counts_are_exact_per_argument() {
        let circuit = table_walk(&WALK_A, &WALK_B);
        let (statement, states) = table_walk_run(&WALK_A, &WALK_B, 150);
        let params = fast_params();
        let proof = prove(&circuit, &params, &statement, &[], &states).unwrap();
        verify(&circuit, &params, &statement, &[], &proof).unwrap();
        let range = 1usize << 16;
        let used_range = proof.multiplicities[..range]
            .iter()
            .position(|m| *m > 0)
            .unwrap();
        // Moving a count between the arguments keeps the overall sum but breaks both arguments' sums.
        let mut across = proof.clone();
        across.multiplicities[used_range] -= 1;
        across.multiplicities[range] += 1;
        assert_eq!(
            verify(&circuit, &params, &statement, &[], &across),
            Err(VerifyError::Multiplicities)
        );
        // Moving a count between two rows of the table argument keeps its sum but not the identity.
        let mut moved = proof.clone();
        moved.multiplicities[range] -= 1;
        moved.multiplicities[range + 1] += 1;
        assert!(verify(&circuit, &params, &statement, &[], &moved).is_err());
        // The table rows' counts must be present.
        let mut short = proof;
        short.multiplicities.truncate(range);
        assert_eq!(
            verify(&circuit, &params, &statement, &[], &short),
            Err(VerifyError::Shape)
        );
    }

    #[test]
    fn honest_prover_refuses_a_value_off_its_table_row() {
        let mut circuit = table_walk(&WALK_A, &WALK_B);
        let (statement, states) = table_walk_run(&WALK_A, &WALK_B, 100);
        // Replace the read of table `first` by a constant that is a value of the table, but not the
        // value of every key: its packed row then names a row that does not exist.
        let column = circuit
            .columns
            .iter()
            .position(|c| c.label == "value a")
            .unwrap();
        circuit.columns[column].hint = crate::circuit::Hint::Quad(crate::circuit::Quad {
            lin: crate::circuit::Lin::constant(12),
            products: Vec::new(),
        });
        assert!(check_trace(&circuit, &statement, &[], &states).is_err());
        assert!(prove(&circuit, &fast_params(), &statement, &[], &states).is_err());
        // With the transition check off, the table lookup itself refuses the row.
        let public = crate::trace::public_columns(&circuit, statement.steps, None, &[]).unwrap();
        let error =
            crate::trace::generate_inner(&circuit, &public, statement.steps, &states, false)
                .unwrap_err();
        assert!(
            matches!(
                error,
                crate::trace::TraceError::LookupOutOfRange { lookup, .. }
                    if lookup >= circuit.lookups.len()
            ),
            "{error}"
        );
    }

    #[test]
    fn packed_rows_decode_to_exactly_one_row() {
        let circuit = table_walk(&WALK_A, &WALK_B);
        let values = circuit.fixed_table_values();
        assert_eq!(values.len(), WALK_A.len() + WALK_B.len());
        for (position, value) in values.iter().enumerate() {
            assert_eq!(circuit.fixed_row_index(*value), Some(position));
            assert!(*value >= 1 << 16, "rows pack above the range values");
            // The neighbouring digits are no row.
            assert_eq!(circuit.fixed_row_index(value + 1), None);
            assert_eq!(circuit.fixed_row_index(value - 1), None);
        }
        assert_eq!(circuit.fixed_row_index(0), None);
        assert_eq!(circuit.fixed_row_index((1 << 16) - 1), None);
        assert_eq!(
            circuit.fixed_row_index(values[values.len() - 1] + (1 << 16)),
            None
        );
        assert_eq!(circuit.fixed_row_index(u64::MAX), None);
        let mut distinct = values.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), values.len());
    }

    #[test]
    fn malformed_fixed_tables_are_rejected_before_any_work() {
        use crate::params::{ParamsError, SecurityShape};
        let circuit = table_walk(&WALK_A, &WALK_B);
        let params = fast_params();
        params.check(&circuit).unwrap();
        assert_eq!(
            SecurityShape::of(&circuit).table,
            (1 << 16) + (WALK_A.len() + WALK_B.len()) as u64
        );
        let mut wrong_min = circuit.clone();
        wrong_min.tables[0].min -= 1;
        let mut empty = circuit.clone();
        empty.tables[1].values.clear();
        let mut wide = circuit.clone();
        wide.tables[1].values[2] = wide.tables[1].min + (1 << 16);
        let mut orphan = circuit;
        orphan.tables.clear();
        for (name, bad) in [
            ("minimum", wrong_min),
            ("empty", empty),
            ("span", wide),
            ("no tables", orphan),
        ] {
            assert_eq!(params.check(&bad), Err(ParamsError::Circuit), "{name}");
        }
    }

    /// The adversarial harness's tables. `A[2] == A[3]`, so a carry out of key 2's offset packs to key 3's
    /// row; `B[0] - min(B) == A[6] - min(A)`, so a carry out of A's last key packs to B's first row.
    const PROBE_A: [i64; 7] = [3, -7, 12, 12, 5, -7, 9];
    const PROBE_B: [i64; 5] = [10, -6, 30, 4, -6];

    /// Reads both tables on every row without using the values, so a forged value changes only the two
    /// lookups of its own read.
    fn table_probe(per: usize) -> Circuit {
        let mut b = Builder::new("table probe v1", 8);
        b.set_lookups_per_helper(per);
        let held = b.register("held", 0, 0);
        let first = b.fixed_table("first", &PROBE_A);
        let second = b.fixed_table("second", &PROBE_B);
        let row = b.step_index();
        let (_, key_a) = b.div_rem_floor(&row, 7, "key a");
        let (_, key_b) = b.div_rem_floor(&row, 5, "key b");
        let _ = b.lookup_table(first, &key_a, "value a");
        let _ = b.lookup_table(second, &key_b, "value b");
        b.set_next(&held, &held);
        b.finish()
    }

    /// How a cheating prover commits the auxiliary columns for a main trace it chose.
    #[derive(Clone, Copy)]
    enum Cheat {
        /// Helpers honestly invert the trace's lookups; each argument's counts stay exact by charging a value
        /// outside its table to that table's first entry.
        Honest,
        /// As `Honest`, but a value of one argument that is an entry of the other is counted there.
        Cross,
        /// As `Honest`, then helper `index` of row `row` absorbs the accumulator's discrepancy, so it closes.
        Close { row: usize, index: usize },
    }

    /// Evaluates every AIR identity (circuit, boundary, both LogUp helper groups and the shared accumulator) on
    /// a main trace with the auxiliary columns `cheat` commits. Returns the failing rows and whether each
    /// argument's counts are still the exact totals the verifier requires.
    fn audit(
        circuit: &Circuit,
        statement: &Statement,
        public: &[Vec<i64>],
        main: &[Vec<Fp>],
        cheat: Cheat,
    ) -> (Vec<usize>, bool) {
        use crate::air::{BetaPowers, CompiledAir};
        let air = CompiledAir::new(
            circuit,
            &statement.initial,
            &statement.final_state,
            &statement.checkpoint_registers(circuit.registers.len()),
        );
        let (n, layout) = (circuit.rows(), air.layout());
        let public_fp: Vec<Vec<Fp>> = public
            .iter()
            .map(|column| column.iter().map(|v| Fp::from_i64(*v)).collect())
            .collect();
        let row_buffer = |k: usize| {
            let mut row = vec![Fp::ZERO; layout.width()];
            for (offset, at) in [(0, k), (layout.row(), (k + 1) % n)] {
                for (c, column) in main.iter().enumerate() {
                    row[offset + c] = column[at];
                }
                for (p, column) in public_fp.iter().enumerate() {
                    row[offset + layout.main + p] = column[at];
                }
            }
            row
        };
        let beta = Fp4([
            Fp::new(0x0123_4567),
            Fp::new(89),
            Fp::new(0xabcd),
            Fp::new(3),
        ]);
        let beta_tables = Fp4([
            Fp::new(0x7654_3210),
            Fp::new(11),
            Fp::new(5),
            Fp::new(0xfeed),
        ]);
        let range = 1u64 << circuit.range_bits;
        let range_lookups = air.range_lookups();
        let mut counts = vec![0u64; circuit.table_len()];
        let mut exact = true;
        let mut helpers = Vec::with_capacity(n);
        let mut values = Vec::new();
        for k in 0..n {
            air.lookup_values(&row_buffer(k), &mut values);
            let mut inverses = Vec::with_capacity(values.len());
            for (i, value) in values.iter().enumerate() {
                let v = value.as_u64();
                let challenge = if i < range_lookups { beta } else { beta_tables };
                inverses.push(
                    (challenge - Fp4::from_base(*value))
                        .inverse()
                        .expect("not a challenge"),
                );
                let in_range = (v < range).then(|| usize::try_from(v).unwrap());
                let in_tables = circuit.fixed_row_index(v).map(|p| p + range as usize);
                let cross = matches!(cheat, Cheat::Cross);
                let slot = if i < range_lookups {
                    in_range.or_else(|| {
                        cross
                            .then_some(in_tables)
                            .flatten()
                            .inspect(|_| exact = false)
                    })
                } else {
                    in_tables.or_else(|| {
                        cross
                            .then_some(in_range)
                            .flatten()
                            .inspect(|_| exact = false)
                    })
                };
                counts[slot.unwrap_or(if i < range_lookups { 0 } else { range as usize })] += 1;
            }
            let mut row_helpers = Vec::new();
            air.helper_values(&inverses, &mut row_helpers);
            helpers.push(row_helpers);
        }
        let n_inverse = Fp::new(n as u64).inverse().unwrap();
        let table_per_row = crate::prover::table_sum(
            &counts,
            &crate::prover::all_table_inverses(beta, beta_tables, circuit),
        )
        .mul_base(n_inverse);
        let accumulate = |helpers: &[Vec<Fp4>]| {
            let mut running = vec![Fp4::ZERO];
            for row in helpers {
                let next = *running.last().unwrap() + row.iter().fold(Fp4::ZERO, |a, h| a + *h)
                    - table_per_row;
                running.push(next);
            }
            running
        };
        let mut accumulator = accumulate(&helpers);
        if let Cheat::Close { row, index } = cheat {
            let discrepancy = accumulator[n];
            helpers[row][index] -= discrepancy;
            accumulator = accumulate(&helpers);
            assert!(accumulator[n].is_zero(), "the accumulator closes");
        }
        let alphas: Vec<Fp4> = (0..air.total_constraints)
            .scan(Fp4::ONE, |power, _| {
                let current = *power;
                *power *= Fp4([Fp::new(31), Fp::new(7), Fp::new(2), Fp::new(9)]);
                Some(current)
            })
            .collect();
        let failing = (0..n)
            .filter(|&k| {
                let row = row_buffer(k);
                let aux: Vec<Fp4> = helpers[k].iter().copied().chain([accumulator[k]]).collect();
                let (every, transition) =
                    air.combine_low(&row, &aux, accumulator[(k + 1) % n], table_per_row, &alphas);
                let helper = air.combine_helpers(
                    &row,
                    &aux,
                    &BetaPowers::new(beta),
                    &BetaPowers::new(beta_tables),
                    &alphas,
                );
                !every.is_zero() || (k + 1 < n && !transition.is_zero()) || !helper.is_zero()
            })
            .collect();
        (failing, exact)
    }

    /// A malicious prover bypasses the honest witness checks: wrong values, offset carries into the next key
    /// or the next table, and lookups counted in the other argument. Every forgery breaks an AIR identity at
    /// every helper width, whether the helpers invert honestly (the accumulator cannot close) or a range or
    /// table helper is bent to close it (that helper's identity fails).
    #[test]
    fn forged_table_reads_break_an_air_identity_at_every_helper_width() {
        let steps = 100;
        let statement = Statement {
            steps,
            initial: vec![0],
            final_state: vec![0],
            checkpoint: None,
            binding: b"table probe".to_vec(),
        };
        for per in 1..=4 {
            let circuit = table_probe(per);
            // Eight range lookups and two table lookups: widths 3 and 4 leave partial helper groups.
            assert_eq!((circuit.lookups.len(), circuit.table_lookups.len()), (8, 2));
            let public = crate::trace::public_columns(&circuit, steps, None, &[]).unwrap();
            let states = vec![vec![0]; steps + 1];
            let honest =
                crate::trace::generate_inner(&circuit, &public, steps, &states, true).unwrap();
            let (failing, exact) = audit(
                &circuit,
                &statement,
                &public,
                &honest.columns,
                Cheat::Honest,
            );
            assert!(
                failing.is_empty() && exact,
                "width {per}: the honest trace satisfies every identity"
            );
            let column = circuit
                .columns
                .iter()
                .position(|c| c.label == "value a")
                .unwrap();
            let (first_table_helper, row_of) =
                (circuit.range_helper_count(), |key: usize| 21 + key);
            let packed = |key: usize, value: i64| {
                let digit = i128::from(value) + 7; // min(A) = -7
                u64::try_from(i128::from(1u64 << 16) + ((key as i128) << 16) + digit).ok()
            };
            for (name, key, forged) in [
                ("wrong value", 1, PROBE_A[4]),
                ("carry into the next key", 2, PROBE_A[2] + (1 << 16)),
                ("carry into the next table", 6, PROBE_A[6] + (1 << 16)),
                ("table row below the range table", 0, 5 - 7 - (1 << 16)),
            ] {
                let row = row_of(key);
                let mut main = honest.columns.clone();
                main[column][row] = Fp::from_i64(forged);
                match name {
                    // Only the range argument can catch a carry: the packed row exists.
                    "carry into the next key" | "carry into the next table" => assert!(
                        packed(key, forged)
                            .and_then(|v| circuit.fixed_row_index(v))
                            .is_some(),
                        "{name}: the carry packs to a real row"
                    ),
                    "table row below the range table" => assert_eq!(packed(key, forged), Some(5)),
                    _ => {}
                }
                for cheat in [
                    Cheat::Honest,
                    Cheat::Cross,
                    Cheat::Close {
                        row,
                        index: first_table_helper,
                    },
                    Cheat::Close { row, index: 0 },
                ] {
                    let (failing, exact) = audit(&circuit, &statement, &public, &main, cheat);
                    assert!(
                        !failing.is_empty(),
                        "width {per}, {name}: a forged read satisfied every identity"
                    );
                    if matches!(cheat, Cheat::Cross) && name == "table row below the range table" {
                        assert!(
                            !exact,
                            "counting a table lookup as a range value breaks the exact counts"
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "beyond the safe integer range")]
    fn table_values_beyond_the_safe_range_are_refused_before_any_arithmetic() {
        let mut b = Builder::new("extreme table", 8);
        let _ = b.fixed_table("extreme", &[i64::MIN, i64::MIN + 1]);
    }

    #[test]
    fn params_refuse_a_table_beyond_the_safe_range() {
        use crate::params::ParamsError;
        let mut circuit = table_walk(&WALK_A, &WALK_B);
        circuit.tables[0].values = vec![i64::MIN, i64::MIN + 1];
        circuit.tables[0].min = i64::MIN;
        assert_eq!(fast_params().check(&circuit), Err(ParamsError::Circuit));
    }

    #[test]
    #[should_panic(expected = "leaves fixed table")]
    fn keys_must_stay_inside_their_table() {
        let mut b = Builder::new("bad key", 8);
        let table = b.fixed_table("short", &[1, 2, 3]);
        let row = b.step_index();
        let _ = b.lookup_table(table, &row, "value");
    }

    use crate::field::{Fp, Fp4};
}
