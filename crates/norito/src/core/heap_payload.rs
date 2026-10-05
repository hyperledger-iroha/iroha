//! Bounded canonical ordering for heap elements whose ordinary order has ties.

use super::{
    BinaryHeap, DecodeFlagsGuard, Encoder, Error, Layout, SerializePayload, alloc_checked,
    effective_layout_flags, encode_seq_payloads, encode_seq_payloads_with, encoded_payload_len,
    limit_to_u64, reserve_decode_allocation, write_counted_payload, write_len_with_flags,
    write_sequence_payload,
};

struct TiePayload {
    index: usize,
    start: usize,
    length: usize,
}

fn exact_scratch<T>(capacity: usize) -> Result<Vec<T>, Error> {
    let layout = Layout::array::<T>(capacity).map_err(|_| Error::LengthMismatch)?;
    if layout.size() == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: the nonzero array layout exactly matches the Vec's capacity and
    // alignment. alloc_checked admits it through every original active context
    // and rejects null before ownership; the Vec drops the same allocation.
    let (allocation, _) = unsafe { alloc_checked(layout) }?;
    // SAFETY: no elements are initialized yet; only admitted capacity is owned.
    Ok(unsafe { Vec::from_raw_parts(allocation.cast::<T>(), 0, capacity) })
}

fn tied<T: Ord>(items: &[&T], index: usize) -> bool {
    (index > 0 && items[index - 1].cmp(items[index]).is_eq())
        || (index + 1 < items.len() && items[index].cmp(items[index + 1]).is_eq())
}

pub(super) fn serialize<T: SerializePayload + Ord>(
    heap: &BinaryHeap<T>,
    writer: &mut Encoder<'_>,
) -> Result<(), Error> {
    // Preserve the reference-only path for elements with unique ordinary order.
    let mut items = Vec::new();
    let allocation_bytes = heap
        .len()
        .checked_mul(core::mem::size_of::<&T>())
        .ok_or(Error::LengthMismatch)?;
    reserve_decode_allocation(allocation_bytes)?;
    items
        .try_reserve_exact(heap.len())
        .map_err(|_| Error::AllocationFailed {
            bytes: limit_to_u64(allocation_bytes),
        })?;
    items.extend(heap.iter());
    items.sort_unstable();
    let tie_count = (0..items.len())
        .filter(|&index| tied(&items, index))
        .count();
    if tie_count == 0 {
        return encode_seq_payloads::<T, _>(writer, items.iter().copied());
    }

    // All keys use the same active element layout as the final sequence.
    let _flags = DecodeFlagsGuard::enter(effective_layout_flags());
    let mut keys = exact_scratch::<TiePayload>(tie_count)?;
    let mut arena_bytes = 0usize;
    for (index, &item) in items.iter().enumerate() {
        if tied(&items, index) {
            let length = encoded_payload_len(item)?;
            let start = arena_bytes;
            arena_bytes = arena_bytes
                .checked_add(length)
                .ok_or(Error::LengthMismatch)?;
            keys.push(TiePayload {
                index,
                start,
                length,
            });
        }
    }
    let mut arena = exact_scratch::<u8>(arena_bytes)?;
    arena.resize(arena_bytes, 0);
    for key in &keys {
        let payload = &mut arena[key.start..key.start + key.length];
        let mut destination = std::io::Cursor::new(payload);
        let mut encoder = Encoder::new(&mut destination);
        // Actual byte destinations retain sticky length checks. Measurement
        // never authorizes a key overrun, even if a serializer ignores errors.
        write_counted_payload(items[key.index], &mut encoder, key.length)?;
    }

    // Sort each equal-Ord group by its already-admitted bytes. Comparison cannot
    // allocate, serialize, fail, or depend on heap backing/insertion order.
    let mut group = 0usize;
    while group < keys.len() {
        let first_index = keys[group].index;
        let mut end = group + 1;
        while end < keys.len() && items[first_index].cmp(items[keys[end].index]).is_eq() {
            end += 1;
        }
        keys[group..end].sort_unstable_by(|left, right| {
            arena[left.start..left.start + left.length]
                .cmp(&arena[right.start..right.start + right.length])
        });
        for (offset, key) in keys[group..end].iter_mut().enumerate() {
            key.index = first_index + offset;
        }
        group = end;
    }

    let mut next_key = 0usize;
    encode_seq_payloads_with(
        writer,
        items.iter().copied().enumerate(),
        |(index, item), writer, flags| {
            if let Some(key) = keys.get(next_key).filter(|key| key.index == index) {
                let length = u64::try_from(key.length).map_err(|_| Error::LengthMismatch)?;
                write_len_with_flags(writer, length, flags)?;
                // This same invocation measured and wrote these exact bytes.
                // A counting destination incorporates them without repeating
                // the measurement; every byte destination receives the arena.
                if !writer.count_measured_bytes(key.length)? {
                    writer.with_exact_length(key.length, |writer| {
                        writer.write_all(&arena[key.start..key.start + key.length])?;
                        Ok(())
                    })?;
                }
                next_key += 1;
                Ok(())
            } else {
                write_sequence_payload(writer, item, flags)
            }
        },
    )?;
    (next_key == keys.len())
        .then_some(())
        .ok_or(Error::LengthMismatch)
}
