//! Bounded untrusted witness generation for the complete result scan.
//!
//! The integer compression computes advice only. Every output word and the
//! final expected digest are independently enforced by the source circuits.
use super::*;

impl ResultScanCircuit {
    /// Read the completed native witness proposal for early work refusal only.
    /// The complete source scan and its Finish equality remain mandatory.
    pub(crate) fn proposed_digest(&self) -> Result<[u8; 32], Error> {
        if !self.known
            || self.plan != ResultScanPlan::Finish
            || self.cursor != RESULT_SCAN_LEAVES - 1
            || self.context.frame_len > MAX_RESULT_BYTES
            || self.before.processed as usize != RESULT_TAG.len() + self.context.frame_len as usize
        {
            return Err(Error::Synthesis);
        }
        let mut digest = [0; 32];
        for (part, word) in digest.chunks_exact_mut(8).zip(self.before.words) {
            part.copy_from_slice(&word.to_le_bytes());
        }
        digest[31] |= 1;
        Ok(digest)
    }
}

/// Untrusted one-block marked BLAKE2b-256 witness generation, never verification.
/// The consuming circuit must constrain the exact message, compression and mark.
/// # Errors
/// The input exceeds one 128-byte compression block.
pub fn marked_hash_one_block(input: &[u8]) -> Result<[u8; 32], Error> {
    if input.len() > 128 {
        return Err(Error::Synthesis);
    }
    let mut block = [0; 128];
    block[..input.len()].copy_from_slice(input);
    let mut initial = IV;
    initial[0] ^= 0x0101_0020;
    let words = native_compress(initial, &block, input.len() as u64, true);
    let mut digest = [0; 32];
    for (part, word) in digest.chunks_exact_mut(8).zip(words) {
        part.copy_from_slice(&word.to_le_bytes());
    }
    digest[31] |= 1;
    Ok(digest)
}

/// Build all Start, consecutive Absorb, and Finish witnesses for one result.
/// A wrong expected digest is retained and makes the Finish circuit fail;
/// this function never turns a native hash verdict into proof authority.
/// The byte tape is allocated once and shared by exactly 515 leaves. Completed
/// streams use unchanged padding steps through the fixed compression schedule.
/// # Errors
/// Frames above the 65,536-byte cap or invalid bounded tape construction.
pub fn prepare_result_scan(
    frame: &[u8],
    expected: [u8; 32],
) -> Result<Vec<ResultScanCircuit>, Error> {
    if frame.len() > MAX_RESULT_BYTES as usize {
        return Err(Error::Synthesis);
    }
    let tape = Arc::new(ResultTapeWitness::from_frame(&Value::known(
        frame.to_vec(),
    ))?);
    let mut root = None;
    let _ = tape.root().map(|value| root = Some(value));
    let context = ResultScanContext {
        root: root.ok_or(Error::Synthesis)?,
        frame_len: u32::try_from(frame.len()).map_err(|_| Error::Synthesis)?,
        expected,
    };
    let mut initial = IV;
    initial[0] ^= 0x0101_0020;
    let mut state = ScanState {
        words: initial,
        processed: 0,
    };
    let total = RESULT_TAG.len() + frame.len();
    let mut out = Vec::with_capacity(RESULT_SCAN_LEAVES as usize);
    out.push(ResultScanCircuit {
        plan: ResultScanPlan::Start,
        cursor: 0,
        context,
        before: ScanState::default(),
        after: state,
        tape: Arc::clone(&tape),
        known: true,
    });
    let mut input = Vec::with_capacity(total.div_ceil(128) * 128);
    input.extend_from_slice(RESULT_TAG);
    input.extend_from_slice(frame);
    input.resize(total.div_ceil(128) * 128, 0);
    for (index, block) in input.chunks_exact(128).enumerate() {
        let processed = u32::try_from((state.processed as usize + 128).min(total))
            .map_err(|_| Error::Synthesis)?;
        let block: [u8; 128] = core::array::from_fn(|i| block[i]);
        let next = ScanState {
            words: native_compress(
                state.words,
                &block,
                u64::from(processed),
                processed as usize == total,
            ),
            processed,
        };
        out.push(ResultScanCircuit {
            plan: ResultScanPlan::Absorb,
            cursor: u32::try_from(index + 1).map_err(|_| Error::Synthesis)?,
            context,
            before: state,
            after: next,
            tape: Arc::clone(&tape),
            known: true,
        });
        state = next;
    }
    for cursor in u32::try_from(out.len()).map_err(|_| Error::Synthesis)?..=RESULT_SCAN_BLOCKS {
        out.push(ResultScanCircuit {
            plan: ResultScanPlan::Absorb,
            cursor,
            context,
            before: state,
            after: state,
            tape: Arc::clone(&tape),
            known: true,
        });
    }
    out.push(ResultScanCircuit {
        plan: ResultScanPlan::Finish,
        cursor: RESULT_SCAN_LEAVES - 1,
        context,
        before: state,
        after: ScanState::default(),
        tape,
        known: true,
    });
    Ok(out)
}

const IV: [u64; 8] = [
    0x6a09_e667_f3bc_c908,
    0xbb67_ae85_84ca_a73b,
    0x3c6e_f372_fe94_f82b,
    0xa54f_f53a_5f1d_36f1,
    0x510e_527f_ade6_82d1,
    0x9b05_688c_2b3e_6c1f,
    0x1f83_d9ab_fb41_bd6b,
    0x5be0_cd19_137e_2179,
];

const SIGMA: [[usize; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

fn native_compress(mut h: [u64; 8], block: &[u8; 128], counter: u64, last: bool) -> [u64; 8] {
    fn mix(words: &mut [u64; 16], [a, b, c, d]: [usize; 4], left: u64, right: u64) {
        words[a] = words[a].wrapping_add(words[b]).wrapping_add(left);
        words[d] = (words[d] ^ words[a]).rotate_right(32);
        words[c] = words[c].wrapping_add(words[d]);
        words[b] = (words[b] ^ words[c]).rotate_right(24);
        words[a] = words[a].wrapping_add(words[b]).wrapping_add(right);
        words[d] = (words[d] ^ words[a]).rotate_right(16);
        words[c] = words[c].wrapping_add(words[d]);
        words[b] = (words[b] ^ words[c]).rotate_right(63);
    }
    let message: [u64; 16] =
        core::array::from_fn(|i| u64::from_le_bytes(core::array::from_fn(|j| block[i * 8 + j])));
    let mut v = [0; 16];
    v[..8].copy_from_slice(&h);
    v[8..].copy_from_slice(&IV);
    v[12] ^= counter;
    if last {
        v[14] = !v[14];
    }
    for r in 0..12 {
        let s = SIGMA[r % 10];
        mix(&mut v, [0, 4, 8, 12], message[s[0]], message[s[1]]);
        mix(&mut v, [1, 5, 9, 13], message[s[2]], message[s[3]]);
        mix(&mut v, [2, 6, 10, 14], message[s[4]], message[s[5]]);
        mix(&mut v, [3, 7, 11, 15], message[s[6]], message[s[7]]);
        mix(&mut v, [0, 5, 10, 15], message[s[8]], message[s[9]]);
        mix(&mut v, [1, 6, 11, 12], message[s[10]], message[s[11]]);
        mix(&mut v, [2, 7, 8, 13], message[s[12]], message[s[13]]);
        mix(&mut v, [3, 4, 9, 14], message[s[14]], message[s[15]]);
    }
    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
    h
}
