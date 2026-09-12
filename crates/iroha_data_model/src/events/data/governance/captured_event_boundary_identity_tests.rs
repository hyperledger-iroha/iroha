//! Source-bound compiler identities for this event capability's existing owners.

use crate::events::captured_event_boundary_identity_tests::check;

// These noncapturing cases keep the literal capture order in static storage.
const CASES: &[fn()] = &[
    || {
        check::<super::GovernanceEvent>(
            "iroha_data_model::events::data::governance::model::GovernanceEvent",
            "45955115e9b1624a40cdc005ea20534a",
            "45955115e9b1624a40cdc005ea20534a",
        )
    },
    || {
        check::<super::GovernanceThresholdKeyLifecycleAppliedV1>(
            "iroha_data_model::events::data::governance::model::GovernanceThresholdKeyLifecycleAppliedV1",
            "eb2f8d1f3865e22f722514929a0e62fb",
            "eb2f8d1f3865e22f722514929a0e62fb",
        )
    },
    || {
        check::<super::GovernanceProposalSubmitted>(
            "iroha_data_model::events::data::governance::model::GovernanceProposalSubmitted",
            "0adb50815821fe1cf0eff45fe508789b",
            "0adb50815821fe1cf0eff45fe508789b",
        )
    },
    || {
        check::<super::GovernanceLockCreated>(
            "iroha_data_model::events::data::governance::model::GovernanceLockCreated",
            "207604b261e010ce4d31ac21bc9abee3",
            "207604b261e010ce4d31ac21bc9abee3",
        )
    },
    || {
        check::<super::GovernanceLockExtended>(
            "iroha_data_model::events::data::governance::model::GovernanceLockExtended",
            "7bd86c43d3c76ed96755a9dd73254035",
            "7bd86c43d3c76ed96755a9dd73254035",
        )
    },
    || {
        check::<super::GovernanceProposalEnacted>(
            "iroha_data_model::events::data::governance::model::GovernanceProposalEnacted",
            "ac8965a3fd234882026b303b59c9a545",
            "ac8965a3fd234882026b303b59c9a545",
        )
    },
    || {
        check::<super::GovernanceProposalRejected>(
            "iroha_data_model::events::data::governance::model::GovernanceProposalRejected",
            "1015b7f6340cccb67592bfb271586937",
            "1015b7f6340cccb67592bfb271586937",
        )
    },
    || {
        check::<super::GovernanceBallotMode>(
            "iroha_data_model::events::data::governance::model::GovernanceBallotMode",
            "3db55b1979e65a9f7a227a3d5be8b42f",
            "3db55b1979e65a9f7a227a3d5be8b42f",
        )
    },
    || {
        check::<super::GovernanceBallotAccepted>(
            "iroha_data_model::events::data::governance::model::GovernanceBallotAccepted",
            "e5da63e437123ba95f830faa9eed228a",
            "e5da63e437123ba95f830faa9eed228a",
        )
    },
    || {
        check::<super::GovernanceBallotRejected>(
            "iroha_data_model::events::data::governance::model::GovernanceBallotRejected",
            "473a4524523586197916e2e3765fc9ac",
            "473a4524523586197916e2e3765fc9ac",
        )
    },
    || {
        check::<super::GovernanceSlashReason>(
            "iroha_data_model::events::data::governance::model::GovernanceSlashReason",
            "8de36b6997cce894ad8e61f91d738ffe",
            "8de36b6997cce894ad8e61f91d738ffe",
        )
    },
    || {
        check::<super::GovernanceReferendumOpened>(
            "iroha_data_model::events::data::governance::model::GovernanceReferendumOpened",
            "daab9d007b275d222d78b65040bce29c",
            "daab9d007b275d222d78b65040bce29c",
        )
    },
    || {
        check::<super::GovernanceReferendumClosed>(
            "iroha_data_model::events::data::governance::model::GovernanceReferendumClosed",
            "69c5eff41fc7d0a752669374f6f5548d",
            "69c5eff41fc7d0a752669374f6f5548d",
        )
    },
    || {
        check::<super::GovernanceLockUnlocked>(
            "iroha_data_model::events::data::governance::model::GovernanceLockUnlocked",
            "34cb1dcf309b6179ff0f34934b1cf061",
            "34cb1dcf309b6179ff0f34934b1cf061",
        )
    },
    || {
        check::<super::GovernanceLockSlashed>(
            "iroha_data_model::events::data::governance::model::GovernanceLockSlashed",
            "d4bb51ac7069ee0ab99ed821b0fb74b8",
            "d4bb51ac7069ee0ab99ed821b0fb74b8",
        )
    },
    || {
        check::<super::GovernanceLockRestituted>(
            "iroha_data_model::events::data::governance::model::GovernanceLockRestituted",
            "597f45e30296b7c7ccf3054925ba09d6",
            "597f45e30296b7c7ccf3054925ba09d6",
        )
    },
    || {
        check::<super::GovernanceCitizenRegistered>(
            "iroha_data_model::events::data::governance::model::GovernanceCitizenRegistered",
            "533826e9d3bb42cea2e84fb84c163c1b",
            "533826e9d3bb42cea2e84fb84c163c1b",
        )
    },
    || {
        check::<super::GovernanceCitizenRevoked>(
            "iroha_data_model::events::data::governance::model::GovernanceCitizenRevoked",
            "a303e53fd65c10e89569df1e0546015a",
            "a303e53fd65c10e89569df1e0546015a",
        )
    },
    || {
        check::<super::GovernanceParliamentAttemptCreated>(
            "iroha_data_model::events::data::governance::model::GovernanceParliamentAttemptCreated",
            "c8f0abaac71fc1f6e38c34091b5cf41d",
            "c8f0abaac71fc1f6e38c34091b5cf41d",
        )
    },
    || {
        check::<super::GovernanceParliamentLifecycleTransitionApplied>(
            "iroha_data_model::events::data::governance::model::GovernanceParliamentLifecycleTransitionApplied",
            "dc1414c02a476f78387c1c334fca26c0",
            "dc1414c02a476f78387c1c334fca26c0",
        )
    },
    || {
        check::<super::GovernanceReferendumDecided>(
            "iroha_data_model::events::data::governance::model::GovernanceReferendumDecided",
            "353a5b05b32d12cafe98dadf80e56fa8",
            "353a5b05b32d12cafe98dadf80e56fa8",
        )
    },
];

#[test]
fn captured_event_codec_schema_identities() {
    for check in CASES {
        check();
    }
}
