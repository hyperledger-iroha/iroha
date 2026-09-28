//! ABI V1 inventory of VM step relations required by a native execution proof.
//!
//! This is an admission-coverage guard, not a proof system. In particular, the
//! classifications below do not constrain any witness, polynomial, memory
//! permutation, syscall, Fiat-Shamir transcript, or FRI opening. They must not
//! be used to accept `IvmProved` transactions.

use super::wide;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepEffectV1 {
    ScalarRegisters,
    MemoryAndRegisters,
    Control,
    HostCall,
    VectorRegisters,
    VectorLength,
    ParallelMarker,
    CryptographicPrimitive,
    ZkFieldOrAssertion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PcTransitionV1 {
    Sequential,
    ConditionalRelative8,
    // JAL writes `rd` unless it is r0; protected rd=r1 also pushes a call.
    DirectRelative16WithOptionalLink,
    DirectRelative24,
    DirectRelative24AndLink,
    IndirectRegisterOrStrictTrap,
    // JALR masks the target; protected returns may halt at the outer sentinel.
    IndirectMaskedOrProtectedReturn,
    // HALT succeeds for raw code but traps inside a protected contract call.
    HaltOrStrictReturnTrap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RelationObligationV1 {
    effect: StepEffectV1,
    pc: PcTransitionV1,
}

impl RelationObligationV1 {
    const fn new(effect: StepEffectV1, pc: PcTransitionV1) -> Self {
        Self { effect, pc }
    }
}

/// Return the relation and PC-transition obligations for one admitted V1 opcode.
///
/// Keep this list independent of `wide::is_valid_opcode`: the compile-time
/// assertion below makes any changed admission set require an explicit proof
/// relation inventory review. Each admitted opcode appears in exactly one arm.
#[deny(unreachable_patterns)]
const fn relation_obligation_v1(op: u8) -> Option<RelationObligationV1> {
    use PcTransitionV1 as Pc;
    use StepEffectV1 as Effect;
    let relation = match op {
        wide::arithmetic::ADD
        | wide::arithmetic::SUB
        | wide::arithmetic::AND
        | wide::arithmetic::OR
        | wide::arithmetic::XOR
        | wide::arithmetic::SLL
        | wide::arithmetic::SRL
        | wide::arithmetic::SRA
        | wide::arithmetic::SLT
        | wide::arithmetic::SLTU
        | wide::arithmetic::CMOV
        | wide::arithmetic::NOT
        | wide::arithmetic::NEG
        | wide::arithmetic::SEQ
        | wide::arithmetic::SNE
        | wide::arithmetic::MUL
        | wide::arithmetic::MULH
        | wide::arithmetic::MULHU
        | wide::arithmetic::MULHSU
        | wide::arithmetic::DIV
        | wide::arithmetic::DIVU
        | wide::arithmetic::REM
        | wide::arithmetic::REMU
        | wide::arithmetic::ROTL
        | wide::arithmetic::ROTR
        | wide::arithmetic::POPCNT
        | wide::arithmetic::CLZ
        | wide::arithmetic::CTZ
        | wide::arithmetic::ISQRT
        | wide::arithmetic::MIN
        | wide::arithmetic::MAX
        | wide::arithmetic::ABS
        | wide::arithmetic::DIV_CEIL
        | wide::arithmetic::GCD
        | wide::arithmetic::MEAN
        | wide::arithmetic::ADDI
        | wide::arithmetic::ANDI
        | wide::arithmetic::ORI
        | wide::arithmetic::XORI
        | wide::arithmetic::CMOVI
        | wide::arithmetic::ROTL_IMM
        | wide::arithmetic::ROTR_IMM
        | wide::system::GETGAS => {
            RelationObligationV1::new(Effect::ScalarRegisters, Pc::Sequential)
        }
        wide::memory::LOAD64
        | wide::memory::STORE64
        | wide::memory::LOAD128
        | wide::memory::STORE128
        | wide::memory::LDLIT
        | wide::memory::LDI64 => {
            RelationObligationV1::new(Effect::MemoryAndRegisters, Pc::Sequential)
        }
        wide::control::BEQ
        | wide::control::BNE
        | wide::control::BLT
        | wide::control::BGE
        | wide::control::BLTU
        | wide::control::BGEU => {
            RelationObligationV1::new(Effect::Control, Pc::ConditionalRelative8)
        }
        wide::control::JAL => {
            RelationObligationV1::new(Effect::Control, Pc::DirectRelative16WithOptionalLink)
        }
        wide::control::JR => {
            RelationObligationV1::new(Effect::Control, Pc::IndirectRegisterOrStrictTrap)
        }
        wide::control::JALR => {
            RelationObligationV1::new(Effect::Control, Pc::IndirectMaskedOrProtectedReturn)
        }
        wide::control::HALT => {
            RelationObligationV1::new(Effect::Control, Pc::HaltOrStrictReturnTrap)
        }
        wide::control::JMP => RelationObligationV1::new(Effect::Control, Pc::DirectRelative24),
        wide::control::JALS => {
            RelationObligationV1::new(Effect::Control, Pc::DirectRelative24AndLink)
        }
        wide::system::SCALL | wide::system::SYSTEM => {
            RelationObligationV1::new(Effect::HostCall, Pc::Sequential)
        }
        wide::crypto::VADD32
        | wide::crypto::VADD64
        | wide::crypto::VAND
        | wide::crypto::VXOR
        | wide::crypto::VOR
        | wide::crypto::VROT32 => {
            RelationObligationV1::new(Effect::VectorRegisters, Pc::Sequential)
        }
        wide::crypto::SETVL => RelationObligationV1::new(Effect::VectorLength, Pc::Sequential),
        wide::crypto::PARBEGIN | wide::crypto::PAREND => {
            RelationObligationV1::new(Effect::ParallelMarker, Pc::Sequential)
        }
        wide::crypto::SHA256BLOCK
        | wide::crypto::SHA3BLOCK
        | wide::crypto::POSEIDON2
        | wide::crypto::POSEIDON6
        | wide::crypto::AESENC
        | wide::crypto::AESDEC
        | wide::crypto::BLAKE2S
        | wide::crypto::ED25519VERIFY
        | wide::crypto::ED25519BATCHVERIFY
        | wide::crypto::ECDSAVERIFY
        | wide::crypto::DILITHIUMVERIFY => {
            RelationObligationV1::new(Effect::CryptographicPrimitive, Pc::Sequential)
        }
        wide::zk::ASSERT
        | wide::zk::ASSERT_EQ
        | wide::zk::FADD
        | wide::zk::FSUB
        | wide::zk::FMUL
        | wide::zk::FINV
        | wide::zk::ASSERT_RANGE => {
            RelationObligationV1::new(Effect::ZkFieldOrAssertion, Pc::Sequential)
        }
        _ => return None,
    };
    Some(relation)
}

// This is deliberately checked during ordinary library compilation, not just
// in tests. `is_valid_opcode` is the prepared/runtime admission gate. A newly
// admitted value cannot compile until its relation obligation is inventoried;
// a retired value cannot silently remain in the inventory.
const _: () = {
    let mut candidate = 0_u16;
    while candidate < 256 {
        let opcode = candidate as u8;
        let inventoried = match relation_obligation_v1(opcode) {
            Some(obligation) => {
                // Read both obligations in ordinary compilation so neither
                // dimension can quietly become a test-only description.
                let _ = obligation.effect;
                let _ = obligation.pc;
                true
            }
            None => false,
        };
        assert!(wide::is_valid_opcode(opcode) == inventoried);
        candidate += 1;
    }
};

// TODO: Build the single native IVM AIR from this inventory and constrain every
// opcode's exact register value/tag update, memory ordering/pointer validity,
// syscall/valcom computation, gas debit, PC edge, and terminal outcome. The
// proof must also constrain fetch and malformed-opcode traps, missing HALT,
// cycle limits, failed assertions, private-trace masking, and padded ZK cycles.
// Commit the complete trace, establish polynomial degree/composition bounds,
// and verify transcript/FRI queries before production IvmProved admission.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admitted_v1_opcodes_have_exactly_one_relation_obligation() {
        let covered = (0..=u8::MAX)
            .filter(|opcode| relation_obligation_v1(*opcode).is_some())
            .count();
        assert_eq!(covered, 90);
        for opcode in 0..=u8::MAX {
            assert_eq!(
                wide::is_valid_opcode(opcode),
                relation_obligation_v1(opcode).is_some(),
                "opcode 0x{opcode:02x} admission/relation mismatch"
            );
        }
    }

    #[test]
    fn control_relations_include_every_direct_indirect_and_terminal_edge() {
        use PcTransitionV1 as Pc;
        let cases = [
            (wide::control::BEQ, Pc::ConditionalRelative8),
            (wide::control::BNE, Pc::ConditionalRelative8),
            (wide::control::BLT, Pc::ConditionalRelative8),
            (wide::control::BGE, Pc::ConditionalRelative8),
            (wide::control::BLTU, Pc::ConditionalRelative8),
            (wide::control::BGEU, Pc::ConditionalRelative8),
            (wide::control::JAL, Pc::DirectRelative16WithOptionalLink),
            (wide::control::JR, Pc::IndirectRegisterOrStrictTrap),
            (wide::control::JALR, Pc::IndirectMaskedOrProtectedReturn),
            (wide::control::HALT, Pc::HaltOrStrictReturnTrap),
            (wide::control::JMP, Pc::DirectRelative24),
            (wide::control::JALS, Pc::DirectRelative24AndLink),
        ];
        for &(opcode, edge) in &cases {
            assert_eq!(relation_obligation_v1(opcode).unwrap().pc, edge);
        }
        let classified_control = (0..=u8::MAX)
            .filter(|opcode| {
                relation_obligation_v1(*opcode)
                    .is_some_and(|obligation| obligation.effect == StepEffectV1::Control)
            })
            .collect::<Vec<_>>();
        let mut expected_control = cases.iter().map(|(opcode, _)| *opcode).collect::<Vec<_>>();
        expected_control.sort_unstable();
        assert_eq!(classified_control, expected_control);
        assert_eq!(
            relation_obligation_v1(wide::system::SCALL).unwrap().effect,
            StepEffectV1::HostCall
        );
        assert_eq!(
            relation_obligation_v1(wide::system::SYSTEM).unwrap().effect,
            StepEffectV1::HostCall
        );
    }
}
