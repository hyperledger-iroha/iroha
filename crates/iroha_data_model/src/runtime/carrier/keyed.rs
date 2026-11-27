//! Separate keyed commitment contract for definition-owned carrier tables.
//!
//! The native keyed profile and its accepted descriptors remain unchanged. This new profile
//! binds every installed storage identity and projects physical carrier rows into the exact
//! selected virtual namespace, bare key and bare carrier value payload.

use std::sync::LazyLock;

use super::CarrierStorageKeyV1;
use crate::runtime::definition::{
    CommitmentProfileDescriptorV1, DefinitionDescriptorV1, DefinitionId, ProjectionV1, SchemaV1,
    StorageDescriptorV1, StorageShapeV1,
};
use norito::NoritoSchema;

/// Physical snapshot and original-source owner; never a virtual keyed namespace.
pub const STORAGE_OWNER: &str = "world.runtime_generic_values";
/// Immutable carrier-aware radix commitment family.
pub const PROFILE_NAME: &str = "iroha.keyed-state.carrier-radix-blake3.v1";

/// Exact physical storage descriptor independently shared by catalog and portable verifiers.
pub fn backing_descriptor() -> DefinitionDescriptorV1 {
    let codec = |name: String| {
        SchemaV1::Codec(format!(
            "norito:{name}@{}.{}.{}",
            norito::core::VERSION_MAJOR,
            norito::core::VERSION_MINOR,
            norito::core::default_encode_flags(),
        ))
    };
    DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
        namespace: STORAGE_OWNER.into(),
        shape: StorageShapeV1::Table {
            key: codec(CarrierStorageKeyV1::nominal_name()),
        },
        value: codec(Vec::<u8>::nominal_name()),
        initial: None,
        invariants: iroha_crypto::Hash::new(
            b"iroha:runtime:carrier-backing:v1:definition-id:bare-key:canonical-record",
        )
        .into(),
        projection: ProjectionV1::BareNorito,
    })
}

/// Exact independently known physical owner identity.
pub fn backing_id() -> DefinitionId {
    static ID: LazyLock<DefinitionId> = LazyLock::new(|| {
        backing_descriptor()
            .id()
            .expect("valid carrier backing descriptor")
    });
    *ID
}

/// New commitment definition: unchanged radix hashing, exact selected schema and virtual rows.
pub fn profile_descriptor() -> DefinitionDescriptorV1 {
    let mut hash =
        blake3::Hasher::new_derive_key("iroha carrier keyed State commitment profile v1");
    hash.update(&crate::runtime::keyed_state::profile_id().0);
    hash.update(&backing_id().0);
    hash.update(b"selected-storage-ids;physical-owner-excluded;virtual-namespace;bare-compact-byte-key;bare-compact-carrier-value;no-orphans");
    DefinitionDescriptorV1::CommitmentProfile(CommitmentProfileDescriptorV1 {
        scheme: PROFILE_NAME.into(),
        parameters: *hash.finalize().as_bytes(),
    })
}

/// Exact carrier-aware commitment identity; it grants no activation authority.
pub fn profile_id() -> DefinitionId {
    static ID: LazyLock<DefinitionId> = LazyLock::new(|| {
        profile_descriptor()
            .id()
            .expect("valid carrier keyed profile")
    });
    *ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_owner_and_profile_are_exact_separate_definitions() {
        assert_eq!(backing_id(), backing_descriptor().id().unwrap());
        assert_eq!(profile_id(), profile_descriptor().id().unwrap());
        assert_ne!(profile_id(), crate::runtime::keyed_state::profile_id());
        let DefinitionDescriptorV1::Storage(backing) = backing_descriptor() else {
            unreachable!()
        };
        assert_eq!(backing.namespace, STORAGE_OWNER);
        assert_eq!(backing.projection, ProjectionV1::BareNorito);
        assert!(backing.initial.is_none());
    }
}
