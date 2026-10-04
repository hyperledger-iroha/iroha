//! Typed sorted-state continuity with existing Boolean and comparison equations.

use super::*;

pub(super) const CONSTRAINTS: usize = 667;

fn source_word(sources: Sources<'_>, operand: usize) -> F {
    word::pack(sources.bits(operand), 1)
}

pub(super) fn append_residues(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    row: &[F],
    next: &[F],
    fixed: &[F],
) {
    append_with_count_boundary(out, row, next, fixed, CountBoundary::PublicExpected);
}

/// Private ordered/sorted counts agree at the last row without being public inputs.
pub(super) fn append_private_residues(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    row: &[F],
    next: &[F],
    fixed: &[F],
) {
    append_with_count_boundary(out, row, next, fixed, CountBoundary::PrivateEquality);
}

#[derive(Clone, Copy)]
enum CountBoundary {
    PublicExpected,
    PrivateEquality,
}

fn append_with_count_boundary(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    row: &[F],
    next: &[F],
    fixed: &[F],
    boundary: CountBoundary,
) {
    let start = out.len();
    let sources = Sources::new(&row[SOURCES..COMPARE]);
    sources.append_residues(out);
    branch::append_bank_residues(
        out,
        &row[COMPARE..CONTROLS],
        std::array::from_fn(|operand| std::array::from_fn(|limb| sources.limb(operand, limb))),
        [sources.sign(0), sources.sign(1)],
        [F::ZERO, F::ZERO, F::ZERO, F::ZERO, F::ONE, F::ZERO],
    );
    append_packet_shape(out, row, next, fixed);
    append_source_routes(out, row, fixed);
    append_state_transitions(out, row, next, fixed, boundary);
    append_read_preservation(out, row, fixed);
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

fn append_packet_shape(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    row: &[F],
    next: &[F],
    fixed: &[F],
) {
    use packet::*;
    let a = &row[ORDERED..SORTED];
    let b = &row[SORTED..PREVIOUS];
    let sources = Sources::new(&row[SOURCES..COMPARE]);
    let phase = &fixed[PHASE_OFFSET..PHASE_OFFSET + PHASES];
    let transition = fixed[TRANSITION];
    let within = F::ONE.sub(phase[PHASES - 1]);
    for packet in [a, b] {
        out.push(bit(packet[ENABLED]));
        out.push(bit(packet[WRITE]));
        out.push(packet[WRITE].mul(F::ONE.sub(packet[ENABLED])));
        for value in packet {
            out.push(F::ONE.sub(packet[ENABLED]).mul(*value));
        }
    }
    out.push(a[CLOCK].sub(a[ENABLED].mul(fixed[SLOT])));
    for offset in ORDERED..PREVIOUS {
        out.push(transition.mul(within).mul(next[offset].sub(row[offset])));
    }
    let kinds = &row[TYPES..ROW_WIDTH];
    out.extend(kinds.iter().copied().map(bit));
    out.push(kinds.iter().copied().fold(F::ZERO, F::add).sub(b[ENABLED]));
    out.push(
        b[SPACE].sub(
            kinds
                .iter()
                .enumerate()
                .fold(F::ZERO, |sum, (index, kind)| {
                    sum.add(kind.mul(F(index as u64 + 1)))
                }),
        ),
    );
    let register = kinds[1];
    let initialized = kinds[2];
    let owner = kinds[3];
    for offset in [BEFORE, AFTER] {
        for limb in 4..8 {
            out.push(register.mul(b[offset + limb]));
            out.push(owner.mul(b[offset + limb]));
        }
        for limb in 1..8 {
            out.push(initialized.mul(b[offset + limb]));
        }
    }
    for offset in [BEFORE_TAG, AFTER_TAG] {
        // Register privacy uses bit index zero, which may be public or private.
        out.push(register.mul(b[offset]).mul(b[offset].sub(F::ONE)));
        out.push(initialized.add(owner).mul(b[offset]));
    }
    out.push(kinds[0].add(register).mul(b[GENERATION]));
    for digit in &sources.bits(0)[8..32] {
        out.push(phase[2].mul(register).mul(*digit));
    }
}

fn append_source_routes(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    row: &[F],
    fixed: &[F],
) {
    use packet::*;
    let b = &row[SORTED..PREVIOUS];
    let previous = &row[PREVIOUS..SOURCES];
    let sources = Sources::new(&row[SOURCES..COMPARE]);
    let phase = &fixed[PHASE_OFFSET..PHASE_OFFSET + PHASES];
    for (mask, offset) in [(phase[0], BEFORE), (phase[1], AFTER)] {
        for limb in 0..8 {
            out.push(mask.mul(b[offset + limb].sub(sources.limb(limb / 4, limb % 4))));
        }
    }
    let key_bits = sources.bits(0);
    out.push(phase[2].mul(b[KEY].sub(source_word(sources, 0))));
    for (offset, bits) in [
        (INDEX, &key_bits[..32]),
        (GENERATION, &key_bits[32..48]),
        (VM, &key_bits[48..56]),
        (SPACE, &key_bits[56..60]),
    ] {
        out.push(phase[2].mul(b[offset].sub(word::pack(bits, 1))));
    }
    for (offset, expected) in [
        (CLOCK, sources.half(1, 0)),
        (BEFORE_TAG, sources.limb(1, 2)),
        (AFTER_TAG, sources.limb(1, 3)),
    ] {
        out.push(phase[2].mul(b[offset].sub(expected)));
    }
    for value in &key_bits[60..64] {
        out.push(phase[2].mul(*value));
    }
    for (operand, key) in [(0, previous[PREV_KEY]), (1, b[KEY])] {
        out.push(phase[3].mul(key.sub(source_word(sources, operand))));
        for value in &sources.bits(operand)[60..64] {
            out.push(phase[3].mul(*value));
        }
    }
    for (operand, clock) in [(0, previous[PREV_CLOCK]), (1, b[CLOCK])] {
        out.push(phase[4].mul(clock.sub(sources.half(operand, 0))));
        for value in &sources.bits(operand)[32..64] {
            out.push(phase[4].mul(*value));
        }
    }
    let dummy = phase[5].add(phase[6]).add(phase[7]);
    for value in &row[SOURCES..COMPARE] {
        out.push(dummy.mul(*value));
    }
}

fn append_state_transitions(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    row: &[F],
    next: &[F],
    fixed: &[F],
    boundary: CountBoundary,
) {
    use packet::*;
    let a = &row[ORDERED..SORTED];
    let b = &row[SORTED..PREVIOUS];
    let previous = &row[PREVIOUS..SOURCES];
    let bank = &row[COMPARE..CONTROLS];
    let phase = &fixed[PHASE_OFFSET..PHASE_OFFSET + PHASES];
    let transition = fixed[TRANSITION];
    let advance = phase[PHASES - 1];
    let within = F::ONE.sub(advance);
    let first = fixed[FIRST];
    let last = fixed[LAST];
    let equal = row[SAME_KEY];
    let new = row[NEW_KEY];
    out.extend(previous.iter().copied().map(|value| first.mul(value)));
    out.push(bit(previous[PREV_VALID]));
    for offset in 0..PREV_WIDTH {
        let update = match offset {
            PREV_KEY => b[KEY],
            PREV_CLOCK => b[CLOCK],
            PREV_TAG => b[AFTER_TAG],
            PREV_VALID => F::ONE,
            _ => b[AFTER + offset - PREV_PAYLOAD],
        };
        out.push(
            transition.mul(
                next[PREVIOUS + offset]
                    .sub(previous[offset])
                    .sub(advance.mul(b[ENABLED]).mul(update.sub(previous[offset]))),
            ),
        );
    }
    for offset in [SAME_KEY, NEW_KEY, KEY_LESS, TIME_LESS, SORTED_ENDED] {
        out.push(bit(row[offset]));
    }
    out.push(row[RESERVED]);
    let comparison_equal = branch::bank_equality(bank);
    let unsigned_less = bank[branch::BORROW + 3];
    out.push(phase[3].mul(equal.sub(b[ENABLED].mul(previous[PREV_VALID]).mul(comparison_equal))));
    out.push(new.add(equal).sub(b[ENABLED]));
    out.push(phase[3].mul(row[KEY_LESS].sub(unsigned_less)));
    out.push(phase[4].mul(row[TIME_LESS].sub(unsigned_less)));
    out.push(
        phase[3]
            .mul(b[ENABLED])
            .mul(previous[PREV_VALID])
            .mul(F::ONE.sub(row[KEY_LESS]).sub(comparison_equal)),
    );
    out.push(phase[4].mul(equal).mul(F::ONE.sub(row[TIME_LESS])));
    for offset in [SAME_KEY, NEW_KEY, KEY_LESS, TIME_LESS] {
        out.push(transition.mul(within).mul(next[offset].sub(row[offset])));
    }
    out.push(
        transition.mul(
            next[SORTED_ENDED].sub(row[SORTED_ENDED]).sub(
                advance
                    .mul(F::ONE.sub(b[ENABLED]))
                    .mul(F::ONE.sub(row[SORTED_ENDED])),
            ),
        ),
    );
    out.push(b[ENABLED].mul(row[SORTED_ENDED]));
    out.push(first.mul(row[SORTED_ENDED]));
    for (offset, packet) in [(ORDERED_COUNT, a), (SORTED_COUNT, b)] {
        out.push(first.mul(row[offset]));
        out.push(
            transition.mul(
                next[offset]
                    .sub(row[offset])
                    .sub(advance.mul(packet[ENABLED])),
            ),
        );
        if matches!(boundary, CountBoundary::PublicExpected) {
            out.push(
                last.mul(
                    row[offset]
                        .add(advance.mul(packet[ENABLED]))
                        .sub(fixed[TOTAL]),
                ),
            );
        }
    }
    if matches!(boundary, CountBoundary::PrivateEquality) {
        out.push(
            last.mul(
                row[ORDERED_COUNT]
                    .add(advance.mul(a[ENABLED]))
                    .sub(row[SORTED_COUNT].add(advance.mul(b[ENABLED]))),
            ),
        );
        // Same bank geometry as public qualification; no private count is placed
        // in a fixed column or exposed as a terminal/public transcript value.
        out.push(F::ZERO);
    }
}

fn append_read_preservation(
    out: &mut impl crate::execution_proofs::ivm_step_air::residues::Sink,
    row: &[F],
    fixed: &[F],
) {
    use packet::*;
    let b = &row[SORTED..PREVIOUS];
    let previous = &row[PREVIOUS..SOURCES];
    let phase = &fixed[PHASE_OFFSET..PHASE_OFFSET + PHASES];
    let equal = row[SAME_KEY];
    let new = row[NEW_KEY];
    for (before, after, previous) in (0..8)
        .map(|limb| {
            (
                b[BEFORE + limb],
                b[AFTER + limb],
                previous[PREV_PAYLOAD + limb],
            )
        })
        .chain(std::iter::once((
            b[BEFORE_TAG],
            b[AFTER_TAG],
            previous[PREV_TAG],
        )))
    {
        out.push(phase[6].mul(new.mul(before).add(equal.mul(before.sub(previous)))));
        out.push(
            phase[6]
                .mul(b[ENABLED].sub(b[WRITE]))
                .mul(after.sub(before)),
        );
    }
}

pub(super) fn source_words(packet: &[F], previous: &[F], phase: usize) -> [u64; 2] {
    use packet::*;
    match phase {
        0 => [half(packet, BEFORE, 0), half(packet, BEFORE, 1)],
        1 => [half(packet, AFTER, 0), half(packet, AFTER, 1)],
        2 => [
            packet[KEY].0,
            packet[CLOCK].0 | (packet[BEFORE_TAG].0 << 32) | (packet[AFTER_TAG].0 << 48),
        ],
        3 => [previous[PREV_KEY].0, packet[KEY].0],
        4 => [previous[PREV_CLOCK].0, packet[CLOCK].0],
        _ => [0, 0],
    }
}
