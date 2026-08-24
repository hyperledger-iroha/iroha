fn validate_vpn_lease_quote_projection(record: &VpnLeaseRecordV1) -> Result<(), String> {
    crate::smartcontracts::isi::vpn::validate_static_vpn_quote(&record.signed_quote).map_err(
        |error| {
            format!(
                "VPN lease {} retains an invalid static quote: {error}",
                hex::encode(record.lease_id)
            )
        },
    )?;
    record
        .signed_quote
        .verify()
        .map_err(|error| format!("VPN lease retains an invalid operator quote: {error}"))?;
    let quote = &record.signed_quote.body;
    if record.opened_at_ms < quote.valid_after_ms || record.opened_at_ms >= quote.expires_at_ms {
        return Err(format!(
            "VPN lease {} opened outside its signed quote validity interval",
            hex::encode(record.lease_id)
        ));
    }
    let refund_available_at_ms = quote
        .expires_at_ms
        .checked_add(quote.settlement_grace_ms)
        .filter(|_| quote.settlement_grace_ms != 0)
        .ok_or_else(|| {
            format!(
                "VPN lease {} has an invalid settlement grace window",
                hex::encode(record.lease_id)
            )
        })?;
    if record.lease_id != quote.lease_id
        || record.session_id != quote.session_id
        || record.quote_id != quote.quote_id
        || record.client_account_id != quote.client_account_id
        || record.operator_account_id != quote.operator_account_id
        || record.metering_public_key != quote.metering_public_key
        || record.asset_definition != quote.asset_definition
        || record.lease_fee != quote.tariff.lease_fee
        || record.relay_id != quote.policy.relay_id
        || record.tariff != quote.tariff
        || record.quote_policy != quote.policy
        || record.custody_account_id != quote.policy.escrow_account_id
        || record.address_slot != quote.address_slot
        || record.expires_at_ms != quote.expires_at_ms
        || record.settlement_grace_ms != quote.settlement_grace_ms
    {
        return Err(format!(
            "VPN lease {} does not match its signed quote projection",
            hex::encode(record.lease_id)
        ));
    }
    match record.status {
        VpnLeaseStatusV1::Active => {
            if record.settled_at_ms.is_some()
                || record.refunded_at_ms.is_some()
                || record.highest_voucher_sequence != 0
                || record.client_voucher_hash.is_some()
                || record.settled_client_voucher.is_some()
                || record.relay_receipt_hash.is_some()
                || record.settled_relay_receipt.is_some()
                || !record.earned_fee.is_zero()
                || !record.refunded_fee.is_zero()
            {
                return Err(format!(
                    "active VPN lease {} carries terminal settlement state",
                    hex::encode(record.lease_id)
                ));
            }
        }
        VpnLeaseStatusV1::Settled => {
            let settled_at_ms = record.settled_at_ms.ok_or_else(|| {
                format!(
                    "settled VPN lease {} has no settlement timestamp",
                    hex::encode(record.lease_id)
                )
            })?;
            let signed_receipt = record.settled_relay_receipt.as_ref().ok_or_else(|| {
                format!(
                    "settled VPN lease {} has no retained relay receipt",
                    hex::encode(record.lease_id)
                )
            })?;
            let voucher = record.settled_client_voucher.as_ref().ok_or_else(|| {
                format!(
                    "settled VPN lease {} has no retained client voucher",
                    hex::encode(record.lease_id)
                )
            })?;
            let verified_earned_fee = record
                .verify_settlement_evidence(signed_receipt, voucher)
                .map_err(|error| {
                    format!(
                        "settled VPN lease {} has invalid retained settlement evidence: {error}",
                        hex::encode(record.lease_id)
                    )
                })?;
            let receipt_hash = signed_receipt.hash();
            let receipt = &signed_receipt.receipt;
            if record.refunded_at_ms.is_some()
                || record.relay_receipt_hash != Some(receipt_hash)
                || record.client_voucher_hash != Some(voucher.hash())
                || record.highest_voucher_sequence != voucher.body.sequence
                || record.earned_fee != verified_earned_fee
                || settled_at_ms < receipt.ended_at_ms
                || settled_at_ms >= refund_available_at_ms
            {
                return Err(format!(
                    "settled VPN lease {} does not match its retained receipt",
                    hex::encode(record.lease_id)
                ));
            }
            let accounted_fee = record
                .earned_fee
                .checked_add(&record.refunded_fee)
                .map_err(|error| {
                    format!(
                        "settled VPN lease {} fee accounting overflows: {error}",
                        hex::encode(record.lease_id)
                    )
                })?;
            if accounted_fee != record.lease_fee {
                return Err(format!(
                    "settled VPN lease {} does not conserve escrowed fees",
                    hex::encode(record.lease_id)
                ));
            }
        }
        VpnLeaseStatusV1::Refunded => {
            let refunded_at_ms = record.refunded_at_ms.ok_or_else(|| {
                format!(
                    "refunded VPN lease {} has no refund timestamp",
                    hex::encode(record.lease_id)
                )
            })?;
            if refunded_at_ms < refund_available_at_ms
                || record.settled_at_ms.is_some()
                || record.highest_voucher_sequence != 0
                || record.client_voucher_hash.is_some()
                || record.settled_client_voucher.is_some()
                || record.relay_receipt_hash.is_some()
                || record.settled_relay_receipt.is_some()
                || !record.earned_fee.is_zero()
                || record.refunded_fee != record.lease_fee
            {
                return Err(format!(
                    "refunded VPN lease {} carries inconsistent terminal state",
                    hex::encode(record.lease_id)
                ));
            }
        }
    }
    Ok(())
}
fn validate_vpn_lease_network(
    record: &VpnLeaseRecordV1,
    expected_network_id: &iroha_data_model::NetworkId,
) -> Result<(), String> {
    if &record.signed_quote.body.network_id != expected_network_id {
        return Err(format!(
            "VPN lease {} belongs to a different exact network",
            hex::encode(record.lease_id)
        ));
    }
    Ok(())
}
fn validate_vpn_lease_transition(
    previous: &VpnLeaseRecordV1,
    next: &VpnLeaseRecordV1,
) -> Result<(), String> {
    if previous.signed_quote != next.signed_quote
        || previous.open_tx_hash != next.open_tx_hash
        || previous.opened_at_ms != next.opened_at_ms
    {
        return Err(format!(
            "VPN lease {} cannot replace its immutable opening",
            hex::encode(next.lease_id)
        ));
    }
    match (previous.status, next.status) {
        (VpnLeaseStatusV1::Active, VpnLeaseStatusV1::Settled | VpnLeaseStatusV1::Refunded) => {
            Ok(())
        }
        (left, right) if left == right && previous == next => Ok(()),
        (VpnLeaseStatusV1::Settled | VpnLeaseStatusV1::Refunded, VpnLeaseStatusV1::Active) => {
            Err(format!(
                "VPN lease {} cannot transition from a terminal status back to active",
                hex::encode(next.lease_id)
            ))
        }
        (left, right) => Err(format!(
            "VPN lease {} has invalid lifecycle transition {left:?} -> {right:?}",
            hex::encode(next.lease_id)
        )),
    }
}
