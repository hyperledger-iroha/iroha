//! Exact source lifetimes, split-reload refusals and full unchanged artifact metadata.
use super::native_pairs::cases;
use super::*;
use crate::session::{CompileOutput, CompileRequest, CompilerSession};

pub(super) fn compile(source: &str, scalar: bool) -> CompileOutput {
    crate::session::run_with_compiler_stack(|| {
        let build = || {
            CompilerSession::default()
                .build(CompileRequest {
                    source,
                    source_name: Some("local_emission.ko"),
                })
                .expect("complete canonical source, SSA, literal and artifact validation")
        };
        if scalar {
            with_scalar_emission(build)
        } else {
            build()
        }
    })
    .expect("original canonical compiler worker")
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
        "all original frame and full argument/result schemas remain exact"
    );
    assert_eq!(
        before.report.access_hint_diagnostics,
        after.report.access_hint_diagnostics
    );
}

fn function(instructions: Vec<Instr>, terminator: Terminator) -> ir::Function {
    ir::Function {
        name: "local".to_owned(),
        params: vec!["left".to_owned(), "right".to_owned()],
        blocks: vec![ir::BasicBlock {
            label: ir::Label(0),
            instrs: instructions,
            terminator,
        }],
        entry: ir::Label(0),
        location: crate::ast::SourceLocation { line: 1, column: 1 },
    }
}
fn plan(function: &ir::Function) -> Plan {
    Plan::new(function, &regalloc::allocate_with_splitting(function)).unwrap()
}
fn add(dest: usize, left: usize, right: usize) -> Instr {
    Instr::NumericBinary {
        dest: ir::Temp(dest),
        op: BinaryOp::Add,
        left: ir::Temp(left),
        right: ir::Temp(right),
        left_kind: ir::WideNumericKind::Int,
        right_kind: ir::WideNumericKind::Int,
        result_kind: ir::WideNumericKind::Int,
    }
}
fn chain(marker: bool) -> ir::Function {
    let mut instructions = vec![
        Instr::DataRef {
            dest: ir::Temp(0),
            kind: ir::DataRefKind::Int,
            value: "3".to_owned(),
        },
        Instr::DataRef {
            dest: ir::Temp(1),
            kind: ir::DataRefKind::Int,
            value: "4".to_owned(),
        },
        add(2, 0, 1),
    ];
    if marker {
        instructions.push(Instr::DataRef {
            dest: ir::Temp(4),
            kind: ir::DataRefKind::Int,
            value: "5".to_owned(),
        });
    }
    instructions.push(Instr::NumericNeg {
        dest: ir::Temp(3),
        value: ir::Temp(2),
        kind: ir::WideNumericKind::Int,
    });
    function(instructions, Terminator::Return(Some(ir::Temp(3))))
}
#[test]
fn unique_owned_result_is_available_only_inside_its_exact_original_local_lifetime() {
    for marker in [false, true] {
        let function = chain(marker);
        let plan = plan(&function);
        let consumer = if marker { 4 } else { 3 };
        let result = plan.output(ir::Temp(2), 2).unwrap();
        assert_eq!(result.first_use_position, 3);
        assert_eq!(result.consumer_position, consumer);
        assert_eq!(result.register(ir::Temp(2), 2), None);
        assert_eq!(result.register(ir::Temp(2), 3), Some(10));
        assert_eq!(result.register(ir::Temp(2), consumer), Some(10));
        assert_eq!(result.register(ir::Temp(2), consumer + 1), None);
        assert_eq!(result.register(ir::Temp(3), consumer), None);
        assert_eq!(
            plan.output(ir::Temp(3), consumer)
                .unwrap()
                .consumer_position,
            consumer + 1
        );
        assert!(plan.output(ir::Temp(2), consumer).is_none());
        assert!(plan.output(ir::Temp(3), 2).is_none());
    }
}
#[test]
fn original_sources_shared_phi_like_definitions_effects_and_fused_raw_comparisons_keep_materialization()
 {
    let mut shared = chain(false);
    shared.blocks[0].terminator = Terminator::Return2(ir::Temp(2), ir::Temp(3));
    assert!(plan(&shared).output(ir::Temp(2), 2).is_none());
    let mut phi = chain(false);
    phi.blocks[0].instrs.insert(3, add(2, 0, 1));
    assert!(plan(&phi).output(ir::Temp(2), 2).is_none());
    assert!(plan(&phi).output(ir::Temp(2), 3).is_none());
    let mut effect = chain(false);
    effect.blocks[0]
        .instrs
        .insert(3, Instr::Info { msg: ir::Temp(0) });
    assert!(plan(&effect).output(ir::Temp(2), 2).is_none());
    assert!(!safe_consumer(&Instr::Binary {
        dest: ir::Temp(3),
        op: BinaryOp::Gt,
        left: ir::Temp(2),
        right: ir::Temp(1)
    }));
    assert!(
        syscall_result(&Instr::PrivateNumericValcom {
            dest: ir::Temp(3),
            value: ir::Temp(2),
            blind: ir::Temp(1)
        })
        .is_none()
    );
    assert!(syscall_result(&Instr::NumericStatus { dest: ir::Temp(3) }).is_none());
}
#[test]
fn split_reload_refuses_overwriting_original_return_register_or_reloading_stale_source_home() {
    use regalloc::SplitReload;
    assert!(!reload_clobbers(ir::Temp(2), &[]));
    assert!(!reload_clobbers(
        ir::Temp(2),
        &[SplitReload {
            temp: ir::Temp(9),
            register: 8
        }]
    ));
    assert!(reload_clobbers(
        ir::Temp(2),
        &[SplitReload {
            temp: ir::Temp(9),
            register: 10
        }]
    ));
    assert!(reload_clobbers(
        ir::Temp(2),
        &[SplitReload {
            temp: ir::Temp(2),
            register: 8
        }]
    ));
    let mut instructions = vec![
        Instr::DataRef {
            dest: ir::Temp(0),
            kind: ir::DataRefKind::Int,
            value: "3".to_owned(),
        },
        Instr::DataRef {
            dest: ir::Temp(1),
            kind: ir::DataRefKind::Int,
            value: "4".to_owned(),
        },
    ];
    for dest in 2..32 {
        instructions.push(add(dest, 0, 1));
    }
    instructions.push(Instr::DataRef {
        dest: ir::Temp(32),
        kind: ir::DataRefKind::NoritoBytes,
        value: "schema".to_owned(),
    });
    instructions.push(Instr::StateValueEncode {
        dest: ir::Temp(33),
        schema: ir::Temp(32),
        words: (2..32).map(ir::Temp).collect(),
    });
    let function = function(instructions, Terminator::Return(Some(ir::Temp(33))));
    let allocation = regalloc::allocate_with_splitting(&function);
    let plan = Plan::new(&function, &allocation).unwrap();
    let spilled = allocation
        .stack
        .keys()
        .filter(|value| (2..32).contains(&value.0))
        .collect::<Vec<_>>();
    assert!(
        !spilled.is_empty(),
        "actual original register-pressure spill homes"
    );
    for value in spilled {
        assert!(
            plan.output(*value, value.0).is_none(),
            "no canonical spill home may remain stale"
        );
    }
}
#[test]
fn consecutive_entry_parameters_keep_all_original_reads_and_late_reads_keep_saved_base() {
    let load = |dest, name: &str| Instr::LoadVar {
        dest: ir::Temp(dest),
        name: name.to_owned(),
    };
    for count in 0..=2 {
        let instructions = ["left", "right"]
            .iter()
            .take(count)
            .enumerate()
            .map(|(i, name)| load(i, name))
            .collect();
        let function = function(instructions, Terminator::Return(None));
        let optimized = plan(&function);
        assert_eq!(optimized.parameter_prefix_words, count);
        assert!(!optimized.stores_argument_base);
        let scalar = with_scalar_emission(|| plan(&function));
        assert_eq!(scalar.parameter_prefix_words, 0);
        assert!(scalar.stores_argument_base);
    }
    for prefix in 1..=2 {
        let mut instructions = vec![load(0, "left")];
        if prefix == 2 {
            instructions.push(load(1, "right"));
        }
        instructions.push(Instr::Const {
            dest: ir::Temp(2),
            value: 0,
        });
        instructions.push(load(3, "left"));
        let function = function(instructions, Terminator::Return(Some(ir::Temp(3))));
        let optimized = plan(&function);
        assert_eq!(
            optimized.parameter_prefix_words,
            if prefix == 2 { 2 } else { 0 }
        );
        assert!(
            optimized.stores_argument_base,
            "late original LoadVar still uses its exact saved argument base"
        );
    }
    let mut invalid = chain(false);
    invalid.entry = ir::Label(99);
    assert_eq!(
        Plan::new(&invalid, &regalloc::allocate_with_splitting(&invalid))
            .err()
            .unwrap(),
        "missing local-emission entry block"
    );
}
#[test]
fn self_copy_and_scalar_scope_restore_keep_exact_architectural_values_and_test_baseline() {
    let mut same = Vec::new();
    emit_move(&mut same, 10, 10).unwrap();
    assert!(same.is_empty());
    with_scalar_emission(|| emit_move(&mut same, 10, 10)).unwrap();
    assert_eq!(same, encode_addi(10, 10, 0).unwrap().to_le_bytes());
    same.clear();
    emit_move(&mut same, 11, 10).unwrap();
    assert_eq!(same, encode_addi(11, 10, 0).unwrap().to_le_bytes());
    assert!(
        std::panic::catch_unwind(|| with_scalar_emission(|| panic!("restore scalar-only scope")))
            .is_err()
    );
    assert!(!retain_scalar());
}
fn syscalls_in(output: &CompileOutput) -> BTreeMap<u32, usize> {
    let offset = ProgramMetadata::parse(&output.artifact)
        .unwrap()
        .code_offset;
    let mut calls = BTreeMap::new();
    for bytes in output.artifact[offset..].chunks_exact(4) {
        let word = u32::from_le_bytes(bytes.try_into().unwrap());
        if matches!(
            instruction::wide::opcode(word),
            instruction::wide::system::SCALL | instruction::wide::system::SYSTEM
        ) {
            *calls.entry(word).or_default() += 1;
        }
    }
    calls
}
#[test]
fn canonical_sources_preserve_every_original_typed_metered_syscall_and_complete_frame_schema() {
    for case in cases::CASES {
        let before = compile(case.source, true);
        let after = compile(case.source, false);
        assert_metadata(&before, &after);
        assert_eq!(
            syscalls_in(&before),
            syscalls_in(&after),
            "every original canonical consumer, publication and metered allocation for {}",
            case.id
        );
        assert!(after.artifact.len() < before.artifact.len());
        assert_eq!(
            compile(case.source, false).artifact,
            after.artifact,
            "complete deterministic artifact for {}",
            case.id
        );
    }
}
#[test]
fn canonical_dlmm_local_emission_measures_full_artifact_and_exact_unchanged_owned_consumers() {
    let source = include_str!("../../../../iroha_core/src/validation_fee/fixtures/dlmm_pool.ko");
    let before = compile(source, true);
    let after = compile(source, false);
    assert_metadata(&before, &after);
    assert_eq!(
        syscalls_in(&before),
        syscalls_in(&after),
        "all original synchronous consumers and allocations remain present"
    );
    assert!(after.artifact.len() < before.artifact.len());
    let old = ProgramMetadata::parse(&before.artifact).unwrap();
    let new = ProgramMetadata::parse(&after.artifact).unwrap();
    eprintln!(
        "dlmm_local_emission before_bytes={} after_bytes={} saved_bytes={} before_code={} after_code={} before_cntr={} after_cntr={} before_literals={} after_literals={} before_hash={} after_hash={}",
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
    // TODO: native Core payout and network qualification under unchanged default4M and64gas/byte remain required.
}
fn compile_with_injected(instructions: Vec<Instr>, scalar: bool) -> Result<Vec<u8>, String> {
    crate::session::run_with_compiler_stack(|| {
        let build = || {
            let options = CompilerOptions::default();
            let session = CompilerSession::new(options.clone());
            let source_name = "local_invalid_literal.ko";
            let parsed = session
                .parse_compilation_unit(CompileRequest {
                    source: include_str!("../fixtures/v1/c001.ko"),
                    source_name: Some(source_name),
                })
                .map_err(|diagnostics| diagnostics.render_human())?;
            let resolved = session
                .resolve_compilation_unit(parsed)
                .map_err(|diagnostics| diagnostics.render_human())?;
            let typed = session
                .type_effect_compilation_unit(resolved)
                .map_err(|diagnostics| diagnostics.render_human())?;
            let compiler = Compiler::new_with_options(options);
            let lowered = compiler
                .lower_typed_program(typed, Some(source_name))
                .map_err(|diagnostics| diagnostics.render_human())?;
            let ssa = compiler
                .construct_ssa_program(lowered)
                .map_err(|diagnostics| diagnostics.render_human())?;
            let optimized = compiler
                .optimize_ssa_program(ssa)
                .map_err(|diagnostics| diagnostics.render_human())?;
            let mut codegen = compiler
                .destroy_ssa_program(optimized)
                .map_err(|diagnostics| diagnostics.render_human())?;
            let function = codegen
                .ir_program
                .functions
                .iter_mut()
                .find(|function| function.name == "run")
                .unwrap();
            function.entry = ir::Label(0);
            function.blocks = vec![ir::BasicBlock {
                label: ir::Label(0),
                instrs: instructions,
                terminator: Terminator::Return(None),
            }];
            compiler
                .compile_codegen(codegen)
                .map(|artifact| artifact.bytes)
        };
        if scalar {
            with_scalar_emission(build)
        } else {
            build()
        }
    })
    .unwrap()
}
#[test]
fn local_numeric_operands_keep_canonical_literal_rejections_for_every_original_typed_position() {
    use ir::{DataRefKind, NumericRoundOp, Temp, WideNumericKind};
    for fused in [false, true] {
        for quantity in [false, true] {
            for invalid in 0..if fused { 4 } else { 3 } {
                let make = || {
                    let kinds = [
                        if quantity {
                            DataRefKind::Quantity
                        } else {
                            DataRefKind::Decimal
                        },
                        DataRefKind::Decimal,
                        DataRefKind::Decimal,
                        DataRefKind::Int,
                    ];
                    let positions = if fused {
                        vec![0, 1, 2, 3]
                    } else {
                        vec![0, 2, 3]
                    };
                    let mut instructions = kinds
                        .iter()
                        .enumerate()
                        .map(|(index, kind)| Instr::DataRef {
                            dest: Temp(770 + index),
                            kind: *kind,
                            value: if index == positions[invalid] {
                                "bad-original-literal"
                            } else {
                                "1"
                            }
                            .to_owned(),
                        })
                        .collect::<Vec<_>>();
                    instructions.push(Instr::Const {
                        dest: Temp(774),
                        value: 0,
                    });
                    instructions.push(Instr::NumericRound {
                        dest: Temp(775),
                        dividend: Temp(770),
                        multiplier: fused.then_some(Temp(771)),
                        divisor: Temp(772),
                        scale: Temp(773),
                        mode: Temp(774),
                        op: match (quantity, fused) {
                            (true, true) => NumericRoundOp::QuantityMulDiv,
                            (false, true) => NumericRoundOp::DecimalMulDiv,
                            (true, false) => NumericRoundOp::QuantityDiv,
                            (false, false) => NumericRoundOp::DecimalDiv,
                        },
                        result_kind: if quantity {
                            WideNumericKind::Quantity
                        } else {
                            WideNumericKind::Decimal
                        },
                    });
                    instructions
                };
                let before = compile_with_injected(make(), true)
                    .expect_err("original canonical literal rejection");
                let after = compile_with_injected(make(), false)
                    .expect_err("same original validator remains");
                assert_eq!(
                    before, after,
                    "complete typed rejection at original operand {invalid}"
                );
            }
        }
    }
}

#[test]
fn original_single_word_state_decode_keeps_schema_data_allocation_and_ordered_word_load() {
    let function = function(
        vec![
            Instr::DataRef {
                dest: ir::Temp(0),
                kind: ir::DataRefKind::NoritoBytes,
                value: "path".to_owned(),
            },
            Instr::StateGet {
                dest: ir::Temp(1),
                path: ir::Temp(0),
            },
            Instr::DataRef {
                dest: ir::Temp(2),
                kind: ir::DataRefKind::NoritoBytes,
                value: "schema".to_owned(),
            },
            Instr::DirectHelperSyscall {
                dest: ir::Temp(3),
                syscall: syscalls::SYSCALL_STATE_VALUE_DECODE,
                args: vec![ir::Temp(2), ir::Temp(1)],
            },
            Instr::Load64Imm {
                dest: ir::Temp(4),
                base: ir::Temp(3),
                imm: ivm_abi::state_value::DECODED_STATE_VALUE_TABLE_OFFSET,
            },
        ],
        Terminator::Return(Some(ir::Temp(4))),
    );
    let optimized = plan(&function);
    assert_eq!(
        optimized.output(ir::Temp(1), 1).unwrap().consumer_position,
        3
    );
    assert_eq!(
        optimized.output(ir::Temp(3), 3).unwrap().consumer_position,
        4
    );
    assert!(parallel_state_decode(
        syscalls::SYSCALL_STATE_VALUE_DECODE,
        2
    ));
    assert!(!parallel_state_decode(
        syscalls::SYSCALL_STATE_VALUE_DECODE,
        1
    ));
    assert!(!parallel_state_decode(
        syscalls::SYSCALL_STATE_VALUE_DECODE,
        3
    ));
    assert!(!parallel_state_decode(syscalls::SYSCALL_INT_MEAN, 2));
    with_scalar_emission(|| {
        assert!(!parallel_state_decode(
            syscalls::SYSCALL_STATE_VALUE_DECODE,
            2
        ));
        assert!(plan(&function).outputs.is_empty());
    });
    let mut wider = function;
    wider.blocks[0].terminator = Terminator::Return2(ir::Temp(3), ir::Temp(4));
    assert!(
        plan(&wider).output(ir::Temp(3), 3).is_none(),
        "shared original tables remain materialized"
    );
}
