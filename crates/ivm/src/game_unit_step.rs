//! Bounded direct-IVM movement kernel for the independent-root unit prototype.
//!
//! This is a deliberately small vertical slice, not the Tides combat kernel.
//! Native callers own authorization, records, sequencing, transfer receipts and
//! durable writes. This module performs only register input/output around a
//! fixed existing-ISA artifact: no host, syscall, ledger access or proof service.
//! The artifact hash is a pin, not an execution proof or an approval mechanism.
//!
//! TODO: qualify a complete game transition separately; this movement fixture
//! does not establish combat, production throughput or proof performance.

use std::sync::OnceLock;

use iroha_crypto::Hash;

use crate::{IVM, ProgramMetadata, VMError, encoding::wide as enc, instruction::wide as op};

/// Hard per-invocation gas ceiling, including the halt instruction. Gas is the only
/// execution budget; the artifact is not a ZK program and declares no cycle bound.
pub const GAS_LIMIT: u64 = 128;

/// Typed subset of a native unit record consumed by this prototype.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitStepInput {
    /// Current hit points. Zero means the unit does not move.
    pub hp: u32,
    /// Signed position in the prototype's single-dimensional coordinate space.
    pub x: i32,
}

/// VM-computed fields and measured resource consumption.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitStepOutput {
    /// Hit points are preserved by this movement-only artifact.
    pub hp: u32,
    /// A living unit advances by one, saturating at [`i32::MAX`].
    pub x: i32,
    /// Actual IVM gas consumed by the artifact, excluding native state I/O.
    pub gas: u64,
    /// Actual IVM instruction cycles consumed by the artifact.
    pub cycles: u64,
}

fn instructions() -> [u32; 8] {
    // Registers: r1=hp, r2=x (signed i64 extension), r3/r4=scratch.
    // All semantic constants are inside the hashed artifact.
    [
        enc::encode_branch(op::control::BEQ, 1, 0, 7),
        enc::encode_ri(op::arithmetic::ADDI, 3, 0, 1),
        enc::encode_ri(op::arithmetic::ADDI, 4, 0, 31),
        enc::encode_rr(op::arithmetic::SLL, 3, 3, 4),
        enc::encode_ri(op::arithmetic::ADDI, 3, 3, -1),
        enc::encode_branch(op::control::BGE, 2, 3, 2),
        enc::encode_ri(op::arithmetic::ADDI, 2, 2, 1),
        enc::encode_halt(),
    ]
}

fn artifact() -> &'static [u8] {
    static ARTIFACT: OnceLock<Vec<u8>> = OnceLock::new();
    ARTIFACT.get_or_init(|| {
        let mut bytes = ProgramMetadata::default().encode();
        for word in instructions() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    })
}

/// Return the Iroha hash of the exact fixed artifact loaded by [`step_unit`].
///
/// Native callers must compare this with their committed kernel pin before
/// invoking the adapter. This function does not authorize arbitrary bytecode.
#[must_use]
pub fn kernel_hash() -> [u8; 32] {
    let hash = Hash::new(artifact());
    let mut bytes = [0; 32];
    bytes.copy_from_slice(hash.as_ref());
    bytes
}

/// Execute one bounded movement step using actual IVM instructions.
///
/// Rust supplies typed inputs and extracts the outputs; movement and its bound
/// execute inside the VM. The fixed artifact never writes `r1`; its only write
/// to `r2` adds one after checking the signed maximum. These properties keep
/// the register projections inside the original `u32`/`i32` input domains.
///
/// # Errors
///
/// Returns the IVM loader or execution error without committing any native
/// record. The caller must perform its durable update only after success.
pub fn step_unit(input: UnitStepInput) -> Result<UnitStepOutput, VMError> {
    let mut vm = IVM::new(GAS_LIMIT);
    vm.load_program(artifact())?;
    vm.set_register(1, u64::from(input.hp));
    vm.set_register(2, i64::from(input.x) as u64);
    vm.run()?;
    Ok(UnitStepOutput {
        hp: vm.register(1) as u32,
        x: vm.register(2) as i64 as i32,
        gas: GAS_LIMIT - vm.remaining_gas(),
        cycles: vm.get_cycle_count(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    /// The fixed artifact has eight instructions and no backward branch.
    const MAX_PATH_CYCLES: u64 = 8;

    fn oracle(input: UnitStepInput) -> (u32, i32) {
        (
            input.hp,
            if input.hp == 0 {
                input.x
            } else {
                input.x.saturating_add(1)
            },
        )
    }

    #[test]
    fn boundary_inputs_match_independent_oracle() {
        for hp in [0, 1, u32::MAX] {
            for x in [i32::MIN, -1, 0, 1, i32::MAX - 1, i32::MAX] {
                let input = UnitStepInput { hp, x };
                let output = step_unit(input).expect("fixed artifact executes");
                assert_eq!((output.hp, output.x), oracle(input));
                assert!(output.gas > 0 && output.gas <= GAS_LIMIT);
                assert!(output.cycles > 0 && output.cycles <= MAX_PATH_CYCLES);
            }
        }
    }

    #[test]
    fn seeded_inputs_match_independent_oracle() {
        let mut seed = 0x5431_4445_535f_4956_u64;
        for _ in 0..64 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let input = UnitStepInput {
                hp: (seed >> 32) as u32,
                x: seed as i32,
            };
            let output = step_unit(input).expect("fixed artifact executes");
            assert_eq!((output.hp, output.x), oracle(input));
        }
    }

    #[test]
    fn repeated_steps_have_no_hidden_host_state() {
        let input = UnitStepInput { hp: 9, x: -3 };
        let first = step_unit(input).expect("first execution");
        let second = step_unit(input).expect("independent execution");
        assert_eq!(first, second);
        let next = step_unit(UnitStepInput {
            hp: first.hp,
            x: first.x,
        })
        .expect("successor execution");
        assert_eq!((next.hp, next.x), (9, -1));
    }

    #[test]
    fn exact_artifact_pin_and_no_effectful_instructions() {
        let mut expected = [0; 32];
        expected.copy_from_slice(Hash::new(artifact()).as_ref());
        assert_eq!(kernel_hash(), expected);
        assert_eq!(kernel_hash(), kernel_hash());
        let allowed = [
            op::control::BEQ,
            op::arithmetic::ADDI,
            op::arithmetic::SLL,
            op::control::BGE,
            op::control::HALT,
        ];
        for word in instructions() {
            assert!(allowed.contains(&((word >> 24) as u8)));
        }
        let mut altered = artifact().to_vec();
        *altered.last_mut().expect("nonempty artifact") ^= 1;
        assert_ne!(Hash::new(altered), Hash::new(artifact()));
    }
}
