//! Platform-gated JNI exports and their Java conversion helpers.
#![cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    windows
))]
#![allow(clippy::missing_safety_doc)]
use super::*;
use iroha_data_model::{
    isi::privacy::SubmitPrivacyProofV1,
    privacy::{
        PRIVACY_CAPABILITY_ARCHIVE_MAX_BYTES_V1, PrivacyCapabilityArchiveValidationStatusV1,
        PrivacyExact12CapabilityManifestV1, TAIRA_PRIVACY_MAX_ACTION_BYTES_V1,
        validate_privacy_capability_archive_v1,
    },
};
include!("platform_jni/part_1.rs");
include!("platform_jni/part_2.rs");
include!("platform_jni/part_3.rs");
