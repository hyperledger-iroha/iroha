//! IR lowering tests for tuple returns and CallMulti/TuplePack/TupleGet.
use ivm::kotodama::{ir, parser::parse, semantic::analyze};

#[test]
fn lower_call_tuple_return_emits_callmulti_and_tuplepack() {
    let src = r#"
        fn g(a: int, b: int) -> (int, int) { return (a, b); }
        fn f(a: int, b: int) -> (int, int) {
            // Return the tuple produced by g(a,b)
            return g(a, b);
        }
    "#;
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("analyze");
    let irp = ir::lower(&typed).expect("lower");
    // Find f
    let f = irp
        .functions
        .iter()
        .find(|x| x.name == "f")
        .expect("f present");
    let mut saw_callmulti = false;
    let mut saw_tuplepack = false;
    for bb in &f.blocks {
        for ins in &bb.instrs {
            match ins {
                ir::Instr::CallMulti { .. } => saw_callmulti = true,
                ir::Instr::TuplePack { .. } => saw_tuplepack = true,
                _ => {}
            }
        }
    }
    assert!(saw_callmulti && saw_tuplepack);
}

#[test]
fn lower_return_tuple_emits_returnn() {
    let src = r#"
        fn h(a: int, b: int, c: int) -> (int, int, int) {
            let t = (a, b);
            // Return three elements via tuple composition
            return (t.0, t.1, c);
        }
    "#;
    let prog = parse(src).expect("parse");
    let typed = analyze(&prog).expect("analyze");
    let irp = ir::lower(&typed).expect("lower");
    let h = irp
        .functions
        .iter()
        .find(|x| x.name == "h")
        .expect("h present");
    assert!(
        matches!(
            h.blocks.last().unwrap().terminator,
            ir::Terminator::ReturnN(_)
        ) || h
            .blocks
            .iter()
            .any(|b| matches!(b.terminator, ir::Terminator::ReturnN(_)))
    );
}
