//! Canonical KAGEMUSHA V1 wire and authenticated release data model.
//!
//! KAGEMUSHA V1 defines aggregate-balance cash data and authenticated release selection.
//! Hardware terminal data and ordinary app approval with separate Native financial custody
//! have distinct wire families; neither data decoding nor an app signature grants money.

pub mod kagemusha_app_enrollment_possession_v1;
pub mod kagemusha_app_enrollment_v1;
pub mod kagemusha_app_operation_approval_v1;
pub mod kagemusha_device_response_v1;
pub mod kagemusha_device_v1;
pub mod kagemusha_enrolled_open_selector_v1;
pub mod kagemusha_mobile_bootstrap_freshness_v1;
pub mod kagemusha_mobile_bootstrap_v1;
pub mod kagemusha_ordinary_app_enrollment_v1;
pub mod kagemusha_ordinary_cash_v1;
pub mod kagemusha_ordinary_issuer_circuit_admission_v1;
pub mod kagemusha_ordinary_retail_enrollment_v1;
pub mod kagemusha_play_integrity_provider_policy_v1;
pub mod kagemusha_play_integrity_refresh_v1;
pub mod kagemusha_raw_app_attestation_admission_v1;
pub mod kagemusha_release_v1;
pub mod kagemusha_retail_enrollment_challenge_v1;
pub mod kagemusha_retail_enrollment_v1;
pub mod kagemusha_v1;
pub mod verifier_registry_v1;

pub use self::{
    kagemusha_app_enrollment_possession_v1::*, kagemusha_app_enrollment_v1::*,
    kagemusha_app_operation_approval_v1::*, kagemusha_device_response_v1::*,
    kagemusha_device_v1::*, kagemusha_enrolled_open_selector_v1::*,
    kagemusha_mobile_bootstrap_freshness_v1::*, kagemusha_mobile_bootstrap_v1::*,
    kagemusha_ordinary_app_enrollment_v1::*, kagemusha_ordinary_cash_v1::*,
    kagemusha_ordinary_issuer_circuit_admission_v1::*, kagemusha_ordinary_retail_enrollment_v1::*,
    kagemusha_play_integrity_provider_policy_v1::*, kagemusha_play_integrity_refresh_v1::*,
    kagemusha_raw_app_attestation_admission_v1::*, kagemusha_release_v1::*,
    kagemusha_retail_enrollment_challenge_v1::*, kagemusha_retail_enrollment_v1::*,
    kagemusha_v1::*, verifier_registry_v1::*,
};

/// Prefix embedded into KAGEMUSHA V1 instruction rejection messages.
///
/// Torii extracts the label following this prefix as a stable machine-readable
/// error code.
pub const KAGEMUSHA_V1_REJECTION_REASON_PREFIX: &str = "kagemusha_v1_reason::";
