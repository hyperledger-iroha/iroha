//! Shared scalar algorithms over original native operands and public code.

use super::*;

/// Every retained temporary is scrubbed, including on witness construction unwind.
struct Temporary<const N: usize>([F; N]);
impl<const N: usize> Drop for Temporary<N> {
    fn drop(&mut self) {
        for value in &mut self.0 {
            value.zeroize_v1();
        }
    }
}

/// The native component covers straight-line public arithmetic and bit operations.
/// Shared scalar equations retain their wider diagnostic instruction surface.
pub(in super::super) fn supported(instruction: u32) -> bool {
    ivm::execution_packets::public_scalar_operands(instruction).is_some()
}

pub(in super::super) fn fill(
    row: &mut [F; super::super::WIDTH],
    instruction: Option<u32>,
    original_left: u64,
    original_right: u64,
) {
    let (left, right, arithmetic_opcode, predicate) = match instruction {
        Some(instruction) => {
            assert!(supported(instruction), "closed native scalar component");
            let (left, right) = if wide::opcode(instruction) == wide::arithmetic::NEG {
                (0, original_left)
            } else {
                (
                    original_left,
                    right_immediate(instruction).unwrap_or(original_right),
                )
            };
            let arithmetic_opcode = if is_alu(instruction) {
                alu_opcode(instruction)
            } else {
                wide::arithmetic::ADD
            };
            let predicate = comparison_predicate(instruction).map_or(0, |predicate| {
                [
                    wide::control::BEQ,
                    wide::control::BNE,
                    wide::control::BLT,
                    wide::control::BGE,
                    wide::control::BLTU,
                    wide::control::BGEU,
                ][predicate]
            });
            (left, right, arithmetic_opcode, predicate)
        }
        None => (0, 0, wide::arithmetic::ADD, 0),
    };
    let source = Temporary(word::witness(left, right));
    row[SCALAR + SOURCES..SCALAR + ALU].copy_from_slice(&source.0);
    let arithmetic = Temporary(alu::witness(arithmetic_opcode, left, right));
    row[SCALAR + ALU..SCALAR + COMPARE].copy_from_slice(&arithmetic.0);
    let comparison = Temporary(branch::bank_witness(predicate, left, right));
    row[SCALAR + COMPARE..SCALAR + PRODUCT_DIGITS].copy_from_slice(&comparison.0);
    let digits = Temporary(multiply::product_digits(left, right));
    row[SCALAR + PRODUCT_DIGITS..SCALAR + MULTIPLY].copy_from_slice(&digits.0);
    let product = Temporary(multiply::witness(left, right, &digits.0, true));
    row[SCALAR + MULTIPLY..SCALAR + COUNT].copy_from_slice(&product.0);
    // Only the immutable CLZ artifact word reverses the canonical prefix bank.
    // All other operations, including padding, retain the low-bit traversal.
    let leading = instruction.is_some_and(|word| wide::opcode(word) == wide::arithmetic::CLZ);
    let counts = Temporary(bit_count::witness(&source.0[..64], leading));
    row[SCALAR + COUNT..SCALAR + MOVE].copy_from_slice(&counts.0);
    let population = F(u64::from(left.count_ones()));
    row[SCALAR + MOVE_ZERO] = F(u64::from(left == 0));
    row[SCALAR + MOVE_INVERSE] = population.inv().unwrap_or(F::ZERO);
    // This public artifact word selects the existing barrel algorithm. Its
    // equations independently constrain direction, arithmetic fill and counts;
    // this witness computation never supplies an expected destination value.
    let shift_opcode = instruction
        .filter(|instruction| shift_kind(*instruction).is_some())
        .map_or(wide::arithmetic::SLL, wide::opcode);
    let shifted = Temporary(shift::bank_witness(shift_opcode, left, right));
    row[SCALAR + SHIFT..super::super::WIDTH].copy_from_slice(&shifted.0);
}
