//! The STARK verifier.

use crate::air::{BetaPowers, CompiledAir};
use crate::circuit::Circuit;
use crate::field::{Fp, Fp4, GENERATOR};
use crate::fri;
use crate::merkle::{Digest, hash_leaf, verify_batch};
use crate::ntt::{Domain, barycentric_weights, evaluate_with_weights};
use crate::params::Params;
use crate::proof::{Proof, Statement};
use crate::prover::{
    DeepLayout, basis, encode_multiplicities, from_components, initial_transcript, table_inverses,
    table_sum,
};
use crate::trace;

/// Verification failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    /// The statement is malformed or outside the circuit's declared bounds.
    #[error("malformed statement")]
    Statement,
    /// Proof vectors have the wrong lengths.
    #[error("malformed proof")]
    Shape,
    /// The out-of-domain point is invalid.
    #[error("invalid out-of-domain sample")]
    Sample,
    /// The constraint identity fails at the out-of-domain point.
    #[error("composition check failed")]
    Composition,
    /// Proof of work is invalid.
    #[error("invalid proof of work")]
    ProofOfWork,
    /// A Merkle opening is invalid.
    #[error("invalid Merkle opening")]
    Merkle,
    /// The FRI low-degree test failed.
    #[error("FRI check failed")]
    Fri,
    /// The parameters cannot be used with this circuit.
    #[error(transparent)]
    Parameters(#[from] crate::params::ParamsError),
    /// The range-table multiplicities are not the exact lookup counts (each count at most the total
    /// and summing to `rows * lookups`).
    #[error("noncanonical range-table multiplicities")]
    Multiplicities,
}

fn row_digests(rows: &[Vec<Fp>], indices: &[usize]) -> Vec<(usize, Digest)> {
    indices
        .iter()
        .zip(rows)
        .map(|(index, row)| {
            let bytes: Vec<u8> = row.iter().flat_map(|v| v.to_le_bytes()).collect();
            (*index, hash_leaf(&bytes))
        })
        .collect()
}

/// Verifies a proof.
///
/// # Errors
/// Returns the first failed check.
pub fn verify(
    circuit: &Circuit,
    params: &Params,
    statement: &Statement,
    public_inputs: &[Vec<i64>],
    proof: &Proof,
) -> Result<(), VerifyError> {
    params.check(circuit)?;
    let registers = circuit.registers.len();
    if statement.initial.len() != registers || statement.final_state.len() != registers {
        return Err(VerifyError::Statement);
    }
    for (spec, (a, b)) in circuit
        .registers
        .iter()
        .zip(statement.initial.iter().zip(&statement.final_state))
    {
        if *a < spec.lo || *a > spec.hi || *b < spec.lo || *b > spec.hi {
            return Err(VerifyError::Statement);
        }
    }
    if let Some(checkpoint) = &statement.checkpoint {
        if checkpoint.step > statement.steps
            || checkpoint.registers.len() != registers
            || circuit
                .registers
                .iter()
                .zip(&checkpoint.registers)
                .any(|(spec, v)| *v < spec.lo || *v > spec.hi)
        {
            return Err(VerifyError::Statement);
        }
    }
    let public = trace::public_columns(
        circuit,
        statement.steps,
        statement.checkpoint_step(),
        public_inputs,
    )
    .map_err(|_| VerifyError::Statement)?;
    let air = CompiledAir::new(
        circuit,
        &statement.initial,
        &statement.final_state,
        &statement.checkpoint_registers(registers),
    );
    let log_n = circuit.log_rows;
    let n = circuit.rows();
    let blowup = 1usize << params.log_blowup;
    let log_lde = log_n + params.log_blowup;
    let shift = Fp::new(GENERATOR);
    let has_lookups = !circuit.lookups.is_empty();
    let next_columns = circuit.next_columns();
    let aux_ext = air.aux_columns();
    let shape = air.shape();
    let chunks = shape.chunks();

    // Shapes.
    let queries = params.queries;
    let width_ok = |rows: &[Vec<Fp>], width: usize| {
        rows.len() == queries && rows.iter().all(|r| r.len() == width)
    };
    if proof.ood_main.len() != circuit.columns.len()
        || proof.ood_main_next.len() != next_columns.len()
        || proof.ood_aux.len() != aux_ext
        || proof.ood_aux_next.len() != usize::from(has_lookups)
        || proof.ood_quotient.len() != chunks
        || proof.aux_root.is_some() != has_lookups
        || !width_ok(&proof.main_rows, circuit.columns.len())
        || (has_lookups && !width_ok(&proof.aux_rows, 4 * aux_ext))
        || (!has_lookups && (!proof.aux_rows.is_empty() || !proof.aux_siblings.is_empty()))
        || !width_ok(&proof.quotient_rows, 4 * chunks)
        || proof.multiplicities.len()
            != if has_lookups {
                1usize << circuit.range_bits
            } else {
                0
            }
    {
        return Err(VerifyError::Shape);
    }

    // Multiplicities are exact integer counts: each at most the total and summing to every lookup
    // of every row. `Params::check` keeps that total below the characteristic, so the field sums
    // below equal the integer sums.
    if has_lookups {
        let expected = u128::from(crate::params::lookup_occurrences(circuit)?);
        let mut sum = 0u128;
        for count in &proof.multiplicities {
            let count = u128::from(*count);
            if count > expected {
                return Err(VerifyError::Multiplicities);
            }
            sum += count;
        }
        if sum != expected {
            return Err(VerifyError::Multiplicities);
        }
    }
    let mut transcript = initial_transcript(circuit, params, statement, &public);
    transcript.absorb_digest(b"main-root", &proof.main_root);
    if has_lookups {
        transcript.absorb(
            b"multiplicities",
            &encode_multiplicities(&proof.multiplicities),
        );
    }
    let (beta, table_per_row) = if has_lookups {
        let beta = transcript.challenge_fp4(b"logup-beta");
        let n_inverse = Fp::new(n as u64).inverse().expect("n != 0");
        let total = table_sum(
            &proof.multiplicities,
            &table_inverses(beta, circuit.range_bits),
        );
        (beta, total.mul_base(n_inverse))
    } else {
        (Fp4::ZERO, Fp4::ZERO)
    };
    if let Some(root) = &proof.aux_root {
        transcript.absorb_digest(b"aux-root", root);
    }
    let alpha = transcript.challenge_fp4(b"composition-alpha");
    transcript.absorb_digest(b"quotient-root", &proof.quotient_root);
    let z = transcript.challenge_fp4(b"ood-z");
    let domain = Domain::new(log_n);
    let omega = domain.generator();
    let wz = z.mul_base(omega);
    let lde_size = (n * blowup) as u64;
    let zn = z.pow(n as u64);
    if (zn - Fp4::ONE).is_zero()
        || (z.pow(lde_size) - Fp4::from_base(shift.pow(lde_size))).is_zero()
        || (wz.pow(lde_size) - Fp4::from_base(shift.pow(lde_size))).is_zero()
    {
        return Err(VerifyError::Sample);
    }

    // Composition identity at z.
    let weights_z = barycentric_weights(&domain, z);
    let layout = air.layout();
    let mut row = vec![Fp4::ZERO; layout.width()];
    row[..layout.main].copy_from_slice(&proof.ood_main);
    for (value, column) in proof.ood_main_next.iter().zip(&next_columns) {
        row[layout.row() + *column as usize] = *value;
    }
    for (p, column) in public.iter().enumerate() {
        let values: Vec<Fp> = column.iter().map(|v| Fp::from_i64(*v)).collect();
        row[layout.main + p] = evaluate_with_weights(&values, &weights_z);
    }
    let mut alphas = Vec::with_capacity(air.total_constraints);
    let mut acc = Fp4::ONE;
    for _ in 0..air.total_constraints {
        alphas.push(acc);
        acc *= alpha;
    }
    let accumulator_next = proof.ood_aux_next.first().copied().unwrap_or(Fp4::ZERO);
    let (every, transition) = air.combine_low(
        &row,
        &proof.ood_aux,
        accumulator_next,
        table_per_row,
        &alphas,
    );
    let last_point = Fp4::from_base(domain.element(n - 1));
    let zerofier_inverse = (zn - Fp4::ONE).inverse().ok_or(VerifyError::Sample)?;
    let expected_low = (every + transition * (z - last_point)) * zerofier_inverse;
    let expected_high = if has_lookups {
        air.combine_helpers(&row, &proof.ood_aux, &BetaPowers::new(beta), &alphas)
            * zerofier_inverse
    } else {
        Fp4::ZERO
    };
    let recombine = |chunks: &[Fp4]| {
        let mut claimed = Fp4::ZERO;
        let mut z_power = Fp4::ONE;
        for chunk in chunks {
            claimed += *chunk * z_power;
            z_power *= zn;
        }
        claimed
    };
    let (low_claim, high_claim) = proof.ood_quotient.split_at(shape.low_chunks());
    if recombine(low_claim) != expected_low
        || (has_lookups && recombine(high_claim) != expected_high)
    {
        return Err(VerifyError::Composition);
    }

    for (label, values) in [
        (&b"ood-main"[..], &proof.ood_main),
        (b"ood-main-next", &proof.ood_main_next),
        (b"ood-aux", &proof.ood_aux),
        (b"ood-aux-next", &proof.ood_aux_next),
        (b"ood-quotient", &proof.ood_quotient),
    ] {
        transcript.absorb_fp4s(label, values);
    }
    let gamma = transcript.challenge_fp4(b"deep-gamma");
    let deep = DeepLayout {
        main: circuit.columns.len(),
        aux_ext,
        quotient_ext: chunks,
        next_main: next_columns.clone(),
        next_aux: has_lookups,
    };
    let (gamma_z, gamma_wz) = deep.coefficients(gamma);
    let claim_z = proof
        .ood_main
        .iter()
        .chain(&proof.ood_aux)
        .chain(&proof.ood_quotient)
        .zip(&gamma_z)
        .fold(Fp4::ZERO, |acc, (v, g)| acc + *v * *g);
    let claim_wz = proof
        .ood_main_next
        .iter()
        .chain(&proof.ood_aux_next)
        .zip(&gamma_wz)
        .fold(Fp4::ZERO, |acc, (v, g)| acc + *v * *g);

    let betas = fri::verifier_challenges(&proof.fri, log_n, params, &mut transcript)
        .ok_or(VerifyError::Fri)?;
    if !transcript.check_grind(params.grinding_bits, proof.pow_nonce) {
        return Err(VerifyError::ProofOfWork);
    }
    let query_indices = transcript.challenge_unique_indices(b"queries", params.queries, log_lde);
    let mut sorted = query_indices.clone();
    sorted.sort_unstable();

    // Trace openings.
    if !verify_batch(
        &proof.main_root,
        log_lde,
        &row_digests(&proof.main_rows, &sorted),
        &proof.main_siblings,
    ) {
        return Err(VerifyError::Merkle);
    }
    if let Some(root) = &proof.aux_root {
        if !verify_batch(
            root,
            log_lde,
            &row_digests(&proof.aux_rows, &sorted),
            &proof.aux_siblings,
        ) {
            return Err(VerifyError::Merkle);
        }
    }
    if !verify_batch(
        &proof.quotient_root,
        log_lde,
        &row_digests(&proof.quotient_rows, &sorted),
        &proof.quotient_siblings,
    ) {
        return Err(VerifyError::Merkle);
    }

    // DEEP values at the query points.
    let v = Fp::two_adic_generator(log_lde);
    let mut initial = Vec::with_capacity(query_indices.len());
    for query in &query_indices {
        let position = sorted.binary_search(query).expect("sorted copy");
        let main_row = &proof.main_rows[position];
        let quotient_row = &proof.quotient_rows[position];
        let x = Fp4::from_base(shift * v.pow(*query as u64));
        let mut sum_z = Fp4::ZERO;
        for (value, g) in main_row.iter().zip(&gamma_z) {
            sum_z += g.mul_base(*value);
        }
        let mut sum_wz = Fp4::ZERO;
        for (i, column) in next_columns.iter().enumerate() {
            sum_wz += gamma_wz[i].mul_base(main_row[*column as usize]);
        }
        if has_lookups {
            let aux_row = &proof.aux_rows[position];
            for a in 0..aux_ext {
                let value = from_components(&aux_row[4 * a..4 * a + 4]);
                sum_z += gamma_z[deep.main + a] * value;
            }
            let last = aux_ext - 1;
            let value = from_components(&aux_row[4 * last..4 * last + 4]);
            sum_wz += gamma_wz[next_columns.len()] * value;
        }
        for chunk in 0..chunks {
            let value = from_components(&quotient_row[4 * chunk..4 * chunk + 4]);
            sum_z += gamma_z[deep.main + aux_ext + chunk] * value;
        }
        let (Some(inv_z), Some(inv_wz)) = ((x - z).inverse(), (x - wz).inverse()) else {
            return Err(VerifyError::Sample);
        };
        initial.push((sum_z - claim_z) * inv_z + (sum_wz - claim_wz) * inv_wz);
    }
    let _ = basis;
    if !fri::verify_queries(
        &proof.fri,
        &betas,
        &query_indices,
        &initial,
        log_lde,
        shift,
        params,
    ) {
        return Err(VerifyError::Fri);
    }
    Ok(())
}
