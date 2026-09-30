//! Exact dataspace ownership of an immutable contract artifact.

use iroha_crypto::Hash;
use iroha_model_base::topology::DataSpaceId;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// A content-addressed artifact inside one exact dataspace of the selected network.
///
/// The network is independently authenticated by the transaction or query context. Equal code
/// hashes in different dataspaces share no registry entry, pending upload or read authority.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::smart_contract::ContractArtifactId")]
pub struct ContractArtifactId {
    /// Exact dataspace; zero explicitly selects the universal dataspace.
    pub dataspace_id: DataSpaceId,
    /// Domain-separated hash of the complete deployable `.to` artifact.
    pub code_hash: Hash,
}

impl ContractArtifactId {
    /// Bind the complete artifact hash to an explicit dataspace.
    #[must_use]
    pub const fn new(dataspace_id: DataSpaceId, code_hash: Hash) -> Self {
        Self {
            dataspace_id,
            code_hash,
        }
    }
    /// Bind a hash to the exact dataspace encoded by a canonical contract address.
    ///
    /// # Errors
    /// Returns the address validation error if its encoded scope is invalid.
    pub fn for_address(
        address: &super::ContractAddress,
        code_hash: Hash,
    ) -> Result<Self, super::ContractAddressError> {
        Ok(Self::new(address.dataspace_id()?, code_hash))
    }
}

impl norito::json::JsonKeyCodec for ContractArtifactId {
    fn encode_json_key(&self, out: &mut String) {
        let key = format!(
            "{}|{}",
            self.dataspace_id.as_u64(),
            hex::encode(self.code_hash.as_ref())
        );
        norito::json::write_json_string(&key, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, norito::json::Error> {
        let invalid = || {
            norito::json::Error::Message("expected canonical dataspace|artifact-hash key".into())
        };
        let (dataspace, hash) = encoded.split_once('|').ok_or_else(invalid)?;
        let dataspace_id = dataspace.parse::<u64>().map_err(|_| invalid())?;
        if dataspace_id.to_string() != dataspace
            || hash.len() != 64
            || !hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid());
        }
        let mut bytes = [0; 32];
        hex::decode_to_slice(hash, &mut bytes).map_err(|_| invalid())?;
        let code_hash = Hash::prehashed(bytes);
        if code_hash.as_ref() != &bytes {
            return Err(invalid());
        }
        Ok(Self::new(DataSpaceId::new(dataspace_id), code_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::json::JsonKeyCodec as _;

    #[test]
    fn artifact_identity_uses_the_complete_address_dataspace() {
        let pair = iroha_crypto::KeyPair::try_from_seed(b"scoped artifact address".to_vec(), iroha_crypto::Algorithm::Ed25519).unwrap();
        let account = crate::account::AccountId::new(pair.public_key().clone());
        let network = crate::NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"scoped artifact network")));
        let address = super::super::ContractAddress::derive(&network, &account, 1, DataSpaceId::new(u64::MAX)).unwrap();
        let hash = Hash::new(b"exact artifact");
        assert_eq!(ContractArtifactId::for_address(&address, hash).unwrap(), ContractArtifactId::new(DataSpaceId::new(u64::MAX), hash));
    }
    #[test]
    fn artifact_identity_preserves_full_scope_in_every_codec() {
        let artifact = ContractArtifactId::new(DataSpaceId::new(u64::MAX), Hash::new(b"artifact"));
        let bytes = norito::encode_canonical(&artifact).unwrap();
        assert_eq!(
            norito::decode_canonical::<ContractArtifactId>(&bytes).unwrap(),
            artifact
        );
        let json = norito::json::to_json(&artifact).unwrap();
        assert_eq!(
            norito::json::from_str::<ContractArtifactId>(&json).unwrap(),
            artifact
        );
        let mut key = String::new();
        artifact.encode_json_key(&mut key);
        let decoded: String = norito::json::from_str(&key).unwrap();
        assert_eq!(
            ContractArtifactId::decode_json_key(&decoded).unwrap(),
            artifact
        );
        assert_ne!(
            artifact,
            ContractArtifactId::new(DataSpaceId::UNIVERSAL, artifact.code_hash)
        );
        for invalid in [
            format!("00|{}", hex::encode(artifact.code_hash.as_ref())),
            format!(
                "{}|{}",
                u64::MAX,
                hex::encode_upper(artifact.code_hash.as_ref())
            ),
            format!("{decoded}|extra"),
            "1|00".into(),
        ] {
            assert!(ContractArtifactId::decode_json_key(&invalid).is_err());
        }
    }
}
