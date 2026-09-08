// Canonical pin lifecycle fixtures with an explicit historical submission epoch.

fn insert_manifest_with_status(
    stx: &mut crate::state::StateTransaction<'_, '_>,
    digest: ManifestDigest,
    chunk_digest: [u8; 32],
    successor_of: Option<ManifestDigest>,
    status: PinStatus,
) {
    insert_manifest_with_status_at_epoch(stx, digest, chunk_digest, successor_of, status, 5);
}

fn insert_manifest_with_status_at_epoch(
    stx: &mut crate::state::StateTransaction<'_, '_>,
    digest: ManifestDigest,
    chunk_digest: [u8; 32],
    successor_of: Option<ManifestDigest>,
    status: PinStatus,
    submitted_epoch: u64,
) {
    let policy = default_policy();
    let content_length = default_content_length();
    let mut record = PinManifestRecord::new(
        digest,
        root_cid_for_manifest(digest),
        default_chunker(),
        chunk_digest,
        por_root_for_manifest(digest),
        content_length,
        policy,
        alice(),
        submitted_epoch,
        None,
        successor_of,
        Metadata::default(),
    );
    match status {
        PinStatus::Pending => {}
        PinStatus::Approved(epoch) => {
            let amount = stx
                .world
                .sorafs_pricing
                .get()
                .public_pin_fee(
                    policy.storage_class,
                    content_length,
                    policy.min_replicas,
                    submitted_epoch,
                    policy.retention_epoch,
                )
                .expect("fixture public pin fee");
            record.record_pin_fee_payment(PinFeePayment {
                paid_by: alice(),
                fee_asset_id: stx.gov.sorafs_pin_fee_asset_id.clone(),
                treasury_account_id: stx.gov.sorafs_pin_fee_treasury_account.clone(),
                amount,
            });
            record.approve(epoch, None);
        }
        PinStatus::Retired(epoch) => record.retire(epoch, None),
    }
    validate_stored_pin_approval_history(&record, &manifest_hex(&digest))
        .expect("pin fixture preserves its immutable submission and approval history");
    insert_pin_record_with_accounting(stx, record);
}
