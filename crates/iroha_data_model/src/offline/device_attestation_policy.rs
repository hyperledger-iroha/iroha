/// Deterministic snapshot of Google's Android Key Attestation status list.
///
/// Governance pins the exact upstream payload digest together with its HTTP
/// freshness metadata and the canonical set of certificate serials whose
/// status is not valid. Consensus consumes this snapshot without performing
/// network I/O.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineAndroidAttestationStatusSnapshotV1 {
    /// Snapshot layout marker.
    pub version: u16,
    /// SHA-256 digest of the exact upstream response payload.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub payload_sha256: [u8; 32],
    /// Upstream HTTP `Date` value in Unix milliseconds.
    pub response_date_ms: u64,
    /// Optional upstream HTTP `Last-Modified` value in Unix milliseconds.
    pub last_modified_ms: Option<u64>,
    /// Upstream `Cache-Control: max-age` lifetime in seconds.
    pub cache_max_age_seconds: u32,
    /// Canonical lowercase hexadecimal serials whose status is not valid.
    pub non_valid_serials: Vec<String>,
}

/// Governed Offline device-attestation verifier policy.
///
/// Nodes require this policy to be installed in chain state before accepting hardware-backed
/// offline registration or transaction authorization. The first-release platform roots are accepted
/// only when included in that explicit governed policy; absence of policy state fails closed.
/// Operators can rotate roots, publish deterministic revocations, and restrict accepted app
/// identities without relying on external middleware state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineDeviceAttestationPolicy {
    /// Policy format marker.
    pub version: u16,
    /// Trusted platform roots accepted by the on-chain verifier.
    pub trusted_roots: Vec<OfflineDeviceAttestationTrustedRoot>,
    /// SHA-256 digests of the exact raw DER encoding of revoked `TBSCertificate` values.
    pub revoked_certificate_tbs_sha256: Vec<Vec<u8>>,
    /// Accepted iOS App Attest app identities.
    pub ios_apps: Vec<OfflineIosAppAttestationPolicy>,
    /// Accepted Android `KeyMint` app identities.
    pub android_apps: Vec<OfflineAndroidAppAttestationPolicy>,
    /// Governed Android Key Attestation status-list snapshot.
    pub android_status_snapshot: Option<OfflineAndroidAttestationStatusSnapshotV1>,
    /// Explicitly enables iOS registration and online assertions when a matching
    /// entry exists in `ios_apps`.
    ///
    /// iOS App Attest is disabled when this is false; there is no implicit app
    /// identity fallback.
    pub require_ios_app_policy: bool,
    /// Explicitly enables Android registration when a matching entry exists in `android_apps`.
    ///
    /// Android `KeyMint` is disabled when this is false; there is no implicit
    /// unlisted-package or signing-certificate fallback.
    pub require_android_app_policy: bool,
}

/// Trusted platform root certificate for Offline device attestation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineDeviceAttestationTrustedRoot {
    /// Platform class, for example `ios-appattest` or `android-keymint`.
    pub platform: String,
    /// Root certificate DER bytes.
    pub der: Vec<u8>,
    /// Optional governance activation time in Unix milliseconds.
    pub not_before_ms: Option<u64>,
    /// Optional governance expiry time in Unix milliseconds.
    pub not_after_ms: Option<u64>,
}

/// Allowed iOS App Attest app identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineIosAppAttestationPolicy {
    /// Apple App ID prefix (normally the Apple Developer Team ID).
    pub team_id: String,
    /// iOS bundle identifier.
    pub bundle_id: String,
    /// App Attest environment, either `production` or `development`.
    pub environment: String,
    /// Allowed Apple validation categories from extension-bearing App Attest data.
    pub allowed_validation_categories: Vec<u32>,
    /// Allowed application bundle versions from extension-bearing App Attest data.
    pub allowed_bundle_versions: Vec<String>,
}

/// Allowed Android `KeyMint` app identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct OfflineAndroidAppAttestationPolicy {
    /// Android package name.
    pub package_name: String,
    /// Allowed Android signing certificate SHA-256 digests.
    pub signing_certificate_sha256: Vec<Vec<u8>>,
}

#[cfg(test)]
mod device_attestation_policy_tests {
    use super::*;
    use norito::codec::DecodeAll as _;

    fn snapshot() -> OfflineAndroidAttestationStatusSnapshotV1 {
        OfflineAndroidAttestationStatusSnapshotV1 {
            version: OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1,
            payload_sha256: [0x5a; 32],
            response_date_ms: 1_800_000_000_000,
            last_modified_ms: Some(1_799_999_000_000),
            cache_max_age_seconds: 3_600,
            non_valid_serials: vec!["1ab".to_owned(), "fe10".to_owned()],
        }
    }

    #[test]
    fn android_status_snapshot_norito_roundtrip() {
        let expected = snapshot();
        let encoded = expected.encode();
        let decoded =
            OfflineAndroidAttestationStatusSnapshotV1::decode_all(&mut encoded.as_slice())
                .expect("decode Android attestation status snapshot");
        assert_eq!(decoded, expected);
    }

    #[test]
    fn trusted_root_decodes_from_explicit_packed_field_layout() {
        let expected = OfflineDeviceAttestationTrustedRoot {
            platform: "ios-appattest".to_owned(),
            der: vec![0x42, 0x01, 0x08],
            not_before_ms: None,
            not_after_ms: None,
        };
        let flags = norito::core::header_flags::PACKED_STRUCT
            | norito::core::header_flags::COMPACT_LEN
            | norito::core::header_flags::FIELD_BITSET;
        let (payload, encoded_flags) = {
            let _guard = norito::core::DecodeFlagsGuard::enter(flags);
            norito::codec::encode_with_header_flags(&expected)
        };
        assert_eq!(encoded_flags & flags, flags);
        let (decoded, used) = {
            let _guard = norito::core::DecodeFlagsGuard::enter(encoded_flags);
            norito::core::decode_field_canonical::<OfflineDeviceAttestationTrustedRoot>(&payload)
                .expect("decode packed trusted-root field")
        };
        assert_eq!(used, payload.len());
        assert_eq!(decoded, expected);
    }

    #[cfg(feature = "json")]
    #[test]
    fn android_status_snapshot_json_shape_and_roundtrip() {
        let expected = snapshot();
        let json = norito::json::to_json(&expected)
            .expect("serialize Android attestation status snapshot JSON");
        assert!(json.contains("\"payload_sha256\":[90,90,90"));
        assert!(json.contains("\"last_modified_ms\":1799999000000"));
        assert!(json.contains("\"non_valid_serials\":[\"1ab\",\"fe10\"]"));
        let decoded: OfflineAndroidAttestationStatusSnapshotV1 =
            norito::json::from_str(&json).expect("decode Android attestation status snapshot JSON");
        assert_eq!(decoded, expected);
    }

    #[cfg(feature = "json")]
    #[test]
    fn device_attestation_policy_json_rejects_unknown_members() {
        let mut value = norito::json::to_value(&snapshot()).expect("encode snapshot JSON");
        value
            .as_object_mut()
            .expect("snapshot JSON object")
            .insert("retired_status".to_owned(), norito::json::Value::Null);
        norito::json::from_value::<OfflineAndroidAttestationStatusSnapshotV1>(value)
            .expect_err("device-attestation policy records must reject unknown members");
    }
}
