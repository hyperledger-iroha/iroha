//! Original pinned-compiler identities and exact canonical container frames.

use super::{persistence_frame_fixture::assert_captured, *};

#[test]
fn captured_persistence_frame_owners_match_original_codec() {
    assert_captured::<PublicationReleaseSignedEnvelopeV1>(
        "musubi::publish::PublicationReleaseSignedEnvelopeV1",
        None,
    );
    assert_captured::<PublicationReleaseSubmissionIntentV1>(
        "musubi::publish::PublicationReleaseSubmissionIntentV1",
        None,
    );
    assert_captured::<PublicationReleaseSubmissionTerminalV1>(
        "musubi::publish::PublicationReleaseSubmissionTerminalV1",
        None,
    );
    assert_captured::<PublicationReleaseSubmissionOutcomeV1>(
        "musubi::publish::PublicationReleaseSubmissionOutcomeV1",
        None,
    );
    assert_captured::<PublicationReleaseSubmissionAttemptV1>(
        "musubi::publish::PublicationReleaseSubmissionAttemptV1",
        None,
    );
    assert_captured::<PublicationAmxSubmissionV1>(
        "musubi::publish::PublicationAmxSubmissionV1",
        None,
    );
    assert_captured::<PublicationFinalEvidenceV1>(
        "musubi::publish::PublicationFinalEvidenceV1",
        None,
    );
    assert_captured::<PublicationFinalCheckpointV1>(
        "musubi::publish::PublicationFinalCheckpointV1",
        None,
    );
    assert_captured::<PublicationJournalV1>("musubi::publish::PublicationJournalV1", None);
}
