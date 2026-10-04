//! Allocation and real source-register cycle controls for the exact emitter whitelist.

use super::super::{
    ALLOC_POOL, Allocation, BasicBlock, CALLER_CLOBBERED_REGS, Function, Instr, Interval, Label,
    Temp, Terminator, allocate, build_split_segments,
};
use super::{operands_staged_before_clobber, with_conservative_host_operands};
use crate::{
    ast::{BinaryOp, SourceLocation},
    ir::WideNumericKind,
};

fn numeric(dest: Temp, left: Temp, right: Temp) -> Instr {
    Instr::NumericBinary {
        dest,
        op: BinaryOp::Sub,
        left,
        right,
        left_kind: WideNumericKind::Int,
        right_kind: WideNumericKind::Int,
        result_kind: WideNumericKind::Int,
    }
}
fn fixture(instruction: Instr, inputs: &[Temp], keep_inputs: bool) -> Function {
    let mut instrs = inputs
        .iter()
        .map(|temp| Instr::LoadVar {
            dest: *temp,
            name: format!("arg{}", temp.0),
        })
        .collect::<Vec<_>>();
    if keep_inputs {
        instrs.push(Instr::TuplePack {
            dest: Temp(90),
            items: inputs.to_vec(),
        });
    }
    instrs.push(instruction);
    Function {
        name: "host_operands".into(),
        params: inputs.iter().map(|temp| format!("arg{}", temp.0)).collect(),
        blocks: vec![BasicBlock {
            label: Label(0),
            instrs,
            terminator: Terminator::Return(keep_inputs.then_some(Temp(90))),
        }],
        entry: Label(0),
        location: SourceLocation { line: 1, column: 1 },
    }
}
fn cases() -> Vec<(Instr, Vec<Temp>)> {
    vec![
        (numeric(Temp(2), Temp(0), Temp(1)), vec![Temp(0), Temp(1)]),
        (
            Instr::NumericCompare {
                dest: Temp(2),
                op: BinaryOp::Lt,
                left: Temp(0),
                right: Temp(1),
                kind: WideNumericKind::Int,
            },
            vec![Temp(0), Temp(1)],
        ),
        (
            Instr::StateSet {
                path: Temp(0),
                value: Temp(1),
            },
            vec![Temp(0), Temp(1)],
        ),
        (
            Instr::PathMapKeyNorito {
                dest: Temp(2),
                base: Temp(0),
                key_blob: Temp(1),
            },
            vec![Temp(0), Temp(1)],
        ),
        (
            Instr::PointerToNorito {
                dest: Temp(2),
                value: Temp(0),
            },
            vec![Temp(0)],
        ),
        (
            Instr::StateGet {
                dest: Temp(2),
                path: Temp(0),
            },
            vec![Temp(0)],
        ),
    ]
}

#[test]
fn exact_staged_consumers_reuse_only_dead_operand_registers() {
    for (instruction, inputs) in cases() {
        assert!(operands_staged_before_clobber(&instruction));
        let description = format!("{instruction:?}");
        let function = fixture(instruction, &inputs, false);
        let before = with_conservative_host_operands(|| allocate(&function));
        let after = allocate(&function);
        for temp in &inputs {
            assert!(
                ALLOC_POOL.contains(&before.regs[temp]),
                "{description}: {before:#?}"
            );
            assert!(
                CALLER_CLOBBERED_REGS.contains(&after.regs[temp]),
                "{description}: {after:#?}"
            );
        }
        assert!(after.stack.is_empty());
        assert_eq!(after, allocate(&function));
    }
}

#[test]
fn staged_consumers_preserve_tuple_members_live_across_the_clobber() {
    for (instruction, inputs) in cases() {
        let description = format!("{instruction:?}");
        let function = fixture(instruction, &inputs, true);
        let allocation = allocate(&function);
        for input in inputs {
            assert!(
                ALLOC_POOL.contains(&allocation.regs[&input]),
                "virtual tuple members must survive {description}: {allocation:#?}"
            );
        }
    }
}

#[test]
fn staged_numeric_split_segments_reload_after_the_original_clobber() {
    let value = Temp(0);
    let arithmetic = |dest| Instr::Binary {
        dest: Temp(dest),
        op: BinaryOp::Add,
        left: value,
        right: value,
    };
    let function = Function {
        name: "split_host_operand".into(),
        params: vec![],
        blocks: vec![BasicBlock {
            label: Label(0),
            instrs: vec![
                arithmetic(1),
                arithmetic(2),
                numeric(Temp(5), value, value),
                arithmetic(3),
                arithmetic(4),
            ],
            terminator: Terminator::Return(None),
        }],
        entry: Label(0),
        location: SourceLocation { line: 1, column: 1 },
    };
    let home = Allocation {
        regs: Default::default(),
        stack: [(value, 0)].into_iter().collect(),
        frame_size: 16,
    };
    let intervals = [Interval {
        temp: value,
        start: 0,
        end: 4,
    }];
    let mut segments = build_split_segments(&function, &intervals, &home);
    segments.sort_unstable_by_key(|segment| segment.start);
    assert_eq!(segments.len(), 2, "{segments:#?}");
    assert_eq!((segments[0].start, segments[0].end), (0, 2));
    assert_eq!((segments[1].start, segments[1].end), (3, 4));
    assert!(
        segments
            .iter()
            .all(|segment| CALLER_CLOBBERED_REGS.contains(&segment.register))
    );
    assert!(
        segments
            .iter()
            .all(|segment| !(segment.start <= 2 && 2 < segment.end))
    );
    assert_eq!(
        home.stack[&value], 0,
        "the actual spill home remains authoritative"
    );
}

#[test]
fn unaudited_host_emitters_keep_original_operand_exclusions() {
    for instruction in [
        Instr::ActorAccount {
            dest: Temp(2),
            actor: Temp(0),
        },
        Instr::NumericNeg {
            dest: Temp(2),
            value: Temp(0),
            kind: WideNumericKind::Int,
        },
        Instr::DirectHelperSyscall {
            dest: Temp(2),
            syscall: 0,
            args: vec![Temp(0)],
        },
    ] {
        assert!(!operands_staged_before_clobber(&instruction));
        let allocation = allocate(&fixture(instruction, &[Temp(0)], false));
        assert!(ALLOC_POOL.contains(&allocation.regs[&Temp(0)]));
    }
    // The existing private-call staging contract is unchanged in both modes.
    let instruction = Instr::Call {
        callee: "callee".into(),
        args: vec![Temp(0)],
        dest: None,
    };
    assert!(operands_staged_before_clobber(&instruction));
    assert!(with_conservative_host_operands(|| {
        operands_staged_before_clobber(&instruction)
    }));
}

#[test]
fn branch_local_numeric_operands_form_a_real_opposite_register_cycle() {
    // Every input is loaded before the first branch. All arithmetic branches
    // return, so none of the later branch's inputs is live across an executed
    // arithmetic syscall. This consumes all 13 original caller registers and
    // makes the last operation read its left/right values from r11/r10.
    let mut blocks = vec![BasicBlock {
        label: Label(0),
        instrs: (0..19)
            .map(|index| Instr::LoadVar {
                dest: Temp(index),
                name: format!("arg{index}"),
            })
            .collect(),
        terminator: Terminator::Branch {
            cond: Temp(13),
            then_bb: Label(1),
            else_bb: Label(2),
        },
    }];
    for branch in 0..6 {
        blocks.push(BasicBlock {
            label: Label(branch * 2 + 1),
            instrs: vec![numeric(
                Temp(20 + branch),
                Temp(branch * 2),
                Temp(branch * 2 + 1),
            )],
            terminator: Terminator::Return(Some(Temp(20 + branch))),
        });
        if branch < 5 {
            blocks.push(BasicBlock {
                label: Label(branch * 2 + 2),
                instrs: vec![],
                terminator: Terminator::Branch {
                    cond: Temp(14 + branch),
                    then_bb: Label(branch * 2 + 3),
                    else_bb: Label(branch * 2 + 4),
                },
            });
        }
    }
    blocks.push(BasicBlock {
        label: Label(12),
        instrs: vec![numeric(Temp(26), Temp(11), Temp(12))],
        terminator: Terminator::Return(Some(Temp(26))),
    });
    let function = Function {
        name: "choose".into(),
        params: (0..19).map(|index| format!("arg{index}")).collect(),
        blocks,
        entry: Label(0),
        location: SourceLocation { line: 1, column: 1 },
    };
    let allocation = allocate(&function);
    assert_eq!(allocation.regs[&Temp(11)], 11, "{allocation:#?}");
    assert_eq!(allocation.regs[&Temp(12)], 10, "{allocation:#?}");
    assert!(allocation.stack.is_empty(), "{allocation:#?}");
}

#[test]
fn conservative_operand_measurement_scope_restores_after_nested_unwind() {
    let instruction = numeric(Temp(2), Temp(0), Temp(1));
    assert!(operands_staged_before_clobber(&instruction));
    with_conservative_host_operands(|| {
        let panic = std::panic::catch_unwind(|| {
            with_conservative_host_operands(|| panic!("intentional nested baseline unwind"))
        });
        assert!(panic.is_err());
        assert!(!operands_staged_before_clobber(&instruction));
    });
    assert!(operands_staged_before_clobber(&instruction));
}
