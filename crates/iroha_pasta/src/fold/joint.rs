//! Joint width-two recoding for the two GLV components of a public wallet challenge.
use super::super::{PastaAffine, PastaCurve, Scratch, ba_add, straus};
use ff::Field;
use group::CurveAffine;

#[derive(Clone, Copy)]
pub(super) struct Digit {
    x_row: usize,
    y_row: usize,
    negative: bool,
}

#[derive(Clone, Copy, Default)]
struct Residue {
    a: i8,
    b: i8,
    orbit: usize,
    rotation: usize,
    negative: bool,
}

/// The twelve residue pairs mod 4 with at least one odd coordinate consist of
/// two disjoint orbits under the units ±1, ±φ, ±φ², with φ² + φ + 1 = 0.
/// Use representatives 1 and 1−φ. Multiplication by φ sends (a,b) to (−b,a−b).
fn residues() -> [Residue; 16] {
    let mut table = [Residue::default(); 16];
    for (orbit, (mut a, mut b)) in [(1_i8, 0_i8), (1, -1)].into_iter().enumerate() {
        for rotation in 0..3 {
            for sign in [1, -1] {
                let (x, y) = (sign * a, sign * b);
                let i = ((x & 3) * 4 + (y & 3)) as usize;
                debug_assert_eq!(table[i].a | table[i].b, 0);
                table[i] = Residue {
                    a: x,
                    b: y,
                    orbit,
                    rotation,
                    negative: sign < 0,
                };
            }
            (a, b) = (-b, a - b);
        }
    }
    table
}

/// Emit at most one addition for both coefficients in each pair of bit
/// positions. Subtracting the residue makes both coefficients divisible by four.
pub(super) fn digits(base: usize, mut a: i64, mut b: i64, columns: &mut Vec<Vec<Digit>>) {
    let table = residues();
    let mut bit = 0;
    while a != 0 || b != 0 {
        if (a | b) & 1 != 0 {
            let r = table[((a & 3) * 4 + (b & 3)) as usize];
            if columns.len() <= bit {
                columns.resize_with(bit + 1, Vec::new);
            }
            columns[bit].push(Digit {
                x_row: (3 * r.orbit + r.rotation) * 2 + base,
                y_row: 2 * r.orbit + base,
                negative: r.negative,
            });
            a -= i64::from(r.a);
            b -= i64::from(r.b);
        }
        a /= 2;
        b /= 2;
        bit += 1;
    }
}

/// For each base P prepare P and P−φ(P). Their rotations change only x:
/// the three x coordinates are x, βx and −x−βx. Both bases share the batch
/// inversion for the difference. Nothing is retained beyond the fold.
pub(super) fn fold_block<C: PastaCurve>(
    bases: [&[C::AffineExt]; 2],
    output: &mut [C::AffineExt],
    columns: &[Vec<Digit>],
) -> bool {
    if output
        .iter()
        .chain(bases.iter().flat_map(|base| base.iter()))
        .any(|p| bool::from(p.is_identity()))
    {
        return false;
    }
    let m = output.len();
    let row = 2 * m;
    let mut scratch = Scratch {
        den: vec![C::Base::ZERO; row],
        prefix: vec![C::Base::ZERO; row],
    };
    let beta = C::AffineExt::endo_beta();
    let mut xs = vec![C::Base::ZERO; 6 * row];
    let mut ys = vec![C::Base::ZERO; 2 * row];
    for (i, p) in bases.into_iter().flatten().enumerate() {
        xs[i] = p.x();
        ys[i] = p.y();
        xs[row + i] = xs[i] * beta;
        xs[2 * row + i] = -xs[i] - xs[row + i];
    }
    let mut dx = xs[..row].to_vec();
    let mut dy = ys[..row].to_vec();
    if !ba_add(
        &mut dx,
        &mut dy,
        &xs[row..2 * row],
        &ys[..row],
        true,
        &mut scratch,
    ) {
        return false;
    }
    xs[3 * row..4 * row].copy_from_slice(&dx);
    ys[row..].copy_from_slice(&dy);
    for (i, &x) in dx.iter().enumerate() {
        xs[4 * row + i] = x * beta;
        xs[5 * row + i] = -x - xs[4 * row + i];
    }
    straus::accumulate::<C, _>(
        output,
        columns,
        &mut scratch,
        (&mut dx[..m], &mut dy[..m]),
        |digit| {
            let x = digit.x_row * m;
            let y = digit.y_row * m;
            (&xs[x..x + m], &ys[y..y + m], digit.negative)
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::{Rng, SeedableRng};

    #[test]
    fn joint_digits_reconstruct_signed_components_without_adjacent_additions() {
        let check = |base, a, b| {
            let mut cols = Vec::new();
            digits(base, a, b, &mut cols);
            let mut sum = (0i64, 0i64);
            let mut previous = false;
            for (bit, col) in cols.iter().enumerate() {
                assert!(col.len() <= 1);
                if let Some(d) = col.first() {
                    assert!(!previous);
                    let orbit = d.y_row / 2;
                    assert_eq!(d.y_row % 2, base);
                    assert_eq!(d.x_row % 2, base);
                    assert_eq!(d.x_row / 6, orbit);
                    let (mut x, mut y) = [(1i64, 0i64), (1, -1)][orbit];
                    for _ in 0..d.x_row / 2 % 3 {
                        (x, y) = (-y, x - y);
                    }
                    let power = if d.negative {
                        -(1i64 << bit)
                    } else {
                        1i64 << bit
                    };
                    sum.0 += x * power;
                    sum.1 += y * power;
                }
                previous = !col.is_empty();
            }
            assert_eq!(sum, (a, b));
        };
        for a in -32..=32 {
            for b in -32..=32 {
                check(0, a, b);
            }
        }
        let limit = (1i64 << 33) - 1;
        for a in [-limit, -1, 0, 1, limit] {
            for b in [-limit, -1, 0, 1, limit] {
                check(1, a, b);
            }
        }
        let mut rng = ChaCha20Rng::seed_from_u64(3945);
        for _ in 0..4096 {
            let mut component = || ((rng.next_u64() & ((1 << 34) - 1)) as i64) - (1i64 << 33);
            check(0, component(), component());
        }
    }
}
