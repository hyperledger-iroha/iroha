//! Independent integer masks, direct linkage mutations and polynomial degree.
use super::*;
use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
use packet::{AFTER, BEFORE, ENABLED, Event, Space};

const CELL: u64 = ivm::Memory::STACK_START + 256;
const BYTES: [u8; 16] = [0x5a; 16];

fn phase(address: u64, length: usize, tracked: bool) -> WritePhase {
    WritePhase::from_fixed_input(
        7,
        19,
        address,
        length,
        40,
        41,
        tracked.then_some(FrameRegions {
            stack: [CELL, CELL + 64],
            results: [ivm::Memory::HEAP_START + 8, ivm::Memory::HEAP_START + 24],
        }),
    )
    .unwrap()
}
fn encoded_mask(value: u16) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..2].copy_from_slice(&value.to_le_bytes());
    bytes
}
#[derive(Clone)]
struct Fixture {
    phase: WritePhase,
    row: [F; WIDTH],
    memory: [F; packet::WIDTH],
    log: [F; memory_store::WRITE_LOG_WIDTH],
    initialized: [F; packet::WIDTH],
}
impl Fixture {
    fn new(p: WritePhase, old: u16, committed: bool) -> Self {
        let active = p.tracked && committed;
        let mut f = Self {
            phase: p,
            row: witness(old, active),
            memory: [F::ZERO; packet::WIDTH],
            log: [F::ZERO; memory_store::WRITE_LOG_WIDTH],
            initialized: [F::ZERO; packet::WIDTH],
        };
        if committed {
            f.memory = Event {
                space: Space::Memory,
                vm: p.vm,
                generation: 0,
                index: (p.address / 16) as u32,
                write: true,
                before: [0x33; 16],
                after: BYTES,
                before_private: 0x1234,
                after_private: 0x5678,
            }
            .fields(p.memory_clock as usize);
            f.log[..4].copy_from_slice(&[
                F(p.address & 0xffff_ffff),
                F(p.address >> 32),
                F(p.length as u64),
                F::ONE,
            ]);
            for (limb, pair) in BYTES[..p.length].chunks_exact(2).enumerate() {
                f.log[4 + limb] = F(u64::from(u16::from_le_bytes([pair[0], pair[1]])));
            }
        }
        if active {
            // Byte-set oracle: does not evaluate the AIR's packed-field formula.
            let mut bytes = core::array::from_fn::<_, 16, _>(|i| old & (1 << i) != 0);
            for address in p.address..p.address + p.length as u64 {
                bytes[(address % 16) as usize] = true;
            }
            let after = bytes
                .iter()
                .enumerate()
                .fold(0u16, |mask, (i, set)| mask | (u16::from(*set) << i));
            f.initialized = Event {
                space: Space::Initialization,
                vm: p.vm,
                generation: p.generation,
                index: (p.address / 16) as u32,
                write: true,
                before: encoded_mask(old),
                after: encoded_mask(after),
                before_private: 0,
                after_private: 0,
            }
            .fields(p.initialization_clock as usize);
        }
        f
    }
    fn residues(&self) -> Vec<F> {
        let mut out = Vec::new();
        append_residues(
            &mut out,
            self.phase,
            &self.row,
            &self.memory,
            &self.log,
            &self.initialized,
        );
        assert_eq!(out.len(), CONSTRAINTS);
        out
    }
    fn accepts(&self) -> bool {
        self.residues().into_iter().all(|x| x == F::ZERO)
    }
}

#[test]
fn initialization_updates_exact_half_and_wide_byte_sets_without_clearing_prior_bits() {
    for old in [0, 1, 0x80, 0xff, 0xff00, 0xaaaa, 0x5555, u16::MAX] {
        for (address, length, added) in [(CELL, 8, 0xff), (CELL + 8, 8, 0xff00), (CELL, 16, 0xffff)]
        {
            let f = Fixture::new(phase(address, length, true), old, true);
            assert!(f.accepts());
            assert_eq!(f.initialized[BEFORE], F(u64::from(old)));
            assert_eq!(f.initialized[AFTER], F(u64::from(old | added)));
        }
    }
    // Native bitmaps are region-relative; neither an eight-byte shift nor a
    // result table spanning two cells changes the absolute bus bit position.
    for (address, wanted) in [
        (ivm::Memory::HEAP_START + 8, 0xff00),
        (ivm::Memory::HEAP_START + 16, 0xff),
    ] {
        let f = Fixture::new(phase(address, 8, true), 0, true);
        assert!(f.accepts());
        assert_eq!(f.initialized[AFTER], F(wanted));
    }
}

#[test]
fn every_initialization_field_and_prior_bit_mutation_is_rejected() {
    let f = Fixture::new(phase(CELL + 8, 8, true), 0x5555, true);
    assert!(f.accepts());
    for i in 0..WIDTH {
        let mut bad = f.clone();
        bad.row[i] = F::ONE.sub(bad.row[i]);
        assert!(!bad.accepts(), "prior initialization bit {i}");
        let mut bad = f.clone();
        bad.row[i] = F(2);
        assert!(!bad.accepts(), "non-Boolean initialization bit {i}");
    }
    for i in 0..packet::WIDTH {
        let mut bad = f.clone();
        bad.initialized[i] = bad.initialized[i].add(F::ONE);
        assert!(!bad.accepts(), "initialization tuple field {i}");
    }
}

#[test]
fn memory_cell_log_half_and_frame_generation_cannot_be_transplanted() {
    let f = Fixture::new(phase(CELL + 8, 8, true), 0x81, true);
    for i in 0..memory_store::WRITE_LOG_WIDTH {
        let mut bad = f.clone();
        bad.log[i] = bad.log[i].add(F::ONE);
        assert!(!bad.accepts(), "log field {i}");
    }
    for i in [
        packet::SPACE,
        packet::VM,
        packet::GENERATION,
        packet::INDEX,
        packet::KEY,
        packet::CLOCK,
        packet::ENABLED,
        packet::WRITE,
        AFTER + 4,
    ] {
        let mut bad = f.clone();
        bad.memory[i] = bad.memory[i].add(F::ONE);
        assert!(!bad.accepts(), "store tuple link {i}");
    }
    for replacement in [
        WritePhase {
            generation: 20,
            ..f.phase
        },
        WritePhase { vm: 8, ..f.phase },
        WritePhase {
            address: CELL,
            ..f.phase
        },
        WritePhase {
            initialization_clock: 42,
            ..f.phase
        },
    ] {
        let mut bad = f.clone();
        bad.phase = replacement;
        assert!(!bad.accepts());
    }
    // A correlated wrong-half initialized value and byte witness still fail the
    // log address linkage; independent tuple validity cannot substitute a store.
    let mut bad = Fixture::new(phase(CELL, 8, true), 0x81, true);
    bad.memory = f.memory;
    bad.log = f.log;
    assert!(!bad.accepts());
}

#[test]
fn refused_or_untracked_stores_cannot_emit_or_disclose_initialization() {
    for tracked in [false, true] {
        for committed in [false, true] {
            if tracked && committed {
                continue;
            }
            let f = Fixture::new(phase(CELL, 8, tracked), 0xffff, committed);
            assert!(f.accepts());
            assert_eq!(f.initialized, [F::ZERO; packet::WIDTH]);
            assert_eq!(f.row, [F::ZERO; WIDTH]);
            for i in 0..packet::WIDTH {
                let mut bad = f.clone();
                bad.initialized[i] = F::ONE;
                assert!(!bad.accepts());
            }
            for i in 0..WIDTH {
                let mut bad = f.clone();
                bad.row[i] = F::ONE;
                assert!(!bad.accepts());
            }
            if !committed {
                let mut bad = f;
                bad.initialized = Fixture::new(phase(CELL, 8, true), 0, true).initialized;
                assert!(!bad.accepts(), "refused STORE must not initialize a result");
            }
        }
    }
}

#[test]
fn adjacent_disjoint_regions_share_cell_bits_but_crossing_writes_are_rejected() {
    let regions = FrameRegions {
        stack: [CELL - 8, CELL + 8],
        results: [CELL + 8, CELL + 16],
    };
    let make = |address, length| {
        WritePhase::from_fixed_input(7, 19, address, length, 40, 41, Some(regions))
    };
    let a = Fixture::new(make(CELL, 8).unwrap(), 0, true);
    let b = Fixture::new(make(CELL + 8, 8).unwrap(), 0xff, true);
    assert!(a.accepts() && b.accepts());
    assert_eq!(a.initialized[AFTER], b.initialized[BEFORE]);
    assert_eq!(b.initialized[AFTER], F(0xffff));
    assert!(
        make(CELL, 16).is_none(),
        "crossing disjoint owners is not one permitted write"
    );
    for invalid in [
        FrameRegions {
            stack: [CELL + 1, CELL + 16],
            ..regions
        },
        FrameRegions {
            stack: [CELL + 16, CELL],
            ..regions
        },
        FrameRegions {
            results: [CELL, CELL + 16],
            ..regions
        },
    ] {
        assert!(WritePhase::from_fixed_input(7, 19, CELL, 8, 40, 41, Some(invalid)).is_none());
    }
    assert!(WritePhase::from_fixed_input(7, 19, CELL + 1, 8, 40, 41, None).is_none());
    assert!(WritePhase::from_fixed_input(7, 19, CELL, 4, 40, 41, None).is_none());
    assert!(WritePhase::from_fixed_input(7, 19, CELL, 8, 41, 41, None).is_none());
    assert!(WritePhase::from_fixed_input(7, 19, 1 << 36, 8, 40, 41, None).is_none());
}

#[test]
fn initialization_degree_geometry_and_missing_lifecycle_authority_are_explicit() {
    let total = WIDTH + 2 * packet::WIDTH + memory_store::WRITE_LOG_WIDTH;
    for p in [
        phase(CELL, 8, true),
        phase(CELL + 8, 8, true),
        phase(CELL, 16, true),
        phase(CELL, 8, false),
    ] {
        let degree = measured_maximum_affine_degree_v1(
            [71; 32],
            [total, 0, 0, 0, 0],
            8,
            2,
            |row, _, _, _, _| {
                let a = WIDTH;
                let b = a + packet::WIDTH;
                let c = b + memory_store::WRITE_LOG_WIDTH;
                let mut out = Vec::new();
                append_residues(
                    &mut out,
                    p,
                    row[..a].try_into().unwrap(),
                    row[a..b].try_into().unwrap(),
                    row[b..c].try_into().unwrap(),
                    row[c..].try_into().unwrap(),
                );
                Ok::<_, core::convert::Infallible>(out)
            },
        );
        assert_eq!(degree, 2);
    }
    let field_bytes = 136 * 2 * 8 + 2 * 32;
    assert_eq!(WIDTH * field_bytes, 35_840);
    assert_eq!(total * field_bytes, 179_200);
    assert!(
        total * field_bytes > 41_152,
        "shared packets are not free; no complete-profile fit claim"
    );
    // No permission, first-state or frame-generation authority is fabricated.
    // The existing bus/lifecycle relation must reject counterfeit old state.
    let a = Fixture::new(phase(CELL, 8, true), 0, true);
    let b = Fixture::new(phase(CELL, 8, true), 0xffff, true);
    assert!(a.accepts() && b.accepts());
    assert_ne!(a.initialized[BEFORE], b.initialized[BEFORE]);
    assert_eq!(a.memory[ENABLED], F::ONE);
}

#[test]
fn initialization_composes_with_the_same_constrained_store_packet_and_log() {
    use ivm::{encoding::wide as enc, instruction::wide};
    let p = phase(CELL + 8, 8, true);
    let mut f = Fixture::new(p, 0x1080, true);
    let value = u64::from_le_bytes([0x5a; 8]);
    let mut after = [0x33; 16];
    after[8..].fill(0x5a);
    f.memory[packet::BEFORE_TAG] = F(0x1234);
    f.memory[packet::AFTER_TAG] = F(0x0034);
    for (i, pair) in after.chunks_exact(2).enumerate() {
        f.memory[AFTER + i] = F(u64::from(u16::from_le_bytes([pair[0], pair[1]])));
    }
    let mut reads = [[F::ZERO; packet::WIDTH]; 3];
    for (slot, index, value) in [(0, 3, CELL + 8), (1, 1, value)] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&value.to_le_bytes());
        reads[slot] = Event {
            space: Space::Register,
            vm: p.vm,
            generation: 0,
            index,
            write: false,
            before: bytes,
            after: bytes,
            before_private: 0,
            after_private: 0,
        }
        .fields(37 + slot);
    }
    let store = memory_store::StorePhase::from_fixed_input(
        enc::encode_store(wide::memory::STORE64, 3, 1, 0),
        CELL + 8,
        p.vm,
        [37, 38, 39, 40],
        100,
        0,
        0,
        CELL + 64,
        0,
        true,
        true,
        false,
        memory_store::MemoryOutcome::Ready,
    )
    .unwrap();
    let mut control = [F::ZERO; memory_store::CONTROL_WIDTH];
    control[0] = F(97);
    control[2] = F(4);
    control[4] = F::ONE;
    control[8] = F::ONE;
    let mut store_residues = Vec::new();
    memory_store::append_residues(
        &mut store_residues,
        store,
        &memory_store::witness(0x1234, true),
        [&reads[0], &reads[1], &reads[2]],
        &f.memory,
        &f.log,
        &control,
    );
    assert_eq!(store_residues.len(), memory_store::CONSTRAINTS);
    assert!(store_residues.iter().all(|v| *v == F::ZERO));
    assert!(f.accepts());
    assert_eq!(f.initialized[AFTER], F(0xff80));
    f.initialized[AFTER] = F(0xffff);
    assert!(
        !f.accepts(),
        "a valid store cannot counterfeit unwritten initialized bytes"
    );
}
