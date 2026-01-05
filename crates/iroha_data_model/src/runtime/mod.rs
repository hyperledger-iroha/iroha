//! Runtime upgrade data model: manifests, records, and identifiers.
//! See `docs/source/runtime_upgrades.md` for the canonical documentation.
//!
//! Types here are used by instructions (ISIs) and events to coordinate
//! deterministic activation of ABI versions without downtime.

use std::{string::String, vec::Vec};

use iroha_crypto::{Hash, KeyPair, Signature};
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize};

use crate::smart_contract::manifest::ManifestProvenance;

/// Runtime upgrade manifest hashing helper.
fn manifest_hash(bytes: &[u8]) -> RuntimeUpgradeId {
    let hash = Hash::new(bytes);
    RuntimeUpgradeId(hash.into())
}

/// Content-addressable runtime-upgrade identifier (32-byte hash of manifest bytes).
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
    iroha_schema::IntoSchema,
)]
pub struct RuntimeUpgradeId(pub [u8; 32]);

impl RuntimeUpgradeId {
    /// Construct an identifier from canonical manifest bytes.
    #[must_use]
    pub fn from_manifest_bytes(bytes: &[u8]) -> Self {
        manifest_hash(bytes)
    }
}

/// SBOM digest bundled with a runtime upgrade.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RuntimeUpgradeSbomDigest {
    /// Digest algorithm identifier (e.g., `sha256`).
    pub algorithm: String,
    /// Digest bytes for the SBOM artifact.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub digest: Vec<u8>,
}

/// Runtime upgrade manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RuntimeUpgradeManifest {
    /// Human-readable name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// ABI version to activate.
    pub abi_version: u16,
    /// ABI hash for the target version.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub abi_hash: [u8; 32],
    /// New syscall numbers introduced in this version (additive only).
    pub added_syscalls: Vec<u16>,
    /// New pointer-type IDs (additive only).
    pub added_pointer_types: Vec<u16>,
    /// Activation window start (inclusive).
    pub start_height: u64,
    /// Activation window end (exclusive).
    pub end_height: u64,
    /// SBOM digests associated with the upgrade artefacts.
    #[norito(default)]
    pub sbom_digests: Vec<RuntimeUpgradeSbomDigest>,
    /// Raw SLSA attestation bytes (base64 in JSON).
    #[norito(default)]
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub slsa_attestation: Vec<u8>,
    /// Provenance signatures over the canonical manifest payload.
    #[norito(default)]
    pub provenance: Vec<ManifestProvenance>,
}

/// Canonical payload signed to attest a runtime upgrade manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RuntimeUpgradeManifestSignaturePayload {
    /// Human-readable name.
    pub name: String,
    /// Short description.
    pub description: String,
    /// ABI version to activate.
    pub abi_version: u16,
    /// ABI hash for the target version.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub abi_hash: [u8; 32],
    /// New syscall numbers introduced in this version (additive only).
    pub added_syscalls: Vec<u16>,
    /// New pointer-type IDs (additive only).
    pub added_pointer_types: Vec<u16>,
    /// Activation window start (inclusive).
    pub start_height: u64,
    /// Activation window end (exclusive).
    pub end_height: u64,
    /// SBOM digests associated with the upgrade artefacts.
    #[norito(default)]
    pub sbom_digests: Vec<RuntimeUpgradeSbomDigest>,
    /// Raw SLSA attestation bytes (base64 in JSON).
    #[norito(default)]
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub slsa_attestation: Vec<u8>,
}

impl From<&RuntimeUpgradeManifest> for RuntimeUpgradeManifestSignaturePayload {
    fn from(manifest: &RuntimeUpgradeManifest) -> Self {
        Self {
            name: manifest.name.clone(),
            description: manifest.description.clone(),
            abi_version: manifest.abi_version,
            abi_hash: manifest.abi_hash,
            added_syscalls: manifest.added_syscalls.clone(),
            added_pointer_types: manifest.added_pointer_types.clone(),
            start_height: manifest.start_height,
            end_height: manifest.end_height,
            sbom_digests: manifest.sbom_digests.clone(),
            slsa_attestation: manifest.slsa_attestation.clone(),
        }
    }
}

impl RuntimeUpgradeManifest {
    /// Compute the canonical Norito bytes for this manifest.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        norito::to_bytes(self).expect("runtime upgrade manifest encoding should succeed")
    }

    /// Compute the content-addressable identifier for this manifest.
    #[must_use]
    pub fn id(&self) -> RuntimeUpgradeId {
        RuntimeUpgradeId::from_manifest_bytes(&self.canonical_bytes())
    }

    /// Build the canonical payload that must be signed for provenance checks.
    #[must_use]
    pub fn signature_payload(&self) -> RuntimeUpgradeManifestSignaturePayload {
        RuntimeUpgradeManifestSignaturePayload::from(self)
    }

    /// Encode the canonical signing payload into Norito bytes.
    #[must_use]
    pub fn signature_payload_bytes(&self) -> Vec<u8> {
        norito::to_bytes(&self.signature_payload())
            .expect("runtime upgrade signature payload encoding should succeed")
    }

    /// Attach provenance by signing the canonical payload with the provided key pair.
    #[must_use]
    pub fn signed(mut self, key_pair: &KeyPair) -> Self {
        let payload = self.signature_payload_bytes();
        let signature = Signature::new(key_pair.private_key(), &payload);
        self.provenance.push(ManifestProvenance {
            signer: key_pair.public_key().clone(),
            signature,
        });
        self
    }
}

/// Runtime upgrade record stored in WSV.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RuntimeUpgradeRecord {
    /// Canonical manifest payload.
    pub manifest: RuntimeUpgradeManifest,
    /// Lifecycle state for the proposal.
    pub status: RuntimeUpgradeStatus,
    /// Account that proposed the upgrade.
    pub proposer: crate::account::AccountId,
    /// Block height where the proposal entered the ledger.
    pub created_height: u64,
}

/// Status of a proposed runtime upgrade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
pub enum RuntimeUpgradeStatus {
    /// Proposal recorded but not yet activated.
    Proposed,
    /// Upgrade scheduled to activate at the provided block height.
    ActivatedAt(u64),
    /// Proposal canceled before activation.
    Canceled,
}

/// Provenance validation failures for runtime upgrade manifests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum RuntimeUpgradeProvenanceError {
    /// Missing provenance payload when required.
    MissingProvenance,
    /// Required SBOM digests are missing.
    MissingSbom,
    /// SBOM digest entry is malformed.
    InvalidSbomDigest,
    /// Required SLSA attestation is missing.
    MissingSlsaAttestation,
    /// Required signatures are missing.
    MissingSignatures,
    /// Signature verification failed.
    InvalidSignature,
    /// Signature signer is not trusted.
    UntrustedSigner,
    /// Signature threshold was not met.
    SignatureThresholdNotMet,
}

impl RuntimeUpgradeProvenanceError {
    /// Stable label for telemetry and error surfaces.
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::MissingProvenance => "missing_provenance",
            Self::MissingSbom => "missing_sbom",
            Self::InvalidSbomDigest => "invalid_sbom_digest",
            Self::MissingSlsaAttestation => "missing_slsa_attestation",
            Self::MissingSignatures => "missing_signatures",
            Self::InvalidSignature => "invalid_signature",
            Self::UntrustedSigner => "untrusted_signer",
            Self::SignatureThresholdNotMet => "signature_threshold_not_met",
        }
    }
}

impl core::fmt::Display for RuntimeUpgradeProvenanceError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_label())
    }
}

/// Render the canonical markdown snippet documenting runtime-upgrade types.
///
/// This string is consumed by a doc-sync test to keep
/// `docs/source/runtime_upgrades.md` in sync with the data model.
#[must_use]
pub fn render_runtime_upgrade_types_markdown_section() -> String {
    let mut out = String::new();
    out.push_str("<!-- BEGIN RUNTIME UPGRADE TYPES -->\n");
    out.push_str(
        "- `RuntimeUpgradeId`: Blake2b-256 of the canonical Norito bytes for a manifest.\n",
    );
    out.push_str("- `RuntimeUpgradeManifest` fields:\n");
    out.push_str("  - `name: String` — human-readable label.\n");
    out.push_str("  - `description: String` — short description for operators.\n");
    out.push_str(
        "  - `abi_version: u16` — target ABI version to activate (must be 1 in the first release).\n",
    );
    out.push_str("  - `abi_hash: [u8; 32]` — canonical ABI hash for the target policy.\n");
    out.push_str(
        "  - `added_syscalls: Vec<u16>` — syscall numbers that become valid with this version.\n",
    );
    out.push_str(
        "  - `added_pointer_types: Vec<u16>` — pointer-type identifiers added by the upgrade.\n",
    );
    out.push_str("  - `start_height: u64` — first block height where activation is permitted.\n");
    out.push_str("  - `end_height: u64` — exclusive upper bound on the activation window.\n");
    out.push_str(
        "  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` — SBOM digests for upgrade artefacts.\n",
    );
    out.push_str(
        "  - `slsa_attestation: Vec<u8>` — raw SLSA attestation bytes (base64 in JSON).\n",
    );
    out.push_str(
        "  - `provenance: Vec<ManifestProvenance>` — signatures over the canonical payload.\n",
    );
    out.push_str("- `RuntimeUpgradeRecord` fields:\n");
    out.push_str("  - `manifest: RuntimeUpgradeManifest` — canonical proposal payload.\n");
    out.push_str("  - `status: RuntimeUpgradeStatus` — proposal lifecycle state.\n");
    out.push_str("  - `proposer: AccountId` — authority that submitted the proposal.\n");
    out.push_str(
        "  - `created_height: u64` — block height where the proposal entered the ledger.\n",
    );
    out.push_str("- `RuntimeUpgradeSbomDigest` fields:\n");
    out.push_str("  - `algorithm: String` — digest algorithm identifier.\n");
    out.push_str("  - `digest: Vec<u8>` — raw digest bytes (base64 in JSON).\n");
    out.push_str("<!-- END RUNTIME UPGRADE TYPES -->");
    out
}

#[cfg(feature = "json")]
impl JsonSerialize for RuntimeUpgradeId {
    fn json_serialize(&self, out: &mut String) {
        crate::json_helpers::fixed_bytes::serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for RuntimeUpgradeId {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        crate::json_helpers::fixed_bytes::deserialize(parser).map(Self)
    }
}

#[cfg(feature = "json")]
impl JsonSerialize for RuntimeUpgradeStatus {
    fn json_serialize(&self, out: &mut String) {
        out.push('{');
        match self {
            Self::Proposed => {
                json::write_json_string("Proposed", out);
                out.push_str(":null");
            }
            Self::ActivatedAt(height) => {
                json::write_json_string("ActivatedAt", out);
                out.push(':');
                height.json_serialize(out);
            }
            Self::Canceled => {
                json::write_json_string("Canceled", out);
                out.push_str(":null");
            }
        }
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for RuntimeUpgradeStatus {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = json::Value::json_deserialize(parser)?;
        let map = match value {
            json::Value::Object(map) => map,
            _ => {
                return Err(json::Error::InvalidField {
                    field: "RuntimeUpgradeStatus".into(),
                    message: String::from("expected object"),
                });
            }
        };
        let mut iter = map.into_iter();
        let (field, payload) = iter.next().ok_or_else(|| json::Error::InvalidField {
            field: "RuntimeUpgradeStatus".into(),
            message: String::from("expected single-key object"),
        })?;
        if let Some((extra, _)) = iter.next() {
            return Err(json::Error::UnknownField { field: extra });
        }
        match field.as_str() {
            "Proposed" => Ok(Self::Proposed),
            "Canceled" => Ok(Self::Canceled),
            "ActivatedAt" => match payload {
                json::Value::Number(num) => num
                    .as_u64()
                    .ok_or_else(|| json::Error::InvalidField {
                        field: "ActivatedAt".into(),
                        message: String::from("expected unsigned integer"),
                    })
                    .map(Self::ActivatedAt),
                other => Err(json::Error::InvalidField {
                    field: "ActivatedAt".into(),
                    message: format!("expected unsigned integer, got {other:?}"),
                }),
            },
            other => Err(json::Error::UnknownField {
                field: other.into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Manifest and record Norito payloads must roundtrip without losing proposer metadata.
    fn encode_decode_manifest_and_record() {
        let manifest = RuntimeUpgradeManifest {
            name: "ABI V1".to_string(),
            description: "Activate ABI v1".to_string(),
            abi_version: 1,
            abi_hash: [0x11; 32],
            added_syscalls: vec![4001, 4002],
            added_pointer_types: vec![0x0101, 0x0102],
            start_height: 1_000_000,
            end_height: 1_000_256,
            sbom_digests: Vec::new(),
            slsa_attestation: Vec::new(),
            provenance: Vec::new(),
        };
        let rec = RuntimeUpgradeRecord {
            manifest: manifest.clone(),
            status: RuntimeUpgradeStatus::Proposed,
            proposer:
                "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4@wonderland"
                    .parse()
                    .expect("account id"),
            created_height: 999_900,
        };
        let expected_id = manifest.id();
        assert_eq!(
            expected_id,
            RuntimeUpgradeId::from_manifest_bytes(&manifest.canonical_bytes())
        );
        let framed_manifest = norito::to_bytes(&manifest).expect("encode manifest");
        assert_eq!(manifest.canonical_bytes(), framed_manifest);
        let rbytes = norito::to_bytes(&rec).expect("encode record");
        let m2: RuntimeUpgradeManifest =
            norito::decode_from_bytes(&framed_manifest).expect("decode manifest");
        let r2: RuntimeUpgradeRecord = norito::decode_from_bytes(&rbytes).expect("decode record");
        assert_eq!(manifest, m2);
        assert_eq!(rec, r2);
    }

    #[test]
    fn signature_payload_excludes_provenance_signatures() {
        let kp = KeyPair::random();
        let mut manifest = RuntimeUpgradeManifest {
            name: "ABI V1".to_string(),
            description: "Activate ABI v1".to_string(),
            abi_version: 1,
            abi_hash: [0x11; 32],
            added_syscalls: vec![4001],
            added_pointer_types: vec![0x0101],
            start_height: 10,
            end_height: 20,
            sbom_digests: vec![RuntimeUpgradeSbomDigest {
                algorithm: "sha256".to_string(),
                digest: vec![0xAA, 0xBB],
            }],
            slsa_attestation: vec![0xCC],
            provenance: Vec::new(),
        };

        let payload = manifest.signature_payload_bytes();
        let signature = Signature::new(kp.private_key(), &payload);
        manifest.provenance.push(ManifestProvenance {
            signer: kp.public_key().clone(),
            signature: signature.clone(),
        });

        assert_eq!(payload, manifest.signature_payload_bytes());
        signature
            .verify(kp.public_key(), &payload)
            .expect("signature must verify");
    }
}
