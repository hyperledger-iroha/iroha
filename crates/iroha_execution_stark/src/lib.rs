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

    use crate::field::{Fp, Fp4};
}
