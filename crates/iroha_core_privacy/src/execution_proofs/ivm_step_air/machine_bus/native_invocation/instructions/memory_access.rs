//! Public-root LOAD/STORE permissions and original atomic register effects.
//!
//! Bounds come only from the admitted root initializer. Private effective
//! addresses, selections, values and prior masks are polynomial inputs, never
//! fixed permission premises. Public LOAD reads only initialized stack bytes;
//! private/wide memory access and fault coverage remain open.

use super::super::super::{
    bit, memory_initialization, memory_load, memory_store, packet, private_dispatch,
};
use super::super::{F, Fields, root};
use crate::execution_proofs::ivm_step_air::residues::Sink;
use iroha_allocation::{AllocationBudget, ChargedBuffer, ChargedBufferError};
use ivm::execution_packets::{MAX_STEPS, NativeInvocation, instruction_clocks};

const ADDRESS: usize = 0;
const END: usize = 64;
const CARRY: usize = 128;
const COMPARISON: usize = 132;
const REGION: usize = 404;
const MASK: usize = 406;
const WIDTH: usize = 422;
const CONSTRAINTS: usize = 586;
const GAPS: [usize; 9] = [20, 21, 22, 23, 27, 28, 29, 30, 31];

struct Witness([F; WIDTH]);
impl Witness {
    fn clear(&mut self) {
        for value in &mut self.0 {
            value.zeroize_v1();
        }
    }
}
impl Drop for Witness {
    fn drop(&mut self) {
        self.clear();
    }
}

pub(super) struct MemoryAccesses {
    bounds: [[u64; 2]; 2],
    rows: ChargedBuffer<Witness>,
}
impl MemoryAccesses {
    pub(super) const BYTES: usize = MAX_STEPS * core::mem::size_of::<Witness>();

    pub(super) fn new(
        native: &NativeInvocation,
        root: &root::Plan,
        budget: &AllocationBudget,
    ) -> Result<Self, ChargedBufferError> {
        let mut rows = ChargedBuffer::new(MAX_STEPS, budget)?;
        let bounds = root.memory_bounds();
        for window in 0..MAX_STEPS {
            let first = first(window);
            let memory = Fields::native(&native.packets()[first + 6]);
            let initialized = Fields::native(&native.packets()[first + 7]);
            let mut row = Witness([F::ZERO; WIDTH]);
            // Witness generation is untrusted arithmetic. The evaluator binds
            // this address to the original decoded effective address, not this
            // memory header or a native acceptance flag.
            let enabled = memory.0[packet::ENABLED] == F::ONE;
            let base = Fields::native(&native.packets()[first + 4]);
            let pc = &native.packets()[first];
            let address = if enabled {
                let pc = u64::from_le_bytes(pc.before()[..8].try_into().unwrap()) as usize;
                let offset = pc + native.artifact().header_len();
                let word = u32::from_le_bytes(
                    native.artifact().artifact()[offset..offset + 4]
                        .try_into()
                        .unwrap(),
                );
                value(&base.0).wrapping_add_signed(i64::from(ivm::instruction::wide::imm8(word)))
            } else {
                0
            };
            fill(
                &mut row.0,
                bounds,
                enabled,
                address,
                initialized.0[packet::BEFORE].0 as u16,
            );
            rows.push_reserved(row);
        }
        Ok(Self { bounds, rows })
    }

    pub(super) fn append_residues(
        &self,
        out: &mut impl Sink,
        native: &NativeInvocation,
        window: usize,
        decoded: &private_dispatch::Decoded<'_>,
    ) {
        let first = first(window);
        let memory = Fields::native(&native.packets()[first + 6]);
        let initialized = Fields::native(&native.packets()[first + 7]);
        append(
            out,
            self.bounds,
            first,
            &self.rows.as_slice()[window].0,
            decoded.store,
            decoded.load,
            decoded.load_destination,
            &decoded.memory_address,
            decoded.store_value,
            decoded.destination,
            &memory.0,
            &initialized.0,
        );
        for offset in GAPS {
            let gap = Fields::native(&native.packets()[first + offset]);
            append_gap(out, &gap.0);
        }
    }
}

fn first(window: usize) -> usize {
    assert!(window < MAX_STEPS);
    instruction_clocks(window).expect("compact native window")[0] as usize
}
fn value(fields: &[F; packet::WIDTH]) -> u64 {
    (0..4).fold(0, |value, limb| {
        value | (fields[packet::BEFORE + limb].0 << (16 * limb))
    })
}
fn bits(row: &mut [F], value: u64) {
    for (bit, cell) in row.iter_mut().enumerate() {
        *cell = F((value >> bit) & 1);
    }
}
fn word(row: &[F]) -> F {
    row.iter()
        .enumerate()
        .fold(F::ZERO, |sum, (bit, value)| sum.add(value.mul(F(1 << bit))))
}
fn limb(row: &[F], index: usize) -> F {
    word(&row[index * 16..index * 16 + 16])
}

fn fill(row: &mut [F; WIDTH], bounds: [[u64; 2]; 2], selected: bool, address: u64, mask: u16) {
    let address = if selected { address } else { 0 };
    let end = address.wrapping_add(if selected { 8 } else { 0 });
    bits(&mut row[ADDRESS..END], address);
    bits(&mut row[END..CARRY], end);
    let mut carry = u64::from(selected) * 8;
    for i in 0..4 {
        carry = (((address >> (16 * i)) & 0xffff) + carry) >> 16;
        row[CARRY + i] = F(carry);
    }
    for (region, [start, stop]) in bounds.into_iter().enumerate() {
        for (comparison, lhs, rhs) in [
            (region * 2, address, if selected { start } else { 0 }),
            (region * 2 + 1, if selected { stop } else { 0 }, end),
        ] {
            let at = COMPARISON + comparison * 68;
            bits(&mut row[at..at + 64], lhs.wrapping_sub(rhs));
            let mut borrow = 0;
            for i in 0..4 {
                let a = (lhs >> (16 * i)) & 0xffff;
                let b = ((rhs >> (16 * i)) & 0xffff) + borrow;
                borrow = u64::from(a < b);
                row[at + 64 + i] = F(borrow);
            }
        }
        row[REGION + region] = F(u64::from(!selected || (address >= start && end <= stop)));
    }
    bits(&mut row[MASK..], u64::from(if selected { mask } else { 0 }));
}

fn append(
    out: &mut impl Sink,
    bounds: [[u64; 2]; 2],
    first: usize,
    row: &[F; WIDTH],
    store: F,
    load: F,
    load_destination: F,
    address: &[F; 4],
    source: &[F; packet::WIDTH],
    destination: &[F; packet::WIDTH],
    memory: &[F; packet::WIDTH],
    initialized: &[F; packet::WIDTH],
) {
    use packet::*;
    let start = out.len();
    let selected = store.add(load);
    out.extend(row.iter().map(|value| bit(*value)));
    for (i, original) in address.iter().enumerate() {
        out.push(limb(&row[ADDRESS..END], i).sub(*original));
    }
    for i in 0..4 {
        let carry = if i == 0 {
            selected.mul(F(8))
        } else {
            row[CARRY + i - 1]
        };
        out.push(
            limb(&row[ADDRESS..END], i)
                .add(carry)
                .sub(limb(&row[END..CARRY], i))
                .sub(row[CARRY + i].mul(F(65536))),
        );
    }
    out.push(row[CARRY + 3]);
    out.extend(row[ADDRESS..ADDRESS + 3].iter().copied());
    for (region, [low, high]) in bounds.into_iter().enumerate() {
        for side in 0..2 {
            let at = COMPARISON + (region * 2 + side) * 68;
            for i in 0..4 {
                let bound = selected.mul(F(
                    ((if side == 0 { low } else { high }) >> (16 * i)) & 0xffff
                ));
                let (lhs, rhs) = if side == 0 {
                    (address[i], bound)
                } else {
                    (bound, limb(&row[END..CARRY], i))
                };
                let borrow = if i == 0 {
                    F::ZERO
                } else {
                    row[at + 64 + i - 1]
                };
                out.push(
                    lhs.sub(rhs)
                        .sub(borrow)
                        .add(row[at + 64 + i].mul(F(65536)))
                        .sub(limb(&row[at..at + 64], i)),
                );
            }
        }
        let at = COMPARISON + region * 136;
        out.push(row[REGION + region].sub(F::ONE.sub(row[at + 67]).mul(F::ONE.sub(row[at + 135]))));
    }
    out.push(
        store
            .mul(F::ONE.sub(row[REGION]))
            .mul(F::ONE.sub(row[REGION + 1])),
    );
    out.push(load.mul(F::ONE.sub(row[REGION])));
    let half = row[ADDRESS + 3];
    out.push(F::ONE.sub(selected).mul(half));
    let index = word(&row[ADDRESS + 4..END]);
    header(
        out,
        memory,
        Space::Memory,
        0,
        first + 6,
        selected,
        store,
        index,
    );
    out.extend(memory.iter().map(|value| F::ONE.sub(selected).mul(*value)));
    out.push(memory[BEFORE_TAG]);
    out.push(memory[AFTER_TAG]);
    for i in 0..8 {
        let expected = memory_store::payload_limb(
            memory[BEFORE + i],
            source[BEFORE + i % 4],
            F::ZERO,
            half,
            false,
            i,
        );
        out.push(
            memory[AFTER + i]
                .sub(store.mul(expected))
                .sub(load.mul(memory[BEFORE + i])),
        );
    }
    out.push(source[BEFORE_TAG].mul(store));
    header(
        out,
        initialized,
        Space::Initialization,
        1,
        first + 7,
        selected,
        store,
        index,
    );
    out.extend(
        initialized
            .iter()
            .map(|value| F::ONE.sub(selected).mul(*value)),
    );
    for at in [BEFORE, AFTER] {
        for i in 1..8 {
            out.push(initialized[at + i]);
        }
    }
    out.push(initialized[BEFORE_TAG]);
    out.push(initialized[AFTER_TAG]);
    let prior: &[F; 16] = row[MASK..].try_into().unwrap();
    let (before, after) = memory_initialization::initialized_masks(prior, half, false);
    out.push(initialized[BEFORE].sub(before));
    out.push(
        initialized[AFTER]
            .sub(store.mul(after))
            .sub(load.mul(before)),
    );
    // Ordinary compiled reads require all eight requested bytes initialized.
    // The other half is retained, not falsely required or rewritten.
    for i in 0..8 {
        out.push(load.mul(F::ONE.sub(memory_load::payload_limb(prior[i], prior[i + 8], half))));
    }
    for limb in 0..4 {
        let value =
            memory_load::payload_limb(memory[BEFORE + limb], memory[BEFORE + 4 + limb], half);
        out.push(load_destination.mul(destination[AFTER + limb].sub(value)));
    }
    out.push(load.mul(destination[AFTER_TAG]));
    out.push(bit(load_destination));
    out.push(load_destination.mul(F::ONE.sub(load)));
    out.extend(prior.iter().map(|value| F::ONE.sub(selected).mul(*value)));
    out.push(bit(store));
    out.push(bit(load));
    out.push(store.mul(load));
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}
fn header(
    out: &mut impl Sink,
    fields: &[F; packet::WIDTH],
    space: packet::Space,
    generation: u64,
    clock: usize,
    selected: F,
    write: F,
    index: F,
) {
    use packet::*;
    for (column, expected) in [
        (SPACE, F(space as u64)),
        (VM, F::ZERO),
        (GENERATION, F(generation)),
        (INDEX, index),
        (
            KEY,
            index.add(F((generation << 32) + ((space as u64) << 56))),
        ),
        (CLOCK, F(clock as u64)),
        (ENABLED, F::ONE),
    ] {
        out.push(fields[column].sub(selected.mul(expected)));
    }
    out.push(fields[WRITE].sub(write));
}
fn append_gap(out: &mut impl Sink, fields: &[F; packet::WIDTH]) {
    out.extend(fields.iter().copied());
}

#[cfg(test)]
mod tests;
