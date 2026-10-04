//! Exact public register observations for the native scalar component.
//!
//! This is a shape lookup over immutable instruction bits. It neither computes
//! a result nor accepts a claimed opcode role, operand value, tag or gas charge.

use crate::instruction::wide;

/// Original registers read by one admitted public scalar instruction.
///
/// The first source always precedes the optional second source. Immediate and
/// unary instructions do not read their encoded final byte as a register.
/// The ordinary interpreter owns value/tag updates and gas; this lookup only
/// bounds the optional native packet producer's current observation coverage.
/// Instructions outside the thirty-one-opcode component return `None`.
#[must_use]
pub fn public_scalar_operands(instruction: u32) -> Option<(usize, Option<usize>)> {
    let right = match wide::opcode(instruction) {
        wide::arithmetic::ADD
        | wide::arithmetic::SUB
        | wide::arithmetic::AND
        | wide::arithmetic::OR
        | wide::arithmetic::XOR
        | wide::arithmetic::SLT
        | wide::arithmetic::SLTU
        | wide::arithmetic::SEQ
        | wide::arithmetic::SNE
        | wide::arithmetic::MIN
        | wide::arithmetic::MAX
        | wide::arithmetic::SLL
        | wide::arithmetic::SRL
        | wide::arithmetic::SRA
        | wide::arithmetic::ROTL
        | wide::arithmetic::ROTR
        | wide::arithmetic::MUL
        | wide::arithmetic::MULH
        | wide::arithmetic::MULHU
        | wide::arithmetic::MULHSU => Some(wide::rs2(instruction)),
        wide::arithmetic::ADDI
        | wide::arithmetic::ANDI
        | wide::arithmetic::ORI
        | wide::arithmetic::XORI
        | wide::arithmetic::NEG
        | wide::arithmetic::NOT
        | wide::arithmetic::ROTL_IMM
        | wide::arithmetic::ROTR_IMM
        | wide::arithmetic::POPCNT
        | wide::arithmetic::CLZ
        | wide::arithmetic::CTZ => None,
        _ => return None,
    };
    Some((wide::rs1(instruction), right))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encoding::wide as enc, gas};

    const BINARY: [u8; 20] = [
        wide::arithmetic::ADD,
        wide::arithmetic::SUB,
        wide::arithmetic::AND,
        wide::arithmetic::OR,
        wide::arithmetic::XOR,
        wide::arithmetic::SLT,
        wide::arithmetic::SLTU,
        wide::arithmetic::SEQ,
        wide::arithmetic::SNE,
        wide::arithmetic::MIN,
        wide::arithmetic::MAX,
        wide::arithmetic::SLL,
        wide::arithmetic::SRL,
        wide::arithmetic::SRA,
        wide::arithmetic::ROTL,
        wide::arithmetic::ROTR,
        wide::arithmetic::MUL,
        wide::arithmetic::MULH,
        wide::arithmetic::MULHU,
        wide::arithmetic::MULHSU,
    ];
    const IMMEDIATE: [u8; 6] = [
        wide::arithmetic::ADDI,
        wide::arithmetic::ANDI,
        wide::arithmetic::ORI,
        wide::arithmetic::XORI,
        wide::arithmetic::ROTL_IMM,
        wide::arithmetic::ROTR_IMM,
    ];
    const UNARY: [u8; 5] = [
        wide::arithmetic::NEG,
        wide::arithmetic::NOT,
        wide::arithmetic::POPCNT,
        wide::arithmetic::CLZ,
        wide::arithmetic::CTZ,
    ];

    #[test]
    fn exact_source_order_preserves_r0_aliases_and_never_reads_an_immediate_register() {
        for (destination, left, right) in [(0, 0, 0), (4, 4, 5), (5, 4, 5), (255, 255, 255)] {
            for opcode in BINARY {
                let word = enc::encode_rr(opcode, destination, left, right);
                assert_eq!(
                    public_scalar_operands(word),
                    Some((usize::from(left), Some(usize::from(right)))),
                );
            }
            for opcode in IMMEDIATE {
                for immediate in [i8::MIN, -1, 0, i8::MAX] {
                    let word = enc::encode_ri(opcode, destination, left, immediate);
                    assert_eq!(
                        public_scalar_operands(word),
                        Some((usize::from(left), None))
                    );
                }
            }
            for opcode in UNARY {
                // The final byte is ignored even when it names a nonzero or
                // aliased register; an observation here would be fabricated.
                let word = enc::encode_rr(opcode, destination, left, right);
                assert_eq!(
                    public_scalar_operands(word),
                    Some((usize::from(left), None))
                );
            }
        }
    }

    #[test]
    fn only_exact_thirty_one_scalar_opcodes_are_admitted() {
        for opcode in u8::MIN..=u8::MAX {
            let word = enc::encode_rr(opcode, 4, 5, 6);
            assert_eq!(
                public_scalar_operands(word).is_some(),
                BINARY.contains(&opcode) || IMMEDIATE.contains(&opcode) || UNARY.contains(&opcode),
                "opcode {opcode:#x}",
            );
        }
        for opcode in [
            wide::arithmetic::DIV,
            wide::arithmetic::REM,
            wide::arithmetic::CMOV,
            wide::system::GETGAS,
            wide::control::BEQ,
            wide::control::JAL,
            wide::memory::LOAD64,
            wide::memory::LDI64,
        ] {
            assert!(public_scalar_operands(enc::encode_rr(opcode, 4, 5, 6)).is_none());
        }
    }

    #[test]
    fn ordinary_artifact_gas_descriptor_preserves_each_scalar_tariff() {
        for opcode in BINARY.into_iter().chain(IMMEDIATE).chain(UNARY) {
            let word = enc::encode_rr(opcode, 4, 5, 6);
            let expected = match opcode {
                wide::arithmetic::SLT
                | wide::arithmetic::SLTU
                | wide::arithmetic::SEQ
                | wide::arithmetic::SNE
                | wide::arithmetic::ROTL
                | wide::arithmetic::ROTR
                | wide::arithmetic::ROTL_IMM
                | wide::arithmetic::ROTR_IMM => 2,
                wide::arithmetic::MUL
                | wide::arithmetic::MULH
                | wide::arithmetic::MULHU
                | wide::arithmetic::MULHSU => 3,
                wide::arithmetic::POPCNT | wide::arithmetic::CLZ | wide::arithmetic::CTZ => 6,
                _ => 1,
            };
            assert_eq!(gas::cost_of(word), Some(expected));
        }
    }
}
