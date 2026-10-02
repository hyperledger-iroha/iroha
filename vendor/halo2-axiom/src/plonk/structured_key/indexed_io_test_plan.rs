//! Test-only independent byte operations for canonical original-source interval modes.

use super::*;
use indexed::reads::IndexedKeyPolynomialV1 as Id;

#[derive(Clone, Copy, Debug)]
pub(super) struct Operation {
    pub(super) at: u64,
    pub(super) bytes: usize,
    pub(super) unit: usize,
}
#[derive(Debug)]
pub(super) struct Observation {
    pub(super) seeks: usize,
    pub(super) reads: usize,
    pub(super) delivered: usize,
    pub(super) largest: usize,
    pub(super) first: Option<u64>,
    pub(super) last: Option<u64>,
    pub(super) position: u64,
}
pub(super) fn total(plan: &[Operation]) -> usize {
    plan.iter().map(|p| p.bytes).sum()
}
pub(super) fn observation(
    plan: &[Operation],
    chunk: usize,
    boundary: Option<usize>,
) -> Observation {
    let mut o = Observation {
        seeks: 0,
        reads: 0,
        delivered: 0,
        largest: 0,
        first: None,
        last: None,
        position: 13,
    };
    for p in plan {
        o.seeks += 1;
        o.position = p.at;
        for at in (0..p.bytes).step_by(p.unit) {
            o.largest = o.largest.max(p.unit);
            if boundary == Some(o.delivered) {
                o.reads += 1;
                return o;
            }
            for from in (0..p.unit).step_by(chunk.min(p.unit)) {
                let size = chunk.min(p.unit - from);
                if boundary == Some(o.delivered) {
                    o.reads += 1;
                    return o;
                }
                let size = boundary.map_or(size, |b| size.min(b - o.delivered));
                o.reads += 1;
                o.first.get_or_insert(p.at + (at + from) as u64);
                o.delivered += size;
                o.position += size as u64;
                o.last = Some(o.position);
                if boundary == Some(o.delivered) && size < chunk.min(p.unit - from) {
                    o.reads += 1;
                    return o;
                }
            }
        }
    }
    o
}

pub(super) fn plan<C: SerdeCurveAffine>(
    bytes: &[u8],
    key: &IndexedStructuredProvingKeyV1<C>,
    id: Id,
    start: usize,
    length: usize,
) -> Vec<Operation>
where
    C::Scalar: SerdePrimeField + FromUniformBytes<64>,
{
    if length == 0 {
        return vec![];
    }
    let m = key.metadata();
    let end = start + length;
    let width = scalar_bytes::<C::Scalar>();
    let op = |at, bytes, unit| Operation { at, bytes, unit };
    match id {
        Id::MaskCoefficient(mask) => vec![op(
            m.masks[mask].offset + (start * width) as u64,
            length * width,
            width,
        )],
        Id::FixedLagrange(column) => {
            let r = &m.fixed[column];
            match r.mode {
                CONSTANT => vec![op(r.payload.offset, width, width)],
                BITSET => vec![op(
                    r.payload.offset + (start / 8) as u64,
                    end.div_ceil(8) - start / 8,
                    1,
                )],
                RAW => vec![op(
                    r.payload.offset + (start * width) as u64,
                    length * width,
                    width,
                )],
                _ => unreachable!(),
            }
        }
        Id::PermutationLagrange(column) => {
            let r = &m.permutations[column];
            match r.mode {
                Mode::Identity => vec![],
                Mode::Dense => vec![op(r.targets.offset + (start * 4) as u64, length * 4, 4)],
                Mode::Sparse => {
                    let pairs = bytes
                        [r.targets.offset as usize..(r.targets.offset + r.targets.length) as usize]
                        .chunks_exact(8)
                        .map(|p| u32::from_le_bytes(p[..4].try_into().unwrap()) as usize)
                        .collect::<Vec<_>>();
                    let mut operations = vec![];
                    let (mut first, mut last) = (0, pairs.len());
                    while first < last {
                        let mid = first + (last - first) / 2;
                        operations.push(op(r.targets.offset + (mid * 8) as u64, 8, 8));
                        if pairs[mid] < start {
                            first = mid + 1;
                        } else {
                            last = mid;
                        }
                    }
                    if first > 0 {
                        operations.push(op(r.targets.offset + ((first - 1) * 8) as u64, 8, 8));
                    }
                    for (at, row) in pairs.iter().enumerate().skip(first) {
                        operations.push(op(r.targets.offset + (at * 8) as u64, 8, 8));
                        if *row >= end {
                            break;
                        }
                    }
                    operations
                }
                Mode::Bitmap => {
                    let bitmap = &bytes
                        [r.bitmap.offset as usize..(r.bitmap.offset + r.bitmap.length) as usize];
                    let mut operations = vec![];
                    for block in start / 4096..end.div_ceil(4096) {
                        let first = block * 4096;
                        let last = (first + 4096).min(m.rows);
                        let count = (last - first).div_ceil(8);
                        operations.push(op(r.bitmap.offset + (first / 8) as u64, count, count));
                        let from = start.max(first);
                        let to = end.min(last);
                        let rank = (0..from)
                            .filter(|row| bitmap[row / 8] >> (row % 8) & 1 != 0)
                            .count();
                        let count = (from..to)
                            .filter(|row| bitmap[row / 8] >> (row % 8) & 1 != 0)
                            .count();
                        if count != 0 {
                            operations.push(op(r.targets.offset + (rank * 4) as u64, count * 4, 4));
                        }
                    }
                    operations
                }
            }
        }
    }
}
