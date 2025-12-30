//! Tests covering payload framing helpers tied to the decode context.

use std::sync::{Mutex, OnceLock};

use iroha_schema::IntoSchema;
use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{
        Error as CoreError, PayloadCtxGuard, frame_current_payload_with_default_header,
        reset_decode_state,
    },
    from_bytes, to_bytes,
};

#[derive(Debug, Clone, IntoSchema, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct FrameSample {
    code: u32,
    message: String,
}

#[derive(Debug, Clone, IntoSchema, NoritoSerialize, NoritoDeserialize)]
struct OtherSample(u32);

fn frame_lock() -> &'static Mutex<()> {
    static FRAME_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    FRAME_LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn frame_current_payload_roundtrips_bytes() {
    let _guard = frame_lock().lock().expect("lock frame context");

    let sample = FrameSample {
        code: 200,
        message: "ok".into(),
    };

    let bytes = to_bytes(&sample).expect("encode sample");
    let archived = from_bytes::<FrameSample>(&bytes).expect("decode view");
    let decoded = <FrameSample as NoritoDeserialize>::deserialize(archived);
    assert_eq!(decoded, sample);

    let reframed = frame_current_payload_with_default_header::<FrameSample>()
        .expect("reframe payload using context");
    assert_eq!(reframed, bytes);

    reset_decode_state();
}

#[test]
fn frame_current_payload_without_context_errors() {
    let _guard = frame_lock().lock().expect("lock frame context");
    reset_decode_state();

    let err = frame_current_payload_with_default_header::<FrameSample>()
        .expect_err("missing payload context should error");
    assert!(matches!(err, CoreError::MissingPayloadContext));
}

#[test]
fn frame_current_payload_rejects_schema_mismatch() {
    let _guard = frame_lock().lock().expect("lock frame context");

    let sample = FrameSample {
        code: 10,
        message: "schema".into(),
    };
    let bytes = to_bytes(&sample).expect("encode sample");
    let (_, body) = bytes.split_at(norito::core::Header::SIZE);
    let _ctx =
        PayloadCtxGuard::enter_with_schema(body, <OtherSample as NoritoSerialize>::schema_hash());

    let err = frame_current_payload_with_default_header::<FrameSample>()
        .expect_err("schema mismatch should error");
    assert!(matches!(err, CoreError::SchemaMismatch));

    reset_decode_state();
}
