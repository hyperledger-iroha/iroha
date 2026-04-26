use super::*;
use crate::offline::{OfflineNoteAuditBundleV2, OfflineNoteIssueV2, OfflineNoteRedeemV2};

isi! {
    /// Issue a production Offline V2 bearer note.
    pub struct IssueOfflineNoteV2 {
        /// Compact note issuance record.
        pub issue: OfflineNoteIssueV2,
    }
}

isi! {
    /// Redeem a production Offline V2 bearer note token.
    pub struct RedeemOfflineNoteV2 {
        /// Compact recursive proof and consumed nullifiers.
        pub redemption: OfflineNoteRedeemV2,
    }
}

isi! {
    /// Submit an optional Offline V2 audit bundle without requiring online settlement for finality.
    pub struct AuditOfflineNoteV2 {
        /// Compact audit payload.
        pub audit: OfflineNoteAuditBundleV2,
    }
}

impl crate::seal::Instruction for IssueOfflineNoteV2 {}
impl crate::seal::Instruction for RedeemOfflineNoteV2 {}
impl crate::seal::Instruction for AuditOfflineNoteV2 {}

impl IssueOfflineNoteV2 {
    /// Construct an Offline V2 note issuance instruction.
    #[must_use]
    pub fn new(issue: OfflineNoteIssueV2) -> Self {
        Self { issue }
    }
}

impl RedeemOfflineNoteV2 {
    /// Construct an Offline V2 note redemption instruction.
    #[must_use]
    pub fn new(redemption: OfflineNoteRedeemV2) -> Self {
        Self { redemption }
    }
}

impl AuditOfflineNoteV2 {
    /// Construct an Offline V2 optional audit instruction.
    #[must_use]
    pub fn new(audit: OfflineNoteAuditBundleV2) -> Self {
        Self { audit }
    }
}
