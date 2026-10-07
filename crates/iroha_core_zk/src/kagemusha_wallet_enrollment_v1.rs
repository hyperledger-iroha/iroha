//! Native account-authorized enrollment over one exclusive hardware custody provider.
//!
//! Trusted native deployment selects the exact scheme, policies and root original. Foreign
//! calls provide retry identity and original account/asset/permit/evidence frames only. Native retains
//! a client nonce before issuer E1 and checks the rooted permit and live same-boot elapsed budget
//! immediately before hardware generation. The issuer independently
//! authenticates its live challenge, current account eligibility and platform evidence before
//! signing. Retained E5 and E6 bytes survive retries; this module grants no Bootstrap proof
//! capability and never replaces the complete source-qualified wallet-open boundary.

mod carrier;
mod credential;
pub mod issuer_worker;
mod owner;
mod prekey;
pub use carrier::{
    EVIDENCE_MAX_BYTES, IssuerEvidenceV1, PlatformEvidenceV1, REQUEST_MAX_BYTES, RESULT_MAX_BYTES,
    RequestBodyV1, RequestV1, ResultV1,
};
pub use owner::{
    EnrollmentConfigV1, EnrollmentOwnerV1, EnrollmentProgressV1, Error, RequestPreparationV1,
};
pub(crate) use prekey::GenerationAuthorizationV1;
pub use prekey::{PREKEY_DISPATCH_MAX_BYTES, PreKeyDispatchV1};
