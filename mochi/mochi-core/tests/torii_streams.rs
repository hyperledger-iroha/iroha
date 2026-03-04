//! Golden fixture decoding smoke tests for Torii payload helpers.

use iroha_data_model::{
    block,
    events::{EventBox, stream::EventMessage},
};
use iroha_version::codec::EncodeVersioned;
use mochi_core::torii::{EventCategory, ToriiError, decode_norito_with_alignment};

const BLOCK_WIRE_FIXTURE: &[u8] = include_bytes!("fixtures/canonical_block_wire.bin");
const EVENT_MESSAGE_FIXTURE: &[u8] = include_bytes!("fixtures/canonical_event_message.bin");

#[test]
fn canonical_block_fixture_roundtrips_via_wire_helpers() {
    let block =
        block::decode_versioned_signed_block(BLOCK_WIRE_FIXTURE).expect("decode canonical block");
    let canonical_wire = block
        .canonical_wire()
        .expect("emit canonical wire representation");

    assert_eq!(canonical_wire.as_framed(), BLOCK_WIRE_FIXTURE);
    assert_eq!(
        canonical_wire.as_versioned(),
        block.encode_versioned().as_slice()
    );
}

#[test]
fn canonical_event_fixture_produces_expected_category() {
    let buffer = EVENT_MESSAGE_FIXTURE.to_vec();
    let message = decode_norito_with_alignment::<EventMessage>(&buffer).expect("event message");
    let event_box: EventBox = message.into();

    let category = match &event_box {
        EventBox::Pipeline(_) | EventBox::PipelineBatch(_) => EventCategory::Pipeline,
        EventBox::Data(_) => EventCategory::Data,
        EventBox::Time(_) => EventCategory::Time,
        EventBox::ExecuteTrigger(_) => EventCategory::ExecuteTrigger,
        EventBox::TriggerCompleted(_) => EventCategory::TriggerCompleted,
    };

    // Sanity check that the decoded category matches the encoded fixture kind.
    assert_eq!(category, EventCategory::Time);

    // Verify helper surfaces decode errors.
    match decode_norito_with_alignment::<EventMessage>(&[0xF0]) {
        Err(ToriiError::Decode(_)) => {}
        other => panic!("expected decode error for malformed event payload, got {other:?}"),
    }
}
