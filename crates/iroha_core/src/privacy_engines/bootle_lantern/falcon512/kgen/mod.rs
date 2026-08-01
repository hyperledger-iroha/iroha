//! Bounded portable Falcon-512 NTRU key generation.
//!
//! Arithmetic is adapted from `fn-dsa-kgen` 0.3.0 at commit
//! `daf14859b5aa3f8d75c42966ba7de83e6eb59997` (Unlicense).  The semantic
//! delta is an explicit candidate cap, a bounded parity sampler, raw secret
//! polynomial output, and mandatory NTRU/public-key self-checks.

mod fxp;
mod gauss;
mod mp31;
mod ntru;
mod poly;
mod vect;
mod zint31;

use super::{DEGREE, LOG_DEGREE, MODULUS, Trapdoor, comm};
use comm::PRNG as _;
use zeroize::{Zeroize, Zeroizing};

const MAX_PARITY_ATTEMPTS_PER_POLYNOMIAL: u32 = 128;

struct Workspace {
    temporary_u16: Vec<u16>,
    temporary_u32: Vec<u32>,
    temporary_fxr: Vec<fxp::FXR>,
}

impl Workspace {
    fn new() -> Self {
        Self {
            temporary_u16: vec![0; 2 * DEGREE],
            temporary_u32: vec![0; 6 * DEGREE],
            temporary_fxr: vec![fxp::FXR::ZERO; 5 * DEGREE / 2],
        }
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        self.temporary_u16.zeroize();
        self.temporary_u32.zeroize();
        self.temporary_fxr.zeroize();
    }
}

pub(super) fn generate_from_seed(seed: &[u8; 32], max_candidates: u32) -> Option<Trapdoor> {
    if max_candidates == 0 {
        return None;
    }

    let mut workspace = Workspace::new();
    let mut f = Zeroizing::new(Box::new([0_i8; DEGREE]));
    let mut g = Zeroizing::new(Box::new([0_i8; DEGREE]));
    let mut capital_f = Zeroizing::new(Box::new([0_i8; DEGREE]));
    let mut capital_g = Zeroizing::new(Box::new([0_i8; DEGREE]));
    let mut h = Zeroizing::new(Box::new([0_u16; DEGREE]));
    let mut generator = comm::shake::SHAKE256_PRNG::new(seed);

    for _ in 0..max_candidates {
        if !gauss::sample_f_bounded(
            LOG_DEGREE,
            &mut generator,
            f.as_mut(),
            MAX_PARITY_ATTEMPTS_PER_POLYNOMIAL,
        ) || !gauss::sample_f_bounded(
            LOG_DEGREE,
            &mut generator,
            g.as_mut(),
            MAX_PARITY_ATTEMPTS_PER_POLYNOMIAL,
        ) {
            continue;
        }

        let squared_norm = f
            .iter()
            .copied()
            .zip(g.iter().copied())
            .map(|(left, right)| {
                let left = i32::from(left);
                let right = i32::from(right);
                left * left + right * right
            })
            .sum::<i32>();
        if squared_norm >= 16_823 {
            continue;
        }
        if !comm::mq::mqpoly_small_is_invertible(
            LOG_DEGREE,
            f.as_ref(),
            &mut workspace.temporary_u16[..DEGREE],
        ) {
            continue;
        }
        if !ntru::check_ortho_norm(
            LOG_DEGREE,
            f.as_ref(),
            g.as_ref(),
            &mut workspace.temporary_fxr,
        ) {
            continue;
        }
        if !ntru::solve_NTRU(
            LOG_DEGREE,
            f.as_ref(),
            g.as_ref(),
            capital_f.as_mut(),
            capital_g.as_mut(),
            &mut workspace.temporary_u32,
            &mut workspace.temporary_fxr,
        ) {
            continue;
        }

        let (division_temporary, _) = workspace.temporary_u16.split_at_mut(DEGREE);
        comm::mq::mqpoly_div_small(
            LOG_DEGREE,
            f.as_ref(),
            g.as_ref(),
            h.as_mut(),
            division_temporary,
        );
        if ntru_equation_holds(f.as_ref(), g.as_ref(), capital_f.as_ref(), capital_g.as_ref())
            && public_key_equation_holds(f.as_ref(), g.as_ref(), h.as_ref())
        {
            return Some(Trapdoor {
                f,
                g,
                capital_f,
                capital_g,
                h,
            });
        }
    }
    None
}

fn ntru_equation_holds(
    f: &[i8; DEGREE],
    g: &[i8; DEGREE],
    capital_f: &[i8; DEGREE],
    capital_g: &[i8; DEGREE],
) -> bool {
    let mut equation = Zeroizing::new(Box::new([0_i64; DEGREE]));
    negacyclic_accumulate_i8(equation.as_mut(), f, capital_g, 1);
    negacyclic_accumulate_i8(equation.as_mut(), g, capital_f, -1);
    equation[0] == i64::from(MODULUS) && equation[1..].iter().all(|value| *value == 0)
}

fn public_key_equation_holds(
    f: &[i8; DEGREE],
    g: &[i8; DEGREE],
    h: &[u16; DEGREE],
) -> bool {
    let modulus = i64::from(MODULUS);
    let mut product = Zeroizing::new(Box::new([0_i64; DEGREE]));
    for (left_index, left) in f.iter().copied().enumerate() {
        for (right_index, right) in h.iter().copied().enumerate() {
            let degree = left_index + right_index;
            let (destination, sign) = if degree < DEGREE {
                (degree, 1_i64)
            } else {
                (degree - DEGREE, -1_i64)
            };
            product[destination] += sign * i64::from(left) * i64::from(right);
        }
    }
    for destination in 0..DEGREE {
        if product[destination].rem_euclid(modulus)
            != i64::from(g[destination]).rem_euclid(modulus)
        {
            return false;
        }
    }
    true
}

fn negacyclic_accumulate_i8(
    output: &mut [i64; DEGREE],
    left: &[i8; DEGREE],
    right: &[i8; DEGREE],
    outer_sign: i64,
) {
    for (left_index, left) in left.iter().copied().enumerate() {
        for (right_index, right) in right.iter().copied().enumerate() {
            let degree = left_index + right_index;
            let (destination, wrap_sign) = if degree < DEGREE {
                (degree, 1_i64)
            } else {
                (degree - DEGREE, -1_i64)
            };
            output[destination] +=
                outer_sign * wrap_sign * i64::from(left) * i64::from(right);
        }
    }
}
