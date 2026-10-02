//! Active-only Option/Result allocation and payload access for the V1 table ABI.

use super::*;

pub(super) fn sum_layout_for_type(ty: &Type) -> Option<ivm_abi::sum::SumLayoutV1> {
    let word_count = |payload: &Type| u64::try_from(runtime_value_word_types(payload).len()).ok();
    match semantic::resolve_struct_type(ty) {
        Type::Option(payload) => ivm_abi::sum::SumLayoutV1::option(word_count(&payload)?).ok(),
        // The canonical tag is zero for `err` and one for `ok`.
        Type::Result(ok, err) => {
            ivm_abi::sum::SumLayoutV1::try_new(word_count(&err)?, word_count(&ok)?).ok()
        }
        _ => None,
    }
}
fn sum_active_payload_type(ty: &Type, tag: u64) -> Option<Option<Type>> {
    match (semantic::resolve_struct_type(ty), tag) {
        (Type::Option(_), 0) => Some(None),
        (Type::Option(payload), 1) => Some(Some(*payload)),
        (Type::Result(_, err), 0) => Some(Some(*err)),
        (Type::Result(ok, _), 1) => Some(Some(*ok)),
        _ => None,
    }
}
/// Allocate one canonical active-only sum value.
///
/// The allocation reserves the larger branch once, writes the discriminant, and writes only the
/// selected branch. In particular, this helper never evaluates or constructs an inactive payload.
pub(super) fn emit_sum_value(
    ctx: &mut LowerCtx,
    sum_ty: &Type,
    tag: u64,
    payload: Option<Temp>,
) -> Temp {
    let Some(layout) = sum_layout_for_type(sum_ty) else {
        ctx.record_error("internal error: invalid sum layout".into());
        let invalid = emit_i64_const(ctx, 0);
        return invalid;
    };
    let Some(payload_ty) = sum_active_payload_type(sum_ty, tag) else {
        ctx.record_error("internal error: invalid sum tag".into());
        let invalid = emit_i64_const(ctx, 0);
        return invalid;
    };
    let mut payload_words = Vec::new();
    match (payload, payload_ty.as_ref()) {
        (Some(value), Some(payload_ty)) => {
            collect_function_value_words(ctx, value, payload_ty, &mut payload_words);
        }
        (None, None) => {}
        _ => ctx.record_error("internal error: sum active payload mismatch".into()),
    }
    let actual_words = u64::try_from(payload_words.len()).unwrap_or(u64::MAX);
    if layout.validate_active_width(tag, actual_words).is_err() {
        ctx.record_error("internal error: sum active payload width mismatch".into());
    }
    let bytes = layout
        .allocation_bytes()
        .ok()
        .and_then(|bytes| i64::try_from(bytes).ok())
        .unwrap_or_else(|| {
            ctx.record_error("internal error: sum allocation exceeds V1 limits".into());
            8
        });
    let byte_count = emit_i64_const(ctx, bytes);
    let value = emit_alloc(ctx, byte_count);
    let tag_temp = emit_i64_const(ctx, i64::try_from(tag).expect("canonical sum tag fits int"));
    emit_store64_imm(ctx, value, 0, tag_temp);
    for (index, word) in payload_words.into_iter().enumerate() {
        let Some((base, imm)) = payload_word_address(ctx, value, index) else {
            break;
        };
        emit_store64_imm(ctx, base, imm, word);
    }
    value
}
pub(super) fn load_sum_tag(ctx: &mut LowerCtx, value: Temp) -> Temp {
    emit_load64_imm(ctx, value, 0)
}
pub(super) fn load_sum_payload(ctx: &mut LowerCtx, value: Temp, payload_ty: &Type) -> Temp {
    let word_types = runtime_value_word_types(payload_ty);
    let mut words = Vec::with_capacity(word_types.len());
    for index in 0..word_types.len() {
        let Some((base, imm)) = payload_word_address(ctx, value, index) else {
            return emit_i64_const(ctx, 0);
        };
        let word = emit_load64_imm(ctx, base, imm);
        words.push(word);
    }
    let mut index = 0;
    let payload = rebuild_function_value_from_words(ctx, payload_ty, &words, &mut index)
        .unwrap_or_else(|| {
            ctx.record_error("internal error: cannot rebuild sum payload".into());
            value
        });
    if index != words.len() {
        ctx.record_error("internal error: sum payload word count mismatch".into());
    }
    payload
}
/// Select a byte address without imposing the IR immediate width on a valid payload.
///
/// Product payloads can span the full V1 call-table width. The signed 16-bit IR
/// immediate covers only the first 4,095 words after the tag, so larger offsets
/// use an explicit scalar address addition. Memory ownership and bounds remain
/// enforced by the same runtime load/store operations.
fn payload_word_address(ctx: &mut LowerCtx, value: Temp, index: usize) -> Option<(Temp, i16)> {
    let offset = index
        .checked_add(1)
        .and_then(|words| words.checked_mul(ivm_abi::sum::SUM_WORD_BYTES_V1 as usize))
        .and_then(|bytes| i64::try_from(bytes).ok());
    let Some(offset) = offset else {
        ctx.record_error("internal error: sum payload offset exceeds V1 limits".into());
        return None;
    };
    if let Ok(immediate) = i16::try_from(offset) {
        Some((value, immediate))
    } else {
        let offset = emit_i64_const(ctx, offset);
        Some((emit_binary(ctx, BinaryOp::Add, value, offset), 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> LowerCtx {
        let mut ctx = LowerCtx::new(Type::Unit, 64, HashMap::new(), HashMap::new());
        let entry = ctx.new_label();
        ctx.start_block(entry);
        ctx
    }

    #[test]
    fn payload_addresses_cross_the_immediate_boundary_without_truncation() {
        for (index, offset) in [(0, 8), (4094, 32760), (4095, 32768), (8191, 65536)] {
            let mut ctx = context();
            let value = emit_i64_const(&mut ctx, 1024);
            let (base, immediate) = payload_word_address(&mut ctx, value, index).unwrap();
            assert!(ctx.error.is_none());
            if offset <= i64::from(i16::MAX) {
                assert_eq!(base, value);
                assert_eq!(i64::from(immediate), offset);
            } else {
                assert_eq!(immediate, 0);
                let instructions = &ctx.current.as_ref().unwrap().instrs;
                let offset_temp = instructions
                    .iter()
                    .find_map(|instruction| match instruction {
                        Instr::Const { dest, value } if *value == offset => Some(*dest),
                        _ => None,
                    })
                    .expect("the complete offset is materialized");
                assert!(instructions.iter().any(|instruction| matches!(instruction,
                    Instr::Binary { dest, op: BinaryOp::Add, left, right }
                        if *dest == base && *left == value && *right == offset_temp
                )));
            }
        }
    }

    #[test]
    fn overflowing_payload_addresses_record_an_error_without_emitting_access() {
        let mut ctx = context();
        let value = emit_i64_const(&mut ctx, 1024);
        let before = ctx.current.as_ref().unwrap().instrs.len();
        for index in [usize::MAX, usize::MAX / 8] {
            assert!(payload_word_address(&mut ctx, value, index).is_none());
            assert!(ctx.error.is_some());
            assert_eq!(ctx.current.as_ref().unwrap().instrs.len(), before);
        }
    }

    #[test]
    fn wide_payload_construction_and_loading_share_the_complete_address_range() {
        let width = ivm_abi::call::MAX_CALL_WORDS_V1;
        let payload_type = Type::Tuple(vec![Type::Bool; width]);
        for sum_type in [
            Type::Option(Box::new(payload_type.clone())),
            Type::Result(
                Box::new(payload_type.clone()),
                Box::new(payload_type.clone()),
            ),
        ] {
            let mut ctx = context();
            let bit = emit_i64_const(&mut ctx, 1);
            let product = emit_tuple_pack(&mut ctx, vec![bit; width]);
            let handle = emit_sum_value(&mut ctx, &sum_type, 1, Some(product));
            let _ = load_sum_payload(&mut ctx, handle, &payload_type);
            assert!(ctx.error.is_none(), "{:?}", ctx.error);
            let instructions = &ctx.current.as_ref().unwrap().instrs;
            assert_eq!(
                instructions
                    .iter()
                    .filter(|instruction| matches!(instruction, Instr::Store64Imm { .. }))
                    .count(),
                width + 1
            );
            assert_eq!(
                instructions
                    .iter()
                    .filter(|instruction| matches!(instruction, Instr::Load64Imm { .. }))
                    .count(),
                width
            );
            assert_eq!(
                instructions
                    .iter()
                    .filter(|instruction| matches!(instruction, Instr::Const { value: 65536, .. }))
                    .count(),
                2
            );
        }
    }
}
