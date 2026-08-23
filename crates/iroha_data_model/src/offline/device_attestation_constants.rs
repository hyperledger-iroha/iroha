/// Domain-separation tag for on-chain Kagemusha device-attestation challenges.
pub const OFFLINE_DEVICE_ATTESTATION_CHALLENGE_DOMAIN: &str =
    "iroha:kagemusha:device-attestation-challenge:v1";
/// Canonical Android hardware-attestation platform label for Kagemusha.
pub const OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_PLATFORM: &str = "android-keymint";
/// Canonical Android one-use assertion scheme for Kagemusha.
pub const OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_ASSERTION_SCHEME: &str =
    "android-keymint-ecdsa-p256-usage-limit-v1";
/// Canonical Android assertion-key algorithm for Kagemusha.
pub const OFFLINE_DEVICE_ATTESTATION_ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM: &str =
    "ecdsa-p256-sha256";
/// Canonical Apple App Attest platform label for Kagemusha.
pub const OFFLINE_DEVICE_ATTESTATION_IOS_APP_ATTEST_PLATFORM: &str = "ios-appattest";
/// Fixed official Android Key Attestation status-list endpoint.
pub const OFFLINE_ANDROID_ATTESTATION_STATUS_URL_V1: &str =
    "https://android.googleapis.com/attestation/status";
/// Current governed Android Key Attestation status snapshot layout.
pub const OFFLINE_ANDROID_ATTESTATION_STATUS_SNAPSHOT_VERSION_V1: u16 = 1;
/// Maximum non-valid certificate serials retained by one governed snapshot.
pub const OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_NON_VALID_SERIALS_V1: usize = 4_096;
/// Maximum lowercase hexadecimal bytes accepted for one certificate serial.
pub const OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_SERIAL_HEX_BYTES_V1: usize = 40;
/// Maximum upstream cache lifetime accepted for one governed snapshot.
pub const OFFLINE_ANDROID_ATTESTATION_STATUS_MAX_CACHE_AGE_SECONDS_V1: u32 = 86_400;
/// Current governed Offline device-attestation policy layout.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_VERSION_V2: u16 = 2;
/// Current finalized Offline device-attestation policy-view layout.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_VIEW_VERSION_V1: u16 = 1;
/// Current Offline device-eligibility credential layout.
pub const OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_VERSION_V1: u16 = 1;
/// Current finalized-policy binding layout used by eligibility credentials.
pub const OFFLINE_DEVICE_POLICY_FINALITY_BINDING_VERSION_V1: u16 = 1;
/// Current attested Android device-property layout.
pub const OFFLINE_ANDROID_ATTESTED_DEVICE_PROPERTIES_VERSION_V2: u16 = 2;
/// Highest Android Key Attestation `osVersion` accepted by policy selectors.
pub const OFFLINE_ANDROID_OS_VERSION_MAX_V2: u32 = 999_999;
/// Maximum lifetime of one Offline device-eligibility credential.
pub const OFFLINE_DEVICE_ELIGIBILITY_CREDENTIAL_MAX_TTL_MS_V1: u64 = 24 * 60 * 60 * 1_000;
/// Android 12 encoded as the Key Attestation `osVersion` integer.
pub const OFFLINE_ANDROID_12_OS_VERSION_FLOOR_V2: u32 = 120_000;
/// Maximum governed Android vulnerability rules in one policy.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_VULNERABILITY_RULES_V2: usize = 256;
/// Maximum source identifiers retained by one governed vulnerability rule.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_SOURCES_V2: usize = 8;
/// Maximum CVE identifiers retained by one governed vulnerability rule.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_CVES_V2: usize = 16;
/// Maximum printable bytes in one governed vulnerability-rule identifier.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_ID_BYTES_V2: usize = 128;
/// Maximum printable bytes in one governed vulnerability source identifier.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_SOURCE_BYTES_V2: usize = 512;
/// Maximum printable bytes in one governed CVE identifier.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_RULE_CVE_BYTES_V2: usize = 32;
/// Maximum attested Android build-property bytes retained per property.
pub const OFFLINE_ANDROID_ATTESTED_PROPERTY_MAX_BYTES_V2: usize = 128;
/// Maximum attested verified-boot key bytes retained in registration state.
pub const OFFLINE_ANDROID_VERIFIED_BOOT_KEY_MAX_BYTES_V2: usize = 1_024;
/// Samsung bulletin containing the August 2021 Keymaster IV-reuse fix.
pub const OFFLINE_SAMSUNG_SMR_AUGUST_2021_SOURCE_V2: &str =
    "https://security.samsungmobile.com/securityUpdate.smsb?month=8&year=2021";
/// Samsung bulletin containing the October 2021 Keymaster downgrade fix.
pub const OFFLINE_SAMSUNG_SMR_OCTOBER_2021_SOURCE_V2: &str =
    "https://security.samsungmobile.com/securityUpdate.smsb?month=10&year=2021";
/// Samsung bulletin containing the July 2026 fabricKeymaster fix.
pub const OFFLINE_SAMSUNG_SMR_JULY_2026_SOURCE_V2: &str =
    "https://security.samsungmobile.com/securityUpdate.smsb?month=7&year=2026";
/// Peer-reviewed analysis of Samsung's 2021 TrustZone Keymaster failures.
pub const OFFLINE_SAMSUNG_KEYMASTER_USENIX_2022_SOURCE_V2: &str =
    "https://www.usenix.org/conference/usenixsecurity22/presentation/shakevsky";
/// Maximum canonical Norito bytes for one governed device-attestation policy.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1: usize = 256 * 1024;
/// Maximum trusted roots retained by one governed device-attestation policy.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1: usize = 8;
/// Maximum trusted roots retained for either supported platform.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1: usize = 4;
/// Maximum DER bytes accepted for one trusted attestation root.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1: usize = 16 * 1024;
/// Maximum revoked `TBSCertificate` DER SHA-256 digests retained by one policy.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_REVOKED_CERTIFICATES_V1: usize = 256;
/// Maximum iOS application identities retained by one policy.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_APPS_V1: usize = 16;
/// Maximum Android application identities retained by one policy.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_APPS_V1: usize = 16;
/// Maximum iOS validation categories retained for one application.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_VALIDATION_CATEGORIES_V1: usize = 7;
/// Maximum iOS bundle versions retained for one application.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSIONS_V1: usize = 32;
/// Maximum ASCII bytes accepted for one iOS bundle version.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_IOS_BUNDLE_VERSION_BYTES_V1: usize = 128;
/// Maximum Android signing-certificate digests retained for one application.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_ANDROID_SIGNING_CERTIFICATES_V1: usize = 8;
/// Maximum ASCII bytes accepted for one Apple Developer Team ID.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TEAM_ID_BYTES_V1: usize = 64;
/// Maximum ASCII bytes accepted for an iOS bundle ID or Android package name.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_APP_IDENTIFIER_BYTES_V1: usize = 255;
/// Maximum bytes accepted for one platform device identifier.
pub const OFFLINE_DEVICE_ATTESTATION_DEVICE_ID_MAX_BYTES_V1: usize = 128;
/// Maximum bytes accepted for one issuer-scoped platform key identifier.
pub const OFFLINE_DEVICE_ATTESTATION_KEY_ID_MAX_BYTES_V1: usize = 64;
/// Fixed current App Attest assertion header before the mandatory extension CBOR.
pub const KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_FIXED_HEADER_BYTES_V1: usize = 37;
/// Minimum extension-bearing App Attest assertion authenticator-data size.
pub const KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MIN_BYTES_V1: usize =
    KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_FIXED_HEADER_BYTES_V1 + 1;
/// Maximum App Attest assertion authenticator-data size, including mandatory extensions.
pub const KAGEMUSHA_IOS_APP_ATTEST_ASSERTION_AUTH_DATA_MAX_BYTES_V1: usize = 4 * 1024;
