//! Bounded untrusted proposal generation; all source decisions are proved again.

use super::*;
use crate::finality::roster::{key_leaf_native, key_tree_native};

#[derive(Clone, Copy, Debug, Default)]
struct Span {
    start: usize,
    len: usize,
    present: bool,
}
impl Span {
    fn end(self) -> Result<usize, Error> {
        self.start.checked_add(self.len).ok_or(Error::Synthesis)
    }
    fn words(self) -> [Fp; 3] {
        [
            Fp::from(self.start as u64),
            Fp::from(self.len as u64),
            Fp::from(u64::from(self.present)),
        ]
    }
}
struct Reader<'a>(&'a [u8]);
impl Reader<'_> {
    fn bytes(&self, span: Span) -> Result<&[u8], Error> {
        self.0.get(span.start..span.end()?).ok_or(Error::Synthesis)
    }
    fn field(&self, position: &mut usize, end: usize) -> Result<Span, Error> {
        let mut value = 0usize;
        let mut width = 0usize;
        loop {
            let byte = *self.0.get(*position).ok_or(Error::Synthesis)?;
            *position += 1;
            if *position > end || width == 3 {
                return Err(Error::Synthesis);
            }
            value |= usize::from(byte & 127) << (7 * width);
            width += 1;
            if byte & 128 == 0 {
                if width > 1 && byte == 0 {
                    return Err(Error::Synthesis);
                }
                break;
            }
        }
        let start = *position;
        *position = position.checked_add(value).ok_or(Error::Synthesis)?;
        if value > 65_536 || *position > end {
            return Err(Error::Synthesis);
        }
        Ok(Span {
            start,
            len: value,
            present: true,
        })
    }
    fn fields<const N: usize>(&self, span: Span) -> Result<[Span; N], Error> {
        if !span.present {
            return Ok([Span::default(); N]);
        }
        let mut position = span.start;
        let end = span.end()?;
        let mut out = Vec::with_capacity(N);
        for _ in 0..N {
            out.push(self.field(&mut position, end)?);
        }
        if position != end {
            return Err(Error::Synthesis);
        }
        out.try_into().map_err(|_| Error::Synthesis)
    }
    fn scalar(&self, span: Span) -> Result<u64, Error> {
        let bytes = self.bytes(span)?;
        if !(1..=8).contains(&bytes.len()) {
            return Err(Error::Synthesis);
        }
        Ok(bytes.iter().rev().fold(0, |n, b| n * 256 + u64::from(*b)))
    }
    fn id(&self, span: Span) -> Result<[u8; 32], Error> {
        self.bytes(span)?.try_into().map_err(|_| Error::Synthesis)
    }
    fn option(&self, span: Span, tag_len: usize) -> Result<Span, Error> {
        let bytes = self.bytes(span)?;
        let tag = bytes
            .get(..tag_len)
            .ok_or(Error::Synthesis)?
            .iter()
            .rev()
            .fold(0_u64, |n, b| n * 256 + u64::from(*b));
        match tag {
            0 if bytes.len() == tag_len => Ok(Span::default()),
            1 => {
                let mut position = span.start + tag_len;
                let body = self.field(&mut position, span.end()?)?;
                if position != span.end()? {
                    return Err(Error::Synthesis);
                }
                Ok(body)
            }
            _ => Err(Error::Synthesis),
        }
    }
    fn slot(&self, span: Span, height: u64) -> Result<SlotProjection, Error> {
        let bytes = self.bytes(span)?;
        let tag = u32::from_le_bytes(
            bytes
                .get(..4)
                .ok_or(Error::Synthesis)?
                .try_into()
                .map_err(|_| Error::Synthesis)?,
        );
        let (pending, boundary_height, predecessor, params) = match tag {
            0 => {
                let fields = self.fields::<2>(Span {
                    start: span.start + 4,
                    len: span.len.checked_sub(4).ok_or(Error::Synthesis)?,
                    present: true,
                })?;
                if self.scalar(fields[0])? != height {
                    return Err(Error::Synthesis);
                }
                (false, 0, [0; 32], fields[1])
            }
            1 => {
                let fields = self.fields::<4>(Span {
                    start: span.start + 4,
                    len: span.len.checked_sub(4).ok_or(Error::Synthesis)?,
                    present: true,
                })?;
                if self.scalar(fields[0])? != height {
                    return Err(Error::Synthesis);
                }
                (
                    true,
                    self.scalar(fields[1])?,
                    self.id(fields[2])?,
                    fields[3],
                )
            }
            _ => return Err(Error::Synthesis),
        };
        let fields = self.fields::<6>(params)?;
        let mut parameters = [0; 6];
        for (out, field) in parameters.iter_mut().zip(fields) {
            *out = self.scalar(field)?;
        }
        Ok(SlotProjection {
            pending,
            boundary_height,
            predecessor,
            parameters,
        })
    }
}
fn write_native(
    state: &mut [Fp; STATE_WORDS],
    offset: usize,
    spans: impl IntoIterator<Item = Span>,
) {
    for (i, span) in spans.into_iter().enumerate() {
        state[offset + 3 * i..offset + 3 * i + 3].copy_from_slice(&span.words());
    }
}

/// Prepare all43 parser proposals from one original R and the selected native
/// context ID. This function creates no authorization capability. The complete
/// parser, exact context-hash scan, R scan, and predecessor/QC sources must join.
/// # Errors
/// Oversized/malformed source framing, invalid roster geometry, missing payload,
/// or a context ID which differs from the selected complete canonical frame.
pub fn prepare_schedule_source(
    frame: Vec<u8>,
    authorized: bool,
    context_id: [u8; 32],
) -> Result<Vec<ScheduleSourceCircuit>, Error> {
    if frame.len() > 65_536 || frame.len() < 40 {
        return Err(Error::Synthesis);
    }
    let r = Reader(&frame);
    let fields = r.fields::<5>(Span {
        start: 40,
        len: frame.len() - 40,
        present: true,
    })?;
    let height = r.scalar(fields[0])?;
    let schedule = r.fields::<5>(fields[2])?;
    if r.scalar(schedule[0])? != height {
        return Err(Error::Synthesis);
    }
    let boundary = r.option(schedule[2], 1)?;
    let boundary_fields = r.fields::<6>(boundary)?;
    let body = if authorized && boundary.present {
        boundary_fields[4]
    } else {
        schedule[1]
    };
    let epoch = r.fields::<7>(body)?;
    let authorization = r.fields::<11>(epoch[4])?;
    let committee = r.bytes(epoch[5])?;
    let n = usize::try_from(u64::from_le_bytes(
        committee
            .get(..8)
            .ok_or(Error::Synthesis)?
            .try_into()
            .map_err(|_| Error::Synthesis)?,
    ))
    .map_err(|_| Error::Synthesis)?;
    if !(4..=31).contains(&n) || !(n - 1).is_multiple_of(3) || committee.len() != 8 + 215 * n {
        return Err(Error::Synthesis);
    }
    let keys: Vec<[u8; 48]> = committee[8..]
        .chunks_exact(215)
        .map(|member| core::array::from_fn(|i| member[15 + 2 * i]))
        .collect();
    let (roster_root, _) = key_tree_native(&keys)?;
    let beacon = r.option(authorization[7], 4)?;
    let beacon_fields = r.fields::<2>(beacon)?;
    let projection = ScheduleProjection {
        members: u8::try_from(n).map_err(|_| Error::Synthesis)?,
        faults: u8::try_from((n - 1) / 3).map_err(|_| Error::Synthesis)?,
        mode: r
            .scalar(epoch[3])?
            .try_into()
            .map_err(|_| Error::Synthesis)?,
        epoch: r.scalar(authorization[2])?,
        first: r.scalar(authorization[3])?,
        last: r.scalar(authorization[4])?,
        generation: r.scalar(authorization[5])?,
        decision: r
            .scalar(authorization[10])?
            .try_into()
            .map_err(|_| Error::Synthesis)?,
        network: r.id(epoch[2])?,
        seed: r.id(epoch[6])?,
        authorization_ids: [
            r.id(authorization[6])?,
            r.id(authorization[8])?,
            r.id(authorization[9])?,
        ],
        beacon_installed: beacon.present,
        beacon_ids: if beacon.present {
            [r.id(beacon_fields[0])?, r.id(beacon_fields[1])?]
        } else {
            [[0; 32]; 2]
        },
        boundary_present: boundary.present,
        boundary_height: if boundary.present {
            r.scalar(boundary_fields[1])?
        } else {
            0
        },
        boundary_ids: if boundary.present {
            [r.id(boundary_fields[2])?, r.id(boundary_fields[3])?]
        } else {
            [[0; 32]; 2]
        },
        slots: [
            r.slot(schedule[3], height.checked_add(1).ok_or(Error::Synthesis)?)?,
            r.slot(schedule[4], height.checked_add(2).ok_or(Error::Synthesis)?)?,
        ],
    };
    let hash_leaves = context_hash::prepare_context_hash(
        frame.clone(),
        u32::try_from(body.start).map_err(|_| Error::Synthesis)?,
        u32::try_from(body.len).map_err(|_| Error::Synthesis)?,
        context_id,
    )?;
    let input = ScheduleSourceInput {
        epoch_hash: *hash_leaves.first().ok_or(Error::Synthesis)?.input(),
        height,
        authorized,
        roster_root,
        projection,
    };
    drop(hash_leaves);
    let frame: std::sync::Arc<[u8]> = frame.into();
    let mut before = [Fp::ZERO; STATE_WORDS];
    let mut leaves = Vec::with_capacity(PROGRAM_LENGTH as usize);
    for cursor in 0..PROGRAM_LENGTH {
        let mut after = before;
        match cursor {
            0 => {
                after[0] = Fp::from(height);
                write_native(&mut after, GRAPH, schedule[1..].iter().copied());
            }
            1 => write_native(&mut after, BODY, [body]),
            2 => write_native(&mut after, EPOCH, epoch),
            4 => write_native(&mut after, AUTHORIZATION, authorization),
            10..=40 => {
                let seat = (cursor - 10) as usize;
                let key = keys.get(seat).copied().unwrap_or([0; 48]);
                after[ROSTER + seat] =
                    key_leaf_native(u8::try_from(seat).map_err(|_| Error::Synthesis)?, &key)?;
            }
            41 => after[ROSTER] = roster_root,
            42 => after = [Fp::ZERO; STATE_WORDS],
            _ => {}
        }
        leaves.push(ScheduleSourceCircuit {
            stage: ScheduleSourceStage::at(cursor).ok_or(Error::Synthesis)?,
            cursor,
            input,
            before,
            after,
            frame: frame.clone(),
            known: true,
        });
        before = after;
    }
    Ok(leaves)
}
