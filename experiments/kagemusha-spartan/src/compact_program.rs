//! PublicProgram.v3 transport. The signed SSA instruction order is reconstructed
//! exactly before operand, geometry and degree admission.
use std::io::Read;

use super::{MAX_PROGRAM_BYTES, Op, op_words};

pub(super) const MAX_WIRE_BYTES: usize = 10 * 1024 * 1024;
const OPCODES: usize = 5;
const STRIDE: usize = 9;
const LEVEL: i32 = 6;
const WINDOW_LOG: u32 = 21;

#[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha.research.kagemusha_spartan.PublicProgram.v3")]
struct CompactPublicProgram {
    frame: Vec<u8>,
}

pub(super) fn encode(value: &impl norito::NoritoSerialize) -> Result<Vec<u8>, String> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
    let original = norito::to_bytes(value).map_err(|e| format!("public program encode: {e}"))?;
    let frame = compress(&original)?;
    let bytes = norito::to_bytes(&CompactPublicProgram { frame }).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_WIRE_BYTES {
        return Err("compact public program wire bound".into());
    }
    Ok(bytes)
}

pub(super) fn decode<T>(bytes: &[u8]) -> Result<T, String>
where
    T: norito::NoritoSerialize + for<'a> norito::NoritoDeserialize<'a>,
{
    if bytes.len() > MAX_WIRE_BYTES {
        return Err("compact public program wire bound".into());
    }
    let envelope: CompactPublicProgram = decode_fixed_frame(bytes, MAX_WIRE_BYTES)?;
    let original = decompress(&envelope.frame)?;
    decode_fixed_frame(&original, MAX_PROGRAM_BYTES)
}

/// Both layers use the fixed Norito layout selected by the versioned profile.
fn decode_fixed_frame<T>(bytes: &[u8], limit: usize) -> Result<T, String>
where
    T: norito::NoritoSerialize + for<'a> norito::NoritoDeserialize<'a>,
{
    let value: T = norito::core::with_decode_limits(
        norito::DecodeLimits::new(limit, limit, 4 * limit, 4 * limit, 16),
        || {
            let payload = norito::canonical_frame_payload_with_archive_limit::<T>(
                bytes,
                norito::core::V1_LAYOUT_FLAGS,
                limit as u64,
            )?;
            let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::V1_LAYOUT_FLAGS);
            let (value, used) = norito::core::decode_field_canonical(payload)?;
            if used != payload.len() {
                return Err(norito::Error::LengthMismatch);
            }
            Ok(value)
        },
    )
    .map_err(|e| format!("public program decode: {e}"))?;
    norito::verify_exact_canonical_frame_with_layout(
        &value,
        bytes,
        norito::core::WireLayout::Fixed,
    )
    .map_err(|e| format!("noncanonical public program: {e}"))?;
    Ok(value)
}
pub(super) fn compress(original: &[u8]) -> Result<Vec<u8>, String> {
    if original.is_empty() || original.len() > MAX_PROGRAM_BYTES {
        return Err("expanded public program bound".into());
    }
    let mut encoder = zstd::bulk::Compressor::new(LEVEL).map_err(|e| e.to_string())?;
    encoder.window_log(WINDOW_LOG).map_err(|e| e.to_string())?;
    encoder.include_checksum(true).map_err(|e| e.to_string())?;
    encoder
        .include_contentsize(true)
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(zstd::zstd_safe::compress_bound(original.len()).min(MAX_WIRE_BYTES))
        .map_err(|e| e.to_string())?;
    encoder
        .compress_to_buffer(original, &mut result)
        .map_err(|e| format!("compact public program: {e}"))?;
    Ok(result)
}

pub(super) fn decompress(frame: &[u8]) -> Result<Vec<u8>, String> {
    use zstd::zstd_safe::{find_frame_compressed_size, get_frame_content_size};
    if frame.len() > MAX_WIRE_BYTES || find_frame_compressed_size(frame) != Ok(frame.len()) {
        return Err("expected one bounded program compression frame".into());
    }
    let size = get_frame_content_size(frame)
        .ok()
        .flatten()
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (1..=MAX_PROGRAM_BYTES).contains(n))
        .ok_or("expanded public program bound")?;
    // An enclosing World or device decoder can impose a smaller allocation
    // budget than this profile's format ceiling.
    norito::core::reserve_decode_allocation(size).map_err(|e| e.to_string())?;
    let mut decoder = zstd::Decoder::with_buffer(frame).map_err(|e| e.to_string())?;
    decoder
        .window_log_max(WINDOW_LOG)
        .map_err(|e| e.to_string())?;
    let mut result = Vec::new();
    result.try_reserve_exact(size).map_err(|e| e.to_string())?;
    result.resize(size, 0);
    decoder
        .read_exact(&mut result)
        .map_err(|e| format!("compact program decode: {e}"))?;
    if decoder.read(&mut [0]).map_err(|e| e.to_string())? != 0 {
        return Err("expanded program length mismatch".into());
    }
    norito::core::reserve_decode_allocation(
        zstd::zstd_safe::compress_bound(size).min(MAX_WIRE_BYTES),
    )
    .map_err(|e| e.to_string())?;
    if compress(&result)? != frame {
        return Err("noncanonical program compression frame".into());
    }
    Ok(result)
}

fn counts_and_offsets(codes: &[u8]) -> Result<([usize; OPCODES], [usize; OPCODES]), String> {
    let mut counts = [0; OPCODES];
    for &tag in codes {
        *counts
            .get_mut(tag as usize)
            .ok_or("unknown public program opcode")? += 1;
    }
    // The caller has checked count * STRIDE. Each opcode contributes eight
    // operand bytes per occurrence, so all plane offsets fit in this buffer.
    let mut end = codes.len();
    let offsets = std::array::from_fn(|tag| {
        let start = end;
        end += counts[tag] * 8;
        start
    });
    Ok((counts, offsets))
}

fn relative(tag: usize, side: usize) -> bool {
    // Add/Sub/Mul reference two previous slots. Scale references only its first.
    tag < 3 || (tag == 3 && side == 0)
}

fn check_count(inputs: u32, count: usize) -> Result<(), String> {
    if count > MAX_PROGRAM_BYTES / 12
        || u32::try_from(count)
            .ok()
            .and_then(|n| n.checked_add(inputs))
            .is_none()
    {
        return Err("program instruction count bound".into());
    }
    Ok(())
}

/// Opcode bytes followed by eight byte planes for each opcode in order
/// (little-endian u32, first operand then second). Each u32 is the wrapping
/// delta from the preceding operand of that opcode. Slot operands first use
/// their backwards distance from the next SSA slot. No encoder choice exists.
pub(super) fn pack<I: ExactSizeIterator<Item = Op>>(
    inputs: u32,
    instructions: impl Fn() -> I,
) -> Result<Vec<u8>, String> {
    let first = instructions();
    let count = first.len();
    check_count(inputs, count)?;
    let mut result = vec![0; count * STRIDE];
    for (i, op) in first.enumerate() {
        result[i] = op_words(op)[0] as u8;
    }
    let (counts, offsets) = counts_and_offsets(&result[..count])?;
    let mut rank = [0; OPCODES];
    let mut previous = [[0u32; 2]; OPCODES];
    for (i, op) in instructions().enumerate() {
        let [tag, a, b] = op_words(op);
        let tag = tag as usize;
        let slot = inputs + i as u32;
        for (side, operand) in [a, b].into_iter().enumerate() {
            let value = if relative(tag, side) {
                slot.wrapping_sub(operand)
            } else {
                operand
            };
            let delta = value.wrapping_sub(previous[tag][side]);
            previous[tag][side] = value;
            for (byte, octet) in delta.to_le_bytes().into_iter().enumerate() {
                result[offsets[tag] + (side * 4 + byte) * counts[tag] + rank[tag]] = octet;
            }
        }
        rank[tag] += 1;
    }
    Ok(result)
}

pub(super) fn unpack(inputs: u32, constants: usize, data: &[u8]) -> Result<Vec<Op>, String> {
    if !data.len().is_multiple_of(STRIDE) {
        return Err("partial public program instruction planes".into());
    }
    let count = data.len() / STRIDE;
    check_count(inputs, count)?;
    let codes = &data[..count];
    let (counts, offsets) = counts_and_offsets(codes)?;
    let mut rank = [0; OPCODES];
    let mut previous = [[0u32; 2]; OPCODES];
    let mut ops = Vec::new();
    norito::core::reserve_decode_allocation(count * std::mem::size_of::<Op>())
        .map_err(|e| e.to_string())?;
    ops.try_reserve_exact(count).map_err(|e| e.to_string())?;
    for (i, &tag) in codes.iter().enumerate() {
        let tag = tag as usize;
        let slot = inputs + i as u32;
        let [a, b] = std::array::from_fn(|side| {
            let delta = u32::from_le_bytes(std::array::from_fn(|byte| {
                data[offsets[tag] + (side * 4 + byte) * counts[tag] + rank[tag]]
            }));
            let value = previous[tag][side].wrapping_add(delta);
            previous[tag][side] = value;
            if relative(tag, side) {
                slot.wrapping_sub(value)
            } else {
                value
            }
        });
        rank[tag] += 1;
        ops.push(match tag {
            0 if a < slot && b < slot => Op::Add(a, b),
            1 if a < slot && b < slot => Op::Sub(a, b),
            2 if a < slot && b < slot => Op::Mul(a, b),
            3 if a < slot && (b as usize) < constants => Op::Scale(a, b),
            4 if (a as usize) < constants && b == 0 => Op::Const(a),
            _ => return Err("public program forward/invalid instruction".into()),
        });
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(norito::NoritoSerialize, norito::NoritoDeserialize, norito::NoritoSchema)]
    #[norito_schema(name = "iroha.research.kagemusha_spartan.test.CompactFrame")]
    struct Sample {
        fields: Vec<u8>,
    }

    #[test]
    fn program_envelope_preserves_exact_fixed_layout_across_compression() {
        let sample = Sample {
            fields: (0u8..=255).cycle().take(80_000).collect(),
        };
        let bytes = encode(&sample).unwrap();
        let decoded: Sample = decode(&bytes).unwrap();
        assert_eq!(decoded.fields, sample.fields);
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert!(decode::<Sample>(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn planes_restore_slots_constants_and_decreasing_deltas() {
        let ops = [
            Op::Add(2, 1),
            Op::Sub(0, 0),
            Op::Mul(4, 1),
            Op::Scale(5, 2),
            Op::Const(1),
            Op::Add(6, 2),
            Op::Scale(7, 0),
            Op::Const(0),
            Op::Add(0, 10),
        ];
        let inputs = 3;
        let bytes = pack(inputs, || ops.iter().copied()).unwrap();
        assert_eq!(&bytes[..ops.len()], &[0, 1, 2, 3, 4, 0, 3, 4, 0]);
        assert_eq!(unpack(inputs, 3, &bytes).unwrap(), ops);
        assert_eq!(unpack(inputs, 0, &[]).unwrap(), []);
        assert!(unpack(inputs, 3, &bytes[1..]).is_err());
        let mut bad = bytes.clone();
        bad[0] = OPCODES as u8;
        assert!(unpack(inputs, 3, &bad).is_err());
        assert!(unpack(inputs, 2, &bytes).is_err());
        assert!(unpack(u32::MAX, 3, &bytes).is_err());
        let mut constant = pack(inputs, || std::iter::once(Op::Const(0))).unwrap();
        constant[5] = 1; // unused second operand must be exactly zero
        assert!(unpack(inputs, 3, &constant).is_err());
        let forward = pack(inputs, || std::iter::once(Op::Sub(inputs, 0))).unwrap();
        assert!(unpack(inputs, 3, &forward).is_err());
    }

    #[test]
    fn canonical_compression_refuses_alternate_frames_and_size_claims() {
        let original = b"canonical public program".repeat(5_000);
        let frame = compress(&original).unwrap();
        assert_eq!(decompress(&frame).unwrap(), original);
        let strict = norito::DecodeLimits::new(2048, 2048, 4096, 4096, 16);
        assert!(norito::core::with_decode_limits_scope(strict, || decompress(&frame)).is_err());
        assert!(decompress(&[frame.as_slice(), frame.as_slice()].concat()).is_err());
        assert!(decompress(&frame[..frame.len() - 1]).is_err());
        let mut changed_checksum = frame.clone();
        *changed_checksum.last_mut().unwrap() ^= 1;
        assert!(decompress(&changed_checksum).is_err());
        let other = zstd::bulk::compress(&original, 1).unwrap();
        assert_ne!(frame, other);
        assert!(decompress(&other).is_err());
        // A valid single zstd frame header advertises an 8-byte content size.
        // Refuse the declaration before constructing a decoder or output buffer.
        let mut oversized = vec![0x28, 0xb5, 0x2f, 0xfd, 0xe0];
        oversized.extend_from_slice(&(MAX_PROGRAM_BYTES as u64 + 1).to_le_bytes());
        oversized.extend_from_slice(&[1, 0, 0]);
        assert_eq!(
            zstd::zstd_safe::get_frame_content_size(&oversized).unwrap(),
            Some(MAX_PROGRAM_BYTES as u64 + 1)
        );
        assert!(decompress(&oversized).is_err());
    }
}
