//! Observes that kernel scratch holding operand copies is cleared before its memory is released.
//!
//! The crate documents that kernels which copy operands hold the copies in clearing buffers.
//! A statement about dropped memory cannot be checked through the public API, so this test
//! binary installs an allocator that inspects every block of one chosen size as it is freed on
//! the current thread. Each scenario picks operand lengths whose scratch has a size no other
//! allocation of the call shares, arms the probe, runs the kernel and asserts that every such
//! block was all zero when it was released: after a successful call, after a rejected input and
//! after a kernel unwinds.
#![allow(unsafe_code)]

use iroha_fhe::{key_switch, modular, ntt, polynomial, rns};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

thread_local! {
    /// Block size under observation on this thread; zero disarms the probe.
    static WATCHED_SIZE: Cell<usize> = const { Cell::new(0) };
    /// Watched blocks freed since the probe was armed.
    static FREED: Cell<usize> = const { Cell::new(0) };
    /// Watched blocks that still held a non-zero byte when they were freed.
    static FREED_UNCLEARED: Cell<usize> = const { Cell::new(0) };
}

struct FreedBlockProbe;

// SAFETY: every method forwards to `System` with the caller's unchanged arguments. `dealloc`
// additionally reads the block, which is still allocated at that point, and only for the block
// size a test armed; those blocks are vector buffers that were written in full.
unsafe impl GlobalAlloc for FreedBlockProbe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // Thread-local access can fail while a thread is torn down; nothing is watched then.
        let watched = WATCHED_SIZE.try_with(Cell::get).unwrap_or(0);
        if watched != 0 && layout.size() == watched {
            // SAFETY: the block is allocated with this layout until `System.dealloc` below.
            let bytes = unsafe { core::slice::from_raw_parts(pointer, layout.size()) };
            let uncleared = bytes.iter().any(|&byte| byte != 0);
            let _ = FREED.try_with(|count| count.set(count.get() + 1));
            if uncleared {
                let _ = FREED_UNCLEARED.try_with(|count| count.set(count.get() + 1));
            }
        }
        // SAFETY: forwarded with the caller's pointer and layout.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's layout.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: forwarded with the caller's arguments. Growing buffers (the test harness's
        // captured output, for one) therefore never pass through the probe.
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static PROBE: FreedBlockProbe = FreedBlockProbe;

/// Freed watched blocks and how many of them were not cleared.
#[derive(Debug, PartialEq, Eq)]
struct Observation {
    freed: usize,
    uncleared: usize,
}

/// Run `body` with the probe armed for blocks of `size` bytes on this thread.
fn observe<T>(size: usize, body: impl FnOnce() -> T) -> (T, Observation) {
    FREED.with(|count| count.set(0));
    FREED_UNCLEARED.with(|count| count.set(0));
    WATCHED_SIZE.with(|watched| watched.set(size));
    let result = body();
    WATCHED_SIZE.with(|watched| watched.set(0));
    let observation = Observation {
        freed: FREED.with(Cell::get),
        uncleared: FREED_UNCLEARED.with(Cell::get),
    };
    (result, observation)
}

const WORD: usize = size_of::<u64>();
const WIDE: usize = size_of::<i128>();
const MODULUS: u64 = 30_593;

/// Non-zero residues, so an uncleared copy cannot look cleared.
fn secret_polynomial(degree: usize, seed: u64) -> Vec<u64> {
    (0..degree as u64)
        .map(|index| 1 + (seed + index * 7_919) % (MODULUS - 1))
        .collect()
}

#[test]
fn the_probe_sees_cleared_and_uncleared_blocks() {
    let plain = vec![7_u64; 32];
    let ((), observation) = observe(32 * WORD, move || drop(plain));
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 1
        },
        "a plain vector is released with its contents"
    );
    let clearing = zeroize::Zeroizing::new(vec![7_u64; 32]);
    let ((), observation) = observe(32 * WORD, move || drop(clearing));
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        }
    );
    // Blocks of another size are not counted.
    let other = vec![7_u64; 31];
    let ((), observation) = observe(32 * WORD, move || drop(other));
    assert_eq!(
        observation,
        Observation {
            freed: 0,
            uncleared: 0
        }
    );
}

#[test]
fn transform_product_clears_operand_copies_on_success_and_on_rejection() {
    let degree = 32_usize;
    let psi = modular::primitive_root_of_order_with_candidate_limit(MODULUS, 64, 4_096)
        .expect("primitive 64th root");
    let lhs = secret_polynomial(degree, 11);
    let rhs = secret_polynomial(degree, 23);
    // Success: the transform of `rhs` is the only operand copy released; the product leaves.
    let (product, observation) = observe(degree * WORD, || {
        ntt::negacyclic_multiply_ntt(&lhs, &rhs, psi, MODULUS)
    });
    let product = product.expect("product");
    assert_eq!(
        product,
        polynomial::negacyclic_mul_mod_schoolbook(&lhs, &rhs, degree, MODULUS)
    );
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        }
    );
    // Rejected before the transforms: `psi = 0` has no inverse. Both copies are released.
    let (rejected, observation) = observe(degree * WORD, || {
        ntt::negacyclic_multiply_ntt(&lhs, &rhs, 0, MODULUS)
    });
    assert_eq!(rejected, None);
    assert_eq!(
        observation,
        Observation {
            freed: 2,
            uncleared: 0
        }
    );
    // Rejected after the twist: the modulus 2 fails the transform. Both copies are released.
    let (rejected, observation) = observe(degree * WORD, || {
        ntt::negacyclic_multiply_ntt(&lhs, &rhs, 1, 2)
    });
    assert_eq!(rejected, None);
    assert_eq!(
        observation,
        Observation {
            freed: 2,
            uncleared: 0
        }
    );
}

#[test]
fn exact_convolution_clears_every_residue_buffer_on_success_and_on_rejection() {
    let degree = 32_usize;
    let len = 2 * degree;
    let lhs = secret_polynomial(degree, 5);
    let rhs = secret_polynomial(degree, 9);
    // Success: per helper prime the transform of each operand is one `len`-word buffer. The
    // right-hand transforms are released inside the prime loop and the four residue rows after
    // recombination.
    let (linear, observation) = observe(len * WORD, || ntt::convolve_linear_crt_ntt(&lhs, &rhs));
    assert!(linear.is_some());
    assert_eq!(
        observation,
        Observation {
            freed: 2 * ntt::CRT_NTT_PRIMES.len(),
            uncleared: 0
        }
    );
    // The exact linear product is released by the fold; only the folded product leaves.
    let (folded, observation) = observe(len * WIDE, || {
        ntt::negacyclic_product_raw_crt_ntt(&lhs, &rhs)
    });
    assert!(folded.is_some());
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        }
    );
    // Rejected in the fold: 512 full-width coefficients give linear coefficients that do not fit
    // `i128`, first at index 34, so the fold buffer is released partially written. It has the
    // size of a residue row (`2 * degree` words), so the watched blocks are the eight rows and
    // the fold; the exact linear product is watched separately.
    let degree = 512_usize;
    let wide = vec![u64::MAX; degree];
    let (rejected, observation) = observe(degree * WIDE, || {
        ntt::negacyclic_product_raw_crt_ntt(&wide, &wide)
    });
    assert_eq!(rejected, None);
    assert_eq!(
        observation,
        Observation {
            freed: 2 * ntt::CRT_NTT_PRIMES.len() + 1,
            uncleared: 0
        },
        "the residue rows and the fold buffer"
    );
    let (rejected, observation) = observe(2 * degree * WIDE, || {
        ntt::negacyclic_product_raw_crt_ntt(&wide, &wide)
    });
    assert_eq!(rejected, None);
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        },
        "the linear product"
    );
}

#[test]
fn raw_schoolbook_product_clears_widened_operands_on_success_and_on_unwind() {
    // A degree that is not a power of two gives buffer sizes nothing else allocates.
    let degree = 37_usize;
    let lhs = secret_polynomial(degree, 3);
    let rhs = secret_polynomial(degree, 4);
    // Success: both widened operands are released; the accumulator leaves as the product.
    let (product, observation) = observe(degree * WIDE, || {
        polynomial::negacyclic_mul_raw_schoolbook(&lhs, &rhs, degree)
    });
    assert_eq!(product.len(), degree);
    assert_eq!(
        observation,
        Observation {
            freed: 2,
            uncleared: 0
        }
    );
    // Unwind: a left operand three times the degree indexes past the accumulator part-way
    // through the product. The widened right operand and the accumulator share the watched
    // size; the unwinding clears both, and the widened left operand below.
    let oversized = secret_polynomial(3 * degree, 6);
    let (unwound, observation) = observe(degree * WIDE, || {
        catch_unwind(AssertUnwindSafe(|| {
            polynomial::negacyclic_mul_raw_schoolbook(&oversized, &rhs, degree)
        }))
    });
    assert!(unwound.is_err(), "an oversized operand panics");
    assert_eq!(
        observation,
        Observation {
            freed: 2,
            uncleared: 0
        }
    );
    let (unwound, observation) = observe(3 * degree * WIDE, || {
        catch_unwind(AssertUnwindSafe(|| {
            polynomial::negacyclic_mul_raw_schoolbook(&oversized, &rhs, degree)
        }))
    });
    assert!(unwound.is_err());
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        }
    );
}

#[test]
fn rejected_products_and_sums_leave_no_partial_result() {
    // A degree that is not a power of two gives buffer sizes nothing else allocates.
    let degree = 37_usize;
    // Centered product: three terms of (2^63 - 1)^2 overflow the accumulator part-way.
    let half = u64::MAX / 2;
    let wide = vec![half; degree];
    let (rejected, observation) = observe(degree * WIDE, || {
        polynomial::negacyclic_mul_centered_raw(&wide, &wide, degree, u64::MAX)
    });
    assert_eq!(rejected, Err(polynomial::PolynomialError::AdditionOverflow));
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        }
    );
    // Raw sum: the last pair overflows after thirty-six sums were written.
    let mut lhs = vec![5_i128; degree];
    let rhs = vec![7_i128; degree];
    lhs[degree - 1] = i128::MAX;
    let (rejected, observation) =
        observe(degree * WIDE, || polynomial::add_centered_raw(&lhs, &rhs));
    assert_eq!(rejected, Err(polynomial::PolynomialError::AdditionOverflow));
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        }
    );
}

#[test]
fn rejected_rns_conversions_leave_no_partial_limb() {
    let degree = 37_usize;
    let source = [30_593_u64, 30_977];
    let target = [31_489_u64, 31_873, 32_257];
    // The second source limb is one residue short, so every conversion below is rejected at
    // the last coefficient, after thirty-six coefficients were produced.
    let full: Vec<u64> = (0..degree as u64).map(|index| 1 + index * 811).collect();
    let short = full[..degree - 1].to_vec();
    let limbs = vec![full, short];
    let (rejected, observation) = observe(degree * WORD, || {
        rns::basis_extend_target_limbs(&limbs, &source, &target, degree)
    });
    assert_eq!(rejected, Err(rns::RnsError::ShortLimb { limb_index: 1 }));
    assert_eq!(
        observation,
        Observation {
            freed: target.len(),
            uncleared: 0
        },
        "one partially built limb per target modulus"
    );
    let (rejected, observation) = observe(degree * size_of::<u128>(), || {
        rns::reconstruct_polynomial(&limbs, &source, degree)
    });
    assert_eq!(rejected, Err(rns::RnsError::ShortLimb { limb_index: 1 }));
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        }
    );
    // A centered reduction rejects the last coefficient: the limb under construction and the
    // completed limbs are released cleared. Each limb has `degree` words.
    let product = 947_679_361_u128;
    let mut coefficients = vec![product / 3; degree];
    let (reduced, observation) = observe(degree * WORD, || {
        rns::reduce_centered_into_limbs(&coefficients, product, &target)
    });
    let reduced = reduced.expect("canonical coefficients");
    assert_eq!(reduced.len(), target.len());
    assert_eq!(
        observation,
        Observation {
            freed: 0,
            uncleared: 0
        },
        "a successful reduction hands every limb to the caller"
    );
    coefficients[degree - 1] = product;
    let (rejected, observation) = observe(degree * WORD, || {
        rns::reduce_centered_into_limbs(&coefficients, product, &target)
    });
    assert_eq!(rejected, Err(rns::RnsError::CenteredSourceExceedsProduct));
    assert_eq!(
        observation,
        Observation {
            freed: 1,
            uncleared: 0
        },
        "the first limb fails at its last coefficient"
    );
    // The zero modulus is the second target: the completed first limb is released cleared.
    let coefficients = vec![product / 3; degree];
    let (rejected, observation) = observe(degree * WORD, || {
        rns::reduce_into_limbs(&coefficients, &[31_489, 0])
    });
    assert_eq!(rejected, Err(rns::RnsError::ZeroModulus));
    assert_eq!(
        observation,
        Observation {
            freed: 2,
            uncleared: 0
        },
        "the completed first limb and the empty second limb"
    );
}

#[test]
fn key_switch_accumulation_clears_superseded_and_abandoned_sums() {
    let degree = 37_usize;
    let digits = [secret_polynomial(degree, 1), secret_polynomial(degree, 2)];
    let rows = [secret_polynomial(degree, 3), secret_polynomial(degree, 4)];
    let multiply = |digit: &Vec<u64>, row: &Vec<u64>| -> Result<Vec<u64>, ()> {
        Ok(polynomial::negacyclic_mul_mod_schoolbook(
            digit, row, degree, MODULUS,
        ))
    };
    let add = |lhs: &[u64], rhs: &[u64]| -> Result<Vec<u64>, ()> {
        Ok(polynomial::add_mod(lhs, rhs, MODULUS))
    };
    let initial = || (secret_polynomial(degree, 5), secret_polynomial(degree, 6));
    let pairs = [(&rows[0], &rows[1]), (&rows[1], &rows[0])];
    // Success over two digits: two initial accumulators, two superseded sums and four
    // contributions are released; the two final sums leave.
    let (switched, observation) = observe(degree * WORD, || {
        key_switch::digit_inner_product_pair(initial(), &digits, pairs, multiply, add)
    });
    assert!(switched.is_ok());
    assert_eq!(
        observation,
        Observation {
            freed: 8,
            uncleared: 0
        }
    );
    // The third product fails: both accumulators are abandoned with the earlier contributions.
    let mut calls = 0_u32;
    let failing = |digit: &Vec<u64>, row: &Vec<u64>| -> Result<Vec<u64>, u32> {
        calls += 1;
        if calls == 3 {
            return Err(calls);
        }
        Ok(polynomial::negacyclic_mul_mod_schoolbook(
            digit, row, degree, MODULUS,
        ))
    };
    let add = |lhs: &[u64], rhs: &[u64]| -> Result<Vec<u64>, u32> {
        Ok(polynomial::add_mod(lhs, rhs, MODULUS))
    };
    let (rejected, observation) = observe(degree * WORD, || {
        key_switch::digit_inner_product_pair(initial(), &digits, pairs, failing, add)
    });
    assert_eq!(rejected, Err(3));
    assert_eq!(
        observation,
        Observation {
            freed: 6,
            uncleared: 0
        },
        "two initial accumulators, two contributions and the two sums of the first digit"
    );
}
