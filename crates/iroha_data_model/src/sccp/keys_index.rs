//! SCCP v1 world-state index values (`specs/sccp.md` §4.10).
//!
//! Most SCCP world maps are keyed by plain tuples, which the storage codec and the SCCP snapshot
//! envelope (headered Norito records) support directly. Cell values persist through JSON, which
//! has no tuple form, so a two-part cell value is a named struct here.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Resumable position of the bounded post-execution pruning step (§4.10).
///
/// Pruning deletes at most 1 024 records per block, so it resumes where the previous block
/// stopped. Both fields start at 0 and only move forward.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::keys_index::SccpPruneCursorV1")]
pub struct SccpPruneCursorV1 {
    /// Lowest attestation-subject height whose ordinary (non-rotation) signatures may still be
    /// retained.
    pub signatures_height: u64,
    /// Lowest rotation-subject height whose signatures may still be retained past the ordinary
    /// retention horizon.
    pub rotation_height: u64,
}

impl SccpPruneCursorV1 {
    /// Return the cursor with each position advanced to at least the given heights; a cursor
    /// never moves backward.
    #[must_use]
    pub fn advanced_to(self, signatures_height: u64, rotation_height: u64) -> Self {
        Self {
            signatures_height: self.signatures_height.max(signatures_height),
            rotation_height: self.rotation_height.max(rotation_height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sccp::test_support::{assert_rejects_unknown_field, roundtrip};

    #[test]
    fn prune_cursor_roundtrips_and_rejects_unknown_fields() {
        let cursor = SccpPruneCursorV1 {
            signatures_height: 17,
            rotation_height: 9,
        };
        roundtrip(&cursor);
        roundtrip(&SccpPruneCursorV1::default());
        assert_rejects_unknown_field(&cursor, &[]);
    }

    #[test]
    fn prune_cursor_never_moves_backward() {
        let cursor = SccpPruneCursorV1 {
            signatures_height: 10,
            rotation_height: 4,
        };
        assert_eq!(
            cursor.advanced_to(12, 2),
            SccpPruneCursorV1 {
                signatures_height: 12,
                rotation_height: 4,
            }
        );
        assert_eq!(cursor.advanced_to(0, 0), cursor);
    }
}
