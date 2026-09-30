//! Post-base-commit Fp4 multiset products with polynomial coordinate constraints.

use super::{E, Error, F, PHASES, TransparentTranscriptV1, packet};

pub(super) const WIDTH: usize = 16;
const N_BEFORE: usize = 0;
const N_AFTER: usize = 4;
const D_BEFORE: usize = 8;
const D_AFTER: usize = 12;
pub(super) const CONSTRAINTS: usize = 28;

/// Challenges are sampled only after all ordered/sorted base coordinates commit.
#[derive(Clone)]
pub(super) struct Challenges {
    alpha: E,
    gamma: E,
}

impl Challenges {
    pub(super) fn derive(transcript: &mut TransparentTranscriptV1) -> Result<Self, Error> {
        Ok(Self {
            alpha: transcript
                .challenge_fp4(b"ivm-machine-bus-alpha-v1")
                .map_err(|_| Error::Transcript)?,
            gamma: transcript
                .challenge_fp4(b"ivm-machine-bus-gamma-v1")
                .map_err(|_| Error::Transcript)?,
        })
    }

    #[cfg(test)]
    pub(super) fn testing(alpha: E, gamma: E) -> Self {
        Self { alpha, gamma }
    }

    fn factor(&self, packet: &[F]) -> E {
        let mut power = E::ONE;
        let mut fingerprint = self.gamma;
        for value in packet {
            fingerprint = fingerprint.add(power.mul_base(*value));
            power = power.mul(self.alpha);
        }
        E::ONE.add(fingerprint.sub(E::ONE).mul_base(packet[packet::ENABLED]))
    }
}

fn read(row: &[F], offset: usize) -> E {
    E::from_coefficients(
        row[offset..offset + 4]
            .try_into()
            .expect("four coordinates"),
    )
    .expect("the native field evaluator supplies canonical residues")
}

pub(super) fn columns(
    ordered: &[Vec<F>],
    sorted: &[Vec<F>],
    challenges: &Challenges,
    rows: usize,
) -> Result<Vec<Vec<F>>, Error> {
    if ordered.len() != packet::WIDTH
        || sorted.len() != packet::WIDTH
        || ordered
            .iter()
            .chain(sorted)
            .any(|column| column.len() != rows)
    {
        return Err(Error::InvalidTrace);
    }
    let mut columns = vec![vec![F::ZERO; rows]; WIDTH];
    let mut numerator = E::ONE;
    let mut denominator = E::ONE;
    for index in 0..rows {
        let n_before = numerator;
        let d_before = denominator;
        if index % PHASES == PHASES - 1 {
            let a: [F; packet::WIDTH] = std::array::from_fn(|column| ordered[column][index]);
            let b: [F; packet::WIDTH] = std::array::from_fn(|column| sorted[column][index]);
            numerator = numerator.mul(challenges.factor(&a));
            denominator = denominator.mul(challenges.factor(&b));
        }
        for (offset, value) in [
            (N_BEFORE, n_before),
            (N_AFTER, numerator),
            (D_BEFORE, d_before),
            (D_AFTER, denominator),
        ] {
            for (coordinate, value) in value.coefficients().into_iter().enumerate() {
                columns[offset + coordinate][index] = value;
            }
        }
    }
    Ok(columns)
}

/// Every coordinate is an explicit base-field polynomial; no product equality
/// supplied by the witness is accepted without all intermediate multiplications.
pub(super) fn append_residues(
    out: &mut Vec<F>,
    row: &[F],
    next: &[F],
    ordered: &[F],
    sorted: &[F],
    masks: [F; 4],
    challenges: &Challenges,
) {
    let [advance, transition, first, last] = masks;
    for (before, after, packet) in [(N_BEFORE, N_AFTER, ordered), (D_BEFORE, D_AFTER, sorted)] {
        let current = read(row, before);
        let result = read(row, after);
        let expected = current.add(
            current
                .mul(challenges.factor(packet).sub(E::ONE))
                .mul_base(advance),
        );
        out.extend(result.sub(expected).coefficients());
        out.extend(
            read(next, before)
                .sub(result)
                .mul_base(transition)
                .coefficients(),
        );
        out.extend(current.sub(E::ONE).mul_base(first).coefficients());
    }
    out.extend(
        read(row, N_AFTER)
            .sub(read(row, D_AFTER))
            .mul_base(last)
            .coefficients(),
    );
}
