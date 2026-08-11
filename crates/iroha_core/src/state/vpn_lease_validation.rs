fn validate_vpn_lease_quote_projection(record: &VpnLeaseRecordV1) -> Result<(), String> {
    record
        .signed_quote
        .verify()
        .map_err(|error| format!("VPN lease retains an invalid operator quote: {error}"))?;
    let quote = &record.signed_quote.body;
    if !record.address_slot.is_valid() {
        return Err(format!(
            "VPN lease {} carries an out-of-range address slot",
            hex::encode(record.lease_id)
        ));
    }
    let canonical_lease_id =
        derive_vpn_lease_id_v1(&quote.network_id, quote.quote_id, &quote.client_account_id);
    if quote.lease_id != canonical_lease_id {
        return Err(format!(
            "VPN lease {} retains a non-canonical network/client/quote id",
            hex::encode(record.lease_id)
        ));
    }
    let canonical_session_id = derive_vpn_session_id_v1(
        &quote.network_id,
        quote.quote_id,
        &quote.client_account_id,
        quote.address_slot,
    );
    if quote.session_id != canonical_session_id {
        return Err(format!(
            "VPN lease {} retains a non-canonical network/client/quote/slot session id",
            hex::encode(record.lease_id)
        ));
    }
    let canonical_addresses = derive_vpn_address_plan_v1(quote.address_slot);
    if quote.policy.tunnel_addresses != canonical_addresses.client_tunnel_addresses {
        return Err(format!(
            "VPN lease {} retains addresses outside its typed slot",
            hex::encode(record.lease_id)
        ));
    }
    let lease_duration_ms = quote
        .expires_at_ms
        .checked_sub(quote.valid_after_ms)
        .ok_or_else(|| {
            format!(
                "VPN lease {} expiry precedes quote validity",
                hex::encode(record.lease_id)
            )
        })?;
    let policy_duration_ms = quote.policy.lease_secs.checked_mul(1_000).ok_or_else(|| {
        format!(
            "VPN lease {} policy duration overflows milliseconds",
            hex::encode(record.lease_id)
        )
    })?;
    if quote.policy.lease_secs == 0 || lease_duration_ms != policy_duration_ms {
        return Err(format!(
            "VPN lease {} validity does not match its signed duration",
            hex::encode(record.lease_id)
        ));
    }
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
    if quote.policy.relay_trust_valid_until_ms < quote.expires_at_ms {
        return Err(format!(
            "VPN lease {} outlives its signed relay trust",
            hex::encode(record.lease_id)
        ));
    }
    if quote.policy.relay_id == [0_u8; 32]
        || quote.policy.descriptor_commit == [0_u8; 32]
        || quote.policy.relay_tls_spki_sha256 == [0_u8; 32]
        || quote.policy.relay_certificate_sha256 == [0_u8; 32]
        || quote.policy.directory_snapshot_digest == [0_u8; 32]
    {
        return Err(format!(
            "VPN lease {} retains an empty relay trust commitment",
            hex::encode(record.lease_id)
        ));
    }
    if quote.policy.fee_asset_id != quote.asset_definition.to_string() {
        return Err(format!(
            "VPN lease {} retains a mismatched fee asset label",
            hex::encode(record.lease_id)
        ));
    }
    let canonical_custody = crate::smartcontracts::isi::vpn::vpn_lease_custody_account_id(
        &quote.network_id,
        &quote.lease_id,
        &quote.asset_definition,
    )
    .map_err(|error| {
        format!(
            "VPN lease {} custody derivation failed: {error}",
            hex::encode(record.lease_id)
        )
    })?;
    if quote.policy.escrow_account_id != canonical_custody {
        return Err(format!(
            "VPN lease {} retains non-canonical protocol custody",
            hex::encode(record.lease_id)
        ));
    }
    if quote.metering_public_key.try_algorithm() != Ok(Algorithm::Ed25519) {
        return Err(format!(
            "VPN lease {} retains a non-Ed25519 V1 metering key",
            hex::encode(record.lease_id)
        ));
    }
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
            let receipt = record.settled_relay_receipt.as_ref().ok_or_else(|| {
                format!(
                    "settled VPN lease {} has no retained relay receipt",
                    hex::encode(record.lease_id)
                )
            })?;
            let receipt_hash = receipt.hash();
            let expected_account_hash =
                *blake3::hash(record.client_account_id.to_string().as_bytes()).as_bytes();
            if record.refunded_at_ms.is_some()
                || record.relay_receipt_hash != Some(receipt_hash)
                || record.client_voucher_hash != Some(receipt.client_voucher_hash)
                || record.highest_voucher_sequence != receipt.highest_voucher_sequence
                || record.earned_fee != receipt.earned_fee
                || receipt.session_id != record.session_id
                || receipt.quote_id != record.quote_id
                || receipt.relay_id != record.relay_id
                || receipt.payment_tx_hash != record.open_tx_hash
                || receipt.account_hash != expected_account_hash
                || receipt.exit_class != record.quote_policy.exit_class
                || receipt.started_at_ms < record.opened_at_ms
                || receipt.ended_at_ms > record.expires_at_ms
                || receipt.ended_at_ms < receipt.started_at_ms
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
