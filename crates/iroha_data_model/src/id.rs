use std::borrow::Borrow;

use derive_more::Display;
use iroha_crypto::HashOf;
use iroha_data_model_derive::{EnumRef, model};
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use norito::core::{DecodeFromSlice, Error as NoritoError};

pub use self::model::*;
use crate::error::ParseError;
use crate::{
    account, asset, block::BlockHeader, domain, nexus, nft, parameter, peer, permission, repo,
    role, rwa, trigger,
};

/// Maximum byte length of a canonical [`ChainId`].
///
/// Chain identifiers are ASCII, so this is also the maximum character count.
/// The bound keeps every signed, configured, and peer-advertised chain identity
/// small before any allocation is performed.
pub const MAX_CHAIN_ID_BYTES: usize = 128;

#[model]
mod model {
    use super::*;

    /// Exact deployment identity derived from the consensus hash of the genesis header.
    ///
    /// Unlike [`ChainId`], this value is not an operator-selected label. Distinct genesis
    /// headers necessarily produce distinct network identities, so signed protocol messages can
    /// use this type as an exact-lineage domain separator.
    #[derive(Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, IntoSchema)]
    #[repr(transparent)]
    #[schema(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
    pub struct NetworkId(HashOf<BlockHeader>);

    impl NetworkId {
        /// Construct the network identity from the exact genesis consensus-header hash.
        #[must_use]
        pub const fn from_genesis_hash(hash: HashOf<BlockHeader>) -> Self {
            Self(hash)
        }

        /// Borrow the exact genesis consensus-header hash.
        #[must_use]
        pub const fn as_genesis_hash(&self) -> &HashOf<BlockHeader> {
            &self.0
        }

        /// Recover the exact genesis consensus-header hash.
        #[must_use]
        pub const fn into_genesis_hash(self) -> HashOf<BlockHeader> {
            self.0
        }

        /// Borrow the canonical 32-byte identity.
        #[must_use]
        pub fn as_bytes(&self) -> &[u8; iroha_crypto::Hash::LENGTH] {
            self.0.as_ref()
        }
    }

    impl From<HashOf<BlockHeader>> for NetworkId {
        fn from(value: HashOf<BlockHeader>) -> Self {
            Self::from_genesis_hash(value)
        }
    }

    impl From<NetworkId> for HashOf<BlockHeader> {
        fn from(value: NetworkId) -> Self {
            value.into_genesis_hash()
        }
    }

    impl core::str::FromStr for NetworkId {
        type Err = iroha_crypto::error::ParseError;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            value.parse::<HashOf<BlockHeader>>().map(Self::from)
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::FastJsonWrite for NetworkId {
        fn write_json(&self, out: &mut String) {
            norito::json::FastJsonWrite::write_json(&self.0, out);
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::JsonDeserialize for NetworkId {
        fn json_deserialize(
            parser: &mut norito::json::Parser<'_>,
        ) -> Result<Self, norito::json::Error> {
            <HashOf<BlockHeader> as norito::json::JsonDeserialize>::json_deserialize(parser)
                .map(Self::from_genesis_hash)
        }
    }

    /// Canonical, deployment-selected identifier of a blockchain.
    ///
    /// The value is exact, case-sensitive ASCII. It starts and ends with an
    /// alphanumeric byte and may otherwise contain ASCII alphanumerics plus
    /// `.`, `_`, `:`, or `-`.
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
    pub struct ChainId(Box<str>);

    impl ChainId {
        fn parse(value: &str) -> Result<Self, ParseError> {
            if value.is_empty() {
                return Err(ParseError::new("`ChainId` must not be empty"));
            }
            if value.len() > MAX_CHAIN_ID_BYTES {
                return Err(ParseError::new(
                    "`ChainId` exceeds the 128-byte ASCII limit",
                ));
            }
            let bytes = value.as_bytes();
            if !bytes.first().is_some_and(u8::is_ascii_alphanumeric)
                || !bytes.last().is_some_and(u8::is_ascii_alphanumeric)
                || bytes.iter().any(|byte| {
                    !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b':' | b'-')
                })
            {
                return Err(ParseError::new(
                    "`ChainId` must be exact ASCII text beginning and ending with an \
                     alphanumeric byte and containing only alphanumerics, `.`, `_`, `:`, or `-`",
                ));
            }
            Ok(Self(value.into()))
        }

        pub(super) fn decode_text_wire(bytes: &[u8]) -> Result<(Self, usize), NoritoError> {
            let (len, header_len) = norito::core::inspect_len_from_slice(bytes)?;
            if len > MAX_CHAIN_ID_BYTES {
                return Err(NoritoError::Message(
                    "`ChainId` exceeds the 128-byte ASCII limit".into(),
                ));
            }
            let end = header_len
                .checked_add(len)
                .ok_or(NoritoError::LengthMismatch)?;
            let raw = bytes
                .get(header_len..end)
                .ok_or(NoritoError::LengthMismatch)?;
            let value = core::str::from_utf8(raw).map_err(|_| NoritoError::InvalidUtf8)?;
            norito::core::reserve_decode_allocation(len)?;
            let chain =
                Self::parse(value).map_err(|error| NoritoError::Message(error.reason.into()))?;
            norito::core::note_payload_access(bytes, end);
            Ok((chain, end))
        }

        pub(super) fn decode_wire(bytes: &[u8]) -> Result<(Self, usize), NoritoError> {
            let (wire, used) = norito::core::decode_field_canonical::<ChainIdWire>(bytes)?;
            Ok((wire.0.0, used))
        }

        /// Access inner string (owned).
        pub fn into_inner(self) -> Box<str> {
            self.0
        }
        /// Borrow inner string.
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl From<&'static str> for ChainId {
        fn from(value: &'static str) -> Self {
            Self::parse(value).expect("static chain id must be canonical")
        }
    }

    impl core::str::FromStr for ChainId {
        type Err = ParseError;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            Self::parse(value)
        }
    }

    impl TryFrom<String> for ChainId {
        type Error = ParseError;

        fn try_from(value: String) -> Result<Self, Self::Error> {
            Self::parse(&value)
        }
    }

    impl TryFrom<Box<str>> for ChainId {
        type Error = ParseError;

        fn try_from(value: Box<str>) -> Result<Self, Self::Error> {
            Self::parse(&value)
        }
    }

    impl AsRef<str> for ChainId {
        fn as_ref(&self) -> &str {
            self.as_str()
        }
    }

    impl Borrow<str> for ChainId {
        fn borrow(&self) -> &str {
            self.as_str()
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::FastJsonWrite for ChainId {
        fn write_json(&self, out: &mut String) {
            norito::json::JsonSerialize::json_serialize(self.as_str(), out);
        }
    }

    #[cfg(feature = "json")]
    impl norito::json::JsonDeserialize for ChainId {
        fn json_deserialize(
            parser: &mut norito::json::Parser<'_>,
        ) -> Result<Self, norito::json::Error> {
            let value = parser.parse_string()?;
            Self::parse(&value).map_err(|error| norito::json::Error::Message(error.reason.into()))
        }
    }

    /// Sized container for all possible identifications.
    #[derive(
        Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, EnumRef, FromVariant, IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(tag = "kind", content = "content"))]
    #[enum_ref(derive(FromVariant))]
    #[allow(clippy::enum_variant_names)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum IdBox {
        /// [`DomainId`](`domain::DomainId`) variant.
        DomainId(domain::DomainId),
        /// [`AccountId`](`account::AccountId`) variant.
        #[display("{_0}")]
        AccountId(account::AccountId),
        /// [`AssetDefinitionId`](`asset::id::AssetDefinitionId`) variant.
        #[display("{_0}")]
        AssetDefinitionId(asset::id::AssetDefinitionId),
        /// [`AssetId`](`asset::id::AssetId`) variant.
        #[display("{_0}")]
        AssetId(asset::id::AssetId),
        /// [`NftId`](`nft::NftId`) variant.
        #[display("{_0}")]
        NftId(nft::NftId),
        /// [`RwaId`](`rwa::RwaId`) variant.
        #[display("{_0}")]
        RwaId(rwa::RwaId),
        /// [`PeerId`](`peer::PeerId`) variant.
        PeerId(peer::PeerId),
        /// [`LaneId`](`nexus::LaneId`) variant.
        LaneId(nexus::LaneId),
        /// [`TriggerId`](trigger::TriggerId) variant.
        TriggerId(trigger::TriggerId),
        /// [`RoleId`](`role::RoleId`) variant.
        RoleId(role::RoleId),
        /// [`Permission`](`permission::Permission`) variant.
        Permission(permission::Permission),
        /// [`CustomParameter`](`parameter::CustomParameter`) variant.
        CustomParameterId(parameter::CustomParameterId),
        /// [`RepoAgreementId`](`repo::RepoAgreementId`) variant.
        RepoAgreementId(repo::RepoAgreementId),
    }
}

impl norito::core::NoritoSerialize for NetworkId {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        norito::core::NoritoSerialize::serialize(self.as_genesis_hash(), writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        Some(iroha_crypto::Hash::LENGTH)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(iroha_crypto::Hash::LENGTH)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for NetworkId {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("NetworkId deserialization must succeed for a valid genesis hash")
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        <HashOf<BlockHeader> as norito::core::NoritoDeserialize<'a>>::try_deserialize(
            archived.cast(),
        )
        .map(Self::from_genesis_hash)
    }
}

impl<'a> DecodeFromSlice<'a> for NetworkId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        <HashOf<BlockHeader> as DecodeFromSlice<'a>>::decode_from_slice(bytes)
            .map(|(hash, used)| (Self::from_genesis_hash(hash), used))
    }
}

/// Validation-aware decoder for the text field inside the structural V1
/// `ChainId` tuple-newtype representation.
struct ChainIdText(ChainId);

impl norito::core::NoritoSerialize for ChainIdText {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        <&str as norito::core::NoritoSerialize>::serialize(&self.0.as_str(), writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_hint(&self.0.as_str())
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        <&str as norito::core::NoritoSerialize>::encoded_len_exact(&self.0.as_str())
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for ChainIdText {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ChainId text deserialization must succeed for valid archives")
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        let payload = norito::core::payload_slice_from_ptr(ptr)?;
        let (chain_id, used) = ChainId::decode_text_wire(payload)?;
        if used != payload.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        Ok(Self(chain_id))
    }
}

/// Mirrors the single-field structural layout originally assigned to
/// `ChainId`, while delegating its inner field to the validating decoder.
#[derive(Encode, Decode)]
struct ChainIdWire(ChainIdText);

impl<'a> norito::core::NoritoDeserialize<'a> for ChainId {
    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived)
            .expect("ChainId deserialization must succeed for valid archives")
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        if let Ok(payload) = norito::core::payload_slice_from_ptr(ptr) {
            return ChainId::decode_wire(payload).map(|(chain, _)| chain);
        }

        let string = norito::core::NoritoDeserialize::deserialize(archived.cast::<String>());
        string
            .parse()
            .map_err(|error: ParseError| norito::core::Error::Message(error.reason.into()))
    }
}

impl<'a> DecodeFromSlice<'a> for ChainId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), NoritoError> {
        Self::decode_wire(bytes)
    }
}

mod id_box_codec {
    use super::*;

    #[derive(Encode, Decode)]
    enum IdBoxCandidate {
        DomainId(domain::DomainId),
        AccountId(account::AccountId),
        AssetDefinitionId(asset::id::AssetDefinitionId),
        AssetId(asset::id::AssetId),
        NftId(nft::NftId),
        RwaId(rwa::RwaId),
        PeerId(peer::PeerId),
        LaneId(nexus::LaneId),
        TriggerId(trigger::TriggerId),
        RoleId(role::RoleId),
        Permission(permission::Permission),
        CustomParameterId(parameter::CustomParameterId),
        RepoAgreementId(repo::RepoAgreementId),
    }

    impl From<IdBox> for IdBoxCandidate {
        fn from(id: IdBox) -> Self {
            match id {
                IdBox::DomainId(v) => Self::DomainId(v),
                IdBox::AccountId(v) => Self::AccountId(v),
                IdBox::AssetDefinitionId(v) => Self::AssetDefinitionId(v),
                IdBox::AssetId(v) => Self::AssetId(v),
                IdBox::NftId(v) => Self::NftId(v),
                IdBox::RwaId(v) => Self::RwaId(v),
                IdBox::PeerId(v) => Self::PeerId(v),
                IdBox::LaneId(v) => Self::LaneId(v),
                IdBox::TriggerId(v) => Self::TriggerId(v),
                IdBox::RoleId(v) => Self::RoleId(v),
                IdBox::Permission(v) => Self::Permission(v),
                IdBox::CustomParameterId(v) => Self::CustomParameterId(v),
                IdBox::RepoAgreementId(v) => Self::RepoAgreementId(v),
            }
        }
    }

    impl From<IdBoxCandidate> for IdBox {
        fn from(id: IdBoxCandidate) -> Self {
            match id {
                IdBoxCandidate::DomainId(v) => Self::DomainId(v),
                IdBoxCandidate::AccountId(v) => Self::AccountId(v),
                IdBoxCandidate::AssetDefinitionId(v) => Self::AssetDefinitionId(v),
                IdBoxCandidate::AssetId(v) => Self::AssetId(v),
                IdBoxCandidate::NftId(v) => Self::NftId(v),
                IdBoxCandidate::RwaId(v) => Self::RwaId(v),
                IdBoxCandidate::PeerId(v) => Self::PeerId(v),
                IdBoxCandidate::LaneId(v) => Self::LaneId(v),
                IdBoxCandidate::TriggerId(v) => Self::TriggerId(v),
                IdBoxCandidate::RoleId(v) => Self::RoleId(v),
                IdBoxCandidate::Permission(v) => Self::Permission(v),
                IdBoxCandidate::CustomParameterId(v) => Self::CustomParameterId(v),
                IdBoxCandidate::RepoAgreementId(v) => Self::RepoAgreementId(v),
            }
        }
    }

    impl norito::core::NoritoSerialize for IdBox {
        fn serialize(
            &self,
            writer: &mut norito::core::Encoder<'_>,
        ) -> Result<(), norito::core::Error> {
            let candidate: IdBoxCandidate = self.clone().into();
            norito::core::NoritoSerialize::serialize(&candidate, writer)
        }
    }

    impl<'de> norito::core::NoritoDeserialize<'de> for IdBox {
        fn deserialize(archived: &'de norito::core::Archived<IdBox>) -> Self {
            let candidate =
                <IdBoxCandidate as norito::core::NoritoDeserialize>::deserialize(archived.cast());
            candidate.into()
        }
    }
}

macro_rules! impl_encode_as_id_box {
    ($($ty:ty),+ $(,)?) => { $(
        impl $ty {
            /// [`Encode`] [`Self`] as [`IdBox`].
            pub fn encode_as_id_box(&self) -> Vec<u8> {
                IdBox::from(self.clone()).encode()
            }
        }
    )+ };
}

impl_encode_as_id_box! {
    peer::PeerId,
    domain::DomainId,
    account::AccountId,
    asset::id::AssetDefinitionId,
    asset::id::AssetId,
    rwa::RwaId,
    trigger::TriggerId,
    permission::Permission,
    role::RoleId,
    repo::RepoAgreementId,
    nexus::LaneId,
}

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice as _;

    use super::*;

    #[derive(Encode)]
    struct UncheckedChainIdWire(Box<str>);

    #[derive(Encode)]
    struct ChainIdEnvelope(ChainId);

    fn network_id_fixture() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::prehashed([0xA5; iroha_crypto::Hash::LENGTH]),
        ))
    }

    #[test]
    fn network_id_is_the_exact_transparent_genesis_hash_wire() {
        let network_id = network_id_fixture();
        let genesis_hash = *network_id.as_genesis_hash();
        let encoded = network_id.encode();

        assert_eq!(encoded.len(), iroha_crypto::Hash::LENGTH);
        assert_eq!(encoded, genesis_hash.encode());
        assert_eq!(network_id.encoded_len(), iroha_crypto::Hash::LENGTH);
        assert_eq!(network_id.as_bytes(), genesis_hash.as_ref());
        assert_eq!(
            NetworkId::decode_from_slice(&encoded).expect("decode exact network identity"),
            (network_id, iroha_crypto::Hash::LENGTH)
        );

        let framed = norito::to_bytes(&network_id).expect("frame network identity");
        assert_eq!(
            norito::decode_from_bytes::<NetworkId>(&framed).expect("framed roundtrip"),
            network_id
        );
        assert_eq!(
            network_id
                .to_string()
                .parse::<NetworkId>()
                .expect("text roundtrip"),
            network_id
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn network_id_json_is_the_canonical_hash_literal() {
        let network_id = network_id_fixture();
        let network_json = norito::json::to_json(&network_id).expect("serialize network identity");
        let hash_json =
            norito::json::to_json(network_id.as_genesis_hash()).expect("serialize genesis hash");

        assert_eq!(network_json, hash_json);
        assert!(network_json.starts_with("\"hash:"));
        assert_eq!(
            norito::json::from_str::<NetworkId>(&network_json).expect("JSON roundtrip"),
            network_id
        );
    }

    #[test]
    fn chain_id_from_str() {
        let id: ChainId = "test".parse().expect("valid chain id");
        assert_eq!(id, ChainId::from("test"));
    }

    #[test]
    fn chain_id_enforces_canonical_ascii_and_byte_limit() {
        let boundary = format!("a{}z", "0".repeat(MAX_CHAIN_ID_BYTES - 2));
        assert_eq!(
            boundary
                .parse::<ChainId>()
                .expect("boundary chain id")
                .as_str(),
            boundary
        );

        for invalid in [
            String::new(),
            "-leading".to_owned(),
            "trailing-".to_owned(),
            "white space".to_owned(),
            "control\u{0000}".to_owned(),
            "unicode-é".to_owned(),
            "a".repeat(MAX_CHAIN_ID_BYTES + 1),
        ] {
            assert!(
                invalid.parse::<ChainId>().is_err(),
                "invalid chain id was accepted: {invalid:?}"
            );
            assert!(
                ChainId::try_from(invalid.clone()).is_err(),
                "owned invalid chain id was accepted: {invalid:?}"
            );
        }
    }

    #[test]
    fn chain_id_uses_one_canonical_structural_v1_wire_layout() {
        let id = ChainId::from("test");
        let encoded = id.encode();
        assert_eq!(encoded, [5, 4, b't', b'e', b's', b't']);
        assert_eq!(id.encoded_len(), encoded.len());
        assert_eq!(
            ChainIdEnvelope(id.clone()).encode(),
            [6, 5, 4, b't', b'e', b's', b't']
        );

        let mut cursor = encoded.as_slice();
        assert_eq!(ChainId::decode(&mut cursor).expect("bare roundtrip"), id);
        assert_eq!(
            ChainId::decode_from_slice(&encoded).expect("slice roundtrip"),
            (id.clone(), encoded.len())
        );
        let framed = norito::to_bytes(&id).expect("frame ChainId");
        assert_eq!(
            norito::decode_from_bytes::<ChainId>(&framed).expect("framed roundtrip"),
            id
        );

        let transparent = "test".to_owned().encode();
        assert!(
            ChainId::decode_from_slice(&transparent).is_err(),
            "the transient transparent representation must not be accepted"
        );

        let mut truncated = encoded.clone();
        truncated.pop();
        assert!(ChainId::decode_from_slice(&truncated).is_err());

        let mut trailing = encoded;
        trailing.push(0);
        assert!(ChainId::decode_from_slice(&trailing).is_err());
    }

    #[test]
    fn chain_id_norito_decoders_cannot_bypass_validation() {
        for invalid in [
            String::new(),
            "bidi\u{202e}".to_owned(),
            "x".repeat(MAX_CHAIN_ID_BYTES + 1),
        ] {
            // Construct the structural representation without calling the
            // validating public constructor.
            let unchecked = UncheckedChainIdWire(invalid.clone().into_boxed_str());
            let encoded = unchecked.encode();
            let mut cursor = encoded.as_slice();
            assert!(
                ChainId::decode(&mut cursor).is_err(),
                "codec accepted invalid ChainId: {invalid:?}"
            );
            assert!(
                ChainId::decode_from_slice(&encoded).is_err(),
                "slice decoder accepted invalid ChainId: {invalid:?}"
            );
            let (payload, flags) = norito::codec::encode_with_header_flags(&unchecked);
            let framed = norito::core::frame_bare_with_header_flags::<ChainId>(&payload, flags)
                .expect("frame invalid structural ChainId fixture");
            assert!(
                norito::decode_from_bytes::<ChainId>(&framed).is_err(),
                "framed decoder accepted invalid ChainId: {invalid:?}"
            );
        }
    }

    #[test]
    fn chain_id_decoder_rejects_declared_oversize_before_body_access() {
        let mut inner = Vec::new();
        norito::core::write_len_to_vec(
            &mut inner,
            u64::try_from(MAX_CHAIN_ID_BYTES + 1).expect("chain id limit fits u64"),
        );
        let mut declared_oversize = Vec::new();
        norito::core::write_len_to_vec(
            &mut declared_oversize,
            u64::try_from(inner.len()).expect("inner header length fits u64"),
        );
        declared_oversize.extend_from_slice(&inner);

        let error = ChainId::decode_from_slice(&declared_oversize)
            .expect_err("oversized declared ChainId must fail before reading the body");
        assert!(
            error.to_string().contains("128-byte"),
            "decoder reached a generic truncation error before the ChainId limit: {error}"
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn chain_id_json_decoder_enforces_the_same_invariant() {
        for invalid in [
            "\"\"".to_owned(),
            "\"white space\"".to_owned(),
            format!("\"{}\"", "x".repeat(MAX_CHAIN_ID_BYTES + 1)),
        ] {
            assert!(
                norito::json::from_str::<ChainId>(&invalid).is_err(),
                "JSON accepted invalid ChainId: {invalid}"
            );
        }
    }
}
