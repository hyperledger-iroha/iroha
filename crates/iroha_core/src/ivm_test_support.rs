//! Authenticated V1 Unit-call fixtures shared by Core's nonshipping test harness.
//!
//! Every first-release interface runs the frozen baseline engine (`SyscallPolicy::AbiV1`), so a
//! program submitted through one carries the baseline header and encoding ([`baseline_header`],
//! [`baseline_unit_return`]). The C10 encodings ([`unit_return`]) serve tests that load the C10
//! engine directly.

/// The profile an artifact's header declares, or the interface profile when it declares no
/// implemented one (whose parser then reports the canonical rejection).
///
/// Core tests read and run fixtures of either engine: Kotodama's default target and every live
/// interface are the baseline, while fixtures that drive the C10 engine directly carry the C10
/// header. Reading a fixture under its declared profile keeps each in its own engine.
pub fn declared_profile(artifact: &[u8]) -> ivm::IvmProfileId {
    ivm::IvmProfileId::declared(artifact)
        .unwrap_or(ivm::IvmProfileId::of_policy(ivm::SyscallPolicy::AbiV1))
}

/// Find the next aligned INPUT allocation through read-only host preflights.
/// Reserving all HEAP headroom prevents an envelope from falling back to HEAP.
pub fn next_host_input_address(context: &dyn ivm::host::SyscallContext) -> u64 {
    let heap_headroom = context.heap_limit() - context.heap_allocated_len();
    let mut low = 0;
    let mut high = ivm::Memory::INPUT_SIZE;
    while low < high {
        let len = low + (high - low).div_ceil(2);
        match context
            .preflight_host_tlv_allocations_with_reserved_heap(&[len as usize], heap_headroom)
        {
            Ok(()) => low = len,
            Err(ivm::VMError::Trap(ivm::error::VmTrap::OutOfMemory)) => high = len - 1,
            Err(error) => panic!("unexpected probe failure: {error}"),
        }
    }
    ivm::Memory::INPUT_START + ivm::Memory::INPUT_SIZE - low
}

/// Parse a fixture's header under [`declared_profile`].
pub fn parse_declared(
    artifact: &[u8],
) -> Result<ivm_abi::metadata::ParsedProgramMetadata, ivm::VMError> {
    ivm::ProgramMetadata::parse_for_profile(artifact, declared_profile(artifact))
}

/// Prepare a contract fixture under [`declared_profile`].
pub fn prepare_declared(
    artifact: std::sync::Arc<[u8]>,
) -> Result<ivm::PreparedContract, ivm::ContractArtifactError> {
    let profile = declared_profile(&artifact);
    ivm::prepare_contract_for_profile(artifact, profile)
}

/// Prepare a contract fixture under [`declared_profile`] against an execution budget.
pub fn prepare_declared_with_memory_budget(
    artifact: &[u8],
    budget: &iroha_allocation::AllocationBudget,
) -> Result<ivm::PreparedContract, ivm::ContractArtifactError> {
    ivm::prepare_contract_with_memory_budget_for_profile(
        artifact,
        budget,
        declared_profile(artifact),
    )
}

/// Descriptor for a zero-argument function returning exactly one initialized Unit word.
pub fn unit_callable(entry_pc: u64) -> ivm::call::EmbeddedCallableV1 {
    ivm::call::EmbeddedCallableV1 {
        entry_pc,
        frame_bytes: 0,
        arguments: ivm::call::CallSchemaV1::empty(),
        results: ivm::call::CallSchemaV1::unit(),
    }
}

/// A complete Unit return through the caller-owned result table and protected return address.
pub fn unit_return() -> [u8; 16] {
    use ivm::{
        encoding::wide,
        instruction::wide::{arithmetic, control, memory},
    };
    let instructions = [
        wide::encode_store(memory::STORE64, 12, 0, 0),
        wide::encode_ri(arithmetic::ADDI, 10, 12, 0),
        wide::encode_ri(arithmetic::ADDI, 11, 0, 1),
        wide::encode_ri(control::JALR, 0, 1, 0),
    ];
    let mut code = [0; 16];
    for (slot, instruction) in code.as_chunks_mut::<4>().0.iter_mut().zip(instructions) {
        slot.copy_from_slice(&instruction.to_le_bytes());
    }
    code
}

/// The fixed baseline header of `meta`: the 49-byte layout with a logical vector length of zero
/// and the baseline ABI hash. A baseline program declares a positive `max_cycles`.
pub fn baseline_header(meta: ivm::ProgramMetadata) -> Vec<u8> {
    meta.encode_for_profile(ivm::IvmProfileId::BASELINE_ABI_V1, 0)
}

/// A baseline generic program: the default header with `max_cycles` and `words`.
pub fn baseline_program(max_cycles: u64, words: &[u32]) -> Vec<u8> {
    let mut program = baseline_header(ivm::ProgramMetadata {
        max_cycles,
        ..ivm::ProgramMetadata::default()
    });
    for word in words {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

/// A baseline syscall instruction: the 8-bit `SCALL` for numbers up to `0xFF`, the 24-bit
/// `SYSTEM` form above.
pub fn baseline_syscall(number: u32) -> u32 {
    use ivm_abi::baseline::{encoding::wide, instruction::wide::system};
    match u8::try_from(number) {
        Ok(short) => wide::encode_sys(system::SCALL, short),
        Err(_) => wide::encode_syscallx(number),
    }
}

/// The baseline encoding of [`unit_return`]: the protected return is the register form of `JALR`.
pub fn baseline_unit_return() -> [u8; 16] {
    use ivm_abi::baseline::{
        encoding::wide,
        instruction::wide::{arithmetic, control, memory},
    };
    let instructions = [
        wide::encode_store(memory::STORE64, 12, 0, 0),
        wide::encode_ri(arithmetic::ADDI, 10, 12, 0),
        wide::encode_ri(arithmetic::ADDI, 11, 0, 1),
        wide::encode_rr(control::JALR, 0, 1, 0),
    ];
    let mut code = [0; 16];
    for (slot, instruction) in code.as_chunks_mut::<4>().0.iter_mut().zip(instructions) {
        slot.copy_from_slice(&instruction.to_le_bytes());
    }
    code
}

#[test]
fn baseline_fixtures_are_admitted_by_the_interface_profile() {
    let program = baseline_program(
        10,
        &[
            baseline_syscall(0x22),
            ivm_abi::baseline::encoding::encode_halt(),
        ],
    );
    let parsed = ivm::ProgramMetadata::parse_for_policy(&program, ivm::SyscallPolicy::AbiV1)
        .expect("the interface parser reads a baseline header");
    assert_eq!(parsed.profile, ivm::IvmProfileId::BASELINE_ABI_V1);
    assert_eq!(parsed.metadata.max_cycles, 10);
    assert_eq!(
        ivm::IvmEngine::Baseline.syscall_number(baseline_syscall(0x22)),
        Some(0x22)
    );
    assert_eq!(
        ivm::IvmEngine::Baseline.syscall_number(baseline_syscall(0x01_0024)),
        Some(0x01_0024)
    );
    assert_eq!(baseline_unit_return().len(), unit_return().len());
}

#[test]
fn declared_fixtures_keep_their_engine() {
    let baseline = baseline_program(10, &[ivm_abi::baseline::encoding::encode_halt()]);
    assert_eq!(
        declared_profile(&baseline),
        ivm::IvmProfileId::BASELINE_ABI_V1
    );
    assert_eq!(
        parse_declared(&baseline).expect("baseline header").profile,
        ivm::IvmProfileId::BASELINE_ABI_V1
    );
    let mut c10 = ivm::ProgramMetadata::default().encode();
    c10.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    assert_eq!(declared_profile(&c10), ivm::IvmProfileId::C10_ABI_V1);
    assert_eq!(
        parse_declared(&c10).expect("C10 header").profile,
        ivm::IvmProfileId::C10_ABI_V1
    );
    // Without an implemented profile the interface parser refuses the bytes.
    assert_eq!(
        declared_profile(b"IVM\0"),
        ivm::IvmProfileId::of_policy(ivm::SyscallPolicy::AbiV1)
    );
    assert!(parse_declared(b"IVM\0").is_err());
}
