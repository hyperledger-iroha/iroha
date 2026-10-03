//! Positive substitution and exclusion controls for private numeric literals.

use super::MAX_FOLDED_LITERAL_TEXT_BYTES;
use crate::{
    ast::{BinaryOp, SourceLocation},
    ir::{self, BasicBlock, DataRefKind, Function, Instr, Label, Temp, Terminator},
    ssa::Program,
};
use std::collections::{BTreeMap, BTreeSet};

fn function(name: &str, instrs: Vec<Instr>, terminator: Terminator) -> Function {
    Function {
        name: name.to_owned(),
        params: Vec::new(),
        blocks: vec![BasicBlock {
            label: Label(0),
            instrs,
            terminator,
        }],
        entry: Label(0),
        location: SourceLocation { line: 1, column: 1 },
    }
}

fn literal(name: &str, kind: DataRefKind, value: &str) -> Function {
    function(
        name,
        vec![Instr::DataRef {
            dest: Temp(0),
            kind,
            value: value.to_owned(),
        }],
        Terminator::Return(Some(Temp(0))),
    )
}

fn call(callee: &str, dest: Option<Temp>) -> Instr {
    Instr::Call {
        callee: callee.to_owned(),
        args: Vec::new(),
        dest,
    }
}

fn optimized(functions: Vec<Function>, roots: &[&str]) -> ir::Program {
    // The component fixture explicitly supplies the typed whitelist; production
    // derives it from the validated declaration rather than from this body.
    let candidates = functions
        .iter()
        .filter_map(|function| {
            function.blocks[0]
                .instrs
                .iter()
                .find_map(|instruction| match instruction {
                    Instr::DataRef { kind, .. } => Some((function.name.clone(), *kind)),
                    _ => None,
                })
        })
        .collect();
    let mut program = Program::from_ir(ir::Program { functions }).expect("verified SSA fixture");
    program
        .optimize_and_retain(
            &roots.iter().map(|root| (*root).to_owned()).collect(),
            &candidates,
        )
        .expect("optimize verified SSA fixture");
    program.into_ir().expect("destroy verified SSA fixture")
}

#[test]
fn private_literal_calls_keep_exact_numeric_kind_and_return_value() {
    for (kind, value) in [
        (DataRefKind::Int, "-19"),
        (DataRefKind::Decimal, "12.5"),
        (DataRefKind::Quantity, "0"),
    ] {
        let program = optimized(
            vec![
                literal("helper", kind, value),
                function(
                    "root",
                    vec![call("helper", Some(Temp(7)))],
                    Terminator::Return(Some(Temp(7))),
                ),
            ],
            &["root"],
        );
        assert_eq!(program.functions.len(), 1);
        let root = &program.functions[0];
        assert_eq!(root.name, "root");
        let [instruction] = root.blocks[0].instrs.as_slice() else {
            panic!("one exact literal expected: {root:?}")
        };
        let Instr::DataRef {
            dest,
            kind: actual_kind,
            value: actual_value,
        } = instruction
        else {
            panic!("typed literal expected: {instruction:?}")
        };
        assert_eq!((*actual_kind, actual_value.as_str()), (kind, value));
        assert_eq!(root.blocks[0].terminator, Terminator::Return(Some(*dest)));
    }
}

#[test]
fn private_literal_folding_keeps_public_discarded_and_argument_calls() {
    let program = optimized(
        vec![
            literal("public_value", DataRefKind::Quantity, "0"),
            function(
                "root",
                vec![call("public_value", Some(Temp(0)))],
                Terminator::Return(Some(Temp(0))),
            ),
        ],
        &["root", "public_value"],
    );
    assert_eq!(program.functions.len(), 2);
    assert!(
        program
            .functions
            .iter()
            .find(|f| f.name == "root")
            .unwrap()
            .blocks[0]
            .instrs
            .iter()
            .any(|i| matches!(i, Instr::Call { callee, .. } if callee == "public_value"))
    );

    let program = optimized(
        vec![
            literal("helper", DataRefKind::Quantity, "0"),
            function("root", vec![call("helper", None)], Terminator::Return(None)),
        ],
        &["root"],
    );
    assert_eq!(
        program.functions.len(),
        2,
        "a discarded typed call stays a real call"
    );

    let mut helper = literal("helper", DataRefKind::Quantity, "0");
    helper.params.push("unused".to_owned());
    let program = optimized(
        vec![
            helper,
            function(
                "root",
                vec![
                    Instr::Const {
                        dest: Temp(1),
                        value: 7,
                    },
                    Instr::Call {
                        callee: "helper".to_owned(),
                        args: vec![Temp(1)],
                        dest: Some(Temp(0)),
                    },
                ],
                Terminator::Return(Some(Temp(0))),
            ),
        ],
        &["root"],
    );
    assert_eq!(
        program.functions.len(),
        2,
        "argument evaluation and call-table checks stay live"
    );
}

#[test]
fn private_literal_folding_keeps_host_effects_traps_and_non_numeric_pointers() {
    let mut effectful = literal("helper", DataRefKind::Quantity, "0");
    effectful.blocks[0]
        .instrs
        .push(Instr::GetAuthority { dest: Temp(1) });
    let mut trapping = literal("helper", DataRefKind::Quantity, "0");
    trapping.blocks[0].instrs.extend([
        Instr::Const {
            dest: Temp(1),
            value: 1,
        },
        Instr::Const {
            dest: Temp(2),
            value: 0,
        },
        Instr::Binary {
            dest: Temp(3),
            op: BinaryOp::Div,
            left: Temp(1),
            right: Temp(2),
        },
    ]);
    for helper in [
        effectful,
        trapping,
        literal("helper", DataRefKind::Name, "ledger"),
    ] {
        let program = optimized(
            vec![
                helper,
                function(
                    "root",
                    vec![call("helper", Some(Temp(0)))],
                    Terminator::Return(Some(Temp(0))),
                ),
            ],
            &["root"],
        );
        assert_eq!(program.functions.len(), 2);
        assert!(
            program
                .functions
                .iter()
                .find(|f| f.name == "root")
                .unwrap()
                .blocks[0]
                .instrs
                .iter()
                .any(|i| matches!(i, Instr::Call { callee, .. } if callee == "helper"))
        );
    }
}

#[test]
fn folded_unused_literal_still_reaches_canonical_validation() {
    let invalid = "not-a-quantity";
    let program = optimized(
        vec![
            literal("helper", DataRefKind::Quantity, invalid),
            function(
                "root",
                vec![call("helper", Some(Temp(0)))],
                Terminator::Return(None),
            ),
        ],
        &["root"],
    );
    assert_eq!(program.functions.len(), 1);
    assert!(
        matches!(program.functions[0].blocks[0].instrs.as_slice(), [Instr::DataRef { kind: DataRefKind::Quantity, value, .. }] if value == invalid),
        "a second DCE pass would erase the literal validator's input"
    );
}

#[test]
fn private_literal_folding_is_bounded_and_does_not_hide_bad_call_graphs() {
    let long = "0".repeat(MAX_FOLDED_LITERAL_TEXT_BYTES + 1);
    let program = optimized(
        vec![
            literal("helper", DataRefKind::Quantity, &long),
            function(
                "root",
                vec![call("helper", Some(Temp(0)))],
                Terminator::Return(Some(Temp(0))),
            ),
        ],
        &["root"],
    );
    assert_eq!(
        program.functions.len(),
        2,
        "long literal text is not cloned into each callsite"
    );

    let mut program = Program::from_ir(ir::Program {
        functions: vec![
            literal("helper", DataRefKind::Quantity, "0"),
            function(
                "root",
                vec![call("helper", Some(Temp(0))), call("missing", None)],
                Terminator::Return(Some(Temp(0))),
            ),
        ],
    })
    .unwrap();
    let error = program
        .optimize_and_retain(
            &BTreeSet::from(["root".to_owned()]),
            &BTreeMap::from([("helper".to_owned(), DataRefKind::Quantity)]),
        )
        .unwrap_err();
    assert!(error.contains("unresolved SSA callee `missing`"), "{error}");
}

#[test]
fn private_literal_folding_requires_exact_typed_eligibility() {
    for candidates in [
        BTreeMap::new(),
        BTreeMap::from([("helper".to_owned(), DataRefKind::Int)]),
    ] {
        let mut program = Program::from_ir(ir::Program {
            functions: vec![
                literal("helper", DataRefKind::Quantity, "0"),
                function(
                    "root",
                    vec![call("helper", Some(Temp(0)))],
                    Terminator::Return(Some(Temp(0))),
                ),
            ],
        })
        .expect("verified body");
        program
            .optimize_and_retain(&BTreeSet::from(["root".to_owned()]), &candidates)
            .expect("optimize");
        let ir = program.into_ir().expect("destroy");
        assert_eq!(
            ir.functions.len(),
            2,
            "missing or wrong typed signature cannot authorize folding"
        );
        assert!(ir.functions.iter().find(|function| function.name == "root").unwrap().blocks[0].instrs.iter().any(|instruction| matches!(instruction, Instr::Call { callee, .. } if callee == "helper")));
    }
}
