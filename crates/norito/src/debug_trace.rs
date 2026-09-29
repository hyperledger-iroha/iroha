//! Cold `NORITO_TRACE` diagnostics called from derive-generated decoders.

use crate::core::payload_ctx;

/// Longest payload preview printed for one traced struct decode.
const STRUCT_PREVIEW_LEN: usize = 32;

/// `NORITO_TRACE` diagnostic for derive-generated struct decoders.
///
/// Reports the struct's offset in the active payload and previews at most
/// [`STRUCT_PREVIEW_LEN`] bytes from that offset. The preview is clamped to the
/// payload, so it never reads outside the live decode buffer. Without a payload
/// context the call is a no-op. Generated code calls this only behind
/// [`crate::debug_trace_enabled`], which release builds fold to `false`.
#[doc(hidden)]
#[cold]
#[inline(never)]
pub fn trace_struct_decode(type_name: &'static str, ptr: *const u8) {
    let Some((base, total)) = payload_ctx() else {
        return;
    };
    let offset = (ptr as usize).saturating_sub(base);
    eprintln!("decode struct {type_name} ptr_off={offset} total={total}");
    // SAFETY: `payload_ctx` describes the live decode buffer of `total` bytes,
    // and its base always comes from a (non-null) slice pointer.
    let payload = unsafe { std::slice::from_raw_parts(base as *const u8, total) };
    let preview = struct_preview(payload, offset);
    eprintln!("decode struct {type_name} payload preview {preview:?}");
}

/// Bytes previewed for a struct starting `offset` bytes into `payload`.
fn struct_preview(payload: &[u8], offset: usize) -> &[u8] {
    let start = offset.min(payload.len());
    let end = start + (payload.len() - start).min(STRUCT_PREVIEW_LEN);
    &payload[start..end]
}

#[cfg(test)]
mod tests {
    use super::{STRUCT_PREVIEW_LEN, struct_preview, trace_struct_decode};
    use crate::core::PayloadCtxGuard;

    #[test]
    fn struct_preview_is_bounded_by_the_payload_and_the_preview_width() {
        let payload: Vec<u8> = (0..40).collect();
        assert_eq!(struct_preview(&payload, 0), &payload[..STRUCT_PREVIEW_LEN]);
        assert_eq!(struct_preview(&payload, 30), &payload[30..]);
        assert!(struct_preview(&payload, 40).is_empty());
        assert!(struct_preview(&payload, 4_096).is_empty());
        assert!(struct_preview(&[], 0).is_empty());
    }

    #[test]
    fn struct_decode_trace_stays_inside_the_active_payload() {
        let outside = [0xAA_u8; 4];
        // Without a payload context the trace has nothing to describe.
        trace_struct_decode("Outside", outside.as_ptr());
        let payload = [1_u8, 2, 3];
        let _ctx = PayloadCtxGuard::enter(&payload);
        trace_struct_decode("Start", payload.as_ptr());
        // A pointer past the payload end is reported but previews nothing.
        trace_struct_decode("PastEnd", payload.as_ptr().wrapping_add(64));
        trace_struct_decode("Unrelated", outside.as_ptr());
    }
}
