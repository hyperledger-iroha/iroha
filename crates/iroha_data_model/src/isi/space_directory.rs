use super::*;
use crate::nexus::{AssetPermissionManifest, DataSpaceId, UniversalAccountId};
isi! {
    /// Publish or replace a capability manifest in the Space Directory.
    pub struct PublishSpaceDirectoryManifest {
        /// Canonical manifest payload (UAID, dataspace, rules, lifecycle schedule).
        pub manifest: AssetPermissionManifest,
    }
}
impl crate::seal::Instruction for PublishSpaceDirectoryManifest {}
impl PublishSpaceDirectoryManifest {
    /// Construct a space-directory manifest publication instruction.
    #[must_use]
    pub fn new(manifest: AssetPermissionManifest) -> Self {
        Self { manifest }
    }
}
isi! {
    /// Revoke an existing Space Directory manifest for a UAID/dataspace pair.
    pub struct RevokeSpaceDirectoryManifest {
        /// UAID that owns the manifest.
        pub uaid: UniversalAccountId,
        /// Dataspace hosting the manifest.
        pub dataspace: DataSpaceId,
        /// Epoch (inclusive) when the revocation takes effect.
        pub revoked_epoch: u64,
        /// Optional audit reason recorded with the revocation.
        #[norito(default)]
        pub reason: Option<String>,
    }
}
impl crate::seal::Instruction for RevokeSpaceDirectoryManifest {}
isi! {
    /// Expire an existing Space Directory manifest for a UAID/dataspace pair.
    pub struct ExpireSpaceDirectoryManifest {
        /// UAID that owns the manifest.
        pub uaid: UniversalAccountId,
        /// Dataspace hosting the manifest.
        pub dataspace: DataSpaceId,
        /// Epoch (inclusive) when the manifest expired.
        pub expired_epoch: u64,
    }
}
impl crate::seal::Instruction for ExpireSpaceDirectoryManifest {}
fn space_directory_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}
impl<'a> norito::core::DecodeFromSlice<'a> for PublishSpaceDirectoryManifest {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = space_directory_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let manifest = super::decode_aos_canonical_field::<AssetPermissionManifest>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { manifest }, offset))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for RevokeSpaceDirectoryManifest {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = space_directory_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let uaid = super::decode_aos_canonical_field::<UniversalAccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let dataspace = super::decode_aos_canonical_field::<DataSpaceId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let revoked_epoch = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let reason = if offset < bytes.len() {
            super::decode_aos_canonical_field::<Option<String>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        } else {
            None
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                uaid,
                dataspace,
                revoked_epoch,
                reason,
            },
            offset,
        ))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for ExpireSpaceDirectoryManifest {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = space_directory_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let uaid = super::decode_aos_canonical_field::<UniversalAccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let dataspace = super::decode_aos_canonical_field::<DataSpaceId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let expired_epoch = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                uaid,
                dataspace,
                expired_epoch,
            },
            offset,
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::ManifestVersion;
    use iroha_crypto::Hash;
    use crate::isi::test_support::{
        assert_registry_decodes_type_name as assert_registry_decodes, assert_slice_roundtrip,
    };
    fn uaid() -> UniversalAccountId {
        UniversalAccountId::from_hash(Hash::new(b"space-directory-uaid"))
    }
    fn dataspace() -> DataSpaceId {
        DataSpaceId::new(44)
    }
    fn manifest() -> AssetPermissionManifest {
        AssetPermissionManifest {
            version: ManifestVersion::default(),
            uaid: uaid(),
            dataspace: dataspace(),
            issued_ms: 123_456,
            activation_epoch: 7,
            expiry_epoch: Some(99),
            entries: Vec::new(),
        }
    }
    #[test]
    fn space_directory_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(PublishSpaceDirectoryManifest {
            manifest: manifest(),
        });
        assert_slice_roundtrip(RevokeSpaceDirectoryManifest {
            uaid: uaid(),
            dataspace: dataspace(),
            revoked_epoch: 11,
            reason: Some("operator request".to_owned()),
        });
        assert_slice_roundtrip(ExpireSpaceDirectoryManifest {
            uaid: uaid(),
            dataspace: dataspace(),
            expired_epoch: 13,
        });
    }
    #[test]
    fn space_directory_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<PublishSpaceDirectoryManifest>()
            .register_slice::<RevokeSpaceDirectoryManifest>()
            .register_slice::<ExpireSpaceDirectoryManifest>();
        assert_registry_decodes(
            &registry,
            PublishSpaceDirectoryManifest {
                manifest: manifest(),
            },
        );
        assert_registry_decodes(
            &registry,
            RevokeSpaceDirectoryManifest {
                uaid: uaid(),
                dataspace: dataspace(),
                revoked_epoch: 11,
                reason: Some("operator request".to_owned()),
            },
        );
        assert_registry_decodes(
            &registry,
            ExpireSpaceDirectoryManifest {
                uaid: uaid(),
                dataspace: dataspace(),
                expired_epoch: 13,
            },
        );
    }
}
