//! Exact function signatures and checked stack layouts for the V1 call-table ABI.

use ivm_abi::call::{CallWordV1, MAX_CALL_FRAME_BYTES_V1, MAX_CALL_WORDS_V1};

use crate::semantic::{Type, TypedFunction};

/// Flattened runtime roles for one typed function.
pub(crate) struct CallSignature {
    pub(crate) arguments: Vec<CallWordV1>,
    pub(crate) results: Vec<CallWordV1>,
}
impl CallSignature {
    pub(crate) fn for_function(function: &TypedFunction) -> Result<Self, String> {
        let mut arguments = Vec::new();
        for parameter in &function.param_types {
            if parameter.is_state {
                arguments.push(CallWordV1::StateRoot);
            } else {
                append_roles(&parameter.ty, &mut arguments)?;
            }
        }
        let mut results = Vec::new();
        append_roles(
            function.ret_ty.as_ref().unwrap_or(&Type::Unit),
            &mut results,
        )?;
        if arguments.len() > MAX_CALL_WORDS_V1 || results.len() > MAX_CALL_WORDS_V1 {
            return Err(format!(
                "function `{}` exceeds the V1 limit of {MAX_CALL_WORDS_V1} words per call table",
                function.name
            ));
        }
        Ok(Self { arguments, results })
    }
}
fn append_roles(ty: &Type, output: &mut Vec<CallWordV1>) -> Result<(), String> {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        let role = match ty {
            Type::Struct { fields, .. } if fields.is_empty() => CallWordV1::Unit,
            Type::Tuple(fields) if fields.is_empty() => CallWordV1::Unit,
            Type::Struct { fields, .. } => {
                pending.extend(fields.iter().rev().map(|(_, field)| field));
                continue;
            }
            Type::Tuple(fields) => {
                pending.extend(fields.iter().rev());
                continue;
            }
            Type::Unit => CallWordV1::Unit,
            Type::Bool => CallWordV1::Bool,
            Type::ErrorEnum(_) => CallWordV1::Error,
            Type::Option(_) | Type::Result(_, _) => CallWordV1::Sum,
            Type::List(_, _) => CallWordV1::List,
            Type::StateCursor(_) => CallWordV1::StateCursor,
            Type::StateMap(_, _) => CallWordV1::StateRoot,
            Type::Secret(inner) => {
                let kind = crate::abi_schema::state_value_kind_for_type(inner)
                    .and_then(|kind| kind.pointer_type())
                    .ok_or_else(|| "invalid private call-table value type".to_owned())?;
                let role = CallWordV1::SecretNumeric(kind as u16);
                if !role.validate() {
                    return Err("invalid private call-table numeric type".to_owned());
                }
                role
            }
            leaf => CallWordV1::Pointer(
                crate::abi_schema::state_value_kind_for_type(leaf)
                    .and_then(|kind| kind.pointer_type())
                    .ok_or_else(|| format!("unresolved call-table value type `{leaf:?}`"))?
                    as u16,
            ),
        };
        output.push(role);
        if output.len() > MAX_CALL_WORDS_V1 {
            return Err(format!(
                "call table exceeds {MAX_CALL_WORDS_V1} flattened words"
            ));
        }
    }
    Ok(())
}

pub(crate) fn checked_align_stack_frame_size(size: usize) -> Result<usize, String> {
    let padding = (16 - size % 16) % 16;
    size.checked_add(padding)
        .ok_or_else(|| "Kotodama stack frame alignment overflow".to_owned())
}

/// Disjoint scratch ranges, reserved once per invocation and reused across loop calls.
pub(crate) struct CallFrameLayout {
    pub(crate) argument_base_slot: usize,
    pub(crate) result_base_slot: usize,
    pub(crate) spill_base: usize,
    pub(crate) save_base: usize,
    pub(crate) state_table_base: usize,
    pub(crate) outgoing_argument_base: usize,
    pub(crate) outgoing_result_base: usize,
    pub(crate) bytes: usize,
}
impl CallFrameLayout {
    pub(crate) fn new(
        saves_return_address: bool,
        spill_bytes: usize,
        saved_registers: usize,
        state_words: usize,
        argument_words: usize,
        result_words: usize,
    ) -> Result<Self, String> {
        if argument_words > MAX_CALL_WORDS_V1 || result_words > MAX_CALL_WORDS_V1 {
            return Err("outgoing call table exceeds the V1 word limit".to_owned());
        }
        let mut cursor = usize::from(saves_return_address) * 8;
        let mut reserve = |bytes: usize| -> Result<usize, String> {
            let base = cursor;
            cursor = cursor
                .checked_add(bytes)
                .ok_or_else(|| "call frame size overflow".to_owned())?;
            Ok(base)
        };
        let argument_base_slot = reserve(8)?;
        let result_base_slot = reserve(8)?;
        let spill_base = reserve(spill_bytes)?;
        let save_base = reserve(
            saved_registers
                .checked_mul(8)
                .ok_or("saved register size overflow")?,
        )?;
        let state_table_base = reserve(
            state_words
                .checked_mul(8)
                .ok_or("state table size overflow")?,
        )?;
        let outgoing_argument_base = reserve(argument_words * 8)?;
        let outgoing_result_base = reserve(result_words * 8)?;
        let bytes = checked_align_stack_frame_size(cursor)?;
        if bytes > MAX_CALL_FRAME_BYTES_V1 as usize {
            return Err("function call frame exceeds the V1 stack limit".to_owned());
        }
        Ok(Self {
            argument_base_slot,
            result_base_slot,
            spill_base,
            save_base,
            state_table_base,
            outgoing_argument_base,
            outgoing_result_base,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_reserves_disjoint_tables_and_rejects_overflow() {
        let layout = CallFrameLayout::new(true, 24, 2, 4, 8192, 8192).expect("bounded frame");
        assert_eq!(layout.argument_base_slot, 8);
        assert_eq!(layout.result_base_slot, 16);
        assert_eq!(layout.spill_base, 24);
        assert_eq!(layout.save_base, 48);
        assert_eq!(layout.state_table_base, 64);
        assert_eq!(layout.outgoing_argument_base, 96);
        assert_eq!(layout.outgoing_result_base, 96 + 65536);
        assert_eq!(layout.bytes, 96 + 131072);
        assert!(CallFrameLayout::new(false, usize::MAX, 0, 0, 0, 0).is_err());
        assert!(CallFrameLayout::new(false, 0, 0, 0, 8193, 1).is_err());
    }
    #[test]
    fn signature_flattening_keeps_active_handles_and_secret_tags() {
        let mut roles = Vec::new();
        append_roles(
            &Type::Tuple(vec![
                Type::Unit,
                Type::Bool,
                Type::Option(Box::new(Type::Int)),
                Type::Secret(Box::new(Type::Int)),
            ]),
            &mut roles,
        )
        .expect("roles");
        assert_eq!(
            roles,
            [
                CallWordV1::Unit,
                CallWordV1::Bool,
                CallWordV1::Sum,
                CallWordV1::SecretNumeric(ivm_abi::pointer_abi::PointerType::Int as u16)
            ]
        );
        assert!(append_roles(&Type::NamedStruct("Unresolved".into()), &mut Vec::new()).is_err());
    }
}
