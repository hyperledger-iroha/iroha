//! State-owned instruction arrays never borrow diagnostic or foreign-pool credit.

use super::*;
use crate::error::VmDeferral;

const LIMIT: usize = 128 * 1024 * 1024;

fn artifact() -> Vec<u8> {
    kotodama_lang::compiler::Compiler::new_c10()
        .compile_source("seiyaku Funded { view fn main() authorize(anyone) -> bool { true } }")
        .unwrap()
}

#[test]
fn funded_generic_loading_cannot_borrow_a_warm_diagnostic_cache() {
    let mut program = ProgramMetadata::default().encode();
    program.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
    let mut diagnostic = IVM::new(100);
    diagnostic.load_program(&program).unwrap();
    let budget = AllocationBudget::new(LIMIT);
    let mut funded = IVM::try_new_with_memory_budget(100, &budget).unwrap();
    let original = budget.reserved_bytes();
    budget.set_limit_bytes(original);
    assert!(matches!(
        funded.load_program(&program),
        Err(VMError::Deferred(VmDeferral::AllocationDeferred(_)))
    ));
    assert_eq!(budget.reserved_bytes(), original);
    budget.set_limit_bytes(LIMIT);
    funded.load_program(&program).unwrap();
    let decoded = funded.predecoded.as_ref().unwrap().clone();
    let prepared = funded.prepared.as_ref().unwrap().clone();
    assert!(decoded.belongs_to(&budget));
    assert!(prepared.belongs_to(&budget));
    assert!(!crate::cache_memory::SharedAllocation::ptr_eq(
        &decoded,
        diagnostic.predecoded.as_ref().unwrap(),
    ));
    funded.run().unwrap();
    diagnostic.run().unwrap();
    assert_eq!(funded.remaining_gas(), diagnostic.remaining_gas());
    drop(funded);
    assert!(budget.reserved_bytes() > 0);
    drop((decoded, prepared));
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn prepared_loading_rebinds_foreign_arrays_to_the_actual_vm_pool() {
    let artifact = artifact();
    let first = AllocationBudget::new(LIMIT);
    let contract = crate::prepare_contract_with_memory_budget(&artifact, &first).unwrap();
    assert!(contract.prepared_program().belongs_to(&first));
    let first_charge = first.reserved_bytes();
    let second = AllocationBudget::new(LIMIT);
    let mut vm = IVM::try_new_with_memory_budget(100, &second).unwrap();
    let original = second.reserved_bytes();
    second.set_limit_bytes(original);
    assert!(matches!(
        vm.load_prepared(&contract),
        Err(VMError::Deferred(VmDeferral::AllocationDeferred(_)))
    ));
    assert_eq!(second.reserved_bytes(), original);
    assert_eq!(first.reserved_bytes(), first_charge);
    second.set_limit_bytes(LIMIT);
    vm.load_prepared(&contract).unwrap();
    assert!(vm.predecoded.is_none());
    let rebound = vm.prepared.as_ref().unwrap();
    assert!(rebound.belongs_to(&second));
    assert!(!crate::cache_memory::SharedAllocation::ptr_eq(
        &rebound.ops,
        &contract.prepared_program().ops,
    ));
    for op in contract.instructions().iter() {
        assert_eq!(
            rebound
                .op_at(contract.instruction_entry_pc() + op.pc)
                .unwrap()
                .inst,
            op.inst
        );
    }
    drop(contract);
    assert_eq!(first.reserved_bytes(), 0);
    drop(vm);
    assert_eq!(second.reserved_bytes(), 0);
}

#[test]
fn same_pool_preparation_and_template_keep_one_charge_until_final_borrower() {
    let budget = AllocationBudget::new(LIMIT);
    let contract = crate::prepare_contract_with_memory_budget(&artifact(), &budget).unwrap();
    let mut vm = IVM::try_new_with_memory_budget(100, &budget).unwrap();
    vm.load_prepared(&contract).unwrap();
    assert!(vm.predecoded.is_none());
    let prepared = vm.prepared.as_ref().unwrap().clone();
    assert!(crate::cache_memory::SharedAllocation::ptr_eq(
        &prepared.ops,
        &contract.prepared_program().ops,
    ));
    let instruction_bytes = prepared.ops.allocation_bytes();
    let template = vm.try_runtime_template().unwrap();
    let before = budget.reserved_bytes();
    budget.set_limit_bytes(0);
    vm.reset_from_runtime_template(&template).unwrap();
    assert_eq!(budget.reserved_bytes(), before);
    drop(contract);
    drop(vm);
    assert!(budget.reserved_bytes() > 0);
    drop(template);
    assert_eq!(budget.reserved_bytes(), instruction_bytes);
    drop(prepared);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn both_profiles_scan_and_execute_the_single_prepared_word_stream() {
    use crate::analysis;
    use kotodama_lang::compiler::Compiler;

    for (profile, compiler) in [
        (IvmProfileId::BASELINE_ABI_V1, Compiler::new()),
        (IvmProfileId::C10_ABI_V1, Compiler::new_c10()),
    ] {
        let bytes = compiler
            .compile_source("誓約 Check { view fn answer() authorize(anyone) -> bool { true } }")
            .unwrap();
        let pool = AllocationBudget::new(LIMIT);
        let contract =
            crate::prepare_contract_with_memory_budget_for_profile(&bytes, &pool, profile).unwrap();
        let scan = analysis::analyze_prepared(&contract).unwrap();
        let raw = analysis::analyze_program_for_profile(&bytes, profile).unwrap();
        assert_eq!(scan.instruction_count, raw.instruction_count);
        assert_eq!(scan.registers.reads, raw.registers.reads);
        assert_eq!(scan.registers.writes, raw.registers.writes);
        assert!(
            analysis::prepared_syscall_numbers(&contract)
                .eq(analysis::program_syscall_numbers_for_profile(&bytes, profile).unwrap())
        );
        let mut vm = IVM::try_new_with_memory_budget(150_000, &pool).unwrap();
        vm.load_prepared(&contract).unwrap();
        assert!(vm.predecoded.is_none());
        assert!(crate::cache_memory::SharedAllocation::ptr_eq(
            &vm.prepared.as_ref().unwrap().ops,
            &contract.prepared_program().ops,
        ));
        vm.select_entrypoint("answer").unwrap();
        vm.run().unwrap();
        assert_eq!(vm.public_call_result_word(0).unwrap(), 1);
        assert_eq!(vm.remaining_gas(), 149_964);
        assert_eq!(vm.get_cycle_count(), 9);

        let mut limited = IVM::try_new_with_memory_budget(15, &pool).unwrap();
        limited.load_prepared(&contract).unwrap();
        limited.select_entrypoint("answer").unwrap();
        let entry = limited.pc();
        assert!(matches!(
            limited.run(),
            Err(VMError::Trap(VmTrap::OutOfGas))
        ));
        assert_eq!(limited.remaining_gas(), 0);
        assert_eq!(limited.get_cycle_count(), 2);
        // The entry thunk transfers to the function body, whose second word faults.
        assert!(contract.is_instruction_boundary(limited.pc() - contract.instruction_entry_pc()));
        let mut reference = IVM::new(15);
        reference.load_program_for_profile(&bytes, profile).unwrap();
        reference.select_entrypoint("answer").unwrap();
        assert_eq!(reference.pc(), entry);
        assert!(matches!(
            reference.run(),
            Err(VMError::Trap(VmTrap::OutOfGas))
        ));
        assert_eq!(limited.pc(), reference.pc());
        drop((contract, vm, limited));
        assert_eq!(pool.reserved_bytes(), 0);
    }
}
