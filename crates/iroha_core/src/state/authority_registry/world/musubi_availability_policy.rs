//! Independent archive-availability anchor retained in the State authority cut.
//!
//! Availability class and location/provider counts are checked projections of
//! the archive, locations, and SoraFS evidence on both retained World cuts.

use iroha_data_model::musubi::{ArchiveId, MusubiArchiveAvailabilityV1};
use norito::{Decode, Encode, NoritoSchema};

/// Canonical finalized anchor and index revision for one archive's availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:musubi-availability-authority:v1")]
pub(in crate::state) struct MusubiAvailabilityAuthorityV1 {
    archive_id: ArchiveId,
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
    index_revision: u64,
}

impl MusubiAvailabilityAuthorityV1 {
    /// Borrow the independently authoritative fields from a validated row.
    pub(in crate::state) fn from_record(row: &MusubiArchiveAvailabilityV1) -> Self {
        Self {
            archive_id: row.archive_id,
            finalized_height: row.finalized_height,
            finalized_block_hash: row.finalized_block_hash,
            index_revision: row.index_revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::musubi::MusubiStorageAvailabilityV1;

    fn row() -> MusubiArchiveAvailabilityV1 {
        MusubiArchiveAvailabilityV1 {
            archive_id: ArchiveId::new([0x11; 32]),
            availability: MusubiStorageAvailabilityV1::Unavailable,
            healthy_replicas: 0,
            active_locations: 0,
            finalized_height: 7,
            finalized_block_hash: [0x22; 32],
            index_revision: 9,
        }
    }

    fn frame(row: &MusubiArchiveAvailabilityV1) -> Vec<u8> {
        norito::encode_canonical(&MusubiAvailabilityAuthorityV1::from_record(row)).unwrap()
    }

    #[test]
    fn availability_anchor_roundtrips_with_explicit_v1_identity() {
        let row = row();
        row.validate().expect("canonical availability fixture");
        let projection = MusubiAvailabilityAuthorityV1::from_record(&row);
        assert_eq!(
            MusubiAvailabilityAuthorityV1::nominal_name(),
            "iroha:state:musubi-availability-authority:v1"
        );
        let encoded = frame(&row);
        assert_eq!(
            norito::decode_canonical::<MusubiAvailabilityAuthorityV1>(&encoded).unwrap(),
            projection
        );
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(frame(&row), encoded);
    }

    #[test]
    fn every_retained_availability_anchor_field_changes_canonical_bytes() {
        let baseline = row();
        let expected = frame(&baseline);
        let mut changed = baseline;
        changed.archive_id = ArchiveId::new([0x12; 32]);
        assert_ne!(frame(&changed), expected, "archive identity");
        let mut changed = baseline;
        changed.finalized_height += 1;
        assert_ne!(frame(&changed), expected, "finalized height");
        let mut changed = baseline;
        changed.finalized_block_hash[0] ^= 1;
        assert_ne!(frame(&changed), expected, "finalized block hash");
        let mut changed = baseline;
        changed.index_revision += 1;
        assert_ne!(frame(&changed), expected, "index revision");
    }

    #[test]
    fn derived_location_counts_and_class_are_not_independent_authority() {
        let baseline = row();
        let expected = frame(&baseline);
        let mut with_evidence = baseline;
        with_evidence.active_locations = 1;
        with_evidence.healthy_replicas = 1;
        with_evidence.availability = MusubiStorageAvailabilityV1::BelowQuorum;
        with_evidence
            .validate()
            .expect("second row is independently canonical");
        assert_eq!(frame(&with_evidence), expected);
    }
}
