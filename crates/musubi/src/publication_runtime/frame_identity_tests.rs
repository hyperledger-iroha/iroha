//! Original pinned-compiler identities and exact canonical container frames.

use super::*;
use crate::persistence_frame_fixture::assert_captured;

#[test]
fn captured_persistence_frame_owners_match_original_codec() {
    assert_captured::<PublicationProviderAttestationSetCheckpointV1>(
        "musubi::publication_runtime::PublicationProviderAttestationSetCheckpointV1",
        None,
    );
    assert_captured::<PublicationProviderAttestationCheckpointV1>(
        "musubi::publication_runtime::PublicationProviderAttestationCheckpointV1",
        None,
    );
}
