//! Physical carrier schema matches native encoding without changing the native profile.

use super::super::{CarrierStorageKeyV1, keyed};
use crate::runtime::definition::{DefinitionDescriptorV1, DefinitionId, SchemaV1, StorageShapeV1};
use norito::NoritoSchema as _;

#[test]
fn physical_descriptor_uses_the_layout_of_an_actual_canonical_storage_key() {
    let key = CarrierStorageKeyV1 {
        storage: DefinitionId([7; 32]),
        key: vec![1, 2, 3],
    };
    let encoded = norito::encode_canonical(&key).unwrap();
    let header = norito::core::Header::read(encoded.as_slice()).unwrap();
    assert_eq!(header.layout(), norito::core::WireLayout::Compact);
    let layout = format!("{}.{}.{}", header.major, header.minor, header.flags);
    let DefinitionDescriptorV1::Storage(backing) = keyed::backing_descriptor() else {
        panic!("physical carrier storage descriptor");
    };
    assert_eq!(
        backing.shape,
        StorageShapeV1::Table {
            key: SchemaV1::Codec(format!(
                "norito:{}@{layout}",
                CarrierStorageKeyV1::nominal_name()
            )),
        }
    );
    assert_eq!(
        backing.value,
        SchemaV1::Codec(format!("norito:{}@{layout}", Vec::<u8>::nominal_name()))
    );
}

#[test]
fn corrected_carrier_identities_leave_the_native_keyed_profile_unchanged() {
    assert_eq!(
        keyed::backing_id().to_string(),
        "516ed0d26e4cabba52bbc4e04bc8bf3d3b3a20b69ef72e3f5a83f62823d698c3"
    );
    assert_eq!(
        keyed::profile_id().to_string(),
        "d5935d1ff2e4a006bb0e4931981586d45719595f74c9af901a44d169e5aedb63"
    );
    assert_eq!(
        crate::runtime::keyed_state::profile_id().to_string(),
        "fdbb36106ac6a5ccf1096e7777e6f27cce88a28289e0412be81d77109d7d800b"
    );
}
