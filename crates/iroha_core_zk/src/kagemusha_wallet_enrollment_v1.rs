//! Native account-authorized enrollment over one exclusive hardware custody provider.
//!
//! Trusted native deployment selects the exact scheme, policies and root original. Foreign
//! calls provide original account/asset/challenge/evidence frames only. The issuer independently
//! authenticates its live challenge, current account eligibility and platform evidence before
//! signing. Retained E5 and E6 bytes survive retries; this module grants no Bootstrap proof
//! capability and never replaces the complete source-qualified wallet-open boundary.

mod carrier;
pub mod issuer_worker;
mod owner;
pub use carrier::{
    EVIDENCE_MAX_BYTES, IssuerEvidenceV1, PlatformEvidenceV1, REQUEST_MAX_BYTES, RESULT_MAX_BYTES,
    RequestBodyV1, RequestV1, ResultV1,
};
pub use owner::{
    EnrollmentConfigV1, EnrollmentOwnerV1, EnrollmentProgressV1, Error, RequestPreparationV1,
};
