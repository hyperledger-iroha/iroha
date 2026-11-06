//! Immutable trigger artifact identity, derived once on registration or restore.
//!
//! Neither this owner nor its digests are serialized. The existing DTO and World
//! projections retain the exact original bytecode. Read-only access cannot change
//! bytes behind a validated identity; replacing the owner recomputes both hashes.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::transaction::IvmBytecode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ImmutableArtifact {
    bytecode: IvmBytecode,
    blob: HashOf<IvmBytecode>,
    code: Hash,
}

impl ImmutableArtifact {
    pub(super) fn bytecode(&self) -> &IvmBytecode {
        &self.bytecode
    }
    pub(super) fn blob_hash(&self) -> HashOf<IvmBytecode> {
        self.blob
    }
    pub(super) fn code_hash(&self) -> Hash {
        self.code
    }
}

impl From<IvmBytecode> for ImmutableArtifact {
    fn from(bytecode: IvmBytecode) -> Self {
        let blob = HashOf::new(&bytecode);
        let code = ivm::contract_code_hash(bytecode.as_ref());
        Self {
            bytecode,
            blob,
            code,
        }
    }
}

impl AsRef<[u8]> for ImmutableArtifact {
    fn as_ref(&self) -> &[u8] {
        self.bytecode.as_ref()
    }
}

impl PartialEq<IvmBytecode> for ImmutableArtifact {
    fn eq(&self, other: &IvmBytecode) -> bool {
        self.bytecode == *other
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ImmutableArtifact {
    fn write_json(&self, output: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.bytecode, output);
    }

    fn write_json_to(
        &self,
        output: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::JsonSerialize::json_serialize_to(&self.bytecode, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_artifact_binding_is_derived_and_byte_identical_on_restore() {
        let original = IvmBytecode::from_compiled(vec![1, 2, 3, 4]);
        let artifact = ImmutableArtifact::from(original.clone());
        assert_eq!(artifact.blob_hash(), HashOf::new(&original));
        assert_eq!(
            artifact.code_hash(),
            ivm::contract_code_hash(original.as_ref())
        );
        assert_eq!(
            norito::encode_canonical(artifact.bytecode()).unwrap(),
            norito::encode_canonical(&original).unwrap()
        );
        assert_eq!(
            norito::json::to_json(&artifact).unwrap(),
            norito::json::to_json(&original).unwrap()
        );
    }

    #[test]
    fn lifecycle_artifact_bounded_json_preserves_original_writer_and_entry_bounds() {
        let original = IvmBytecode::from_compiled(vec![1, 2, 3, 4]);
        let artifact = ImmutableArtifact::from(original.clone());
        let expected = norito::json::to_json(&original).unwrap();
        for maximum in [expected.len(), expected.len() - 1] {
            assert_eq!(
                norito::json::to_json_bounded(&artifact, maximum),
                norito::json::to_json_bounded(&original, maximum)
            );
        }
        let entry = super::super::IvmBytecodeEntry {
            code_hash: artifact.code_hash(),
            original_contract: artifact,
            count: std::num::NonZeroU64::MIN,
        };
        let expected = norito::json::to_json(&entry).unwrap();
        assert_eq!(
            norito::json::to_json_bounded(&entry, expected.len()).unwrap(),
            expected
        );
        assert_eq!(
            norito::json::to_json_bounded(&entry, expected.len() - 1),
            Err(norito::json::BoundedJsonError::BodyTooLarge)
        );
    }
}
