//! Exact wrapping effective-address equations for the unfinished memory relation.
//!
//! The two existing 64-bit Boolean source words hold the base and effective
//! address. Two additional signed carries prove the low/high 32-bit additions.
//! This unregistered component does not authenticate its instruction, source
//! register, privacy tag, enable/phase owner, access permission or memory effect.
// TODO: Link the instruction and both words to the unified fetch/register/request
// schedule, including exact native trap priority, before any proof admission.

use super::{F, Sources, wide, word};

pub(super) const WIDTH: usize = 2;
pub(super) const CONSTRAINTS: usize = word::WIDTH + 4;
const RADIX: u64 = 1 << 32;

/// Memory opcode semantics derived from one still-unauthenticated instruction.
#[derive(Clone, Copy)]
pub(super) struct Operation {
    immediate: i8,
}

impl Operation {
    pub(super) fn from_word(instruction: u32) -> Option<Self> {
        match wide::opcode(instruction) {
            wide::memory::LOAD64 | wide::memory::STORE64 => Some(Self {
                immediate: wide::imm8(instruction),
            }),
            _ => None,
        }
    }

    fn immediate_field(self) -> F {
        if self.immediate < 0 {
            F::ZERO.sub(F(u64::from(self.immediate.unsigned_abs())))
        } else {
            F(u64::try_from(self.immediate).expect("nonnegative immediate"))
        }
    }
}

fn signed_field(value: i128) -> F {
    if value < 0 {
        F::ZERO.sub(F(u64::from_le_bytes(
            i128::to_le_bytes(-value)[..8]
                .try_into()
                .expect("low-byte width"),
        )))
    } else {
        F(u64::from_le_bytes(
            i128::to_le_bytes(value)[..8]
                .try_into()
                .expect("low-byte width"),
        ))
    }
}

/// Candidate carries only; the evaluator owns their range and both equalities.
pub(super) fn witness(base: u64, operation: Operation) -> [F; WIDTH] {
    let low = i128::from(base & u64::from(u32::MAX)) + i128::from(operation.immediate);
    let low_carry = low.div_euclid(i128::from(RADIX));
    let high = i128::from(base >> 32) + low_carry;
    let high_carry = high.div_euclid(i128::from(RADIX));
    [signed_field(low_carry), signed_field(high_carry)]
}

pub(super) fn append_residues(
    out: &mut Vec<F>,
    sources: Sources<'_>,
    carries: &[F; WIDTH],
    operation: Operation,
) {
    let start = out.len();
    sources.append_residues(out);
    // Signed carry values are exactly -1, 0 or 1. Each limb equation has
    // integer magnitude below 2*2^32+128, so field zero cannot hide a p-offset.
    for carry in carries {
        out.push(carry.mul(carry.sub(F::ONE)).mul(carry.add(F::ONE)));
    }
    out.push(
        sources
            .half(0, 0)
            .add(operation.immediate_field())
            .sub(sources.half(1, 0))
            .sub(carries[0].mul(F(RADIX))),
    );
    out.push(
        sources
            .half(0, 1)
            .add(carries[0])
            .sub(sources.half(1, 1))
            .sub(carries[1].mul(F(RADIX))),
    );
    debug_assert_eq!(out.len() - start, CONSTRAINTS);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution_proofs::stark::proof_managed_note_stark::degree_audit::measured_maximum_affine_degree_v1;
    use ivm::{IVM, Memory, ProgramMetadata, encoding::wide as enc, ivm_mode};

    fn residues(bits: &[F], carries: &[F; WIDTH], operation: Operation) -> Vec<F> {
        let mut out = Vec::new();
        append_residues(&mut out, Sources::new(bits), carries, operation);
        assert_eq!(out.len(), CONSTRAINTS);
        out
    }

    fn accepts(bits: &[F], carries: &[F; WIDTH], operation: Operation) -> bool {
        residues(bits, carries, operation)
            .into_iter()
            .all(|x| x == F::ZERO)
    }

    fn operation(opcode: u8, last: u8) -> Operation {
        Operation::from_word(enc::encode_rr(opcode, 1, 3, last)).unwrap()
    }

    #[test]
    fn every_signed_immediate_and_limb_boundary_has_unique_wrapping_carries() {
        let bases = [
            0,
            1,
            127,
            128,
            255,
            u64::from(u32::MAX) - 127,
            u64::from(u32::MAX),
            RADIX,
            RADIX + 127,
            (1 << 63) - 1,
            1 << 63,
            u64::MAX - 127,
            u64::MAX - 1,
            u64::MAX,
        ];
        for base in bases {
            for immediate in i8::MIN..=i8::MAX {
                for opcode in [wide::memory::LOAD64, wide::memory::STORE64] {
                    let op = operation(opcode, i8::cast_unsigned(immediate));
                    // Independent machine-integer oracle, not the carry witness formula.
                    let effective = i64::cast_unsigned(
                        u64::cast_signed(base).wrapping_add(i64::from(immediate)),
                    );
                    let bits = word::witness(base, effective);
                    let carries = witness(base, op);
                    assert!(accepts(&bits, &carries, op), "base={base} imm={immediate}");
                    for a in [-1_i128, 0, 1] {
                        for b in [-1_i128, 0, 1] {
                            let candidate = [signed_field(a), signed_field(b)];
                            assert_eq!(accepts(&bits, &candidate, op), candidate == carries);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_address_bit_and_signed_carry_rejects_mutation() {
        for (base, immediate) in [(0, -1_i8), (u64::MAX, 1), (RADIX - 1, 127), (RADIX, -128)] {
            let op = operation(wide::memory::LOAD64, i8::cast_unsigned(immediate));
            let effective = base.wrapping_add_signed(i64::from(immediate));
            let bits = word::witness(base, effective);
            let carries = witness(base, op);
            for index in 0..word::WIDTH {
                let mut changed = bits;
                changed[index] = F::ONE.sub(changed[index]);
                assert!(!accepts(&changed, &carries, op), "bit {index}");
                changed[index] = F(2);
                assert!(!accepts(&changed, &carries, op));
            }
            for index in 0..WIDTH {
                for value in [
                    signed_field(-2),
                    signed_field(-1),
                    F::ZERO,
                    F::ONE,
                    F(2),
                    F(1 << 31),
                ] {
                    if value == carries[index] {
                        continue;
                    }
                    let mut changed = carries;
                    changed[index] = value;
                    assert!(!accepts(&bits, &changed, op));
                }
            }
        }
    }

    #[test]
    fn only_the_two_scalar_memory_opcodes_select_an_imm8_that_cannot_be_dropped() {
        // Every other opcode byte, assigned or not, has no address operation.
        for opcode in 0..=u8::MAX {
            assert_eq!(
                Operation::from_word(enc::encode_rr(opcode, 1, 3, 127)).is_some(),
                matches!(opcode, wide::memory::LOAD64 | wide::memory::STORE64),
                "opcode {opcode:#x}"
            );
        }
        let base = RADIX + 256;
        for last in 1..=u8::MAX {
            for opcode in [wide::memory::LOAD64, wide::memory::STORE64] {
                let scalar = operation(opcode, last);
                let shifted = base.wrapping_add_signed(i64::from(u8::cast_signed(last)));
                assert!(accepts(
                    &word::witness(base, shifted),
                    &witness(base, scalar),
                    scalar
                ));
                // A nonzero final operand byte is always the signed offset.
                assert!(!accepts(
                    &word::witness(base, base),
                    &witness(base, scalar),
                    scalar
                ));
            }
        }
    }

    #[test]
    fn coherent_foreign_inputs_demonstrate_the_missing_outer_linkage() {
        let base = Memory::HEAP_START + 128;
        let first = operation(wide::memory::LOAD64, 8);
        let foreign = operation(wide::memory::STORE64, 16);
        assert!(accepts(
            &word::witness(base, base + 8),
            &witness(base, first),
            first
        ));
        assert!(accepts(
            &word::witness(base + 64, base + 80),
            &witness(base + 64, foreign),
            foreign,
        ));
        assert!(!accepts(
            &word::witness(base, base + 80),
            &witness(base, first),
            first,
        ));
        // A valid address equation is deliberately not instruction/register
        // authority: the unified fetch and packet relation must reject the
        // entire otherwise-valid foreign input above.
    }

    fn native(words: &[u32]) -> IVM {
        let mut bytes = ProgramMetadata {
            mode: ivm_mode::ZK,
            max_cycles: 2,
            ..ProgramMetadata::default()
        }
        .encode();
        for w in words {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        let mut vm = IVM::new(100);
        vm.load_program(&bytes).unwrap();
        vm
    }

    #[test]
    fn address_equations_match_native_signed_offsets_and_wrap() {
        let address = Memory::HEAP_START + 256;
        let value = 0xfedc_ba98_7654_3210_u64;
        for immediate in [-128_i8, -127, -1, 0, 1, 63, 126, 127] {
            for store in [false, true] {
                let word = if store {
                    enc::encode_store(wide::memory::STORE64, 3, 1, immediate)
                } else {
                    enc::encode_load(wide::memory::LOAD64, 1, 3, immediate)
                };
                let base = address.wrapping_sub(i64::cast_unsigned(i64::from(immediate)));
                let op = Operation::from_word(word).unwrap();
                assert!(accepts(
                    &word::witness(base, address),
                    &witness(base, op),
                    op
                ));
                let mut vm = native(&[word, enc::encode_halt()]);
                vm.set_register(3, base);
                if store {
                    vm.set_register(1, value);
                } else {
                    vm.memory.store_u64(address, value).unwrap();
                }
                vm.run().unwrap();
                assert_eq!(vm.memory.load_u64(address).unwrap(), value);
                assert_eq!(vm.registers.get(1), value);
                assert!(!vm.registers.tag(1));
                assert_eq!(
                    (vm.pc(), vm.get_cycle_count(), vm.gas_remaining),
                    (8, 2, 97)
                );
            }
        }
        for (base, imm) in [(u64::MAX, 1_i8), (u64::MAX - 7, 8)] {
            let word = enc::encode_load(wide::memory::LOAD64, 1, 3, imm);
            let op = Operation::from_word(word).unwrap();
            assert!(accepts(&word::witness(base, 0), &witness(base, op), op));
            let mut vm = native(&[word, enc::encode_halt()]);
            let code = vm.memory.load_u64(0).unwrap();
            vm.set_register(3, base);
            vm.run().unwrap();
            assert_eq!(vm.registers.get(1), code);
        }
    }

    #[test]
    fn address_bank_uses_two_fields_and_has_exact_degree_three() {
        assert_eq!(WIDTH, 2);
        assert_eq!(CONSTRAINTS, 132);
        for opcode in [wide::memory::LOAD64, wide::memory::STORE64] {
            let op = operation(opcode, 128);
            assert_eq!(
                measured_maximum_affine_degree_v1(
                    [opcode; 32],
                    [word::WIDTH + WIDTH, 0, 0, 0, 0],
                    4,
                    3,
                    |row, _, _, _, _| {
                        let carries = [row[word::WIDTH], row[word::WIDTH + 1]];
                        Ok::<_, core::convert::Infallible>(residues(
                            &row[..word::WIDTH],
                            &carries,
                            op,
                        ))
                    }
                ),
                3
            );
        }
        // Conditional geometry only: two additional fields under the unchanged
        // 136-query/two-DEEP-point encoding cost exactly 4480 bytes. This is not
        // a new integrated profile, which still needs request/owner linkage.
        assert_eq!(WIDTH * (136 * 2 * 8 + 2 * 32), 4480);
    }
}
