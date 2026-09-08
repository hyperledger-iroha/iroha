//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::ExecuteTriggerEvent>(
        "iroha_data_model::events::execute_trigger::model::ExecuteTriggerEvent",
        "99179dd8b0ccf2b7e472063d560604dc",
        "99179dd8b0ccf2b7e472063d560604dc",
    );
    check::<super::ExecuteTriggerEventFilter>(
        "iroha_data_model::events::execute_trigger::model::ExecuteTriggerEventFilter",
        "7cd7d68a50c6d916d8f008517a439327",
        "7cd7d68a50c6d916d8f008517a439327",
    );
}
