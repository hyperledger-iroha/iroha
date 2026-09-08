//! Untrusted owner projection for native enrollment and recovered-session lookup.
//!
//! This sole canonical selector replaces host storage paths at the native open boundary.
//! Its identity is deterministic correlation data. Decoding or matching it proves neither
//! MiBank approval, enrollment, hardware custody, current selection nor monetary authority.

use super::{KAGEMUSHA_WIRE_VERSION_V1, KagemushaRetailEnrollmentOwnerV1};

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_crypto::Algorithm;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Bound enforced before parsing any selector header or collection length.
pub const KAGEMUSHA_ENROLLED_OPEN_SELECTOR_MAX_BYTES_V1: usize = 16 * 1024;

/// Exact first-release native owner lookup and signing-correlation projection.
///
/// A qualified native backend must independently derive its owner and compare every field.
/// Neither a caller-provided matching digest nor these bytes can create that native owner.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(schema_name = "iroha.kagemusha.v1.enrolled-open-selector")]
#[norito(deny_unknown_fields)]
pub struct KagemushaEnrolledOpenSelectorV1 {
    /// Sole first-release format, 1.
    pub version: u16,
    /// Canonical account and full immutable institution, ledger, asset and lane scope.
    pub owner: KagemushaRetailEnrollmentOwnerV1,
    /// Exact domain-separated digest derived from the entire immutable owner.
    pub enrollment_id: [u8; 32],
}

/// Closed failures for an untrusted selector; none carries partial admission evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KagemushaEnrolledOpenSelectorErrorV1 {
    /// Empty, oversized, malformed, alternate-schema or noncanonical encoding.
    Encoding,
    /// Unsupported format, malformed scope or unsupported account controller.
    Shape,
    /// Supplied enrollment identity does not commit to the complete supplied owner.
    Binding,
}

impl core::fmt::Display for KagemushaEnrolledOpenSelectorErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid enrolled-open selector: {self:?}")
    }
}

impl std::error::Error for KagemushaEnrolledOpenSelectorErrorV1 {}

type Result<T> = core::result::Result<T, KagemushaEnrolledOpenSelectorErrorV1>;

impl KagemushaEnrolledOpenSelectorV1 {
    /// Derive correlation bytes for the exact owner without granting it any authority.
    ///
    /// # Errors
    /// Rejects invalid scope, unsupported account controller or oversized encoding.
    pub fn new(owner: KagemushaRetailEnrollmentOwnerV1) -> Result<Self> {
        let enrollment_id = owner
            .enrollment_id()
            .map_err(|_| KagemushaEnrolledOpenSelectorErrorV1::Shape)?;
        let selector = Self {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            owner,
            enrollment_id,
        };
        selector.canonical_bytes()?;
        Ok(selector)
    }

    fn validate_shape(&self) -> Result<()> {
        use KagemushaEnrolledOpenSelectorErrorV1::{Binding, Shape};
        if self.version != KAGEMUSHA_WIRE_VERSION_V1
            || self
                .owner
                .account_id
                .controller()
                .single_signatory()
                .is_none_or(|key| key.algorithm() != Algorithm::Ed25519)
        {
            return Err(Shape);
        }
        if self.enrollment_id != self.owner.enrollment_id().map_err(|_| Shape)? {
            return Err(Binding);
        }
        Ok(())
    }

    /// Encode only the current canonical owner projection.
    ///
    /// # Errors
    /// Rejects unsupported format, invalid scope, mismatched digest or an oversized encoding.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        let bytes = norito::encode_canonical(self)
            .map_err(|_| KagemushaEnrolledOpenSelectorErrorV1::Encoding)?;
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ENROLLED_OPEN_SELECTOR_MAX_BYTES_V1 {
            return Err(KagemushaEnrolledOpenSelectorErrorV1::Encoding);
        }
        Ok(bytes)
    }

    /// Decode bounded canonical bytes and require byte-for-byte re-encoding equality.
    ///
    /// # Errors
    /// Rejects old paths, JSON, alternate schemas, unknown fields, trailing bytes, malformed
    /// identities and invalid or oversized inputs. Success is still untrusted lookup data.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        use KagemushaEnrolledOpenSelectorErrorV1::Encoding;
        if bytes.is_empty() || bytes.len() > KAGEMUSHA_ENROLLED_OPEN_SELECTOR_MAX_BYTES_V1 {
            return Err(Encoding);
        }
        let selector: Self = norito::decode_canonical_with_limits(
            bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| Encoding)?;
        if selector.canonical_bytes()? != bytes {
            return Err(Encoding);
        }
        Ok(selector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        NetworkId,
        account::AccountId,
        asset::AssetDefinitionId,
        kagemusha::{KAGEMUSHA_ASSET_SCALE_MAX_V1, KagemushaRetailEnrollmentRuntimeV1},
        nexus::{AxtAssetIncarnationV1, DataSpaceId},
    };
    use iroha_crypto::{Hash, HashOf, KeyPair};

    fn owner() -> KagemushaRetailEnrollmentOwnerV1 {
        KagemushaRetailEnrollmentOwnerV1 {
            account_id: AccountId::new(
                KeyPair::from_seed(vec![12; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            ),
            runtime: KagemushaRetailEnrollmentRuntimeV1 {
                fi_id: "mibank".parse().expect("FI"),
                ledger_dataspace_id: DataSpaceId::new(10),
                authentication_namespace: "mibank.bpng".parse().expect("namespace"),
                network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                    Hash::new(b"selector-fixture-genesis"),
                )),
                asset: AssetDefinitionId::from_uuid_bytes([
                    0x2f, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48, 0x84,
                    0xfd, 0xcd, 0x2f,
                ])
                .expect("asset"),
                asset_incarnation: AxtAssetIncarnationV1::try_from_bytes(
                    *Hash::new(b"selector-fixture-incarnation").as_ref(),
                )
                .expect("incarnation"),
                scale: 2,
            },
            lane_id: [32; 32],
        }
    }

    #[test]
    fn selector_roundtrip_preserves_full_immutable_identity() {
        let owner = owner();
        let selector = KagemushaEnrolledOpenSelectorV1::new(owner.clone()).expect("selector");
        assert_eq!(
            selector.enrollment_id,
            owner.enrollment_id().expect("identity")
        );
        let bytes = selector.canonical_bytes().expect("encode");
        assert_eq!(
            KagemushaEnrolledOpenSelectorV1::decode_canonical_exact(&bytes).expect("decode"),
            selector
        );
        assert_eq!(selector.canonical_bytes().expect("second encode"), bytes);

        {
            let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../fixtures/offline/kagemusha_enrolled_open_selector_v1.json"
            )))
            .expect("shared selector fixture");
            assert_eq!(
                hex::encode(&bytes),
                fixture
                    .get("selector_canonical_hex")
                    .and_then(norito::json::Value::as_str)
                    .expect("canonical fixture bytes")
            );
            assert_eq!(
                hex::encode(selector.enrollment_id),
                fixture
                    .get("enrollment_id_hex")
                    .and_then(norito::json::Value::as_str)
                    .expect("fixture identity")
            );
        }
    }

    #[test]
    fn selector_rejects_identity_substitution_for_every_scope_field() {
        let original = KagemushaEnrolledOpenSelectorV1::new(owner()).expect("selector");
        for field in 0..9 {
            let mut changed = original.clone();
            match field {
                0 => {
                    changed.owner.account_id = AccountId::new(
                        KeyPair::from_seed(vec![13; 32], Algorithm::Ed25519)
                            .public_key()
                            .clone(),
                    )
                }
                1 => changed.owner.runtime.fi_id = "other-bank".parse().expect("FI"),
                2 => changed.owner.runtime.ledger_dataspace_id = DataSpaceId::new(11),
                3 => {
                    changed.owner.runtime.authentication_namespace =
                        "other.bpng".parse().expect("namespace")
                }
                4 => {
                    changed.owner.runtime.network_id = NetworkId::from_genesis_hash(
                        HashOf::from_untyped_unchecked(Hash::new(b"different-genesis")),
                    )
                }
                5 => {
                    changed.owner.runtime.asset = AssetDefinitionId::from_uuid_bytes([
                        0x30, 0x17, 0xc7, 0x24, 0x66, 0xf8, 0x4a, 0x4b, 0xb8, 0xa8, 0xe2, 0x48,
                        0x84, 0xfd, 0xcd, 0x2f,
                    ])
                    .expect("asset")
                }
                6 => {
                    changed.owner.runtime.asset_incarnation = AxtAssetIncarnationV1::try_from_bytes(
                        *Hash::new(b"different-incarnation").as_ref(),
                    )
                    .expect("incarnation")
                }
                7 => changed.owner.runtime.scale = 3,
                8 => changed.owner.lane_id = [33; 32],
                _ => unreachable!(),
            }
            assert_eq!(
                changed.canonical_bytes(),
                Err(KagemushaEnrolledOpenSelectorErrorV1::Binding),
                "field {field}"
            );
            let bytes = norito::encode_canonical(&changed).expect("malformed projection encoding");
            assert_eq!(
                KagemushaEnrolledOpenSelectorV1::decode_canonical_exact(&bytes),
                Err(KagemushaEnrolledOpenSelectorErrorV1::Binding),
                "field {field}"
            );
            assert_ne!(
                KagemushaEnrolledOpenSelectorV1::new(changed.owner)
                    .expect("new projection")
                    .enrollment_id,
                original.enrollment_id
            );
        }
    }

    #[test]
    fn selector_rejects_reserved_version_and_mismatched_enrollment_digest() {
        let original = KagemushaEnrolledOpenSelectorV1::new(owner()).expect("selector");
        for version in [0, 2, u16::MAX] {
            let mut value = original.clone();
            value.version = version;
            assert_eq!(
                value.canonical_bytes(),
                Err(KagemushaEnrolledOpenSelectorErrorV1::Shape)
            );
            assert!(
                KagemushaEnrolledOpenSelectorV1::decode_canonical_exact(
                    &norito::encode_canonical(&value).expect("raw encode")
                )
                .is_err()
            );
        }
        for id in [[0; 32], [7; 32]] {
            let mut value = original.clone();
            value.enrollment_id = id;
            assert_eq!(
                value.canonical_bytes(),
                Err(KagemushaEnrolledOpenSelectorErrorV1::Binding)
            );
        }
    }

    #[test]
    fn selector_rejects_invalid_lane_scale_and_account_controller() {
        let mut bad = owner();
        bad.lane_id = [0; 32];
        assert_eq!(
            KagemushaEnrolledOpenSelectorV1::new(bad),
            Err(KagemushaEnrolledOpenSelectorErrorV1::Shape)
        );
        let mut bad = owner();
        bad.runtime.scale = KAGEMUSHA_ASSET_SCALE_MAX_V1 + 1;
        assert_eq!(
            KagemushaEnrolledOpenSelectorV1::new(bad),
            Err(KagemushaEnrolledOpenSelectorErrorV1::Shape)
        );
        let mut bad = owner();
        bad.account_id = AccountId::new(
            KeyPair::from_seed(vec![14; 32], Algorithm::Secp256k1)
                .public_key()
                .clone(),
        );
        assert_eq!(
            KagemushaEnrolledOpenSelectorV1::new(bad),
            Err(KagemushaEnrolledOpenSelectorErrorV1::Shape)
        );
    }

    #[test]
    fn selector_rejects_paths_json_other_schema_truncation_trailing_and_oversize() {
        let selector = KagemushaEnrolledOpenSelectorV1::new(owner()).expect("selector");
        let bytes = selector.canonical_bytes().expect("encode");
        for length in 0..bytes.len() {
            assert!(
                KagemushaEnrolledOpenSelectorV1::decode_canonical_exact(&bytes[..length]).is_err(),
                "truncation {length}"
            );
        }
        for malformed in [
            b"/durable/wallet.db".to_vec(),
            b"{\"version\":1}".to_vec(),
            norito::encode_canonical(&selector.owner).expect("owner encoding"),
            vec![0; KAGEMUSHA_ENROLLED_OPEN_SELECTOR_MAX_BYTES_V1 + 1],
        ] {
            assert!(KagemushaEnrolledOpenSelectorV1::decode_canonical_exact(&malformed).is_err());
        }
        let mut trailing = bytes;
        trailing.push(0);
        assert!(KagemushaEnrolledOpenSelectorV1::decode_canonical_exact(&trailing).is_err());
    }
}
