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
/// Maximum canonical Norito bytes for one governed device-attestation policy.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_CANONICAL_BYTES_V1: usize = 64 * 1024;
/// Maximum trusted roots retained by one governed device-attestation policy.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_V1: usize = 8;
/// Maximum trusted roots retained for either supported platform.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOTS_PER_PLATFORM_V1: usize = 4;
/// Maximum DER bytes accepted for one trusted attestation root.
pub const OFFLINE_DEVICE_ATTESTATION_POLICY_MAX_TRUSTED_ROOT_DER_BYTES_V1: usize = 16 * 1024;
/// Maximum revoked-certificate SHA-256 digests retained by one policy.
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
