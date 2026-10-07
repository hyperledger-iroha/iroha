//! Untrusted integer witness generation for the fixed context scan.

use super::*;

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
fn compress(mut h: [u64; 8], block: &[u8; 128], counter: u64, last: bool) -> [u64; 8] {
    fn g(
        lanes: &mut [u64; 16],
        first: usize,
        second: usize,
        third: usize,
        fourth: usize,
        left_message: u64,
        right_message: u64,
    ) {
        lanes[first] = lanes[first]
            .wrapping_add(lanes[second])
            .wrapping_add(left_message);
        lanes[fourth] = (lanes[fourth] ^ lanes[first]).rotate_right(32);
        lanes[third] = lanes[third].wrapping_add(lanes[fourth]);
        lanes[second] = (lanes[second] ^ lanes[third]).rotate_right(24);
        lanes[first] = lanes[first]
            .wrapping_add(lanes[second])
            .wrapping_add(right_message);
        lanes[fourth] = (lanes[fourth] ^ lanes[first]).rotate_right(16);
        lanes[third] = lanes[third].wrapping_add(lanes[fourth]);
        lanes[second] = (lanes[second] ^ lanes[third]).rotate_right(63);
    }
    let m: [u64; 16] = core::array::from_fn(|i| {
        u64::from_le_bytes(block[i * 8..i * 8 + 8].try_into().expect("fixed block"))
    });
    let mut v = [0; 16];
    v[..8].copy_from_slice(&h);
    v[8..].copy_from_slice(&IV);
    v[12] ^= counter;
    if last {
        v[14] = !v[14];
    }
    for round in 0..12 {
        let s = SIGMA[round % 10];
        g(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        g(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        g(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        g(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        g(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        g(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        g(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        g(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }
    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
    h
}

/// Prepare the complete CRC/header/hash scan from an original result payload span.
/// All outputs are witness proposals. The finality owner must derive the selected
/// span with the circuit parser and verify every source leaf and interval join.
/// # Errors
/// Oversized frame, invalid span, checksum/layout failure or unexpected context ID.
pub fn prepare_context_hash(
    frame: Vec<u8>,
    payload_start: u32,
    payload_len: u32,
    context_id: [u8; 32],
) -> Result<Vec<ContextHashLeafCircuit>, Error> {
    if frame.len() > 65_536 || payload_len == 0 {
        return Err(Error::Synthesis);
    }
    let end = payload_start
        .checked_add(payload_len)
        .ok_or(Error::Synthesis)? as usize;
    let payload = frame
        .get(payload_start as usize..end)
        .ok_or(Error::Synthesis)?;
    let checksum = norito::core::hardware_crc64(payload);
    let crc_steps = payload.len().div_ceil(32);
    let mut preimage = EPOCH_TAG.to_vec();
    preimage.extend_from_slice(b"NRT0\0\0");
    preimage.extend_from_slice(&EPOCH_CODEC_ID);
    preimage.push(0);
    preimage.extend_from_slice(&u64::from(payload_len).to_le_bytes());
    preimage.extend_from_slice(&checksum.to_le_bytes());
    preimage.push(2);
    preimage.extend_from_slice(payload);
    let mut states = Vec::with_capacity(PROGRAM_LENGTH as usize);
    let mut raw = u64::MAX;
    let mut previous = ContextHashState::default();
    for (i, chunk) in payload.chunks(32).enumerate() {
        for byte in chunk {
            raw ^= u64::from(*byte);
            for _ in 0..8 {
                raw = (raw >> 1)
                    ^ if raw & 1 == 1 {
                        0xc96c_5795_d787_0f42
                    } else {
                        0
                    };
            }
        }
        let after = if i + 1 == crc_steps {
            ContextHashState::default()
        } else {
            ContextHashState {
                crc: raw,
                ..ContextHashState::default()
            }
        };
        states.push((ContextHashPhase::Crc, previous, after));
        previous = after;
    }
    if !raw != checksum {
        return Err(Error::Synthesis);
    }
    states.resize(
        usize::try_from(CRC_LEAVES).map_err(|_| Error::Synthesis)?,
        (
            ContextHashPhase::Crc,
            ContextHashState::default(),
            ContextHashState::default(),
        ),
    );
    let mut h = IV;
    h[0] ^= 0x0101_0020;
    for (i, chunk) in preimage.chunks(128).enumerate() {
        let mut block = [0; 128];
        block[..chunk.len()].copy_from_slice(chunk);
        let consumed = i * 128 + chunk.len();
        let count = u64::try_from(consumed).map_err(|_| Error::Synthesis)?;
        let last = consumed == preimage.len();
        h = compress(h, &block, count, last);
        let after = if last {
            ContextHashState::default()
        } else {
            ContextHashState {
                blake: h,
                ..ContextHashState::default()
            }
        };
        states.push((ContextHashPhase::Blake, previous, after));
        previous = after;
    }
    let mut digest = [0; 32];
    for (out, word) in digest.chunks_exact_mut(8).zip(h) {
        out.copy_from_slice(&word.to_le_bytes());
    }
    digest[31] |= 1;
    if digest != context_id {
        return Err(Error::Synthesis);
    }
    let result_len = u32::try_from(frame.len()).map_err(|_| Error::Synthesis)?;
    let witness = std::sync::Arc::new(ResultTapeWitness::from_frame(&Value::known(frame))?);
    let mut root = None;
    let _ = witness.root().map(|v| root = Some(v));
    let input = ContextHashInput {
        tape_root: root.ok_or(Error::Synthesis)?,
        result_len,
        payload_start,
        payload_len,
        checksum,
        context_id,
    };
    states.resize(
        usize::try_from(PROGRAM_LENGTH).map_err(|_| Error::Synthesis)?,
        (
            ContextHashPhase::Blake,
            ContextHashState::default(),
            ContextHashState::default(),
        ),
    );
    states
        .into_iter()
        .enumerate()
        .map(|(cursor, (phase, before, after))| {
            Ok(ContextHashLeafCircuit {
                phase,
                cursor: u32::try_from(cursor).map_err(|_| Error::Synthesis)?,
                input,
                before,
                after,
                tape: witness.clone(),
                known: true,
            })
        })
        .collect::<Result<Vec<_>, Error>>()
}
