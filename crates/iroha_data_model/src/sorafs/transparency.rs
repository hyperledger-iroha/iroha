//! Canonical SoraFS transparency payloads.
//!
//! The wire schemas and validators live in [`sorafs_manifest::transparency`]
//! so manifest validation and the data model share one implementation.

pub use sorafs_manifest::transparency::*;

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use super::*;

    #[test]
    fn data_model_reexport_has_exact_manifest_type_and_wire_identity() {
        assert_eq!(
            TypeId::of::<ModerationLedgerEntryV1>(),
            TypeId::of::<sorafs_manifest::transparency::ModerationLedgerEntryV1>()
        );
        let entry = ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id: [0x11; 16],
            entry_id: [0x12; 16],
            sequence: 7,
            occurred_at_unix: 1_800_000_001,
            kind: ModerationLedgerEntryKindV1::GarEnforcementReceipt,
            subject: "gar-receipt-7".to_owned(),
            subject_digest: [0x21; 32],
            payload_digest: [0x22; 32],
            summary_digest: [0x23; 32],
            policy_digest: Some([0x24; 32]),
            evidence_uris: vec!["sora://transparency/gar-receipt-7".to_owned()],
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "source".to_owned(),
                value: "gar".to_owned(),
            }],
        };
        entry
            .validate()
            .expect("reexported validator accepts entry");

        let data_model_bytes = norito::to_bytes(&entry).expect("encode through reexport");
        let canonical: sorafs_manifest::transparency::ModerationLedgerEntryV1 = entry;
        let manifest_bytes = norito::to_bytes(&canonical).expect("encode canonical type");
        assert_eq!(data_model_bytes, manifest_bytes);
    }
}
