//! Exact saved-slot geometry, conservative scope and whole-artifact attribution.
use super::*;
use crate::session::{CompileOutput, CompileRequest, CompilerSession};

pub(super) fn compile(source: &str, scalar: bool) -> CompileOutput {
    crate::session::run_with_compiler_stack(|| {
        let build = || {
            CompilerSession::default()
                .build(CompileRequest {
                    source,
                    source_name: Some("frame_emission.ko"),
                })
                .expect("complete canonical compiler validation")
        };
        if scalar {
            with_scalar_frame_emission(build)
        } else {
            build()
        }
    })
    .expect("canonical compiler stack")
}
pub(super) fn assert_metadata(before: &CompileOutput, after: &CompileOutput) {
    super::super::single_use_private::assert_public_metadata(before, after);
    let geometry = |output: &CompileOutput| {
        output
            .report
            .budget_report
            .iter()
            .map(|function| {
                let callable = output
                    .contract_interface
                    .callables
                    .iter()
                    .find(|callable| callable.entry_pc == function.pc_start)
                    .unwrap();
                (
                    function.function_name.clone(),
                    (
                        function.frame_bytes,
                        callable.arguments.clone(),
                        callable.results.clone(),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(
        before.report.budget_report.len(),
        after.report.budget_report.len()
    );
    assert_eq!(
        before.contract_interface.callables.len(),
        after.contract_interface.callables.len()
    );
    assert_eq!(
        geometry(before),
        geometry(after),
        "every exact retained root, frame and full argument/result schema remains"
    );
    assert_eq!(
        before.report.access_hint_diagnostics,
        after.report.access_hint_diagnostics
    );
}
#[test]
fn saved_register_windows_keep_every_ordered_memory_access_and_amortize_literal_growth() {
    let registers = [2, 3, 4, 5, 6, 7, 8, 9, 23, 24];
    for restore in [false, true] {
        let mut scalar = Vec::new();
        let scalar_fixups = LiteralFixups::default();
        with_scalar_frame_emission(|| {
            emit_saved_registers(&mut scalar, &scalar_fixups, &registers, 256, restore)
        })
        .unwrap();
        let mut windows = Vec::new();
        let window_fixups = LiteralFixups::default();
        emit_saved_registers(&mut windows, &window_fixups, &registers, 256, restore).unwrap();
        let opcode = if restore {
            instruction::wide::memory::LOAD64
        } else {
            instruction::wide::memory::STORE64
        };
        let accesses = |code: &[u8]| {
            code.chunks_exact(4)
                .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
                .filter(|word| instruction::wide::opcode(*word) == opcode)
                .collect::<Vec<_>>()
        };
        assert_eq!(accesses(&scalar).len(), registers.len());
        assert_eq!(accesses(&windows).len(), registers.len());
        assert_eq!(scalar_fixups.borrow().len(), registers.len());
        assert_eq!(window_fixups.borrow().len(), 1);
        assert_eq!(scalar.len() - windows.len(), (registers.len() - 1) * 8);
        let window_anchor = 256 - i64::from(WIDE_IMM_MIN);
        let fixups = window_fixups.borrow();
        assert_eq!(fixups[0].0, 0);
        assert_eq!(fixups[0].1, LITERAL_SHIFT_REG);
        assert_eq!(
            fixups[0].2,
            DataKey(DataKind::I64, window_anchor.to_string())
        );
        let words = windows
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(
            words[1],
            encoding::wide::encode_rr(
                instruction::wide::arithmetic::ADD,
                27,
                regalloc::SP_REG as u8,
                LITERAL_SHIFT_REG
            )
        );
        let expected = registers
            .iter()
            .enumerate()
            .map(|(index, register)| {
                let relative =
                    i16::try_from(256 + index * 8).unwrap() - i16::try_from(window_anchor).unwrap();
                if restore {
                    encode_load64_rv(*register, 27, relative).unwrap()
                } else {
                    encode_store64_rv(27, *register, relative).unwrap()
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            accesses(&windows),
            expected,
            "every exact slot, source/destination register and memory-access order"
        );
    }
    for base in [0, 104, 112, 120] {
        let mut scalar = Vec::new();
        let a = LiteralFixups::default();
        with_scalar_frame_emission(|| {
            emit_saved_registers(&mut scalar, &a, &registers[..3], base, false)
        })
        .unwrap();
        let mut ordinary = Vec::new();
        let b = LiteralFixups::default();
        emit_saved_registers(&mut ordinary, &b, &registers[..3], base, false).unwrap();
        assert_eq!(scalar, ordinary);
        assert_eq!(*a.borrow(), *b.borrow());
    }
}
#[test]
fn saved_slot_overflow_and_reserved_scratch_aliases_refuse_before_emission() {
    let mut cases = vec![(&[27u8][..], 0), (&[2, 3][..], usize::MAX)];
    if let Ok(large_signed_base) = usize::try_from(i64::MAX - 64) {
        cases.push((&[2, 3, 4, 5][..], large_signed_base));
    }
    for (registers, base) in cases {
        let mut code = Vec::new();
        let fixups = LiteralFixups::default();
        assert!(emit_saved_registers(&mut code, &fixups, registers, base, false).is_err());
        assert!(code.is_empty());
        assert!(fixups.borrow().is_empty());
    }
}
#[test]
fn scalar_frame_comparison_scope_restores_after_nested_unwind() {
    assert!(!retain_scalar());
    with_scalar_frame_emission(|| {
        assert!(retain_scalar());
        assert!(
            std::panic::catch_unwind(|| with_scalar_frame_emission(|| panic!("scope unwind")))
                .is_err()
        );
        assert!(retain_scalar());
    });
    assert!(!retain_scalar());
}
#[test]
fn canonical_dlmm_frame_emission_measures_same_compiler_full_artifact_and_exact_abi() {
    let source = include_str!("../../../../iroha_core/src/validation_fee/fixtures/dlmm_pool.ko");
    let before = compile(source, true);
    let after = compile(source, false);
    assert_metadata(&before, &after);
    let old = ProgramMetadata::parse(&before.artifact).unwrap();
    let new = ProgramMetadata::parse(&after.artifact).unwrap();
    assert!(after.artifact.len() < before.artifact.len());
    eprintln!(
        "dlmm_frame_emission before_bytes={} after_bytes={} saved_bytes={} before_code={} after_code={} before_cntr={} after_cntr={} before_ltlb={} after_ltlb={} before_hash={} after_hash={}",
        before.artifact.len(),
        after.artifact.len(),
        before.artifact.len() - after.artifact.len(),
        before.artifact.len() - old.code_offset,
        after.artifact.len() - new.code_offset,
        before.contract_interface.encode_section().len(),
        after.contract_interface.encode_section().len(),
        old.code_offset - old.header_len - before.contract_interface.encode_section().len(),
        new.code_offset - new.header_len - after.contract_interface.encode_section().len(),
        before.report.artifact_hash,
        after.report.artifact_hash
    );
}

#[test]
fn shared_epilogue_labels_preserve_return_roots_and_refuse_label_overflow() {
    let mut function = ir::Function {
        name: "same_return_restore".to_owned(),
        params: Vec::new(),
        blocks: [3, 7]
            .map(|label| ir::BasicBlock {
                label: ir::Label(label),
                instrs: Vec::new(),
                terminator: Terminator::Return(None),
            })
            .into(),
        entry: ir::Label(3),
        location: crate::ast::SourceLocation { line: 1, column: 1 },
    };
    assert_eq!(shared_epilogue_label(&function, 2), Some(8));
    assert_eq!(shared_epilogue_label(&function, 1), None);
    assert_eq!(
        with_scalar_frame_emission(|| shared_epilogue_label(&function, 2)),
        None
    );
    function.blocks[1].label = ir::Label(usize::MAX);
    assert_eq!(shared_epilogue_label(&function, 2), None);
    function.blocks[1].terminator = Terminator::Jump(ir::Label(3));
    assert_eq!(shared_epilogue_label(&function, 2), None);
}
