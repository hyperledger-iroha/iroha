//! Musubi package registry instructions.

use super::*;
use crate::musubi::{
    MusubiPackageId, MusubiPackageRef, MusubiRelease, MusubiShortAlias, MusubiVersion,
};

isi! {
    /// Publish an immutable Musubi package release into the registry.
    pub struct PublishMusubiRelease {
        /// Complete release record requested by the publisher.
        pub release: MusubiRelease,
    }
}

impl PublishMusubiRelease {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.musubi.release.publish";

    /// Construct a publish instruction.
    #[must_use]
    pub const fn new(release: MusubiRelease) -> Self {
        Self { release }
    }
}

impl crate::seal::Instruction for PublishMusubiRelease {}

isi! {
    /// Yank an existing Musubi release without deleting or replacing it.
    pub struct YankMusubiRelease {
        /// Exact release to yank.
        pub package: MusubiPackageRef,
        /// Human-readable reason recorded with the yank.
        pub reason: String,
    }
}

impl YankMusubiRelease {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.musubi.release.yank";

    /// Construct a yank instruction.
    #[must_use]
    pub fn new(package: MusubiPackageRef, reason: impl Into<String>) -> Self {
        Self {
            package,
            reason: reason.into(),
        }
    }
}

impl crate::seal::Instruction for YankMusubiRelease {}

isi! {
    /// Bind or update a curated global short alias for a Musubi package id.
    pub struct SetMusubiShortAlias {
        /// Alias binding selected by governance or an elevated registry authority.
        pub alias: MusubiShortAlias,
    }
}

impl SetMusubiShortAlias {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.musubi.short_alias.set";

    /// Construct a short-alias instruction.
    #[must_use]
    pub const fn new(alias: MusubiShortAlias) -> Self {
        Self { alias }
    }
}

impl crate::seal::Instruction for SetMusubiShortAlias {}

isi! {
    /// Assert that a package id has a concrete released version.
    pub struct AssertMusubiReleaseExists {
        /// Canonical package id.
        pub package: MusubiPackageId,
        /// Exact version that must exist.
        pub version: MusubiVersion,
    }
}

impl AssertMusubiReleaseExists {
    /// Stable wire identifier for this instruction.
    pub const WIRE_ID: &'static str = "iroha.musubi.release.assert_exists";

    /// Construct an existence assertion.
    #[must_use]
    pub const fn new(package: MusubiPackageId, version: MusubiVersion) -> Self {
        Self { package, version }
    }
}

impl crate::seal::Instruction for AssertMusubiReleaseExists {}

fn musubi_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

macro_rules! impl_decode_one_musubi_field {
    ($ty:ident { $field:ident: $field_ty:ty }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = musubi_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                let $field = super::decode_aos_canonical_field::<$field_ty>(
                    super::read_aos_field(bytes, &mut offset, flags)?,
                    flags,
                )?;
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $field }, offset))
            }
        }
    };
}

impl_decode_one_musubi_field!(PublishMusubiRelease {
    release: MusubiRelease
});
impl_decode_one_musubi_field!(SetMusubiShortAlias {
    alias: MusubiShortAlias
});

impl<'a> norito::core::DecodeFromSlice<'a> for YankMusubiRelease {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = musubi_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let package = super::decode_aos_canonical_field::<MusubiPackageRef>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let reason = super::decode_aos_canonical_field::<String>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { package, reason }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for AssertMusubiReleaseExists {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = musubi_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let package = super::decode_aos_canonical_field::<MusubiPackageId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let version = super::decode_aos_canonical_field::<MusubiVersion>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { package, version }, offset))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{account::AccountId, sorafs::pin_registry::ManifestDigest};

    fn package_id() -> MusubiPackageId {
        "dex.universal/swap-core"
            .parse()
            .expect("musubi package id")
    }

    fn version() -> MusubiVersion {
        "1.2.3".parse().expect("musubi version")
    }

    fn package_ref() -> MusubiPackageRef {
        MusubiPackageRef::new(package_id(), version())
    }

    fn release() -> MusubiRelease {
        let keypair = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
        MusubiRelease::new(
            package_ref(),
            crate::musubi::MusubiArchiveRef::new(
                ManifestDigest::new([0x11; 32]),
                [0x22; 32],
                128,
                2,
            ),
            Vec::new(),
            vec!["quote".parse().expect("export")],
            None,
            AccountId::new(keypair.public_key().clone()),
            42,
        )
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &'static str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn musubi_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(PublishMusubiRelease::new(release()));
        assert_slice_roundtrip(YankMusubiRelease::new(package_ref(), "superseded"));
        assert_slice_roundtrip(SetMusubiShortAlias::new(MusubiShortAlias::new(
            "swap".parse().expect("alias"),
            package_id(),
        )));
        assert_slice_roundtrip(AssertMusubiReleaseExists::new(package_id(), version()));
    }

    #[test]
    fn musubi_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<PublishMusubiRelease>(PublishMusubiRelease::WIRE_ID)
            .register_with_id_slice::<YankMusubiRelease>(YankMusubiRelease::WIRE_ID)
            .register_with_id_slice::<SetMusubiShortAlias>(SetMusubiShortAlias::WIRE_ID)
            .register_with_id_slice::<AssertMusubiReleaseExists>(
                AssertMusubiReleaseExists::WIRE_ID,
            );

        assert_registry_decodes(
            &registry,
            PublishMusubiRelease::WIRE_ID,
            PublishMusubiRelease::new(release()),
        );
        assert_registry_decodes(
            &registry,
            YankMusubiRelease::WIRE_ID,
            YankMusubiRelease::new(package_ref(), "superseded"),
        );
        assert_registry_decodes(
            &registry,
            SetMusubiShortAlias::WIRE_ID,
            SetMusubiShortAlias::new(MusubiShortAlias::new(
                "swap".parse().expect("alias"),
                package_id(),
            )),
        );
        assert_registry_decodes(
            &registry,
            AssertMusubiReleaseExists::WIRE_ID,
            AssertMusubiReleaseExists::new(package_id(), version()),
        );
    }
}
