//! Resource ceilings for the canonical replay proof and default admitted batch.
//!
//! These bounds size the existing V1 openings; they do not change cryptographic
//! parameters or grant production qualification. Norito framing is checked against
//! the actual proof schema by the prover's maximum-shape regression.

use crate::params::{FASTPQ_FINAL_V1, FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1};

/// Maximum transitions in a default-admitted replay batch.
pub const FASTPQ_DEFAULT_MAX_TRANSITIONS_V1: usize = 256;
/// Maximum columns in an admitted replay AIR row.
pub const FASTPQ_MAX_TRACE_COLUMNS_V1: usize = 512;
/// Evaluations carried by one canonical mixed-trace LDE leaf.
pub const FASTPQ_LDE_CHUNK_VALUES_V1: usize = 64;
/// Boolean, relation, metadata/stable, and integer-transfer AIR challenges.
pub const FASTPQ_REPLAY_AIR_ALPHA_COUNT_V1: usize = 8 + 4 + 6 + 2 + 205;

const DEPTH: usize = FASTPQ_DEFAULT_MAX_TRANSITIONS_V1.ilog2() as usize
    + FASTPQ_FINAL_V1.fri.blowup_factor.ilog2() as usize;
const ROUNDS: usize = DEPTH - FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1.ilog2() as usize;
const QUERIES: usize = FASTPQ_FINAL_V1.fri.queries as usize;
const FP4_BYTES: usize = FASTPQ_FINAL_V1.field.extension_degree as usize * 8;
const DIGEST_BYTES: usize = FASTPQ_FINAL_V1.hash.digest_bytes as usize;
const LDE_PATH: usize = DEPTH - FASTPQ_LDE_CHUNK_VALUES_V1.ilog2() as usize;
const TERMINAL_VALUES: usize = FASTPQ_FRI_TERMINAL_DOMAIN_SIZE_V1 as usize;

/// Maximum unframed payload charged by `VerifyLimits::default()`.
///
/// Derived from 256 transitions, 512 columns, the canonical query count and all
/// mixed/AIR/FRI openings. Batch bytes and individual collection limits still apply.
pub const FASTPQ_DEFAULT_MAX_PROOF_PAYLOAD_BYTES_V1: usize = payload_bound();
/// Maximum complete canonical Norito replay proof frame for the same geometry.
///
/// Includes compact field/element lengths, u64 sequence counts, public-input
/// arrays and the 40-byte frame header. Consumers must not substitute the smaller
/// approximate payload charge for this transport/persistence bound.
pub const FASTPQ_DEFAULT_MAX_PROOF_FRAME_BYTES_V1: usize = frame_bound();

const fn payload_bound() -> usize {
    let fixed = 2
        + FASTPQ_FINAL_V1.name.len()
        + DIGEST_BYTES
        + (16 + 8 + 32 * 5)
        + 4 * DIGEST_BYTES
        + 4
        + 16
        + FASTPQ_REPLAY_AIR_ALPHA_COUNT_V1 * FP4_BYTES
        + ROUNDS * FP4_BYTES
        + (ROUNDS + 1) * DIGEST_BYTES;
    let query = 4 + FP4_BYTES + FASTPQ_LDE_CHUNK_VALUES_V1 * FP4_BYTES + LDE_PATH * DIGEST_BYTES;
    let air = 4 + 2 * FASTPQ_MAX_TRACE_COLUMNS_V1 * 8 + 3 * DEPTH * DIGEST_BYTES + FP4_BYTES;
    let mut fri = 8 + TERMINAL_VALUES * FP4_BYTES + DIGEST_BYTES;
    let mut round = 0;
    while round < ROUNDS {
        fri += 8
            + (FASTPQ_FINAL_V1.fri.arity as usize + 1) * FP4_BYTES
            + (DEPTH - round - 1) * DIGEST_BYTES;
        round += 1;
    }
    fixed + QUERIES * (query + air + fri)
}

// Canonical Norito compact length prefix around a field or sequence element.
const fn prefixed(payload: usize) -> usize {
    let mut remaining = payload;
    let mut length = 1;
    while remaining >= 128 {
        length += 1;
        remaining >>= 7;
    }
    payload + length
}
const fn vector(count: usize, element: usize) -> usize {
    8 + count * prefixed(element)
}
const fn fields(lengths: &[usize]) -> usize {
    let mut result = 0;
    let mut index = 0;
    while index < lengths.len() {
        result += prefixed(lengths[index]);
        index += 1;
    }
    result
}
const fn frame_bound() -> usize {
    let query = fields(&[
        4,
        FP4_BYTES,
        vector(FASTPQ_LDE_CHUNK_VALUES_V1, FP4_BYTES),
        vector(LDE_PATH, DIGEST_BYTES),
    ]);
    let air = fields(&[
        4,
        vector(FASTPQ_MAX_TRACE_COLUMNS_V1, 8),
        vector(FASTPQ_MAX_TRACE_COLUMNS_V1, 8),
        vector(DEPTH, DIGEST_BYTES),
        vector(DEPTH, DIGEST_BYTES),
        FP4_BYTES,
        vector(DEPTH, DIGEST_BYTES),
    ]);
    let mut rounds = 8;
    let mut round = 0;
    while round < ROUNDS {
        rounds += prefixed(fields(&[
            4,
            4,
            vector(FASTPQ_FINAL_V1.fri.arity as usize, FP4_BYTES),
            FP4_BYTES,
            vector(DEPTH - round - 1, DIGEST_BYTES),
        ]));
        round += 1;
    }
    let fri = fields(&[
        4,
        rounds,
        4,
        vector(TERMINAL_VALUES, FP4_BYTES),
        vector(1, DIGEST_BYTES),
    ]);
    // Norito derives encode fixed byte-array fields as raw bytes; each array
    // has one outer field prefix, unlike the generic per-element array codec.
    let public_io = fields(&[16, 8, 32, 32, 32, 32, 32]);
    40 + fields(&[
        2,
        prefixed(FASTPQ_FINAL_V1.name.len()),
        DIGEST_BYTES,
        public_io,
        DIGEST_BYTES,
        DIGEST_BYTES,
        DIGEST_BYTES,
        DIGEST_BYTES,
        4,
        8,
        8,
        vector(FASTPQ_REPLAY_AIR_ALPHA_COUNT_V1, FP4_BYTES),
        vector(ROUNDS, FP4_BYTES),
        vector(ROUNDS + 1, DIGEST_BYTES),
        vector(QUERIES, query),
        vector(QUERIES, air),
        vector(QUERIES, fri),
    ])
}

const _: () = assert!(FASTPQ_DEFAULT_MAX_TRANSITIONS_V1.is_power_of_two());
const _: () = assert!(DEPTH <= FASTPQ_FINAL_V1.lde_log_size as usize);
const _: () = assert!(ROUNDS <= FASTPQ_FINAL_V1.fri.max_reductions as usize);
const _: () =
    assert!(FASTPQ_DEFAULT_MAX_PROOF_FRAME_BYTES_V1 > FASTPQ_DEFAULT_MAX_PROOF_PAYLOAD_BYTES_V1);
