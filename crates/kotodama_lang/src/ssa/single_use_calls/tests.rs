//! CFG, effect, exclusion and resource controls for exact-body private inlining.

use super::*;
use crate::{
    ast::SourceLocation,
    ir::{self, BasicBlock as IrBlock, DataRefKind, Function as IrFunction},
};

fn function(name: &str, parameters: &[&str], blocks: Vec<IrBlock>) -> IrFunction {
    IrFunction {
        name: name.to_owned(),
        params: parameters.iter().map(|value| (*value).to_owned()).collect(),
        blocks,
        entry: Label(0),
        location: SourceLocation { line: 1, column: 1 },
    }
}
fn block(label: usize, instrs: Vec<Instr>, terminator: Terminator) -> IrBlock {
    IrBlock {
        label: Label(label),
        instrs,
        terminator,
    }
}
fn root_call(name: &str, times: usize) -> IrFunction {
    function(
        "root",
        &[],
        vec![block(
            0,
            (0..times)
                .map(|index| Instr::Call {
                    callee: name.to_owned(),
                    args: Vec::new(),
                    dest: Some(Temp(index)),
                })
                .collect(),
            Terminator::Return(Some(Temp(times - 1))),
        )],
    )
}
fn literal(name: &str, value: &str) -> IrFunction {
    function(
        name,
        &[],
        vec![block(
            0,
            vec![Instr::DataRef {
                dest: Temp(0),
                kind: DataRefKind::Quantity,
                value: value.to_owned(),
            }],
            Terminator::Return(Some(Temp(0))),
        )],
    )
}
fn run(functions: Vec<IrFunction>, roots: &[&str], candidates: &[(&str, bool)]) -> Program {
    let mut program = Program::from_ir(ir::Program { functions }).unwrap();
    program
        .inline_single_use_private_calls(
            &roots.iter().map(|name| (*name).to_owned()).collect(),
            &candidates
                .iter()
                .map(|(name, unit)| ((*name).to_owned(), *unit))
                .collect(),
        )
        .unwrap();
    program.verify().unwrap();
    program
}

#[test]
fn scalar_returns_merge_verified_edges_and_arguments_are_evaluated_once() {
    let helper = function(
        "helper",
        &["condition"],
        vec![
            block(
                0,
                vec![Instr::LoadVar {
                    dest: Temp(0),
                    name: "condition".to_owned(),
                }],
                Terminator::Branch {
                    cond: Temp(0),
                    then_bb: Label(1),
                    else_bb: Label(2),
                },
            ),
            block(
                1,
                vec![Instr::Const {
                    dest: Temp(1),
                    value: 7,
                }],
                Terminator::Return(Some(Temp(1))),
            ),
            block(
                2,
                vec![Instr::Const {
                    dest: Temp(2),
                    value: 11,
                }],
                Terminator::Return(Some(Temp(2))),
            ),
        ],
    );
    let root = function(
        "root",
        &[],
        vec![block(
            0,
            vec![
                Instr::GetAuthority { dest: Temp(0) },
                Instr::Call {
                    callee: "helper".to_owned(),
                    args: vec![Temp(0)],
                    dest: Some(Temp(1)),
                },
                Instr::GetAuthority { dest: Temp(2) },
            ],
            Terminator::Return(Some(Temp(1))),
        )],
    );
    let program = run(vec![helper, root], &["root"], &[("helper", false)]);
    assert_eq!(program.functions.len(), 1);
    let root = &program.functions[0];
    assert_eq!(
        root.blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .filter(|i| matches!(i.as_ir(), Instr::GetAuthority { .. }))
            .count(),
        2
    );
    assert!(matches!(
        root.blocks[0].instructions[0].as_ir(),
        Instr::GetAuthority { .. }
    ));
    let continuation = root
        .blocks
        .iter()
        .find(|b| matches!(b.terminator.as_ir(), Terminator::Return(_)))
        .unwrap();
    assert!(matches!(
        continuation.instructions[0].as_ir(),
        Instr::GetAuthority { .. }
    ));
    assert_eq!(continuation.phis.len(), 1);
    assert_eq!(continuation.phis[0].inputs.len(), 2);
    assert!(
        !root
            .blocks
            .iter()
            .flat_map(|b| &b.instructions)
            .any(|i| matches!(i.as_ir(), Instr::Call { .. } | Instr::LoadVar { .. }))
    );
    program.into_ir().unwrap();
}

#[test]
fn nested_loops_retarget_the_callers_back_edge_phi() {
    let helper = function(
        "helper",
        &["limit"],
        vec![
            block(
                0,
                vec![
                    Instr::LoadVar {
                        dest: Temp(0),
                        name: "limit".to_owned(),
                    },
                    Instr::Const {
                        dest: Temp(1),
                        value: 0,
                    },
                ],
                Terminator::Jump(Label(1)),
            ),
            block(
                1,
                Vec::new(),
                Terminator::Branch {
                    cond: Temp(0),
                    then_bb: Label(2),
                    else_bb: Label(3),
                },
            ),
            block(
                2,
                vec![Instr::GetAuthority { dest: Temp(1) }],
                Terminator::Jump(Label(1)),
            ),
            block(3, Vec::new(), Terminator::Return(Some(Temp(1)))),
        ],
    );
    let root = function(
        "root",
        &["condition"],
        vec![
            block(
                0,
                vec![
                    Instr::LoadVar {
                        dest: Temp(0),
                        name: "condition".to_owned(),
                    },
                    Instr::Const {
                        dest: Temp(1),
                        value: 0,
                    },
                ],
                Terminator::Jump(Label(1)),
            ),
            block(
                1,
                vec![
                    Instr::GetAuthority { dest: Temp(2) },
                    Instr::Call {
                        callee: "helper".to_owned(),
                        args: vec![Temp(1)],
                        dest: Some(Temp(1)),
                    },
                ],
                Terminator::Branch {
                    cond: Temp(0),
                    then_bb: Label(1),
                    else_bb: Label(2),
                },
            ),
            block(2, Vec::new(), Terminator::Return(Some(Temp(1)))),
        ],
    );
    let program = run(vec![helper, root], &["root"], &[("helper", false)]);
    assert_eq!(program.functions.len(), 1);
    let root = &program.functions[0];
    let loop_header = root.blocks.iter().find(|b| b.label == Label(1)).unwrap();
    assert!(
        !loop_header.phis.is_empty(),
        "the carried call argument needs the original caller-loop Phi"
    );
    assert!(
        loop_header
            .phis
            .iter()
            .all(|phi| phi.inputs.iter().all(|input| input.predecessor != Label(1)))
    );
    assert!(
        root.blocks.iter().any(|b| !b.phis.is_empty()),
        "callee loop values remain actual SSA merges"
    );
    program.into_ir().unwrap();
}

#[test]
fn discarded_results_preserve_literals_effects_traps_and_unit_return() {
    let helper = function(
        "helper",
        &[],
        vec![block(
            0,
            vec![
                // Deliberately malformed payload: even a discarded call must still
                // reach the downstream canonical literal validator after this pass.
                Instr::DataRef {
                    dest: Temp(0),
                    kind: DataRefKind::Quantity,
                    value: "-1".to_owned(),
                },
                Instr::GetAuthority { dest: Temp(1) },
                Instr::AbortIf {
                    cond: Temp(1),
                    descriptor: Temp(1),
                    code: Temp(1),
                },
            ],
            Terminator::Return(None),
        )],
    );
    let root = function(
        "root",
        &[],
        vec![block(
            0,
            vec![Instr::Call {
                callee: "helper".to_owned(),
                args: Vec::new(),
                dest: None,
            }],
            Terminator::Return(None),
        )],
    );
    let program = run(vec![helper, root], &["root"], &[("helper", true)])
        .into_ir()
        .unwrap();
    let instructions = program.functions[0]
        .blocks
        .iter()
        .flat_map(|b| &b.instrs)
        .collect::<Vec<_>>();
    assert_eq!(
        instructions
            .iter()
            .filter(|i| matches!(i, Instr::DataRef { value, .. } if value == "-1"))
            .count(),
        1
    );
    assert_eq!(
        instructions
            .iter()
            .filter(|i| matches!(i, Instr::GetAuthority { .. }))
            .count(),
        1
    );
    assert_eq!(
        instructions
            .iter()
            .filter(|i| matches!(i, Instr::AbortIf { .. }))
            .count(),
        1
    );

    let helper = function(
        "helper",
        &[],
        vec![block(
            0,
            vec![Instr::Const {
                dest: Temp(0),
                value: 0,
            }],
            Terminator::Return(Some(Temp(0))),
        )],
    );
    let program = run(
        vec![helper, root_call("helper", 1)],
        &["root"],
        &[("helper", true)],
    )
    .into_ir()
    .unwrap();
    assert_eq!(program.functions.len(), 1);
    assert!(
        program.functions[0]
            .blocks
            .iter()
            .flat_map(|b| &b.instrs)
            .any(|i| matches!(i, Instr::Const { value: 0, .. }))
    );
}

#[test]
fn repeated_calls_roots_unlisted_types_and_recursive_cycles_keep_original_bodies() {
    for (times, roots, candidates) in [
        (2, vec!["root"], vec![("helper", false)]),
        (1, vec!["root", "helper"], vec![("helper", false)]),
        (1, vec!["root"], Vec::new()),
    ] {
        let program = run(
            vec![literal("helper", "1"), root_call("helper", times)],
            &roots,
            &candidates,
        );
        assert_eq!(program.functions.len(), 2);
        assert_eq!(
            program
                .functions
                .iter()
                .flat_map(|f| &f.blocks)
                .flat_map(|b| &b.instructions)
                .filter(|i| matches!(i.as_ir(), Instr::Call { .. }))
                .count(),
            times
        );
    }
    let mut helper = root_call("root", 1);
    helper.name = "helper".to_owned();
    let program = run(
        vec![helper, root_call("helper", 1)],
        &["root"],
        &[("helper", false)],
    );
    assert_eq!(
        program.functions.len(),
        2,
        "recursive call graph is not rewritten or hidden"
    );
}

#[test]
fn unresolved_symbols_are_rejected_before_any_private_body_is_discarded() {
    let mut program = Program::from_ir(ir::Program {
        functions: vec![literal("unused", "1"), root_call("unknown", 1)],
    })
    .unwrap();
    let error = program
        .inline_single_use_private_calls(
            &BTreeSet::from(["root".to_owned()]),
            &BTreeMap::from([("unused".to_owned(), false)]),
        )
        .unwrap_err();
    assert!(error.contains("unknown"));
}

#[test]
fn original_ssa_resource_limits_skip_inlining_without_changing_the_call() {
    let mut helper_blocks = vec![block(0, Vec::new(), Terminator::Jump(Label(1)))];
    for index in 1..MAX_SSA_BLOCKS_PER_FUNCTION {
        helper_blocks.push(block(
            index,
            vec![Instr::GetAuthority { dest: Temp(index) }],
            if index + 1 == MAX_SSA_BLOCKS_PER_FUNCTION {
                Terminator::Return(Some(Temp(index)))
            } else {
                Terminator::Jump(Label(index + 1))
            },
        ));
    }
    let program = run(
        vec![
            function("helper", &[], helper_blocks),
            root_call("helper", 1),
        ],
        &["root"],
        &[("helper", false)],
    );
    assert_eq!(program.functions.len(), 2);
    assert!(matches!(
        program
            .functions
            .iter()
            .find(|f| f.name == "root")
            .unwrap()
            .blocks[0]
            .instructions[0]
            .as_ir(),
        Instr::Call { .. }
    ));
}

#[test]
fn checked_value_and_label_namespaces_skip_without_rewriting_the_original_call() {
    for labels in [false, true] {
        let mut program = Program::from_ir(ir::Program {
            functions: vec![literal("helper", "1"), root_call("helper", 1)],
        })
        .unwrap();
        let root = program
            .functions
            .iter_mut()
            .find(|function| function.name == "root")
            .unwrap();
        if labels {
            root.entry = Label(usize::MAX - 1);
            root.blocks[0].label = root.entry;
        } else {
            let Instr::Call { dest, .. } = root.blocks[0].instructions[0].as_ir_mut() else {
                unreachable!();
            };
            *dest = Some(Temp(usize::MAX));
            root.blocks[0].terminator =
                ValueTerminator::new(Terminator::Return(Some(Temp(usize::MAX))));
        }
        program.verify().unwrap();
        program
            .inline_single_use_private_calls(
                &BTreeSet::from(["root".to_owned()]),
                &BTreeMap::from([("helper".to_owned(), false)]),
            )
            .unwrap();
        assert_eq!(program.functions.len(), 2);
        assert!(matches!(
            program
                .functions
                .iter()
                .find(|function| function.name == "root")
                .unwrap()
                .blocks[0]
                .instructions[0]
                .as_ir(),
            Instr::Call { .. }
        ));
    }
}
