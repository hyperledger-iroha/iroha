//! JNI projection of the explicit-layout instruction transcoder used by Swift and C#.

use super::{
    CONNECT_NORITO_BRIDGE_ABI_VERSION, catch_unwind_to_java, read_java_byte_array,
    read_java_byte_array_bounded, throw_java_illegal_argument, throw_java_illegal_state,
};
use crate::{
    BridgeError,
    transaction_layout::{layout, transaction_max_bytes, transcode_instruction},
};
use iroha_data_model::isi::MAX_INSTRUCTION_WIRE_ID_BYTES;
use jni::objects::JByteArray;

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_tx_norito_NativeInstructionLayoutBridge_nativeBridgeAbiVersion(
    _env: jni::EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
) -> jni::sys::jint {
    CONNECT_NORITO_BRIDGE_ABI_VERSION as jni::sys::jint
}

/// Transcode one instruction envelope payload between explicitly selected layouts.
///
/// Layout selectors are `0` (current: the Norito frame) and `1` (dense: the bare payload).
/// Returns `null` without an exception when the bridge registers no instruction under the wire
/// ID, so the caller keeps that envelope opaque; any other refusal throws.
///
/// # Safety
/// The JVM supplies a valid environment, class and byte arrays for this call.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_org_hyperledger_iroha_sdk_tx_norito_NativeInstructionLayoutBridge_nativeTranscodeInstruction(
    env: jni::EnvUnowned<'_>,
    _class: jni::objects::JClass<'_>,
    wire_id: JByteArray<'_>,
    payload: JByteArray<'_>,
    from_layout: jni::sys::jint,
    to_layout: jni::sys::jint,
) -> jni::sys::jbyteArray {
    crate::platform_jni::with_java_env(env, |env| {
        let Some(wire_id) =
            read_java_byte_array_bounded(env, &wire_id, "wireId", MAX_INSTRUCTION_WIRE_ID_BYTES)
        else {
            return std::ptr::null_mut();
        };
        // A dense payload of an instruction without fields is empty, so only the upper bound
        // applies before the array is copied.
        match payload.len(env) {
            Ok(len) if len <= transaction_max_bytes() => {}
            _ => {
                throw_java_illegal_argument(
                    env,
                    format!(
                        "instructionPayload must contain 0..{} bytes",
                        transaction_max_bytes()
                    ),
                );
                return std::ptr::null_mut();
            }
        }
        let Some(payload) = read_java_byte_array(env, &payload, "instructionPayload") else {
            return std::ptr::null_mut();
        };
        let Some(result) = catch_unwind_to_java(env, "instruction layout transcoding", || {
            let wire_id = std::str::from_utf8(&wire_id).map_err(|_| BridgeError::Utf8)?;
            let from = u8::try_from(from_layout)
                .map_err(|_| BridgeError::WireLayout)
                .and_then(layout)?;
            let to = u8::try_from(to_layout)
                .map_err(|_| BridgeError::WireLayout)
                .and_then(layout)?;
            transcode_instruction(wire_id, &payload, from, to)
        }) else {
            return std::ptr::null_mut();
        };
        let transcoded = match result {
            Ok(bytes) => bytes,
            Err(BridgeError::InstructionUnregistered) => return std::ptr::null_mut(),
            Err(BridgeError::WireLayout) => {
                throw_java_illegal_argument(env, "layout must be 0 (current) or 1 (dense)".into());
                return std::ptr::null_mut();
            }
            Err(_) => {
                throw_java_illegal_argument(
                    env,
                    "instruction payload is not the canonical payload of its wire ID in the selected layout"
                        .into(),
                );
                return std::ptr::null_mut();
            }
        };
        match env.byte_array_from_slice(&transcoded) {
            Ok(value) => value.into_raw(),
            Err(error) => {
                throw_java_illegal_state(
                    env,
                    format!("instruction payload allocation failed: {error}"),
                );
                std::ptr::null_mut()
            }
        }
    })
}
