//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

#[test]
fn captured_event_codec_schema_identities() {
    check::<super::TriggerCompletedEvent>(
        "iroha_data_model::events::trigger_completed::model::TriggerCompletedEvent",
        "42e4786db007ca09c342bdde85d7028f",
        "42e4786db007ca09c342bdde85d7028f",
    );
    check::<super::TriggerCompletedOutcome>(
        "iroha_data_model::events::trigger_completed::model::TriggerCompletedOutcome",
        "910b6abe0bdacd58240da2cc16a423ce",
        "910b6abe0bdacd58240da2cc16a423ce",
    );
    check::<super::TriggerCompletedOutcomeType>(
        "iroha_data_model::events::trigger_completed::model::TriggerCompletedOutcomeType",
        "f9a41dc7e53eca777b891b614c9581ce",
        "f9a41dc7e53eca777b891b614c9581ce",
    );
    check::<super::TriggerCompletedEventFilter>(
        "iroha_data_model::events::trigger_completed::model::TriggerCompletedEventFilter",
        "1a99af4797dade1bdefefb5739d44f12",
        "1a99af4797dade1bdefefb5739d44f12",
    );
}
