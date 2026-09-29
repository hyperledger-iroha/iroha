//! Exact semantic lineage/history codecs; no whole-runtime or State-root claim.

use crate::state::{
    AutoscaleSampleRecord, SnapshotDataSpaceMetadata, SnapshotLaneIncarnationLineage,
    SnapshotLaneValidatorMode, SnapshotNexusOwnerPolicy,
};
use iroha_crypto::{Hash, HashOf};
use iroha_model_base::topology::{DataSpaceId, LaneId};

fn canonical_roundtrip<T>(value: &T, identity: &str) -> Vec<u8>
where
    T: norito::NoritoSerialize + std::fmt::Debug + PartialEq,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    assert_eq!(T::nominal_name(), identity);
    let frame = norito::encode_canonical(value).expect("canonical semantic owner");
    assert_eq!(norito::canonical_frame_len(value).unwrap(), frame.len());
    assert_eq!(&norito::decode_canonical::<T>(&frame).unwrap(), value);
    {
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(norito::encode_canonical(value).unwrap(), frame);
        let mut streamed = Vec::new();
        norito::core::write_canonical_to_writer(value, &mut streamed).unwrap();
        assert_eq!(streamed, frame);
    }
    let mut wrong_owner = frame.clone();
    wrong_owner[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<T>(&wrong_owner),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_canonical::<T>(&frame[..frame.len() - 1]).is_err());
    let mut trailing = frame.clone();
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
    frame
}

#[test]
fn canonical_lineage_codec_binds_every_retained_incarnation_field() {
    let record = SnapshotLaneIncarnationLineage {
        lane_id: LaneId::new(17),
        generation: 3,
        incarnation: Hash::new(b"retained lane incarnation"),
        activation_height: 6,
    };
    let frame = canonical_roundtrip(&record, "iroha_core::state::SnapshotLaneIncarnationLineage");
    for change in [
        |value: &mut SnapshotLaneIncarnationLineage| value.lane_id = LaneId::new(18),
        |value: &mut SnapshotLaneIncarnationLineage| value.generation += 1,
        |value: &mut SnapshotLaneIncarnationLineage| {
            value.incarnation = Hash::new(b"different incarnation")
        },
        |value: &mut SnapshotLaneIncarnationLineage| value.activation_height += 1,
    ] {
        let mut altered = record.clone();
        change(&mut altered);
        assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    }
    let records = vec![record];
    let encoded = norito::encode_canonical(&records).unwrap();
    assert_eq!(
        norito::decode_canonical::<Vec<SnapshotLaneIncarnationLineage>>(&encoded).unwrap(),
        records
    );
}

#[test]
fn canonical_history_codec_binds_every_autoscale_input() {
    let record = AutoscaleSampleRecord {
        block_height: 7,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"retained canonical header")),
        creation_time_ms: 123,
        work_count: 9,
    };
    let frame = canonical_roundtrip(&record, "iroha_core::state::AutoscaleSampleRecord");
    for change in [
        |value: &mut AutoscaleSampleRecord| value.block_height += 1,
        |value: &mut AutoscaleSampleRecord| {
            value.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"different header"))
        },
        |value: &mut AutoscaleSampleRecord| value.creation_time_ms += 1,
        |value: &mut AutoscaleSampleRecord| value.work_count += 1,
    ] {
        let mut altered = record;
        change(&mut altered);
        assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    }
    let records = vec![record];
    let encoded = norito::encode_canonical(&records).unwrap();
    assert_eq!(
        norito::decode_canonical::<Vec<AutoscaleSampleRecord>>(&encoded).unwrap(),
        records
    );
}

#[test]
fn canonical_owner_policy_codec_binds_dataspace_validator_routing_and_autoscale_inputs() {
    let record = SnapshotNexusOwnerPolicy {
        dataspaces: vec![
            SnapshotDataSpaceMetadata {
                id: DataSpaceId::UNIVERSAL,
                alias: "universal".to_owned(),
                fault_tolerance: 0,
            },
            SnapshotDataSpaceMetadata {
                id: DataSpaceId::new(17),
                alias: "restricted".to_owned(),
                fault_tolerance: 1,
            },
        ],
        public_validator_mode: SnapshotLaneValidatorMode::StakeElected,
        restricted_validator_mode: SnapshotLaneValidatorMode::AdminManaged,
        max_validators: 13,
        routing_default_lane: LaneId::new(3),
        routing_default_dataspace: DataSpaceId::new(17),
        autoscale_enabled: true,
        autoscale_min_lane_id: 10,
        autoscale_max_lane_id_exclusive: 20,
    };
    let frame = canonical_roundtrip(&record, "iroha_core::state::SnapshotNexusOwnerPolicy");
    let mut altered = record.clone();
    altered.dataspaces[0].alias = "changed".to_owned();
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.dataspaces[1].fault_tolerance += 1;
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.dataspaces[1].id = DataSpaceId::new(18);
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.public_validator_mode = SnapshotLaneValidatorMode::AdminManaged;
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.restricted_validator_mode = SnapshotLaneValidatorMode::StakeElected;
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.max_validators += 1;
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.routing_default_lane = LaneId::new(4);
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.routing_default_dataspace = DataSpaceId::UNIVERSAL;
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.autoscale_enabled = false;
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record.clone();
    altered.autoscale_min_lane_id += 1;
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
    let mut altered = record;
    altered.autoscale_max_lane_id_exclusive += 1;
    assert_ne!(norito::encode_canonical(&altered).unwrap(), frame);
}
