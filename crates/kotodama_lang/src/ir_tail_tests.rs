#[test]
fn rounded_numeric_division_is_constant_folded_or_one_numeric_round_instruction() {
    let dynamic = parse(
        "fn rounded(quantity value, decimal divisor, int scale) -> quantity { \
                return value.div_round( \
                    divisor: divisor, \
                    scale: scale, \
                    mode: Rounding::nearest_even, \
                ); \
            }",
    )
    .expect("parse dynamic rounded quantity division");
    let dynamic = lower(&analyze(&dynamic).expect("analyze rounded quantity division"))
        .expect("lower rounded quantity division");
    let calls = dynamic.functions[0]
        .blocks
        .iter()
        .flat_map(|block| &block.instrs)
        .filter(|instruction| {
            matches!(
                instruction,
                Instr::NumericRound {
                    op: NumericRoundOp::QuantityDiv,
                    ..
                }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let folded = parse(
        "fn rounded() -> decimal { \
                return 1.0.div_round( \
                    divisor: 8.0, \
                    scale: 2, \
                    mode: Rounding::nearest_even, \
                ); \
            }",
    )
    .expect("parse constant rounded decimal division");
    let folded = lower(&analyze(&folded).expect("analyze constant rounded decimal division"))
        .expect("lower constant rounded decimal division");
    assert!(folded.functions[0].blocks.iter().any(|block| {
        block.instrs.iter().any(|instruction| {
            matches!(
                instruction,
                Instr::DataRef {
                    kind: DataRefKind::Decimal,
                    value,
                    ..
                } if value == "0.12"
            )
        })
    }));
    assert!(folded.functions[0].blocks.iter().all(|block| {
        block
            .instrs
            .iter()
            .all(|instruction| !matches!(instruction, Instr::NumericRound { .. }))
    }));
}
#[test]
fn wrapping_builtins_have_distinct_ir() {
    let program = parse(include_str!(
        "ir/test_sources/wrapping_builtins_have_distinct_ir_1.ko"
    ))
    .expect("parse wrapping builtins");
    let program = lower(&analyze(&program).expect("analyze wrapping builtins"))
        .expect("lower wrapping builtins");
    let instructions = program.functions[0]
        .blocks
        .iter()
        .flat_map(|block| block.instrs.iter())
        .collect::<Vec<_>>();
    assert_eq!(
        instructions
            .iter()
            .filter(|instr| matches!(instr, Instr::WrappingBinary { .. }))
            .count(),
        3
    );
    assert_eq!(
        instructions
            .iter()
            .filter(|instr| matches!(instr, Instr::WrappingNeg { .. }))
            .count(),
        1
    );
}
