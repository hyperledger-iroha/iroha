//! Cold `NORITO_TRACE` diagnostics called from derive-generated decoders.

use crate::core::{Archived, payload_ctx, payload_slice};

/// Longest payload preview printed for one traced struct decode.
const STRUCT_PREVIEW_LEN: usize = 32;

/// `NORITO_TRACE` diagnostic for derive-generated struct decoders.
///
/// Reports the struct's offset in the active payload and previews at most
/// [`STRUCT_PREVIEW_LEN`] bytes from that offset. The preview is read through
/// [`payload_slice`], so it never reaches outside the live decode buffer, and a
/// marker outside the payload previews nothing. Without a payload context the
/// call is a no-op. Generated code calls this only behind
/// [`crate::debug_trace_enabled`], which release builds fold to `false`.
#[doc(hidden)]
#[inline]
pub fn trace_struct_decode(type_name: &'static str, archived: &Archived<impl Sized>) {
    trace_struct_decode_at(type_name, archived.cast());
}

#[cold]
#[inline(never)]
fn trace_struct_decode_at(type_name: &'static str, archived: &Archived<()>) {
    let Some((base, total)) = payload_ctx() else {
        return;
    };
    let offset = core::ptr::from_ref(archived).addr().saturating_sub(base);
    eprintln!("decode struct {type_name} ptr_off={offset} total={total}");
    let preview = payload_slice(archived).map_or(&[][..], struct_preview);
    eprintln!("decode struct {type_name} payload preview {preview:?}");
}

/// The first bytes of `tail` that a struct preview prints.
fn struct_preview(tail: &[u8]) -> &[u8] {
    &tail[..tail.len().min(STRUCT_PREVIEW_LEN)]
}

#[cfg(test)]
mod tests {
    use super::{STRUCT_PREVIEW_LEN, struct_preview, trace_struct_decode};
    use crate::core::{Archived, with_payload_ctx};

    #[test]
    fn struct_preview_is_bounded_by_the_payload_and_the_preview_width() {
        let payload: Vec<u8> = (0..40).collect();
        assert_eq!(struct_preview(&payload), &payload[..STRUCT_PREVIEW_LEN]);
        assert_eq!(struct_preview(&payload[30..]), &payload[30..]);
        assert!(struct_preview(&payload[40..]).is_empty());
        assert!(struct_preview(&[]).is_empty());
    }

    #[test]
    fn struct_decode_trace_stays_inside_the_active_payload() {
        let outside = [0xAA_u8; 4];
        // Without a payload context the trace has nothing to describe.
        trace_struct_decode("Outside", Archived::<u8>::start_of(&outside));
        let payload = [1_u8, 2, 3];
        with_payload_ctx(&payload, || {
            trace_struct_decode("Start", Archived::<u8>::start_of(&payload));
            // A marker at the payload end is reported but previews nothing.
            trace_struct_decode("End", Archived::<u8>::start_of(&payload[3..]));
            trace_struct_decode("Unrelated", Archived::<u8>::start_of(&outside));
        });
    }
}
