use derive_more::Display;
use iroha_data_model_derive::{EnumRef, model};
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

pub use self::model::*;
use crate::{account, asset, domain, nexus, nft, parameter, peer, permission, repo, role, trigger};

#[model]
mod model {
    use super::*;

    /// Unique id of blockchain
    #[derive(Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
    #[repr(transparent)]
    #[cfg_attr(feature = "json", norito(transparent))]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(unsafe {robust}))]
    pub struct ChainId(Box<str>);

    impl ChainId {
        /// Access inner string (owned).
        pub fn into_inner(self) -> Box<str> {
            self.0
        }
        /// Borrow inner string.
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }

    impl<T> From<T> for ChainId
    where
        T: Into<Box<str>>,
    {
        fn from(value: T) -> Self {
            ChainId(value.into())
        }
    }

    impl core::str::FromStr for ChainId {
        type Err = core::convert::Infallible;

        fn from_str(value: &str) -> Result<Self, Self::Err> {
            Ok(value.into())
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
            Ok(value.into())
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

mod id_box_codec {
    use super::*;

    #[derive(Encode, Decode)]
    enum IdBoxCandidate {
        DomainId(domain::DomainId),
        AccountId(account::AccountId),
        AssetDefinitionId(asset::id::AssetDefinitionId),
        AssetId(asset::id::AssetId),
        NftId(nft::NftId),
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
        fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    trigger::TriggerId,
    permission::Permission,
    role::RoleId,
    repo::RepoAgreementId,
    nexus::LaneId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_id_from_str() {
        let id: ChainId = "test".parse().expect("valid chain id");
        assert_eq!(id, ChainId::from("test"));
    }
}
